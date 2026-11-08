# Check for issue #638: steps between rings can be blended

> [!NOTE]
> **Draft, to run on the release candidate.** [Decision 0545](https://github.com/paulhampson/OpenChuckCAM/pull/647/files) (lane RINGSTEP, accepted by the owner on 8 Nov 2026) is in docs pull request #647; the code is in draft pull request #655 and not merged. This file gives today's behaviour, run on `main` at `81bade8a3`, and what the record says the fix does. When it merges, the expected notes are copied from that build, the pull request is named here, and this note is removed.

**What is to change:** the five strategies that cut in **Rings** (helical finishing, rest machining, continuous rotary roughing, round Facing, simultaneous finishing's spiral pattern) gain **Steps between rings**, shown beside **Passes** only when Passes is Rings:

- **Sharp**: today's step (a whole ring, then a move along X at fixed A, then the next ring), byte-identical. An operation saved before the setting reads as Sharp.
- **Blended**: after each whole ring, the tool moves to the next ring along a short piece of helix over **Blend over** degrees (default 45, more than 0 and at most 180; anything else is refused naming Blend over). A turns one way throughout. Each ring starts **Blend over** further round than the one before, so the joins no longer lie on one line of the part.
- A **new** ring operation is written Blended; saved projects are not changed.
- The ring note says how the steps are made, for example "steps between rings blended over 45°, at most 9° off the ring".

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the path's shape and its note. Do not run this project's G-code on a machine. A blended step keeps the load nearer the stepover; it says nothing about whether the program suits your machine or tool. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 74 mm of 24 mm stock from X -13 to 61, the chuck holding 8 mm, and a 3.175 mm ball. Two helical finishing operations in **Rings**, 1 mm apart:

- **Rings, as saved**: no Steps between rings setting, as a project saved before it existed;
- **Rings, blended**: Steps between rings **Blended**, Blend over **45**.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`638-blended-ring-steps.zip`](638-blended-ring-steps.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/638-blended-ring-steps/638-blended-ring-steps.zip)), unzip it anywhere, and open the folder `638-blended-ring-steps/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Click **1 Rings, as saved**. In the Inspector, **Steps between rings** reads **Sharp**. Zoom on the path: between rings the tool steps along X at one angle, all the steps on one line of the part. Read its Results note.
4. Click **2 Rings, blended**. **Steps between rings** reads **Blended** and **Blend over** 45. Each step is a short slanted piece; the joins are spread round the part. Read its Results note.
5. On **2 Rings, blended**, set **Blend over** to `0`, then `200`: each is refused, naming Blend over. Set it back to `45`.
6. Add a new helical finishing operation and set **Passes** to Rings: **Steps between rings** starts as **Blended**.
7. Optional: simulate both and compare the surface where the steps were.

## Expected result

- Both generate. **Rings, as saved** is unchanged from today (the program is the same as before the fix, apart from the version comment).
- **Rings, blended**: blended steps as described; A never reverses; the note names the blend (exact text to be copied from the merged build).
- Step 5: refused, naming **Blend over**. Step 6: a new ring operation is Blended.

**Old behaviour (today, `main` at `81bade8a3`):** there is no Steps between rings setting. **Rings, blended** is refused: `operation 'Rings, blended' cannot be generated: parameter ring_blend_angle: this strategy has no parameter of that name`. **Rings, as saved** cuts sharp steps; its note reads "Rings (operation 'Rings, as saved'): helical finishing cut 75 rings, no more than 1.0000 mm apart, each a whole turn at one place along the part, joined by steps along it at A 0".

## Report back

Reply on issue #638 with: **PASS**, or **FAIL and what you saw** (the operation, the note, and what the steps look like). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

`openchuck-cli validate project` on `main` at `81bade8a3` (Linux x86_64, release build, 8 November 2026) refuses **Rings, blended** with the message above; `regenerate` on a copy holding only **Rings, as saved** prints the Rings note quoted above. When the fix merges, both operations will be run on the merged build and on `81bade8a3`, and the programs of **Rings, as saved** compared. The interface (the setting, the view) cannot be run without a display here.
