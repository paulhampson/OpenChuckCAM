# SPDX-License-Identifier: AGPL-3.0-or-later
"""Reads a G-code file written by OpenChuckCAM and prints where its fed moves (G1) go:

    python3 owner-checks/gcode_extent.py FILE.ngc [--below-x X] [--above-x X]

It prints the number of fed moves, their least and greatest X, and the least Z (the tool tip's
height above the rotary axis, for these projects' work offset) among the fed moves that end at an
X below --below-x or above --above-x. A helper of the owner checks (lane OWNERCHECKS): it only
reads the file, it does not check that the program is safe, and it knows only the words
G0, G1, X, Y, Z and A of these programs. AGPL-3.0-or-later, as the rest of the repository.
"""
import argparse
import re

WORD = re.compile(r"([GXYZAF])(-?\d+(?:\.\d*)?)")


def moves(path):
    """Yields (motion, x, z) at the end of each move, with modal G0/G1 and modal axes."""
    motion, x, z = None, None, None
    for line in open(path, encoding="utf-8"):
        line = line.split("(")[0].split(";")[0]
        words = WORD.findall(line.upper())
        moved = False
        for letter, value in words:
            if letter == "G" and value in ("0", "00", "1", "01"):
                motion = int(float(value))
            elif letter == "X":
                x, moved = float(value), True
            elif letter == "Z":
                z, moved = float(value), True
            elif letter in "YA":
                moved = True
        if moved and motion is not None:
            yield motion, x, z


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("file")
    parser.add_argument("--below-x", type=float)
    parser.add_argument("--above-x", type=float)
    args = parser.parse_args()
    fed = [(x, z) for motion, x, z in moves(args.file) if motion == 1 and x is not None]
    print(f"fed moves: {len(fed)}")
    if not fed:
        return
    print(f"fed X from {min(x for x, _ in fed):.3f} to {max(x for x, _ in fed):.3f}")
    for label, keep in (("below", args.below_x), ("above", args.above_x)):
        if keep is None:
            continue
        near = [(x, z) for x, z in fed
                if z is not None and (x < keep if label == "below" else x > keep)]
        if near:
            x, z = min(near, key=lambda p: p[1])
            print(f"fed moves ending at X {label} {keep:.3f}: {len(near)}, "
                  f"lowest Z {z:.3f} (at X {x:.3f})")
        else:
            print(f"fed moves ending at X {label} {keep:.3f}: 0")


if __name__ == "__main__":
    main()
