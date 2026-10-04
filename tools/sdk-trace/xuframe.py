#!/usr/bin/env python3
"""Build, and optionally send, one framed command for XU selector 2, encoded
exactly like crates/obsbot-core/src/protocol.rs (encode_command).

    xuframe.py DST CMD PAYLOAD [--flags 25] [--send /dev/videoN]

DST and CMD are hex as they appear in traces (e.g. `04` `4434`), PAYLOAD is
hex bytes (`''` for none). Without --send it only prints the frame.

Only send frames you have seen work: in an SDK trace, an OBSBOT Center
capture, or docs/protocol.md. Unknown commands have crashed cameras.
"""
import argparse
import ctypes
import fcntl
import os
import random


class Query(ctypes.Structure):
    _fields_ = [("unit", ctypes.c_uint8), ("selector", ctypes.c_uint8),
                ("query", ctypes.c_uint8), ("size", ctypes.c_uint16),
                ("data", ctypes.c_void_p)]


UVCIOC_CTRL_QUERY = (3 << 30) | (ctypes.sizeof(Query) << 16) | (ord("u") << 8) | 0x21
SET_CUR = 0x01


def crc16_usb(data):
    crc = 0xFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc ^ 0xFFFF


def frame(seq, dst, cmd, payload, flags=0x25):
    p = bytearray(60)
    p[0], p[1] = 0xAA, flags
    p[2:4] = seq.to_bytes(2, "little")
    p[4:6] = (12).to_bytes(2, "little")
    p[8], p[9] = 0x0A, dst
    p[10:12] = cmd
    p[12:14] = len(payload).to_bytes(2, "little")
    p[16:16 + len(payload)] = payload
    p[14:16] = crc16_usb(p[12:16 + len(payload)]).to_bytes(2, "little")
    p[6:8] = crc16_usb(p[:12]).to_bytes(2, "little")
    return bytes(p)


def selftest():
    # 11-gesture-locked-target capture (protocol.rs locked_target_matches_capture)
    assert frame(0x20, 4, bytes.fromhex("c430"), b"\0")[:17].hex() == \
        "aa2520000c00abda0a04c4300100e63f00"


def main():
    selftest()
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dst")
    ap.add_argument("cmd")
    ap.add_argument("payload")
    ap.add_argument("--flags", default="25")
    ap.add_argument("--send", metavar="DEVICE")
    a = ap.parse_args()
    payload = bytes.fromhex(a.payload)
    f = frame(random.randint(0x100, 0xFFF0), int(a.dst, 16), bytes.fromhex(a.cmd),
              payload, int(a.flags, 16))
    print(f[:16 + len(payload)].hex(" "))
    if a.send:
        fd = os.open(a.send, os.O_RDWR)
        try:
            buf = (ctypes.c_uint8 * 60).from_buffer_copy(f)
            fcntl.ioctl(fd, UVCIOC_CTRL_QUERY, Query(2, 2, SET_CUR, 60, ctypes.addressof(buf)))
        finally:
            os.close(fd)
        print("sent")


main()
