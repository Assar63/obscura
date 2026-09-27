# Remaining captures

What's still needed from OBSBOT Center on Windows, and what each capture
should tell us. Setup, Wireshark settings and pacing are the same as in
[`docs/CAPTURING.md`](../docs/CAPTURING.md) §1.1–1.3. In short:

- Stay on firmware **6.4.4.1**.
- Preview **closed** unless a capture says otherwise.
- Wait 5 s before the first action, about 5 s between actions, and 5 s after the last.
- Toggles: ON → OFF → ON → OFF. Lists: every option in order, then back to the original.
  Sliders: type min, max, middle, then two neighbouring values.
- Add one entry per file to `NOTES.md` with the actions in order and anything you saw
  (LED, gimbal movement, controls that appeared or disappeared).
- Screenshot every dropdown's full option list and any new panel into `reference-screenshots/`.

"Expect" below is my best guess. If a capture shows **no USB traffic at all**
for an action, that's a useful answer too: it means OBSBOT Center does it on the
PC, and we'll do the same in the app.

---

## 0. Notes and quick re-dos

| What | Why |
|---|---|
| `23-ai-hand-dropdown` NOTES entry | Still needed: the dropdown's option names, in the order picked, and which was selected at the start. Values are dst 04 `4422` u8 and `18 01 v`. |
| Noise Reduction and Radio Distance labels | `50-audio` shows Noise Reduction values 0–3 and Radio Distance 0–2, but I only know "Off" and "Close". Screenshot both dropdowns open, or list the labels top to bottom. The app shows Low/Medium/High and Medium/Far until then. |
| `51b-sleep-background` (new, short) | In `51-device-sleep`, choosing a black or default Sleep Background sent nothing I could see. Redo just that: black → default → black → default, about 5 s apart, and note whether the picker changed in the app. |
| Disable Microphone: which way round? | Your 9/26 screenshot shows the toggle **on**. Sending `1c 01 01` made the camera re-enumerate and set status bit 0x10. The app assumes 1 = microphone disabled; a note on what the toggle looked like before and after would confirm it. |
| `46-autoexposure` manual sliders | Optional. With AE off I only saw `4224` and two `8229` writes and no "AE on" at the end, so the manual sliders may not have been exercised. The app already does manual exposure over UVC, so only redo this if you want parity with OBSBOT Center's sliders. |

Done and decoded: `02`, `24`, `31`, `32`, `33`, `34`, `35`, `36`, `44`, `45`,
`46`, `50`, `51`, `53`, `54`. `55-version-info` is no longer needed: `02`
already shows the firmware version and serial number queries. Rotate, flip
and Portrait turned out to be PC-side in OBSBOT Center.

Short NOTES entries for `31`–`35` and `54` are still welcome (anything that
differed from the TODO steps), but nothing is blocked on them.

---

## 1. Next batch

### `17-header-icon-3`, `18-header-icon-4`, `19-header-icon-5`
- **Preview:** closed
- **Actions:** open each remaining header popover and exercise every control in it.
  **Describe and screenshot each icon** in NOTES.
- **Expect:** unknown. Probably some of the features above, reachable from a second place.
- **Unlocks:** identifies whatever is left in the header.

### `25-gesture-disable-all-prompt`
- **Preview:** closed
- **Actions:** trigger "Disable all Gesture Control features at the same time?", answer OK. Repeat, answer No.
- **Expect:** OK sends the same four frames as the master switch (`c430`, `4431`, `4433`,
  `c433`) with 0. No sends nothing. This confirms the current implementation.

## 2. Optional, do last

### `52-sleep-bg-custom`
- **Preview:** closed
- **Actions:** upload a small custom sleep-background image. **Copy the exact image file into `captures/`.**
- **Expect:** a long run of chunked frames carrying the image data, probably converted
  (resized or re-encoded) by the app first, plus a start/finish handshake. Having the source
  image lets me match the bytes.
- **Unlocks:** custom sleep images. This is the most work, so it's the lowest priority.

### `99-factory-reset`
- **Preview:** closed
- **Actions:** Factory Reset. **This wipes your settings, so do it after everything else.**
- **Expect:** a single system command (probably dst `0x0d`), then the camera reboots and drops
  off USB for about 40 s.
- **Unlocks:** Factory Reset in the More tab.

---

## Not needed

- `41-saturation`, `42-sharpness`, `43-hue`, `47-anti-flicker`, `48-white-balance`: these are
  standard UVC controls that already work, and `40-contrast` confirmed OBSBOT Center uses UVC for them.
- Firmware updates: flashing is deliberately out of scope.
