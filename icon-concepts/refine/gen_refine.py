#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Concept A refinement: colour options (C1..C5) and variations (A1..A6).

Grid: 512 x 512 canvas on a 32-unit grid (16 x 16 cells, one cell = one
pixel at 16 px). Centre (256, 256). Keylines: tile edge 256, ring outer
radius 216, chuck body 128, stock 64. Main radii and widths are multiples
of 16 (half a 16-px pixel) so edges land on pixel or half-pixel lines.
"""
import math

PAL = {
    # id: tile, rim, body, jaw, stock, accent
    "C1": dict(name="slate and orange", tile="#22344A", rim="#4A6280", body="#3E4F64", jaw="#AEBBC9", stock="#E8EEF4", accent="#F59E1B"),
    "C2": dict(name="light tile", tile="#F3F5F8", rim="#C3CCD7", body="#D3DAE3", jaw="#5A6A7E", stock="#22344A", accent="#E07400"),
    "C3": dict(name="one colour (ink / white)", mono=True),
    "C4": dict(name="bold blue and amber", tile="#1556C9", rim="#3D78E0", body="#0F3F96", jaw="#DCE6F5", stock="#FFFFFF", accent="#FFB81C"),
    "C5": dict(name="graphite and signal orange", tile="#1C1D20", rim="#3A3C42", body="#33363C", jaw="#C8CDD4", stock="#F4F5F6", accent="#FF5A1F"),
}

VAR = {
    "A1": dict(name="base: rounded square, ring 48, three square jaws on a body disc", tile="rrect", ring_w=48, jaw="rect", body=True, jaw_gap=0),
    "A2": dict(name="heavy ring 64, stepped jaws, no body disc, squircle", tile="squircle", ring_w=64, jaw="stepped", body=False, jaw_gap=0),
    "A3": dict(name="circle tile, thin ring 32, tapered jaws", tile="circle", ring_w=32, jaw="wedge", body=True, jaw_gap=0),
    "A4": dict(name="mark without tile, jaws spaced from the stock", tile="none", ring_w=56, jaw="rect", body=False, jaw_gap=24),
    "A5": dict(name="wide gap and large arrowhead, short jaws, stock with a centre bore", tile="rrect", ring_w=48, jaw="short", body=True, jaw_gap=0, span=250, head=1.25, bore=True),
    "A6": dict(name="small-size glyph on the 16 px grid: ring 2 px, no body disc, short wide jaws, 1 px gaps", tile="rrect", ring_w=64, ring_outer=224, jaw="rect", body=False, jaw_gap=0, jaw_out=128, head=1.1),
}

CX = CY = 256

def pt(r, deg):
    a = math.radians(deg)
    return CX + r * math.cos(a), CY + r * math.sin(a)

def tile(kind, p):
    stroke = f' stroke="{p["rim"]}" stroke-width="12"' if p.get("rim") else ""
    if kind == "rrect":
        return f'<rect x="6" y="6" width="500" height="500" rx="108" fill="{p["tile"]}"{stroke}/>\n'
    if kind == "circle":
        return f'<circle cx="256" cy="256" r="250" fill="{p["tile"]}"{stroke}/>\n'
    if kind == "squircle":
        # superellipse n=5, sampled
        pts = []
        for i in range(0, 360, 3):
            t = math.radians(i)
            c, s = math.cos(t), math.sin(t)
            x = 250 * math.copysign(abs(c) ** (2 / 5), c)
            y = 250 * math.copysign(abs(s) ** (2 / 5), s)
            pts.append(f"{CX + x:.1f},{CY + y:.1f}")
        return f'<polygon points="{" ".join(pts)}" fill="{p["tile"]}"{stroke} stroke-linejoin="round"/>\n'
    return ""

def jaw(kind, r_in, r_out, colour):
    w = 64
    x0 = CX - w / 2
    if kind == "rect":
        return f'<rect x="{x0}" y="{CY - r_out}" width="{w}" height="{r_out - r_in}" rx="8" fill="{colour}"/>'
    if kind == "short":
        r_out = r_in + (r_out - r_in) * 0.75
        return f'<rect x="{x0}" y="{CY - r_out}" width="{w}" height="{r_out - r_in}" rx="8" fill="{colour}"/>'
    if kind == "stepped":
        # gripping step: narrow nose at the stock, full width outside
        m = r_in + (r_out - r_in) * 0.4
        n = 40
        return (f'<path d="M{CX - n / 2} {CY - r_in} H{CX + n / 2} V{CY - m} H{CX + w / 2 + 8} V{CY - r_out} '
                f'H{CX - w / 2 - 8} V{CY - m} H{CX - n / 2} Z" fill="{colour}" stroke="{colour}" stroke-width="8" stroke-linejoin="round"/>')
    if kind == "wedge":
        return (f'<path d="M{CX - 20} {CY - r_in} H{CX + 20} L{CX + 40} {CY - r_out} H{CX - 40} Z" fill="{colour}" '
                f'stroke="{colour}" stroke-width="8" stroke-linejoin="round"/>')
    raise ValueError(kind)

def arrow_ring(ring_w, colour, span=285, head=1.0, outer=216):
    r = outer - ring_w / 2
    a_end = 190                     # head at about 8 o'clock, pointing up and clockwise
    a_start = a_end - span
    x0, y0 = pt(r, a_start); x1, y1 = pt(r, a_end)
    large = 1 if span > 180 else 0
    s = (f'<path d="M{x0:.1f} {y0:.1f} A{r} {r} 0 {large} 1 {x1:.1f} {y1:.1f}" fill="none" stroke="{colour}" '
         f'stroke-width="{ring_w}" stroke-linecap="round"/>\n')
    t = math.radians(a_end)
    tx, ty = -math.sin(t), math.cos(t)
    nx, ny = math.cos(t), math.sin(t)
    L = (52 + ring_w * 0.75) * head
    W = (ring_w / 2 + 30) * head
    bx, by = x1 - tx * 4, y1 - ty * 4   # optical: base slightly behind the arc end to hide the cap
    p = [(bx + tx * L, by + ty * L), (bx + nx * W, by + ny * W), (bx - nx * W, by - ny * W)]
    pts = " ".join(f"{x:.1f},{y:.1f}" for x, y in p)
    s += f'<polygon points="{pts}" fill="{colour}" stroke="{colour}" stroke-width="10" stroke-linejoin="round"/>\n'
    return s

def svg(var, pal_id, ink=None, centre=None):
    v, p = VAR[var], dict(PAL[pal_id])
    if centre:
        p.update(jaw=centre[0], stock=centre[1])
    if p.get("mono"):
        p = dict(tile=None, rim=None, body=None, jaw=ink, stock=ink, accent=ink)
        v = dict(v, tile="none", body=False, jaw_gap=max(v.get("jaw_gap", 0), 24))
    s = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">\n<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->\n'
    if v["tile"] != "none":
        s += tile(v["tile"], p)
    if v["body"]:
        s += f'<circle cx="256" cy="256" r="128" fill="{p["body"]}"/>\n'
    stock_r = 64
    r_in = stock_r + v.get("jaw_gap", 0) - (4 if v.get("jaw_gap", 0) == 0 else 0)
    r_out = v.get("jaw_out", 152 if v["body"] else 160)
    for ang in (0, 120, 240):
        s += f'<g transform="rotate({ang} 256 256)">{jaw(v["jaw"], r_in, r_out, p["jaw"])}</g>\n'
    s += f'<circle cx="256" cy="256" r="{stock_r}" fill="{p["stock"]}"/>\n'
    if v.get("bore"):
        s += f'<circle cx="256" cy="256" r="20" fill="{p["body"]}"/>\n'
    s += arrow_ring(v["ring_w"], p["accent"], v.get("span", 285), v.get("head", 1.0), v.get("ring_outer", 216))
    return s + '</svg>\n'

if __name__ == "__main__":
    import json
    out = []
    INK, WHITE = "#1B1F24", "#F2F4F7"
    for c in PAL:
        if PAL[c].get("mono"):
            open(f"A1-{c}-ink.svg", "w").write(svg("A1", c, INK)); open(f"A1-{c}-white.svg", "w").write(svg("A1", c, WHITE))
            out.append(dict(id=f"A1-{c}", group="colour", label=f"{c} {PAL[c]['name']} (on A1)", light=f"A1-{c}-ink", dark=f"A1-{c}-white"))
        else:
            open(f"A1-{c}.svg", "w").write(svg("A1", c))
            out.append(dict(id=f"A1-{c}", group="colour", label=f"{c} {PAL[c]['name']} (on A1)", light=f"A1-{c}", dark=f"A1-{c}"))
    for a in VAR:
        if a == "A1":
            continue
        if VAR[a]["tile"] == "none":
            open(f"{a}-C1-onlight.svg", "w").write(svg(a, "C1", centre=("#5A6A7E", "#22344A")))
            open(f"{a}-C1-ondark.svg", "w").write(svg(a, "C1"))
            out.append(dict(id=f"{a}-C1", group="variation", label=f"{a} {VAR[a]['name']} (C1; centre darkened on light)", light=f"{a}-C1-onlight", dark=f"{a}-C1-ondark"))
            continue
        open(f"{a}-C1.svg", "w").write(svg(a, "C1"))
        out.append(dict(id=f"{a}-C1", group="variation", label=f"{a} {VAR[a]['name']} (C1)", light=f"{a}-C1", dark=f"{a}-C1"))
    open("A6-C3-ink.svg", "w").write(svg("A6", "C3", INK)); open("A6-C3-white.svg", "w").write(svg("A6", "C3", WHITE))
    out.append(dict(id="A6-C3", group="variation", label="A6 small-size glyph, one colour (C3)", light="A6-C3-ink", dark="A6-C3-white"))
    json.dump(out, open("options.json", "w"), indent=1)
