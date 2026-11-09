# Check for issue #644: on ring and spiral passes, Extend past the stock follows the pass

> [!NOTE]
> **Draft, to run on the release candidate.** [Decision 0546](https://github.com/paulhampson/OpenChuckCAM/pull/647/files) (lane RINGSTEP, accepted by the owner on 8 Nov 2026) is in docs pull request #647; the code is in draft pull request #655 and not merged. This file gives today's behaviour, run on `main` at `81bade8a3`, and what the record says the fix does. When it merges, the expected notes and counts are copied from that build, the pull request is named here, and this note is removed.

**What is to change** (helical finishing only): with **Extend past the stock** on, today the run-out past the part's end is a line along X at the pass's start angle, then a 90-degree turn into the pass, cutting one strip of the overhang. Instead:

- **Spiral (Passes Helix):** the helix carries on past the part's end at its own pitch, turning the same way, out to the extended end.
- **Rings:** rings at the operation's own spacing are added past the part's end out to the extended end, joined by the operation's own steps (Sharp or Blended, decision 0545).
- The stock past the part is then cleared all round at the pass's own stepover. The run-out is longer, and a note gives the turns or rings added at each end and the path they add. A run-out towards a held end is still refused.
- This applies to saved operations too (the owner's choice); axial and indexed finishing keep the line along X.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the path's shape and its note. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 78 mm of 24 mm stock from X -13 to 65 (4 mm past the part's far end at X 61), the chuck holding 8 mm, and a 3.175 mm ball. Two helical finishing operations, 1 mm apart, with **Extend past the stock** at the **far end**:

- **Spiral, extend**: Passes Helix;
- **Rings, extend**: Passes Rings.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`644-run-outs-follow-the-pass.zip`](644-run-outs-follow-the-pass.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/644-run-outs-follow-the-pass/644-run-outs-follow-the-pass.zip)), unzip it anywhere, and open the folder `644-run-outs-follow-the-pass/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Click each operation and zoom on the far end (X 61 to 67). Read its notes in Results.
4. **Simulate** and look at the stock past the part's far end: it should be cleared all round, not in one strip.

## Expected result

- **Spiral, extend**: past X 61 the path keeps turning round the part as a helix out to the extended end; no straight line along X.
- **Rings, extend**: rings are added past X 61 out to the extended end, joined like the other rings.
- A note on each gives the turns or rings added and the length of path they add (exact text to be copied from the merged build).
- After simulating, no skin of stock is left round the overhang past X 61.

**Old behaviour (today, `main` at `81bade8a3`):** both run-outs are a line along X at one angle, out to X 66.888, with no turn of A.

## Report back

Reply on issue #644 with: **PASS**, or **FAIL and what you saw** (the operation, its note, and what the far end looks like). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool on `main` at `81bade8a3` (Linux x86_64, release build, 8 November 2026): `openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --grouping operation --out OUT`, then [`gcode_past_x.py`](../../gcode_past_x.py)` 61 OUT/*.nc`:

```text
01-setup-1-spiral-extend.nc: fed moves ending past X 61: 6; greatest X 66.888; A turned over them: 0.0 degrees
02-setup-1-rings-extend.nc: fed moves ending past X 61: 6; greatest X 66.888; A turned over them: 0.0 degrees
```

After the fix, the same command should show many more moves past X 61 and A turning over them (several turns). The view and the simulation cannot be run without a display here.

**On lane RINGSTEP's branch** (`claude/lane-ringstep` at `eb33ebf4a`, not merged; run 8 November 2026), the same commands give:

```text
01-setup-1-spiral-extend.nc: fed moves ending past X 61: 424; greatest X 66.888; A turned over them: 2119.5 degrees
02-setup-1-rings-extend.nc: fed moves ending past X 61: 14; greatest X 66.888; A turned over them: 2160.0 degrees
```

with the notes "Past the ends (operation 'Spiral, extend'): the passes carry on past the part in their own motion (the spiral continued at its pitch) to the extended ends, 0.0 turns at the start end and 5.9 at the far end, 99 mm of path in all" and "Past the ends (operation 'Rings, extend'): ... (rings at the ring spacing) ..., 0.0 turns at the start end and 6.0 at the far end, 106 mm of path in all".
