# Check for issue #442: swarf runs past the end of the blade into the neighbouring faces

**What was fixed** (pull request #571, [decision 0532](../../../docs/decisions/0532-swarf-keeps-off-the-faces-at-its-ends.md)): swarf now stops each pass short of a face that meets an end of the cut face, by the least length that keeps the tool off it. The setting **Keep off the end faces** (in **Pattern and tolerance**) is on by default; off, the pass runs to the ends as before, and each cut into an end face is warned of, by face. The faces that share an edge with the cut face (the hub floor, the blade-top cylinder) are measured too, and a cut beyond the tolerance is a warning. Before, nothing said anything, and the tool cut the disc wall.

**Time:** about 10 minutes, plus a minute to write the project.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Swarf checks the faces that share an edge with the blade, and only against the cutting part of the tool. Do not run the G-code of this project on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

[`generate.py`](generate.py) copies the user guide's blisk starting project ([`../../examples/blisk-start`](../../examples/blisk-start/): the public blisk, 42 mm oak stock, a 3 mm flat end mill and a 3 mm ball) into a folder you name, and adds the face set **Blade wall** (face 19, the side of the blade above the floor in the middle of the front view) and one operation, **1 Blade swarf** (swarf with the 3 mm flat end mill on Blade wall, every setting at its default). It is the set-up of the user guide's worked example [Swarf on a blisk blade](../../../user-docs/src/examples/swarf.md); the project is not stored here so as not to keep a second copy of the blisk.

```text
python3 testdata/owner-checks/442-swarf-past-blade-end/generate.py C:\checks\442
```

(The folder must not exist yet or must be empty. On Windows the command may be `py`.)

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Open the written folder with **File > Open project**.
2. Click **Operations** at the top, then **Generate**. It takes about half a minute.
3. Click **1 Blade swarf** and read its note (in the **Messages** list or the operation's notes).
4. Click **Simulate** and read the overcut.
5. Click **Operations** and **1 Blade swarf**; under **Pattern and tolerance**, clear **Keep off the end faces**; click **Generate stale**, read the warnings, and simulate again.
6. Tick **Keep off the end faces** again.

## Expected result

As the command-line tool prints them (the program shows the same words on the operation):

- **Step 3**, the note includes: "The pass stops short of face 5, which meets this face at an end, so the tool keeps off it: about 0.471 mm along the face is left uncut there. The pass stops short of face 15, which meets this face at an end, so the tool keeps off it: about 0.833 mm along the face is left uncut there. The faces that share an edge with this face were checked against the tool's cutting part; the shank, the holder and other faces were not." There are also warnings that the swarf cuts face 1 by 0.014 mm and face 20 (the floor) by 0.023 mm, and the face's own 0.0158 mm error.
- **Step 4**, the deepest overcut is about **0.056 mm**, and nothing is reported as cut beyond one cell (0.1 mm).
- **Step 5**, with the setting off, new warnings name the end faces: "Swarf on face 19 cuts 0.283 mm into face 5, which meets it at an end of the pass, at (20.000, -16.142, -6.165), more than this operation's tolerance of 0.01 mm. Keep off the end faces is off for this operation: turn it on to stop the pass short of face 5 ...", the same for face 15 (0.012 mm), and face 1 is now cut 0.279 mm. The simulation reports an overcut of **0.300 mm** at X 20 (the disc wall).

**Old behaviour (not fixed):** the path was the one of step 5, with **no** warning about faces 5, 15 or 1, and the 0.300 mm overcut of the disc wall shown only by the simulation.

**Open for you, OI-2079.** On some blades a small end face that meets the pass only at its very end (on this blisk, the piece of the rim at X 60, face 16 on blade 13) stops the pass 0.8 to 2.1 mm short, to avoid a 0.013 to 0.064 mm cut made by the blade's own twist. Face 19 here does not show it. If that strip matters to you, the option is a depth below which such an end face only warns; say so on the issue.

## Report back

Reply on issue #442 with: **PASS**, or **FAIL and what you saw** (the stop-short lengths, the warnings with the setting off, the overcut), and whether OI-2079's strip matters to you.

## What we saw when preparing this

The command-line tool (`openchuck-cli simulate OUT`, which generates and then simulates at the default 0.1 mm cell), Linux x86_64, 8 November 2026, with the STEP reader built from `main`.

| Build and setting | Moves | Notes and warnings about the ends | Deepest overcut (simulation) |
| --- | --- | --- | --- |
| Before the fix: `9f843457b`, the parent of the fix's merge commit `e869d932` | 48 | none (the face's own 0.0158 mm error only) | **0.300 mm** at (20.000, -18.853, -6.674); "OVERCUT beyond 0.1 mm (one cell) at 900 samples" |
| `main` at `ae608e28f` (0.50.0), Keep off the end faces on (default) | 47 | stops short of face 5 (0.471 mm) and face 15 (0.833 mm); cuts face 1 by 0.014 mm and face 20 by 0.023 mm, warned | 0.0558 mm at (51.156, -0.557, 19.791); no overcut beyond one cell |
| `main`, Keep off the end faces off | 48 | cuts face 5 by 0.283 mm and face 15 by 0.012 mm (end faces), face 1 by 0.279 mm and face 20 by 0.026 mm, all warned | 0.300 mm, the same places as before the fix |

With the setting off, `main` removes the same 1,433.0 mm³ in the same 48 moves as the old build: the path is the old one, now warned.
