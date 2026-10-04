---
name: add-obsbot-camera
description: Add or extend support for an OBSBOT camera model in OBSCura (new USB ID, new profile, or new vendor features for an existing model such as gestures, AI modes or audio). Use when the user connects an unsupported OBSBOT camera, asks to support a new model, or wants a feature that a model's profile doesn't bind yet. Covers identifying the camera, writing the profile, finding commands (SDK trace or captures), testing each feature on the real camera, and shipping.
---

# Adding an OBSBOT camera or feature

OBSCura supports a model through **data, not code**: a TOML profile in
`profiles/` maps each catalog feature (`crates/obsbot-core/src/features.rs`)
to a transport binding. Code changes are only needed when a binding needs
something the profile format can't express yet.

Work **with the user**: most steps need them to watch the camera, make a
gesture, or run `sudo`. Tell them what you're about to send and what they
should see.

## Ground rules (learned the hard way)

- **Read before you write.** Start with read-only steps (descriptors,
  status block, queries already used for other models). Unknown framed
  commands, and write command IDs sent as empty queries, have crashed
  cameras; they reboot and drop off USB for ~40 s.
- **"Accepted" is not evidence.** A Tiny 3 accepts Tiny SE commands and
  silently ignores them. Every feature needs an **oracle**: a status byte, a
  query reply, `obsbotctl set … --check`, the status light, or the user
  watching the picture.
- **Restore the user's settings** after each test, and say what you
  changed. Note the starting state first (`obsbotctl dump`,
  `obsbotctl status`).
- **Never ship a command that wasn't tested on the camera.** If something
  can't be tested (e.g. no mic to pair), record it in `TODO.md` instead.
- The SDK's `@category` notes are out of date: functions documented for
  other models can be exactly what a newer model uses.
- No SDK code or files in the repo. Only our own tools
  (`tools/sdk-trace/`) and what we measured.

## Things that look like bugs but aren't

- **Auto-sleep** puts the camera to sleep after ~2 min without a stream,
  and a sleeping camera ignores most writes (zoom, Human tracking). Check
  `obsbotctl status` (Power). For long tests: `obsbotctl set auto_sleep
  off`, and switch it back on afterwards.
- **Some features need a video stream**: on the Tiny 3, Human tracking and
  gesture recognition only work while something streams (the app's
  preview, OBS, or `timeout 60 v4l2-ctl -d /dev/videoN
  --set-fmt-video=width=1280,height=720,pixelformat=MJPG -p 15
  --stream-mmap --stream-to=/dev/null`). Only one app can stream at a time,
  so ask before taking the camera from the user.
- **Status lag**: the status block reflects a write ~1–2.5 s later
  (`STATUS_LAG` in `crates/obsbot-core/src/device/mod.rs`).
- **Gestures get missed** now and then; the status light blinks twice when
  one is recognised. Ask the user to repeat before concluding anything.
- A snap-confined process can't be killed from outside, even with sudo
  (AppArmor). Use `snap run --shell obscura.obsbotctl -c 'kill <pid>'`.

## 1. Identify the camera (read-only)

```sh
lsusb | grep -i 3564                       # USB ID and product name
v4l2-ctl --list-devices                    # its /dev/videoN
lsusb -v -d 3564:<pid> | grep -A14 "VideoControl Interface" \
  | grep -E "Subtype|UnitID|guidExtension|bNumControls|bmControls"
v4l2-ctl -d /dev/videoN -L                 # standard controls and menus
tools/sdk-trace/xustatus.py /dev/videoN --raw   # selector 6 status block
```

- Extension Unit **2** with GUID `9a1e7291-6843-4683-6d92-39bc7906ee49` and
  60-byte selectors means the same vendor protocol as the existing models
  (`docs/protocol.md`).
- Compare the status block with the **"Status block layout"** table in
  `docs/protocol.md` (from the SDK's `CameraStatus::tiny`). Plausible
  values at the known offsets (e.g. [9] = 1 awake, [10..12] = sleep time,
  [24] = AI mode, [33] = light level) mean the layout matches.
- Check the `auto_exposure` menu values: profiles override them when they
  differ from the standard (see `tiny-se.toml`).
- Find the model in the SDK's `ObsbotProductType` (`dev.hpp`) if the user
  has the SDK.

## 2. Write the profile

1. Create `profiles/<model>.toml` that **inherits** the closest profile
   (`tiny-3` for the Tiny 3 family, `tiny-se` for older Tinys, `meet-2`
   without a gimbal) and only adds what differs: `id`, `name`, `[[match]]`
   `product_id`, overrides, and `drop = [...]` for inherited parts the
   model lacks (see `tiny-3-lite.toml`, `meet-2.toml`). Inheritance is
   recursive. Write in the header comment what was verified and how.
2. Register it in `BUILTIN` in `crates/obsbot-core/src/profile.rs` and
   add a test next to `tiny_3_matches_by_product_id`.
3. Add it to the "Supported cameras" table in `README.md`, honestly
   labelled ("assumed", "status block checked", "supported except …").
4. Add a `[firmware]` section (`page`, `key`) if the model has an OBSBOT
   download page. The key is the model's entry in the page's embedded data
   (`<key>:{firmware:{… version:"v…"}}`); `obsbotctl firmware` tests it.

Binding options worth knowing (all in `profile.rs`, `VendorBinding`):
`frames` (selector 2 commands with `prefix`, `value` encoding, `flags`,
fixed `payload`), `short` (selector 6 `[id, len, value]`), `status`
offset with `status_value`/`status_mask`, `query` readback (with `flags`,
`payload`, `offsets`, `value = "f32"`, `scale`), `invert`, `level` +
`default_level` + `level_status` for switch/level pairs, `transient`
status values, and `options` to replace a choice list for one model.

## 3. Verify every feature

Build the CLI (`cargo build -p obsbotctl`) and use
`./target/debug/obsbotctl -d /dev/videoN`:

1. `dump`: every value should be plausible. Anything odd (e.g. 0.00x)
   means a binding's layout differs on this model.
2. For each writable feature, in this order: reversible ones first
   (mirror, status light, HDR), then the rest. Use
   `set <feature> <value> --check` and set it back. `--check` waits out
   the status lag and warns when the camera reports something else.
   `OBSCURA_TRACE=1` prints every frame sent and read.
3. For features without a readback (gestures, gimbal moves), agree on an
   oracle with the user first (light blinks, picture moves) and test one
   change at a time, with a baseline before and after.
4. Unbind (leave out of the profile) anything that doesn't work, with a
   comment saying what was tried. The app then shows it greyed out.

## 4. When a feature's command is unknown

In order of preference:

1. **Existing notes**: `docs/protocol.md`, `TODO.md`, other models'
   profiles, and public projects (e.g. joshualambert/obsbot-tiny3-linux,
   brendanwelsh/obsbot-tiny3-protocol, cgevans/tiny2). Treat them as
   leads, not proof.
2. **SDK trace** (if the user has OBSBOT's SDK): see
   `tools/sdk-trace/README.md`. Build with
   `SDK=<unpacked libdev> tools/sdk-trace/build.sh`, run a driver
   (`gprobe`, `micprobe`, or a new one in the same style) under
   `LD_PRELOAD=…/xulog.so`, and read the frames between the `===`
   markers. Trace reads before writes. Functions missing from the header
   are often exported anyway: `nm -D -C --defined-only libdev.so.1.0.0`.
3. **Windows capture** of OBSBOT Center: `docs/CAPTURING.md`,
   `tools/obsbot_pcap.py`, and add what's needed to `captures/TODO.md`.
4. **Probing** only as a last resort, with the user's explicit OK, using
   commands from a related model, one at a time, with an oracle.

Then send the frame from Linux (`tools/sdk-trace/xuframe.py … --send`,
or a profile binding) and confirm it with the oracle before shipping.

## 5. Ship

```sh
cargo fmt --all --check
cargo clippy -p obsbot-core -p obsbotctl -p obscura-indicator --all-features -- -D warnings
cargo test -p obsbot-core --all-features
(cd app && pnpm install --frozen-lockfile && pnpm check && pnpm build)
```

- Add a byte-exact test in `vendor/protocol.rs` for each new frame encoding,
  using the traced or captured bytes.
- Record measurements in `docs/protocol.md` (a section per model when it
  differs), tick or add items in `TODO.md`, and update `README.md`.
- To try it in the installed app, rebuild the snap. The `obscura` part
  must be cleaned first or snapcraft packs the previous build:
  `sg lxd -c "snapcraft clean obscura --use-lxd" && sg lxd -c
  "snapcraft pack --use-lxd"` (only committed files are built;
  `tools/bootstrap.sh snap` fixes LXD networking next to Docker). The user
  installs it with `sudo snap install --dangerous <file>.snap`.
- Commit with a message saying what was measured on which firmware.
