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

Done and decoded from the last batch: `02`, `24`, `36`, `44`, `45`, `46`, `50`,
`51`, `53`. `55-version-info` is no longer needed: `02` already shows the
firmware version and serial number queries.

---

## 1. Next batch

### `34-presets` (do this first)
- **Preview:** closed (open it briefly to aim if needed, then close before recording)
- **Actions:**
  1. Wait 5 s. Move the gimbal with the joystick, then **Add** preset 1.
  2. Move it somewhere clearly different (and change Manual Zoom), then **Add** preset 2.
  3. Recall 1, recall 2, recall 1.
  4. Rename, delete or clear a preset if the UI allows it.
  5. Close OBSBOT Center, reopen it, wait 20 s, and recall 2 again.
- **Notes:** the slot numbers you used, roughly where the camera pointed for each preset,
  whether the presets were still listed after reopening, and a screenshot of the preset
  dropdown or list.
- **Expect:** "save preset N" and "go to preset N" commands with a slot index, probably on
  dst 04. Saving may be preceded by a position query. Presets listed after the restart
  suggest they're stored on the camera, or the app queries them at startup.
- **Why a capture is needed:** from Linux, the camera's standard pan/tilt/zoom controls
  only echo the last value written. They don't change when the joystick or AI tracking
  moves the gimbal, so the app can't save "where the camera is now" without the vendor
  commands.
- **Unlocks:** Presets on the Image tab and in the tray menu.

### `32-manual-zoom`
- **Preview:** closed
- **Actions:** type 1.0, max, 2.0, 2.1, 1.0.
- **Expect:** either standard UVC zoom absolute (then I just need the 1.0×–max mapping onto the
  raw 0–12 steps) or a vendor f32 frame. Note the max value the UI allows.
- **Unlocks:** zoom shown as 1.0×–4.0× instead of raw steps.

### `54-gimbal-reverse`
- **Preview:** closed
- **Actions:** View and Gimbal Reverse ×2 (on/off/on/off).
- **Expect:** a u8 toggle (short setting or dst 04 frame), probably with a status byte. Note if
  the image flipped (for ceiling mounting).
- **Unlocks:** View and Gimbal Reverse.

### `31-gimbal-joystick`
- **Preview:** **open** (keep it under 60 s; it will contain video of you)
- **Actions:** hold up 2 s, release. Then down, left, right. Then one short tap in each
  direction. Then the View & Gimbal reset icon.
- **Expect:** dst 04 `8464` with 3×f32 `[0, pitch, yaw]` at ~10 Hz and zeros on release. This
  confirms the sign of each axis and the magnitudes. The reset icon should be a separate
  command; the app currently uses a UVC pan/tilt reset.
- **Unlocks:** marking the joystick verified, and matching OBSBOT Center's reset exactly.

### `33-rotate-flip`
- **Preview:** closed
- **Actions:** −90 button, +90 button, flip icon, each twice.
- **Expect:** possibly **no traffic** (rotation done on the PC's preview only). If there is
  traffic, it's a rotation value (0/90/180/270) and a flip u8.
- **Unlocks:** the rotate and flip buttons in the bottom bar.

### `35-resolution-aspect`
- **Preview:** **open** (about 40 s)
- **Actions:** 720p, 1080p, 30 fps, 60 fps, Portrait, Landscape.
- **Expect:** resolution and fps changes are standard UVC stream negotiation. Portrait is either
  a vendor command (camera crops) or PC-side only. Portrait is the part that matters.
- **Unlocks:** Portrait/Landscape.

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
