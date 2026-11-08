# Check for issue #548: the tab length check and an operation that never reaches the tab

**What was fixed** (pull request #552, [decision 0523](../../../docs/decisions/0523-tab-checks-follow-the-operations-reach.md)): an operation limited by a selection set is checked only against the tabs its path can touch, and a tab shorter than the tool is a warning, not a refusal. Before the fix, every operation using a tool wider than a tab was refused, wherever it cut.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Do not run the G-code of this project on a machine, not even as an air cut: it is made only to show the messages. Generated G-code must be verified by you before it is run on a machine.

## The project

[`project/`](project/), written by [`generate.py`](generate.py) from public material only: the user guide's baluster (X 0 to 61), 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), and **tab 1**, a cylinder 14 mm across from X -5 to 0 (5 mm long). The tool is a 6 mm ball (T6), wider than the tab is long. Two axial finishing operations use it:

- **1 Vase only**: limited to the selection set **Vase** (the 2,304 triangles from X 25 to 48), far from the tab;
- **2 Whole part**: no selection set, so it may cut down beside the tab.

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Copy the folder `project` to a folder of your own (so the repository's copy stays as it is), and open the copy with **File > Open project**.
2. Click **Operations** at the top, then **Generate**. It takes under a minute.
3. In the project panel, click **1 Vase only**. Look at its status and at the **Messages** list.
4. Click **2 Whole part**. Look at its status and at the **Messages** list.

You can ignore the many lines saying "the machine's feed limits slow this move"; they are about the tool's feed on this small part, not about this check.

## Expected result

- **1 Vase only** is generated (a path is drawn on the vase) and has **no** "Tab length" line.
- **2 Whole part** is generated and has one warning that starts "Tab length". The command-line tool prints it as below; in the Messages list the program puts the operation and the tool in front, as "Tab length (operation 'Whole part', tool '6 mm ball (check 548)'): ...":

  > Tab length: tab 1 (X -5.000 to 0.000 mm) is 5.000 mm long along X, shorter than the tool is wide (6.000 mm), so the tool may be unable to cut the gap beside it to shape. The path was made: check it before running it. Use a smaller tool, or make the tab longer

- **2 Whole part** also says it "cannot cut down beside tab 1 ... has 4.500 mm of room". That is a separate, expected note (decision 0503).

**Old behaviour (not fixed):** generation stops at **1 Vase only** with the error

> operation 'Vase only' cannot be generated: invalid workholding: tab 1 is 5.000 mm long along X, shorter than the tool '6 mm ball (check 548)' (6.000 mm); make the tab longer or use a smaller tool

**Known, not part of this fix:** the setup's own check (**Setup**, **Workholding**, **Check against the part and tools**, and the setup's notes) still lists the tab against the tool of both operations: "Tab 1 is 5.000 mm long along X, shorter than the tool T6 6 mm ball (check 548) (6.000 mm) of 1 Vase only. ...". That check does not know the paths yet; it is open item OI-2045 in [decision 0523](../../../docs/decisions/0523-tab-checks-follow-the-operations-reach.md)'s audit. Report it only if it now stops anything from being generated.

## Report back

Reply on issue #548 with: **PASS**, or **FAIL and what you saw** (the operation, its status, and the message text). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project`), Linux x86_64, on 8 November 2026.

| Build | Result |
| --- | --- |
| Before the fix: `19ed9e178` (0.45.0), the parent of the fix's merge commit `d3e988a5` | Refused: `error: operation 'Vase only' cannot be generated: invalid workholding: tab 1 is 5.000 mm long along X, shorter than the tool '6 mm ball (check 548)' (6.000 mm); make the tab longer or use a smaller tool`. Nothing was generated. |
| `main` at `ae608e28f` (0.50.0) | Both operations generated (2 toolpath groups). **Vase only**: no tab line. **Whole part**: the "Tab length: tab 1 (X -5.000 to 0.000 mm) ..." warning above and the "cannot cut down beside tab 1" note. |

`openchuck-cli validate project` on `main` also prints the two setup warnings of OI-2045 (one per operation).
