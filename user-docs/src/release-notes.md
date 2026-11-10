# Release notes

What changed in each release of OpenChuckCAM, what it cannot do yet, and which Makera Z1 machine values were confirmed for it. Each release adds a section headed with its version number, newest first, before it is released. The release workflow refuses to publish a release whose section is missing.

Every section ends with the same reminder: generated G-code must be verified by you before it is run on a machine.

## 0.55.0

The first release of OpenChuckCAM, an alpha (2026.1-alpha). It is for trying the program out and for reporting what you find. Expect rough edges, and expect the project file format and the generated programs to change in later releases.

### What is in this release

- **Models.** Import STL (ASCII and binary) and STEP files. A STEP part keeps its exact faces and edges for selection, drilling and swarf.
- **Setups.** Stock, [held regions](glossary.md#held-region) for the chuck and the tailstock, [tabs](glossary.md#tab), and a [machine profile](glossary.md#machine-profile) for each setup.
- **Strategies.** Rotary roughing; Facing (flat, round, the unheld end and box sides); helical, axial, indexed and simultaneous 4-axis finishing; rest machining; pencil clean-up; face flow finishing; edge and curve following; swarf; radial drilling. See [Operations and strategies](reference/operations.md).
- **Simulation** of the stock as it is cut, with deviation colours and a probe.
- **Posts** for the Makera Z1 and for LinuxCNC, a setup sheet with each program, and a [command line](reference/command-line.md), `openchuck-cli`, that regenerates, simulates and posts a project without a window.
- **Messages.** Errors, warnings and notes about each operation, setup and export, in one place. Possible collisions and partial paths must be acknowledged before any G-code is written.
- **Example projects** in a separate download, used by the tutorial and the worked examples.

### Changes since the recent development builds

If you used a development build, these change what the program does or what it writes. Operations saved with an earlier build open in this one, but some give a different program, as listed here. Generate them again and check the new program before you run it.

Ramp entry
: A ramp now follows the pass's own path down to its first cut point: back and forth over the start of a line, round a ring before its first point, or round the part at the start of a spiral. The program of every operation with **Entry** set to `ramp` changes. Ramps beside a held region, on a part that rises along its length and on passes that turn A now generate where they were refused before.

: **Back-and-forth ramp** no longer has an effect. It is shown only on an operation saved with it on.

: Each leg of a back-and-forth ramp is now at least as long as the tool radius wherever the start of the pass is that long. A ramp that needs little descent is then shallower than the ramp angle, never steeper. Ramps whose legs were shorter than the tool radius are now longer.

: A pass whose start is shorter than the tool radius is no longer refused: the ramp goes back and forth over it, down to a length of ten times the [tolerance](glossary.md#tolerance) (0.1 mm at the default). Over such a short length the tool's centre may go down into uncut stock, so a tool that cannot cut at its centre is refused there.

New entry settings
: Under **Entry**, used only with ramp entry: **Allow single-point plunge** (off by default) plunges a pass that is a single point instead of refusing the operation. **Steeper ramp near the axis** (off by default) uses a steeper angle, up to **Maximum ramp angle** (15° by default), where a ramp round a small ring would need more than 10 turns.

: **Plunge to remove face nubs** (off by default), with **Face nub diameter**: in roughing, where the part comes to a point near the rotary axis with stock beyond it, the tool plunges at the axis while the part turns to remove the narrow nub of stock left there, instead of ramping round ever smaller rings. The tool must be able to plunge (centre cutting), and it cuts with its full width.

Lead entry
: On rings and spirals the lead arcs now come down onto the cut and rise from it while the part turns, at the ring's position along the part. They no longer run along the part, so operations that were refused next to a held region or on a part that rises along its length now generate. Lead entry on other passes is unchanged.

: A lead whose descent goes down through stock, rather than through air, is now refused for a tool that cannot cut at its centre, as a plunge entry already was. A pass with no cut to lead into (a single point, or a ring on the rotary axis) is entered by a straight plunge, and a note names it.

Plunges into stock
: An operation with ramp or lead entry is refused, naming the tool and the first pass, when the tool cannot cut at its centre and a pass is entered by a straight plunge into the stock.

Steps between rings
: A new setting, **Steps between rings**, beside **Passes** for helical finishing, rest machining, continuous rotary roughing, round Facing and the spiral pattern of simultaneous finishing. **Blended**, the default for new operations, moves to the next ring while A turns on by **Blend over** (45° by default), instead of stepping along the part with A held. Operations saved before keep **Sharp** steps and their program. Blended steps make a program longer: on the baluster example in Rings, about 18 % for roughing and 10 % for finishing, as programmed time.

Rings beside a held region
: Helical finishing and rest machining in **Rings** beside a held region now start on a whole ring at the edge of the held region, as roughing does, instead of with a short cut along the part with A held. Ring operations with a held region at an end of the part change, saved ones too.

Extend past the stock
: Helical finishing with **Extend past the stock** now carries its spiral or rings on past the part in their own motion, so the stock past the part's end is cut all round. Saved operations with it on change, and the run-out is longer; the operation's note gives the turns and the length added.

Roughing descents
: Rotary roughing and Facing (flat and round) now go down into the stock inside a pass no faster than the plunge feed.

End facing
: Facing with the **Unheld end** shape: **Stepover (slice thickness along X)** now sets the slice along X and **Stepdown (band along the tool)** the band along the tool, as a tool's cutting data defines them. Saved operations are converted when the project is opened, so they cut as before.

: Where there is room beside the stock, End facing now enters each band from the air instead of moving the tool sideways into the end face; where there is not, it enters at the plunge feed. It checks how deep each band cuts along the tool against the stepdown, and adds passes from the top of the stock where the stock rises above the area being faced.

Swarf
: A new swarf setting, **Keep off the end faces** (on by default), stops a pass short of a face that meets an end of the faces it cuts, and the note says how much is left uncut. Saved swarf operations may give a different program.

Messages
: Refusals and other messages now take one form: what was checked, then what it was checked on, then what was found, for example "Stepover (operation 'Rough'): must be above 0 mm.". **Show setting** in Messages opens the setting a message names. Messages no longer quote requirement or decision numbers, and a count of one reads in the singular ("1 layer").

: A possible collision, such as a tab shorter than the tool is wide, a rapid move that the simulation found removing material, or a safe cylinder that does not enclose the stock, is now a warning that must be acknowledged before the files are written.

: The overall status reads "Nothing to report" when there is nothing to report. "Saved", "Opened" and "Added" are entries in the Jobs tab, not messages.

Workholding
: **Apply** in the workholding section now stores the workholding when an automatic tab has no room or overlaps a tab placed by hand, and keeps that finding as a warning on the tab's row. Only a tab outside the end zones is still refused.

Icon
: OpenChuckCAM has its own icon in the window and the taskbar, and on the Windows program file.

### Bug fixes

- The default tabs and **Add default tabs** in the workholding section now follow a change to the stock straight away.
- A workholding **Apply** is stored when its check finishes, even if you have moved on to another item first.
- A ramp is no longer refused as a cut into the finished surface on a pass whose tool edge touches a face of the part, such as a roughing ring beside an end face.
- Helical finishing and rest machining in **Rings**, with no held region over an end, no longer leave out a ring past the part's end.
- An operation limited to selected faces no longer runs past them at a convex edge.
- End facing on stock that is exactly a whole number of slices longer than needed now writes its G-code.
- The refusal to write files with possible collisions not yet acknowledged no longer shows a doubled full stop.
- When a recovered project's checkpoints cannot be written, Messages always shows the error "Recovery checkpoint: …" with **Save**, instead of sometimes a warning.
- Zooming over a shown selection set on a large model no longer redraws the whole set on each frame.

### Known limitations

[Known limitations](start/limitations.md) lists what the program cannot do yet. The most important for this release:

- **Very little has run on a real machine.** The recorded runs are attended air cuts of two reference programs and one small first cut in tooling board, on one Makera Z1. Nothing has been run on any other machine.
- **The programs are not signed.** Builds for this release are not signed or notarised. macOS refuses to open the program the first time and Windows SmartScreen may warn. [Install and first run](start/install.md) and the README in each download give the steps.
- **The macOS build has not been tested by hand.** It is built and its automatic tests run, including starting the program, but nobody has worked through it on a Mac: not the first-run dialogs of a downloaded copy, opening it from Finder or the Dock, the clipboard, Command-key shortcuts, the menu bar, full screen or the file dialogs. It is for Apple silicon only. The Windows build has had less hands-on testing than the Linux build.
- **Collisions with the machine are not checked.** The program does not model the chuck, the tailstock, the spindle, the tool holder or the machine body.
- **Partial paths.** With [adaptive spacing](glossary.md#adaptive-spacing) on, a finishing operation can stop short and ask you to accept a partial path. A partial path is not complete.
- **No screen reader support.**
- **The project format may change** in a later release.

### Makera Z1 machine values

These values in the built-in Makera Z1 profile were confirmed on the project owner's machine (firmware 1.1.2): the rotary axis along X, the direction of A+, the continuous A range, the axis height with the probe as the reference tool, the maximum feeds, the tool length measurement and the minimum block time.

These six are **not confirmed** on a machine, and the program marks them so: the travel, the clearance (safe Z and clearance radius), the block window, the spindle speed range, the spindle to axis distance and the recommended minimum safe Z. Measure them on your own machine before you rely on them: see [Confirm machine profile values](how-to/confirm-machine-values.md). Every value can differ on your machine, confirmed or not.

Generated G-code must be verified by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup.

## Development version

The *latest* version of this guide describes the nightly build, the [development build](start/install.md#download-a-development-build) made each night from the main branch. Its limits are on [Known limitations](start/limitations.md).

Generated G-code must be verified by you before it is run on a machine. OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup.
