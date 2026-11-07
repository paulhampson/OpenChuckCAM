# Command line

`openchuck-cli` is OpenChuckCAM without a window. It opens, checks and changes a project folder, regenerates its toolpaths, simulates them and writes the G-code, so you can script a job or run it on a computer with no screen. It reads and writes the same project folders as the program with a window.

> [!WARNING]
> Every run that writes G-code ends with the program's notice: generated G-code must be verified by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. See [Checking G-code before you run it](../safety/checking-gcode.md).

The command line cannot add or change operations: make them in the program window. It can do everything else a job needs: create a project, import and place the model, choose the machine, set up the stock and the workholding, add tools, and regenerate, simulate and post a project that has operations.

## Running it

```text
openchuck-cli <command> [options]
openchuck-cli --help
openchuck-cli <command> --help
```

In the commands below, `PROJECT` is a project folder.

These options work with every command:

`--threads N`
: The number of worker threads. The default is one per core. The output is the same whatever the number (GC-07).

`--create`
: If the folder has no `project.toml`, create a project there. Without it, the command asks on a terminal and otherwise stops with an error.

`--version`
: The version, the commit it was built from, and the version of each part.

Results go to standard output. Warnings, notes and questions go to standard error. The command asks a question only when it runs in a terminal: to create a project, about the unit of an imported model, and to confirm the tool numbers. In a script, give the answer as an option instead.

| Exit code | Meaning |
| --- | --- |
| 0 | The command worked. |
| 1 | An error, printed as `error: ...`, or a check failed. |
| 2 | The command line itself was wrong, for example an unknown option. |

## Projects

`new PROJECT [--name NAME]`
: Creates an empty project. The folder is created if needed. The name defaults to the folder's name.

`validate PROJECT`
: Opens the project, checks its files, checks that each model file still matches what was imported, and regenerates to show every note. A model that cannot be read, has changed or is missing is an error. Writes nothing.

`report PROJECT`
: Prints a summary: the models, setups and operations, the machine profile with where each value came from, each setup and its checks, and each tool. Writes nothing.

`resave PROJECT`
: Loads and saves the project, rewriting only the files whose text changes.

## Models

`inspect PATH [--units U]`
: Prints the size, bounds and checks of a model file, or of every model in a project. `--units` (`um`, `mm`, `cm`, `m` or `in`, default `mm`) applies to a file.

`import PROJECT FILE [--link] [--units U]`
: Adds a model to the project. By default the file is copied into the project's `models/` folder; `--link` leaves it where it is and refers to it by its path, so the project breaks if the file moves.

`copy-model PROJECT [MODEL]`
: Copies linked models into the project's `models/` folder. Without `MODEL`, every linked model.

`units PROJECT MODEL UNIT`
: Sets the unit a model was drawn in: `um`, `mm`, `cm`, `m`, `in`, or `default`.

`transform PROJECT MODEL [STEPS...] [--clear]`
: Adds placement steps, applied in order after the model's existing ones: `translate=X,Y,Z`, `rotate=AXIS:DEGREES`, `scale=FACTOR`, `mirror=AXIS`, `centre` (onto the rotary axis), and `orient=DIRECTION` (for example `orient=+z`: turn that direction onto the rotary axis and centre the part). `--clear` removes the existing steps first.

A model can be named by its ID, its path or its file name.

## The machine

`machines [--library FOLDER]`
: Lists the built-in machine profiles and those in a library folder, with how many values are unconfirmed.

`machine PROJECT [PROFILE] [--library FOLDER] [--clear]`
: Shows the project's machine profile, every value marked `confirmed` or `UNCONFIRMED` with its source, or sets it. `PROFILE` is a built-in name such as `makera-z1`, a profile in the library folder, or the path of a profile file. The project keeps its own copy. `--clear` removes it. See [Machine profiles](machine-profiles.md).

## Setups

`setup PROJECT [SETUP] [options]`
: Adds a setup (`--new NAME`) or changes one, then prints its checks. With no options it only prints them.

| Option | Value |
| --- | --- |
| `--new NAME`, `--name NAME` | Add a setup, or rename one. |
| `--part MODEL` | The part, or `none`. |
| `--stock` | `cylinder=DIAMETER,LENGTH,X_START`, `box=X0,Y0,Z0,X1,Y1,Z1`, `bbox=OFFSET` (the part's bounding box grown by the offset), or `none`. In millimetres. |
| `--material NAME` | The stock material, or `none`. It chooses the tools' cutting data. |
| `--part-offset MM` | Moves the part along the rotary axis; the stock, tabs and work zero stay. |
| `--work-offset G54` to `G59` | The work offset register. |
| `--work-x MM` | Where the work zero is along the rotary axis. |
| `--clearance RADIUS,SAFE_Z` or `machine` | The safe cylinder and safe Z, or the machine profile's. |
| `--min-clearance MM` or `machine` | The smallest safe cylinder allowed. |
| `--clearance-from-stock MM` or `off` | A margin added to the stock's radius for the safe cylinder. |
| `--held-start LENGTH[,DIAMETER]`, `--held-end ...` or `none` | The [held region](../glossary.md#held-region) at the low-X or high-X end, never cut. |
| `--tab SPEC` | A tab: `cylinder=X_START,LENGTH,DIAMETER` or `frustum=X_START,LENGTH,START_DIAMETER,END_DIAMETER`, optionally followed by `@COUNT,START_ANGLE,RADIAL_OFFSET`. Repeat it for more tabs. |
| `--clear-tabs`, `--default-tabs` | Remove the tabs first, or add the default head and tail tabs worked out from the part. |

A tab outside the head and tail zones is refused before anything is saved. The printout includes where to set the work offset on the machine, for example "set G54 to machine X ... Y ... Z ... at A 0".

## Tools

`tools PROJECT [--library FOLDER]`
: Lists the project's tools with their checks and, with `--library`, how each differs from the library. The project's copy is always the one used.

`tool PROJECT TOOL [options]`
: Shows a project tool, or changes it. `TOOL` is a name, an ID or `T<number>`.

  - `--add --library FOLDER` copies the tool from the library into the project, with the number from the machine's tool set, or the next free number.
  - `--update --library FOLDER` replaces the project's copy with the library's, keeping its number.
  - `--number N`, `--length-offset N`, `--diameter-offset N` set the T number and the H and D registers (`0` follows the tool number).
  - `--remove` takes it out of the project.

`tool-numbers PROJECT [--auto | --set TOOL=NUMBER ...] [--max N]`
: Shows the tool numbers the program will use, or changes them and saves them in the project. `--auto` numbers the tools in order of first use from 1; `--set` gives one tool a number and can be repeated.

`library LIBRARY <subcommand>`
: Works with a tool library folder: `list`, `import FROM`, `export TO [TOOLS...]`, `profile TOOL FILE` (give a tool a custom profile), `store PROJECT TOOL` (copy a project's tool into the library), and `tool-set MACHINE [--set NUMBER=TOOL ...] [--unset NUMBER ...]`. See [Tool library](tool-library.md).

The command line does not install the ready-made tools: the program window installs them into your tool library the first time it starts. Point `--library` at that folder (see [Tool library](tool-library.md#where-the-library-is-kept)).

## Regenerating and writing G-code

`regenerate PROJECT [options]`
: Regenerates every toolpath. With `--post` and `--out`, it writes the G-code.

| Option | Default | What it does |
| --- | --- | --- |
| `--post POST` | none: toolpaths only | `makera-z1`, `linuxcnc`, or the folder of a post of your own. See [G-code and posts](gcode-and-posts.md). |
| `--out FOLDER` | none: a summary only | The folder to write into. It needs `--post` or `--setup-sheet`. |
| `--tool-numbers keep` or `auto` | none | Confirms the tool numbers shown (`keep`), or numbers the tools in order of use (`auto`). Required with `--post` when no terminal can ask. A change is saved in the project. |
| `--grouping` | `setup` | How the program is split into files: `project` (one file; only for a project with one setup), `setup`, `operation`, or `tool`. |
| `--setup-sheet` | off | Also writes the setup sheet as HTML and PDF. It needs `--out`. |
| `--acknowledge-partial-path` | off | Writes the files even though an operation's path is partial (see [Operations and strategies](operations.md#a-partial-path-must-be-acknowledged)), for this run only. Without it, nothing is written while a path is partial. |

After it writes, `regenerate` reads each file back and checks it. For each file it prints the number of moves and whether the replay matches the toolpath, then any check that failed, and then the notice. If a file fails its checks, the command ends with an error and exit code 1. The file is still in the output folder: do not use it.

`simulate PROJECT [--cell MM]`
: Regenerates and simulates the material removal of every operation, then compares the stock left with each setup's part: the most material left and the deepest overcut, and any rapid move that removes material. It exits with 1 if it finds an overcut deeper than one cell or a rapid move through material. `--cell` sets the simulation's cell size, from 0.025 mm to 1 mm (default 0.1 mm), and saves it in the project: finer costs time and memory.

`gcode-sim PROGRAM REPORT [--tolerance T] [--source-lines FIRST:LAST]`
: A check the project uses: compares the report of a run in the LinuxCNC simulator with the program it ran.

## An example

This takes the finished tutorial project from the repository, [`testdata/examples/baluster/baluster-project`](https://github.com/paulhampson/OpenChuckCAM/tree/main/testdata/examples/baluster/baluster-project), and writes its G-code and setup sheet. Copy it first, so the example stays as it is:

```text
cp -r testdata/examples/baluster/baluster-project baluster
openchuck-cli validate baluster
openchuck-cli tool-numbers baluster
openchuck-cli simulate baluster
openchuck-cli regenerate baluster --post makera-z1 --tool-numbers keep --out baluster-out --setup-sheet
```

`tool-numbers` shows `T104 Z1 O-flute 3.175x25` and `T204 Z1 ball 3.175x22`. The last command writes `01-setup-1.nc`, `baluster-setup-sheet.html` and `baluster-setup-sheet.pdf` into `baluster-out`, then ends with lines like these:

```text
baluster: 2 toolpath groups, 1 G-code files
  01-setup-1.nc: 27964 moves; replay matches the toolpath (tip 0.0007 mm, A 0.0005 deg; cut 1564.5 s, toolpath 1564.5 s)
Generated G-code must be verified by the user before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup.
```

"Replay matches the toolpath" means the file says what the toolpath says. It does not mean the program is right for your machine, stock or fixture.

On this project, `validate` and `regenerate` also print one warning line for each move the machine's feed limits slow down: many thousands. To read the rest, send the warnings to a file:

```text
openchuck-cli regenerate baluster --post makera-z1 --tool-numbers keep --out baluster-out 2> warnings.txt
```

A project started on the command line goes as far as the setup and its tools; add the operations in the program window:

```text
openchuck-cli new baluster2
openchuck-cli import baluster2 testdata/examples/baluster/baluster.stl
openchuck-cli machine baluster2 makera-z1
openchuck-cli transform baluster2 baluster.stl centre
openchuck-cli setup baluster2 --new "Setup 1" --part baluster.stl --stock cylinder=24,63,-1 --material Hardwood --held-start 8
openchuck-cli tool baluster2 "Z1 ball 3.175x22" --add --library ~/.config/OpenChuckCAM/tools
```

The library path is the Linux one; see [Tool library](tool-library.md#where-the-library-is-kept) for the others.
