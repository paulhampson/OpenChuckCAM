# Check for issue #512: face-flow finishing (OP-28)

**What was added** (pull requests #561 and #569, [decision 0525](../../docs/decisions/0525-face-flow-finishing.md), requirement OP-28): **Face flow finishing** cuts the selected STEP faces with passes that run parallel to a guide edge and step across the face from it. The faces go in **Cut on**, the guide in **Guide edge** (an edge set; by default the longest boundary edge chain), the neighbouring faces can be check or avoid sets, and what the tool cannot reach is left uncut and reported. It is blocked on a mesh part with a plain message, and has no exact-contact setting (decision 0525 D5).

This is a new strategy, so there is no "old behaviour" to compare beyond its absence. The check is that it does what you asked for on the blisk's blade floors.

**Time:** about 15 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Face-flow finishing checks only what its notes and warnings name. Do not run the G-code of this project on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is written by [`generate.py`](generate.py): it copies the user guide's blisk starting project ([`testdata/examples/blisk-start`](../../testdata/examples/blisk-start/)) and adds the sets and the operation of step 4 of the worked example [Selection sets on a blisk](../../user-docs/src/examples/selection-sets.md): **Hub floor** (face 20), **Floor guide** (edge 65, where the blade above the floor meets it), **Blade wall** (face 19), and **1 Floor finish**: face-flow finishing with the 3 mm ball, Cut on Hub floor, Guide edge Floor guide, Check Blade wall (Cut short), Stock to leave 0.2 mm. (You can also make the same sets and operation by hand, following that worked example.)

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Download [`512-face-flow-finishing.zip`](512-face-flow-finishing.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/512-face-flow-finishing/512-face-flow-finishing.zip)), unzip it anywhere, and open the folder `512-face-flow-finishing/project` with **File > Open project**. The zip also holds a one-page `README.txt` with these steps in short.
2. Click **Operations** at the top, **Front** and **Fit**, then **Generate**. It takes about half a minute.
3. Click **1 Floor finish**. Look at the passes on the floor in the view and read the **Results** and **Messages**.
4. Click **Simulate** and look at the floor and the blade wall.
5. Try it your way: change the guide (leave **Guide edge** empty to take the default, the longest boundary edge chain, or make an edge set of another edge of the floor), change **Stepover** or set a scallop height, add another floor to **Cut on**, and generate again. The worked example [Face-flow finishing on a blisk floor](../../user-docs/src/examples/faceflow.md) goes through these settings.

## Expected result

- **Step 3:** the passes follow the floor, running along the blade's foot (the guide) and stepping across the floor towards the next blade, not along the machine's axes. As the command-line tool prints them:
  - "face 20 is cut from its guide, edge 65 (49.1 mm long), towards the opposite side in 17 passes up to 0.953 mm apart along the face";
  - "face-flow finishing cannot reach 1 area of the selected faces, which stay uncut: X 20.000 to 60.000 near A 282 (208.6 mm2), a neighbouring face is in the way of the tool; check them in the simulation before running the program" (the strip next to the far blade, OI-2004 below);
  - 2,133 moves.
- **Step 4:** no overcut; 5,196 mm³ removed.
- **Step 5:** the passes follow the guide you choose. Report anything that does not behave as you expect: that is the point of this check.

**Open for you:**

- **OI-2004:** with the blade walls in the way, the cutter cannot reach about 35 % of the hub floors with a 3 mm ball (24 % with a 2 mm ball); the unreached strip above is that. Whether a spike on reaching narrow floors is needed is your decision.
- **OI-2053:** the guide edges are not drawn in the view yet (later work).

## Report back

Reply on issue #512 with: **PASS**, or **FAIL and what you saw** (on this project and on the floors you try), and your answer on OI-2004 if you have one.

## What we saw when preparing this

The command-line tool, Linux x86_64, 8 November 2026, with the STEP reader built from `main`.

| Build | Result |
| --- | --- |
| Before the strategy: `e1d5b516e` (0.46.0), the parent of #561's merge commit `c5aec1df` | `error: operation 'Floor finish' cannot be generated: no strategy named "face_flow" is available` |
| `main` at `ae608e28f` (0.50.0), `openchuck-cli simulate` | Generated and simulated in about 29 s: the three lines above, plus "face-flow finishing cuts 12 passes on 1 face"; 2,133 moves, 5,196.0 mm³ removed, no overcut. These are the numbers of the user guide's worked example. |

The note "cuts 12 passes on 1 face" beside "in 17 passes" reads oddly; it has been passed to the coordinator to look at. It does not change the check.
