# SPDX-License-Identifier: AGPL-3.0-or-later
# Writes baluster.stl, the model of the user guide's tutorial (a small turned and fluted piece):
#   python3 testdata/examples/baluster/generate.py testdata/examples/baluster/baluster.stl
# Deterministic: the same script always writes the same bytes. Written for this
# repository (lane DOCSUSER); AGPL-3.0-or-later, as the rest of the repository.
#
# The piece lies along X, the rotary axis, from X = 0 to X = 61 mm, millimetres:
#   X  0 to 10   plain tenon, diameter 14 (the part the chuck holds)
#   X 10         flat shoulder up to a collar of diameter 16
#   X 13 to 19   bead (a half round, diameter 22 at its crest)
#   X 19 to 25   cove (a half round groove, diameter 10 at its deepest)
#   X 25 to 48   vase that swells then tapers, with six flutes between X 29 and X 46
#   X 48 to 51   neck, diameter 7
#   X 51 to 61   ball finial, diameter 10
import math
import struct
import sys

N_THETA = 48
FLUTES = 6
FLUTE_DEPTH = 1.4
FLUTE_X0, FLUTE_X1 = 29.0, 46.0


def smooth(t):
    t = min(1.0, max(0.0, t))
    return t * t * (3 - 2 * t)


def vase(x):
    t = (x - 25.0) / 23.0
    return 8.0 + 2.8 * max(0.0, math.sin(math.pi * t)) ** 0.8 - 4.0 * smooth((t - 0.75) / 0.25)


def profile():
    """Rings as (x, radius, fluted); a repeated x makes a flat step."""
    rings = [(0.0, 7.0, False), (10.0, 7.0, False), (10.0, 8.0, False), (13.0, 8.0, False)]
    for i in range(1, 8):  # bead, centre X 16, radius 3, ends at radius 8
        a = math.pi * i / 8
        rings.append((16.0 - 3.0 * math.cos(a), 8.0 + 3.0 * math.sin(a), False))
    rings.append((19.0, 8.0, False))
    for i in range(1, 8):  # cove, centre X 22
        a = math.pi * i / 8
        rings.append((22.0 - 3.0 * math.cos(a), 8.0 - 3.0 * math.sin(a), False))
    rings.append((25.0, 8.0, False))
    x = 25.0
    while x < 48.0 - 1e-9:  # vase, every 1 mm so the flute ends are smooth
        x += 1.0
        rings.append((x, vase(x), FLUTE_X0 <= x <= FLUTE_X1))
    a0 = math.asin(3.5 / 5.0)  # where the neck meets the ball
    rings.append((48.0, 3.5, False))
    for i in range(0, 13):  # neck then ball finial, centre X 56, radius 5; ends in a point
        a = a0 + (math.pi - a0) * i / 12
        r = 0.0 if i == 12 else 5.0 * math.sin(a)
        rings.append((56.0 - 5.0 * math.cos(a), r, False))
    return rings


def flute_depth(x):
    if not (FLUTE_X0 <= x <= FLUTE_X1):
        return 0.0
    return FLUTE_DEPTH * smooth((x - FLUTE_X0) / 3.0) * smooth((FLUTE_X1 - x) / 3.0)


def main(out):
    rings = profile()
    verts = []  # per ring, a list of (x, y, z)
    for x, r, fluted in rings:
        ring = []
        for j in range(N_THETA):
            th = 2 * math.pi * j / N_THETA
            rr = r - (flute_depth(x) * (1 + math.cos(FLUTES * th)) / 2 if fluted else 0.0)
            ring.append((x, rr * math.cos(th), rr * math.sin(th)))
        verts.append(ring)
    tris = []
    # Tenon end disc, normal -X: a fan around the axis point.
    c0 = (0.0, 0.0, 0.0)
    for j in range(N_THETA):
        tris.append((c0, verts[0][(j + 1) % N_THETA], verts[0][j]))
    for i in range(len(verts) - 1):
        a, b = verts[i], verts[i + 1]
        for j in range(N_THETA):
            k = (j + 1) % N_THETA
            tris.append((a[j], a[k], b[k]))
            tris.append((a[j], b[k], b[j]))
    return tris


def normal(t):
    a, b, c = t
    u = [b[i] - a[i] for i in range(3)]
    v = [c[i] - a[i] for i in range(3)]
    n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
    ln = math.sqrt(sum(x * x for x in n))
    return [x / ln for x in n]


if __name__ == "__main__":
    tris = main(sys.argv[1])
    # The last ring is one point (the tip), so its quads collapse to a fan: drop the slivers.
    tris = [t for t in tris if math.dist(t[0], t[1]) > 1e-6 and math.dist(t[1], t[2]) > 1e-6 and math.dist(t[0], t[2]) > 1e-6]
    data = b"OpenChuckCAM tutorial baluster".ljust(80, b"\0") + struct.pack("<I", len(tris))
    for t in tris:
        data += struct.pack("<3f", *normal(t))
        for v in t:
            data += struct.pack("<3f", *v)
        data += b"\0\0"
    open(sys.argv[1], "wb").write(data)
