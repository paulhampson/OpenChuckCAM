# Check for issue #643: a steep drop inside a roughing pass goes down no faster than the plunge feed


**What was fixed** (pull request #654, lane PATHFEED, [decision 0548](../../docs/decisions/0548-a-steep-descent-inside-a-roughing-pass-is-fed-no-faster-than-a-plunge.md); rotary roughing, and Facing's Flat and Round modes): a cutting move inside a pass that goes down the tool axis is fed so that the tool tip goes down no faster than the operation's **plunge feed**. A move already that slow is unchanged; a faster one is written with a lower F word. Finishing strategies, entries, exits and links are unchanged.

**Time:** about 5 minutes (the program text, or the command line).

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It reads feed words only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the public test blisk (`testdata/stl/blisk.stl`, X 0 to 50, radius 20) in a 42 mm cylinder of stock 52 mm long from X -1, no workholding, with the user guide's tutorial machine, its 3.175 mm flat end mill and its **Rough** operation (plunge feed 300 mm/min, feed 1000 mm/min). This is the set-up decision 0548 measured.

## Steps

Use the release candidate, or a build from `main` at commit `864bfcf8` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`643-roughing-descent-feed.zip`](643-roughing-descent-feed.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/643-roughing-descent-feed/643-roughing-descent-feed.zip)), unzip it anywhere, and open the folder `643-roughing-descent-feed/project` with **File > Open project**.
2. Click **Operations**, then **Generate**, then **File > Post and export...** with the **LinuxCNC** post, and write the program.
3. If you can run Python: `python3 gcode_descent.py <the .ngc file> 300` (the script is [`gcode_descent.py`](../gcode_descent.py) on this branch). Otherwise, note the programmed time shown for Rough.

## Expected result

- `gcode_descent.py` reports **0** moves faster down the axis than 300 mm/min.
- The programmed time of Rough is a little longer than today's (the record measured about 7.8 to 10.8 s more on this job).
- Levels and shallow moves keep the cutting feed; no message is expected.

**Old behaviour (not fixed, `main` at `81bade8a3`):**

```text
descending fed moves: 2268; faster down the axis than 300 mm/min: 193; fastest 601.6 mm/min; by operation: {'Rough': 193}
```

## Report back

Reply on issue #643 with: **PASS**, or **FAIL and what you saw** (the count, or the time). Numbers only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --out OUT`, then `gcode_descent.py OUT/01-setup-1.ngc 300`), Linux x86_64, release builds each in a target folder of its own:

| Build | Fast descents | Cut time | Moves |
| --- | --- | --- | --- |
| Before the fix: `main` at `81bade8a3` (8 Nov 2026) | 193, the fastest 601.6 mm/min (the record's 193 and 600 mm/min) | 2577.1 s | 13,094 |
| `main` at `864bfcf8` (9 Nov 2026), the fix merged | 0 | 2585.4 s (8.3 s more, within the record's 7.8 to 10.8 s) | 13,094 |

The program on `864bfcf8` is the same as the one lane PATHFEED's branch wrote before the merge (`259528e9a`), apart from the version line. On the tutorial baluster's own roughing the script finds no such move before or after, so the baluster is not a stand-in here. No message is expected, and none appeared.
