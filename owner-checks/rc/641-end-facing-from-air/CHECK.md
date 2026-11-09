# Check for issue #641: End facing enters each band from air and checks its engagement

> [!NOTE]
> **Draft, to run on the release candidate. The code is not merged.** [Decision 0547](https://github.com/paulhampson/OpenChuckCAM/pull/649/files) (lane PATHFEED, accepted by the owner on 8 Nov 2026, amended by his answer A to question 4) is in docs pull request #649; the code is on lane PATHFEED's branch `claude/lane-pathfeed-641-wip`. This file gives `main` at `81bade8a3`, what that branch did on 9 Nov 2026, and what the record says. When the code merges, the expected texts are copied from the merged build, its pull request is named here, and this note is removed.

**What is to change** (Facing, **Shape** Unheld end):

1. **Each band starts and ends in air beside the stock** where the stock allows: the cut across the face is lengthened at each end, the tool comes down there, the whole tool the approach distance clear of the stock, and cuts across only. Today each band comes down past the end of the stock, then feeds sideways along X into the end face at full band depth and turns 90 degrees in the material.
2. **Engagement is checked** against the operation's own settings: the slice along X at most **Stepover**, the band along the tool at most **Stepdown**; more is refused, naming the setting.
3. **Stepover and Stepdown swap meaning** to match the tool's cutting data: Stepover is the slice thickness along X, Stepdown the band along the tool. Saved End operations are migrated so their paths do not change.
4. **A whole number of slices posts**: today a depth that divides exactly into full slices can be refused at posting.
5. **Stock above the area** (question 4, option A): with Coverage extent Part only on stock that rises above the area, bands are added from the stock's top down to the area's top, each no deeper than Stepdown, with an information note.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the path's shape and the messages. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project with 3 mm of stock past the part's far end (65 mm of stock from X -1), and two Facing operations first: **Face end**: Shape **Unheld end** (the far end), the tutorial's 3.175 mm flat end mill, Stepover and Stepdown from the tool's cutting data; and **Face end, part only**: the same with **Coverage extent** Part only. The project is saved at schema 25, so the program migrates the two End operations' Stepover and Stepdown when it opens it.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`641-end-facing-from-air.zip`](641-end-facing-from-air.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/641-end-facing-from-air/641-end-facing-from-air.zip)), unzip it anywhere, and open the folder `641-end-facing-from-air/project` with **File > Open project**.
2. Click **Operations**, **1 Face end**. Read the Inspector's **Stepover** and **Stepdown** and their labels.
3. Click **Generate**, then look at the path at the far end: where each band comes down and how it enters the face.
4. **File > Post and export...** (Makera Z1 or LinuxCNC).
5. On **Face end**, set **Stepdown** to a value larger than the tool's flute length (for example `30`), and generate: it is refused, naming Stepdown. Set it back.
6. Click **2 Face end, part only** and read its notes; look at where its bands start.

## Expected result

- **Step 2**: Stepover is the slice thickness along X, Stepdown the band along the tool (labels as in the merged build).
- **Step 3**: each band comes down in air beside the stock and only cuts across; no fed move runs along X into the end face.
- **Step 4**: the program is written (no refusal at posting).
- **Step 5**: refused, naming Stepdown. On the branch: "operation 'Face end' cannot be generated: End facing: the Stepdown (band along the tool) must be above zero and no greater than the flute length".
- **Step 6**: a note "Stock above the area (operation 'Face end, part only'): the stock rises above the requested area, so 2 passes ... are added, each no deeper than the Stepdown (band along the tool); they take about 3 s more at the cutting feed". Where the bands cannot start in air (inside the stock's width), the entry from the side is fed at the plunge feed.

**Old behaviour (today, `main` at `81bade8a3`):**

- With the tool's cutting data (Stepover 2.2225, Stepdown 1 mm for Hardwood), the program is refused at posting: `error: cannot post the G-code: Facing written-motion verification: Facing final validation: an actual End cut exceeds the selected maximum axial stepdown`.
- With Stepover 1 and Stepdown 0.7 set on the operation (so it posts), each band reads, in the LinuxCNC program: `G0 X66.588 Y-12.000 ...`, `G1 Z11.001 F300.0` (down past the end of the stock), `X64.987 F1000.6` (sideways along X into the end face, at the cutting feed), `Y12.000 F1000.0` (across), `X66.588 F1000.6` (back out along X).

## Report back

Reply on issue #641 with: **PASS**, or **FAIL and what you saw** (where a band enters, the labels, and any message). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool on `main` at `81bade8a3` (Linux x86_64, release build, 8 November 2026): `openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT` refuses with the error above (decision 0547 item 4 names the same refusal); with `stepdown = 0.7` and `stepover = 1.0` added to the operation it posts, with the band moves quoted. When the fix merges, the same runs will be repeated on the merged build. The interface cannot be run without a display here.

**On lane PATHFEED's branch** (`claude/lane-pathfeed-641-wip` at `67a6ff1d0`, not merged; run 9 November 2026, LinuxCNC post):

- **Face end** posts with the tool's cutting data (item 4). Each band is now `G0 X66.588 Y-14.588`, `G1 Z11.001 F300.0` (down past the end of the stock, beside it), `X64.088 F1000.0` (along X **in air beside the stock**: Y -14.588 is the stock's radius 12 plus the tool's radius and the approach distance), `Y14.588` (across, from air to air), `X66.588`. Today's band runs across from Y -12.000 to 12.000.
- A schema-25 End operation keeps its path after the swap of Stepover and Stepdown (with `stepdown = 0.7`, `stepover = 1.0` saved at schema 25 the band moves are those of `main`).
- Stepdown 30 on a schema-28 operation: refused naming Stepdown (quoted above). Stepover too large: "End facing: the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter".
- **Face end, part only**: the "Stock above the area" note quoted above. Its bands start at `G0 X66.588 Y-11.000`, `G1 Z11.001 F300.0`, then `X64.088 F1000.0`: the side entry runs along X at Y -11.000, where the tool overlaps the stock, at the **cutting feed** (1000 mm/min), not the plunge feed (300 mm/min) the record gives for this case (its test `req_op_24_req_op_11_part_extent_band_keeps_a_plunge_fed_side_entry`). Reported to the coordinator; to be confirmed on the merged build.
- The note "2 passes are from the stock's top down to the area are added" has a word too many; reported.
