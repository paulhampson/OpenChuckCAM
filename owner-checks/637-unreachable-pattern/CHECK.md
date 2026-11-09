# Check for issue #637: unreachable areas are hatched on the surface, not marked with screen-fixed dots

**What was fixed** (pull request #652, lane VIEWHATCH, [decision 0549](../../docs/decisions/0549-unreached-area-hatch-and-footprint.md), the owner's choice of pattern A): the view marked the areas a strategy could not reach with dots that stayed still on the screen while the part turned, which was uncomfortable to look at. Now:

- the area is **hatched**: dark diagonal lines (`#0c0e12`) in a white casing, 1 mm apart, **painted on the surface**, so they move, turn and foreshorten with the part;
- far away, or at a steep angle, the lines **fade into a flat dark tint** instead of shimmering;
- the part's grey and any deviation colours show between the lines; the legend row "Unreachable (operation name)" shows a swatch of two hatch lines;
- the area's **footprint is closed**: the holes between the marked triangles of a narrow strip are filled (within 1.5 mm of the area's samples, on the same surface), so the strip reads as one area along its length, not as ticks across it.

**Time:** about 3 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the view only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is the project of the #512 check, which you attached to the issue, written again by [`generate.py`](generate.py) with that check's generator: the user guide's blisk starting project with face-flow finishing (**Floor finish**) on the hub floor between two blades. It leaves one strip it cannot reach next to the far blade, about 2 mm wide and 40 mm long.

## Steps

Use the release candidate, or a build from `main` at commit `01aa5e32` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`637-unreachable-pattern.zip`](637-unreachable-pattern.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/637-unreachable-pattern/637-unreachable-pattern.zip)), unzip it anywhere, and open the folder `637-unreachable-pattern/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Click **1 Floor finish**. If the area is not shown, turn on **Unreachable areas** in the view's **Show** menu.
3. In **Messages**, follow the link on "Unreached areas (operation 'Floor finish'): ..." so the view turns to the area.
4. Look at the strip close up, then turn the part with the mouse and zoom out a long way, and back in.
5. Look at the legend row "Unreachable (Floor finish)".
6. Optional: **Simulate**, turn on the deviation colours, and look at the strip again.

## Expected result

- **Step 4, close up:** dark diagonal lines with a white edge, about 1 mm apart, across the whole strip; the strip reads as one area along its length, with no isolated ticks or holes in it.
- **Step 4, turning:** the lines stay on the surface: they move and foreshorten with the part. Nothing stays still on the screen while the part moves. No dots or small circles anywhere.
- **Step 4, far away:** the lines fade into a flat dark tint; they do not shimmer or form moiré while the view moves.
- **Step 5:** the swatch shows two hatch lines.
- **Step 6:** the deviation colours show between the lines.
- The Messages line reads: "Unreached areas (operation 'Floor finish'): face-flow finishing cannot reach 1 area of the selected faces, which stay uncut: X 20.000 to 60.000 near A 282 (208.6 mm2), a neighbouring face is in the way of the tool; check them in the simulation before running the program".

**Old behaviour (not fixed, the issue's screenshot):** dots that stayed still on the screen while the part turned, showing as ticks across the narrow strip.

## Report back

Reply on issue #637 with: **PASS**, or **FAIL and what you saw** (how the pattern looks close up and far away, and whether it moves with the part). A screenshot of the public blisk is fine. Numbers and messages only otherwise; no project files from your own work.

## What we saw when preparing this

The view cannot be drawn without a display here, so the hatch itself was not seen in this lane. On `main` at `01aa5e32` (Linux x86_64, release build in a target folder of its own, 9 November 2026), `openchuck-cli regenerate project` generates **Floor finish** with the Messages line quoted above, unchanged from `81bade8a3`; the project is byte-identical to the #512 check's (`diff -r`). The lane's own record ([decision 0549](../../docs/decisions/0549-unreached-area-hatch-and-footprint.md), Verification) gives the rendered tests (the lines spaced in millimetres at any zoom, staying on the surface when the camera moves, fading far away) and, on this same project, the area growing from 336 to 411 triangles with its boundary against unmarked triangles falling from 316 to 221 edges. That record also says the light theme and Windows or macOS displays were not verified by the lane: on Windows you are the first to see it.
