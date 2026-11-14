# Machine profiles

A [machine profile](../glossary.md#machine-profile) describes one machine: its rotary axis, how far it can travel, how fast it can move, where it is safe to retract to, and how its controller handles short moves. The program uses it to place the part, to check every program it writes against the machine's limits, and to work out the feeds.

The profile does not name a [post-processor](../glossary.md#post-processor). You choose the post when you export; see [G-code and posts](gcode-and-posts.md).

## Where profiles are kept

Built in
: The Makera Z1 profile is part of the program and cannot be edited. Use **Copy to edit** to make a copy you can change.

Library
: Your own profiles are files in the library folder, one `.toml` file each. The folder is `OpenChuckCAM/machines` in your system's settings folder:

  | System | Folder |
  | --- | --- |
  | Linux | `~/.config/OpenChuckCAM/machines` (or `$XDG_CONFIG_HOME/OpenChuckCAM/machines`) |
  | Windows | `%APPDATA%\OpenChuckCAM\machines` |
  | macOS | `~/Library/Application Support/OpenChuckCAM/machines` |

  The folder does not exist until you create it: the machine section offers **Create the library folder**.

The project
: Each project keeps its own copy of the profile it uses, in `machine.toml` in the project folder. Changing a library profile never changes a project. The machine section says whether the project's copy still matches the profile it came from ("Matches ..." or "Differs from ... in: ..."), and **Update the copy from the library** brings the library's values in.

## Confirmed and unconfirmed values

Each value records where it came from, and whether it has been confirmed on the machine. A confirmed value has a tick; an unconfirmed one has a triangle, and its tooltip says where the value came from and how to check it. The status bar counts the unconfirmed values, for example "6 machine values unconfirmed".

When you change a value, it becomes unconfirmed, with the source "Changed by the user; check it on the machine". After you have measured or checked it on the machine, use **Mark confirmed...** and say how you confirmed it. [Confirm machine profile values](../how-to/confirm-machine-values.md) says how to check each one.

## The fields

The machine library window groups the fields as below. The Inspector on the Setup step shows the most-used ones, with the rest under **Travel and maximum feeds**. Positions are machine coordinates (the controller's `G53` frame) in millimetres.

### Profile

Name
: The name shown in lists. It cannot be empty.

### Rotary axis

Rotary axis
: The machine axis the chuck turns about. Only X is supported: the [A axis](../glossary.md#a-axis) turns about X.

A+ direction
: Which way the part turns for a positive A move, **positive** or **negative**. Positive is anticlockwise when you look along the axis from its positive end towards the chuck (the right-hand rule).

A range, Lowest A, Highest A
: **Continuous** if A can turn without limit, or a limited range in degrees. On a limited axis every program is checked against the range, and no turn-dropping command is written.

Rotary reference X, Rotary reference Y
: A point on the rotary axis, such as the centre of the chuck face, in machine coordinates. Along the axis it is where a setup's part position is measured from.

Axis height (machine Z)
: The machine Z of the rotary axis. Everything the program writes in Z is measured from this height, so it matters more than any other value. Measure it on your machine.

### Travel

Lowest X, Highest X, Lowest Y, Highest Y, Lowest Z, Highest Z
: The travel limits of the linear axes. Every move of an exported program is checked against them, shifted by the setup's work zero.

### Maximum feeds

X and Y feed, Z feed, A feed
: The highest [feed](../glossary.md#feed) of each axis, in mm/min (A in degrees per minute). Feeds in a program are checked against them.

X and Y rapid, Z rapid, A rapid
: The rapid move speeds, used to estimate the program's time.

A in the feed word
: How the controller reads the F word on a move that turns A and moves X, Y or Z at the same time: **separate** (LinuxCNC's rule) or **makera** (the Makera Z1's rule). It must match the feed rule of the post you export with. The program does not check that they match, so keep them consistent yourself.

### Clearance

Safe cylinder radius
: The default radius about the rotary axis that retract and rapid moves stay outside. A setup can set its own.

Safe Z
: The height above the rotary axis for large rotations, tool changes and the start and end of the program. It cannot be below the safe cylinder.

Minimum clearance radius
: Optional. The smallest safe cylinder that clears the chuck, its jaws and anything else round the axis. A setup with a smaller safe cylinder gets a warning.

### Tools and spindle

Highest tool number
: The highest T number the controller accepts. Exporting refuses a tool numbered outside 1 to this value.

Lowest spindle speed, Highest spindle speed
: The spindle speed range, in rpm. An operation whose speed is outside it cannot be posted until you change the speed.

### Short moves

Minimum block time, Block window, Short moves
: Whether the controller can read the program's lines as fast as the program supplies them. [Machine setup: short moves](../how-to/short-block-settings.md) explains the three settings.

### Spindle height

Tool length measurement
: **Automatic (tool setter)** if the controller measures each tool on a tool setter, so that a longer tool raises the spindle; otherwise none.

Spindle to axis
: The distance from the spindle, at the top of its Z travel, down to the rotary axis. A tool that sticks out further than this distance less the safe Z would need the spindle above the top of its travel at safe Z, and the program warns you.

Recommended minimum safe Z
: Advice only: the lowest safe Z that is recommended for this machine. It is never used as a limit.

## The built-in Makera Z1 profile

| Field | Value | Confirmed on a machine |
| --- | --- | --- |
| Rotary axis, A+ direction | A about X, positive (at A +90 the top of the part faces the front of the machine) | yes |
| A range | continuous | yes |
| Axis height (machine Z) | -48.6 mm | yes |
| Rotary reference X, Y | -180.4 mm, -108.8 mm | yes, but the X datum is still being checked |
| Travel | X -200 to 0, Y -200 to 0, Z -100 to 0 mm | no |
| X and Y feed, Z feed, A feed | 1200 mm/min, 600 mm/min, 1800 degrees/min | yes (A on firmware 1.1.3: `M203.1` prints `A: 30`, in degrees per second) |
| A in the feed word | makera | yes |
| Safe cylinder radius, Safe Z | 40 mm, 40 mm | no (program defaults, not machine values) |
| Highest tool number | 1000 | not applicable |
| Highest spindle speed | 13000 rpm | no |
| Minimum block time, Block window, Short moves | 7 ms, 32 blocks, Report | 7 ms yes; the window no; the mode is a program choice |
| Tool length measurement | Automatic (tool setter) | yes |
| Spindle to axis | 73.1 mm | no |
| Recommended minimum safe Z | 30 mm | no |

The unconfirmed values are the program's best information, not measurements of your machine. Measure them before you rely on them.

## Make a profile for another machine

1. On the Setup step, in the machine section, click **Manage machines...** to open the machine library window.
2. Choose a starting point:
   - **New machine** makes a blank profile. Its 18 required values (the name, the rotary reference, the six travel limits, the six feed and rapid limits, the safe cylinder radius and the safe Z) are empty until you fill them in.
   - **Duplicate** copies a library profile, and **Copy to edit** copies the built-in one.
3. Fill in or change the values.
4. Click **Save**. Nothing is written to the library until you save, and **Cancel** discards the changes.
5. Select the profile and click **Use this machine** to give the project a copy of it.

**Import...** and **Export...** read and write profile files, so you can share a profile. The project's own copy can be changed in the same window; **Save to library...** writes it back over its source, or saves it as a new profile.

A machine that is not 4-axis with A about X cannot be described. Generated G-code must be verified by you before it is run on a machine, whatever profile you use; a profile you made is yours to check.
