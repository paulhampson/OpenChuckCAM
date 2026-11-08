# Check for issue #555: axial finishing runs past the selected faces and down onto the tab

**What was fixed** (pull request #562, [decision 0527](../../../docs/decisions/0527-a-selection-stops-at-its-edge-with-an-unselected-face.md)): a contact on the edge between a selected face and an unselected one no longer counts as on the selection, so the tool no longer rolls over the edge of the selection. The pass ends where the tool still sits on the selected faces. A tab is not a face of the model, so no selection holds it; rolling over the edge was the only way in.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Do not run the G-code of this project on a machine, not even as an air cut: it is made only to show where the path goes. Generated G-code must be verified by you before it is run on a machine.

## The project

[`project/`](project/), written by [`generate.py`](generate.py) from public material only: the user guide's baluster (X 0 to 61) in 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), and **tab 1**, a cylinder **11 mm** across from X -5 to 0. The baluster's tenon (X 0 to 10) is 14 mm across, so its end face at X 0 is a 1.5 mm rim between the tenon's side and the tab.

One operation, **1 Tenon side**: axial finishing with a 60 degree V-bit, 4 mm across, limited to the selection set **Tenon side** (the 96 triangles of the tenon's side, X 0 to 10; the end face at X 0 and the tab are not in it). This is the shape of your carving's case on a public part: a V-bit, a selection beside an unselected face, and a tab beyond it.

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Copy the folder `project` to a folder of your own and open the copy with **File > Open project**.
2. Click **Operations** at the top, then **Generate**. It takes a few seconds.
3. Click **Front** above the view, then zoom in with the mouse wheel on the left end of the part, where the tenon meets the tab (X 0).
4. Look at where the orange passes end at that end.
5. Optional: click **Simulate** and look at the rim and the tab.

## Expected result

- Every pass ends **on the tenon's side, at X 0**. No pass goes over the rim, down the end face or along the tab.
- The **Messages** list has no "Tab length" line for this operation (the tool, 4 mm, is narrower than the tab is long).

**Old behaviour (not fixed):** passes ran over the rim at X 0, down the end face and along the top of the tab, up to about 0.9 mm past X 0, with the tip down at the tab's radius (5.5 mm from the axis).

## Report back

Reply on issue #555 with: **PASS**, or **FAIL and what you saw** (where the passes end, as X and the height you read in the view). It is still worth checking your own carving the same way (numbers only, nothing from the project).

## What we saw when preparing this

The project was generated and posted with the LinuxCNC post by the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --out OUT`), Linux x86_64, 8 November 2026, and the fed moves (G1) of the program were read with [`../gcode_extent.py`](../gcode_extent.py) (`--below-x 0`). In these programs Z is the tip's height above the rotary axis.

| Build | Fed moves | Fed X from | Fed moves ending below X 0 | Lowest of those |
| --- | --- | --- | --- | --- |
| Before the fix: `c5aec1dff`, the parent of the fix's merge commit `3c3b3f57` | 2,202 | -0.916 | 616 | Z 5.500 at X -0.916 (the tab's surface, radius 5.5) |
| `main` at `ae608e28f` (0.50.0) | 1,812 | 0.000 | 0 | none |

Fed moves ending below X 0.5 on `main`: 301, the lowest at Z 6.985, on the tenon's side (radius 7). Neither build printed a tab warning for this operation.
