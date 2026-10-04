#!/usr/bin/env bash
# Builds the SDK trace tools. The SDK is not part of this repository: point
# SDK at an unpacked copy of OBSBOT's libdev (the folder holding include/ and
# linux/x86_64-release/).
#
# Usage: SDK=~/obsbot/sdk-work/libdev_v2.1.0_8 tools/sdk-trace/build.sh

set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
: "${SDK:?set SDK to the unpacked libdev folder (with include/ and linux/)}"
lib="$SDK/linux/x86_64-release"
[ -f "$lib/libdev.so.1.0.0" ] || { echo "no $lib/libdev.so.1.0.0" >&2; exit 1; }

# libdev ships one file; the linker and loader want the usual names.
ln -sf libdev.so.1.0.0 "$lib/libdev.so.1"
ln -sf libdev.so.1.0.0 "$lib/libdev.so"

out="$here/build"
mkdir -p "$out"
gcc -shared -fPIC -O2 -o "$out/xulog.so" "$here/xulog.c" -ldl
for tool in gprobe micprobe evprobe; do
  g++ -std=c++11 -O1 -w -I"$SDK/include" "$here/$tool.cpp" \
    -L"$lib" -ldev -Wl,-rpath,"$lib" -lpthread -o "$out/$tool"
done
echo "built: $out/{xulog.so,gprobe,micprobe,evprobe}"
