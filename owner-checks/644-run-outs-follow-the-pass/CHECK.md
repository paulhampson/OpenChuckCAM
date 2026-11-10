# Check for issue #644: on ring and spiral passes, Extend past the stock follows the pass

> [!NOTE]
> **Re-issued on 10 November 2026** after your FAIL on rc1 ("leaves essentially a tab of material at the tip of the finial"). Lane RUNOUT-TIP found that this is how the run-out is designed (decision 0212 part B item 3, decision 0546 item 3): the run-out keeps the **height of the pass's last point**; it does not follow the part down to a point. The first issue of this check promised more than the design gives ("cleared all round"). This issue says what the design gives. The tip is **not** cut to a point in 0.55.0: that is ticket #779 (a run-out that follows the drop to a true point). The project now also has Face free ends on (the default).

**What was fixed** (pull request #655, lane RINGSTEP, [decision 0546](../../docs/decisions/0546-run-outs-follow-ring-and-spiral-passes.md); helical finishing): with **Extend past the stock** on, the run-out past the part's end was a straight line along X at the pass's start angle, then a 90-degree turn into the pass. Now:

- **Spiral (Passes Helix):** the helix carries on past the part's end at its own pitch, turning the same way, out to the extended end.
- **Rings:** rings at the operation's own spacing are added past the part's end out to the extended end, joined by the operation's own steps (Sharp or Blended, decision 0545).
- The run-out stays at the **height of the pass's last point** on the part (here the finial's end at X 61, a radius of about 2.67 mm). A note gives the turns added at each end and the path they add. A run-out towards a held end is still refused. Saved operations get the new run-out too; axial and indexed finishing keep the line along X.

**What #655 is checked for:** only that the run-out **follows the pass** past X 61 (the spiral keeps turning, or rings are added), not a line along X. What is left at the finial's tip depends on Face free ends, as described under "Expected result"; it is the design, not a fault of #655.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the path's shape, its notes and what the simulation leaves. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 78 mm of 24 mm stock from X -13 to 65 (4 mm past the part's far end at X 61), the chuck holding 8 mm, and a 3.175 mm ball. Two setups on that same stock, each with two helical finishing operations, 1 mm apart, **Extend past the stock** at the **far end**:

| Setup | Operations | Face free ends |
| --- | --- | --- |
| **Setup 1, Face free ends on** | **Spiral, extend** (Passes Helix); **Rings, extend** (Passes Rings) | on (the default) |
| **Setup 2, Face free ends off** | **Spiral, extend, ends off**; **Rings, extend, ends off** | off, as the first issue of this check had it |

## Short list (what to click)

1. Open the project, **Operations**, **Generate**.
2. Click each operation and zoom on the far end (X 61 to 67): the spiral keeps turning past X 61; rings are added past X 61. No straight line along X. **This is the pass criterion.**
3. Read the "Past the ends" notes in Results.
4. Optional: **Simulate** each setup and look at the finial's tip: with Face free ends off a core about 5.3 mm across stays; with it on, a small blunt nub stays at the tip (expected; see below).

## Steps

Use the release candidate (`main` at `ea1f3f96`), or a build from `main` at commit `1f2c0be6` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`644-run-outs-follow-the-pass.zip`](644-run-outs-follow-the-pass.zip) (or take it from the raw link: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/644-run-outs-follow-the-pass/644-run-outs-follow-the-pass.zip), unzip it anywhere, and open the folder `644-run-outs-follow-the-pass/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Click each operation and zoom on the far end (X 61 to 67). Read its notes in Results.
4. Optional: **Simulate**, and look at the stock past the part's far end in each setup.

## Expected result

As the command-line tool prints them on `main` at `ea1f3f96`, each operation generated on its own:

- **The run-out follows the pass (all four operations):**
  - **Spiral**: past X 61 the path keeps turning round the part as a helix out to X 66.888; no straight line along X. Note: "Past the ends (operation 'Spiral, extend'): the passes carry on past the part in their own motion (the spiral continued at its pitch) to the extended ends, 0.0 turns at the start end and 5.9 at the far end, 99 mm of path in all."
  - **Rings**: rings are added past X 61 out to X 66.888. Note: "Past the ends (operation 'Rings, extend'): the passes carry on past the part in their own motion (rings at the ring spacing) to the extended ends, 0.0 turns at the start end and 6.0 at the far end, 106 mm of path in all."
  - The run-out turns at the **height of the pass's end point** (a radius of about 2.67 mm), not lower.
- **Setup 2, Face free ends off** (what you saw on rc1): a **core about 5.3 mm across** (radius about 2.67 mm) stays from about X 60.2 to the end of the stock at X 65. This is expected with this design; it is not a failure of #655.
- **Setup 1, Face free ends on**: the end face at X 61 is also faced down to the rotary axis (13 layers of 1 mm, in a ring 3.175 mm wide from X 61.001 to 64.176), so the stock past it is cut free. That is said by **Part cut free** warnings, to acknowledge before export: "Part cut free (operation 'Spiral, extend'): the end face at X 61.000 is faced down to the rotary axis: the tool cuts a ring 3.175 mm wide from X 61.001 to 64.176 down to the axis, and the stock past the ring is not held by anything in this setup, so it can be cut free there and thrown while the rotary axis turns. ..." (and the same at X 0). A **small blunt nub** stays at the finial's point: lane RUNOUT-TIP measured a radius of 2.68 mm at X 61 down to 0.04 mm at X 62.25.
- **The finial's tip is not cut to a point in 0.55.0.** A run-out that follows the drop to a true point (with an adaptive stepover for curves) is ticket #779.

**Old behaviour (not fixed, `main` at `07ec6990` and earlier):** both run-outs were a line along X at one angle, out to X 66.888, with no turn of A, and no note.

## Report back

Reply on issue #644 with: **PASS** if the run-out follows the pass past X 61 (turns or rings, not a line along X), or **FAIL and what you saw** (the operation, its note, and what the far end looks like). What stays at the tip is described above and is not the pass criterion. Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT --acknowledge-collision-warnings`, each operation on its own, then [`gcode_past_x.py`](../gcode_past_x.py)` 61 OUT/*.ngc`), Linux x86_64, a release build of `main` at `ea1f3f96` (`--version` `ea1f3f96`, no uncommitted changes) in a target folder of its own, 10 November 2026 (`main` at `ccc9a878` has the same code):

| Operation | Fed moves ending past X 61 | A turned over them | Greatest X | Moves, programmed time |
| --- | --- | --- | --- | --- |
| Spiral, extend, ends off | 424 | 2119.5° | 66.888 | 10,081, 969.2 s |
| Rings, extend, ends off | 14 | 2160.0° | 66.888 | 10,439, 995.4 s |
| Spiral, extend (ends on) | 437 | 6799.5° (with the end facing's 13 turns) | 66.888 | 10,213, 1397.8 s |
| Rings, extend (ends on) | 27 | 6840.0° (the same) | 66.888 | 10,571, 1424.0 s |

The radius left past X 60, estimated from the programs (the 1.5875 mm ball's envelope over the fed points, not the simulation): with Face free ends off about 2.67 mm from X 61 to 66.9 (the core); with it on 2.67 mm at X 61, 0.43 mm at X 61.5, 0.11 mm at X 62.0 and 0.05 mm at X 62.2 (the nub; lane RUNOUT-TIP's figures above are from its own measurement), and the cut-free stock past X 64.2. The figures before the fix (`07ec6990`) are those of the first issue of this check: 6 fed moves past X 61, A turned 0.0°. The view and the simulation cannot be run without a display here.
