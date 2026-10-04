# SDK trace tools

Tools for finding a camera's vendor commands on Linux, without Windows
captures: call a function of OBSBOT's own SDK (`libdev`) and record the
Extension Unit frames it sends, then implement and test those frames in
OBSCura. This is how the Tiny 3's gesture command, its event queue and
the Vox SE wireless mic commands were found.

**No SDK code is in this repository.** Download OBSBOT's SDK yourself
(<https://www.obsbot.com/sdk>), unpack it outside the repo, and check the
terms it comes with before tracing it. Never commit SDK files.

| File | What it does |
|---|---|
| `xulog.c` | `LD_PRELOAD` shim: logs every `UVCIOC_CTRL_QUERY` (unit, selector, request, bytes) to stderr. `libdev` talks to the camera through plain `ioctl` on `/dev/videoN`, so this needs no root. |
| `gprobe.cpp` | Calls the SDK's gesture functions (`aiGet/SetGestureParaR`), with a `=== marker` line before each call so the log lines up with the API. |
| `micprobe.cpp` | Calls the Vox SE / audio functions. They're exported by `libdev` but not declared in its header, so they're bound by symbol name (`nm -D` lists them). |
| `evprobe.cpp` | Registers the SDK's event and status callbacks and logs everything it receives, with timestamps. This is how the camera's event queue (`021d`) was found. |
| `evnames.py` | Adds `RmEventType` names to `evprobe` output: `evnames.py <SDK dir> < ev.log`. |
| `xustatus.py` | Reads the selector 6 status block (read-only). `--raw` prints all 60 bytes. |
| `xuframe.py` | Builds a selector 2 frame exactly like `vendor/protocol.rs`, and sends it with `--send`. |
| `build.sh` | Builds `xulog.so`, `gprobe`, `micprobe` and `evprobe` into `build/` (git-ignored). |

```sh
SDK=~/obsbot/sdk-work/libdev_v2.1.0_8 tools/sdk-trace/build.sh
cd tools/sdk-trace/build

# Read first: trace the SDK reading the gesture settings.
LD_PRELOAD=$PWD/xulog.so ./gprobe get 2> trace.log
grep -E "^===|XU SET unit=2 sel=2" trace.log

../xustatus.py /dev/video4          # mic/audio summary
../xustatus.py /dev/video4 --raw    # the whole status block
```

Other models: `OBSBOT_PRODUCT=<ObsbotProductType number from dev.hpp>`
before `gprobe`/`micprobe` (they default to the Tiny 3, 18).

## Reading a trace

- Lines start with `XU SET`/`XU GET`, then the unit, selector and request
  (`q=0x01` SET_CUR, `q=0x81` GET_CUR), then the bytes, zero padding
  trimmed. Selector 2 frames are laid out in `docs/protocol.md`
  ("Framed commands").
- The SDK reuses its send buffer, so bytes past a frame's payload length
  are left over from earlier frames. On startup that includes the text of
  a shell command the SDK sends the camera (`touch
  /app/private/resolution.conf`). Ignore the leftovers, and never copy
  that command.
- Parameters can be in the header instead of the payload. The Vox SE
  pairing command, for example, picks the mic with the destination
  (`58`/`98`) and on/off with the command bytes.
- Repeated empty `XU GET` lines are the SDK polling for a reply that
  never came: the function timed out.

## Before shipping a traced command

A frame the SDK sends is a lead, not proof. The Tiny 3 accepts many
commands and ignores them. Send it from Linux (`xuframe.py --send`, or a
profile binding with `obsbotctl set … --check`), confirm the effect with
an oracle (status byte, query reply, status light, or the picture), put
the user's setting back, and record what you measured in
`docs/protocol.md`.
