# Issue 637: unreachable-area pattern, review screenshots (throwaway branch, no PR)

1920 x 1080, 100 % scale, Mesa lavapipe on Xvfb, dark theme (the shipped app has only the dark theme; the light theme is P2, UI-20). Built from `claude/viewhatch-work` (6b92f0d3) with `OPENCHUCK_AREA_PATTERN`.

File names: `<case>-<pattern>-<view>.png`, and `compare-<case>-<view>.png` (cropped 2 x 2: top left `dots`, top right `a`, bottom left `b`, bottom right `c`).

Cases: `faceflow` = the 512 repro, face-flow finishing, unreached strip 208.6 mm2 (the strip at the foot of the blade wall, where the pink hatch is the check set "Blade wall"); `rest` = the baluster with a 2 mm ball rest machining after Rough and Finish ("cannot reach 9 areas", markers 3, 7, 8, 9; the large one near the chuck).

Patterns: `dots` = today (12 px screen-space dot grid, decision 0515). `a` = hatching painted on the surface: lines where planes 2.5 mm apart cut the surface (anchored in model space), dark core in a white casing, fading to a flat tint as lines near pixel size. `b` = flat translucent dark tint (40 %) with a 1.5 px white outline. `c` = `a` plus the outline.

Views: `01-oblique` (after one orbit step), `02-near` (zoomed in), `03-near-orbited` (same, orbited), `04-far` (zoomed out).
