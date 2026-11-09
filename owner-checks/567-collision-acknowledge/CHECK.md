# Check for issue #567 (decisions 0538 and 0540): a possible collision must be acknowledged before writing, and the command line checks before it writes

**What was fixed** (pull requests #623 for [decision 0538](../../docs/decisions/0538-a-possible-collision-is-a-warning-to-acknowledge.md) and #584 for [decision 0540](../../docs/decisions/0540-the-command-line-checks-before-it-writes.md)):

- A warning that may be a collision of the tool, its shank or its holder (here: a tab shorter than the tool) is a **warning to acknowledge**. The operation's mark reads "Needs acknowledging:", Messages lists it first among the warnings, and **Post and export** writes nothing until you tick a box for it. The command line writes nothing without `--acknowledge-collision-warnings`. The G-code itself does not change.
- The command line runs the export dialog's checks before it writes, and prints them (operations not simulated, machine values not confirmed, and the rest).

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Ticking the box only says you have read the warning. Do not run this project's G-code on a machine, not even as an air cut. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 78 mm of 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), and **tab 1** at the head, X -5 to 0 (5 mm long, 14 mm across). One axial finishing operation, **Whole part**, uses a 6 mm ball (T6), wider than the tab is long, so it gets the "Tab length" warning, which is a possible collision.

## Steps

Use a nightly or release-candidate build made from commit `3b99ea23e` or later (**Help > About OpenChuckCAM** shows the commit), or a build from `main`.

1. Download [`567-collision-acknowledge.zip`](567-collision-acknowledge.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/567-collision-acknowledge/567-collision-acknowledge.zip)), unzip it anywhere, and open the folder `567-collision-acknowledge/project` with **File > Open project**.
2. Click **Operations** at the top, then **Generate** (under a minute).
3. Hover over the mark beside **1 Whole part** in the project panel, and read the **Messages** list.
4. Choose **File > Post and export...**, keep the Makera Z1 post, type an output folder, and scroll down to **Checks before writing**.
5. Tick **I will verify this G-code before running it on a machine** if it is asked, but **not** the possible-collision box. Look at **Write G-code** and the line beside it.
6. Tick the possible-collision box and press **Write G-code**.
7. Optional, if you use the command-line tool: run the commands in "What we saw" below on a copy of the project.

## Expected result

> [!NOTE]
> Since decision 0536 step 4 (#659, on `main` from `0d640692`) the text after the topic starts in lower case and every line ends with a full stop; the quotes below are as `0d640692` prints them. Builds before that print the same sentences with a capital after the lead-in or without the last full stop; either form passes.

- **Step 3**: the mark is the amber Warning; its text starts **"Needs acknowledging: Tab length (operation 'Whole part', tool '6 mm ball (check 567)')"**. In Messages this warning comes first among the warnings. The other warnings ("Tab clearance (operation 'Whole part'): 1 Whole part cannot cut down beside tab 1 ...", and two "Held region (operation 'Whole part'): ..." lines) do not need acknowledging.
- **Step 4**: the checks list the tab-length warning as a possible collision, under the text "These files hold possible collisions of the tool, its shank or its holder with the part, the stock, a tab or a held region. Read each one, here and in Messages, and check it before running the program:", and a box labelled:

  > I have read the possible collisions listed for this result and will check them before running the program

- **Step 5**: **Write G-code** is greyed, with "Tick the box to say you have read the possible collisions of these files."
- **Step 6**: the file is written. Generating again asks for the tick again.

**Old behaviour (not fixed):** the tab-length warning was a plain warning: no "Needs acknowledging", no box, and the export (and the command line) wrote the file without asking. The command line printed none of the export's checks.

**Known, not part of this check:** "the tool does not turn at a step ... at X -16.001", an X outside the stock, is issue #633. Up to `81bade8a3` the command line, refusing for want of `--acknowledge-collision-warnings`, did not print the "Generated G-code must be verified ..." notice (issue #645); from `0d640692` (#659) it prints it on that refusal too.

## Report back

Reply on issue #567 with: **PASS**, or **FAIL and what you saw** (the step, the mark's text, the box's label, and whether the file was written). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

The interface cannot be run without a display here, so steps 3 to 6 were not run in the interface; the texts above are those of the program on `main`. The command-line tool was run as `openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --out OUT`, Linux x86_64, release builds, on 8 November 2026.

| Build | Without the flag | With `--acknowledge-collision-warnings` |
| --- | --- | --- |
| Before both fixes: `1fccd5847` (0.49.0), the parent of #584 | Exit 0. The file is written. The tab-length line is a plain `warning:`; no "checks before writing" are printed. | (the flag did not exist) |
| `main` at `1f3675d52` (0.50.0) | Exit 1, nothing written. It prints "checks before writing the G-code:", with `warning: Operation 'Whole part' has a possible collision, which must be acknowledged before the files are written: Tab length (...)`, the operation's three other warnings, "has not been simulated" and the unconfirmed machine values, then `error: This result has 1 possible collision that must be acknowledged before the files are written: Tab length (...). Read them, then acknowledge them to write the files. Pass --acknowledge-collision-warnings to write them.` | Exit 0. The same checks, then `warning: writing with 1 possible collision acknowledged for this run: check each before running the program`; the file is written, and the verify notice is printed. |
| `main` at `81bade8a3` (0.51.0), after decision 0536 steps 3 and 5 | Exit 1, nothing written, as above. The check now reads `warning: Tab length (operation 'Whole part', tool '6 mm ball (check 567)'): tab 1 (X -5.000 to 0.000 mm) ... Use a smaller tool, or make the tab longer. A possible collision: acknowledge it before the files are written`; the error and the flag are unchanged. The interface's mark, box and greyed-button texts quoted above are the same in the code at this commit. | (as above) |

The program written by `main` with the flag and the one written by the old build differ only in the version comment on line 2 (`0.49.0` against `0.50.0`): the acknowledgement does not change the G-code.
