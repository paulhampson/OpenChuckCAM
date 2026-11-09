# Check for issue #643: a steep drop inside a roughing pass goes down no faster than the plunge feed

> [!NOTE]
> **Draft, to run on the release candidate.** [Decision 0548](https://github.com/paulhampson/OpenChuckCAM/pull/649/files) (lane PATHFEED, accepted by the owner on 8 Nov 2026) is in docs pull request #649; the code is in draft pull request #654 and not merged. This file gives today's behaviour, run on `main` at `81bade8a3`, and what the record says the fix does. When it merges, the expected counts are copied from that build, the pull request is named here, and this note is removed.

**What is to change** (rotary roughing, and Facing's Flat and Round modes): a cutting move inside a pass that goes down the tool axis is fed so that the tool tip goes down no faster than the operation's **plunge feed**. A move already that slow is unchanged; a faster one is written with a lower F word. Finishing strategies, entries, exits and links are unchanged.

**Time:** about 5 minutes (the program text, or the command line).

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It reads feed words only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the public test blisk (`testdata/stl/blisk.stl`, X 0 to 50, radius 20) in a 42 mm cylinder of stock 52 mm long from X -1, no workholding, with the user guide's tutorial machine, its 3.175 mm flat end mill and its **Rough** operation (plunge feed 300 mm/min, feed 1000 mm/min). This is the set-up decision 0548 measured.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`643-roughing-descent-feed.zip`](643-roughing-descent-feed.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/643-roughing-descent-feed/643-roughing-descent-feed.zip)), unzip it anywhere, and open the folder `643-roughing-descent-feed/project` with **File > Open project**.
2. Click **Operations**, then **Generate**, then **File > Post and export...** with the **LinuxCNC** post, and write the program.
3. If you can run Python: `python3 gcode_descent.py <the .ngc file> 300` (the script is [`gcode_descent.py`](../../gcode_descent.py) on this branch). Otherwise, note the programmed time shown for Rough.

## Expected result

- `gcode_descent.py` reports **0** moves faster down the axis than 300 mm/min.
- The programmed time of Rough is a little longer than today's (the record measured about 7.8 to 10.8 s more on this job).
- Levels and shallow moves keep the cutting feed; no message is expected.

**Old behaviour (today, `main` at `81bade8a3`):**

```text
descending fed moves: 2268; faster down the axis than 300 mm/min: 193; fastest 601.6 mm/min; by operation: {'Rough': 193}
```

## Report back

Reply on issue #643 with: **PASS**, or **FAIL and what you saw** (the count, or the time). Numbers only; no project files from your own work.

## What we saw when preparing this

Run on `main` at `81bade8a3` (Linux x86_64, release build, 8 November 2026): `openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --out OUT` (generated in under 2 s, 13,094 moves, cut 2577.1 s), then `gcode_descent.py OUT/01-setup-1.ngc 300`, which gives the line above: the 193 moves and the 600 mm/min of the record. On the tutorial baluster's own roughing the same script finds no such move, so the baluster is not a stand-in here. When the fix merges, the same run will be repeated on the merged build.

**On lane PATHFEED's branch** (`claude/lane-pathfeed-643-wip` at `259528e9a`, not merged; run 8 November 2026), the same commands give `descending fed moves: 2268; faster down the axis than 300 mm/min: 0` and a cut time of 2585.4 s against 2577.1 s on `main` (8.3 s more, within the record's 7.8 to 10.8 s), with the same 13,094 moves.
