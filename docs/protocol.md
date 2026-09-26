# OBSBOT Tiny SE protocol notes

Device: `3564:feff` "OBSBOT Tiny SE", Remo Tech Co., Ltd., bcdDevice 5.10,
firmware v6.4.4.1. Decoded from the USB captures in `captures/` (decoder:
`tools/obsbot_pcap.py`) and confirmed on the camera from Linux unless
marked *unverified*.

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
| Gimbal velocity (joystick) | dst 04 `8464` 3×f32 `[0, pitch, yaw]`, ~10 Hz while held; zeros to stop | – | *unverified* |
| Hand-tracking option (dropdown) | dst 04 `4422` u8 | ? | *meaning pending NOTES* |
| Hand-tracking limits? | `4420`/`c420` f32 −45/45, `4421`/`c421` f32 −30/30, `c424` 24 bytes, `c426` u8 | | *unverified* |
| Gimbal Speed | none (host-side scale for the joystick) | | |

## Short settings (selector 6)

| Feature | Write | Status byte | Values |
|---|---|---|---|
| Mirror Image | `14 01 v` | [19] | 0/1 ✅ |
| AI mode | `16 02 v 00` | [24] | 0 off, 1 group, 2 human, 3 hand ✅ |
| ? (hand dropdown) | `18 01 v` | | *pending* |

Status block bytes seen changing: [1] locked target, [4] and [17] tracking
state, [19] mirror, [24] AI mode.
