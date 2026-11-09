# Check for issue #641: End facing enters each band from air and checks its engagement

**What was fixed** (pull request #656, lane PATHFEED, [decision 0547](../../docs/decisions/0547-end-facing-enters-from-air-and-checks-its-engagement.md) with the owner's answer A to its question 4; Facing, **Shape** Unheld end):

1. **Each band starts and ends in air beside the stock** where the stock allows: the cut across the face is lengthened at each end so the tool comes down, and leaves, with the whole tool the approach distance clear of the stock. Before, each band fed sideways along X into the end face at full band depth and turned 90 degrees in the material.
2. **Where it cannot start in air** (Coverage extent Part only, inside the stock's width), the side entry is fed at the **plunge feed**.
3. **Engagement is checked** against the operation's own settings: the slice along X at most **Stepover**, the band along the tool at most **Stepdown**; more is refused, naming the setting.
4. **Stepover and Stepdown swap meaning** to match the tool's cutting data: Stepover is the slice thickness along X, Stepdown the band along the tool. Saved End operations (project schema 27 or older) are migrated, so they keep their slices and bands.
5. **A whole number of slices posts**: before, a depth that divided exactly into full slices could be refused at posting.
6. **Stock above the area** (question 4): with Coverage extent Part only on stock that rises above the area, passes are added from the stock's top down to the area, each no deeper than Stepdown, with a note.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the path's shape and the messages. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project with 3 mm of stock past the part's far end (65 mm of stock from X -1), and two Facing operations first, both **Shape** Unheld end (the far end) with the tutorial's 3.175 mm flat end mill and Stepover and Stepdown from the tool's cutting data:

- **Face end**: Coverage extent Whole stock (the default);
- **Face end, part only**: Coverage extent Part only.

The project is saved at schema 25, so the program migrates the End operations when it opens it.

## Short list (what to click)

1. Open the project. **Operations**, **1 Face end**: read **Stepover** and **Stepdown** and their labels.
2. **Generate**. Look at Face end's path at the far end, then Face end, part only's. Read the notes in **Results**.
3. **File > Post and export...**: it writes.
4. Set Face end's **Stepdown** to `30` and generate: refused, naming Stepdown. Set it back.

## Steps

Use the release candidate, or a build from `main` at commit `db6bc84b` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`641-end-facing-from-air.zip`](641-end-facing-from-air.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/641-end-facing-from-air/641-end-facing-from-air.zip)), unzip it anywhere, and open the folder `641-end-facing-from-air/project` with **File > Open project**.
2. Click **Operations**, **1 Face end**. Read the Inspector's **Stepover** and **Stepdown** and their labels.
3. Click **Generate**, then look at **Face end**'s path at the far end (X 62 to 67): where each band comes down and how it enters the face.
4. Click **2 Face end, part only**: look at its bands and read its notes in **Results**.
5. **File > Post and export...** (Makera Z1 or LinuxCNC): the files are written.
6. On **Face end**, set **Stepdown** to `30` (more than the tool's flute length) and generate: it is refused, naming Stepdown. Set it back.

## Expected result

- **Step 2:** Stepover is the slice thickness along X, Stepdown the band along the tool.
- **Step 3:** each band comes down in air beside the stock, past its end, and only cuts across the face from air to air; nothing feeds sideways into the end face. In the LinuxCNC program a band reads `G0 X66.588 Y-14.588`, `G1 Z11.001 F300.0`, `X64.088 F1000.0` (in air beside the stock), `Y14.588` (across), `X66.588`.
- **Step 4:** the note "Stock above the area (operation 'Face end, part only'): the stock rises above the requested area, so 2 passes from the stock's top down to the area are added, each no deeper than the Stepdown (band along the tool); they take about 3 s more at the cutting feed." The side entry inside the stock's width is at the plunge feed: `G0 X66.588 Y-11.000`, `G1 Z11.001 F300.0`, `X64.088` (still at F300), `Y11.000 F1000.0`, `X66.588 F300.0`.
- **Step 5:** written, not refused.
- **Step 6:** "End facing: the Stepdown (band along the tool) must be above zero and no greater than the flute length".

**Old behaviour (not fixed, `main` at `81bade8a3`):** with the tool's cutting data the program was refused at posting: "Facing written-motion verification: Facing final validation: an actual End cut exceeds the selected maximum axial stepdown". With smaller values each band fed sideways into the end face at the cutting feed (`G0 X66.588 Y-12.000`, `G1 Z11.001 F300.0`, `X64.987 F1000.6`, `Y12.000`, `X66.588`). There was no Part-only note and no engagement refusal.

## Report back

Reply on issue #641 with: **PASS**, or **FAIL and what you saw** (where a band enters, the feed of the side entry, the labels and any message). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`), Linux x86_64, release builds each in a target folder of its own:

| | `main` at `81bade8a3` (before) | `main` at `db6bc84b` (after) |
| --- | --- | --- |
| Face end, tool's cutting data | refused at posting (the error above) | posts: 350 moves, 117.0 s |
| Face end, band entry | sideways along X into the face, `X64.987 F1000.6` from `Y-12.000` | down beside the stock at `Y-14.588`, across from air to air |
| Face end, part only | (no such case) | 336 moves, 135.4 s; the note above; side entry at F300 |
| Stepdown 30 (schema 28) | (no such check) | refused, naming Stepdown |
| Face end saved at schema 25 with Stepdown 0.7 and Stepover 1 | 25 bands at Z 11.001 to -12.001, 5 slices at X 64.987 to 62.587 | the same 25 bands and 5 slices; the bands now start at `Y-14.588`, in air |

Lane PATHFEED's branch at `67a6ff1d0` (9 Nov 2026) fed the part-only side entry at the cutting feed and had a word too many in the note ("2 passes are from the stock's top down to the area are added"); both are fixed on `db6bc84b`. The Facing line still has no topic in front ("operation 'Face end': Facing covered the end faces it was asked to."): your wording, parked for your decision. The interface (the view, the labels) cannot be run without a display here.
