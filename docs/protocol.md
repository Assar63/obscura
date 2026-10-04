# OBSBOT vendor protocol

What OBSCura knows about how OBSBOT cameras are controlled over USB, and how
each piece was found. Measured on two cameras:

- **Tiny SE** (`3564:feff`, firmware 6.4.4.1): decoded from USB captures of
  OBSBOT Center (`captures/`, decoder `tools/capture/obsbot_pcap.py`) and
  confirmed from Linux unless marked *unverified*. Entries from the
  2026-09-27 batch (sleep, HDR, AF/AE modes, audio, device sleep, status
  light, AI lock, queries) were also set and read back from Linux.
- **Tiny 3** (`3564:ff02`, firmware 6.6.8.3): tested from Linux; the
  commands it doesn't share with the Tiny SE (gestures, extra AI modes,
  events, Vox SE) were recorded from the traffic of OBSBOT's SDK
  (`tools/sdk-trace/`). See [OBSBOT Tiny 3](#obsbot-tiny-3-3564ff02).

Sections: [Channels](#channels) · [Framed commands](#framed-commands-selector-2)
· [Commands](#commands) · [Short settings](#short-settings-selector-6)
· [Standard UVC controls](#standard-uvc-controls-used-by-obsbot-center)
· [Status block layout](#status-block-layout-selector-6)
· [Tiny 3: gestures, AI modes, events, Vox SE](#obsbot-tiny-3-3564ff02)

In code: frames and encodings are in `crates/obsbot-core/src/vendor/`
(`protocol`, `status`, `events`, `mics`), per-model bindings in
`profiles/*.toml`.

## Channels

| Path | What |
|---|---|
| Standard UVC (V4L2) | brightness, contrast, saturation, sharpness, hue (PU unit 3); pan/tilt (±130°/±90°), zoom (0..12), focus, exposure, gain, white balance, power-line frequency |
| XU unit 2, GUID `9a1e7291-6843-4683-6d92-39bc7906ee49` | 22 selectors, all 60 bytes, GET+SET |
| XU selector 2 | framed commands and queries (below) |
| XU selector 6 | SET: short settings `[id, len, value LE]`; GET: status block |

## Framed commands (selector 2)

```
0      aa           magic
1      flags        0x25 command, 0x01 query (0x21/0x05 also seen); responses 0x29, errors 0x89
2..4   seq u16le    the camera accepts any value, including repeats
4..6   0x000c
6..8   token u16le  CRC-16/USB over bytes 0..12 with the token zeroed
8      sender       0x0a host (responses: 0x04/0x02 -> 0x0a)
9      receiver     0x04 AI/gimbal module, 0x02 ISP, 0x0d system, 0x12 ?
10..12 cmd          u16le = (id << 7) | op ; op 0x44 write / 0x04 read (dst 04), 0x42 (dst 02)
12..14 len u16le
14..16 crc u16le    CRC-16/USB over bytes 12..16+len with this field zeroed
16..   payload      bytes after the payload are ignored (captures contain stale data there)
```

CRC-16/USB = poly 0x8005 reflected, init 0xFFFF, xorout 0xFFFF. **The camera
silently drops frames with a wrong token or payload CRC.** The status block
reflects a command about 1.1 s later.

Queries: send the frame with flags `0x01`, then GET_CUR selector 2 about 50 ms
later to read an `aa 29` response carrying the same cmd and a payload.

> ⚠️ **Never send a write command ID (op 0x44) as an empty query.** The camera
> answers with `aa 89` errors, and during probing it then stopped responding
> and dropped off USB. Only send queries that were seen in captures.

## Commands

| Feature | Write | Read (query) | Value |
|---|---|---|---|
| Gesture: Locked Target | dst 04 `c430` | `8430` → u8 | u8 0/1; also status[1] |
| Gesture: Zoom | dst 04 `4431` | `0431` → u8 | u8 0/1 |
| Gesture: Zoom Factor | dst 02 `4219` payload `02000000` + u32 (factor×100), then dst 04 `4432` f32 factor | `0468` → f32? (*unverified*) | 1.00–4.00 |
| Gesture: Dynamic Zoom | dst 04 `4433` | ? (`0433` query was sent just before the camera hung) | u8 0/1 |
| Gesture: Direction Flip | dst 04 `c433` | `8433` → u8 | u8 0/1 |
| Gesture Control (master) | the four switch frames above, same value | status[1] | |
| Gimbal velocity (joystick) | dst 04 `8464` 3×f32 `[0, pitch, yaw]`, ~10 Hz while held; zeros to stop | – | ✅ measured from Linux: positive pitch tilts the camera down (the picture moves up); positive yaw turns the view right (the picture moves left), with Mirror Image on or off. OBSBOT Center sends negative pitch for joystick up, and wraps each move in dst 04 `4402` u8 0 … 1 |
| Re-center (View & Gimbal reset) | query dst 04 `8438` payload `01`, then `0439` with flags `0x05` and no payload | – | ✅ UVC pan/tilt 0 does *not* re-center after velocity moves |
| Manual zoom | dst 02 `4219` payload `02000000` + u32 (factor×100, 100–400) | `0468` → f32 | ✅ ignored while the camera sleeps |
| View and Gimbal Reverse | dst 04 `843b` u8 | none seen | |
| Gimbal position | – | dst 04 `0466` payload `01` → 24 bytes | i16 angles ×10 at [8] and [10] |
| Preset: save | dst 04 `4439`: slot u32, f32 angle[10]×0.1, f32 angle[8]×0.1, 0.0, zoom f32, −1000.0 | | ✅ 3 slots (0–2) |
| Preset: name | dst 04 `843a`: slot u32 + name bytes | `043b` flags `0x21`, payload slot u32 → name, or flags `0x09` if empty | ✅ |
| Preset: recall | dst 04 `c439`: slot u32, 1.0 ×4 | | ✅ |
| Hand-tracking option (dropdown) | dst 04 `4422` u8 | ? | *meaning pending NOTES* |
| Hand-tracking limits? | `4420`/`c420` f32 −45/45, `4421`/`c421` f32 −30/30, `c424` 24 bytes, `c426` u8 | | *unverified* |
| Gimbal Speed | none (host-side scale for the joystick) | | ✅ `02-known-state-readback` shows no traffic |
| Sleep / Resume | dst 02 `c2a0` u32 1 / 0 | status[2] | ✅ status[9] also goes 01 → 03 while asleep |
| Auto Focus Mode | dst 02 `0236` u32 | status[13] | 0 Global, 1 Face |
| Firmware version | – | dst 0d `0804` → 4 bytes, last component first | `01 04 04 06` = 6.4.4.1 |
| Serial number | – | dst 0d `c818` → ASCII | 14 characters |
| Gesture state | – | dst 04 `0401` → 12 bytes | ✅ [3] locked target, [4] zoom, [5] dynamic zoom, [6] direction flip, [7] zoom factor ×10 |
| Current zoom? | – | dst 04 `0468` → f32 | 1.01–2.57 seen; follows tracking zoom, not the gesture zoom factor |

Queries without a payload leave the payload CRC (bytes 14..16) zeroed;
queries with one (`0466`, `043b`, `8438`) carry a normal CRC. Responses echo
the query's seq, which matters when several queries are in flight.

OBSBOT Center saves a preset by querying the position (`0466`) and zoom
(`0468`), writing `4439` and then naming it with `843a` ("Preset1".."Preset3"
by default). At startup it lists names with `043b` for each slot. It also
sends `443a` (read a stored preset) and `c43d` around each recall, which
aren't needed to move there. No delete command was seen.

**Rotate, flip and Portrait** send nothing to the camera
(`33-rotate-flip`, `35-resolution-aspect`): OBSBOT Center applies them to
its virtual camera on the PC. Resolution and frame rate are standard UVC
stream negotiation. OBSBOT Center's startup
sequence (`02-known-state-readback`) is dst 0d `0818`, `0804`, `4819`,
`c818`, then dst 02 `42c4`, dst 04 `0401` and `0468`, dst 02 `c229` and
`4229`. It never queries the individual gesture switches.

`0401` layout, confirmed from Linux by toggling each switch and re-querying:
`00 00 00 LT ZM DZ DF ZF 00 02 00 00`, where ZF is the zoom factor ×10
(`0f` = 1.5×, `19` = 2.5×, `28` = 4.0×). Byte [0] was 1 in the capture
(AI mode Group) and 0 on Linux (AI mode off); [9] was always 2.

**AI lock** has no command of its own. Locking sends AI mode 0 and turns off
the gesture switches that were on; unlocking restores the AI mode only.

## Short settings (selector 6)

| Feature | Write | Status byte | Values |
|---|---|---|---|
| Mirror Image | `14 01 v` | [19] | 0/1 ✅ |
| AI mode | `16 02 v 00` | [24] | 0 off, 1 group, 2 human, 3 hand ✅ |
| ? (hand dropdown) | `18 01 v` | | *pending* |
| HDR | `01 01 v` | [6] | 0/1 |
| Auto Exposure Mode | `03 01 v` | [7] | 0 Global, 1 Face |
| Noise Reduction | `0a 01 v` | [8] | 0 Off, 1–3 (labels pending) |
| Auto Sleep + Sleep Time | `0b 02 t` (i16 seconds) | [10..12] i16 | 30 / 120 / 600; negated (−30) when Auto Sleep is off |
| Sleep Background Mirror | `0e 02 02 v` | [29] | 0/1 |
| Mic Status During Sleep | `13 01 v` | [16] | 0/1 |
| Auto Gain | `17 01 v` | [25] | 0/1 |
| Status Light | `1a 01 v` | [33] | 0 off, 1–3 brightness |
| Radio Distance | `1b 01 v` | [34] low bits | 0 Close, 1 Standard, 2 Far (SDK: near, standard, far) |
| Microphone enabled (OBSBOT Center: "Disable Microphone", inverted) | `1c 01 v` | [34] bit 4 (0x10) | 1 enabled, 0 disabled ✅; the camera re-enumerates on USB, without its audio interface when disabled |

## Standard UVC controls used by OBSBOT Center

Confirmed in the captures: Auto Focus and Focus are Camera Terminal
selectors 8 and 6; Exposure Compensation is Processing Unit selector 1
(backlight compensation, 0..18 = −3..+3 EV in 1/3 steps); Anti-Flicker is
PU selector 5; Contrast and Saturation are PU selectors 3 and 7.

## Status block layout (selector 6)

OBSBOT's SDK headers (`libdev` 2.1.0, `Device::CameraStatus::tiny`, packed,
bitfields LSB first) name every byte. The SDK uses this layout for the
Tiny, Tiny 4K, Tiny 2 series, Tiny SE, Meet 2 and Meet SE, and the Tiny 3
matches it too. Bytes marked ✅ were measured from Linux.

| Byte | Field | Notes |
|---|---|---|
| 0 | `ai_target` / `length` | `2e` (46) on the Tiny 3 |
| 1–2 | `rvd1`, `rvd2` ("not used") | [1] follows Locked Target on the Tiny SE; [2] is 1 while asleep ✅ |
| 3 | `anti_flicker` | |
| 4–5 | `zoom_ratio`, u16 | 0–100 = 1x–4x, linear ✅ (Tiny 3: 2x 33, 3x 67, 4x 100) |
| 6 | `hdr` | ✅ |
| 7 | `face_ae` (AE mode) | ✅ |
| 8 | `noise_cancellation` | SDK says on/off; the Tiny SE and Tiny 3 take and report 0–3 ✅ |
| 9 | `dev_status` | 1 run, 3 sleep, 4 privacy |
| 10–11 | `auto_sleep_time`, i16 s | SDK: 0 means never sleep |
| 12 | `vertical` | portrait mode |
| 13 | `face_auto_focus` (AF mode) | ✅ |
| 14 | `auto_focus` | |
| 15 | `manual_focus_value` | |
| 16 | `sleep_micro` | mic during sleep ✅ |
| 17 | `fov` | 0 86°, 1 78°, 2 65°, 3 none |
| 18 | `rvd3` | |
| 19 | `image_flip_hor` | mirror ✅ |
| 20 | `voice_ctrl_language` | 0 Chinese, 1 English |
| 21 | `voice_ctrl` | one bit per voice command |
| 22–23 | `voice_ctrl_zoom`, u16 | 0–100 |
| 24 | `ai_mode` (AiWorkModeType) | 0 off, 1 group, 2 human, 3 hand, 4 whiteboard, 5 desk, **6 switching** (a change in progress) ✅, 7 speech, 14 portrait tracking |
| 25 | `audio_auto_gain` | ✅ |
| 26 | `sleep_bg_type` | bits 0–3 image, 4–7 video |
| 27 | `bg_img_idx` | |
| 28 | `ai_sub_mode` (AiSubModeType) | normal, upper body, close-up, headless, lower body |
| 29 | `bg_img_mirror` | ✅ |
| 30 | `hdr_support` | HDR possible in the current mode |
| 31 | `fps` | current stream fps ✅ (`1e` 30, `0f` 15, `3c` 60) |
| 32 | `boot_mode` | bits 0–4 sub-mode, 5–7 AI mode |
| 33 | `led_brightness_level` | 0 off, 1–3; on the Tiny 3, writing 0 keeps the level here ✅ |
| 34 | `audio_opt` | bits 0–3 pickup distance (0 near, 1 standard, 2 far), bit 4 UAC enabled ✅ |
| 35 | `ble_status` | |
| 36 | `ai_tracker_speed` | 0 normal, 2 motion |
| 37 | `live_stream_mode` | |
| 38–39 | `gesture_para` | Meet 2 / Meet SE gestures |
| 40 | `audio_mode` | bits 0–2 source, 3–7 AudioModeType (omni, stereo, front, back, dipole, music) |
| 41 | `wireless_mic` | Tiny 3 |
| 42 | `auto_frame` | low nibble landscape, high nibble portrait |
| 43 | `event_count` | number of queued camera events; query `021d` pops one ✅ |
| 44 | `kws_extend` | wake-word flags |
| 45 | `led_enable` | Tiny 3: status light on/off ✅ (see "OBSBOT Tiny 3") |
| 46 | `doa_set` | Tiny 3: sound-source assisted tracking and range |

## OBSBOT Tiny 3 (`3564:ff02`)

Tested from Linux on firmware 6.6.8.3 (bcdDevice 5.10), without captures.
Same Extension Unit (unit 2, same GUID, 60-byte selectors) and the same
selector 6 status block layout as the Tiny SE; see `profiles/tiny-3.toml`
for what was set and read back.

**Query `0401` (wire 0x0104) is the AI status.** On the Tiny 3 it returns
20 bytes: [0] tracking active, [1] AI mode (matches status[24]), [4] 0xc8,
constant. It carries no gesture switches, unlike the Tiny SE's 12-byte
reply with gestures at [3..6].

**Gesture settings use one command** (not the Tiny SE's command per
switch). Recorded from the traffic of OBSBOT's SDK (libdev 2.1.0,
`aiSetGestureParaR` / `aiGetGestureParaR`) by logging its
`UVCIOC_CTRL_QUERY` calls, then sent from Linux and tested on the camera:

| Action | Frame | Payload |
|---|---|---|
| Write | flags `0x25`, dst 04 `4434` | u32 type, then u8 switch or f32 zoom factor |
| Read | flags `0x21`, dst 04 `8434` | u32 type → reply u8 or f32 |

| Type | Setting | Tested |
|---|---|---|
| 0 | Gesture master | ✅ off disables gestures |
| 1 | Target selection: palm turns human tracking on/off | ✅ ignored while off |
| 2 | Zoom: "L" zooms to the zoom factor and back | ✅ ignored while off |
| 3 | Dynamic zoom | read back only |
| 7 | Direction mirror (Tiny 2/SE per the SDK) | reads 0; not bound |
| 8 | Zoom factor, f32 | ✅ 3.0 made "L" zoom to 3.00x |

Switching the master off also switches types 1–3 off, and switching it on
leaves them off, so gestures stay dead until each is switched on again. The
profile's master switch therefore writes all four.

The status light shows gestures: it blinks twice when one is recognised,
then shows the new state (manual: green no target, blue human tracking,
purple hand/desk/whiteboard, yellow target lost). Gestures need AI
recognition running, and a busy background makes them less reliable.
While a palm gesture starts tracking, status[24] briefly reads 6 before
settling on 2 (human).

**Camera events: a queue.** The camera queues events and status[43]
counts them; query dst 02 `021d` (flags 0x01, no payload) pops one. The
reply is a u32 source (`0x71`), a u32 type and an optional u32 value.
OBSBOT's SDK polls it every ~2 s (seen in its traffic; it never uses the
CDC ACM serial port). Read from Linux on a Tiny 3:

| Type | Value | Meaning |
|---|---|---|
| 20 | 1 / 0 | tracking target lost / found again (SDK `kEvtInfoTargetLoss`) |
| 16 | – | unknown; ten had piled up from earlier gesture testing |

Gestures themselves queue nothing: a palm shows as status[24] changing
(0 ↔ 2), an "L" as the zoom ratio at [4..5] ramping. The SDK's event list
also has Vox SE events (`kEvtTipsTWS…`: connect, battery, mute, …), which
should come through the same queue once a mic is paired.

**Presets on the Tiny 3** (traced from the SDK, tested on the camera,
firmware 6.6.8.3). The Tiny 3 keeps the Tiny SE's recall frame but stores
presets differently:

| Operation | Frame | Notes |
|---|---|---|
| Save (`aiAddGimbalPresetR`) | dst 04 `4439`, 24 bytes: slot u32; yaw, pitch, roll, zoom as int16 ×100; 12 more bytes | The camera fills the last 12 bytes itself (reads back `10 00 10 00 53 00 53 00 64 00 00 00` whatever was sent). The SDK stores exactly the values given, not the current position. |
| Name (`aiSetGimbalPresetNameWithIdR`) | `843a`: slot u32 + name | Write-only: `043b` gets no answer, and the SDK has no name reader for this model. |
| Recall (`aiTrgGimbalPresetR`) | `c439`: slot u32, 1.0 ×4 | Same as the Tiny SE. ✅ the camera turned and zoomed to the stored values. |
| Delete (`aiDelGimbalPresetR`) | `8439`: slot u32 | |
| List (`aiGetGimbalPresetListR`) | query `043d` | Stored slots, one byte each; "nothing there" (flags 0x09) when empty. |
| Info (`aiGetGimbalPresetInfoWithIdR`) | query `443a`, flags 0x21, payload slot u32 + 1 byte | The saved values as above. |
| Current position | query `0466` without payload | Motor pitch at [8], yaw at [10], ×0.1° (Tiny SE offsets). With the Tiny SE's payload byte `01` it gets no answer. |

OBSCura saves the current position (from `0466`) and zoom (`0468`) this way.

**Vox SE button** (`cameraSetTWSKeyTypeR`): dst 02 `42c0` u8, read back at
mic info [0]. 0 human tracking, 1 switch tracking mode, 2 zoom to 1.0x,
3 record (in OBSBOT Center). ✅ with 2, a press zoomed the camera from 2x to
1x.

**Field of view, tracking and voice control** (traced from the SDK, tested
on the camera, firmware 6.6.8.3):

| Feature | Write | Readback | Tested |
|---|---|---|---|
| Field of view (`cameraSetFovU`) | short `04 01 <0/1/2>` (86°/78°/65°) | status[17]; reads 3 ("none") during a gesture zoom | ✅ the picture visibly crops |
| Human framing (`cameraSetAiModeU`) | short `16 02 02 <sub>` | status[28] | ❌ stored, but the picture doesn't change |
| Tracking control parameter (`aiSetControlParaR`, human target) | dst 04 `4454`: u32 target 0, u32 parameter, value | query `8454` (flags 0x21), payload target + parameter | |
| ↳ parameter 5, speed mode | 0 super lazy, 1 lazy, 2 slow (default), 3 fast, 4 crazy | | ✅ visibly lags at 0, snaps after at 4 |
| ↳ parameter 0, motion mode | u8 0/1 | | stored and read back; effect not checked |
| `aiSetTrackingModeR`, `aiSetTrackSpeedTypeR` | dst 04 `c40c` u8, `4409` u8 | none | not used (no readback; `c40c` found ignored by brendanwelsh) |
| Voice control (`cameraSetAudioCtrlStateU`) | short `15 02 <command> <value>` | commands 0–6: bit n of status[21]; 100 voice zoom (0–100 = 1x–4x) at status[22]; 101 language (0 Chinese, 1 English) at status[20] | ✅ "Unlock Me"/"Track Me" by voice, ignored when switched off |
| Sound-assisted tracking (`cameraSetDoaFindBack`, exported, not in the header) | short `25 01 <0/1>` | status[46] bit 0 (on by default) | ✅ turned ~80° toward a voice out of view; not when off |
| DOA range (`cameraSetDoaRange`) | short `24 01 <n>` | status[46] bit 1; 2 reads back as 1 | not used (meaning undocumented) |
| Audio mode (`cameraSetAudioModeU`, exported, not in the header) | short `22 02 <source 0> <mode>` | status[40] bits 3–7: 0 omni, 1 stereo (default), 2 front, 3 back, 4 front+back, 5 music | ❌ stored, but speech in front was no quieter in Back than in Front; USB audio is mono |
| Privacy (`cameraSetDevRunStatusR(4)`) | sends `c2a0` u32 1 | status[9] = 3 | same as sleep; not a separate mode |

Status[36] (`ai_tracker_speed`) didn't follow any of the tracking settings.
The voice command bits are 0 Hi Tiny, 1 Sleep Tiny, 2 Track Me, 3 Unlock Me,
4 Zoom In, 5 Zoom Out, 6 presets (the SDK header lists them in another
order).

**Vox SE wireless microphones: two slots, TX1 and TX2.** Frames traced from
OBSBOT's SDK; commands for a mic go to the mic itself, dst `0x58` (TX1) or
`0x98` (TX2), relayed by the camera.

| Action | Frames |
|---|---|
| Pair a slot ✅ (TX1 and TX2) | flags 0x05 to dst 58/98 `130c` (no payload), then flags 0x21 to dst `13` `0e0c` payload `00` (SDK `setBlePairingEnable`); then hold the mic's button ~6 s |
| Stop pairing | flags 0x05 to dst 58/98 `530c` |
| Forget a slot ✅ | flags 0x05 to dst 58/98 `930e`: TX1 dropped offline and didn't reconnect on its own; auto-select's "has pairing record" stayed 1 |
| Mute a mic ✅ | dst 58/98 `d32d` u8; read back at mic info [2] bit 0/1 |
| Mic gain ✅ | dst 58/98 `532d` i32; read back at mic info [5]/[6] (i8). Any value -30..+30 is stored; OBSBOT publishes no range |
| Audio source ✅ | dst 02 `4289` u8: 0 built-in, 3 wireless; read at status[40] bits 0–2 |
| Auto-select ✅ | dst 02 `c28a` u8; query `828a` → `[supported, on, has pairing record]` |
| Mic info ✅ | query dst 02 `02c0` → 16 bytes, below |

Neither pairing frame works alone: TX1 paired only when both were sent
before putting the mic into pairing mode (found by the user trying the
traced commands). The same Vox SE also paired on TX2 after being forgotten
on TX1 (status[41] = 04). The camera then switched the source to the wireless mic
and turned auto-select on; with the mic back in its charging case it
switched back to the built-in mics. Status[41] (`wireless_mic`): bit 0 TWS
mode, bits 1–2 TX1/TX2 online, bit 3 Bluetooth connected, bit 4 scanning.

Mic info (SDK `DevTWSInfo`), with one Vox SE on TX1 at 80 %:
`00 27 00 02 02 00 00 50 00 00 00 …`

| Byte | Field |
|---|---|
| 0 | button function (0 track, 1 switch tracking mode, 2 zoom 1x, 3 record) |
| 1 | bits: button, vibration, LED, sound mode (2 bits), auto shutdown |
| 2 | bit 0/1 mic 1/2 muted, bit 2/3 mic 1/2 noise suppression |
| 3, 4 | noise suppression level, mic 1/2 |
| 5, 6 | gain, mic 1/2 |
| 7, 8 | battery percent, mic 1/2 ✅ |
| 9, 10 | charging, mic 1/2 ✅ |

The per-mic SDK reads (battery, mute, gain, name `Vox SE 1EBB48`, serial,
version 0x02030700) answer once a mic is paired.

Earlier tests: the Tiny SE's gesture commands (`c430`, `4431`, `4433`,
`c433`) are accepted and ignored by the Tiny 3.

On startup the SDK also sends the system module (dst 0d, `c81a`) a shell
command, `touch /app/private/resolution.conf`, so it can run commands on
the camera's Linux system. Nothing here sends that.

**More AI modes.** Short setting `16 02 <mode> 00` also takes 4
Whiteboard, 5 Desk and 7 Speech (OBSBOT's "Voice Tracking"), each echoed at
status[24]; 7 also set status[28] (sub-mode) to 2. 14, portrait tracking
in the SDK, is ignored.

**Status light: on/off and brightness are separate.** Short setting `1a`
with 1–3 sets the brightness ([33]) and switches the light on ([45] = 1).
With 0 it switches the light off ([45] = 0) and leaves the brightness at
[33], unlike the Tiny SE, where [33] drops to 0. Switching on again means
writing the brightness from [33].

Independent Tiny 3 notes, which agree with the above where they overlap:
[joshualambert/obsbot-tiny3-linux](https://github.com/joshualambert/obsbot-tiny3-linux)
(AI mode short setting `16 02 <category> <submode>`, sleep, recenter) and
[brendanwelsh/obsbot-tiny3-protocol](https://github.com/brendanwelsh/obsbot-tiny3-protocol)
(which Tiny 2 commands the Tiny 3 ignores, and its AI quick status).
