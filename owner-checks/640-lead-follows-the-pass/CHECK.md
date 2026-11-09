# Check for issue #640: a lead on a ring stays in the plane of the cut

**What was fixed** (pull request #665, lane RAMPLEAD, [decision 0544](../../docs/decisions/0544-a-lead-on-a-pass-that-turns-a-stays-in-the-plane-of-the-cut.md)): on a pass that turns A (rings, the spiral, partial rings), Entry **Lead** used to come in as a quarter arc in the X-Y plane, along the part's axis, and was refused where that reached a held region. Now:

- **The lead follows the pass:** the lead-in comes down onto the cut, and the lead-out rises from it, in the plane of the cut at the pass's X, with A and Z moving together and A turning the pass's way. Leads on passes along X (line passes) are unchanged.
- **A pass that starts on the rotary axis** (an end face's ring) has no plane for a lead arc: it is entered by a straight plunge, has no lead-out, and a note names those passes.
- **A tool that cannot cut straight down** (not centre cutting) is now refused where a lead's descent to its arc goes down through stock, as a plunge into stock already was. Where the descent is through air, it generates as before.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at where the lead moves go and at the messages. Do not run this project's G-code on a machine, not even as an air cut. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's tutorial project (the public baluster in 63 mm of 24 mm stock from X -1) **without its 8 mm grip**, with the public `cylinder.step` (radius 10, 50 mm long, lying along X from 0 to 50) added as a second model, and three operations, all with Entry **Lead**:

1. **Finish rings, lead**: helical finishing in Rings, 3.175 mm ball, on **Setup 1, no grip**.
2. **Axial lead, through stock**: axial finishing of the cylinder with **O-flute, not centre cutting** (the tutorial's 3.175 mm flat end mill, marked as not centre cutting), every pass the same way, Extend past the stock at the far end, on **Setup 2, cylinder in long stock** (24 mm stock from X -10, so stock stands beyond the cylinder's start). **Expected to be refused.**
3. **Axial lead, through air**: the same, on **Setup 3, cylinder in flush stock** (the stock starts at the cylinder's end, X 0).

**With the grip** (the tutorial as it is), the rings pass starts with a short cut along X before its first ring, so it keeps the line lead and is still refused at the held region. Pull request #664 (rings start on a whole ring at the held region's edge) changes that; it is not merged, so this check does not expect the grip case to generate.

## Short list (what to click)

1. Open the project, **Operations**, **Generate**: 1 and 3 generate; 2 is refused, naming the passes and the tool.
2. Click **1 Finish rings, lead** and look at its first lead-in at X 0: it comes down turning with the part, at the ring's X; nothing comes in along X.
3. Read the **Lead entry** note on 1 (passes 14 and 27 are plunged).
4. Optional: post 1 with the LinuxCNC post and read the first `G1` moves: Z falls while A turns, at X 0, no Y.

## Steps

Use the release candidate, or a build from `main` at commit `3fac35f7` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`640-lead-follows-the-pass.zip`](640-lead-follows-the-pass.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/640-lead-follows-the-pass/640-lead-follows-the-pass.zip)), unzip it anywhere, and open the folder `640-lead-follows-the-pass/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Read each operation's status and its Messages and Results lines.
3. Click **1 Finish rings, lead** and zoom on the head end (X 0): look at where the first lead-in comes down and where the last lead-out leaves.
4. Read the message of **2 Axial lead, through stock**, and look at **3 Axial lead, through air**: its leads come in from beyond the cylinder's end, over no stock.
5. Optional: **File > Post and export...** with the LinuxCNC post, one file per operation, and in the **Preview** read the first moves of **Finish rings, lead**.

## Expected result

As the command-line tool prints them on `main` at `3fac35f7`:

- **1 Finish rings, lead** generates (22,278 moves). Its program starts `G0 X0.000 Y0.000 Z40.000 A0.000`, `A-12.994`, `Z13.000`, `G1 Z8.587 F200.0`, `Z8.380 A-12.883 F497.9`, `Z8.177 A-12.551`, `Z7.980 A-12.005`, ...: the tool comes down at X 0 and A and Z move together into the ring, with no move in Y. Note: "Lead entry (operation 'Finish rings, lead'): passes 14, 27 start on the rotary axis, where a lead arc has no plane to follow, so each is entered by a straight plunge and has no lead-out." Without a grip it also gives two **Part cut free** warnings at the end faces, which need acknowledging before export; they are expected here and are not part of the check.
- **2 Axial lead, through stock** is refused: "Lead entry (operation 'Axial lead, through stock'): 88 passes, the first pass 1, are entered by a straight plunge into the stock (a single point, a ring on the rotary axis, where the tool descends while the part turns, a face nub, or a lead whose descent to its arc goes down through the stock), and the tool 'O-flute, not centre cutting' cannot cut straight down (it is not centre cutting). Set Entry to Helix, or choose a tool that can plunge".
- **3 Axial lead, through air** generates (7,216 moves); its first lead starts `X-2.359 Y1.587`, `Z11.000`, `G1 Z10.000 F300.0`, then the quarter arc to the cylinder's end: the line lead as before, its descent 1 mm through air.

**Old behaviour (`main` at `fa1c2de1`, before the fix):** **1** generated with the old lead: `X-1.588 Y1.587`, `Z12.732`, `G1 Z7.000 F200.0`, then a quarter arc in the X-Y plane coming in along X to X 0, and no Lead entry note. **2** generated: the tool went down from `Z13.000` to `Z10.000` at the plunge feed through 2 mm of the stock beyond the cylinder's end, with a tool that cannot cut straight down. **3** was the same program as today.

## Report back

Reply on issue #640 with: **PASS**, or **FAIL and what you saw** (the operation, where its lead goes, and the message). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`, each operation on its own), Linux x86_64, release builds each in a target folder of its own, 9 November 2026, on `main` at `3fac35f7` (`--version` `3fac35f7`, no uncommitted changes) and at `fa1c2de1` for the old behaviour:

| | `main` at `fa1c2de1` (before) | `main` at `3fac35f7` (after) |
| --- | --- | --- |
| 1 Finish rings, lead | 21,671 moves, quarter arc in X-Y from `X-1.588 Y1.587` at Z 7 | 22,278 moves, A and Z together at X 0 from `A-12.994`; Lead entry note, passes 14 and 27 |
| 2 Axial lead, through stock | 7,216 moves, descent from Z 13 to Z 10 through the stock | refused, 88 passes from pass 1 |
| 3 Axial lead, through air | 7,216 moves | the same program, byte for byte |

- **The grip case:** with the 8 mm grip put back, **1** on `3fac35f7` is still refused: "operation 'Finish rings, lead' cannot be generated: pass 1: the lead-in needs an arc of radius 1.587 mm, and it would run over the held region at the start of the stock at X 7.604 mm, where the workholding grips the stock; use a plunge entry, or a size or angle that keeps it clear of the held region". That waits for #664.
- **Lane RAMPLEAD's own run** of operation 1 before its rebase began `A-12.444`, `G1 Z8.586`; the merged pull request also changed the arc's shape on a cut that is not level (it never turns A back), which gives `A-12.994`, `G1 Z8.587` on `main`.
- A non-centre-cutting tool with Lead on the baluster's rings was refused before the fix as well (passes on the rotary axis, plunged), so the cylinder is used for the new refusal: it shows the lead's descent alone.

The interface (the view and the preview) cannot be run without a display here.
