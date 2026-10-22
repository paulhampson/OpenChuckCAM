# SPDX-License-Identifier: AGPL-3.0-or-later
"""SP-14: writes the Z1 block-rate air-cut program (decision 0222 item 7).

Run: python3 -I gen_block_rate.py > block_rate_z1.nc

Air cut only: no tool, no material, attended, gate 4 rules. Each section is
100 mm of X at F1000 (6.0 s nominal) made of equal collinear blocks of one
block time. Part A moves X only; part B also advances A in every block, by
60 % of the X length's worth under the Z1 feed rule (decision 0007), so the
X length still sets each block's time and A stays far below its 3600 deg/min
limit. Sections alternate direction to stay inside X 0..100.
"""
import math

F = 1000.0                     # mm/min
TIMES_MS = [20, 10, 7, 5, 3, 2, 1]
LENGTH = 100.0                 # mm of X per section
Z = 50.0                       # tip 50 mm above the A axis (r = 50)
P = 2 * math.pi * Z + 30.0     # Z1 rule perimeter, mm
A_PER_MM = 0.6 * 360.0 / P     # deg of A per mm of X in part B


def fmt(v):
    s = f"{v:.3f}"
    return "0.000" if s == "-0.000" else s


out = [
    "(SP-14 Z1 block-rate air cut - decision 0222)",
    "(NO TOOL, NO MATERIAL. Attended. E-stop in reach.)",
    "G90 G21 G17",
    "G54",
    "G0 X0.000 Y0.000 Z50.000 A0.000",
]
a = 0.0
x0 = 0.0
section = 0
for part in ("A", "B"):
    for t in TIMES_MS:
        section += 1
        n = round(LENGTH * 60000.0 / (F * t))   # blocks in the section
        step = LENGTH / n
        sign = 1.0 if x0 == 0.0 else -1.0
        out.append(f"(section {section}: part {part}, {t} ms blocks, {n} lines)")
        # Marker before each section: Z up 5 mm and back (0.6 s), then go.
        out.append("G1 Z55.000 F1000.0")
        out.append("G1 Z50.000 F1000.0")
        a_start = a
        for k in range(1, n + 1):
            x = x0 + sign * step * k
            if part == "B":
                a = a_start + A_PER_MM * step * k
            out.append(f"G1 X{fmt(x)} Y0.000 Z50.000 A{fmt(a)} F1000.0")
        x0 = x0 + sign * LENGTH
out += ["G0 Z50.000", "G92.4 A0 S0", "G53 G0 Z-3", "G28", "M2"]
print("\n".join(out))
