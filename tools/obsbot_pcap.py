#!/usr/bin/env python3
"""Decode OBSBOT UVC control traffic from USBPcap pcapng captures.

Finds the OBSBOT device (VID 0x3564) from its descriptors, pairs control
transfer requests with their completions, and prints:
  * every SET_CUR (host -> camera) with unit/selector and payload
  * GET_CUR results whose content changed since the last read of the same
    unit/selector (so periodic status polling only shows diffs)

Usage: obsbot_pcap.py CAPTURE [--all-gets] [--raw]
"""
import argparse
import struct
from dataclasses import dataclass

OBSBOT_VID = 0x3564
REQ = {0x01: "SET_CUR", 0x81: "GET_CUR", 0x82: "GET_MIN", 0x83: "GET_MAX",
       0x84: "GET_RES", 0x85: "GET_LEN", 0x86: "GET_INFO", 0x87: "GET_DEF"}


def blocks(path):
    with open(path, "rb") as f:
        endian = "<"
        while True:
            hdr = f.read(8)
            if len(hdr) < 8:
                return
            btype = struct.unpack("<I", hdr[:4])[0]
            if btype == 0x0A0D0D0A:
                bom = f.read(4)
                endian = "<" if bom == b"\x4d\x3c\x2b\x1a" else ">"
                blen = struct.unpack(endian + "I", hdr[4:])[0]
                f.seek(blen - 12, 1)
                continue
            blen = struct.unpack(endian + "I", hdr[4:])[0]
            body = f.read(blen - 8)
            if btype == 6:
                _i, hi, lo, caplen, _o = struct.unpack(endian + "IIIII", body[:20])
                yield ((hi << 32) | lo) / 1e6, body[20:20 + caplen]


@dataclass
class Ctrl:
    t: float
    bm: int
    req: int
    wvalue: int
    windex: int
    wlength: int
    out_data: bytes
    in_data: bytes = b""
    done: bool = False

    @property
    def unit(self):
        return self.windex >> 8

    @property
    def selector(self):
        return self.wvalue >> 8


def transfers(path):
    """Yields completed control transfers to the OBSBOT device."""
    pending = {}
    obsbot_dev = None
    t0 = None
    for ts, pkt in blocks(path):
        t0 = ts if t0 is None else t0
        hlen, irp, _st, _fn, info, _bus, dev, _ep, xfer, dlen = struct.unpack("<HQIHBHHBBI", pkt[:27])
        if xfer != 2:
            continue
        stage = pkt[27]
        data = pkt[hlen:hlen + dlen]
        is_completion = info & 1
        if not is_completion and stage == 0:
            bm, req, wv, wi, wl = struct.unpack("<BBHHH", data[:8])
            pending[irp] = Ctrl(ts - t0, bm, req, wv, wi, wl, data[8:])
            continue
        c = pending.pop(irp, None)
        if c is None:
            continue
        c.in_data = data
        # Device descriptor response identifies the OBSBOT.
        if c.bm == 0x80 and c.req == 0x06 and c.wvalue == 0x0100 and len(data) >= 12:
            if struct.unpack("<H", data[8:10])[0] == OBSBOT_VID:
                obsbot_dev = dev
        if dev == obsbot_dev:
            yield c


def decode(body):
    """Hex plus float32/int32 hints for 4-byte-aligned payloads."""
    out = body.hex(" ")
    if body and len(body) % 4 == 0 and len(body) <= 24:
        floats = struct.unpack(f"<{len(body)//4}f", body)
        if all(abs(v) < 1e5 and (v == 0 or abs(v) > 1e-4) for v in floats):
            out += "  f32=" + ",".join(f"{v:.3f}" for v in floats)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--all-gets", action="store_true", help="print unchanged GET_CUR results too")
    ap.add_argument("--raw", action="store_true", help="print every transfer")
    args = ap.parse_args()

    last = {}
    for c in transfers(args.capture):
        name = REQ.get(c.req, f"req{c.req:02x}")
        key = (c.unit, c.selector)
        if c.bm == 0x21 and c.req == 0x01:
            d = c.out_data
            if c.selector == 2 and d[:1] == b"\xaa" and not args.raw:
                n = struct.unpack("<H", d[12:14])[0]
                body = d[16:16 + n] if d[1] == 0x25 else d[12:24]
                print(f"{c.t:8.3f} CMD f={d[1]:02x} seq={struct.unpack('<H', d[2:4])[0]:<4} "
                      f"dst={d[9]:02x} cmd={d[10]:02x}{d[11]:02x} len={n:<3} {decode(body)}")
            else:
                print(f"{c.t:8.3f} SET u{c.unit} s{c.selector:<2} {c.out_data.hex(' ')}")
        elif c.bm == 0xA1 and c.req == 0x81:
            prev = last.get(key)
            last[key] = c.in_data
            if args.all_gets or args.raw or prev != c.in_data:
                if prev is not None and len(prev) == len(c.in_data):
                    diff = [f"[{i}] {prev[i]:02x}->{b:02x}" for i, b in enumerate(c.in_data) if prev[i] != b]
                    print(f"{c.t:8.3f} GET u{c.unit} s{c.selector:<2} changed: {', '.join(diff)}")
                else:
                    print(f"{c.t:8.3f} GET u{c.unit} s{c.selector:<2} = {c.in_data.hex(' ')}")
        elif args.raw:
            print(f"{c.t:8.3f} {name} bm={c.bm:02x} u{c.unit} s{c.selector} out={c.out_data.hex(' ')} in={c.in_data.hex(' ')}")


if __name__ == "__main__":
    main()
