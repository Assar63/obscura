# TODO: findings from OBSBOT's SDK

What OBSCura can correct, improve and add, based on the public headers of
OBSBOT's SDK (`libdev` 2.1.0: `include/dev/dev.hpp`, `devs.hpp`). Nothing
here comes from the SDK's binaries, and no SDK code goes into this
repository. Captures still needed from OBSBOT Center are in
[`captures/TODO.md`](captures/TODO.md).

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

## 1. The status block, decoded

`Device::CameraStatus::tiny` describes the selector 6 status block byte by
byte. The header says it's used for "tiny, tiny4k, tiny2 series, tinySE,
meet2, meetSE", and the Tiny 3's block matches it: every offset our
profiles use lands on the field the SDK names. The struct is
`#pragma pack(1)`, and bitfields are LSB first.

| Byte | SDK field | OBSCura today | Tiny 3 value seen |
|---|---|---|---|
| 0 | `ai_target` / `length` | – | `2e` (46, the length) |
| 1 | `rvd1` "not used" | docs say "locked target" | 0 |
| 2 | `rvd2` "not used" | `sleep` (status) | 0 awake, 1 asleep |
| 3 | `anti_flicker` (PowerLineFreqType) | UVC | 0 |
| 4–5 | `zoom_ratio`, u16, 0–100 | docs say "tracking state" | 0, `16` (22) while zoomed |
| 6 | `hdr` | `hdr` ✅ | 0/1 |
| 7 | `face_ae` | `auto_exposure_mode` ✅ | 1 |
| 8 | `noise_cancellation` "0 off, 1 on" | `noise_reduction`, 0–3 | 3 |
| 9 | `dev_status` (1 run, 3 sleep, **4 privacy**) | docs only | 1/3 |
| 10–11 | `auto_sleep_time`, i16 s, "0 → do not sleep" | `auto_sleep`/`sleep_time`, negative = off | 120 |
| 12 | `vertical` (portrait mode) | – | 0 |
| 13 | `face_auto_focus` | `auto_focus_mode` ✅ | 1 |
| 14 | `auto_focus` | UVC | 1 |
| 15 | `manual_focus_value` | UVC | 0 |
| 16 | `sleep_micro` | `mic_during_sleep` ✅ | 1 |
| 17 | `fov` (FovType: 0 86°, 1 78°, 2 65°, 3 none) | docs say "tracking state" | 0/3 |
| 19 | `image_flip_hor` | `mirror_image` ✅ | 0 |
| 20 | `voice_ctrl_language` (0 Chinese, 1 English) | – | 1 |
| 21 | `voice_ctrl`, bit per voice command | – | 0 |
| 22–23 | `voice_ctrl_zoom`, u16, 0–100 | – | `21` (33) |
| 24 | `ai_mode` (AiWorkModeType) | `ai_mode`, 0–3 only | 0, 2, 3, **6** |
| 25 | `audio_auto_gain` | `auto_gain` ✅ | 1 |
| 26 | `sleep_bg_type` (bits 0–3 image, 4–7 video) | – | `43` |
| 27 | `bg_img_idx` | – | 0 |
| 28 | `ai_sub_mode` (AiSubModeType) | – | 0 |
| 29 | `bg_img_mirror` | `sleep_background_mirror` ✅ | 0 |
| 30 | `hdr_support` (HDR possible in this mode) | – | 0 |
| 31 | `fps` of the current stream | docs say "0x3c/0x1e" | `1e`/`0f` (30/15) |
| 32 | `boot_mode` (bits 0–4 sub-mode, 5–7 AI mode) | – | 0 |
| 33 | `led_brightness_level` (0 off, 1–3) | `status_light` ✅ | 3 |
| 34 | `audio_opt`: bits 0–3 distance (0 near, 1 standard, 2 far), bit 4 UAC enabled | `pickup_distance`, `disable_microphone` ✅ | `10` |
| 35 | `ble_status` | – | 0 |
| 36 | `ai_tracker_speed` (0 normal, 2 motion) | – | 0 |
| 37 | `live_stream_mode` | – | 0 |
| 38–39 | `gesture_para` (Meet 2 / Meet SE only) | – | 0 |
| 40 | `audio_mode`: bits 0–2 source, 3–7 AudioModeType | – | `08` (stereo) |
| 41 | `wireless_mic` (Tiny 3) | – | 0 |
| 42 | `auto_frame`: low nibble landscape, high nibble portrait | – | 0 |
| 43 | `event_count`: queued camera events | ✅ read with query `021d` | 10 stale, then 0–1 |
| 44 | `kws_extend` (wake-word flags) | – | 3 |
| 45 | `led_enable` | – | 1 |
| 46 | `doa_set` (Tiny 3): bit 0 sound-source assisted tracking, bits 1–2 range, bit 3 audio mode limit | – | 1 |

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
  guess (`captures/TODO.md` §0). Use Near, Standard, Far.
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
- [ ] **Human tracking framing (AI sub-mode).** `AiSubModeType`: Normal,
  Upper Body, Close-up, Headless, Lower Body, at [28]. joshualambert writes
  it as the 4th byte of `16 02 02 <sub>`. OBSBOT Center calls this "Auto
  Zoom" on the Tiny 3, and brendanwelsh found the Tiny 2 framing command
  ignored, so test on the picture. Also compare with `auto_frame` [42].
- [ ] **Field of view: 86° / 78° / 65°.** `cameraSetFovU(FovType)`, status
  [17]. joshualambert confirmed short setting `04 01 <level>` changes the
  picture on a Tiny 3 Lite. A small choice control in the Image tab.
- [ ] **Tracking speed / mode.** Status [36] `ai_tracker_speed` (0 normal,
  2 motion). The Tiny 3 preset struct names speeds 5 super lazy, 6 slow,
  7 fast, and OBSBOT's OSC table has Tracking Speed (slow, standard, fast)
  and Tracking Mode (headroom, standard, motion) for the Tiny 3. Candidates:
  `aiSetTrackingModeR(AiVerticalTrackType)`, `aiSetTrackSpeedTypeR`, and
  `aiSetControlParaR(Human, Motion)`. Trace all three to find the one the
  Tiny 3 acts on.
- [ ] **Voice control.** The Tiny 3 manual lists "Hi Tiny", "Sleep Tiny",
  "Track Me", "Unlock Me", "Zoom in Closer", "Zoom out Further" and
  "Position One/Two/Three". `cameraSetAudioCtrlStateU(AudioCtrlCmdType,
  state)` switches each command, sets the language (status [20]) and the
  voice zoom factor ([22–23]); status [21] holds one bit per command.
  Add a Voice Control card under More, next to Gesture Control.
- [ ] **Privacy mode.** `DevStatus` 4 = privacy: "stream will not be fetched
  from the device". Try `cameraSetDevRunStatusR(DevStatusPrivacy)` and
  read [9]. Could be a privacy button next to Sleep.

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
- [ ] **Wireless mic status.** [41] `wireless_mic` (pairing, TX0/TX1
  online). Read-only display in Device details, if someone has the mic.
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
| Clear pairing for TXn | flags 0x05 to dst `58`/`98`, `93 0e` | not tested |
| `cameraDevBluetoothMatchU` | nothing sent | |
| `setBlePairingEnable(on, 0)` | dst `13` `0e0c`, u8 | second half of pairing (after the slot's pair enable) ✅ |

**Pairing works from Linux** (solved by trying the traced commands): "pair
enable" for the slot, then Bluetooth pairing mode, then hold the mic's
button. See docs/protocol.md.

- [x] **Pairing, slot status and battery.** Audio → Wireless Microphones:
  per-slot state (connected, battery, charging, muted) from query `02c0`,
  a guided Pair button that waits for the camera to report the mic, Cancel
  and Forget; `obsbotctl mics`. Tiny 3 verified (TX1), Tiny 3 Lite assumed.
- [ ] **Test TX2** with a second Vox SE, and **Forget**.
- [ ] **P3: Mic battery in the tray menu.**
- [ ] **P2: Mute and gain per mic.**
- [ ] **P2: Button function.** `cameraSetTWSKeyTypeR`: track, switch
  tracking mode, zoom 1x, record.
- [ ] **P2: Audio source.** Built-in vs. wireless, and auto select.
- [ ] **P3: Mic options.** `cameraSetTWSFuncR` (button, vibration, LED,
  noise suppression, auto shutdown), `cameraSetTWSSoundModeR` (mono/stereo).

## 4. Other models

- [ ] **Tiny 3 Lite (`3564:ff04`).** Same SDK family (`ObsbotProdTiny3Lite
  = 19`) and the same verified commands in joshualambert's notes. Likely a
  copy of `tiny-3.toml` with its USB ID; needs a tester.
- [ ] **Re-check the Tiny 2 / 2 Lite / 4K / Meet 2 / Meet SE profiles**
  against the SDK. E.g. `aiSetGestureCtrlIndividualR` (gesture types 0–4
  on the Tiny 2 series) may be the Tiny 2's gesture path, and the Meet 2's
  gestures live in its own `gesture_para` bits [38–39].

---

## 5. Tooling

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
