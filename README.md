# OBSCura

[![CI](https://github.com/kenvandine/obscura/actions/workflows/ci.yml/badge.svg)](https://github.com/kenvandine/obscura/actions/workflows/ci.yml)
[![Snap](https://github.com/kenvandine/obscura/actions/workflows/snap.yml/badge.svg)](https://github.com/kenvandine/obscura/actions/workflows/snap.yml)

**OBSCura: Unofficial control panel for OBSBOT webcams.**

A Linux configuration tool for OBSBOT webcams: AI tracking,
gesture control, gimbal, mirror and image settings, microphones (including
OBSBOT's Vox SE wireless mics), with a live preview and a diagnostics view.
Built with Rust, Tauri 2 and Svelte.

OBSBOT's own *OBSBOT Center* only runs on Windows and macOS. On Linux the
cameras work as plain UVC webcams, but their AI and gimbal features can't be
changed. This project talks to the camera the same way the official app does,
using a vendor protocol reverse-engineered from USB captures.

> **Not affiliated with OBSBOT / Remo Tech.** "OBSBOT" is their trademark.
> This project contains no vendor code and was written without vendor
> documentation. The protocol comes from watching OBSBOT's own software
> talk to the camera: OBSBOT Center's USB traffic, and for the Tiny 3's
> gesture settings, the traffic of OBSBOT's SDK. Use it at your own risk.

<p align="center">
  <img src="screenshots/obscura1.png" width="48%" alt="OBSCura main window">
  <img src="screenshots/obscura2.png" width="48%" alt="OBSCura image settings">
</p>

## Contents

- [Supported cameras](#supported-cameras)
- [Features](#features)
- [Installing](#installing)
- [Using the app](#using-the-app)
- [Using the CLI (`obsbotctl`)](#using-the-cli-obsbotctl)
- [How it works](#how-it-works)
- [Adding support for another camera](#adding-support-for-another-camera)
- [Development](#development)
- [Troubleshooting](#troubleshooting)
- [License](#license)

## Supported cameras

| Camera | USB ID | Status |
|---|---|---|
| OBSBOT Tiny SE | `3564:feff` | Supported (firmware 6.4.4.1) |
| OBSBOT Tiny 3 | `3564:ff02` | Supported, including Vox SE wireless mics (firmware 6.6.8.3) |
| OBSBOT Tiny 3 Lite | `3564:ff04` | Assumed like the Tiny 3, unverified |
| OBSBOT Tiny 2 | `3564:fef8` | Assumed like the Tiny SE, cross-checked (see below) |
| OBSBOT Tiny 2 Lite | `3564:fef9` | Assumed like the Tiny SE, unverified |
| OBSBOT Tiny 4K | `3564:fef4` | Assumed like the Tiny SE, unverified |
| OBSBOT Meet 2 | `3564:fefb` | Assumed like the Tiny SE without the gimbal, unverified |
| OBSBOT Meet SE | `3564:fefe` | Assumed like the Meet 2, unverified |
| Other OBSBOT models | `3564:*` | Standard UVC controls only |
| Any other UVC webcam | – | Standard UVC controls, with `--all` / `OBSBOT_ALL_CAMERAS=1` |

The **Tiny SE** and **Tiny 3** were tested on real cameras: the Tiny SE from
OBSBOT Center captures, the Tiny 3 from Linux, with the commands it doesn't
share with the Tiny SE (gestures, extra AI modes, Vox SE) recorded from
OBSBOT's SDK. The Tiny 3's presets store their values differently and their
names can't be read back, so its presets are numbered (Preset 1–3).

The other models' profiles **inherit** a tested one (`profiles/*.toml`): the
Tiny 2 family, Tiny 4K and Meet line the Tiny SE's, the Tiny 3 Lite the
Tiny 3's. That's unverified per model, but not a blind guess: OBSBOT's own
SDK groups all of them under one status-block layout (a different one from
the plain Meet/Meet 4K), and [cgevans/tiny2](https://github.com/cgevans/tiny2),
an independent reverse-engineering of the Tiny 2, confirms several of the
same command IDs. The Meet 2 and Meet SE have no mechanical gimbal, so their
profiles leave out gimbal velocity, presets and gimbal reset/reverse.

Other OBSBOT models (Meet, Meet 4K, …) get the standard webcam controls;
their vendor features stay off until someone records their traffic, since
command IDs and status layouts differ between models. If something doesn't
work, or you can record a model's traffic, see
[Adding support for another camera](#adding-support-for-another-camera).

## Features

| Area | Feature | Tiny SE | Tiny 3 |
|---|---|---|---|
| AI tracking | Human / Group / Hand Tracking, AI lock | ✅ | ✅ |
| AI tracking | Whiteboard, Desk, Voice Tracking modes | – | ✅ |
| Gesture control | Master switch, Locked Target (palm), Zoom ("L"), Dynamic Zoom, Zoom Factor (1–4×) | ✅ | ✅ |
| Gesture control | Direction Flip | ✅ | – |
| View & gimbal | Joystick (velocity), view reset, manual zoom (1–4×) | ✅ | ✅ |
| View & gimbal | Field of view (86° / 78° / 65°), preset speed | – | ✅ |
| AI tracking | Tracking speed (5 steps), motion mode, sound-assisted tracking | – | ✅ |
| View & gimbal | View and Gimbal Reverse (upside-down mounting) | ✅ | – ² |
| View & gimbal | Reverse pan control (joystick) | – | ✅ |
| View & gimbal | Presets: save and recall 3 camera-side presets (also in the tray menu); rename on the Tiny SE, delete on the Tiny 3 | ✅ | ✅ |
| Image | Mirror image (on the camera, so every app sees it) | ✅ | ✅ |
| Image | Brightness, contrast, saturation, sharpness, hue | ✅ | ✅ |
| Image | Focus, exposure and white balance, each Auto / Manual; gain, anti-flicker | ✅ | ✅ |
| Image | HDR, Global/Face AF and AE modes, exposure compensation | ✅ | ✅ |
| Audio | Microphone on/off and level (the camera's USB audio, any webcam) | ✅ | ✅ |
| Audio | Mic during sleep, noise reduction, auto gain, disable microphone, pickup distance | ✅ | ✅ |
| Audio | Vox SE wireless mics: pair, forget, battery, mute and gain per mic, button function and lock, mic LED, audio source | – | ✅ |
| Audio | Voice control: switch each spoken command, language, voice zoom | – | ✅ |
| Device | Sleep/resume, auto sleep and sleep time, sleep background mirror | ✅ | ✅ |
| Device | Status light and brightness, firmware version | ✅ | ✅ |
| Device | Firmware update check against OBSBOT's download page (on request) | – | ✅ |
| Diagnostics | Live camera state, activity log, camera events (target lost/found, gesture and voice changes) | ✅ | ✅ |
| Preview | Live MJPEG preview, 1080p/720p at 30/60 fps | ✅ | ✅ |
| Device | Hot-plug and reboot recovery | ✅ | ✅ |
| Panel | Tray indicator with quick toggles and show/hide | ✅ | ✅ |
| Pending captures | Custom sleep background, factory reset | 🚧 | 🚧 |
| Out of scope | Beauty/background effects, portrait mode, rotate/flip, recording, firmware updates | ❌ ¹ | ❌ ¹ |

¹ In OBSBOT Center these effects (and portrait mode, rotate and flip) run on
the PC, not the camera. Firmware flashing is deliberately never implemented:
the update check only points you to OBSBOT Center.

² The Tiny SE's command was never tested on a Tiny 3, and OBSBOT's SDK uses a different one there; left out until it's checked (see `TODO.md`).

Controls that aren't supported yet are still shown, greyed out, with a tooltip
explaining why. The layout follows the official app.

## Installing

[![Get it from the Snap Store](https://snapcraft.io/en/dark/install.svg)](https://snapcraft.io/obscura)

[![obscura](https://snapcraft.io/obscura/badge.svg)](https://snapcraft.io/obscura)

### Snap (strictly confined)

```sh
sudo snap install obscura
sudo snap connect obscura:camera                 # required
sudo snap connect obscura:hardware-observe       # optional: USB product name/serial
sudo snap connect obscura:alsa                   # optional: microphone level/switch
sudo snap alias obscura.obsbotctl obsbotctl      # optional: plain `obsbotctl`
```

The snap uses the GNOME 46 runtime (which provides WebKitGTK) and never
needs raw USB access. Interfaces: `camera` (required) reaches the camera;
`hardware-observe` shows its USB details; `alsa` sets the microphone's
on/off and level; `network` is only used when you check for a firmware
update (it's connected automatically).

### From source

#### 1. Dependencies

Rust (stable, 1.77.2+), Node.js 20.19+ or 22.12+, [pnpm](https://pnpm.io/), and the
[Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux).
On Debian/Ubuntu:

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
    libasound2-dev
```

Or let `tools/bootstrap.sh dev` check all of this and install what's
missing (apt packages, Rust via rustup, Node 22 via snap, pnpm). Add
`--check` to only report.

#### 2. Build

```sh
git clone https://github.com/kenvandine/obscura.git
cd obscura

# CLI only
cargo build --release -p obsbotctl        # -> target/release/obsbotctl
cargo build --release -p obscura-indicator  # -> target/release/obscura-indicator

# Desktop app (+ .deb and AppImage bundles)
cd app
pnpm install
pnpm tauri build                         # -> target/release/obscura
                                         #    target/release/bundle/{deb,appimage}/
```

#### 3. Permissions

Everything goes through `/dev/videoN`, so no root access, udev rules or kernel
modules are needed. On desktop distributions the logged-in user can already
open the camera. On other setups, add yourself to the `video` group:

```sh
sudo usermod -aG video "$USER"   # then log out and back in
```

## Using the app

Run `obscura` (or `pnpm tauri dev` from `app/` during development).

- **Top bar:** camera picker, the Gesture Control popover, the Voice
  Control popover (loudspeaker button, red while any spoken command is on;
  Tiny 3 series), the mirror
  popover (Mirror Image on the camera, Mirror Preview only in OBSCura, and
  Share as "OBSCura Camera"), Sleep/Resume, and Open/Close Preview.
- **Preview:** a live view from the camera. Choose resolution and frame rate
  from the format button at the bottom left. **Mirror Preview** (in the
  top bar's mirror popover) shows it mirrored, only in OBSCura; other apps
  still get the normal picture. **Share** in the same popover lets other
  apps use the camera at the same time (see
  [Sharing the camera](#sharing-the-camera-with-other-apps)).
- **Bottom bar:** AI tracking modes (Human / Group / Hand Tracking, plus
  Whiteboard / Desk / Voice Tracking on the Tiny 3) and the AI lock.
- **Image tab:** presets, the gimbal joystick and zoom, and image adjustments.
- **Audio tab:** the camera's microphone (on/off, level, noise reduction,
  auto gain, pickup distance, disable, during sleep) and, on the Tiny 3
  series, **Wireless Microphones**: two Vox SE slots with battery, mute and
  gain, guided pairing (press Pair, then hold the mic's button for 6 s) and
  the audio source.
- **More tab:** device sleep, status light, the panel indicator, other
  settings, and device details (with a firmware update check).
- **Ctrl+Shift+D** opens the **Diagnostics** window: the camera's live state
  and an activity log (see [Troubleshooting](#troubleshooting)).

Settings are written to the camera itself, so they apply to every app that
uses it (Zoom, Meet, OBS, …). You don't need to keep OBSCura running.

Only one application can stream from the camera at a time. **Close the preview
before joining a call**, or switch on **Share** (below); the controls keep
working while another app is streaming.

### Sharing the camera with other apps

**Share as "OBSCura Camera"** (in the top bar's mirror popover) passes the camera to a
virtual camera, **OBSCura Camera**, that OBS, Teams, Zoom or a browser can
use while OBSCura shows the preview. Pick "OBSCura Camera" as the camera in
those apps. Sharing runs in the background: it keeps going when the preview
or OBSCura's window is closed, until you switch it off in OBSCura or choose
**Stop sharing** in the panel indicator's menu. It shares the preview's
resolution and frame rate (changing them restarts the share).

One app at a time can stream from "OBSCura Camera" (a v4l2loopback
limitation): a second one gets "device busy" until the first lets go.
OBSCura's own preview doesn't count, since it gets the frames directly from
the share process.

The virtual camera comes from the `v4l2loopback` kernel module, which needs
administrator rights to load, once:

```sh
tools/bootstrap.sh virtual-camera   # installs, loads, and loads at every boot
```

or by hand:

```sh
sudo apt install v4l2loopback-dkms     # if `modinfo v4l2loopback` finds nothing
sudo modprobe v4l2loopback devices=1 video_nr=42 card_label="OBSCura Camera" exclusive_caps=1
# and to load it at every boot:
echo v4l2loopback | sudo tee /etc/modules-load.d/obscura-camera.conf
echo 'options v4l2loopback devices=1 video_nr=42 card_label="OBSCura Camera" exclusive_caps=1' | sudo tee /etc/modprobe.d/obscura-camera.conf
```

If it isn't loaded, switching on Share shows these commands. From the
command line, `obsbotctl share` does the same as the switch.

### Panel indicator

`obscura-indicator` (`obscura.indicator` in the snap) is a tiny tray icon,
using about 4 MiB and showing the app's icon, that you can leave running all
the time. Its menu offers:

- **Show / Hide OBSCura**: starts the full app, and quits it again, so the
  heavy GUI only uses memory while it's open.
- **AI tracking**: the camera's modes (Off, Human, Group, Hand tracking, and
  Whiteboard, Desk and Voice Tracking on the Tiny 3).
- **Gesture control** and **Mirror image** on/off.
- **Re-center camera**, and **Presets** to move to one stored on the camera.
- **Start at login**: the indicator's autostart entry (inside the snap,
  snapd's per-app autostart). It's on by default after first launch, and can
  also be switched in the app under **More → Panel Indicator**.

The app and the indicator run together. Starting OBSCura, from the indicator
or directly, also starts the indicator if it isn't already running. When you
close the app, the indicator keeps running if **Start at login** is on, and
quits with the app if it's off.

The menu re-reads the camera each time it opens, so it reflects changes made
with gestures or in the app. It uses no GTK or webview: the desktop shell
draws the menu over D-Bus (StatusNotifierItem). On GNOME this needs the
AppIndicator extension, which Ubuntu enables by default.

To try the app without an OBSBOT camera, using another webcam's standard
controls:

```sh
OBSBOT_ALL_CAMERAS=1 obscura
```

## Using the CLI (`obsbotctl`)

```sh
obsbotctl list                     # connected cameras (--all for non-OBSBOT)
obsbotctl info                     # USB details and the matched profile
obsbotctl dump                     # every supported feature and its value
obsbotctl dump --unsupported       # ...including the unsupported ones and why
obsbotctl dump --json

obsbotctl status                   # live state: power, AI mode, zoom, fps…
obsbotctl firmware                 # is newer firmware on OBSBOT's download page?
obsbotctl events                   # follow camera events: target lost, gestures…
obsbotctl mics                     # Vox SE slots: connected, battery
obsbotctl set mic_pair_tx1 1       # pair TX1, then hold the mic's button ~6 s
obsbotctl set mic_tx1_gain -3      # wireless mic gain (-12..12)
obsbotctl set mic_level -6         # camera microphone level in dB (0 = full)
obsbotctl get ai_mode
obsbotctl set ai_mode human        # off | human | group | "hand tracking"
obsbotctl set ai_mode human --check  # …and report whether the camera took it
obsbotctl set gesture_control on
obsbotctl set gesture_zoom_factor 2.5
obsbotctl set mirror_image off
obsbotctl set contrast 60
obsbotctl set white_balance_temperature 4800K
obsbotctl set anti_flicker "50 Hz"

obsbotctl gimbal 0.5 0 --ms 800    # pan right at half speed for 0.8 s
obsbotctl gimbal 0 -1 --ms 300     # tilt down at full speed

obsbotctl preset list              # camera-side presets (slots 1-3)
obsbotctl preset save 1 Desk       # store the current view in slot 1
obsbotctl preset recall 1
obsbotctl preset rename 1 Whiteboard

obsbotctl controls                 # raw V4L2 controls exposed by the driver
```

Global options: `-d /dev/videoN` selects a camera, and `--all` includes
non-OBSBOT cameras. Values are parsed per feature: `on`/`off` for switches,
labels or numbers for choices, and numbers in display units for ranges
(`1.5` for 1.5× zoom, `4800K` for white balance).

There is also a hidden `obsbotctl raw` command for low-level Extension Unit
access, used during reverse engineering. **Only replay commands you have seen
in captures or SDK traces.** The camera has crashed and rebooted from
malformed commands.

## How it works

```
┌────────────────────┐ ┌───────────┐ ┌───────────────────┐
│ app/ (Tauri+Svelte)│ │ obsbotctl │ │ obscura-indicator │
└─────────┬──────────┘ └─────┬─────┘ └─────────┬─────────┘
          └──────────────────┼─────────────────┘
                     ┌───────▼────────┐    profiles/*.toml
                     │  obsbot-core   │◄── (per-model feature bindings)
                     └───────┬────────┘
     ┌──────────────┬────────┴───────┬──────────────┬──────────────┐
  V4L2 controls  UVC XU unit 2    UVC XU unit 2   V4L2 streaming  ALSA mixer
  VIDIOC_S_CTRL  selector 2:      selector 6:     (MJPEG preview) (mic level)
                 framed commands, settings +
                 queries, events  status block
```

- **Standard controls** (image, focus, exposure, white balance, pan/tilt/zoom)
  are ordinary UVC controls, exposed by the `uvcvideo` driver as V4L2 controls.
- **Vendor features** go through the camera's UVC *Extension Unit* (unit 2,
  GUID `9a1e7291-6843-4683-6d92-39bc7906ee49`) via the `UVCIOC_CTRL_QUERY`
  ioctl, which works while another application is streaming:
  - **Selector 2** carries 60-byte framed commands with two CRC-16/USB
    checksums (the camera silently drops frames with a bad checksum).
  - **Selector 6** takes short `[id, len, value]` settings, and reading it
    returns a status block reflecting the current state, including how many
    camera events are queued (fetched with a selector 2 query).
- **Profiles** (`profiles/*.toml`) map each catalog feature to a transport
  binding, so supporting a new model is data, not code. A profile can
  inherit another and only add or `drop` what differs.
- **The microphone level** is the camera's USB audio mixer, through ALSA.
- **Preview** captures MJPEG on its own file handle and sends JPEG bytes to the
  webview over a Tauri channel, with flow control so frames drop rather than
  queue.

The full protocol write-up is in [`docs/protocol.md`](docs/protocol.md).

### Repository layout

| Path | Contents |
|---|---|
| `crates/obsbot-core/` | Library shared by the app, CLI and indicator (layout below) |
| `crates/obsbotctl/` | Command-line tool |
| `crates/obscura-indicator/` | Lightweight tray indicator |
| `app/` | Tauri 2 desktop app: Rust backend in `src-tauri/`, Svelte 5 frontend in `src/` (components in `ui/`, `controls/`, `layout/`, `tabs/`, `windows/`) |
| `profiles/` | Per-model device profiles (compiled into the binaries) |
| `docs/protocol.md` | Decoded vendor protocol |
| `docs/CAPTURING.md` | How to capture OBSBOT Center's USB traffic on Windows |
| `captures/README.md` | Which OBSBOT Center actions still need recording (recordings themselves stay local) |
| `TODO.md` | Roadmap: done, next, and what's known about each open item |
| `tools/capture/obsbot_pcap.py` | Decoder for USBPcap captures |
| `tools/sdk-trace/` | Linux alternative to captures: record the frames OBSBOT's SDK sends (SDK not included) |
| `tools/bootstrap.sh` | Installs build dependencies and fixes LXD networking for snap builds |
| `.claude/skills/add-obsbot-camera/` | Claude Code skill: the step-by-step workflow for adding a model or feature |

Inside `crates/obsbot-core/src/`:

| Module | Contents |
|---|---|
| `features.rs` | The feature catalog: what OBSCura can show, by id |
| `profile.rs` | Profiles: how each model binds features (with inheritance) |
| `device/` | An opened camera: `read`, `write`, `vendor` frames, `observe` (events, camera-side changes), presets, gimbal, info |
| `transport/` | Hardware access: `v4l2` (video node), `v4l2_ctrl`, `uvc_xu` (Extension Unit), `audio` (ALSA) |
| `vendor/` | OBSBOT's protocol, no I/O: `protocol` (frames), `status` (status block), `events`, `mics` |
| `discovery.rs`, `preview.rs`, `companion.rs` | Finding cameras, the MJPEG preview, app/indicator coordination |
| `log.rs`, `firmware.rs` | Activity log and trace mode; firmware update check |

## Adding support for another camera

1. **Capture** OBSBOT Center's USB traffic on Windows while using each feature,
   following [`docs/CAPTURING.md`](docs/CAPTURING.md). On Linux, if you have
   OBSBOT's SDK, [`tools/sdk-trace/`](tools/sdk-trace/README.md) records the
   frames it sends instead. With Claude Code, `/add-obsbot-camera` walks
   through the whole process.
2. **Decode** the captures:

   ```sh
   python3 tools/capture/obsbot_pcap.py captures/10-gesture-master.pcapng
   ```

   This prints each command with its destination, command ID and payload, and
   only the bytes of the polled status block that changed.
3. **Write a profile** in `profiles/<model>.toml`. If the model works like
   one already supported, inherit it and only add what differs, as
   [`profiles/tiny-3-lite.toml`](profiles/tiny-3-lite.toml) and
   [`profiles/meet-2.toml`](profiles/meet-2.toml) do (`inherits`, and
   `drop` for parts the model lacks). Otherwise start from
   [`profiles/tiny-se.toml`](profiles/tiny-se.toml): match it by USB ID,
   inherit the standard controls, and bind features:

   ```toml
   id = "tiny-2"
   name = "OBSBOT Tiny 2"
   inherits = "generic-uvc"

   [[match]]
   vendor_id = 0x3564
   product_id = 0x....

   [features.mirror_image]
   status = 19                  # byte in the selector 6 status block
   short = { id = 0x14 }        # selector 6 setting [0x14, len, value]

   [features.gesture_locked_target]
   frames = [{ dst = 0x04, cmd = "c430" }]   # selector 2 framed command
   ```

4. **Register** the file in `BUILTIN` in `crates/obsbot-core/src/profile.rs`,
   and add a test that encodes a command and compares it byte-for-byte with a
   captured packet (see `crates/obsbot-core/src/vendor/protocol.rs`).

Pull requests with captures and profiles for other models are very welcome.
Please leave the raw `.pcapng` files out of the repository: they're large and
contain video frames. Only commit the decoded evidence in `docs/protocol.md`.

## Development

```sh
cargo test                         # protocol encoders vs captured packets, profiles, parsing
cargo build                        # workspace: obsbot-core, obsbotctl, obscura
cd app && pnpm check               # Svelte/TypeScript type checking
cd app && pnpm tauri dev           # run the app with hot reload
```

### Building the snap locally

```sh
tools/bootstrap.sh snap            # snapcraft, LXD, and firewall rules if needed
snapcraft clean obscura --use-lxd  # after new commits, or pack reuses the old build
snapcraft pack --use-lxd           # builds from committed files only
sudo snap install --dangerous obscura_*.snap
sudo snap connect obscura:camera
```

On machines that also run Docker, Docker's `DROP` forwarding policy cuts
LXD containers off the network, and snapcraft fails with "A network related
operation failed in a context of no network access". The bootstrap script
detects this and adds two `DOCKER-USER` rules for `lxdbr0`. They don't
survive a reboot, so run it again before the next build. A `--dangerous`
install doesn't update from the store; `sudo snap refresh obscura --amend`
switches back.

### Continuous integration and releases

- **CI** (`.github/workflows/ci.yml`) runs on every push and pull request:
  `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `svelte-check`
  and a frontend build.
- **Snap** (`.github/workflows/snap.yml`) builds the snap natively on amd64
  and arm64. Pull requests get the snaps as downloadable artifacts. Pushes to
  `main` publish both architectures to the **candidate** channel, from which
  they can be promoted to stable:

  ```sh
  snapcraft release obscura <revision> stable
  ```

  Publishing uses a `SNAPCRAFT_STORE_CREDENTIALS` repository secret, created
  with:

  ```sh
  snapcraft export-login --snaps obscura --channels candidate \
    --acls package_access,package_push,package_update,package_release -
  ```

The dev watcher doesn't track `profiles/`. Restart `pnpm tauri dev` after
editing a profile.

## Troubleshooting

**Seeing what happens**: press **Ctrl+Shift+D** in OBSCura to open the
Diagnostics window (again to close it). Keep it beside the main window: it
shows the camera's live state and an activity log of what you change, with
a warning when the camera ignores a change, plus what the camera does on
its own: tracking target lost or found, and gesture or voice changes to
AI mode and zoom. For a full debug log, start the app (or `obsbotctl`, or
the indicator) from a terminal with `OBSCURA_TRACE=1`; it then also prints
every frame sent to and read from the camera:

```sh
OBSCURA_TRACE=1 obscura
```

**Human tracking doesn't start**: the camera only starts human tracking
while it's streaming video, since it has to see someone. Open the preview
or start your call first. Group, Hand Tracking and the other modes don't
need this.

**"No OBSBOT camera found"**: check that `obsbotctl list --all` shows the
camera and that you can open `/dev/videoN` (see
[Permissions](#3-permissions)).

**"The camera is in use by another application"**: another app is streaming.
Close it, or close this app's preview; settings still work either way.

**A setting flips back for a moment**: the camera updates its status block
about a second after a command. The app hides this lag, but the CLI's `dump`
can show the old value if run immediately after a `set`; use
`set … --check` to wait and confirm.

**A gesture does nothing**: the status light blinks twice when the camera
recognises a gesture. No blinks means it wasn't recognised: hold the gesture
still for 2–3 s beside your face with your fingers spread, in good light.
The camera must be awake (it ignores most changes while asleep).

**The microphone is too quiet**: two levels apply. OBSCura's **Level** is the
camera's own (in dB, 0 = full); your system's input volume for the camera is
a second one on top, and the two don't follow each other.

**A Vox SE won't pair**: press **Pair** in OBSCura first, then hold the mic's
button for about 6 s until its light flashes fast. The camera needs to be
awake. Once paired, the mic reconnects on its own.

**`Corrupt JPEG data: … extraneous bytes before marker` in the terminal**:
harmless. It's WebKit's JPEG decoder complaining about padding in the camera's
MJPEG frames.

**Blank window or rendering glitches on some GPUs**: try
`WEBKIT_DISABLE_DMABUF_RENDERER=1 obscura`.

**The camera disappears from USB**: it can reboot after an invalid vendor
command, and comes back after about 40 seconds. The app reconnects on its own.

## License

Copyright © 2026 Ken VanDine

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. See [LICENSE](LICENSE) for the full text.
