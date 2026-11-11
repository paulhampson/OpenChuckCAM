# Step 5 re-check for issue #641: posting with the setup grouping

> [!NOTE]
> **Re-issued on 11 November 2026** after your FAIL of step 5 on rc1 (`ea1f3f96`): **File > Post and export...** with the Makera Z1 post and the files grouped by setup was refused: "cannot post the G-code: Facing written-motion verification: line 2842: rotary counter reset is not at a reached declared boundary or does not match the controller". The rest of the [#641 check](CHECK.md) is unchanged; this file replaces only its step 5.

**What was fixed** (pull request #782, lane ENDFACE-POST, [decision 0646](../../docs/decisions/0646-a-combined-rotation-is-matched-at-the-written-decimals.md)): before writing a program that holds a Facing operation, the program replays the file it is about to write and checks the motion against the toolpath. A ring that turns only A, written as one block (for example `G0 X9.088 A0.000` then `A360.000`), was compared at a slightly different X than the one written (9.0880003), so the replay fell out of step and refused a correct counter reset (`G92.4 A0 S0`, line 2842 in the setup file). The replay now reads such a block at the decimals written. **The program written is unchanged**; only the check before writing is fixed. It refused whenever such a ring followed the reset in one file: grouped by setup, by project or by tool. Grouped by operation, each file starts afresh, so it always posted.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at whether the files are written and that the programs are the same as before. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The same project as the rest of the #641 check, unchanged: the zip and [`project/`](project/) in this folder (the public baluster of the user guide's tutorial with 65 mm of stock from X -1; **Face end**, **Face end, part only**, **Rough** and **Finish**, in that order).

## Steps

Use a build from `main` at commit `f12329a4` or later (**Help > About OpenChuckCAM** shows the commit). The release candidate `ea1f3f96` does **not** have this fix.

1. Download [`641-end-facing-from-air.zip`](641-end-facing-from-air.zip) (or take it from the raw link: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/641-end-facing-from-air/641-end-facing-from-air.zip), unzip it anywhere, and open the folder `641-end-facing-from-air/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. **File > Post and export...**: choose the **Makera Z1** post and leave the files grouped **by setup** (the default). Acknowledge the possible collisions the dialog lists (the Part cut free warnings at the free ends), then write the files.
4. Do the same with **group by project**, then **by tool**.
5. Do the same with the **LinuxCNC** post, each grouping.

Optional, on the command line, for each post (`makera-z1`, `linuxcnc`) and grouping (`setup`, `project`, `tool`, `operation`):

`openchuck-cli regenerate <project folder> --post makera-z1 --tool-numbers keep --grouping setup --out <folder> --acknowledge-collision-warnings`

## Expected result

- **Every post and grouping writes its files, with no refusal:** by setup 1 file (`01-setup-1.nc`, or `.ngc` for LinuxCNC), by project 1 file, by tool 2 files (one per tool), by operation 4 files.
- In the Makera Z1 file grouped by setup, line 2842 is `G92.4 A0 S0` (the counter reset after **Face end, part only**'s last ring), followed by `G0 X9.088 A0.000`, `G1 Z10.003 F300.0` and `A360.000 F928.5`: the ring of **Rough** that turns only A.
- **The programs are the same as before the fix.** The per-operation files of both posts are byte-identical between `ea1f3f96` and `f12329a4`.

**Old behaviour (`ea1f3f96`, rc1):**

| Post | By setup | By project | By tool | By operation |
| --- | --- | --- | --- | --- |
| Makera Z1 | refused: "... line 2842: rotary counter reset is not at a reached declared boundary or does not match the controller" | refused, the same | refused, the same | written, 4 files |
| LinuxCNC | refused: "... the G-code ends before the IR does: group 3 needs a move to tip (57.731, 0.452, -4.716) A 78654.522" | refused, the same | refused: "... group 2 needs a move to tip (53.513, 5.125, -0.904) A 36820.000" | written, 4 files |

Each refusal begins "cannot post the G-code: Facing written-motion verification: ...", and nothing is written.

## Report back

Reply on issue #641 with: **PASS** (every post and grouping writes, step 5 of the check), or **FAIL and what you saw** (the post, the grouping and the message). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post POST --tool-numbers keep --grouping GROUPING --out OUT --acknowledge-collision-warnings`, the two posts in the four groupings), Linux x86_64, release builds each in a target folder of its own, 11 November 2026, on `main` at `f12329a4` (after; `--version` `f12329a4`, no uncommitted changes) and at `ea1f3f96` (rc1, before):

| Post, grouping | `ea1f3f96` (before) | `f12329a4` (after) |
| --- | --- | --- |
| Makera Z1, setup | refused, line 2842 | 1 file |
| Makera Z1, project | refused, line 2842 | 1 file |
| Makera Z1, tool | refused, line 2842 | 2 files |
| Makera Z1, operation | 4 files | 4 files, byte-identical to before |
| LinuxCNC, setup | refused, "the G-code ends before the IR does" (group 3) | 1 file |
| LinuxCNC, project | refused, the same (group 3) | 1 file |
| LinuxCNC, tool | refused, the same (group 2) | 2 files |
| LinuxCNC, operation | 4 files | 4 files, byte-identical to before |

The per-operation files were compared with `cmp`: all eight are byte-identical between the two builds. The dialog (**Post and export...**) cannot be run without a display here; it uses the same check before writing as the command line.
