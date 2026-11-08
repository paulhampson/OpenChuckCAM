# G-code and posts

A [post-processor](../glossary.md#post-processor), or post, turns the program's toolpaths into the G-code one controller reads. This page describes the two posts that come with OpenChuckCAM, what they write, and how to read the result.

> [!WARNING]
> Generated G-code must be verified by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup. [Checking G-code before you run it](../safety/checking-gcode.md) says what to look for.

## The built-in posts

Makera Z1
: For the Makera Z1. Files end in `.nc`.

LinuxCNC verification
: For the LinuxCNC simulator, which the project uses to check its output. Files end in `.ngc`. It is not set up for any particular LinuxCNC machine.

Both use the same template and differ only in their options and their feed rule. You choose the post when you export: in the export panel, under **Post-processor**, pick it from **Post**. **A post folder** loads a post of your own from a folder; see [Writing a post-processor](writing-a-post.md). Below the list, the panel shows what the post sets: **Units**, **Rotary axis**, **Wrap** and **Feed mode**. These come from the post's files and cannot be changed in the panel.

On the command line, `openchuck-cli regenerate` takes `--post makera-z1`, `--post linuxcnc` or `--post` and a folder.

## What a program looks like

This is the start of a program from the project's reference jobs, written by the Makera Z1 post. In a real program the second line holds the program's version, for example `(OpenChuckCAM 0.49.0)`, and the operation line holds the operation's name, its tool and its time.

```text
(cone_axial - Setup 1)
(OpenChuckCAM <version>)
(This program has not been run on your machine. Check it before you run it.)
(Simulate it and air cut it first. You are responsible for what the machine does.)
G90 G21 G17
G54
(operation <id>)
M5
T1 M6
S12000 M3
G0 X0.000 Y0.000 Z40.000 A0.000
Z12.000
G1 Z10.000 F150.0
```

And this is the end of a Makera Z1 program that turned A ten times:

```text
X10.000 Z4.177 A3600.000 F562.4
Z12.000 F150.0
G0 Z40.000
G92.4 A0 S0
M5
G53 G0 Z-3
G28
M2
```

## Line by line

Title
: The first line names the project and the setup, as a comment.

Program version
: `(OpenChuckCAM <version>)`: the version of the program that wrote the file. There is no date, so the same project and version always give the same file.

Safety notice
: Two comment lines that say the program has not been run on your machine and that you are responsible for what the machine does. Both built-in posts write them. A post of your own can leave them out by setting `safety_notice = false`; there is no switch for it when you export.

Header
: `G90` (absolute positions), `G21` (millimetres) and `G17` (the XY plane). The LinuxCNC post adds `G61`, exact stop, so the simulator stops at each point.

Work offset
: The setup's work offset register, `G54` to `G59`. It replaces the post's own.

Operation
: Each operation starts with a comment that names it, its tool and its programmed time, for example `(operation Rough, tool T1 Z1 O-flute 3.175x25, about 1923 s)`.

Tool change
: `M5` (spindle off), then `T<n> M6`, written when the tool changes. Neither post writes a tool length offset (`G43`). The Makera Z1 measures each tool on its tool setter.

Spindle
: `S<rpm> M3` at the start of every operation. Neither built-in post turns coolant on.

Moves
: `G0` for a rapid move and `G1` for a feed move. There are no arcs: curves are written as short straight moves within the operation's [tolerance](../glossary.md#tolerance). The G code, each axis word and the F word are written only when they change. Positions have 3 decimal places, feeds at least 3 significant figures.

End
: The Makera Z1 post ends with `M5`, `G53 G0 Z-3` (raise the spindle in machine coordinates), `G28` (go to the reference position) and `M2`. The LinuxCNC post ends with `M5` and `M30`.

## Units and feeds

The output is in millimetres only. Every feed is in units per minute (`G94`). The Makera Z1 does not support inverse-time feed (`G93`), so neither post writes it.

On a move that turns A and moves X, Y or Z at the same time, controllers read the F word in different ways. Each post computes F with its controller's rule, so that the move takes the time the toolpath intends:

- The LinuxCNC post applies F to the X, Y and Z distance, and to degrees per minute on a move that turns only A.
- The Makera Z1 post uses the Z1 firmware's rule, which also counts the A movement. This rule was checked in an air cut on a Z1 (firmware 1.1.2).

The machine profile's **A in the feed word** must match the post you use; see [Machine profiles](machine-profiles.md#maximum-feeds).

## The A axis

A is written as a continuous angle that is never wrapped to 0 to 360 degrees: after ten turns a program says `A3600.000`. The sign follows the machine profile's **A+ direction**.

The Makera Z1 post drops whole turns from the controller's count with `G92.4 A0 S0`. This does not move the machine. It is written only between operations, with the tool at or above the safe Z, and only when the count is beyond one turn either way. The LinuxCNC post writes no such command, and A keeps its full value to the end of the program.

## Checks when you export

Every program is read back after it is written and checked against the machine profile and the setup: units and the work offset before the first move, a feed on every cutting move, the spindle on while cutting, the travel and feed limits, the safe Z, tool changes with the spindle off, and the end of the program. It is then compared with the toolpath it came from. Export is refused when a tool number is outside the machine's range or a spindle speed is outside its spindle range.

These checks find some mistakes. They do not show that a program is right for your machine, tool, stock or fixture.

## Not yet confirmed on the Makera Z1

The A direction, the turn-dropping command and the feed rule were checked in an attended air cut on a Makera Z1. The tool change (`M5`, `T<n> M6`) and the `G28` at the end have not been run with a real tool yet. Check them in your own air cut; see [Air cut on the Makera Z1](../safety/air-cut.md).
