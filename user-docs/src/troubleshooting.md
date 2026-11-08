# Troubleshooting

Common problems, the messages the program shows for them, and what to do. [Messages and item states](reference/messages.md) explains where messages appear.

## A model will not import

The **Import model** window shows the reason in red.

"'...' is not a model format OpenChuckCAM reads: use an .stl, .step or .stp file"
: The program reads STL and STEP files only. Export the model as one of them.

"not a valid STL file: ..."
: The file is empty, cut short, or not an STL file. The message says which, for example "it is truncated". Export it again.

"STEP import failed: ..."
: See [Import a STEP file](how-to/import-step.md#if-it-does-not-import), which lists the STEP messages, including a missing STEP reader.

A question about the unit, in an amber box
: Not an error: the file's size suggests another unit, for example "read in mm the part is only 1.6 mm across". Choose the unit the model was drawn in.

## An operation cannot be generated

The operation's mark turns red, and Messages and the Results tab say "operation '...' cannot be generated:" with the reason. Common reasons:

"the project has no machine profile", "its setup has no stock", "it has no tool"
: Something the operation needs is missing. Set it on the Setup step, or choose a tool in the operation's Tool section.

"... is required: set it on the operation, or fix the cause: ..."
: A cutting value, such as the stepover or the stepdown, is empty and no cutting data fills it. The reason says why: the setup has no stock material, the tool has no cutting data for that material, or there is no tool. Type the value, or add cutting data to the tool (see [Add a tool to the library](how-to/add-a-tool.md)) and click **Update from library**.

"parameter ...: ..."
: A setting is out of its range or does not suit the tool, for example a stepdown deeper than the tool's flutes. Change the setting the message names.

"tab ... is not wholly in the head or the tail zone"
: Move the tab into a zone, in the setup's Workholding section. See [Tabs](concepts/tabs.md).

"selection set '...' cannot be used: ..."
: The set does not suit this strategy or this part. Choose another set, or **Reselect** it.

"the job '...' stopped on an internal error; please report this as a defect"
: A fault in the program. Report it (below), with what you did.

An operation that was generated with a warning still works: read the warning in Messages, and **Show setting** goes to the setting it is about.

## The simulation is slow or runs out of memory

The simulation's time and memory grow quickly as the cell gets smaller: half the cell takes about eight times the time and four times the memory.

- Keep **Cell size**, on the Simulate step under **RESOLUTION**, at the default 0.1 mm unless you need finer.
- "a ... mm cell needs ... columns for this stock, more than the 64000000 supported; choose a coarser resolution": the stock is too large for that cell. Use a larger cell.
- Lower **Memory for playback checkpoints** in **File > Settings...** if the computer runs short of memory while playing.
- **Skip stock simulation** in the Playback tab makes only the toolpaths. There is then no cut part and no check of the stock, and the export asks you to confirm that.

See [Simulation and deviation colours](concepts/simulation.md).

## The export is greyed out or refuses

**Write G-code** stays greyed out while something blocks it; the line above the button says what, and **Checks before writing** lists each problem marked **Blocks writing**. The usual cause is an operation that is out of date: generate again. The other common one is a box not yet ticked: the line then starts "Tick the box to say ...", for example "Tick the box to say you have read the possible collisions of these files." Read what the box is about before you tick it. On the command line, a result with possible collisions is written only with `--acknowledge-collision-warnings`. See [Export G-code and the setup sheet](how-to/export.md).

## Messages about recovery copies

"Recovered checkpoint: the project holds changes recovered from a checkpoint, ..."
: A warning, after you recovered a project. The changes are kept only in memory until you save. Click **Save** in the message, or use **File > Save**.

"Recovery checkpoint: the latest checkpoint could not be written (...), ..."
: An error: the program could not write a recovery copy, for example because the project folder cannot be written. The changes since the last copy are not protected if the program closes unexpectedly. Save the project, then check that its folder can be written. The error goes when a later copy is written.

"Recovery: another application instance owns this project's checkpoints. ..."
: The project is open in another copy of the program, so this one writes no recovery copies. Close the other copy.

"Recovery: this project folder is a copy of ..."
: The recovery copies in the folder belong to the project it was copied from, so they are not offered here. They are kept aside, in a folder inside the project folder.

## A command fails with a dialog

The dialog's title names what failed, for example "Save failed" or "Import model failed", and its text says why. "Import model: there is no project open." means the model was dropped on the window with no project open: open or create a project first.

## The controller rejects a program, or the machine does something unexpected

The program is not connected to the machine and cannot see what the controller does.

1. Stop the machine. Do not run the program again until you know why.
2. Note the line the controller rejected, or the line it was on.
3. Check the file on the machine is the one you exported: compare its SHA-256 hash with the setup sheet.
4. Check the values the program could not know: the origin, the height of the rotary axis, the tool lengths, the direction of A, and the unwinding at the end of the program. Confirm the unconfirmed machine values (see [Confirm machine profile values](how-to/confirm-machine-values.md)).
5. If the controller's firmware differs from the one the post was checked with (firmware 1.1.2 on the Makera Z1), its conventions may differ.

Run an [air cut](safety/air-cut.md) before trying again. If the program wrote something wrong, report it, with the program and the build.

## Reporting a problem

[Reporting a problem or asking for a feature](reporting.md) says how to open an issue, what to include, and what the program's log file holds.
