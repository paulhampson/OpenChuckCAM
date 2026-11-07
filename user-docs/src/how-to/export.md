# Export G-code and the setup sheet

The export writes the G-code files and the [setup sheet](../glossary.md#setup-sheet) from the project's generated toolpaths. The tutorial's [Post and export](../tutorial/10-export.md) step shows the usual case; this page covers every part of the dialog.

> [!WARNING]
> Generated G-code must be verified by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. See [Checking G-code before you run it](../safety/checking-gcode.md).

## Open it

On the Simulate step press **Post and export...**, or choose **File > Post and export...** (<kbd>Ctrl</kbd>+<kbd>E</kbd>). If the menu item is greyed out, its tooltip says what the project still needs.

## Choose what to write

Post-processor
: Pick the **Post**: Makera Z1, LinuxCNC verification, or **A post folder** for one of your own, typed in **Post folder**. Below it, **Units**, **Rotary axis**, **Wrap** and **Feed mode** show what the post writes; they come from the post and cannot be changed here. See [G-code and posts](../reference/gcode-and-posts.md).

Files
: **One file per setup** (the default), **One file per operation**, **One file per tool** (one file for each run of operations that use one tool), or **One file for the project**. A project with more than one setup cannot go in one file, because each setup has its own work offset.

Output folder
: Where the files are written; **Choose...** picks it.

File names
: **Program files** and **Setup sheet** name patterns. The defaults are `{n}-{contents}` (for example `01-setup-1.nc`; the post adds the extension) and `{project}-setup-sheet` (written as `.html` and `.pdf`). Open **Fields** for the list, such as `{project}`, `{setup}`, `{operation}`, `{tool}` and `{date}`. **Reset** returns to the default. The patterns are kept for every project on this computer.

Tool numbers
: The T numbers the program will use. **Keep as saved**, **Renumber in order of use**, or **By hand**, then **Confirm numbers** or **Use these numbers**. The numbers are saved in the project, so the next export gives the same G-code; <kbd>Ctrl</kbd>+<kbd>Z</kbd> undoes the change.

## Read the checks

**Checks before writing** lists what the program found, each line marked:

Blocks writing
: Nothing can be written until it is fixed: no operation is enabled, an operation is out of date or failed, or a file failed its read-back check. Generate again, or fix the operation the line names.

Warning
: The files can be written, but read it first: an operation not simulated, an operation made with a warning, a partial path, or machine values not confirmed on the machine.

Disabled operations are not written, and the list says so.

## Read the files

**Files to write** lists each file with its setup, its number of lines and its programmed time (the moves at their programmed feeds, not an estimate of the machine's time). Click a file to see it in **Preview**: the exact bytes that will be written, with the file's size and SHA-256 hash. **Go to line** and **Find** help you read it.

## Acknowledge and write

Under the files, the program's notice asks you to verify the program. Depending on the project you tick:

- **I will verify this G-code before running it on a machine**: once for each project on each computer;
- **I know the cut has not been simulated for this project**: when the project skips the stock simulation;
- **I accept the partial path made for this result: it is not complete, and I will check the surface before running the program**: when an operation's path is partial, for these files only.

Then press **Write G-code**. If it is greyed out, the line above it says why. If a file already exists, the program lists them and asks whether to **Replace files**; **Cancel** writes nothing.

## The result

The **Export result** window lists the files **Written**, any that **Failed** with the reason, and any **Not attempted**, with **Copy paths**. You can open it again later from Messages with **Show result**.

The setup sheet lists the project, the machine, the stock, the tools and the operations, and each program file with its SHA-256 hash. Before you run a file on the machine, compute its hash and compare it with the sheet: `certutil -hashfile <file> SHA256` on Windows, `shasum -a 256 <file>` on macOS, `sha256sum <file>` on Linux. A file whose hash differs is not the file the sheet describes.

From the command line, `openchuck-cli regenerate` does the same; see [Command line](../reference/command-line.md).
