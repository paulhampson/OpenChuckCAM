#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""The chosen mark A6-C3 in candidate colours and on tiles, for the surface question (throwaway)."""
import sys
sys.path.insert(0, "../refine")
import gen_refine as g

def mark(colour, name):
    open(f"{name}.svg", "w").write(g.svg("A6", "C3", colour))

def tiled(tile, ink, name, rim=None):
    s = g.svg("A6", "C3", ink)
    t = f'<rect x="6" y="6" width="500" height="500" rx="108" fill="{tile}"' + (f' stroke="{rim}" stroke-width="12"' if rim else "") + "/>\n"
    head, body = s.split("-->\n", 1)
    body = body.replace("</svg>\n", "")
    # the mark at 78% inside the tile, so the arrowhead clears the tile's edge
    s = head + "-->\n" + t + f'<g transform="translate(256 256) scale(0.78) translate(-256 -256)">\n{body}</g>\n</svg>\n'
    open(f"{name}.svg", "w").write(s)

mark("#1B1F24", "mark-ink")
mark("#F2F4F7", "mark-white")
mark("#6E7F96", "mark-slate")
mark("#C46400", "mark-orange")
tiled("#1B1F24", "#F2F4F7", "tile-ink", rim="#3A4048")
tiled("#F2F4F7", "#1B1F24", "tile-white", rim="#C9CED6")
# favicon that follows the browser's colour scheme
s = g.svg("A6", "C3", "currentColor").replace("-->\n", '-->\n<style>svg{color:#1B1F24}@media (prefers-color-scheme: dark){svg{color:#F2F4F7}}</style>\n', 1)
open("favicon-adaptive.svg", "w").write(s)
