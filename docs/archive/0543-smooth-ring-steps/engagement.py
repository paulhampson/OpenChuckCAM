"""Width of cut along ring paths in one roughing layer, unrolled.

The layer is the plane (x along the part, y = arc length round it at the
cut radius r, periodic with C = 2 pi r). A flat end mill with its axis
radial removes a disc of radius R. Material removed per mm of path is the
width of cut (radial engagement a_e): s on a spiral or ring, 2R in a slot.
The engagement angle is the arc of the tool's edge in contact.
"""
import math
import sys

import numpy as np

R = 1.5875          # 3.175 mm end mill
s = 1.27            # 40 % stepover
r = 10.0            # cut radius, mm
C = 2 * math.pi * r
L = 8 * s           # 9 rings
h = 0.02            # grid cell, mm
ds = 0.02           # path sample, mm

nx = int((L + 2 * R + 2) / h)
ny = int(round(C / h))
x_org = -R - 1.0


def disc_offsets():
    k = int(math.ceil(R / h)) + 1
    ii, jj = np.mgrid[-k:k + 1, -k:k + 1]
    m = (ii * h) ** 2 + (jj * h) ** 2 <= R * R
    return ii[m], jj[m]


DI, DJ = disc_offsets()


def path(kind, phi_deg=0.0):
    """Polyline [(x, y, tag)] with tag 'ring' or 'step'."""
    xs = [k * s for k in range(int(round(L / s)) + 1)]
    b = r * math.radians(phi_deg)
    pts = []

    def seg(x0, y0, x1, y1, tag):
        n = max(1, int(math.hypot(x1 - x0, y1 - y0) / ds))
        for t in range(n):
            u = t / n
            pts.append((x0 + (x1 - x0) * u, y0 + (y1 - y0) * u, tag))

    if kind == "spiral":
        turns = len(xs) - 1
        seg(0, 0, 0, C, "ring")  # first turn flat, as the end ring
        seg(0, C, L, C * (turns + 1), "ring")
        return pts
    y = 0.0
    for k, x in enumerate(xs):
        last = k == len(xs) - 1
        if kind == "sharp":
            seg(x, y, x, y + C, "ring")
            y += C
            if not last:
                seg(x, y, xs[k + 1], y, "step")
        elif kind == "seam":      # blend replaces the end of the ring
            if last:
                seg(x, y, x, y + C, "ring")
                y += C
            else:
                seg(x, y, x, y + C - b, "ring")
                y += C - b
                seg(x, y, xs[k + 1], y + b, "step")
                y += b
        elif kind == "whole":     # whole ring, then a blend
            seg(x, y, x, y + C, "ring")
            y += C
            if not last:
                seg(x, y, xs[k + 1], y + b, "step")
                y += b
    return pts


def run(kind, phi=0.0):
    grid = np.ones((nx, ny), dtype=bool)
    pts = path(kind, phi)
    widths = {"ring": [], "step": []}
    first_ring_end = C / ds  # skip the first turn: a full slot on any path
    per = np.zeros(len(pts))
    for n, (x, y, tag) in enumerate(pts):
        i0 = int(round((x - x_org) / h))
        j0 = int(round(y / h))
        ii = i0 + DI
        jj = (j0 + DJ) % ny
        per[n] = grid[ii, jj].sum() * h * h / ds
        grid[ii, jj] = False
    # Average over 0.5 mm of path: the grid removes a whole column of cells
    # at once when the tool crosses a cell boundary in x.
    w = int(0.5 / ds)
    smooth = np.convolve(per, np.ones(w) / w, mode="same")
    for n, (_, _, tag) in enumerate(pts):
        if n > first_ring_end + w:
            widths[tag].append(smooth[n])
    return widths


def summary(w):
    if not w:
        return "-"
    a = np.array(w)
    return f"max {a.max():.2f} mm, 99th {np.percentile(a, 99):.2f}, median {np.median(a):.2f}"


def angle(width):
    # engagement arc of a side cut of width a_e (a_e <= 2R)
    a = min(width, 2 * R)
    return math.degrees(math.acos(1 - 2 * a / (2 * R))) if a <= R else 180 - math.degrees(
        math.acos(1 - 2 * (2 * R - a) / (2 * R)))


if __name__ == "__main__":
    print(f"tool D {2*R} mm, stepover {s} mm ({s/(2*R):.0%}), cut radius {r} mm")
    print(f"engagement arc at a_e = s: {math.degrees(math.acos(1 - s / R)):.0f} deg; slot: 180 deg")
    cases = [("spiral", 0), ("sharp", 0)] + [
        (k, p) for p in (15, 30, 45, 90) for k in ("seam", "whole")
    ]
    for kind, phi in cases:
        w = run(kind, phi)
        b = r * math.radians(phi)
        lateral = math.degrees(math.atan2(s, b)) if phi else (0 if kind == "spiral" else 90)
        print(f"{kind:6} phi {phi:3}: corner {lateral:5.1f} deg | ring: {summary(w['ring'])} | "
              f"step/blend: {summary(w['step'])}")
        sys.stdout.flush()
