# SPDX-License-Identifier: AGPL-3.0-or-later
"""SP-12 spike (throwaway): move-time models for the Makera Z1 and LinuxCNC.

The Z1 model transcribes `Robot::append_milestone` in the Makera Z1 firmware
(MakeraInc/MakeraZ1Firmware, commit b3a2e26, version 1.1.2,
src/modules/robot/Robot.cpp). It is read from source, not measured; the air
cut (AIR-CUT.md) checks it. Acceleration, per-axis rate caps and junction
blending are left out, so these are cruise-only times.

The LinuxCNC model is the documented rule for trivkins: F applies to the XYZ
length when XYZ moves; to A (degrees per minute) when only A moves.

Run: python3 -I feed_model.py
       prints the expected times for the air-cut program,
       and the SP-3 G94 file's time under both rules.
     python3 -I feed_model.py FILE.ngc Y0 Z0
       does the second for another file, with the rotary axis at (Y0, Z0).
"""

import math
import re
import sys


def z1_perimeter(y_wcs, z_wcs):
    """The firmware's 'A perimeter': 2*pi*r + 30 mm, with r from the target's
    WCS Y and Z (so it assumes G54 Y0 Z0 is on the A axis). r <= 1 mm uses r = 1."""
    r = math.hypot(y_wcs, z_wcs)
    return 2 * math.pi * (r if r > 1.0 else 1.0) + 30.0


def z1_seconds(linear_mm, delta_a_deg, f_per_min, y_wcs, z_wcs):
    """Cruise time of one G1 on the Z1 firmware, in seconds."""
    rate = f_per_min / 60.0  # the firmware's rate_mm_s
    da = abs(delta_a_deg)
    if linear_mm < 1e-5 and da < 1e-5:
        return 0.0
    p = z1_perimeter(y_wcs, z_wcs)
    if linear_mm < 1e-5:
        # A-only ("auxiliary") move: distance is degrees; the rate is raised
        # until the perimeter speed reaches F, never lowered.
        mm_per_s = p * rate / 360.0
        if mm_per_s < rate:
            rate *= rate / mm_per_s
        return da / rate
    # XYZ (and maybe A): distance is the XYZ length; if A's perimeter speed
    # would exceed F, the rate is lowered so that it equals F.
    seconds = linear_mm / rate
    if da >= 1e-5:
        a_rate = da / seconds
        mm_per_s = a_rate * p / 360.0
        if mm_per_s > rate:
            rate *= rate / mm_per_s
            seconds = linear_mm / rate
    return seconds


def linuxcnc_seconds(linear_mm, delta_a_deg, f_per_min):
    """Cruise time of one G1 under the LinuxCNC (trivkins) feed rule, G94."""
    if linear_mm >= 1e-6:
        return linear_mm / f_per_min * 60.0
    return abs(delta_a_deg) / f_per_min * 60.0


def z1_feed_for(seconds, linear_mm, delta_a_deg, y_wcs, z_wcs):
    """The F (per minute) that makes the Z1 take `seconds` for this move.

    Mixed move: F = max(L, S) / T, where S = |dA| * perimeter / 360.
    A-only move: F = |dA| * min(perimeter, 360) / 360 / T.
    """
    p = z1_perimeter(y_wcs, z_wcs)
    s = abs(delta_a_deg) * p / 360.0
    if linear_mm < 1e-5:
        return abs(delta_a_deg) * min(p, 360.0) / 360.0 / seconds * 60.0
    return max(linear_mm, s) / seconds * 60.0


# The timed moves in z1_conventions_test.cnc: (label, start XYZA, end XYZA, F).
AIR_CUT_MOVES = [
    ("T3a X only", (0, 0, 30, 0), (60, 0, 30, 0), 300),
    ("T3b A only, r = 30", (0, 0, 30, 0), (0, 0, 30, 360), 600),
    ("T3c X and A, A larger", (0, 0, 30, 360), (20, 0, 30, 540), 600),
    ("T3d X and A, X larger", (0, 0, 30, 540), (60, 0, 30, 570), 300),
    ("T4 A only, r = 60", (0, 0, 60, 210), (0, 0, 60, 570), 600),
]


def air_cut_table():
    print("| Move | G-code | Z1 rule (s) | LinuxCNC rule (s) |")
    print("| --- | --- | --- | --- |")
    for label, a, b, f in AIR_CUT_MOVES:
        lin = math.dist(a[:3], b[:3])
        da = b[3] - a[3]
        z1 = z1_seconds(lin, da, f, b[1], b[2])
        lc = linuxcnc_seconds(lin, da, f)
        words = " ".join(
            f"{n}{v:g}" for n, u, v in zip("XYZA", a, b) if u != v
        )
        print(f"| {label} | G1 {words} F{f} | {z1:.1f} | {lc:.1f} |")
    # G93 test: 'G93 G1 X10 F6'. Supported: 1/6 min. Ignored: 10 mm at 6 mm/min.
    print(f"| T5 G93 | G93 G1 X10 F6 | {600 / 60:.1f} if G93 works, "
          f"{z1_seconds(10, 0, 6, 0, 30):.1f} if ignored | 10.0 |")


WORD = re.compile(r"([A-Z])\s*([-+]?[0-9]*\.?[0-9]+)")


def file_times(path, y0, z0):
    """Sum G1 times in a G94 file under both rules. (y0, z0) is the rotary
    axis in the file's coordinates; the Z1 rule needs it at WCS Y0 Z0."""
    pos = {"X": 0.0, "Y": 0.0, "Z": 0.0, "A": 0.0}
    motion, feed = None, None
    t_z1 = t_lc = 0.0
    worst = (0.0, "")
    f_lc, f_z1 = [], []
    with open(path, encoding="ascii") as fh:
        for raw in fh:
            line = re.sub(r"\(.*?\)|;.*", "", raw).upper()
            words = WORD.findall(line)
            target = dict(pos)
            for letter, value in words:
                v = float(value)
                if letter == "G" and v in (0, 1):
                    motion = int(v)
                elif letter == "F":
                    feed = v
                elif letter in target:
                    target[letter] = v
            if target == pos:
                continue
            if motion == 1 and feed:
                lin = math.dist([pos[k] for k in "XYZ"], [target[k] for k in "XYZ"])
                da = target["A"] - pos["A"]
                a = z1_seconds(lin, da, feed, target["Y"] - y0, target["Z"] - z0)
                b = linuxcnc_seconds(lin, da, feed)
                t_z1 += a
                t_lc += b
                if b > 0:
                    # This file was compensated so LinuxCNC takes the IR time b;
                    # the F the Z1 needs for the same time:
                    f_lc.append(feed)
                    f_z1.append(z1_feed_for(b, lin, da, target["Y"] - y0, target["Z"] - z0))
                if b > 0 and a / b > worst[0]:
                    worst = (a / b, raw.strip())
            pos = target
    return t_z1, t_lc, worst, (min(f_lc), max(f_lc)), (min(f_z1), max(f_z1))


def main(argv):
    if len(argv) == 4:
        files = [(argv[1], float(argv[2]), float(argv[3]))]
    else:
        air_cut_table()
        files = []
    for path, y0, z0 in files:
        t_z1, t_lc, worst, f_lc, f_z1 = file_times(path, y0, z0)
        print(f"{path}: LinuxCNC rule {t_lc:.1f} s, Z1 rule {t_z1:.1f} s "
              f"({(t_z1 / t_lc - 1) * 100:+.1f} %); worst move x{worst[0]:.2f}: {worst[1]}")
        print(f"  G1 F in the file (LinuxCNC-compensated): {f_lc[0]:.1f} to {f_lc[1]:.1f}")
        print(f"  F the Z1 needs for the same move times:   {f_z1[0]:.1f} to {f_z1[1]:.1f}")


if __name__ == "__main__":
    main(sys.argv)
