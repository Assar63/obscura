# Roadmap

What's done, what's next, and what's known about each open item. Most
items came from OBSBOT's SDK headers (`libdev` 2.1.0) and tests on a
Tiny 3; nothing here comes from the SDK's binaries, and no SDK code goes
into this repository. What still needs recording from OBSBOT Center is in
[`captures/README.md`](captures/README.md); the protocol itself is in
[`docs/protocol.md`](docs/protocol.md).

**The headers are not proof.** Their `@category` notes lag behind the
hardware. `aiSetGestureParaR` is documented as "tail2 and later products"
but is exactly how the Tiny 3's gestures work. And the Tiny 3 accepts many
older commands without acting on them. So every item below needs the same
two steps before it ships:

1. **Trace:** call the SDK function on the camera, with the `ioctl`
   logging shim, and record the frames it sends (see
   [Tooling](#5-tooling)).
2. **Test on the camera:** send those frames from OBSCura and confirm the
   effect with an oracle: a status byte, a query reply, the status light,
   or something you can see in the picture. "Accepted" isn't evidence.

Priorities: **P1** fixes something wrong or confusing today, **P2** is a
useful feature with a clear path, **P3** is nice to have or speculative.

---

## 1. The status block

Decoded byte by byte from the SDK's `CameraStatus::tiny`; the table, with
the bytes measured from Linux, is in
[`docs/protocol.md`](docs/protocol.md#status-block-layout-selector-6). In
code, `vendor::status::StatusBlock` is the only place that knows the
offsets, for profiles with `status_layout = "tiny"`.

---

## 2. Corrections (P1)

- [x] **AI mode values 4–15.** The catalog only knows 0 Off, 1 Group,
  2 Human, 3 Hand, so the app showed a raw "6" while the AI mode changed.
  `AiWorkModeType` is 4 Whiteboard, 5 Desk, **6 Switching** (a change in
  progress), 7 Speech (Tiny 3's voice tracking), 14 Portrait tracking (Tiny
  3). *Done:* profiles mark 6 as `transient`, and the app keeps showing the
  last settled mode. The Tiny 3's extra modes are a feature (§3).
- [x] **Fix the status byte notes in `docs/protocol.md`.** *Done:* the
  SDK layout is now a table there, with measured bytes marked. [1] and [2] are
  `rvd` in the SDK; [2] still works as "asleep" on the Tiny SE and Tiny 3.
  [4–5] are the zoom ratio and [17] the field of view, not "tracking state".
  [31] is the stream fps. [43] is an event counter. Add the table above.
- [x] **Auto sleep "off".** The SDK says `auto_sleep_time` 0 means "do not
  sleep"; OBSCura writes a negative time, as OBSBOT Center did for the Tiny
  SE. *Measured:* with −120 and nothing streaming, the Tiny 3 stayed awake
  for 3 minutes (with 120 it sleeps after 2). No change needed.
- [x] **Pickup distance labels.** *Done:* Close, Standard, Far ("Close"
  is OBSBOT Center's label for 0 on the Tiny SE, so it stays). The SDK names `audio_opt.distance` 0 near,
  1 standard, 2 far. The app shows Close, Medium, Far, with Medium a
  guess (`captures/README.md` §0). Use Near, Standard, Far.
- [x] **Noise reduction range.** The SDK documents `noise_cancellation` as
  0 off, 1 on, but the Tiny SE captures show values 0–3 and the Tiny 3
  reports 3. *Measured:* the Tiny 3 takes and reports 0–3, so the levels
  stay. Whether they sound different is untested.
- [x] **Status light: two bytes.** [33] is the brightness (0 = off) and [45]
  `led_enable` is a separate switch. *Fixed:* on the Tiny 3, writing 0
  clears [45] and keeps the brightness at [33], so the app showed the light
  as on while it was off. The Tiny 3 profile now reads the switch from [45],
  and switching on writes the brightness from [33] (new `level_status`
  option).
- [x] **Zoom readback from the status block.** [4–5] holds the zoom ratio
  0–100. *Measured:* linear, 1x–4x = 0–100 (2x 33, 3x 67). *Decision:* keep
  the `0468` query. It returns an exact float, and switching would need a
  new scaling option for one query saved per refresh.

---

## 3. New features for the Tiny 3

### P2: status byte known, likely simple

- [x] **More AI modes: Whiteboard, Desk, Voice tracking, Portrait.** *Done:*
  short setting `16 02 <mode> 00` with 4 Whiteboard, 5 Desk, 7 Voice
  Tracking (SDK "speech") is echoed at status[24] on the Tiny 3. Writing 7
  also set the sub-mode [28] to 2. 14 (portrait tracking) is ignored. The
  Tiny 3 profile lists them (new `options` binding field), the app shows them
  in a second column next to Group/Hand, and the tray menu follows the
  camera's list.
- [ ] **Human tracking framing (AI sub-mode).** *Tested, not working on
  firmware 6.6.8.3:* the SDK's `cameraSetAiModeU(Human, sub)` sends short
  setting `16 02 02 <sub>`; the camera stores it (status[28] follows) but
  the picture and zoom don't change (alternated normal/close-up while being
  tracked). Matches brendanwelsh's "framing value discarded". Left unbound;
  retest after a firmware update. OBSBOT's notes for 6.6.10.1 and 6.6.11.1
  don't mention framing.
- [x] **Field of view: 86° / 78° / 65°.** *Done:* short setting `04 01
  <level>`, status[17]; checked on the picture. Image tab, next to zoom.
- [x] **Tracking speed / mode.** *Done:* the SDK's control parameters
  (`aiSetControlParaR`, human target): write dst 04 `4454` (u32 target,
  u32 parameter, value), read with query `8454`. Parameter 5 is the speed
  mode (0 super lazy … 2 default … 4 crazy): stored, read back, and the
  camera visibly lagged at 0 and snapped after at 4. Parameter 0 is motion
  mode: stored and read back; its effect wasn't checked separately.
  Status[36] doesn't follow either. `aiSetTrackingModeR` (`c40c`,
  standard/headroom/motion) and `aiSetTrackSpeedTypeR` (`4409`) have no
  readback and are left out (brendanwelsh found `c40c` ignored).
- [x] **Voice control.** *Done:* short setting `15 02 <command> <value>`
  (SDK `cameraSetAudioCtrlStateU`). Commands 0-6 (Hi Tiny, Sleep Tiny,
  Track Me, Unlock Me, Zoom In, Zoom Out, presets) are bit n of status[21]
  (the SDK header's bit list is in another order); 100 is the voice zoom
  (0-100 = 1x-4x, status[22]), 101 the language (status[20]). Tested by
  voice: "Unlock Me" and "Track Me" switched tracking, and "Unlock Me" was
  ignored once switched off (repeated hands-free, to rule out gestures).
  Audio tab → Voice Control.
- [x] **Privacy mode.** *Tested:* the SDK's `cameraSetDevRunStatusR(4)`
  sends the same frame as sleep (`c2a0` u32 1) and the camera reports
  itself asleep (status[9] = 3), so it isn't a separate mode on the Tiny 3;
  Sleep covers it.

### P3: needs more work or may not apply

- [ ] **Presets.** The Tiny SE's preset query (`043b`) gets no answer from
  the Tiny 3, and brendanwelsh found preset *recall* works but *writing*
  is ignored. Trace `aiGetGimbalPresetListR`, `aiTrgGimbalPresetR`,
  `aiAddGimbalPresetR`, and `aiSetGimbalPresetNameWithIdR`. The Tiny 3's
  `PresetsAction` also stores framing (`auto_frame`), AI mode (desk,
  whiteboard, speech), tracking speed and white balance per preset.
- [ ] **Audio mode / beamforming.** `AudioModeType` (Tiny 3): Omni, Stereo,
  Front, Back, Dipole, Music; status [40] bits 3–7 (Tiny 3 reads stereo).
  Find the setter by tracing; there's no obvious `cameraSet…` for it in the
  header.
- [ ] **Sound-source assisted tracking (DOA).** `doa_set` [46]: "sound
  source assisted tracking", range, "audio mode limit". The Tiny 3 manual
  says sound localization for voice control can be switched in OBSBOT
  Center. Find the setter by tracing.
- [x] **Wireless mic status.** [41] `wireless_mic` (pairing, TX0/TX1
  online). Read-only display in Device details, if someone has the mic. *Done:* Audio → Wireless
  Microphones shows each slot from [41] and the mic info query.
- [ ] **Gimbal limits, pan reverse, preset speed.** `aiSetGimbalParaR`
  (tail2 and later): pan/pitch min/max, pan reverse, preset speed
  (0.1–0.8). Might replace the Tiny SE "View and Gimbal Reverse" command,
  which has no readback.
- [ ] **Tracking zone / composition.** `aiSetControlParaR`: pan/pitch lock,
  limited tracking zone, composition offsets (headroom). Equivalent to
  OBSBOT Center's Zone Tracking. Big UI work.
- [ ] **White balance presets and R/B gain.** `cameraSetWhiteBalanceR`
  (DevWhiteBalanceType: daylight, fluorescent, tungsten, cloudy, … R/B
  gain); OBSBOT's OSC table marks WB shift and R/B gain "Only for Tiny 3".
  Mind the "last written WB control wins" quirk in joshualambert's notes.
- [ ] **Boot mode and boot position.** `boot_mode` [32]; `cameraSetBootModeU`
  and `aiSetGimbalBootPosR`: the AI mode and gimbal position the camera
  starts in.
- [ ] **Portrait mode.** `vertical` [12], `cameraSetVerticalModeU` ("the
  device will restart automatically"). The Tiny 3 manual mentions portrait
  mode. Restarting makes it a careful, confirm-first action.
- [ ] **Factory reset.** `cameraSetRestoreFactorySettingsR` ("all"). Tracing
  it means actually resetting the camera, so do it last and only on
  purpose. Unlocks the greyed-out Factory Reset button.
- [ ] **HDR availability.** [30] `hdr_support`: grey out HDR when the
  current mode can't use it, instead of letting the switch do nothing.
- [ ] **Show stream fps.** [31] is the camera's current fps; could feed the
  preview's format display.

---

### Camera events

- [x] **Event queue and camera-side changes.** *Done:* status[43] counts
  queued events and query `021d` pops one (see docs/protocol.md). OBSCura
  drains it on each status read (Tiny 3 only, `event_queue = true`) and
  logs target lost/found; it also logs AI mode, power and zoom changes the
  camera made itself. `obsbotctl events` follows them live.
- [ ] **Decode more event types** (16 is unknown). With a Vox SE paired,
  run `obsbotctl events` while connecting, muting and charging it: the
  SDK's `kEvtTipsTWS…` events should arrive through the same queue.

### Vox SE wireless microphone (Tiny 3)

The Vox SE pairs with a receiver inside the Tiny 3 (up to two mics, TX1
and TX2). `libdev` exports a full mic API that its header doesn't declare
(`cameraGetTWSInfoR`, `cameraTX*`, `cameraSetTWS*`, `cameraSetAudioSourceR`,
`cameraSetAudioSelect`); the data types (`DevTWSInfo`, `DevTXType`,
`DevTWSKeyType`, `AudioSelectAttr`) are in the header. Traced on a Tiny 3
(firmware 6.6.8.3) with no mic paired:

| Call | Frame | Result |
|---|---|---|
| Mic info (`cameraGetTWSInfoR`) | query dst 02 `02c0` | 16 bytes, all 0 without mics |
| Auto audio select, read | query dst 02 `828a` | `01 00 00`: auto supported, off, no pairing record |
| Auto audio select, write | dst 02 `c28a`, u8 | ✅ 1 made the camera pick the built-in mics (source 0) |
| Selected audio source, read | query dst 02 `428a` | u8, 0 built-in |
| Audio source, write (`cameraSetAudioSourceR`) | | ✅ 3 set status [40] bits 0–2 to 3 (Bluetooth) |
| Per-mic reads: battery, charging, mute, gain, name, version, serial | query to dst `58` (TX1) / `98` (TX2), e.g. `1330` | no answer without a paired mic |
| Pair enable/disable for TXn (`cameraTXSetPairEnabled`) | flags 0x05 to dst `58`/`98`, `13 0c` on / `53 0c` off | sent to the mic; together with `setBlePairingEnable` it pairs ✅ |
| Clear pairing for TXn | flags 0x05 to dst `58`/`98`, `93 0e` | ✅ unpairs (TX1 tested) |
| `cameraDevBluetoothMatchU` | nothing sent | |
| `setBlePairingEnable(on, 0)` | dst `13` `0e0c`, u8 | second half of pairing (after the slot's pair enable) ✅ |

**Pairing works from Linux** (solved by trying the traced commands): "pair
enable" for the slot, then Bluetooth pairing mode, then hold the mic's
button. See docs/protocol.md.

- [x] **Pairing, slot status and battery.** Audio → Wireless Microphones:
  per-slot state (connected, battery, charging, muted) from query `02c0`,
  a guided Pair button that waits for the camera to report the mic, Cancel
  and Forget; `obsbotctl mics`. Tiny 3 verified (TX1 and TX2), Tiny 3 Lite assumed.
- [x] **Forget** (TX1): the mic went offline and didn't reconnect.
- [x] **TX2**: the same mic, forgotten on TX1, paired on TX2.
- [x] **Camera microphone on/off and level.** The camera's USB audio mixer
  ("Capture Volume", ALSA), in dB (0 = full; the Tiny 3's control runs
  -100..0 dB, the slider -50..0). For every webcam with a microphone
  (generic profile). The system's input volume (PipeWire) is a separate
  layer on top that doesn't follow it. Snap: `obscura:alsa`.
- [ ] **Two mics at once** (needs a second Vox SE).
- [ ] **P3: Mic battery in the tray menu.**
- [x] **P2: Mute and gain per mic.** *Done:* `d32d` (mute, u8) and `532d` (gain,
  i32) to the mic; readback from `02c0` [2] bits and [5]/[6]. Gain is
  offered as -12..+12 (the mic stores -30..+30; OBSBOT publishes no range).
- [ ] **P2: Button function.** `cameraSetTWSKeyTypeR`: track, switch
  tracking mode, zoom 1x, record.
- [x] **P2: Audio source.** Built-in vs. wireless, and auto select. *Done:* audio source and auto-select controls.
- [ ] **P3: Mic options.** `cameraSetTWSFuncR` (button, vibration, LED,
  noise suppression, auto shutdown), `cameraSetTWSSoundModeR` (mono/stereo).

## 4. Other models

- [x] **Tiny 3 Lite (`3564:ff04`).** Same SDK family (`ObsbotProdTiny3Lite
  = 19`) and the same verified commands in joshualambert's notes. Likely a
  copy of `tiny-3.toml` with its USB ID; needs a tester. *Done:* `profiles/tiny-3-lite.toml`,
  a copy of the Tiny 3's, marked unverified.
- [ ] **Verify the Tiny 3 Lite profile** on a real camera.
- [ ] **Re-check the Tiny 2 / 2 Lite / 4K / Meet 2 / Meet SE profiles**
  against the SDK. E.g. `aiSetGestureCtrlIndividualR` (gesture types 0–4
  on the Tiny 2 series) may be the Tiny 2's gesture path, and the Meet 2's
  gestures live in its own `gesture_para` bits [38–39].

---

## 5. Tooling

- [ ] **Two programs querying the camera at once.** Query replies come back
  through one slot on the camera (selector 2), so when the app and
  `obsbotctl` query at the same moment, one can read the other's reply
  and time out ("unavailable" in `obsbotctl dump` while the app is open).
  Retrying on a mismatched sequence number, or serialising through the
  running app, would fix it.
- [ ] **Firmware 6.6.10.1's new gesture toggle** ("Motion Capture to
  Avatar"): after updating, trace `aiSetGestureParaR` types beyond 8.

- [x] **Add the SDK trace tooling to `tools/`.** *Done:* `tools/sdk-trace/`,
  with a build script and README; the workflow is also in the
  `add-obsbot-camera` Claude Code skill. Two small files, our own
  code, no SDK inside:
  - `xulog.c`: an `LD_PRELOAD` shim that logs every `UVCIOC_CTRL_QUERY`
    (unit, selector, request, bytes). `libdev` uses plain `ioctl` on
    `/dev/videoN`, so this needs no root.
  - a small C++ driver that calls one SDK function per run and prints a
    marker before each call, so the log lines up with the API.

  Build instructions should point at a locally downloaded SDK. Document the
  workflow in `docs/CAPTURING.md` as the Linux alternative to Windows
  captures.
- [x] **Never send the SDK's startup shell command.** *Done:* noted in
  `tools/sdk-trace/README.md` and the skill. On start, the SDK
  sends the system module (dst 0d, `c81a`) the text
  `touch /app/private/resolution.conf`. The trace tooling should note this,
  and OBSCura must not copy it.
- [ ] **Licence check.** The SDK zip has no licence text. Before tracing
  more of it, check the terms OBSBOT gave with the download.
