#!/usr/bin/env python3
"""Add event names to evprobe output, from the SDK header's RmEventType enum.

    evnames.py SDK_DIR < evprobe.log
"""
import re
import sys


def event_names(header):
    text = open(header, encoding="utf-8", errors="replace").read()
    body = re.search(r"enum\s+RmEventType\s*\{(.*?)\};", text, re.S)
    if not body:
        sys.exit("RmEventType not found in " + header)
    names, value = {}, -1
    for line in body.group(1).splitlines():
        line = re.sub(r"//.*", "", line).strip().rstrip(",")
        m = re.match(r"(k\w+)\s*(?:=\s*(-?\d+))?$", line)
        if m:
            value = int(m.group(2)) if m.group(2) else value + 1
            names[value] = m.group(1)
    return names


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    names = event_names(sys.argv[1].rstrip("/") + "/include/dev/dev.hpp")
    for line in sys.stdin:
        m = re.search(r"EVENT (-?\d+)", line)
        if m:
            line = line.rstrip("\n") + f"  <- {names.get(int(m.group(1)), '?')}\n"
        sys.stdout.write(line)


main()
