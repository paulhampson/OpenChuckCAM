# Operations and strategies

An operation is one cutting job: a strategy, a tool, a setup, the surfaces to work on, and the strategy's settings. This page lists every strategy, every setting with its default, and the warnings and errors an operation can give. [Roughing and finishing](../concepts/roughing-and-finishing.md) explains the ideas behind them.

> [!WARNING]
> Every default on this page is a starting value, not a cutting recipe tested on a machine. Generated G-code must be verified by you before it is run on a machine; see [Checking G-code before you run it](../safety/checking-gcode.md).

## The strategies

| Group | Strategy | What it does | Needs |
| --- | --- | --- | --- |
| Roughing | [Rotary roughing](#rotary-roughing) | Removes most of the stock in layers, from the outside in. | |
| Roughing | [Facing](#facing) | Flat faces, a round diameter, the free end, or box sides on the stock. | |
| Finishing | [Helical finishing](#helical-finishing) | Follows the surface in a spiral round the part. | |
| Finishing | [Axial finishing](#axial-finishing) | Passes along the part, turning it a step between passes. | |
| Finishing | [Indexed finishing](#indexed-finishing) | Passes along the part at a few fixed angles. | |
| Finishing | [Simultaneous 4-axis finishing](#simultaneous-4-axis-finishing) | Turns the part while cutting, to keep the tool near the surface normal. | |
| Finishing | [Rest machining](#rest-machining) | Cuts the material earlier operations left. | |
| Finishing | [Pencil clean-up](#pencil-clean-up) | Follows sharp inside corners. | |
| Finishing | [Face flow finishing](#face-flow-finishing) | Passes that follow a chosen edge across selected faces. | a STEP model |
| Specialist | [Edge and curve following](#edge-and-curve-following) | Engraves or carves along edges. | |
| Specialist | [Swarf (flank) machining](#swarf-flank-machining) | Cuts walls with the side of the tool. | a STEP model |
| Specialist | [Radial drilling](#radial-drilling) | Drills round holes in the side of the part. | a STEP model |

## Adding an operation

On the Operations step, open **Add operation**. Give the operation a **Name**, choose a **Strategy**, the **Setup** and a **Tool** (**From the library...** adds one to the project), and a **Material preset**. Everything else starts at the strategy's defaults.

A strategy that needs a STEP model is shown greyed out when the setup's part is a mesh (an STL file), with the reason under it. Facing, Rest machining, Pencil clean-up, Edge and curve following and Radial drilling are listed with "ignores selections": they work on the whole part, and selection sets do not limit them.

The **Material preset** fills the spindle speed, feed per tooth, plunge feed, stepover and stepdown from the tool's cutting data for one stock material. It starts at the setup's stock material. Changing one of those values drops the preset. Check the values against your tool, material and machine.

## The operation inspector

Select an operation to show its settings in the Inspector, from the top:

- the name, **Delete**, and the operation's state;
- **Strategy**: the strategy's name and description, which cannot be changed here;
- a line such as "2 values are missing", with **Show first**, when a required value is empty;
- a red banner when the operation cannot be generated: export is blocked while an operation is in error, and turning it off lets the others export;
- **Geometry**: the selection sets the operation uses (below);
- the **Tool** section: the tool, its **Tool number**, **Material preset** and **Spindle speed**, and **Update from library**, **Edit tool...** and **Save to library...**;
- the settings, in the sections **Tool and feeds**, **Pattern and tolerance**, **Tool axis and A**, **Entry, links and retract** and **Advanced**;
- the operation's notes.

Each setting's label has a tooltip that says what it does. An empty field shows the value that applies and where it comes from: "default", "from the tool's cutting data", or "required". The ↺ button returns a field to that value. A value outside the setting's range is refused with a message such as "Ramp angle: above 0, at most 89. Not applied." A setting that the last path warned about is marked; change it to remove the warning.

Choices are shown as the program stores them, for example `clearance` and `safe_z` for **Retract**. A few have names: **Shape**, **Coverage extent**, **Layer distribution**, **Passes** and **Links between passes**.

## Settings every operation has

| Setting | Default | What it does |
| --- | --- | --- |
| Tolerance | 0.01 mm | How far the toolpath may be from the ideal surface. |
| Stepover | from the tool's cutting data, else required | The distance between neighbouring passes, measured as the strategy states. |
| Stock to leave | 0 mm | Material left on the surfaces for a later operation. A new rotary roughing operation starts at a tenth of the tool's diameter, rounded to 0.05 mm and kept between 0.1 and 1 mm. |
| Cutting feed | from the tool's cutting data, else required | The [feed](../glossary.md#feed) at the cutter's contact point while cutting, in mm/min. |
| Plunge feed | from the tool's cutting data, else required | The feed of the tool tip moving into the material. |
| Retract | `clearance` | Where moves between passes go: up to the safe cylinder (`clearance`), or to the safe Z (`safe_z`). |
| Approach distance | 1 mm | A rapid approach stops this far above the stock; the rest is a plunge at the plunge feed. |
| Entry | `plunge` | How the tool enters the cut: a straight `plunge`, a tangent arc (`lead`), a `ramp` that follows the pass's own path down to its first cut point, or a descending `helix`. With `lead`, a matching arc also leaves the cut at the end of the pass. |
| Allow single-point plunge | off | A pass that is a single point (a roughing layer that only touches the part's tip, for example) has no path for a ramp to follow. With this on, the tool plunges straight down to that point instead of refusing the operation. The tool must be able to plunge (centre cutting). Only for ramp entry. |
| Steeper ramp near the axis | off | Near the rotary axis a ramp round a ring needs many turns. With this on, a ring or spiral ramp that would need more than 10 turns at the Ramp angle uses a steeper angle, just enough to fit in 10 turns, never steeper than the Maximum ramp angle. Lines are not affected. Only for ramp entry. |
| Maximum ramp angle | 15° | The steepest angle the steeper ramp near the axis may use, at most 45°. |
| Plunge to remove face nubs | off | Roughing only. Where roughing in rings has narrowed the stock round the rotary axis to a nub no wider than the Face nub diameter, and the part leaves nothing inside it, the tool plunges at the axis while the part turns once, to remove the nub, instead of ramping round ever smaller rings. The tool must be able to plunge (centre cutting), and it cuts with its full width. Only for ramp entry. |
| Face nub diameter | 0 mm | The widest nub that is plunged; 0 uses the tool's cutting diameter, and a larger value is held to it. |
| Back-and-forth ramp | off | No longer has an effect: shown only where an operation saved by an earlier version has it on. Every ramp now follows the pass's own path. |
| Ramp angle | 3° | The angle of descent for a ramp or helix entry, above 0 and at most 89°. |
| Helix diameter | 0.75 | The diameter of the helix entry, as a fraction of the tool's diameter. |
| Lead radius | 0 mm | The radius of the lead arcs; 0 uses the tool's radius. |

**A flat free end.** Where the part ends in a flat face square to the rotary axis with stock beyond it, a Facing operation in End mode before roughing removes the stock past the end across Y, so roughing leaves no nub round the axis there.

The entry defaults are starting values that have not been checked on a machine. A tool that is not centre-cutting cannot plunge: choose ramp or helix entry, or say on the tool that it can plunge.

**How a ramp follows the pass.** A ramp starts the approach distance above the highest stock under the stretch of the pass it uses, and descends at no more than the ramp angle to the pass's first cut point, where the pass is cut unchanged. On a pass that starts along a line it goes back and forth over the start of the line, ending at the first cut point. On a ring it goes round the ring before its first point, the part turning the same way as the pass; on a spiral it goes round the part at the spiral's starting X. Because it stays over the pass's own path, it keeps clear of a held region, tabs and the next ring as the pass does. It enters the first line or ring with the full width of the tool, as the pass cuts it. Near the rotary axis a ring ramp can take several turns; more than 10 is refused, and Plunge or Helix is then the way in. Check the program before running it on a machine.

When a required value is empty and no cutting data fills it, the operation cannot be generated, and the message says why: the setup has no stock material, the tool has no cutting data for that material, or the operation has no tool.

## Geometry: the surfaces an operation works on

Selection sets are named groups of faces, triangles or edges, made in **Selection sets**. The Geometry section puts them in four roles. With nothing in **Cut on**, the operation cuts the whole part.

Cut on
: The operation cuts only on these surfaces.

Contain within
: The tool may cut only inside the window of X and angle that this set spans.

Avoid
: These surfaces are never cut; the tool may pass over them.

Check
: The tool must not touch these surfaces.

For each set in **Avoid** or **Check**, the strategies that take selections (rotary roughing, helical, axial, indexed, simultaneous and face flow finishing, and swarf) offer three choices:

Cut short
: Where the tool would break the set, the pass stops short of it and starts again after it. In rotary roughing, on a Check set, the tool is instead lifted to stay the stock to leave above the set, and the material over it is still roughed. In swarf, the tool is first lifted up the face it cuts by the least that keeps it off the set, leaving a strip of that width uncut.

Warn only
: The path is kept where the tool would break the set: the tool is not kept clear of it. A warning says how far along the path it breaks the set. On a **Check** set it is a warning to acknowledge, since a check set often stands for a clamp. It samples the path and can miss a narrow set, so no warning does not show that the tool is clear.

Ignore
: The set limits nothing, and the operation warns that it limits nothing.

Swarf also has two choices for a set in **Contain within**: **Limits the pass** (cut only the rulings that lie wholly inside the boundary) and **Limits the tip** (raise the tool tip so the bottom of the flank stops where a ruling enters the boundary).

The Geometry section also warns when a set cannot be used by this strategy on this part, or when a set reaches a [held region](../glossary.md#held-region) or a tab, with **Show in view** and **Edit workholding**. On a mesh part, a strategy that ignores selections cannot be generated while it has selection sets: take them out of the operation.

## Settings some strategies share

Extend past the stock
: `no`, `both ends`, `start end` or `far end`. Carries the passes past the chosen ends of the stock, so the tool leaves the stock completely and leaves no thin skin at its edge. The start end is the low X end. An end held by the chuck cannot be extended past. Rotary roughing, facing, and helical, axial and indexed finishing.

Distance past the stock
: How far past the edge of the stock the near side of the tool goes, from 0 up to the tool's diameter. It starts at a tenth of the tool's diameter, rounded to 0.05 mm, at least 0.1 mm. A distance that takes the path beyond the machine's travel is refused.

Adaptive spacing
: On by default. Spaces the passes by the material they leave: the stepover is the widest spacing, and passes are added where more than the **Allowed excess** (default 0.01 mm) would stand between them. They are never closer than **Smallest spacing** (default 0.01 mm). Rotary roughing when its passes run along the axis, flat facing, helical, axial and indexed finishing, and simultaneous finishing with the axial pattern. See [adaptive spacing](../glossary.md#adaptive-spacing) and [Partial paths](#a-partial-path-must-be-acknowledged).

Scallop height
: On helical and axial finishing with **Adaptive spacing** off: the largest cusp between passes for a ball-end or bull-nose tool. The passes are as close as the smaller of this and the stepover needs. 0 turns it off. With **Adaptive spacing** on, the same field is **Allowed excess**.

Exact STEP surface contact
: Only on a STEP part. Drops the tool on the part's exact surfaces instead of the triangles made from them: more accurate, and slower. On by default for helical, indexed and simultaneous finishing and rest machining; off for axial finishing and pencil clean-up, where it is much slower. Some tool shapes always use the triangles, and a line under the field says so.

Links between passes
: **Retract to the clearance**, or **Stay low where clear**, the default for new operations: links stay the approach distance above the stock where the whole link is clear of the stock, the held regions and the part. The stock is checked at points along each link, so a very thin piece of stock between them could be missed. A link that cannot stay low retracts, and a note says how many did. Facing (end and box shapes), helical, axial, indexed and simultaneous finishing, and rest machining.

Pass order
: `by_region`, the default for new operations, cuts the turns at end faces and the passes on hidden surfaces next to the passes of their region; `after_main` cuts them after all the main passes. The passes are the same either way. Helical and simultaneous finishing, and rest machining.

Face free ends
: On by default. At each end face of the part with nothing beyond it, the tool steps down beside the face by the **Stepdown**, so the face keeps only the stock to leave. Helical and axial finishing, and rest machining. See [A free end deeper than the flutes](#a-free-end-deeper-than-the-flutes).

Finishing layers, Finish allowance, Stepdown
: **Finishing layers** (off by default) cuts in layers the **Stepdown** apart along the tool axis, the last on the finished surface. The layers stop **Finish allowance** (default 0.1 mm, less than the stepdown) above the surface, and the final pass takes the rest. **Stepdown** comes from the tool's cutting data when it is not set, and is required with finishing layers on. Helical, axial and indexed finishing.

## Rotary roughing

Removes most of the material from the stock in layers, from the outside in. The part can turn continuously or be held at chosen angles. Use it before finishing, with a stock to leave for the finishing tool.

| Setting | Default | What it does |
| --- | --- | --- |
| Form | `continuous` | `continuous` layers turn A as the tool cuts; `indexed` layers hold A at fixed positions (3+1). |
| Cut direction | `usual for the form` | `along the length of the part` runs passes along X; `around the part` wraps them round the part. The usual is around for continuous, along for indexed. |
| Stepdown | from the tool's cutting data, else required | The depth of each layer. Layers are made equal. It cannot be more than the tool's flute length. |
| Index angle | 30° | Indexed form: the angle between positions of A. |
| Start angle | 0° | A at the first index position, or at the start of the helix. |
| Largest angle step | 5° | Continuous form: the most A may turn between two points of a layer. |
| Smallest cut | 0.15 mm | Material standing less than this above the tool is treated as air. Keep it at least the simulation's cell size (0.1 mm by default). |
| Passes | Spiral | Continuous form around the part: **Spiral**, one continuous spiral per layer, or **Rings**, one full turn at a time. |

It also has **Extend past the stock**, and **Adaptive spacing** when its passes run along the axis.

## Facing

Prepares the stock: flat faces along it, a round diameter, the end opposite the held end, or chosen box sides. It ignores selections, and keeps off the held regions and tabs.

| Setting | Default | What it does |
| --- | --- | --- |
| Shape | Flat along stock | **Flat along stock**: a flat face at each angle listed. **Round**: the stock turned down to a cylinder. **Unheld end**: the end opposite the only held end. **Box sides**: chosen standard sides and extra faces at any angle. |
| Finished size | required | Flat: how far the face is from the rotary axis. Round: the diameter to turn down to. |
| Angles | 0 | Flat and round: the A positions to face at, in degrees, separated by commas, in cutting order. |
| Coverage extent | Whole stock | **Whole stock**, **Part only** (the part's outline), or **Part plus tabs**. |
| Extent margin | 0 mm | Extra coverage beyond each edge of the coverage. For round it is **Axial margin**, at both ends. |
| Layer distribution | Full stepdown, shorter final pass | Box sides: full layers with a shorter last one, or **Equal stepdowns**. |
| Face +Z (A 0°), +Y (A 90°), −Z (A 180°), −Y (A 270°) | off | Box sides: the standard sides to face; **All four sides** ticks them all. For each, **part plus allowance** (on) sets the face from the part and its stock to leave; clear it to type a **plane offset**. |
| Arbitrary A faces | none | Box sides: more faces at any A, each with its offset, in your order. |
| Stepdown | from the tool's cutting data, else required | The depth of each layer. For the unheld end it is **Axial stepdown**. |
| Largest angle step, Start angle | 5°, 0° | As for rotary roughing. |
| Smallest cut | 0.15 mm | Flat and round: as for rotary roughing. |
| Passes | Spiral | Round: **Spiral** or **Rings**. |

For the unheld end, **Stepover** is labelled **Flute band spacing**. Facing also has **Extend past the stock** with **Also past the sides**, and flat facing has **Adaptive spacing**.

## Helical finishing

Follows the surface in a spiral round the part as the tool moves along it: a general finish for round shapes. Selections and gaps can split the spiral, and the tool may not reach tight inside corners.

| Setting | Default | What it does |
| --- | --- | --- |
| Rotation | `positive` | Which way A turns as X increases. |
| Start angle | 0° | A at the lowest X of the part. |
| Largest angle step | 5° | The most A may turn between two points; points are added where the surface needs them. |
| Scallop height / Allowed excess | 0 / 0.01 mm | See [the shared settings](#settings-some-strategies-share). |
| Passes | Spiral | **Spiral** or **Rings** (one full turn at a time, at the start angle). |

Also: **Exact STEP surface contact** (on), **Extend past the stock**, **Face free ends**, **Finishing layers**, **Adaptive spacing**, **Pass order** and **Links between passes**.

## Axial finishing

Passes along the length of the part, turning it a small step between passes. Use it where marks along the part suit the job better than a spiral.

| Setting | Default | What it does |
| --- | --- | --- |
| Direction | `zigzag` | `zigzag` alternates the direction of each pass; `climb` cuts every pass the same way. |
| Start angle | 0° | A of the first pass. |
| Scallop height / Allowed excess | 0 / 0.01 mm | See [the shared settings](#settings-some-strategies-share). |
| Largest step along X | 1 mm | The most X may advance between two points of a pass. |

Also: **Exact STEP surface contact** (off), **Links between passes**, **Extend past the stock**, **Face free ends**, **Finishing layers** and **Adaptive spacing**.

## Indexed finishing

Turns the part to each chosen angle, holds it still, and follows the surface along its length. It does not turn the part during a pass.

| Setting | Default | What it does |
| --- | --- | --- |
| Angles | empty | The A positions to cut at, in degrees, separated by commas, in cutting order (up to 720). Empty uses **Count**. |
| Count | 4 | How many equally spaced positions to cut at, from 1 to 720. |
| Start angle | 0° | A of the first equally spaced position. |
| Direction | `zigzag` | As for axial finishing. |
| Largest step along X | 1 mm | As for axial finishing. |

Also: **Exact STEP surface contact** (on), **Links between passes**, **Extend past the stock**, **Finishing layers** and **Adaptive spacing**. It has no Scallop height and no Face free ends.

## Simultaneous 4-axis finishing

Turns the part while the tool moves, so the tool meets the surface from different directions and stays close to the surface normal. Four axes cannot follow every surface direction; where it cannot follow the part it says so instead of cutting it badly. Surfaces a tool cannot reach from outside, such as the walls of a slot, get passes of their own, and a section of the part that is off the rotary axis, such as a crank pin, is driven round its own centre.

| Setting | Default | What it does |
| --- | --- | --- |
| Pattern | `spiral` | `spiral`: one helix round the axis, the stepover apart per turn. `axial`: passes along the axis, the stepover apart at the part's largest radius. |
| Rotation | `positive` | Which way A turns as X increases along a spiral. |
| Start angle | 0° | A at the start of the path. |
| Largest angle step | 5° | The most the path turns round the axis between two points. |
| Lead angle | 0° | From −30° to 30°. Leans the tool from the normal towards the direction of travel round the axis, for example to keep the centre of a ball, where it barely cuts, off the surface. Axial passes need 0. |
| Largest tilt | 10° | From 0° to 30°. How far the tool may lean away from the normal so that A turns smoothly. A larger value cuts with more of the tool's side. |
| Largest lean on hidden surfaces | 90° | From 0° to 90°. How far the tool may lean on surfaces a straight approach cannot see. Anything that needs more is left uncut and listed in a warning. |
| Passes | Spiral | Spiral pattern: **Spiral** or **Rings**. |

Also: **Exact STEP surface contact** (on), **Pass order** and **Links between passes**, and with the axial pattern **Adaptive spacing**. It has no Extend past the stock.

## Rest machining

Uses the stock left by earlier operations in the same setup to find and cut what remains, for example with a smaller tool in inside corners. The remaining material comes from the stock simulation. It ignores selections. If no earlier operation in the setup has cut, the setup's whole stock counts as material left.

| Setting | Default | What it does |
| --- | --- | --- |
| Smallest material to clean | 0.15 mm | Material thinner than this above the part is taken as clean. Keep it above one and a half simulation cells. |
| Rotation, Start angle | `positive`, 0° | As for helical finishing. |
| Largest angle step | 2° | The most A may turn between two points. |
| Passes | Spiral | **Spiral** or **Rings**. Either way only the places with material left are cut. |

Also: **Exact STEP surface contact** (on), **Face free ends**, **Stepdown**, **Pass order** and **Links between passes**.

## Pencil clean-up

Follows sharp inside corners to remove ridges left by other finishing operations. Use it as a final clean-up, not to cut large areas. It does not cover every inside corner. It ignores selections.

| Setting | Default | What it does |
| --- | --- | --- |
| Smallest material to clean | 0.15 mm | As for rest machining. |
| Crease angle | 30° | The smallest turn between two faces that counts as an inside corner. |
| Largest step | 0.5 mm | The furthest apart that points of the path are along the corner. |

Also: **Exact STEP surface contact** (off).

## Face flow finishing

Finishes selected faces of a STEP model with passes that run along one edge of each face, the guide, and step across the face to the opposite side. The passes follow the face's shape rather than the machine's axes: for example the floor between the blades of a blisk. Where the two sides differ in length, the passes blend between them. It always uses the exact faces.

In its Geometry section, **Cut on** has two rows:

Cut on
: The faces to cut, each with passes along its guide. Only face sets can go here, and at least one is needed.

Guide edge
: The edges each face's passes run along. With none, each face starts from its longest side: the face's outline is split into sides where it turns by more than 30°, and the longest is the guide. The guide edges must be on the outline of the faces, joined end to end, and not the whole outline.

Each face is cut on its own, longest guide first, then the faces that share an edge with it. No pass crosses from one face to the next.

| Setting | Default | What it does |
| --- | --- | --- |
| Scallop height | 0 mm | The highest ridge left between passes for a ball-end or bull-nose tool; passes are put closer than the stepover where it needs it. 0 leaves the stepover alone. With other tool shapes it is ignored, and a note says so. |
| Cutting direction | `meander` | `meander` alternates the direction of the passes; `climb` and `conventional` cut every pass the same way relative to the uncut material. |
| Lead angle | 0° | From −30° to 30°. How far the tool leans from the normal towards the travel round the axis. Passes along the axis cannot lean. |
| Maximum tilt | 5° | From 0° to 30°. How far the tool may leave its leaned direction to smooth the turning of the part. |

The notes say how many passes were cut on how many faces, and the guide of each face. Where a neighbouring face, such as a blade wall, is in the way of the tool, or the face is too narrow for it, that stretch is left uncut and a warning lists where (up to five places, then how many more). Check those places in the simulation.

## Edge and curve following

Moves the tool along the part's edges to cut lines, for engraving or carving. It follows lines rather than clearing areas.

| Setting | Default | What it does |
| --- | --- | --- |
| Mode | `engrave` | `engrave` cuts one line along each chain of edges; `carve` cuts a band of the **Carve width**. |
| Depth | 0.2 mm | How far below the surface the tool tip goes, along the tool axis. |
| Line width | 0 mm | Above 0, the depth is the one at which the tool cuts this width (for a V-bit). |
| Depth per pass | 0 mm | The depth of each level; 0 cuts the whole depth at once. |
| Carve width | 0 mm | Carve only: the width of the band, centred on the chain. |
| Start | `first` | Where each chain starts: as stored (`first`), `reversed`, at the lowest or highest X (`low_x`, `high_x`), or nearest the **Start angle** (`angle`). |
| Start angle | 0° | From −360° to 360°, measured from +Y towards +Z. |
| Crease angle | 30° | The smallest turn between two faces that makes a mesh edge one to follow. |
| Shortest chain | 0 mm | Chains shorter than this are not cut. |
| Largest step | 0.5 mm | The furthest apart that points of the path are along a chain. |

## Swarf (flank) machining

Cuts selected wall faces, such as a blade, with the side of the tool. The wall can curve along its length, but must have straight lines (rulings) across its height for the side of the tool to follow. Put a face set of the walls in **Cut on**. Faces that are not ruled, or whose rulings run along the rotary axis, are refused, and the message names them.

Some faces make the tool cut into the surface by more than the tolerance. A warning then gives the depth and the place, and says which tool or tolerance would avoid it.

| Setting | Default | What it does |
| --- | --- | --- |
| Keep off the end faces | on | Stops each pass short of the faces that meet the ends of the face, such as a wall it stands against, by the least length that keeps the tool off them. That part of the face is left uncut, and the note says how much. Off, the pass runs to the ends, and any cut into those faces is warned of. Only the faces that share an edge with the face are checked. |
| Direction | `forward` | Which way along the face the tool moves. |
| Tip extension | 0 mm | How far the tool tip reaches beyond the end of the ruling it sits at. A negative value pulls the tool up the face, leaving a strip that wide uncut. |
| Tip end | `auto` | Which end of the rulings the tip sits at: `auto` chooses the end with less tilt, then the one nearer the axis; `reversed` takes the other. |
| Most material left | 0 (meaning 0.1 mm) | The most material the flank may leave anywhere on the face. More than the tolerance is reported; more than this refuses the face. |

**Keep off the end faces** checks the faces next to the face against the tool's cutting part only: the shank, the holder and the rest of the part are not checked. The swarf warnings each point at the setting to change. The three below are about a cut into a face next to the face, and are warnings to acknowledge:

- The tool cuts into a face along the ends of the rulings, such as a floor, by more than the tolerance: put that face in an avoid set on **Cut short**, or set a negative **Tip extension**.
- With **Keep off the end faces** off, the tool cuts into an end face: turn the setting on, or put the face in an avoid set on **Cut short**.
- With it on, the tool still reaches an end face between the points checked: put the face in an avoid set on **Cut short**, or use a tool with a smaller radius.

If no position along the face keeps the tool off its end faces, the face is refused: use a tool with a smaller radius, turn the setting off, or take the face out of the set.

## Radial drilling

Drills holes into the side of the part, turning the part so each hole faces the tool. It finds the round holes in a STEP model. Use a drill that matches the hole.

| Setting | Default | What it does |
| --- | --- | --- |
| Peck depth | 0 mm | The deepest one peck goes; 0 drills each hole in one move. |
| Peck mode | `full_retract` | Between pecks the tool goes back to the retract height (`full_retract`), or backs out a short way (`chip_break`). |
| Chip-break retract | 0.5 mm | How far the tool backs out in `chip_break` mode. |
| Retract height | 2 mm | How far above the top of the hole the tool retracts to and starts feeding from. |
| Re-entry clearance | 0.5 mm | In `full_retract` mode, a rapid back into the hole stops this far above the last peck. |
| Through allowance | 0.5 mm | How far past the end of a through hole the tool's full diameter goes. |
| Blind hole depth | `full_diameter` | A blind hole is drilled until the full diameter (`full_diameter`), or the tip (`tip`), reaches its depth. |
| Diameter tolerance | 0.1 mm | A hole is drilled only if its diameter is within this of the tool's. |
| Axis tolerance | 0.5° | A hole tilted more than this from the plane across the axis cannot be drilled on four axes and is left out. |

Holes left out are listed in the notes with the reason. If no hole can be drilled with the tool, the operation cannot be generated. The output has no canned drilling cycles.

## Checks on every operation

### Errors, warnings and notes

Errors
: The operation cannot be generated, and the message says why. Export is blocked while an operation is in error; turn it off to export the others.

Warnings
: The path was made, but something needs your attention. The export lists each one in the same words as Messages (for example "Held region (operation 'Finish'): ..."). Read them before you run the program.

Warnings to acknowledge
: A warning that may be a collision of the tool, its shank or its holder with the part, the stock, a tab or a held region. The operation's mark reads "Needs acknowledging: ...", Messages lists it first among the warnings, and the export writes nothing until you tick that you have read it (see [Possible collisions must be acknowledged](#possible-collisions-must-be-acknowledged)).

Information
: Notes about what the operation did, such as how many links stayed low.

### Flute length

For strategies that do not cut in layers, the program checks the written path against the stock in the tool's reach. Where the stock stands higher above the tool's tip than the tool's flutes are long, the shank may meet the stock. This is a warning to acknowledge, "Flute length (operation '...', tool '...'): ...", and the path is kept. It gives the depth and the place, and suggests a tool with longer flutes, roughing the part first in this setup, or turning on **Finishing layers** where the strategy has it.

Rotary roughing, facing and finishing layers check their own **Stepdown** instead: a stepdown deeper than the flutes is an error.

### A free end deeper than the flutes

With **Face free ends** on, where the stock beside a free end is deeper than the tool's flutes, that end is not faced. It keeps what earlier operations left, the rest of the path is made, and a "Flute length" warning to acknowledge says so. Use a tool with longer flutes, face the end with a Facing operation, or turn off **Face free ends**.

### Tabs

An operation is checked against the tabs it can reach. A tab shorter than the tool is wide gives a warning to acknowledge, "Tab length (operation '...', tool '...'): ...": the tool may not be able to cut the gap beside it to shape, and may cut through it. Use a smaller tool, or make the tab longer. A tab that is not wholly in the head or tail zone, between a held region or the end of the stock and the part, is an error for the operations that can reach it. See [Tabs](../concepts/tabs.md).

### A partial path must be acknowledged

Where **Adaptive spacing** cannot confirm the material left between passes before it reaches its limits, it keeps the path made so far and warns that it is not complete. Before it writes any G-code, the export asks you to tick "I accept the partial path made for this result: it is not complete, and I will check the surface before running the program". Without the tick it does not write the files.

### Possible collisions must be acknowledged

These warnings may be a collision, so the export asks you to tick "I have read the possible collisions listed for this result and will check them before running the program" before it writes any G-code. The tick is separate from the partial-path tick, and holds for these files only.

| Asks for the tick | Does not |
| --- | --- |
| The stock deeper than the tool's flutes, in any of the flute-length warnings | A swarf overcut into the face it cuts |
| A tab shorter than the tool is wide | A cut into an avoid set |
| Swarf cutting into a face next to it | The ring of stock left at a faced end |
| A check set the tool touches (**Warn only**) | A move beyond the machine's travel |
| A part that can be cut free at a free end and thrown | Areas simultaneous finishing could not reach |
| A rapid move that removes material, found by the simulation | A shank wider than the cutting part, where the path is broken |
| A safe cylinder that does not enclose the stock or the part, or is inside the minimum clearance | |
| A tool that may be too long to clear the workholding at the safe Z | |

On the command line, `openchuck-cli regenerate --out` refuses to write while there are possible collisions, unless you pass `--acknowledge-collision-warnings` (see [Command line](command-line.md#regenerating-and-writing-g-code)).

None of these checks shows that a program is right for your machine. The program does not check the chuck, a tailstock, the tool holder or the machine body against the part; see [Known limitations](../start/limitations.md).
