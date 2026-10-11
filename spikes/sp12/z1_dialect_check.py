# SPDX-License-Identifier: AGPL-3.0-or-later
"""SP-12 spike (throwaway): split a file the way the Z1 firmware does.

Transcribes the line handling in GcodeDispatch::on_console_line_received
(MakeraInc/MakeraZ1Firmware b3a2e26, src/modules/communication/GcodeDispatch.cpp):
everything from the first ';' or '(' is dropped, then the line is cut before
each further G, M or T word (and S when the line also has an M), and each
piece is run as its own command. Prints the commands, and flags pieces that
lose words or use codes the firmware ignores.

Run: python3 -I z1_dialect_check.py z1_conventions_test.cnc
"""

import sys

# G codes Robot::on_gcode_received acts on (plus G28/G38/G53 handled elsewhere).
KNOWN_G = {"0", "1", "2", "3", "4", "10", "17", "18", "19", "20", "21", "28", "38.2",
           "53", "54", "55", "56", "57", "58", "59", "90", "91", "92", "92.1",
           "92.4"}
SILENT_G = {"93", "94"}  # parsed, no case in the firmware: silently ignored


def split_line(line):
    for c in ";(":
        cut = line.find(c)
        if cut != -1:
            line = line[:cut]
    line = line.strip()
    pieces = []
    while line:
        first = line[0]
        if first == "G":
            seps = "GMST" if ("S" in line[2:] and "M" in line[2:]) else "GMT"
            pos = min((i for i in (line.find(c, 2) for c in seps) if i != -1), default=-1)
        elif first == "M":
            pos = min((i for i in (line.find(c, 2) for c in "GM") if i != -1), default=-1)
        elif first in "TS":
            m = line.find("M", 2)
            if m == -1:
                pos = min((i for i in (line.find(c, 2) for c in "GST") if i != -1), default=-1)
            else:
                pos = min((i for i in (line.find(c, m + 2) for c in "GMST") if i != -1), default=-1)
        else:
            pos = -1
        piece, line = (line, "") if pos == -1 else (line[:pos], line[pos:])
        pieces.append(piece.strip())
    return pieces


def main(path):
    problems = 0
    with open(path, encoding="ascii") as fh:
        for n, raw in enumerate(fh, 1):
            raw = raw.rstrip("\n")
            stripped = raw.lstrip()
            if stripped.startswith(("(", ";")) and ")" in stripped and not stripped.endswith(")"):
                print(f"{n}: words after a comment are dropped: {raw}")
                problems += 1
            for piece in split_line(raw):
                note = ""
                if piece.startswith("G"):
                    code = piece[1:].split()[0]
                    if code in SILENT_G:
                        note = "  <- ignored by the firmware"
                    elif code not in KNOWN_G:
                        note = "  <- not handled by Robot"
                        problems += 1
                print(f"{n:3d}: {piece}{note}")
    print(f"{problems} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
