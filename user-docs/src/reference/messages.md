# Messages and item states

The program tells you about problems in three places: the marks on the items in the project panel, the overall status at the top right, and the **Messages** tab of the dock. This page says what each means and what to do. None of them says that a program is right for your machine: generated G-code must be verified by you before it is run on a machine.

## Message levels

Error
: A red triangle. The item cannot be used as it is: an operation cannot be generated, a selection set cannot be used, a file failed its check. Export is blocked while an enabled operation is in error. Read the message, fix the cause it names, and generate again. To export the other operations meanwhile, turn the failed one off.

Warning
: An amber triangle. The item can be used, with a caveat: a path made with a warning, an operation not simulated, unconfirmed machine values. The export goes ahead and lists the warnings, so read each one and decide whether it matters for your job.

Warning to acknowledge
: An amber triangle whose tooltip starts "Needs acknowledging:". A warning that may be a collision of the tool, its shank or its holder with the part, the stock, a tab or a held region: the stock deeper than the tool's flutes, a tab shorter than the tool is wide, swarf cutting into a face next to it, a check set the tool touches, a part that can be cut free and thrown, a rapid move that removes material, a safe cylinder that does not clear the stock, or a tool too long to clear the workholding. Messages lists these first among the warnings. The export writes nothing until you tick that you have read them, and the command line until you pass `--acknowledge-collision-warnings`. A partial path is accepted with a tick of its own. [Operations and strategies](operations.md) lists which checks ask for the tick.

Information
: A blue "i". A note about what the program did, such as how many links stayed low or that an STL file has only triangles. Nothing to do, but worth reading once.

## How a message reads

Every message starts with what it is about, then says what happened, why and what to change:

```text
Flute length (operation 'Finish', tool 'Z1 ball 3.175x22'): the stock stands up to 27.249 mm above the tool's tip …
```

The first words name the check ("Flute length", "Held region", "Feed limit"), the same in Messages, the **Results** tab, the export checks and on the command line. The brackets name the items it is about as they are called now: the operation, and the tool when its size or flutes matter. Rename an operation and its messages follow.

## The Messages tab

The **Messages** tab of the dock lists every current message; its name shows how many errors and warnings there are, for example "Messages (3)". The chips **Errors**, **Warnings** and **Information** show or hide each level.

Each message has up to two links:

- the name of the item it is about, which selects the item in the project panel and the view;
- an action, which goes straight to the fix.

| Action | What it does |
| --- | --- |
| **Show** | Selects the item. |
| **Show setting** | Opens the operation at the setting the message names, brings it into view and marks it, whether it is a number, a tick box or a list. |
| **Show in view 1** (2, 3 ...) | Turns the view to an area simultaneous finishing could not reach. |
| **Review setup** | Opens the Setup step on that setup. |
| **Edit workholding** | Opens the setup's Workholding section. |
| **Reselect** | Starts picking the selection set again. |
| **Generate** | Generates the out-of-date operations. |
| **Show result** | Opens a kept export result again. |

The **Results** tab holds the full text of the last run, grouped by operation, with the same three level chips. Each line takes its message's level, and a warning appears once. The moves the machine's feed limits slowed are one line for each operation, and notes about consecutive moves that say the same thing are one line. **Copy all** copies every line, with the build information, for a bug report.

On the Setup step, the **Setup checks** tab lists the checks of the selected setup.

## Item states

Each item in the project panel has a mark; the legend under the panel names them, and the mark's tooltip says why.

| Mark | State | Means |
| --- | --- | --- |
| Green circle | Up to date | Generated, and nothing it reads has changed since. |
| Amber diamond | Stale | Something it reads changed, or it was never generated. Generate again. |
| Blue ring | Running | A job is working on it. |
| Half-filled blue circle | Finalising | Its own work is done; the run it is part of has not finished. |
| Amber triangle | Warning | It can be used, with a caveat (above). A tooltip that starts "Needs acknowledging:" is a possible collision the export asks you to acknowledge. |
| Red triangle | Error | It cannot be used (above). |
| Grey ring, name struck through | Off | You turned it off; it is not generated or written. |

## The overall status

At the right end of the bar along the top, one badge sums up the project: "Good to go", "Needs attention" or "Error to fix", the worst of the four steps. Its tooltip lists what is not right on each step. Click it to go to the first problem: a missing value, then an error, then an out-of-date operation, then a warning.

"Good to go" means only that the program found nothing more to report. It does not mean the program is right for your machine.

The status bar at the bottom also shows how many machine values are not confirmed, as a link to the machine section; see [Confirm machine profile values](../how-to/confirm-machine-values.md).

## On the command line

`openchuck-cli` prints errors as `error: ...`, warnings as `warning: ...` and information as `note: ...` on standard error, and ends with exit code 1 on an error. The notes are those of the **Results** tab: one line for each operation the machine's feed limits slowed, and one line for notes about consecutive moves that say the same thing. See [Command line](command-line.md).
