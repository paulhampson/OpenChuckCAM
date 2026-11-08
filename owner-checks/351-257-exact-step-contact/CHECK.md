# Check for issues #351 and #257: Exact STEP surface contact, on and off

**What was added** (pull request #481, [decision 0526](../../docs/decisions/0526-stepall-close-out-and-exact-contact-setting.md), requirement OP-27): each operation whose strategy drops the tool on a STEP part has the setting **Exact STEP surface contact**, in the Inspector's **Pattern and tolerance** section, right after **Tolerance**. On, the tool is dropped on the STEP part's exact faces; off, on the import's tessellation. For helical, indexed, rest and simultaneous finishing it is on by default. For **axial finishing** and **pencil clean-up**, the two strategies this check is about, it is **off** by default, because exact contact there was measured at about 7 and 31 times the generation time, over the 2-times limit (OI-1351). The operation's note says which surface each tool was dropped on and why. On a mesh part the row is not shown.

This closes the STEP programme (#257, whose last step is #351): close #351 first, then #257.

**Time:** about 15 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Do not run the G-code of this project on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the STEP fixture `testdata/step/valid/shouldered_shaft.step` (a turned shaft 40 mm long: radius 8, a **sharp** shoulder out to radius 12 at X 12, radius 12, a shoulder in to radius 10 at X 24 with a 2 mm fillet down to radius 8), turned to lie along X, in 26 mm stock from X -10, the chuck holding 6 mm. Two operations on the whole part:

- **1 Axial, tapered ball**: axial finishing with a tapered ball (tip radius 0.5 mm, 5 degrees, 3.175 mm);
- **2 Pencil, flat**: pencil clean-up with a 2 mm flat end mill. It finds the one sharp inside corner, at X 12.

The project is written as one saved before the setting existed, so both operations open with the setting **off**, as decision 0526 item 5 says.

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Download [`351-257-exact-step-contact.zip`](351-257-exact-step-contact.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/351-257-exact-step-contact/351-257-exact-step-contact.zip)), unzip it anywhere, and open the folder `351-257-exact-step-contact/project` with **File > Open project**. The zip also holds a one-page `README.txt` with these steps in short.
2. Click **Operations** at the top, then **Generate**. It takes under half a minute.
3. Click **1 Axial, tapered ball**. In the Inspector, under **Pattern and tolerance**, find **Exact STEP surface contact**: it is not ticked. Read the operation's notes (or the **Messages** list).
4. Do the same for **2 Pencil, flat**. Note its number of moves in **Results**.
5. Tick **Exact STEP surface contact** on both operations and click **Generate stale**. Read the notes, the moves in **Results**, and how long it took.
6. Untick it again on both and generate: the notes and the paths are as in steps 3 and 4.

## Expected result

As the command-line tool prints the notes (the program shows them on each operation):

| | Off (steps 3, 4, 6) | On (step 5) |
| --- | --- | --- |
| **1 Axial, tapered ball** | "Axial finishing drops the tool on the STEP part's tessellation, made at a deflection of 0.01 mm, not on its exact faces: Exact STEP surface contact is off for this operation"; 18,900 moves in the LinuxCNC program | "Axial finishing drops the tool on the STEP part's exact faces, to within 0.0001 mm, not on its tessellation; selected faces limit the cut as exact faces too"; 18,648 moves |
| **2 Pencil, flat** | "Pencil clean-up follows the STEP part's inside corners on its exact edges, and drops the tool on the STEP part's tessellation, made at a deflection of 0.01 mm, not on its exact faces: Exact STEP surface contact is off for this operation"; "1 concave creases along the part's edges, 6 passes"; 40 moves | "... drops the tool on the STEP part's exact faces, to within 0.0001 mm, not on its tessellation; ..."; "1 concave creases along the part's edges, 31 passes"; 390 moves |

The move counts are those of the posted LinuxCNC program; **Results** in the program may count moves differently, but the off and on counts differ in the same way. With the setting on, generation took about 25 s against about 19 s off on the computer used here (four threads); on a larger part the difference grows (decision 0526 item 7). With it off, the paths are the same as before the setting existed.

**Old behaviour (before #481):** no such row; both operations always used the tessellation, and the notes ended "dropping on the exact faces is not built yet for this strategy".

## Report back

Reply on issue #351 with: **PASS**, or **FAIL and what you saw** (the notes and moves, off and on). If it passes, close #351, then #257.

## What we saw when preparing this

The command-line tool (`openchuck-cli --threads 4 regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT`), Linux x86_64, 8 November 2026, with the STEP reader built from `main`. "On" is the project saved by `main` (schema 27) with `exact_contact = true` on both operations.

| Build and setting | Axial, tapered ball | Pencil, flat | Time |
| --- | --- | --- | --- |
| Before the fix: `5c53a2157` (0.45.0), the parent of the fix's merge commit `a94facc3` (no setting) | 18,900 moves; note "... dropping on the exact faces is not built yet for this strategy" | 6 passes, 40 moves; same note | 19.5 s |
| `main` at `ae608e28f` (0.50.0), off (as opened) | 18,900 moves; "... Exact STEP surface contact is off for this operation" | 6 passes, 40 moves | 18.8 s |
| `main`, on | 18,648 moves; "... drops the tool on the STEP part's exact faces, to within 0.0001 mm ..." | 31 passes, 390 moves, all at X 11.000 (the corner at X 12, less the tool's radius) | 24.8 s |

The two programs of the old build and of `main` with the setting off are the same line for line, apart from the program header (the version, and the two notice lines `main` adds, decision 0533).
