# Check for issue #633: no "does not turn at a step" warning at an X outside the selection and the stock

**What was fixed** (pull request #769, lane BALUSTER-WARN, [decision 0551](../../docs/decisions/0551-the-held-regions-own-end-is-not-a-shoulder.md); found while checking #765): operations on a setup with a held region warned "the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X ...", at an X you could not find on the part: outside the operation's selection and even outside the stock (X -15.001, X -16.001, X -3.411). That X is one tool radius beyond an end of the held bar itself. The face search took the held region's own end face for a face of the part. Now only faces of the part are named, so these warnings go. No turn is added or removed: **every program is byte-identical**.

A real face of the part that a held region keeps the tool from is still named, by design (decision 0343 item 6), even when it is outside a selection.

**Time:** about 5 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at the warnings only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py): the three owner-check projects #633 names, put together as three setups of one project. All three are made from the public baluster of the user guide's tutorial only (none comes from a private project), each in 78 mm of 24 mm stock from X -13 with the chuck holding 8 mm (X -13 to -5):

| Setup | Operations | From check | The warning #633 saw |
| --- | --- | --- | --- |
| **Setup 1, tenon side (check 555)** | **Tenon side**: axial finishing with a 60° V-bit, limited to the tenon's side, beside an 11 mm tab | [`555-axial-into-tab`](../555-axial-into-tab/CHECK.md) | at X -15.001 |
| **Setup 2, vase and whole part (check 548)** | **Vase only** (limited to the vase) and **Whole part**: axial finishing with a 6 mm ball, beside a 14 mm tab | [`548-tab-length-limited-op`](../548-tab-length-limited-op/CHECK.md) | at X -16.001, on each |
| **Setup 3, finish vase (check 547)** | **Rough**, then **Finish vase**: helical finishing limited to the vase, with a short-flute ball; tabs at both ends | [`547-flute-check-outside-selection`](../547-flute-check-outside-selection/CHECK.md) | at X -3.411, on Finish vase |

## Short list (what to click)

1. Open the project, **Operations**, **Generate**.
2. Open **Messages** and look for "does not turn at a step": there is none.
3. The only Held region warning left is on **Whole part**: "nothing within one tool radius of the held region ... is cut" (by design).

## Steps

Use the release candidate (`main` at `ea1f3f96`), or a build from `main` at `55e6348d` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`633-no-step-note-outside-selection.zip`](633-no-step-note-outside-selection.zip) (or take it from the raw link: https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/633-no-step-note-outside-selection/633-no-step-note-outside-selection.zip), unzip it anywhere, and open the folder `633-no-step-note-outside-selection/project` with **File > Open project**.
2. Click **Operations**, then **Generate** (all five operations; a few seconds).
3. Open **Messages**. Read the warnings of each operation: **Tenon side**, **Vase only**, **Whole part**, **Finish vase** (and **Rough**). The same warnings show on each operation's **Results** lines when you click it.
4. Look for any line starting "Held region (operation '...'): the tool does not turn at a step across the rotary axis".

Optional, on the command line: `openchuck-cli regenerate <project folder> --post linuxcnc --tool-numbers keep --grouping operation --out <folder> --acknowledge-collision-warnings` prints the same warnings (the acknowledgement is needed only because **Whole part** has a Tab length warning, see below).

## Expected result

As the command-line tool prints them on `main` at `ea1f3f96`:

- **No** "does not turn at a step across the rotary axis" warning on any operation.
- **Tenon side**, **Vase only**, **Rough** and **Finish vase**: no Held region warning.
- **Whole part** keeps three warnings that are not part of this issue and are right by design:
  - "Held region (operation 'Whole part'): nothing within one tool radius of the held region at the start of the stock is cut: no part of the tool reaches over the gripped length of the stock."
  - "Tab clearance (operation 'Whole part', tool '6 mm ball (check 548)'): the tool cannot cut down beside tab 1: ..."
  - "Tab length (operation 'Whole part', tool '6 mm ball (check 548)'): tab 1 (X -5.000 to 0.000 mm) is 5.000 mm long along X, shorter than the tool is wide (6.000 mm), ...", a possible collision to acknowledge before export.
- Every operation also says it has not been simulated, as always before a simulation.
- The status badge reads **Needs attention** (the warnings above).

**Old behaviour (`main` at `a3e1ad22`, just before #769):** four more warnings, each "Held region (operation '...'): the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X ...: there it would reach over the gripped length of the stock, so that shoulder keeps what earlier operations left.":

- **Tenon side** at X -15.001;
- **Vase only** at X -16.001;
- **Whole part** at X -16.001;
- **Finish vase** at X -3.411.

## Report back

Reply on issue #633 with: **PASS**, or **FAIL and what you saw** (the operation and the warning, with its X). Numbers and messages only; no project files from your own work. On #668 the box is "#633 passes".

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post linuxcnc --tool-numbers keep --grouping operation --out OUT --acknowledge-collision-warnings`, the whole project and each operation on its own, Finish vase after Rough), Linux x86_64, release builds each in a target folder of its own, 10 November 2026, on `main` at `ea1f3f96` (the frozen release candidate; `--version` `ea1f3f96`, no uncommitted changes) and at `a3e1ad22` (the parent of #769) for the old behaviour:

| Operation | `a3e1ad22` (before) | `ea1f3f96` (after) | G-code |
| --- | --- | --- | --- |
| Tenon side | "... does not turn at a step ... at X -15.001 ..." | no such warning | byte-identical |
| Vase only | "... at X -16.001 ..." | no such warning | byte-identical |
| Whole part | "... at X -16.001 ..." | no such warning | byte-identical |
| Rough | no such warning | no such warning | byte-identical |
| Finish vase | "... at X -3.411 ..." | no such warning | byte-identical |

The five program files of the whole project compared with `cmp`, before and after: identical. The same results come from the three original check projects on `main` at `55e6348d`. The interface (Messages, the badge) cannot be run without a display here; the badge follows from the warnings listed.
