# Check for issue #622: a note about the whole path no longer reads "move 0:"

**What was fixed** (pull request #625, [decision 0536](../../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) build step 2): a note or warning about the whole path names no move. Before, each was printed as "move 0: ...", as if it were about the first move. With the same change, an operation's warnings are said once (as warnings), not repeated as notes.

**Time:** about 3 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)) is an unchanged copy of the user guide's tutorial project, [`testdata/examples/baluster/baluster-project`](../../testdata/examples/baluster/baluster-project/), written by [`generate.py`](generate.py): the public baluster, a **Rough** and a **Finish** operation. It is the same project as the check for #600; you can do both in one sitting.

## Steps

Use a nightly or release-candidate build made from commit `1f3675d52` or later (**Help > About OpenChuckCAM** shows the commit), or a build from `main`.

1. Download [`622-whole-path-notes.zip`](622-whole-path-notes.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/622-whole-path-notes/622-whole-path-notes.zip)), unzip it anywhere, and open the folder `622-whole-path-notes/project` with **File > Open project**.
2. Click **Operations** at the top, then **Generate**.
3. Open the **Results** tab in the lower dock and read every line. Then read the **Messages** list.

## Expected result

> [!NOTE]
> The texts below are those of the release candidate: since decision 0536 step 3 (#648, on `main` from `645256c3`) every line starts with a topic and the operation. A build from `1f3675d52` up to that change prints them as "operation 'Rough': the part has a step ..." and "Operation 'Finish': the tool does not turn ..."; either form passes, as long as none says "move 0".

- No line in Results or Messages contains "move 0".
- Results shows these notes about the whole path, at the information level (as the command-line tool prints them):

  > Shoulders (operation 'Rough'): the part has a step across the rotary axis (a shoulder facing along X): the tool makes one extra turn round the axis, without moving along it, at X 49.889, so the shoulder keeps only the stock to leave

  > Free ends (operation 'Finish'): the end face at X 61.000 has nothing beyond it: the tool goes down beside it in 5 layers of 1 mm to the rotary axis, turning once round the axis in each

- **Finish** has two warnings (Warning mark, and in Messages), each said once:

  > Held region (operation 'Finish'): the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X 8.589: there it would reach over the gripped length of the stock, so that shoulder keeps what earlier operations left

  > Held region (operation 'Finish'): Nothing within one tool radius of the held region at the start of the stock is cut: no part of the tool reaches over the gripped length of the stock.

- A note about some moves still names them, for example "moves 19077 to 19148 (72 moves): ..." (that is the #600 check).

**Old behaviour (not fixed):** the same four lines were printed as `operation 'Rough': move 0: the part has a step ...`, `operation 'Finish': move 0: the tool does not turn ...`, and so on, all as warnings.

## Report back

Reply on issue #622 with: **PASS**, or **FAIL and what you saw** (the line, as it reads). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project --post makera-z1 --tool-numbers keep --out OUT`), Linux x86_64, release builds, on 8 November 2026. The Results tab was not run here (it needs a display); it reads the same notes.

| Build | Result |
| --- | --- |
| Before the fix: `9dae85d30` (0.50.0), the parent of the fix's merge commit `1f3675d52` | Four lines with `move 0:`, all `warning:`: Rough's shoulder at X 49.889, and Finish's shoulder at X 8.589, end face at X 61.000 and held region. |
| `main` at `1f3675d52` (0.50.0) | No `move 0` anywhere. Rough's shoulder and Finish's end face are `note:`; Finish's shoulder and held-region lines are `warning:`, each once in the generation output and once more under "checks before writing the G-code" as "Operation 'Finish' was made with a warning: ..." (the export's own list, decision 0540). |
| `main` at `81bade8a3` (0.51.0), after decision 0536 steps 3 and 5 | No `move 0`. The same lines with their topics: "Shoulders (operation 'Rough'): ..." and "Free ends (operation 'Finish'): ..." (`note:`), "Held region (operation 'Finish'): ..." twice (`warning:`). |

The two programs are byte-identical.
