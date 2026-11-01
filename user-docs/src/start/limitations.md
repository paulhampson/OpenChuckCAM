# Known limitations

OpenChuckCAM is in early development. Read this page before you rely on it for a real job. It describes the development version; each release's copy of this guide describes that release, and its [release notes](../release-notes.md) list what changed.

## You check every program

The program does not know your machine, your fixture or your tool. Before you run a program:

- check the origin, the height of the rotary axis, the tool lengths, the direction of A and the unwinding at the end of the program on your controller;
- run an [air cut](../glossary.md#air-cut) first, with no tool and no material.

[Checking G-code before you run it](../safety/checking-gcode.md) says how. The Makera Z1 [air cut](../safety/air-cut.md) and [safe-practice checklist](../safety/checklist.md) are the project's own starting points.

## Very little has run on a real machine

The project owner has run attended air cuts of two reference programs on one Makera Z1, and one small first cut in tooling board. Nothing else has been run, and nothing has been run on any other machine.

Some values in the built-in Makera Z1 profile have not been confirmed on a machine. The program marks them as unconfirmed, for example "6 machine values unconfirmed" in the status bar, and you must measure them on your own machine. See [Confirm machine profile values](../how-to/confirm-machine-values.md).

## What the program covers

Machines
: 4-axis rotary work only: the work is held in a chuck and turned by one [rotary axis](../glossary.md#rotary-axis), called A, which turns about X. There is no 3-axis flat-bed milling, no 5-axis work and no second rotary axis.

Controllers
: Two [posts](../glossary.md#post-processor): the Makera Z1, and LinuxCNC (the one the project uses to check its output). Other controllers need a post you write and check yourself. The output is in millimetres only, and A is continuous: it does not wrap at 0 to 360 degrees.

Collisions
: The program does not model the chuck, a tailstock, the spindle, the tool holder or the machine body, and does not check them against the part. The chuck is represented only by a [held region](../glossary.md#held-region), a length at the end of the stock that is never cut. The flute length and shank of a tool are not checked against the depth of a roughing layer. Check clearances yourself.

Models
: STL (ASCII and binary) and STEP. An STL file has no exact surface, so accuracy is measured against its triangles. A STEP file keeps its exact faces and edges for selection, drilling and swarf, but most toolpaths on a STEP part are still computed from a mesh made from it. STEP files from CAD programs other than OpenCASCADE have had little testing. A changed model cannot be re-imported into a project, and drawings (DXF or SVG curves) cannot be imported. The import dialog can copy the model into the project folder; a project that only links to a file breaks if the file moves.

Strategies
: Rotary roughing; facing; helical, axial, indexed and simultaneous 4-axis finishing; rest machining; pencil clean-up; edge and curve following; swarf; radial drilling. Swarf and radial drilling need a STEP model. Reaching tight inside corners is limited, and the faces that look along the axis (a shoulder, the end of a part) can be left with material by a finishing operation: check them in the simulation with **Deviation colours**. Simultaneous 4-axis finishing can refuse a part it cannot follow; it says so instead of cutting it badly. There are no canned drilling cycles in the output.

Partial paths
: New finishing operations have [adaptive spacing](../glossary.md#adaptive-spacing) on. Where it cannot confirm the material left between passes before it reaches its limits, it keeps the path made so far, warns you, and asks you to accept the partial path before it writes any G-code. A partial path is not complete. On the tutorial's part with the default settings this happens, so the tutorial turns adaptive spacing off.

Project files
: The project format may still change before the first release.

## Time and size

Generating toolpaths and simulating are not instant. On the tutorial's small part, generating the two operations and preparing the simulation each took a few minutes on a computer with no graphics card. Fine [stepovers](../glossary.md#stepover) on large parts take much longer, and the simulation's cell size trades accuracy against memory and time. The programmed time the program shows is the sum of the moves at their programmed feeds, not an estimate of the machine's time.

## Systems

Development builds exist for Linux, Windows and macOS (Apple silicon). They are not signed. Most hands-on checking has been done on Linux; the Windows and macOS builds have had less.
