#!/usr/bin/env python3
"""Read the selector 6 status block (read-only) and print the mic-related bytes.

Usage: xustatus.py [/dev/videoN] [--raw]
"""
import ctypes
import fcntl
import os
import sys


class Query(ctypes.Structure):
    _fields_ = [("unit", ctypes.c_uint8), ("selector", ctypes.c_uint8),
                ("query", ctypes.c_uint8), ("size", ctypes.c_uint16),
                ("data", ctypes.c_void_p)]


UVCIOC_CTRL_QUERY = (3 << 30) | (ctypes.sizeof(Query) << 16) | (ord("u") << 8) | 0x21
GET_CUR = 0x81


def status(dev):
    fd = os.open(dev, os.O_RDWR)
    try:
        buf = (ctypes.c_uint8 * 60)()
        fcntl.ioctl(fd, UVCIOC_CTRL_QUERY, Query(2, 6, GET_CUR, 60, ctypes.addressof(buf)))
        return bytes(buf)
    finally:
        os.close(fd)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    b = status(args[0] if args else "/dev/video4")
    if "--raw" in sys.argv:
        print(b.hex(" "))
        return
    mic, audio = b[41], b[40]
    print(f"[41]={mic:02x} tws={mic & 1} tx={(mic >> 1) & 3} pairing={(mic >> 3) & 1} "
          f"scanning={(mic >> 4) & 1} | [40] source={audio & 7} mode={audio >> 3} "
          f"| [34]={b[34]:02x} asleep={b[2]}")


main()
