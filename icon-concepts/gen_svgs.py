#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the three icon concept SVGs (throwaway, lane ICON)."""
import math

TILE = "#22344A"; BODY = "#3E4F64"; STEEL = "#AEBBC9"; LIGHT = "#E8EEF4"; ACCENT = "#F59E1B"
HEAD = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">\n<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->\n'
TILE_RECT = f'<rect x="6" y="6" width="500" height="500" rx="104" fill="{TILE}" stroke="#4A6280" stroke-width="12"/>\n'

def pt(cx, cy, r, deg):
    a = math.radians(deg)
    return cx + r * math.cos(a), cy + r * math.sin(a)

def concept_a():
    cx = cy = 256
    s = HEAD + TILE_RECT
    s += f'<circle cx="256" cy="256" r="146" fill="{BODY}"/>\n'
    for ang in (0, 120, 240):
        s += f'<rect x="228" y="96" width="56" height="104" rx="10" fill="{STEEL}" transform="rotate({ang} 256 256)"/>\n'
    s += f'<circle cx="256" cy="256" r="60" fill="{LIGHT}"/>\n'
    # A-axis arc: 285 degrees clockwise (screen), ending in an arrowhead
    r, a0, a1 = 200, -70, 190
    x0, y0 = pt(cx, cy, r, a0); x1, y1 = pt(cx, cy, r, a1)
    s += (f'<path d="M{x0:.1f} {y0:.1f} A{r} {r} 0 1 1 {x1:.1f} {y1:.1f}" fill="none" '
          f'stroke="{ACCENT}" stroke-width="34" stroke-linecap="round"/>\n')
    # arrowhead at a0 end? put it at the start, pointing backwards (counter-clockwise is the start direction reversed)
    tip = pt(cx, cy, r, a0 + 0)  # base centre
    # tangent for clockwise travel at angle a is (-sin a, cos a); the head points along travel beyond a1
    ah = a1
    bx, by = pt(cx, cy, r, ah)
    tx, ty = -math.sin(math.radians(ah)), math.cos(math.radians(ah))
    nx, ny = math.cos(math.radians(ah)), math.sin(math.radians(ah))
    L, W = 74, 52
    p1 = (bx + tx * L, by + ty * L)
    p2 = (bx + nx * W, by + ny * W)
    p3 = (bx - nx * W, by - ny * W)
    s += f'<polygon points="{p1[0]:.1f},{p1[1]:.1f} {p2[0]:.1f},{p2[1]:.1f} {p3[0]:.1f},{p3[1]:.1f}" fill="{ACCENT}" stroke="{ACCENT}" stroke-width="8" stroke-linejoin="round"/>\n'
    return s + '</svg>\n'

def concept_b():
    s = HEAD + TILE_RECT
    cy, r = 256, 112
    xl, xr = 140, 400
    # chuck
    s += f'<rect x="52" y="104" width="100" height="304" rx="24" fill="{BODY}"/>\n'
    # stock body + end face
    s += '<clipPath id="stock"><path d="M{0} {1} H{2} A34 {3} 0 0 1 {2} {4} H{0} Z"/></clipPath>\n'.format(xl, cy - r, xr, r, cy + r)
    s += f'<path d="M{xl} {cy - r} H{xr} A34 {r} 0 0 1 {xr} {cy + r} H{xl} Z" fill="{STEEL}"/>\n'
    s += f'<ellipse cx="{xr}" cy="{cy}" rx="34" ry="{r}" fill="{LIGHT}"/>\n'
    # helical toolpath, front strands only
    pitch = 116
    paths = []
    x0 = xl - pitch / 4
    while x0 < xr:
        pts = []
        for i in range(0, 41):
            t = math.pi * i / 40
            pts.append((x0 + pitch * t / (2 * math.pi), cy - r * math.cos(t)))
        paths.append("M" + " L".join(f"{x:.1f} {y:.1f}" for x, y in pts))
        x0 += pitch
    s += f'<g clip-path="url(#stock)"><path d="{" ".join(paths)}" fill="none" stroke="{ACCENT}" stroke-width="32" stroke-linecap="butt"/></g>\n'
    s += f'<ellipse cx="{xr}" cy="{cy}" rx="34" ry="{r}" fill="{LIGHT}"/>\n'
    return s + '</svg>\n'

def concept_c():
    s = HEAD + TILE_RECT
    cy, r = 316, 72
    xl, xr = 120, 378
    s += f'<rect x="44" y="200" width="88" height="232" rx="22" fill="{BODY}"/>\n'
    s += f'<path d="M{xl} {cy - r} H{xr} A28 {r} 0 0 1 {xr} {cy + r} H{xl} Z" fill="{STEEL}"/>\n'
    s += f'<ellipse cx="{xr}" cy="{cy}" rx="28" ry="{r}" fill="{LIGHT}"/>\n'
    # end mill from above, tip touching the stock
    s += f'<rect x="198" y="40" width="88" height="70" rx="8" fill="{BODY}"/>\n'
    s += f'<path d="M204 104 H280 V214 Q242 252 204 214 Z" fill="{LIGHT}"/>\n'
    s += f'<g stroke="{TILE}" stroke-width="10" stroke-linecap="round"><path d="M210 146 L274 116"/><path d="M210 190 L274 160"/></g>\n'
    # A-axis rotation arrow round the stock (front half of a ring)
    rcx, rry, rrx = 446, 132, 44
    s += (f'<path d="M{rcx} {cy - rry} A{rrx} {rry} 0 0 1 {rcx} {cy + rry - 30}" fill="none" '
          f'stroke="{ACCENT}" stroke-width="30" stroke-linecap="round" transform="translate(-24 0)"/>\n')
    # arrowhead near bottom, pointing down-left along the ellipse
    bx, by = rcx - 24 + 14, cy + rry - 36
    s += f'<polygon points="{bx - 66},{by + 34} {bx + 30},{by - 26} {bx + 4},{by + 70}" fill="{ACCENT}" stroke="{ACCENT}" stroke-width="8" stroke-linejoin="round"/>\n'
    return s + '</svg>\n'

for name, fn in (("concept-a-jaws-a-axis", concept_a), ("concept-b-helix", concept_b), ("concept-c-tool-rotation", concept_c)):
    open(f"{name}.svg", "w").write(fn())
