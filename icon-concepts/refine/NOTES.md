# Concept A refinement: colour options and variations (lane ICON, throwaway)

Sheet: `contact-sheet-refine.png` (every option at 512 shown half, 128, 32, 16, and 32 and 16 magnified, on light and dark).
Sources: `gen_refine.py` writes the SVGs, `node render_refine.js` renders `png/` and the sheet (Chromium through Playwright).

## Grid and optical corrections

- 512 x 512 canvas on a 32-unit grid: 16 x 16 cells, so one cell is one pixel at 16 px and two at 32 px. Centre (256, 256).
- Keylines: tile 6..506 (rim 12), arrow ring outer radius 216 (A6: 224 = 7 px at 16 px), chuck body 128, stock 64 (2 px at 16 px).
- Radii and widths are multiples of 16 (half a 16-px pixel); A6 uses multiples of 32 so the ring (64 = 2 px), jaws (64 x 64 = 2 x 2 px) and the 1-px gaps between ring and jaws land on whole pixels.
- Optical corrections: the arrowhead base sits 4 units behind the arc end so the round cap does not show; the head is rounded with a stroke so it does not look sharper than the ring; jaws overlap the stock by 4 units so there is no hairline seam; the rim on dark tiles is lighter than the tile so the tile edge holds on a dark desktop.
- 16 px checked pixel by pixel in the x6 views: A1 and A5 lose the jaws into a grey blob; A6 keeps a 2-px orange ring, the arrowhead as a 3 x 3 px notch on the left, and a light centre with three bumps.

## Options

Colours (shown on A1): C1 slate and orange; C2 light tile; C3 one colour (ink on light, white on dark, no tile); C4 bold blue and amber; C5 graphite and signal orange.
Variations (shown in C1): A1 base; A2 heavy ring, stepped jaws, no body disc, squircle; A3 circle tile, thin ring, tapered jaws; A4 mark without tile, jaws spaced from the stock; A5 wide gap, big arrowhead, short jaws, centre bore; A6 small-size glyph on the 16-px grid (ring 2 px, no body disc, short wide jaws, 1-px gaps), also in C3.

## Scores against the logo tests

Y = passes, ~ = partly, N = fails.

| Option | Simple | Distinctive | 16/32 px | One colour, light and dark | Scalable | Timeless | Appropriate | Balanced | Distinct from egui and CAM marks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A1 base | ~ four layers | Y | ~ jaws blur at 16 | ~ needs C3 redraw | Y | Y | Y | Y | Y |
| A2 heavy ring, stepped jaws | ~ | ~ stepped jaws read as a fan | ~ | ~ | Y | Y | Y | ~ heavy ring crowds the tile | Y |
| A3 circle tile, tapered jaws | ~ | N tapered jaws read as a fan or radiation sign | ~ | ~ | Y | Y | ~ | Y | ~ fan / trefoil |
| A4 no tile | Y | Y | ~ thin at 16 without a tile | Y (it is the C3 shape) | Y | Y | Y | Y | Y |
| A5 bore, wide gap | ~ five shapes | Y the bore says "workpiece" | ~ bore and jaws vanish at 16 | ~ | ~ bore is a small detail | Y | Y | Y | Y |
| A6 small-size glyph | Y three shapes: ring, jaws, stock | Y | Y best of all at 16 and 32 | Y (A6-C3) | Y | Y | Y | Y | Y |
| C1 slate and orange | | | Y high contrast | Y rim holds on dark | | Y | Y precise, workshop | | Y |
| C2 light tile | | | ~ low contrast on light desktops | ~ | | Y | Y | | Y |
| C4 bold blue and amber | | Y most eye-catching | Y | Y | | ~ blue tiles are common | Y | | ~ many apps use a blue tile |
| C5 graphite and signal orange | | Y | Y | ~ tile nearly vanishes on dark | | Y | Y | | Y |

Risks for every option: a ring with an arrowhead is also the common "refresh" or "rotate" glyph; the jaws and the stock in the centre are what make it OpenChuckCAM, so the centre must stay readable (A6 keeps it). The egui logo is a white hexagon with a four-part cross on a black tile; three jaws in a circle with an orange arrow do not resemble it, and four jaws were deliberately not offered because they would.
Licence: original work, AGPL-3.0-or-later.
