# Check for decision 0550 (issues #638 and #640): rings start on a whole ring at a held region's edge

**What was fixed** (pull request #664, lane RINGSTEP, [decision 0550](../../docs/decisions/0550-rings-start-on-a-whole-ring-at-a-held-regions-edge.md)): helical finishing and rest machining in **Rings** placed their rings over the whole part, and the keep-out band of a held region then dropped the rings inside it and cut short the step into the first ring outside it. So beside the chuck the pass started with a short cut **along X at A 0** (Sharp), or with a piece of a blend (Blended), not on a ring. Now the rings go from the band's edge, with a **whole ring** there, Sharp or Blended. Without a held region at an end of the part nothing changes. This is a follow-up to the #638 check ([`638-blended-ring-steps`](../638-blended-ring-steps/CHECK.md)), whose project holds the stock well away from the part and so does not show it; the lead on rings beside the chuck (#640) is checked in [`640-lead-follows-the-pass`](../640-lead-follows-the-pass/CHECK.md), operation 4.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at where the pass starts and at its programmed time. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's tutorial project (the public baluster in 63 mm of 24 mm stock from X -1, **the chuck holding 8 mm**) with its two operations replaced by the tutorial's **Finish** in **Rings** twice (3.175 mm ball):

1. **Finish rings, sharp**: Steps between rings **Sharp**;
2. **Finish rings, blended**: Steps between rings **Blended**, Blend over 45.

## Short list (what to click)

1. Open the project, **Operations**, **Generate**.
2. Click each operation and zoom on the chuck end (X 7 to 12): the pass starts on a whole ring at X 9.088, the held region's edge; no short cut along X before it.
3. Optional: post both and read the first `G1` moves: A turns from the first cutting move.

## Steps

Use the release candidate, or a build from `main` at commit `aedad524` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`638b-rings-start-at-held-edge.zip`](638b-rings-start-at-held-edge.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/638b-rings-start-at-held-edge/638b-rings-start-at-held-edge.zip)), unzip it anywhere, and open the folder `638b-rings-start-at-held-edge/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Click **1 Finish rings, sharp** and zoom on the chuck end: where the first ring is, and whether anything runs along X before it.
4. The same for **2 Finish rings, blended**.
5. Optional: **File > Post and export...** with the LinuxCNC post, one file per operation, and in the **Preview** read the first moves of each.

## Expected result

As the command-line tool prints them on `main` at `aedad524`, each operation generated on its own:

- **1 Finish rings, sharp** generates (18,681 moves): "Rings (operation 'Finish rings, sharp'): helical finishing cut 116 rings, no more than 0.4514 mm apart, each a whole turn at one place along the part, joined by steps along it at A 0." The program starts `G0 X9.088 Y0.000 Z40.000 A0.000`, `Z13.000`, `G1 Z7.712 F200.0`, `Z7.697 A2.500`, `A5.000`, `Z7.712 A7.500`, ...: the first cutting move after the descent turns A, at X 9.088.
- **2 Finish rings, blended** generates (20,339 moves), "... steps between rings blended over 45°, 3° off the ring at a radius of 12.0 mm (steeper nearer the axis)", and starts the same way, at X 9.088 with A turning.
- Both also warn, as before, that the shoulder at X 8.589 is not turned ("Held region (operation '...'): the tool does not turn at a step across the rotary axis ... at X 8.589 ..."): it lies inside the held region's keep-out band.

**Old behaviour (`main` at `fa1c2de1`, before #664):** **1** started `G0 X9.137 ... A0.000`, `Z13.000`, `G1 Z7.745 F200.0`, then `X9.228 Z7.799`, `X9.318 Z7.846`, `X9.409 Z7.886` (a 0.27 mm cut along X at A 0 into the stock beside the chuck), and only then the first ring (`Z7.871 A2.500`, ...); the note said 138 rings, no more than 0.4526 mm apart. **2** started at `X9.107 A-75.000` on a piece of a blend (X and A moving together), not on a ring.

## Report back

Reply on issue #638 with: **PASS**, or **FAIL and what you saw** (the operation, where its pass starts, and what runs before the first ring). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`, each operation on its own), Linux x86_64, release builds each in a target folder of its own, 9 November 2026, on `main` at `aedad524` (`--version` `aedad524`, no uncommitted changes) and at `fa1c2de1` for the old behaviour. The programmed times are the replay's figures (the cut time the command-line tool prints), not a machine measurement:

| | `main` at `fa1c2de1` (before) | `main` at `aedad524` (after) | Change |
| --- | --- | --- | --- |
| Finish rings, sharp | 18,578 moves, 1074.8 s; starts with 3 moves along X at A 0 from X 9.137 | 18,681 moves, 1083.3 s; starts on a ring at X 9.088 | +8.5 s (+0.8 %) |
| Finish rings, blended | 20,239 moves, 1167.4 s; starts on a piece of a blend at X 9.107 | 20,339 moves, 1176.0 s; starts on a ring at X 9.088 | +8.6 s (+0.7 %) |

Generated together in one setup, the blended operation runs after the sharp one has cut, so it removes less and its time is shorter (1040.2 s); the figures above are each operation on its own. The view cannot be run without a display here.
