# Capturing OBSBOT Center USB traffic (Windows)

How to record what OBSBOT Center sends the camera, so OBSCura can do the same.
It was written for the Tiny SE captures; for another model, use that camera
wherever this says Tiny SE. What still needs recording is listed in
[`captures/README.md`](../captures/README.md).

**On Linux, with OBSBOT's SDK**, there's an alternative that needs no
Windows: record the frames the SDK sends instead
([`tools/sdk-trace/`](../tools/sdk-trace/README.md)). That's how the Tiny 3's
gestures, events and Vox SE commands were found.

### 1.1 One-time setup
1. **Firmware:** OBSBOT Center shows "Device 1 update". Choose now whether to update, then **don't change firmware again** until all captures are done. Updating first is recommended, so the protocol we decode matches current firmware. Note the final firmware version in `NOTES.md`.
2. Install **Wireshark for Windows** (https://www.wireshark.org/download.html, 64-bit installer). On the component screen, **check "USBPcap"**, and accept the USBPcap driver install. **Reboot** when it finishes.
3. Plug the Tiny SE **directly into a laptop USB port**, not a hub or dock. If you can, use a port that nothing else (keyboard, mouse, dongle) is on, which keeps unrelated traffic out.
4. Close every app that could open the camera or its mic: Teams, Zoom, Camera, browsers with meeting tabs, OBS.
5. Find the camera's root hub. Open **Command Prompt as Administrator** and run:
   ```
   "C:\Program Files\USBPcap\USBPcapCMD.exe"
   ```
   It lists `\\.\USBPcap1`, `\\.\USBPcap2`, and so on, with the devices attached to each. Note which one has the OBSBOT or Tiny SE composite device (video + audio). Press Ctrl+C or close the window when done.

### 1.2 Wireshark capture settings (apply to every capture)
1. Open Wireshark **as Administrator**.
2. On the start screen, find the `USBPcapN` interface from step 1.1.5 and click the **gear icon** next to it:
   - Snapshot length: **262144** (or leave the default; don't lower it)
   - ✅ **Inject already connected devices descriptors into capture data** (keep this on for every capture, because it gives me the descriptors each time)
   - In the device tree, check **only the OBSBOT device** if it's listed. Otherwise check "Capture from all devices connected".
   - For capture `00` only, also check ✅ **Capture from newly connected devices**.
3. Double-click the interface to start. Use ■ to stop.
4. Save with **File → Save As…**, format **pcapng**, and name it exactly as in the table below. Don't apply display filters and don't use "Export Specified Packets". I need the whole capture.

### 1.3 How to perform each capture
- **Close the preview** in OBSBOT Center ("Close Preview" button) unless the table says otherwise. This stops video streaming, keeps files small, and leaves only control traffic.
- Start the capture and **wait 5 s** before the first action. Then do one action at a time, with **about 5 s between actions**. The gaps are how I line packets up with your actions.
- **Toggles:** ON → OFF → ON → OFF.
- **Choice lists and segmented buttons:** select every option in order, then go back to the original.
- **Sliders:** don't drag. **Type exact values into the number box**: min, max, a middle value, then two neighbouring values (for example 0, 100, 50, 51, 52). If there's no number box, drag to each end and to the middle once.
- Wait 5 s after the last action, then stop.
- In `captures/NOTES.md`, add one entry per file listing the actions in order. Include anything else you noticed, such as the camera moving, the LED changing, or an option that hid or showed other controls. Wall-clock times are nice to have but optional.
- **If a control shows options we haven't seen** (the Hand Tracking dropdown, the Noise Reduction/Distance/Sleep Time lists, the unidentified header icons, manual exposure or focus sliders that appear when Auto is off), screenshot them into `reference-screenshots/` and capture them too.

### 1.4 Capture list
Batch **A** first: send those four files and I'll check the pipeline before you do the rest (batch **B**).

| # / filename | Batch | Preview | Actions |
|---|---|---|---|
| `00-enumeration-startup.pcapng` | A | as app defaults | Close OBSBOT Center. **Unplug the camera.** Start the capture (with "newly connected devices" checked). Plug the camera in, wait 10 s, launch OBSBOT Center, wait 30 s, stop. This records the descriptors and the app's full state read at startup. |
| `01-idle-baseline.pcapng` | A | closed | App open, touch nothing for **60 s**. This shows the app's background polling so I can filter it out of everything else. |
| `10-gesture-master.pcapng` | A | closed | Gesture Control master toggle ×4 |
| `40-contrast.pcapng` | A | closed | Contrast: 0, 100, 50, 51, 52, reset icon |
| `02-known-state-readback.pcapng` | B | closed | Before capturing, set these: Contrast 23, Saturation 77, Status Light brightness 2, Anti-Flicker 50 Hz, Gimbal speed Slow. Close the app. Start the capture, launch the app, wait 30 s, stop. Record the values in NOTES. This lets me decode read responses. |
| `11-gesture-locked-target` / `12-gesture-zoom` / `13-gesture-zoom-factor` / `14-gesture-dynamic-zoom` / `15-gesture-direction-flip` | B | closed | Each sub-toggle ×4. For Zoom Factor, type min, max, 1.5, 2.0, 2.1 |
| `16-mirror-image` | B | closed | Mirror Image ×4 |
| `17-header-icon-3`, `18-header-icon-4`, `19-header-icon-5` | B | closed | The other header icons: open each popover and exercise every control. Describe each icon in NOTES. |
| `20-ai-human` / `21-ai-group` / `22-ai-hand` | B | closed | Select mode, then deselect, ×2 |
| `23-ai-hand-dropdown` | B | closed | Each option in the Hand Tracking dropdown |
| `24-ai-lock` | B | closed | Click lock → OK ("Disable AI features and lock"), unlock, repeat. Then lock → **No**. |
| `25-gesture-disable-all-prompt` | B | closed | Trigger "Disable all Gesture Control features at the same time?", answer OK, then repeat and answer No |
| `30-gimbal-speed` | B | closed | Slow, Med, Fast, back |
| `31-gimbal-joystick` | B | **open** | Hold up 2 s, release. Then down, left, right. Then one short tap in each direction. Then the View & Gimbal reset icon. Keep it under 60 s. |
| `32-manual-zoom` | B | closed | Type 1.0, max, 2.0, 2.1, 1.0 |
| `33-rotate-flip` | B | closed | −90 button, +90 button, flip icon, each twice |
| `34-presets` | B | closed | Move the gimbal, Add preset 1. Move it again, Add preset 2. Recall 1, recall 2. Delete or clear a preset if the UI allows it. |
| `35-resolution-aspect` | B | **open** | 720p, 1080p, 30 fps, 60 fps, Portrait, Landscape. Keep it short, about 40 s. |
| `36-sleep-resume` | B | closed | Sleep button, wait 10 s, Resume. Repeat. |
| `41-saturation` / `42-sharpness` / `43-hue` | B | closed | Same as contrast |
| `44-hdr` | B | closed | ×4 |
| `45-autofocus` | B | closed | AF off ×1 (exercise any manual focus slider that appears: min, max, middle), AF on. Then AF Mode Global/Face ×2. |
| `46-autoexposure` | B | closed | AE Mode Global/Face ×2. Compensation: min, max, 0, 1, 2. AE off: exercise every manual slider (exposure, ISO, and so on) with min, max, middle. AE on. |
| `47-anti-flicker` | B | closed | Off, 50, 60, back |
| `48-white-balance` | B | closed | AWB off. Temperature: min, max, 4800, 4900, 5000. AWB on. |
| `50-audio` | B | closed | Mic-status-during-sleep ×2, every Noise Reduction option, Auto Gain ×2, Disable Microphone ×2, every Radio Distance option |
| `51-device-sleep` | B | closed | Auto Sleep ×2, every Sleep Time option, Sleep Background: black then default image, Sleep Background Mirror ×2 |
| `52-sleep-bg-custom` | B | closed | Upload a small custom sleep-background image. Put a copy of the same image file in `captures/`. |
| `53-status-light` | B | closed | Status Light ×2, Brightness 1, 2, 3 |
| `54-gimbal-reverse` | B | closed | View and Gimbal Reverse ×2 |
| `55-version-info` | B | closed | Click "Vers. Info", and "Export Log" if it asks the device for anything |
| `99-factory-reset` | B (last, optional) | closed | Factory Reset. Do this **last**, because it wipes your settings. |

**Don't capture firmware upgrades.** We won't implement flashing.

### 1.5 Returning the data
1. On this Linux machine, create `/home/ken/src/github/kenvandine/obsbot/captures/`.
2. Copy all `.pcapng` files and `NOTES.md` into it, using a USB stick, `scp`, or a network share (zip them if that's easier). Put new screenshots in `reference-screenshots/`.
3. Install the analysis tools here: `sudo apt install tshark v4l-utils`. Answer "No" to letting non-superusers capture; we only read files.
4. Plug the Tiny SE into **this Linux machine** so I can read its descriptors and test commands (`lsusb -v`, `v4l2-ctl --all`, `v4l2-ctl --list-ctrls-menus`).
5. Tell me here, for example: "Batch A is in ./captures". The files stay local, so nothing gets uploaded. Note that the two captures with the preview open contain video frames of you.

---
