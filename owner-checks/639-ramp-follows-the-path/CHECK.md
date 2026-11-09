# Check for issue #639: a ramp entry follows the pass's own toolpath

**What was fixed** (pull request #663, lane RAMPENTRY, [decision 0543](../../docs/decisions/0543-a-ramp-entry-follows-the-toolpath.md)): a ramp used to run straight back along X from the first cut point; on rings and spirals it ran along the part's axis, off the end of the part and over the held region, and was refused there. Now:

- **A ramp follows the pass**, raised along the tool axis, from the approach distance above the stock down to the pass's own depth at its first point. On a ring it comes down around the ring at the ring's X while A turns the way the ring turns; on a line it goes back and forth over the start of the pass. Where a pass starts with too short a stretch, it goes back and forth over the pass's own path, A reversing during the entry only.
- **Allow single-point plunge** (Entry, links and retract; **off** by default): a pass that is a single point has no path for a ramp. Off, the operation is refused naming this setting; on, the tool plunges there and a note names the passes.
- **Steeper ramp near the axis**, with **Maximum ramp angle** (default 15°; off by default): a ring ramp that would need more than 10 turns at the Ramp angle uses a steeper angle, just enough for 10 turns.
- **A ring with its tip on the rotary axis** (an end face): the tool descends there while the part turns.
- **The notes are grouped**: one note per kind of entry and operation, not one per pass.
- **Plunge to remove face nubs** (off by default) is also added. It changes no program on public material: it acts only where the part comes to a point near the axis inside the roughed length with stock left round it, which none of the public models gives (see "What we saw").

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at where the entry moves go and at the messages. A ramp that follows the path says nothing about chip load or tool suitability; the first ring of a layer is still entered at full tool width. Do not run this project's G-code on a machine, not even as an air cut. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the user guide's tutorial project (the public baluster in 63 mm of 24 mm stock from X -1, the chuck holding 8 mm) with its two operations replaced by four, all with a 3° ramp:

1. **Rough rings, ramp**: rotary roughing in Rings, Entry Ramp, Allow single-point plunge on.
2. **Rough spiral, ramp**: rotary roughing (spiral), Entry Ramp, Allow single-point plunge on: **expected to be refused**.
3. **Rough spiral, steeper**: as 2, with Steeper ramp near the axis on (Maximum ramp angle 15°).
4. **Finish rings, ramp**: helical finishing in Rings, Entry Ramp, Allow single-point plunge on.

## Short list (what to click)

1. Open the project, **Operations**, **Generate**: 1, 3 and 4 generate; 2 is refused (17.6 turns).
2. Click 1 and 4: each ramp comes down over its pass, around the ring at its X; nothing runs along X towards the chuck. Read the grouped notes.
3. On 1, untick **Allow single-point plunge** and generate: refused, naming it. Tick it again.
4. Read the tooltips of the three new settings.

## Steps

Use the release candidate, or a build from `main` at commit `fa1c2de1` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`639-ramp-follows-the-path.zip`](639-ramp-follows-the-path.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/639-ramp-follows-the-path/639-ramp-follows-the-path.zip)), unzip it anywhere, and open the folder `639-ramp-follows-the-path/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Read each operation's status and its Messages and Results lines.
3. Click **1 Rough rings, ramp** and **4 Finish rings, ramp** and look at where each pass comes down.
4. On **1 Rough rings, ramp**, untick **Allow single-point plunge** (Entry, links and retract) and generate it again. Then tick it again.
5. Open the Entry, links and retract section and read the tooltips of **Allow single-point plunge**, **Steeper ramp near the axis** and **Plunge to remove face nubs**.
6. Optional: **File > Post and export...** with the LinuxCNC post, one file per operation; in the **Preview** of **Finish rings, ramp**, read the first `G1` moves.

## Expected result

As the command-line tool prints them on `main` at `fa1c2de1`:

- **1 Rough rings, ramp** generates (11,951 moves), with two notes:
  - "Ramp entry (operation 'Rough rings, ramp'): 49 passes, from pass 4 to pass 83 (4, 5, 6 ... 78, 80, 83) start with too short a stretch to ramp along without turning the part, so their ramps go back and forth over each pass's own path, reversing A 445 times in all during the entries."
  - "Ramp entry (operation 'Rough rings, ramp'): 26 passes, from pass 31 to pass 87 (31, 32, 33 ... 54, 86, 87) are single points with no path for a ramp to follow, so they are entered by a straight plunge, as Allow single-point plunge asks."
- **2 Rough spiral, ramp** is refused: "Ramp entry (operation 'Rough spiral, ramp'): pass 85 turns round the part with its tip 0.864 mm from the rotary axis, so at 3.00 degrees a ramp round it would take 17.6 turns, more than the 10 allowed. Raise Ramp angle, or turn on Steeper ramp near the axis, or set Entry to Plunge or Helix."
- **3 Rough spiral, steeper** generates (6,362 moves), with "... pass 73 turns round the part near the rotary axis, where a ramp at the Ramp angle would take more than 10 turns, so its ramp is steeper, up to 3.69 degrees, as Steeper ramp near the axis asks." (5.26 degrees when it is generated on its own, before operation 1 has cut.)
- **4 Finish rings, ramp** generates (19,098 moves), with "Ramp entry (operation 'Finish rings, ramp'): pass 4 starts on the rotary axis, where a ramp has no path to follow, so the tool descends there while the part turns 3.64 turns." In its program the ramp reads `G0 X9.137 Y0.000 Z40.000 A0.000`, `Z9.829`, three short moves along X to X 9.409 (the back and forth of a short stretch), then `Z9.797 A2.500`, `Z9.779 A5.000`, ...: Z falls while A turns, at one X.
- **Step 4**: refused, "Ramp entry (operation 'Rough rings, ramp'): pass 31 is a single point, so there is no path for the ramp to follow. Turn on Allow single-point plunge, or set Entry to Plunge or Helix."
- No message names the held region for these entries.

**Old behaviour (not fixed, `main` at `1f2c0be6` and earlier):** the new settings did not exist, so 1 to 4 were refused ("parameter allow_single_point_plunge: this strategy has no parameter of that name"). Without them, the ramp ran along X from the first cut point and was refused at the held region, for example **4**: "pass 1: the ramp needs a run of 100.270 mm to descend 5.255 mm at 3.00 degrees, and it would run over the held region at the start of the stock at X 7.156 mm, where the workholding grips the stock; ...".

## Report back

Reply on issue #639 with: **PASS**, or **FAIL and what you saw** (the operation, where its entry goes, and the message). Numbers and messages only; no project files from your own work. The record also asks you to re-run your own projects behind OI-553 and OI-790 and report numbers only.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`), Linux x86_64, release builds each in a target folder of its own, 9 November 2026, on `main` at `fa1c2de1` (`--version` `fa1c2de1`, no uncommitted changes): operations 1, 3 and 4 together, 2 on its own, and 1 with Allow single-point plunge taken out, giving the texts above; and on `main` at `1f2c0be6` for the old behaviour. The same numbers came from lane RAMPENTRY's branch before the merge.

**Plunge to remove face nubs**: on every public model tried, the program with the setting on is identical to the one with it off, so no public check of it is possible: the tutorial baluster (its far end tapers to a point that the part fills), the public `cylinder.step` with stock past its flat free end (roughing stays at the part's radius past a flat end, so no nub forms), and the public `sphere.step` with stock past its pole (identical programs apart from the operation's name). Lane RAMPENTRY agrees: a nub forms only where the part comes to a point near the axis inside the roughed length with stock left round it; its own test (`req_op_11_req_op_03_roughing_past_a_flat_free_end_plunges_its_nub`) drives the path library directly.

The interface (the view, the settings and tooltips) cannot be run without a display here. The lead on rings (#640) is its own check, [`640-lead-follows-the-pass`](../640-lead-follows-the-pass/CHECK.md).
