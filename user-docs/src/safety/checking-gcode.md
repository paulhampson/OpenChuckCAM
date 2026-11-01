# Checking G-code before you run it

> [!WARNING]
> Generated G-code must be verified by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. A program that looks right in the simulation can still damage your machine, your tool or you.

The program knows only what you told it: the model, the stock, the machine profile and the tools. It does not know your actual machine, your fixture, the tool you fitted or how far it sticks out. This page lists what to check, in the order you meet it.

## In the program

1. Read every message the program shows in the Setup, Operations and Simulate steps. A warning there is often the first sign that a setting does not match your job.
2. Check the machine values marked unconfirmed. The checks the program makes, such as whether a move stays inside the machine's travel, are only as good as these values. See [Confirm machine profile values](../how-to/confirm-machine-values.md).
3. Watch the whole [simulation](../concepts/simulation.md). Look for cuts into the held region, passes that leave the part, and places where the deviation colours show the cutter going too deep.

The simulation shows what the toolpath removes from the stock. It does not model your chuck, a tailstock, the tool holder or the machine, so it cannot show a collision with any of them.

## In the G-code file

Open the program in a text editor, or in the preview of the export dialog, and read it, at least to the first cut and the end.

| Check | What to look for |
| --- | --- |
| Header comment | The project, setup, post and software version you expect. |
| Units | Millimetres (`G21`). |
| Work offset | The work offset your machine is set up with (normally `G54`), with its zero where the program expects it: on the rotary axis, at the part's X origin. |
| Tools | Each tool number is the tool you will fit, in the order you expect. |
| Safe height | The safe Z clears the chuck, the jaws and the rotary axis assembly along the whole length the program uses. |
| Speeds and feeds | Spindle speeds and feeds suit your tool and material, and stay within your machine's limits. |
| A moves | The direction of A matches your machine. A program with A reversed carves a mirror image. |
| End of the program | The spindle stops, the cutter retracts, and A is unwound the way your controller expects. |

## On the machine

1. Run an [air cut](../glossary.md#air-cut) first: the program with no tool fitted and no material in the chuck, at reduced feed, with your hand near the stop. For the Makera Z1, follow [Air cut on the Makera Z1](air-cut.md).
2. Make the first cut in soft, low-cost material such as wax, foam or tooling board.
3. Stay at the machine for the whole run, and stop at any unexpected move, sound or alarm.

The [safe-practice checklist](checklist.md) puts all of this in one list to tick before each run.
