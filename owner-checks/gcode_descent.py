# SPDX-License-Identifier: AGPL-3.0-or-later
"""Reads a G-code file written by OpenChuckCAM and counts the fed moves (G1) that go down the
tool axis (Z falls) faster than a given speed, by operation:

    python3 owner-checks/gcode_descent.py FILE.ngc PLUNGE_FEED

The speed down the axis of a move is F x (Z fall) / (its X, Y, Z length), with F in mm/min as the
LinuxCNC post writes it (F applies to the X, Y and Z length; A is timed with it). Use it on a
program written with the LinuxCNC post. A helper of the owner checks (lane OWNERCHECKS2): it
only reads the file, it does not check that the program is safe, and it knows only the words G0,
G1, X, Y, Z, A and F and the "(operation ...)" comments. AGPL-3.0-or-later.
"""
import math
import re
import sys

WORD = re.compile(r"([GXYZAF])(-?\d+(?:\.\d*)?)")


def main():
    path, limit = sys.argv[1], float(sys.argv[2])
    motion, x, y, z, feed, op = None, 0.0, 0.0, 0.0, None, None
    count, fast, worst, by_op = 0, 0, 0.0, {}
    for raw in open(path, encoding="utf-8"):
        if raw.startswith("(operation "):
            op = raw[len("(operation "):].split(",")[0]
        nx, ny, nz, moved = x, y, z, False
        for letter, value in WORD.findall(raw.split("(")[0].upper()):
            if letter == "G":
                if value in ("0", "00", "1", "01"):
                    motion = int(float(value))
                continue
            value = float(value)
            if letter == "X":
                nx, moved = value, True
            elif letter == "Y":
                ny, moved = value, True
            elif letter == "Z":
                nz, moved = value, True
            elif letter == "A":
                moved = True
            elif letter == "F":
                feed = value
        if moved and motion == 1 and nz < z - 1e-9 and feed:
            length = math.dist((x, y, z), (nx, ny, nz))
            if length > 0:
                count += 1
                speed = feed * (z - nz) / length
                if speed > limit * 1.001:
                    fast += 1
                    worst = max(worst, speed)
                    by_op[op] = by_op.get(op, 0) + 1
        x, y, z = nx, ny, nz
    print(f"descending fed moves: {count}; faster down the axis than {limit:g} mm/min: {fast}; "
          f"fastest {worst:.1f} mm/min; by operation: {by_op}")


if __name__ == "__main__":
    main()
