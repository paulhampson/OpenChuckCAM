# Check for issues #639, #640 and #642: a ramp follows the pass, and a lead on a ring stays in the plane of the cut

> [!NOTE]
> **Draft, to run on the release candidate.** Lane RAMPENTRY's records are proposed in pull request #646 ([decision 0543](https://github.com/paulhampson/OpenChuckCAM/pull/646/files), a ramp entry follows the toolpath; decision 0544, a lead on a pass that turns A stays in the plane of the cut, and the move to an entry's start is checked against the stock). Nothing is built yet (8 Nov 2026). This file gives today's behaviour, run on `main` at `1f3675d52`, and what the records say the fix does. When the fix merges, the expected result is checked against that build (with the command line before and after), its pull request is named here, and this note is removed.

**What is to change:**

- **Ramp (0543):** a ramp no longer runs straight back along X from the first cut point. It follows the pass's own path: on a ring it comes down around the ring, before its first point, A turning the Rotation way, at the ring's X; on a line it goes back and forth over the start of the pass. It starts at the approach distance above the stock under it and arrives at the first cut point at full depth. It is allowed on STEP parts. A single-point pass is refused unless the new setting **Allow single-point plunge** (Entry, links and retract; off by default) is on.
- **Lead (0544):** on a ring or a spiral the lead-in and lead-out are arcs in the plane of the cut, at the ring's X, A and Z moving together, tangent to the cut; they no longer run along X. Line-pass leads are unchanged.
- **#642 (0544 item 6):** the move at the link height to any entry's start is checked against the stock; the tool rises first where it must, or the operation is refused naming the pass.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at where the entry moves go, in the view and in the program text. Do not run this project's G-code on a machine, not even as an air cut. A ramp or lead that follows the path says nothing about chip load or tool suitability. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 74 mm of 24 mm stock from X -13 to 61, the chuck holding 8 mm (X -13 to -5), with the tutorial's 3.175 mm flat end mill and 3.175 mm ball. Three operations:

- **Rough, ramp**: rotary roughing, Entry **Ramp**, 3 degrees;
- **Rings, ramp**: helical finishing in **Rings**, Entry **Ramp**, 3 degrees;
- **Rings, lead**: helical finishing in **Rings**, Entry **Lead**.

The held region (X -13 to -5) is where today's entries, which run along X from the start of the part, reach.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`639-640-ramp-and-lead-follow-the-path.zip`](639-640-ramp-and-lead-follow-the-path.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/639-640-ramp-and-lead-follow-the-path/639-640-ramp-and-lead-follow-the-path.zip)), unzip it anywhere, and open the folder `639-640-ramp-and-lead-follow-the-path/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. For each operation, click it and look at the start of its path in the view (zoom on the head end, X 0), and read its messages.
4. Optional: **Simulate** each operation and look at the head end.
5. Optional: **File > Post and export...**, one file per operation, and in the **Preview** read the first moves after the first `G1`: on **Rings, ramp** and **Rings, lead** the entry should change A and Z, not X.

## Expected result

- All three operations are generated; none is refused because of the held region.
- **Rough, ramp** and **Rings, ramp**: each ramp lies over the pass it enters. On rings it comes down around the first ring at that ring's X, turning the same way as the ring; nothing runs along X towards the chuck.
- **Rings, lead**: the lead-in and lead-out are arcs that turn with the part at the ring's X, meeting the cut tangentially; nothing runs along X beyond the ends of the part.
- No message names the held region for these entries.
- The exact messages (and any information note, such as one counting entries that had to rise first, #642) will be copied here from the merged build.

**Old behaviour (today, `main` at `1f3675d52`):** each operation is refused. As the command-line tool prints them:

> operation 'Rough, ramp' cannot be generated: pass 1: the ramp needs a run of 38.020 mm to descend 1.993 mm at 3.00 degrees, and it would run over the held region at the start of the stock at X -3.309 mm, where the workholding grips the stock; use a plunge entry, or a size or angle that keeps it clear of the held region

> operation 'Rings, ramp' cannot be generated: pass 1: the ramp needs a run of 134.860 mm to descend 7.068 mm at 3.00 degrees, and it would run over the held region at the start of the stock at X -5.070 mm, where the workholding grips the stock; use a plunge entry, or a size or angle that keeps it clear of the held region

> operation 'Rings, lead' cannot be generated: a move at X -3.087 mm would reach over the held region at the start of the stock, where the workholding grips the stock; change the entry or the link settings

## Report back

Reply on the issue with: **PASS**, or **FAIL and what you saw** (the operation, where its entry goes, and any message). Numbers and messages only; no project files from your own work. The records also ask you to re-run your own projects behind OI-553 and OI-790 and report numbers only.

## What we saw when preparing this

Run with the command-line tool on `main` at `1f3675d52` (Linux x86_64, release build, 8 November 2026), each operation in a copy of the project on its own (the command line stops at the first refusal): `openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --out OUT`. All three are refused with the texts above. The interface (the view of the entries) cannot be run without a display here. When the fix merges, the same runs will be repeated on the merged build, and the entry moves read from the programs.
