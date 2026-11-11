# Check for issue #778: a swarf ramp entry goes along the blade, not round the part

**What was fixed** (pull request #781, lane RAMPENTRY, [decision 0645](../../docs/decisions/0645-a-swarf-ramp-follows-the-pass-along-the-face.md)): a swarf pass on a twisted blade turns A as it moves along the blade, so its entry read the pass's start as a spiral and, with Entry **Ramp**, ramped **round the rotary axis** at the pass's start (decision 0543 D2). A swarf pass does not cut all round the part, so that ramp swept the tool through the other blades' tips at the far end. Its check looked only at the operation's own face set, so it never saw the other blades. Now a swarf ramp goes **back and forth over the pass's own path** along the blade, A reversing during the entry only, and comes down along the tool axis to the pass's first point. Every other strategy, and swarf with any other entry, is unchanged.

**Not the owner's project:** this check uses the public blisk example only. Your own project attached to the issue is for you to re-run (numbers only).

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at where the entry goes, at the messages and at what the simulation removes. The swarf pass itself still cuts a little into a twisted face (the Overcut warnings below). Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): a copy of the public blisk example, [`testdata/examples/blisk-start/blisk-project`](../../testdata/examples/blisk-start/blisk-project/) (the repository's `testdata/step/valid/blisk.step`, X 10 to 60, five twisted blades; 42 mm oak stock from X 9 to 61; the 3 mm flat end mill), with the user guide's swarf set-up:

- the face set **Blade wall**: face 19, the side of one blade above the hub floor;
- **Blade swarf, ramp**: swarf with the 3 mm flat end mill on Blade wall, Entry **Ramp**, every other setting at its default;
- **Blade swarf, lead**: the same with Entry **Lead**, to show that case is unchanged.

The model needs the STEP import of a release build (the release candidate has it).

## Short list (what to click)

1. Open the project, **Operations**, **Generate**: **Blade swarf, ramp** generates; **Blade swarf, lead** is refused (unchanged).
2. Click **Blade swarf, ramp** and zoom on the far end of the blade (X 50 to 60): the entry runs back and forth along the blade, not round the part.
3. Read its two **Ramp entry** notes.
4. **Simulate** and look at the far-end tips of the other four blades: not cut by the entry.

## Steps

Use the release candidate with the fix, or a build from `main` at commit `c7623522` or later (**Help > About OpenChuckCAM** shows the commit). The release candidate `ea1f3f96` does **not** have this fix.

1. Download [`778-swarf-ramp-along-the-blade.zip`](778-swarf-ramp-along-the-blade.zip) (or take it from the raw link: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/778-swarf-ramp-along-the-blade/778-swarf-ramp-along-the-blade.zip), unzip it anywhere, and open the folder `778-swarf-ramp-along-the-blade/project` with **File > Open project**.
2. Click **Operations**, then **Generate**. Read **Messages** and each operation's **Results** lines.
3. Click **Blade swarf, ramp** and zoom on the blade, especially its far end (X 50 to 60). Follow the first moves after the rapid: they run along the blade, turning A only with the blade's own twist, and reverse three times.
4. Click **Simulate** and run it. Look at the far-end tips (X 55 to 60) of the four blades that are not being cut.
5. Optional, on the command line: `openchuck-cli regenerate <project folder> --post linuxcnc --tool-numbers keep --grouping operation --out <folder>` writes the program; its first fed moves are the entry.

## Expected result

As the command-line tool prints them on `main` at `c7623522`:

- **Blade swarf, ramp** generates (219 moves, 91.9 s programmed). Its entry is **172 moves** from the rapid at X 59.804, A 16.822 down to the pass's first point at Z 9.059: back and forth along the blade between X 21.490 and 59.804, A moving only with the blade (15.6° to 16.8° at its ends, 117.6° over the blade's length), never round the part. Notes:
  - "Ramp entry (operation 'Blade swarf, ramp'): pass 1: its ramp goes back and forth over the pass's own path, reversing A 3 times in all during the entry."
  - "Ramp entry (operation 'Blade swarf, ramp'): pass 1 drops more steeply than the ramp angle just before its first point, so its ramp follows that drop there, as the pass itself does."
- The swarf pass's own warnings are as before (the pass is unchanged): "Overcut (operation 'Blade swarf, ramp'): swarf on face 19 cuts into the finished surface by 0.0158 mm at (50.509, -1.120, 10.440), more than this operation's tolerance of 0.01 mm. The face is ruled but not developable ...", and the overcuts into faces 1 (0.014 mm) and 20 (0.023 mm) at the ends of the rulings.
- **Blade swarf, lead** is refused, as before: "Lead entry (operation 'Blade swarf, lead'): pass 1: the lead-in on an arc of radius 1.500 mm would cut 11.046 mm into the finished surface at position 14 of its path, more than the tolerance of 0.010 mm. Make Lead radius smaller, or set Entry to Plunge, Ramp or Helix." That refusal is the known lead case (#572) and is not changed here.
- After **Simulate**, the far-end tips of the other blades are not cut by the entry: the simulation's deepest overcut is 0.0558 mm, within one 0.1 mm cell, and it reports no problems.

**Old behaviour (`main` at `ccc9a878`, and the release candidate `ea1f3f96`):** **Blade swarf, ramp** generated (499 moves, 216.8 s) with an entry of **451 moves at one X, 59.804**, the far end of the blades: a rapid turned A to 1524.237°, and the ramp then came down from Z 21.952 to 9.059 while A turned back to 16.8°, **1504° (about 4.2 turns) round the part**, through the other blades' tips. Lane RAMPENTRY's bounded checker measured that entry cutting **0.946 mm** into the blisk at X 60.6. Only one Ramp entry note (the steep drop), no reversals. Simulated, the far-end tips were cut up to 1.700 mm deep (the simulation's figure; it reports "10 problem(s) found").

## Report back

Reply on issue #778 with: **PASS** (the entry runs along the blade, no cut into the other blades' tips), or **FAIL and what you saw** (where the entry goes, the notes, what the simulation shows at the far-end tips). Also re-run your own project from the issue and give its numbers only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`, each operation on its own; the entry read from the program's first fed moves), Linux x86_64, release builds each in a target folder of its own, 11 November 2026, on `main` at `c7623522` (after) and `ccc9a878` (before), `--version` matching, no uncommitted changes:

| | `ccc9a878` (before) | `c7623522` (after) |
| --- | --- | --- |
| Blade swarf, ramp: moves, time | 499, 216.8 s | 219, 91.9 s |
| Entry moves | 451 | 172 |
| Entry X | 59.804 only | 21.490 to 59.804 (along the blade) |
| Entry A | 1520.9° down to 16.8° (1504°, ~4.2 turns round the part) | 15.6° to 16.8°, span 117.6°, 3 reversals |
| Entry Z | 21.952 down to 9.059 | 21.931 down to 9.059 |
| Blade swarf, lead | refused, 11.046 mm | refused, the same |

The depths are lane RAMPENTRY's, from the independent bounded checker in its test `req_op_11_req_op_07_a_swarf_ramp_on_the_blisk_cuts_no_deeper_than_its_pass` (`crates/openchuck-app/tests/step_strategies.rs`, against every face of the blisk): before, 0.946 mm at X 60.6; after, the entry's deepest 0.0268 mm, no deeper than the swarf pass's own cutting (the twist overcut swarf already warns about). That test needs the STEP kernel and was not run here.

**Simulation** (`openchuck-cli simulate project`, cell 0.1 mm, Blade swarf, ramp on its own):

| | `ccc9a878` (before) | `c7623522` (after) |
| --- | --- | --- |
| Deepest overcut against the part | 1.700 mm at (60.000, 19.997, 0.292), the far-end tips of the blades | 0.0558 mm at (51.156, -0.557, 19.791), within one cell |
| Overcut beyond one cell (0.1 mm) | at 22,315 samples, all at X 59.3 to 60.0 (the far-end tips) | none |
| Result | "10 problem(s) found in the simulation" | no problems |

The simulation's depths are limited by its 0.1 mm cell; the checker's figures above are the finer ones.

The view cannot be run without a display here.
