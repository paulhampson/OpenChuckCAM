# SPDX-License-Identifier: AGPL-3.0-or-later
"""Reads a G-code file written by OpenChuckCAM and prints, for the fed moves (G1) that end past
a given X, how many there are, the greatest X and how far A turns over them:

    python3 owner-checks/gcode_past_x.py X FILE ...

A run-out along X turns A by nothing past the part's end; one that follows a ring or spiral pass
turns it. A helper of the owner checks (lane OWNERCHECKS2): it only reads the file, it does not
check that the program is safe, and it knows only the words G0, G1, X, A. AGPL-3.0-or-later.
"""
import re
import sys

WORD = re.compile(r"([GXA])(-?\d+(?:\.\d*)?)")


def main():
    limit = float(sys.argv[1])
    for path in sys.argv[2:]:
        motion, x, a, past, turned, top = None, None, None, 0, 0.0, None
        for raw in open(path, encoding="utf-8"):
            last_a, moved = a, False
            for letter, value in WORD.findall(raw.split("(")[0].upper()):
                if letter == "G":
                    if value in ("0", "00", "1", "01"):
                        motion = int(float(value))
                    continue
                if letter == "X":
                    x, moved = float(value), True
                else:
                    a, moved = float(value), True
            if moved and motion == 1 and x is not None and x > limit + 1e-6:
                past += 1
                top = x if top is None else max(top, x)
                if last_a is not None and a is not None:
                    turned += abs(a - last_a)
        print(f"{path.rsplit('/', 1)[-1]}: fed moves ending past X {limit:g}: {past}; "
              f"greatest X {top}; A turned over them: {turned:.1f} degrees")


if __name__ == "__main__":
    main()
