# Check for issue #644: on ring and spiral passes, Extend past the stock follows the pass

**What was fixed** (pull request #655, lane RINGSTEP, [decision 0546](../../docs/decisions/0546-run-outs-follow-ring-and-spiral-passes.md); helical finishing): with **Extend past the stock** on, the run-out past the part's end was a straight line along X at the pass's start angle, then a 90-degree turn into the pass, cutting one strip of the overhang. Now:

- **Spiral (Passes Helix):** the helix carries on past the part's end at its own pitch, turning the same way, out to the extended end.
- **Rings:** rings at the operation's own spacing are added past the part's end out to the extended end, joined by the operation's own steps (Sharp or Blended, decision 0545).
- The stock past the part is cleared all round at the pass's own stepover. The run-out is longer, and a note gives the turns added at each end and the path they add. A run-out towards a held end is still refused. Saved operations get the new run-out too (your choice); axial and indexed finishing keep the line along X.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the path's shape and its note. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 78 mm of 24 mm stock from X -13 to 65 (4 mm past the part's far end at X 61), the chuck holding 8 mm, and a 3.175 mm ball. Two helical finishing operations, 1 mm apart, with **Extend past the stock** at the **far end**:

- **Spiral, extend**: Passes Helix;
- **Rings, extend**: Passes Rings.

## Short list (what to click)

1. Open the project, **Operations**, **Generate**.
2. Click each operation and zoom on the far end (X 61 to 67): the spiral keeps turning past X 61; rings are added past X 61.
3. Read the "Past the ends" notes in Results.
4. Optional: **Simulate** and look at the stock past the far end: cleared all round.

## Steps

Use the release candidate, or a build from `main` at commit `1f2c0be6` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`644-run-outs-follow-the-pass.zip`](644-run-outs-follow-the-pass.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/644-run-outs-follow-the-pass/644-run-outs-follow-the-pass.zip)), unzip it anywhere, and open the folder `644-run-outs-follow-the-pass/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Click each operation and zoom on the far end (X 61 to 67). Read its notes in Results.
4. **Simulate** and look at the stock past the part's far end.

## Expected result

- **Spiral, extend**: past X 61 the path keeps turning round the part as a helix out to the extended end; no straight line along X. Note: "Past the ends (operation 'Spiral, extend'): the passes carry on past the part in their own motion (the spiral continued at its pitch) to the extended ends, 0.0 turns at the start end and 5.9 at the far end, 99 mm of path in all."
- **Rings, extend**: rings are added past X 61 out to the extended end, joined like the other rings. Note: "Past the ends (operation 'Rings, extend'): the passes carry on past the part in their own motion (rings at the ring spacing) to the extended ends, 0.0 turns at the start end and 6.0 at the far end, 106 mm of path in all."
- After simulating, no skin of stock is left round the overhang past X 61.

**Old behaviour (not fixed, `main` at `07ec6990` and earlier):** both run-outs were a line along X at one angle, out to X 66.888, with no turn of A, and no note.

## Report back

Reply on issue #644 with: **PASS**, or **FAIL and what you saw** (the operation, its note, and what the far end looks like). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --grouping operation --out OUT`, then [`gcode_past_x.py`](../gcode_past_x.py)` 61 OUT/*.nc`), Linux x86_64, release builds each in a target folder of its own, 9 November 2026:

| | `main` at `07ec6990` (before) | `main` at `1f2c0be6` (after) |
| --- | --- | --- |
| Spiral, extend | 6 fed moves past X 61 (to X 66.888), A turned 0.0°; 9,664 moves, 935.8 s | 424 fed moves past X 61, A turned 2119.5°; 10,082 moves, 969.2 s |
| Rings, extend | 6 fed moves past X 61, A turned 0.0°; 10,337 moves, 960.3 s | 14 fed moves past X 61, A turned 2160.0°; 10,345 moves, 996.3 s |

The programmed time grows by 33.4 s and 36.0 s (the replay's figures, not a machine measurement), as the record says the longer run-out costs. The view and the simulation cannot be run without a display here.
