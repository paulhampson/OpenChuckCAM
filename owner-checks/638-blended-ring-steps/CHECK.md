# Check for issue #638: steps between rings can be blended

**What was fixed** (pull request #655, lane RINGSTEP, [decision 0545](../../docs/decisions/0545-blended-steps-between-rings.md)): the five strategies that cut in **Rings** (helical finishing, rest machining, continuous rotary roughing, round Facing, simultaneous finishing's spiral pattern) gain **Steps between rings**, shown beside **Passes** only when Passes is Rings:

- **Sharp**: the step as before (a whole ring, a move along X at one angle, the next ring), byte for byte. An operation saved before the setting reads as Sharp.
- **Blended**: after each whole ring the tool moves to the next ring along a short piece of helix over **Blend over** degrees (default 45, more than 0 and at most 180; anything else is refused naming Blend over). A turns one way throughout, and the joins are spread round the part instead of lying on one line.
- A **new** ring operation is written Blended; saved projects are not changed.
- The ring note says how the steps are made.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the path's shape, its note and its programmed time. Do not run this project's G-code on a machine. A blended step keeps the load nearer the stepover; it says nothing about whether the program suits your machine or tool. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 74 mm of 24 mm stock from X -13 to 61, the chuck holding 8 mm, and a 3.175 mm ball. Two helical finishing operations in **Rings**, 1 mm apart:

- **Rings, as saved**: no Steps between rings setting, as a project saved before it existed;
- **Rings, blended**: Steps between rings **Blended**, Blend over **45**.

## Short list (what to click)

1. Open the project, **Operations**, **Generate**.
2. **1 Rings, as saved**: Steps between rings reads **Sharp**; steps along X at one angle.
3. **2 Rings, blended**: **Blended**, Blend over 45; short slanted steps, joins spread round the part; read the note.
4. Blend over `0`, then `200`: refused, naming Blend over. Back to `45`.
5. Add a new helical finishing operation in Rings: it starts **Blended**.

## Steps

Use the release candidate, or a build from `main` at commit `1f2c0be6` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`638-blended-ring-steps.zip`](638-blended-ring-steps.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/638-blended-ring-steps/638-blended-ring-steps.zip)), unzip it anywhere, and open the folder `638-blended-ring-steps/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Click **1 Rings, as saved**. In the Inspector, **Steps between rings** reads **Sharp**. Zoom on the path: between rings the tool steps along X at one angle, all the steps on one line of the part. Read its Results note.
4. Click **2 Rings, blended**. **Steps between rings** reads **Blended** and **Blend over** 45. Each step is a short slanted piece; the joins are spread round the part. Read its Results note.
5. On **2 Rings, blended**, set **Blend over** to `0`, then `200`: each is refused, naming Blend over. Set it back to `45`.
6. Add a new helical finishing operation and set **Passes** to Rings: **Steps between rings** starts as **Blended**.
7. Optional: simulate both and compare the surface where the steps were.

## Expected result

- Both generate. **Rings, as saved** is unchanged from before the fix.
- **Rings, as saved**: "Rings (operation 'Rings, as saved'): helical finishing cut 75 rings, no more than 1.0000 mm apart, each a whole turn at one place along the part, joined by steps along it at A 0."
- **Rings, blended**: "Rings (operation 'Rings, blended'): helical finishing cut 75 rings, no more than 1.0000 mm apart, each a whole turn at one place along the part; steps between rings blended over 45°, 6° off the ring at a radius of 12.0 mm (steeper nearer the axis)." A never reverses.
- Step 5: refused, naming **Blend over**. Step 6: the new ring operation is Blended.

**Old behaviour (not fixed, `main` at `07ec6990` and earlier):** there was no Steps between rings setting; **Rings, blended** was refused: "operation 'Rings, blended' cannot be generated: parameter ring_blend_angle: this strategy has no parameter of that name". Steps were always sharp.

## Report back

Reply on issue #638 with: **PASS**, or **FAIL and what you saw** (the operation, the note, and what the steps look like). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --grouping operation --out OUT`), Linux x86_64, release builds each in a target folder of its own, 9 November 2026:

| | `main` at `07ec6990` (before) | `main` at `1f2c0be6` (after) |
| --- | --- | --- |
| Rings, as saved | 10,332 moves, 961.6 s | the same program, byte for byte apart from the version line |
| Rings, blended | refused (no such setting) | 11,062 moves, 1056.3 s, the note above |

**Programmed time, as a plain fact** (the programmed time the replay gives, not a machine measurement): the user guide's tutorial job with both operations in Rings (its own settings; `pass_shape = "rings"` added), on `1f2c0be6`:

| Steps between rings | Rough | Finish | Both |
| --- | --- | --- | --- |
| Sharp | 756.7 s (3,805 moves) | 959.7 s (18,533 moves) | 1716.4 s |
| Blended (45°) | 863.9 s (4,882 moves) | 1056.0 s (20,196 moves) | 1919.9 s |
| Change | +14.2 % | +10.0 % | +11.9 % |

On the check's own project, Rings, blended takes 9.8 % longer than Rings, as saved (1056.3 s against 961.6 s). Lane RINGSTEP measured about +13 % on the tutorial job (roughing +18 %, finishing +10 %), probably with other settings; the figures above are those of this check's runs. The interface (the setting, the view) cannot be run without a display here.
