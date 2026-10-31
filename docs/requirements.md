# OpenChuckCAM – Initial Requirements

2026-10-10 · @Paul H

## 1. Introduction

This document sets the initial requirements for a Rust CAM application that turns a 3D model into G-code for true 4-axis (XYZA) CNC machining.

**Aim.** A user loads a 3D object, applies machining strategies (optionally driven by geometry selected on the model), checks the result in a simulation, and exports G-code for their machine. The focus is solely 4th-axis machining: toolpaths are real XYZA motion that can express complex operations, going well beyond basic unwrap strategies.

**In scope**

- Import and visualisation of 3D models
- Stock, setup and 4th-axis (rotary) definition
- Tool management
- 4-axis toolpath strategies, selected by geometry
- Toolpath visualisation and material removal simulation
- G-code output through multiple post-processors
- Human-readable, version-control-friendly project files
- Secondary goals: collision detection, built-in version control, and a pathing library other projects such as FreeCAD can integrate

**Out of scope (initial release)**

- Direct interfacing with, or streaming to, a CNC machine
- CAD modelling or editing of the imported geometry
- 5-axis machining and lathe/turning operations

**Priority key**

| Priority | Meaning |
|---|---|
| P1 | Core: needed for the first usable release |
| P2 | Secondary: planned, follows the core |
| P3 | Suggested: included only if you approve it (see section 6) |

**Glossary**

| Term | Meaning |
|---|---|
| 4th axis / A axis | Rotary axis, assumed to rotate the workpiece about the X axis |
| XYZA | Simultaneous linear X, Y, Z motion plus rotation A |
| Unwrap | Mapping a cylindrical surface onto a flat plane to apply 2D/3-axis strategies |
| Post-processor | Translates neutral toolpath data into the G-code dialect of a specific controller |
| WCS | Work coordinate system, the origin and orientation of the part on the machine |
| Stock | The raw material blank before cutting |

## 2. Users and primary workflow

The assumed user is a machinist, maker or small-shop programmer who owns a mill with a rotary axis and wants more than unwrap-style toolpaths, without the cost of a commercial multi-axis CAM package. They are comfortable with G-code, tooling and feeds and speeds.

The core workflow is:

1. Create a project and import a 3D model.
2. Define the machine, stock and rotary-axis setup, and position the part in it.
3. Choose or define the tools.
4. Add an operation: pick a strategy, select the geometry it applies to, and set its parameters.
5. Generate the toolpath and review it in the 3D view.
6. Simulate material removal; return to step 4 to adjust until the result is right.
7. Post-process the operations into G-code for the target controller and export it.
8. Save the project at any point so it can be reopened or committed to version control.

## 3. Core functional requirements

These requirements define the first usable release unless marked P2. Each has a unique ID so it can be referenced in discussion, tasks and tests.

### 3.1 Model import and geometry

| ID | Requirement | Priority |
|---|---|---|
| IMP-01 | Import STL files (ASCII and binary). | P1 |
| IMP-02 | Import STEP files and retain the B-rep (faces, edges), so selection and surface evaluation use exact geometry, not only a mesh. Conditional waiver: if spike SP-9 finds no usable exact evaluation of NURBS faces, the first release evaluates non-analytic faces on the tessellation and the UI says so. | P1 |
| IMP-03 | Read model units from the file, allow the user to override them, and work internally in a single unit system (millimetres). | P1 |
| IMP-04 | Check imported meshes for holes, non-manifold edges, flipped normals and degenerate triangles, and report problems to the user. | P1 |
| IMP-05 | Translate, rotate, scale and mirror the model, and orient it relative to the rotary axis. | P1 |
| IMP-06 | Tessellate B-rep models to a mesh at a user-controlled tolerance for display and toolpath computation. | P1 |
| IMP-07 | Re-import a changed source file while keeping operations and geometry selections attached where they can still be matched. | P2 |
| IMP-08 | Import curves from DXF and SVG files, for engraving and carving along curves (see OP-08). | P2 |

### 3.2 Visualisation and user interface

| ID | Requirement | Priority |
|---|---|---|
| UI-01 | Cross-platform desktop application for Windows, macOS and Linux from one codebase. | P1 |
| UI-02 | Interactive 3D viewport: orbit, pan, zoom, standard views, perspective and orthographic cameras, shaded, wireframe and edge display. | P1 |
| UI-03 | Display the rotary axis, work coordinate system, stock, part, held regions and retaining tabs together in the viewport. | P1 |
| UI-04 | Select geometry in the viewport: faces and edges on B-rep models; triangle regions on meshes, including select-by-angle and flood fill. | P1 |
| UI-05 | Save selections as named sets that operations can reference and that update when the selection is edited. | P1 |
| UI-06 | Project tree listing the model, stock, setups and operations; operations can be reordered, enabled, disabled and duplicated. | P1 |
| UI-07 | Parameter panels with units, validation and sensible defaults. | P1 |
| UI-08 | Undo and redo for all edits to the project. | P1 |
| UI-09 | Run toolpath generation and simulation off the UI thread, with progress reporting and cancellation. | P1 |
| UI-10 | Organise the work as ordered steps (model, setup, operations, simulate) that the user can move between at any time, with post and export as an action of its own, on the simulate step and in the menu. A step or action whose inputs do not exist yet is unavailable and says why. | P1 |
| UI-11 | Show the state of each model, setup, selection set and operation (up to date, stale, running, error, warning, off) in the project tree, by shape as well as colour, with a legend that stays visible. An item in error says why and how to fix it. | P1 |
| UI-12 | Collect errors, warnings and information from import, setup checks, file checks, generation and jobs in one messages panel, by severity, each naming its source and offering the action that fixes it. After an export attempt, keep a dismissible result listing every full path actually written, the failed file and reason, and files not attempted; offer copying of written paths. Retain the result in Messages for the open project session, with an action to show it again after dismissal. | P1 |
| UI-13 | Show, for each machine profile value, whether it is confirmed on the machine or not, with where the value came from and how to confirm it. | P1 |
| UI-14 | Every command is available from the menus and the keyboard, every control can be used with the keyboard, and anything shown on hover is also shown on keyboard focus. | P1 |
| UI-15 | Keep per-user settings on the computer (window layout, scale, theme, display units, recent projects, export acknowledgements), never in the project. | P1 |
| UI-16 | The window is usable at 1366 by 768 logical pixels at 100% scale, and its panels can be resized and collapsed. | P1 |
| UI-17 | A command search that finds any command by name and runs it. | P2 |
| UI-18 | Display and enter lengths in inches as a user setting, and accept a unit typed with a value; values are stored in millimetres. | P2 |
| UI-19 | Parameter forms group settings as basic and advanced, mark values that differ from their default with a way to reset them, and show derived values with how they were worked out. | P2 |
| UI-20 | A light theme beside the dark one. | P2 |
| UI-21 | Measure distances, angles and radii in the viewport, cut the view with a section plane, and turn the part to a chosen A angle in the view without changing the program. | P2 |
| UI-22 | Show the undo history and let the user return to any step in it. | P2 |
| UI-23 | A start page with recent projects, and import by dropping a file on the window. | P2 |
| UI-24 | When a selection set no longer matches its model, offer to select again, to show what is missing, and to keep the part that still matches. | P2 |
| UI-25 | View, add, edit and remove a setup's held regions and retaining tabs in the interface, with validation, the default tabs shown as defaults, and a warning when an operation's selection reaches one. | P1 |
| UI-26 | Number boxes accept simple arithmetic (`+ - * /` and brackets), worked out when the entry is confirmed, shown as its result rounded to the field's precision, with an entry that cannot be read reported under the box and on the step bar, and the last good value kept. | P2 |

### 3.3 Machine, stock and setup

| ID | Requirement | Priority |
|---|---|---|
| SET-01 | Define a machine profile: rotary axis orientation (about X), A-axis range and whether it rotates continuously, linear travel limits, and maximum feed rates. | P1 |
| SET-02 | Support a rotary axis about Y as well as X. | P2 |
| SET-03 | Define stock as a cylinder, a box, or the model bounding box plus an offset. | P1 |
| SET-04 | Use an imported model as custom stock (for example a casting or a pre-cut blank). | P2 |
| SET-05 | Position the part relative to the rotary centreline and work coordinate system, including a work offset reference (for example G54). | P1 |
| SET-06 | Define clearance geometry (a safe cylinder around the axis plus a safe Z) used by retracts and rapid links. | P1 |
| SET-07 | Multiple setups per project (for example re-gripping or flipping the part), each with its own coordinate system and starting stock. | P2 |
| SET-08 | Optionally define a maximum acceleration for the rotary axis in the machine profile. Where it is set, time estimates (SIM-08) and motion smoothing use it; where it is not, no limit is assumed. Maximum feed rates stay in SET-01. | P2 |
| SET-09 | Workholding: define, for each setup, how much of the stock the chuck or other workholding grips at the start and at the end of the stock along the axis (the gripped length, the chuck depth). The held region is a cylinder of the holding diameter when one is given, otherwise the stock's own section over the gripped length. The held region is never machined, strategies and links keep clear of it, and it stays in the simulated stock. | P1 |
| SET-10 | Manage machine profiles in the application: view, create, edit, duplicate, import and export as files, and delete library profiles; edit the project's copy and save it to the library. Built-in profiles are read-only and can be copied to edit. ([Decision 0380](decisions/0380-machine-library-in-the-application.md); text agreed by the owner, relayed by the coordinator, 31 Oct 2026.) | P1 |

### 3.4 Tool management

| ID | Requirement | Priority |
|---|---|---|
| TL-01 | A tool library stored separately from projects, with import and export in a human-readable format. | P1 |
| TL-02 | Tool types: flat end mill, ball end mill, bull-nose (corner radius) and tapered ball nose. | P1 |
| TL-03 | Further tool types: V-bit or chamfer, drill, and tools defined by a custom profile, which can be imported from a text file or drawn in the UI. | P1 |
| TL-04 | Tool parameters: diameter, flute length, overall length, shank diameter, flute count, corner radius, taper angle, material and tool number. | P1 |
| TL-05 | Cutting data per tool and stock material: spindle speed, feed per tooth, plunge feed, default stepover and stepdown. | P1 |
| TL-06 | Map each tool to controller values (tool number, length offset, diameter offset) used by the post-processor. | P1 |
| TL-07 | The project stores a snapshot of every tool it uses, so it opens correctly on a machine without the original library. | P1 |
| TL-08 | Define holder and collet geometry as part of a tool assembly (needed for collision checking in section 4). | P2 |
| TL-09 | A material library holding cutting data per material and tool, used to pre-fill the values in TL-05. | P2 |
| TL-10 | Import tool tables from other systems (for example LinuxCNC tool tables). | P3 |
| TL-11 | A default tool set per machine profile: the tools the machine normally holds and their tool numbers, kept with the tool library and used to number a tool when it is added to a project for that machine. Tool numbers belong to the project, not to the library tool. | P1 |
| TL-12 | A default tool library ships with the application: the reference tools for the Makera Z1, taken from Makera's published tool pages, with tool numbers assigned by the project as the machine's default tool set (TL-11), and a set of common standard tools of each type in TL-02 and TL-03. The user can edit, add and delete tools, and an update never overwrites a tool the user has changed. | P1 |
| TL-13 | The tool library supports undercutting tools, starting with thread mills: a profile that cuts below an overhang or in a bore, with its thread pitch and diameter, so operations can use it for thread milling and undercut features. The default tool library does not ship thread mills until this is built. | P2 |

### 3.5 Machining strategies and toolpath generation

Every strategy must produce toolpaths in XYZA. Geometry selected on the model (faces, regions, edge chains) defines where an operation cuts, and what surface drives it. With a rotary axis, the tool axis can only tilt in the plane perpendicular to the rotation axis, so strategies choose A to control orientation within that limit.

| ID | Requirement | Priority |
|---|---|---|
| OP-01 | An operation combines a strategy, tool, setup, geometry selection and parameters, and can be regenerated at any time. | P1 |
| OP-02 | Limit an operation to selected faces or regions, with an optional containment boundary and optional avoid or check surfaces. | P1 |
| OP-03 | Rotary roughing that is aware of remaining stock, in both indexed (3+1 positional) and continuous forms. | P1 |
| OP-04 | Baseline finishing: helical (spiral around the axis), axial (parallel to the axis, stepping in A) and indexed finishing at fixed A positions. | P1 |
| OP-05 | True simultaneous 4-axis finishing, where A varies continuously with X, Y and Z to follow the selected surface. | P1 |
| OP-06 | Tool-axis control: choose A to keep the tool as close to the surface normal as possible, within user-set lead and tilt limits. | P1 |
| OP-07 | Swarf (flank) machining of ruled or near-ruled selected surfaces using the side of the tool. | P1 |
| OP-08 | Edge and curve following: engraving or carving along selected edge chains or curves projected onto the surface. | P1 |
| OP-09 | Rest machining and pencil clean-up of material left by earlier operations. | P1 |
| OP-10 | Radial drilling at indexed A positions, with holes selectable from model geometry. | P1 |
| OP-11 | Common parameters: tolerance, stepover, stepdown, stock to leave (per operation and per selection), feeds, lead-in and lead-out, ramping, and retract strategy. | P1 |
| OP-12 | Gouge protection: paths never cut into the model beyond tolerance for the tool, and for the holder where it is defined. | P1 |
| OP-13 | Feed calculation for simultaneous motion so the feed at the cutter contact point is correct, using compensated feed rates in units per minute by default; inverse-time feed (G93) is offered only as an optional post-processor capability for controllers that support it. | P1 |
| OP-14 | Linking moves that retract and traverse using the clearance geometry, without crossing the stock or part. | P1 |
| OP-15 | A-axis management: respect range limits, minimise reversals, and handle wrap-around and unwinding. | P1 |
| OP-16 | Strategies implement a common internal interface, so new ones can be added without changing the rest of the application. | P1 |
| OP-17 | Adaptive (constant-engagement) roughing for rotary parts. | P2 |
| OP-18 | Feature recognition: detect holes, grooves and cylindrical features on the model and propose operations for them. | P3 |
| OP-19 | Alternative surface-covering patterns for finishing: geodesic-parallel (constant scallop height) and spiral, selectable per operation, with scallop height as the stepover control. | P2 |
| OP-20 | Simultaneous 4-axis finishing of surfaces that a ray from outside the part towards the rotary axis does not meet first, such as undercuts and the far walls of slots and pockets, wherever the tool can still reach them (for example by closest-point or offset-surface drive curves); surfaces the tool cannot reach are reported. Off-axis sections are covered by OP-26. | P2 |
| OP-21 | Retaining tabs: keep the machined part joined to the stock so it cannot drop or shift before the final cut-off, and so it can be removed from the stock easily. A tab is shaped as a cylinder or as a frustum (a cone with a flat end), set by its position along the axis, its diameter (both end diameters for a frustum) and, for tabs around the axis, how many there are and where. Operations leave the tab uncut, and the simulation keeps it. | P1 |
| OP-22 | Selection sets that are a separately defined bounding volume, a box or a cylinder the user defines and can edit later, which an operation takes where it takes any other selection set: as a boundary (the toolpath stays inside), an avoid volume (the tool stays out) or a check volume, following OP-02; where a strategy cannot honour a volume, generation says so. A volume never lets a path enter the held region (SET-09). | P2 |
| OP-23 | On a STEP (B-rep) part, every strategy that works on a mesh also works, takes face selections wherever it takes triangle selections on a mesh (as the region, boundary, avoid and check sets, and with a stock to leave per selection), and uses the part's exact faces and edges wherever that measurably improves the result; a strategy that keeps the tessellation for a STEP part says so in its notes, with the reason. | P1 |
| OP-24 | Extend past the stock, and facing: roughing and finishing strategies that cut along or around the part can carry their passes past the edges of the stock by a distance the user sets, at the ends of the stock (and, for a strategy whose passes run across, its sides) the user chooses, so the tool leaves the stock fully and leaves no skin or witness line. The option is off by default; the distance is measured from the edge of the stock to the near side of the tool and may be set from zero up to the tool's diameter. A Facing strategy cuts the stock down to a flat face at chosen A positions, or round to a chosen diameter, using the same extension. No extension enters the held region (SET-09), cuts a tab (OP-21), crosses the clearance geometry (SET-06) or goes beyond the machine's travel; where one would, generation says which and writes no path. Facing offers the end opposite exactly one defined held end, preserving tabs and refusing neither-held or both-held setups. The end target is the placed part's unheld end displaced outward by existing stock-to-leave; target coordinate and length to the opposite model end are displayed. Indexed box-side facing offers individual faces, All four and ordered arbitrary A angles, editable per-face target offsets and opposite-plane dimensions. It offers full stepdowns with a shortened final pass by default, or equal stepdowns within the selected maximum. Coverage is Whole stock by default, the projected part bounding rectangle, or the part-and-tabs union's projected bounding rectangle, with optional nonnegative margin defaulting to zero. Existing Round uses the corresponding full-circumference axial span and axial margin. Tabs and held regions remain protected under every extent. Necessary cutter overlap is previewed. Requested faces that cannot be reached completely with supported tool geometry, clearance and machine limits refuse without a usable partial path. Viewport and Setup show consistent extents, margin, targets, dimensions and cutter overlap. Saved default Facing behaviour is preserved relative to the merged rotary-boundary baseline. (decision 0252, accepted). | P1 |
| OP-25 | Cutting direction: an operation can ask for climb, conventional or meander (both) cutting. For each pass, the side of the tool the uncut material is on, the feed direction relative to the turning part, and the spindle direction decide which it is. Strategies that can honour the choice order and direct their passes to meet it; a strategy, or a part of a path, that cannot (the tool cuts on both sides, or on neither) says so in the operation and does not claim a direction. The spindle direction (`M3` or `M4`) and the cutting direction of each operation are shown in the operation inspector and on the setup sheet. Each strategy's default is the direction it cuts today, so existing projects and their G-code do not change. | P2 |
| OP-26 | Simultaneous 4-axis finishing drives a section of the part that does not surround the rotary axis (an off-axis feature such as a crank pin) round that section's own centre, so its whole outline is cut with the tool on the normal; a section in several separate pieces, and surfaces hidden from that centre, stay under OP-20. | P1 |

**OP-22 notes (P2, not scheduled; [decision 0188](decisions/0188-requirement-op-22-user-defined-bounding-volumes.md), issue 236).** OP-02's containment boundary is today the bounding window of a set's triangles (decision 0032, OI-137), so a user-defined volume is a new kind of set. It relates to the selection model (decisions 0032 and 0066), swarf boundary sets (decision 0176), selection sets in the inspector, the held region (SET-09; decisions 0160 and 0161) and the part-position question (the frame depends on which control positions the part). Results are deterministic (GC-07, NFR-06) and invalid input gives a clear error (NFR-07).
**Open points for the owner (OI-700):** the frame the volume is defined in (the part, the stock or the machine, and how it follows a part move); how it is defined (numbers, handles in the viewport, or from the stock or from selected faces); whether a cylinder is always about the rotary axis or may be placed freely, and whether a box is axis-aligned only; and how volumes combine with face sets and with each other.

### 3.6 Visualisation and material removal simulation

| ID | Requirement | Priority |
|---|---|---|
| SIM-01 | Show toolpaths in the 3D view, with rapid and cutting moves distinguished, and per-operation visibility. | P1 |
| SIM-02 | Show the tool at any point along the path, with the correct A rotation of the workpiece. | P1 |
| SIM-03 | Material removal simulation: the stock is updated as the tool sweeps through it, including rotation about the A axis. | P1 |
| SIM-04 | Playback controls: play, pause, step, speed, and jump to an operation or move. | P1 |
| SIM-05 | Compare the simulated stock with the target model and colour the result by remaining material and overcut; report the worst values. Facing additionally has an operation-local completion result; stock outside its extent is labelled Intentionally unfaced, while whole-part deviation and other operations’ misses remain available and counted, and all overcuts remain reported (0252). | P1 |
| SIM-06 | Carry the simulated stock from one operation to the next, so later operations see the material actually left. | P1 |
| SIM-07 | Adjustable simulation resolution, trading accuracy against speed. The default is a moderate resolution (0.1 mm); the user may choose a finer one at the cost of speed and memory, and a finer setting is never the default. | P1 |
| SIM-08 | Estimate machining time per operation and in total from feeds, rapids and tool changes. | P2 |
| SIM-09 | Load exported G-code into the simulator and run it, to check the post-processor output as well as the internal toolpath. | P3 |
| SIM-10 | Adjust feed rates using the tool engagement measured in the simulation, for faster cuts at a steady tool load. | P3 |
| SIM-11 | Read the deviation at any point of the simulated stock by pointing at it, with the operation and move that left it, and go to the worst overcut and the most material left. | P2 |
| SIM-12 | Show the posted G-code beside playback, following the current move. | P2 |
| SIM-13 | Summarise each generated operation: moves, cut length, A travel, A unwinds and programmed time. | P2 |
| SIM-14 | Show, for each operation, the largest overcut and the most material left after it, and the worst values after the current operation during playback. | P2 |
| SIM-15 | Choose which deviation bands the result views show, as the histogram's bins and as a range the user sets, so the part shows only the overcut or the material left of interest; the colour scale, the legend and the histogram follow the choice, and the worst values and counts stay those of the whole part. | P2 |

### 3.7 G-code generation and post-processing

| ID | Requirement | Priority |
|---|---|---|
| GC-01 | Toolpaths are held in a neutral intermediate form, independent of any controller. | P1 |
| GC-02 | A post-processor interface, with several post-processors available and one selected per machine profile. | P1 |
| GC-03 | Post-processors are defined as text templates that the user can edit or add without recompiling the application. | P1 |
| GC-04 | Post options: units, A-axis letter, direction and wrap behaviour, header and footer, tool change sequence, coolant, work offsets, line numbering, decimal precision, maximum feed, and feed mode for rotary moves. These machine-specific conventions live in the post-processor definition, not in the engine. | P1 |
| GC-05 | Output as one file for the whole project, or as one file per setup, operation or tool. | P1 |
| GC-06 | Comments in the output naming the operation and tool, and the estimated time once SIM-08 is available. | P1 |
| GC-07 | Deterministic output: the same project, post-processor and software version always give byte-identical G-code. The software version is an input: the generator comment `(OpenChuckCAM <version>)` is the only text that changes with it. | P1 |
| GC-08 | Check generated G-code for axis limit violations, missing feeds and invalid numbers before export. | P2 |
| GC-09 | A post-processor for the Makera Z1, the reference machine for the first release; existing Makera machine profiles are inspected during development to derive its rotary-axis conventions. | P1 |
| GC-10 | A verification post-processor producing LinuxCNC-dialect G-code, to be run in the LinuxCNC simulator to confirm the toolpaths and the post-processing. | P1 |
| GC-11 | A command-line mode that regenerates G-code from a project file without the UI. | P1 |
| GC-12 | Export a setup sheet (tools, operations, stock and estimated times) as HTML or PDF; times appear once SIM-08 is available. The sheet also records when it was generated and when the project was last saved, the software version and a fingerprint of the project, and a version-control line that is filled in when version control is available, so a printed sheet can be traced to the files it came from. | P1 |
| GC-13 | An embedded scripting language for post-processors, for logic that text templates cannot express. | P2 |
| GC-14 | Before G-code is written, show the tool numbers the program will use and let the user confirm them, renumber automatically in operation order, or renumber by hand. The chosen numbers are saved in the project, so the next post gives the same output. | P1 |
| GC-15 | Refuse to write G-code while an enabled operation is stale or in error, and warn when an operation has not been simulated, listing the checks before writing. Desktop export lists existing full destinations and asks before replacing them; Cancel writes nothing. Revalidate readiness and destinations before writing; stop at the first failure and report partial success accurately. | P1 |
| GC-16 | Before the first export of a project on a computer, the user confirms that they will verify the G-code before running it. The confirmation is kept on that computer, not in the project, so another computer asks again; the notice itself is shown at every export. The command-line tool runs unattended: it prints the notice at every run and asks for no confirmation. | P1 |
| GC-17 | A machine profile can set a minimum average block time in milliseconds and a block window, with optional post overrides; zero or Off disables the check. Report gives per-operation short-move counts, runs, worst windows, effective feed and predicted added time without changing G-code. Slow down lengthens move durations only where needed to meet the window target, preserves programmed geometry, and reports the added time. Settings and their source are shown before export and on the setup sheet. Merge remains deferred; if later added, it must use a stated tolerance and the independent model checker. | P1 |

### 3.8 Project files

| ID | Requirement | Priority |
|---|---|---|
| PRJ-01 | Projects are saved in a text-based format with stable ordering and formatting, so changes show cleanly in a line-based diff. | P1 |
| PRJ-02 | Large model files stay as separate files referenced by relative path, with a content hash recorded so a changed model is detected on load. | P1 |
| PRJ-03 | Generated data (toolpaths, simulation caches, G-code) is not stored in the project file; it is regenerated, and any cache is kept somewhere easy to ignore. | P1 |
| PRJ-04 | Every project file records its schema version, and older versions are migrated on load. | P1 |
| PRJ-05 | Operations, tools, setups and selection sets have stable unique IDs, and their order is stored explicitly, so two branches that edit different items merge cleanly. | P1 |
| PRJ-06 | Geometry selections are stored in a form that survives save and load of the same model. | P1 |
| PRJ-07 | Selections also store descriptors (position, normal, area) so they can be matched again after the model is re-exported. | P2 |
| PRJ-08 | Autosave and crash recovery. | P2 |

**PRJ-08 scope (owner's CHECKPOINT answers, 24–25 Oct 2026; implementation design in accepted [0243](decisions/0243-project-recovery-checkpoints.md)).** Capture committed project state only, excluding unconfirmed fields, generated data and running-generation state. Retain three periodic checkpoints and one independently replaced interim checkpoint in an ignored recovery directory inside the project. The per-user periodic interval defaults to three minutes, configurable from one to fifteen whole minutes; confirmed edits also trigger the interim checkpoint after fifteen seconds of inactivity. After unexpected termination, present the retained valid choices with their project, timestamp and type. Recovery changes unsaved memory only; explicit Save changes the saved project. Save and clean-exit retirement must preserve newer states. Keep saved and explicitly discard recovery data are separate choices. Corrupt/partial data gives a clear error and preserves other recovery choices.

## 4. Secondary requirements

These follow the core release. The core architecture should leave room for them: collision checking needs the tool assembly (TL-08) and machine geometry, in-app version control builds on the project file rules in section 3.8, and library integration builds on the separation of engine and UI in NFR-01.

### 4.1 Collision detection in simulation

| ID | Requirement | Priority |
|---|---|---|
| COL-01 | Detect collisions between the cutting part of the tool and the model or stock during non-cutting moves (rapids and links). | P2 |
| COL-02 | Detect collisions between the tool shank, collet and holder and the model, stock and workholding. | P2 |
| COL-03 | Detect collisions between the machine head or spindle and the part, stock, workholding and rotary hardware (chuck, tailstock). | P2 |
| COL-04 | Define machine head and fixture geometry in the machine profile, as simplified imported solids. | P2 |
| COL-05 | Test collisions against the in-process stock as simulated, not only the final model. | P2 |
| COL-06 | A user-set safety margin for collision checks. | P2 |
| COL-07 | List collisions with the operation, move and location, highlight them in the viewport, and jump to each from the list. | P2 |
| COL-08 | Automatically avoid collisions in linking moves and retracts, using the checks in COL-01 to COL-05. | P3 |

### 4.2 Version control within the software

| ID | Requirement | Priority |
|---|---|---|
| VC-01 | Save named revisions of a project, each with a timestamp and a message. | P2 |
| VC-02 | Browse revision history and restore any earlier revision. | P2 |
| VC-03 | Compare two revisions as a structured difference of operations, tools, setups and parameters, not only as text. | P2 |
| VC-04 | Keep alternative approaches (for example two roughing strategies) as branches and merge them back. | P2 |
| VC-05 | Build the revision system on an embedded Git library, so the project folder is a standard Git repository that external Git tools can read; using the built-in system must never stop a project folder being committed to Git. The chosen library (git2) is evaluated in section 4.3. | P2 |
| VC-06 | Write the revision identifier into the G-code header, so output can be traced to the project state that produced it. | P2 |

### 4.3 Embedded Git library evaluation

Decision: use libgit2 through the git2 crate, and keep revision handling behind a small internal interface so gix can replace it later. In-app revisions need only local operations (commit, history, diff, restore, branch, merge), not network access.

| Option | Benefits | Risks |
|---|---|---|
| [git2](https://github.com/rust-lang/git2-rs) (Rust bindings to libgit2) | Maintained under the rust-lang GitHub organisation. Local repositories work by default, with network support optional. The libgit2 source is bundled and built automatically. The crate is MIT or Apache-2.0, and libgit2 is GPLv2 with a linking exception, which suits a GPL-compatible project ([libgit2 licence](https://github.com/serenial/libgit2)). | A C library to build on all three platforms, which you have accepted. libgit2 is an independent implementation of Git, so edge-case behaviour can differ from the git command. It tracks libgit2 releases (currently 1.9.6 or newer). |
| [gix](https://github.com/GitoxideLabs/gitoxide) (gitoxide, pure Rust) | No C toolchain. Supports commit, status, diffing, fetch and low-level editing of objects and the index. | Its [status page](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md) lists checkout, restore, reset and merge workflows as still to do, and push as unimplemented. Restoring a revision and merging branches (VC-02, VC-04) need those, so more of that code would be ours to write. |

Both options share one risk: STEP and STL files are binary, so every changed model version adds to the repository size. PRJ-02 keeps models as separate files, which leaves room to handle them differently later. Before committing to a library, prototype commit, restore and merge on a real project folder.

### 4.4 Pathing library for use by other applications

A secondary goal is that the toolpath engine can be used as a library by other projects such as FreeCAD, not only through this application's UI. It builds on NFR-01.

| ID | Requirement | Priority |
|---|---|---|
| INT-01 | The engine is a standalone library with a documented public API, and no dependency on this application's UI or project file. | P2 |
| INT-02 | The API takes geometry (meshes, and B-rep faces and edges), stock, tools, machine profile and operation parameters as plain data, and returns toolpaths in the neutral intermediate form (GC-01). | P2 |
| INT-03 | Post-processing can be used on its own, so a host can turn its own toolpaths into G-code with the templates from GC-03. | P2 |
| INT-04 | Progress reporting and cancellation are part of the API, so a host can run generation in the background (see UI-09). | P2 |
| INT-05 | Python bindings, so the library can be called from FreeCAD's scripting environment. | P2 |
| INT-06 | A C-compatible interface for hosts that cannot use the Rust API or Python bindings directly. | P3 |
| INT-07 | A versioned public API, with breaking changes recorded in a changelog. | P2 |
| INT-08 | Documentation and a small example host application showing how to call the library. | P2 |
| INT-09 | The library is AGPL like the rest of the project; if FreeCAD needs it, add a licence exception or a dual licence. | P2 |

## 5. Non-functional requirements and constraints

Technology choices (crates, geometry kernel, rendering stack) are made in the architecture document, not here; this section states only the constraints that bind them, and the git2 decision in section 4.3 is the one exception. The performance figures are confirmed as a starting point and can be revised once there is something to measure.

| ID | Requirement | Priority |
|---|---|---|
| NFR-01 | Written in Rust, with the engine (import, toolpaths, simulation, post-processing) in a library separate from the UI, so it can run headless and be tested without a window. | P1 |
| NFR-02 | Runs on Windows, macOS and Linux, with 3D rendering through each platform's native graphics support. | P1 |
| NFR-03 | Orbit, pan and zoom stay smooth (target 30 frames per second or better) on models of up to 1 million triangles. | P1 |
| NFR-04 | Toolpath generation and simulation use multiple CPU cores, and never block the UI. | P1 |
| NFR-05 | Geometry and toolpaths are computed in double precision, with a default toolpath tolerance of 0.01 mm that the user can change. | P1 |
| NFR-06 | Repeatable results: the same project and software version produce identical toolpaths and G-code, regardless of thread count. | P1 |
| NFR-07 | Invalid or damaged input files produce a clear error message, never a crash. | P1 |
| NFR-08 | Automated tests include golden-file tests for G-code and regression tests on known geometry with known expected toolpaths. | P1 |
| NFR-09 | Works fully offline, with no account, licence server or telemetry. | P1 |
| NFR-10 | The project is open source under the AGPL, covering every crate including the pathing library; third-party dependency licences are recorded and must be compatible with it. | P1 |
| NFR-11 | User documentation, including a guide to writing a post-processor. | P2 |
| NFR-12 | The software states clearly that generated G-code must be verified by the user before it is run on a machine. | P1 |
| NFR-13 | Bindings to C++ libraries, for example for STEP import, are acceptable where they are the best-maintained option, provided they build on Windows, macOS and Linux. | P1 |
| NFR-14 | Choose dependencies for long-term health: well featured, actively maintained, thoroughly documented, tested with CI, and used by several mature open-source projects. Newer crates, and crates with a high proportion of AI-generated contribution, count as a risk that must be justified. | P1 |
| NFR-15 | The interface is legible and accessible: text at least 12 pixels at 100% scale, scaling from 80% to 200%, contrast of 4.5:1 for text and 3:1 for meaningful graphics, no state shown by colour alone, and colour maps that differ in lightness. | P1 |
| NFR-16 | The About window and the command-line `--version` report which build is running: the version-control commit, whether the build tree had uncommitted changes, the build date and time (UTC), the Rust compiler version, the target triple and the version of each crate in the build. A fact that cannot be determined (for example, no Git directory in a source archive) reads "unknown"; building needs no network. The About window also shows the licence text in a scrollable box and a Copy action that puts the build information, and only that, on the clipboard. The generated G-code names the tool and its version in a comment (for example `(OpenChuckCAM 0.11.0)`), the workspace version only. No other build information (commit, date, uncommitted-changes flag) is written into G-code, golden files or project files, and the library family does not contain it (GC-07, NFR-06). | P1 |
| NFR-17 | The desktop application opens no console window when started from the desktop or a shortcut. It writes a local diagnostic log (start-up, graphics adapter, warnings, errors and panics) to a folder the user can write to, size-limited by rotation, and never sends it anywhere. Output the user asked for on the command line (--bench, --help) still reaches the terminal it was started from. | P1 |
| NFR-18 | The application records the user's actions (menu commands, buttons, dialogs opened and closed, edits and undo, and the jobs started) as short entries in the diagnostic log, so a bug report can show the steps taken. An entry names the action and the kind and name of the item acted on; it contains no file contents. The log stays on the computer (NFR-09). | P2 |

## 6. Suggested additional features for decision

These are not in your brief. Each has a proposed priority; your decisions are in the last column. Items marked Include are now requirements in sections 3 to 5, and the two marked Defer stay here for later.

| ID | Suggestion and benefit | Proposed | Decision |
|---|---|---|---|
| SUG-01 | Simulate the exported G-code, not only the internal toolpath. Catches post-processor errors before the machine does. | P3 | Include |
| SUG-02 | Adaptive (constant-engagement) roughing for rotary parts. Shorter cycle times and lower tool load in hard materials. | P2 | Include |
| SUG-03 | Command-line mode that regenerates G-code from a project file. Gives repeatable builds and automated tests. | P2 | Include |
| SUG-04 | Setup sheet export (tools, operations, stock, estimated times) as HTML or PDF. A printable job summary for the operator. | P1 | Include |
| SUG-05 | Material library holding cutting data per material and tool. Pre-fills feeds and speeds. | P2 | Include |
| SUG-06 | Curve import (DXF, SVG) for engraving and carving along curves. Extends OP-08 beyond model edges. | P2 | Include |
| SUG-07 | Automatic collision avoidance in linking moves and retracts, building on section 4.1. Less manual fixing. | P3 | Include |
| SUG-08 | Feature recognition: detect holes, grooves and cylindrical features and propose operations. Faster programming. | P3 | Include |
| SUG-09 | Feed optimisation using tool engagement data from the simulation. Faster cuts at a steady tool load. | P3 | Include |
| SUG-10 | Extra model formats (OBJ, 3MF, IGES). Wider compatibility with other tools. | P3 | Defer |
| SUG-11 | Scripting or plugin interface for custom strategies and post-processors. Lets users extend the software without forking it. | P3 | Defer |
| SUG-12 | Import of existing tool tables from other systems (for example LinuxCNC). Saves re-entering tools. | P3 | Include |

## 7. Decisions, assumptions and open questions

Decisions are settled and already reflected in the requirements. The assumptions still need confirming, and no open questions remain.

**Decisions**

- Reference machine is the Makera Z1. The first release needs a Z1 post-processor and a verification post-processor that targets the LinuxCNC simulator (GC-09, GC-10).
- Rotary axis: X only for the first release; Y stays P2 (SET-02).
- STEP import may wrap a C++ library such as OpenCASCADE (NFR-13).
- Post-processors start as text templates, with scripting later (GC-03, GC-13).
- Machine-specific rotary conventions (axis letter, units, wrap-around, feed mode) live in the post-processor definition, informed by existing Makera machine profiles during development (GC-04, GC-09).
- The project is open source under the AGPL, covering every crate including the pathing library. A licence exception or dual licence for FreeCAD can be added later if needed (NFR-10, INT-09).
- In-app version control uses the git2 crate (section 4.3).
- The performance targets in NFR-03 are accepted as a starting point.

**Assumptions**

- The A axis rotates the workpiece about the machine X axis, with a vertical tool axis (Z).
- The target is a milling machine with a rotary axis programmed in degrees; the Makera Z1 is the reference machine, and G93 inverse-time feed is unsupported (confirmed by the SP-12 air cut, decision 0007).
- The internal unit is millimetres; inch display and input are a user setting (UI-18, P2; decision 0038).
- Work is for a single user on a single machine at a time; there is no multi-user collaboration.
- Threading, turning and 5-axis tool orientation are not needed.

