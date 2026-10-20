# OpenChuckCAM user interface specification

- **Status:** accepted by the project owner on 15 Oct 2026, with [decision 0038](../decisions/0038-ui-specification-and-requirements.md). It ranks with the decision records (`docs/README.md`, precedence 5): where it disagrees with an accepted decision record, the record wins until one of them is changed. The known differences from what slice 8 built are listed in section 17, with the agreed resolutions. Sections 7.5 and 11.7 (held regions and retaining tabs) were added on 18 Oct 2026 for [decision 0104](../decisions/0104-held-regions-and-tabs-in-the-interface.md) (work breakdown slice 21d), with the other sections' references to them; what the current build does differently is D13 in section 17. On 18 Oct 2026 the catch-up edits agreed with [decision 0114](../decisions/0114-slice-22-conformance-report.md) were made (OI-340 items 1 to 7, and the hover and selection widths of OI-341; the owner's answers, relayed by the coordinator and confirmed by the owner in the lane X1 session): sections 7.5, 11.2, 11.5, 11.6, 12.2, 15 and 16. On 19 Oct 2026 the four readings of [decision 0117](../decisions/0117-slice-22-shell-frame.md) (lane X2, the shell frame) were written into sections 3, 4, 5 and 9 (OI-350, the owner's answer A, relayed by the coordinator). On 19 Oct 2026 section 11.3 item 3 (Part position) was edited to the owner's answer on OI-366 ([decision 0123](../decisions/0123-slice-22-inspectors.md)): the model's turn about the rotary axis is set by its placement steps.
- **Date:** 15 Oct 2026; sections 7.5 and 11.7, 18 Oct 2026; the decision 0114 catch-up edits, 18 Oct 2026; the decision 0117 readings, 19 Oct 2026; the decision 0123 edit of 11.3, 19 Oct 2026.
- **Design source:** the design canvas "OpenChuckCAM UI Design" (a claude.ai artifact, private to the project owner: <https://claude.ai/artifact/1Qi5cpiyAiz3r5DkFnXkbu>). Its boards are named in section 1. The mockups use example data (the "Groove demo" project); the numbers in them are not results.
- **Requirements:** the requirements this design needs were added by decision 0038: UI-10 to UI-16, GC-15, GC-16 and NFR-15 (P1), and UI-17 to UI-24 and SIM-11 to SIM-13 (P2); SIM-14 (P2) was added later for the per-operation deviation. An element that serves a P2 requirement is marked with its ID in italics and is not part of the first release. UI-25 (P1) and the amended UI-03 were added by decision 0104.

## 1. Purpose and scope

This document says how the desktop application (`openchuck-ui` on egui, with `openchuck-viewport` on wgpu) looks and behaves: the layout, the workflow, the states shown, the wording and the visual tokens. It is written for the lanes that build the user interface (lanes A, M, N, O, S and T of the work breakdown's parallel plan).

It does not change what the engine does. Requirements stay in `requirements.md`, and how the code is split stays in `architecture.md`. Three rules from the architecture apply throughout:

- The UI holds no CAM logic (architecture section 5.8). Every value it shows comes from a view model or query in `openchuck-app`.
- Every edit to the project is a command, and undoable (UI-08).
- Long work runs as a job, never on the UI thread (UI-09, NFR-04).

Each element below names the requirement it serves. An element that serves only a P2 requirement is not part of the first release.

**Boards in the design canvas**

| Board | Section |
| --- | --- |
| 0 New project (empty state) | 11.1 |
| 1 Model; 1b Import dialog | 11.2 |
| 2 Setup | 11.3 |
| 2c Workholding and tabs | 11.7 |
| Held regions and tabs in the view | 7.5 |
| 3 Operations; 3b Inspector, operation in error | 11.4 |
| 4 Simulate and review | 11.5 |
| 5 Post and export | 11.6 |
| Tool library and profile editor | 12.1 |
| Selection sets (dock tab) | 12.2 |
| Undo history (popover) | 12.3 |
| Operations at 1366 × 768 | 3 |

Every workspace board has a dark and a light theme (its Tweaks panel on the canvas). Rendered images of the boards are in [mockups/](mockups/), listed in section 20.

## 2. Users and design principles

The user is the one in requirements section 2: a machinist, maker or small-shop programmer, comfortable with G-code, tooling, feeds and speeds, who may know Fusion, SolidWorks or FreeCAD. The workflow is the eight steps of that section.

1. **Familiar CAD layout.** Menu bar, project tree on the left, 3D view in the centre, properties on the right, messages below, status at the bottom. A user of another CAD or CAM tool should find each thing where they expect it.
2. **The workflow order is visible.** The five steps (Model, Setup, Operations, Simulate, Post and export) follow requirements section 2 and architecture section 6, left to right.
3. **Hierarchy of importance.** On any screen, in this order: whether the job can go on (errors, stale results, running work), then the result of the selected item, then its parameters, then reference information. Layout, size and weight follow that order.
4. **State is always visible, and never by colour alone.** Each state has a shape as well as a colour, and a legend that does not scroll away.
5. **Problems are shown where they are, and in one list.** A problem is marked on its item, explained in the inspector, and listed in the messages panel with a way to fix it.
6. **One primary action, bottom right.** Each screen has one filled button for the next step of the job (section 5).
7. **Nothing implies the G-code is safe.** Generated G-code is the user's to verify (NFR-12). No wording, colour or icon says "ready to run", "safe" or "verified".
8. **Geometry in the view, numbers in the panels.** The viewport shows shape and position. Exact values are in readouts, panels and tooltips.
9. **Keyboard and pointer are equal.** Every command has a menu entry, most have a shortcut, and anything shown on hover is also shown on keyboard focus.
10. **Never block.** Import, scene building, generation and simulation run as jobs with progress and cancel (UI-09).

## 3. Window structure

Sizes are logical pixels at 100% scale. The default scale is 120% (decision 0029, owner's run, observation 1; kept by the owner on 15 Oct 2026, OI-166), with View > Larger, Smaller and Reset, and the chosen scale is remembered (UI-15).

| Region | Size | Contents |
| --- | --- | --- |
| Title bar | 32 high | Menus (File, Edit, View, Project, Tools, Help); project name and save state; command search *(UI-17)*; Undo, Redo, Undo history *(UI-22)* |
| Workflow bar | 44 high | The five steps (section 4) on the left; actions that start something new or change context on the right (New operation, Duplicate, Add setup, Import model, Simulate from). Nothing that moves the job forward is here (section 5) |
| Project panel | 264 wide (200 to 400) | The project tree (section 6) and its status legend |
| Viewport toolbar | 44 high | View controls (section 7.1). A strip, not an overlay |
| Viewport | the rest | The 3D view and at most five overlays (section 7.2) |
| Legend strip | 36 high, optional | A legend in a strip under the view on Model and Setup, where there are no paths |
| Dock | 196 high (120 to half the window) | Tabs: Messages, Selection sets, Jobs; Setup checks on Setup; Playback on Simulate (section 9) |
| Inspector | 340 wide (280 to 520) | The selected item (section 8) |
| Footer | 48 high | Status on the left; display settings and the step's actions on the right, primary right-most (section 10) |

- **Splitters** are 5 pixels with a grip mark. Double-clicking one resets its panel to the default size.
- **Collapsing.** The project panel, the inspector and the dock collapse with the chevron in their headers, or Ctrl 1, Ctrl 2 and Ctrl 3. A collapsed side panel leaves a 28-pixel rail with its icon. *(UI-16)*
- **Smallest window.** At 100% scale, everything stays usable at 1366 × 768 logical pixels (UI-16). At the default 120% a 1366 × 768 screen gives about 1138 × 640 logical pixels, so on such a screen the user reduces the scale once with View > Smaller. At that size: the panels keep their defaults, the viewport shrinks, and overlays keep clear of each other (the "Operations at 1366 × 768" board). Below that size the dock collapses, and the inspector too when the viewport would be narrower than 420 pixels. Neither is kept as the user's choice: the user can open either again, and the rule applies afresh once the window is large again. *(UI-16)*
- **Remembered layout.** Panel sizes, collapsed panels, open sections, scale and theme are remembered per user on the computer, never in the project (section 13). *(UI-15)*
- **Dialogs.**
  - Modal dialogs (open, new, unsaved changes, errors) stay inside the main window.
  - Tool windows (import, tool library, export) open in their own operating-system window where the platform allows, through the `floating` helper of decision 0029 item 17, and inside the main window where it does not.
  - A tool window draws no 3D. The import preview stays in the main viewport.

## 4. Workflow steps

The steps are views over one project, not modes. The tree, menus, undo history and messages are the same in every step. Any available step can be opened at any time. An edit in an earlier step marks later results stale (architecture section 6; decision 0030).

| Step | Purpose | Available when | Reason shown when not | Primary action in the footer |
| --- | --- | --- | --- | --- |
| 1 Model | Import, check units and mesh, place on the rotary axis (IMP-01 to IMP-06) | Always | | "Import model…" with no model; "Continue to setup" otherwise |
| 2 Setup | Machine, stock, part position, work offset, clearance (SET-01, SET-03, SET-05, SET-06) | A model is in the project | "Import a model first" | "Continue to operations" |
| 3 Operations | Select geometry, add operations, generate (OP, UI-04 to UI-07) | The setup has a machine and stock | "Set up the machine and stock first" | Section 5 |
| 4 Simulate | Play the cut and compare stock with model (SIM-01 to SIM-07) | One enabled operation has been generated | "Generate an operation first" | Section 5 |
| 5 Post and export | Write G-code and the setup sheet (GC-02 to GC-06, GC-12, GC-14) | One enabled operation has been generated | "Generate an operation first" | Opens the export dialog; "Write G-code" is its primary action |

**The step indicator.** Each step shows a numbered circle and a name.

- The active step has an accent underline and a filled circle.
- A step whose outputs are complete has a green ring round its number. Complete means:
  - Model: there is a model and none is in error;
  - Setup: there is a machine, every setup has stock and a part, and no setup check warns;
  - Operations: every enabled operation is up to date;
  - Simulate: every enabled operation has been simulated as it is now;
  - Post and export: G-code has been written from every enabled operation as it is now.
- An unavailable step is dimmed, and its tooltip gives the reason.
- Alt 1 to Alt 5 open the steps.

*(UI-10)*

## 5. The primary action

- **One per screen.** Each screen has exactly one primary (filled) button, right-most in the footer. Secondary actions sit to its left, the most likely nearest. Cancel for running work sits left of them.
- **It is the next required step.** It is the first of these that applies:
  1. If an enabled item has an error that blocks the next step, the primary button is disabled. Its tooltip names the problem, and the problem's row in the messages panel is highlighted.
  2. If enabled operations are stale, it is "Generate stale (n)".
  3. On Simulate, if an operation has not been simulated, it is "Run simulation". If it has not been generated either, it is "Generate and simulate op N". On Simulate this rule is checked before rule 2, since simulating generates.
  4. Otherwise it continues to the next step.
- **Disabled means explained.** A disabled primary button always has a tooltip with the reason, and the footer status says the same.
- **Dialogs follow the rule.** Import has "Add to project", the tool library "Add to project" (when opened from "+"), and export "Write G-code". Each is bottom right.
- **Other actions go elsewhere.** Actions that start something new (New operation, Add setup, Import model) go in the workflow bar. Row actions go on the row.

## 6. Project panel

**One tree in every step** (UI-06). Groups, in pipeline order:

1. **Model.** One row per model, with child rows for its placement and mesh check.
2. **Setups.** One row per setup, with child rows for the machine profile, stock, part position, work offset and clearance, and workholding (11.7).
3. **Selection sets.** Each set's colour swatch, name and size.
4. **Tools.** Each tool's project tool number chip and name.
5. **Operations.** In machining order.

Each group header shows a count and a "+" button (22 pixels):

| "+" on | Does |
| --- | --- |
| Model | Opens the import dialog (11.2) |
| Setups | Opens Setup with a new setup, "Setup 1", with the project's first model as its part (decision 0144). It is disabled with the tooltip "Import a model first" until the project has a model. It creates exactly one: once a setup exists the "+" stays visible but disabled, with the tooltip "More than one setup per project is not supported yet (SET-07)", until SET-07 (P2) is built |
| Selection sets | Opens Operations with picking on and an empty working selection ("Select on the part…", decision 0037 item 7) |
| Tools | Opens the tool library (12.1) with "Add to project" as its primary action |
| Operations | Opens Operations and the New operation menu, which lists the registered strategies (OP-16) |

**Rows** are 28 high. Columns, left to right:

1. A drag handle (operations).
2. An enable box (operations).
3. The status mark.
4. An icon or colour swatch.
5. The name, cut with an ellipsis.
6. A chip (tool number, triangle count, work offset).
7. A row action: progress and cancel while running, or the visibility eye on Simulate.

The selected row is tinted, with a 2-pixel accent bar on its left edge.

**Operation rows** (decision 0029 item 10):

- the enable box;
- move earlier and later, by dragging or Alt ↑ and Alt ↓;
- duplicate (Ctrl D), and delete (Del), each one undoable step.

A disabled operation's name is struck through and muted. It is skipped everywhere: no toolpath, no stock removal, no G-code, and its tool is not in the tool-number plan.

**Context menu** (right click or the menu key): Rename, Duplicate, Enable or Disable, Delete, Show in view, and Generate (operations).

**Status marks** *(UI-11)*:

| State | Mark | Applies to | Meaning |
| --- | --- | --- | --- |
| Up to date | Filled green circle | Operations | Generated, and nothing it reads has changed since (decision 0030) |
| Stale | Amber diamond | Operations | Something it reads changed; its toolpath and the stock after it are out of date |
| Running | Blue ring with a gap, the percentage, a cancel button, and a 3-pixel progress bar under the row | Operations, import, scene | A job is working on it |
| Error | Red triangle | Operations, selection sets, models | It cannot be used. The tooltip and inspector say why and how to fix it (decision 0032 item 7, decision 0037 item 8) |
| Warning | Amber triangle outline | Models, setups, machine | It can be used, with a caveat (a mesh finding, an unconfirmed machine value) |
| Off | Grey ring, name struck through | Operations | Disabled by the user |

- **The legend** of these marks is pinned to the bottom of the panel and never scrolls away.
- **Tooltips** on a mark give the cause, for example "Stale: operation 2 changed" or "Error: 12 of 40 edges are missing".
- **Problems go up the tree.** An operation that uses a selection set with a problem shows its own error mark (decision 0037 item 7).

## 7. Viewport

### 7.1 Toolbar

A 44-pixel strip above the view, never over it. Icon-only buttons have a tooltip with their shortcut and an accessible name.

| Control | Steps | Requirement |
| --- | --- | --- |
| Display: Shaded, Wire, Edges (shaded with feature edges at 30°, decision 0029 item 4) | All | UI-02 |
| Projection: Persp, Ortho | All | UI-02 |
| Standard views: Front, Top, Iso (all seven in the View menu and on the view cube) | Model, Setup | UI-02 |
| Fit (F) | All | UI-02 |
| Show or hide (V): part, stock, rotary axis and work axes, held regions and tabs, selections, toolpaths by operation | All | UI-03, SIM-01 |
| Section plane (S) | All | *(UI-21)* |
| Measure (M): distance, angle, radius about the rotary axis | All | *(UI-21)* |
| Turn part to A [angle] Turn: turns the part in the view only, never the program; a chip "View turned to A −53.20°" shows while it is turned, and Fit resets it | All | *(UI-21)* |
| Show: Stock, Part + path, Both | Simulate | SIM-03, SIM-05 |
| Show findings | Model | IMP-04 |

### 7.2 Overlays

At most five floating boxes, each in a fixed place, never overlapping. All are dark in both themes, because the viewport stays dark (section 14.2).

| Place | Box | Steps |
| --- | --- | --- |
| Top left | Selection tools: a filter, "Pick: Both, Faces, Edges" (Both, the default, picks an edge when the pointer is near one and the surface otherwise), then Select one, Flood fill, By angle, Same direction, Box *(Box is OI-201)*; on edges, Select one, Chain and Smooth chain *(decision 0066 items 3 and 4)* | Operations, while picking |
| Left, below the tools | Path legend: Cut, Link, Rapid, Entry, Selected, Avoid; then Held region and Tab when the setup has them (7.5) | Operations |
| Top right | View cube; clicking a face gives that standard view | All with a 3D view |
| Top right, under the cube | Axis triad: X red, Y green, Z blue | All with a 3D view |
| Bottom right | Readout: X, Y, Z in mm to 3 places, A in degrees to 2; its frame (G54 work coordinates) and source (cursor on surface, or tool at move N) | Operations, Simulate |
| Bottom left | Colour scale (11.5) | Simulate |

- **Small windows.** When the view is under 480 pixels high, the path legend collapses to a button.
- **Probe.** A probe tooltip (11.5) may sit anywhere near the pointer, and is drawn above the other boxes.

### 7.3 Scene and camera

- **What is drawn** (UI-03; decision 0029 item 3): the part, the stock outline, the held regions and retaining tabs (7.5), the rotary axis along model X past both ends, the A+ arrow from the shared kinematics, and the work coordinate system at the work zero labelled X, Y and Z. The A+ label sits an eighth of a turn along the arc so it never covers the Y label.
- **Camera** (decision 0029 item 6):
  - Z-up, model X along the rotary axis.
  - Standard views as that record's table.
  - Orbit with the primary button, pan with the secondary or middle, zoom with the wheel.
  - Fit on open and on each standard view.
  - Switching projection keeps the size on screen.

### 7.4 Colour roles

The same role has the same colour everywhere. Hex values are the dark theme; the viewport uses them in both themes.

| Role | Colour | Mark |
| --- | --- | --- |
| Part | Grey `#b7bfca`, shaded | |
| Feature edges | `#d3d9e0` | Thin line |
| Stock outline | Blue `#7fb4ea` | Dashed |
| Rotary axis and A+ arrow | Cyan `#35d0e0` | Dash-dot line, arrow with "A+" |
| X, Y, Z | Red `#ff7b72`, green `#6fd3a8`, blue `#6fb2ff` | Arrows with letters |
| Cut moves | Amber `#ffb454` | Solid |
| Link moves | Green `#6fd3a8` | Solid, thinner |
| Rapid moves | Grey `#9aa6b5` | Dotted |
| Entry and exit moves | Lilac `#e8b6ff` | Solid |
| Tool | Grey body, lilac tip | |
| Working selection | Blue `#3d8bff` at 40% | Fill (OI-163, settled) |
| Hovered triangle or face | Yellow `#ffe066` | Outline (OI-163, settled) |
| Hovered or selected edge | Hover yellow `#ffe066`; selected in the working selection's blue `#3d8bff` or the shown set's swatch | Line 2 pixels wide hovered, 3 pixels selected, against the 1-pixel feature edges, so state is not by colour alone (section 16; decision 0112, OI-335) |
| Saved set | Its own swatch from a fixed set of eight | Fill when shown |
| Avoid and check surfaces | Red `#ff7b72` | Hatched |
| Mesh findings | Red outline; holes ringed amber | (decision 0029 item 13) |
| Clearance cylinder and safe Z | Green `#4cc38a` | Dashed |
| Machine travel limit | Red `#ff7b72` | Hatched band |
| Held region | Violet `#9d8df1` at 35% | Cross-hatched fill, solid outline (7.5) |
| Retaining tab | Sand `#d8b98a`, shaded | Opaque, dark solid outline (7.5) |
| Deviation | Section 11.5 | |

### 7.5 Held regions and retaining tabs

UI-03 and UI-25 ([decision 0104](../decisions/0104-held-regions-and-tabs-in-the-interface.md)); the solids are those of decision 0045 item 1 (a held region at each end, then each tab; a tab around the axis is `count` solids). The view draws what the setup stores, except while the workholding section has unapplied changes (11.7), when it draws the draft. Board "Held regions and tabs in the view" (section 20).

- **Two roles that differ by more than colour** (principle 4; NFR-15):

  | Role | Colour | Fill | Outline |
  | --- | --- | --- | --- |
  | Held region | Violet `#9d8df1` | Translucent (35%), cross-hatched in two directions at 6 pixels on screen | Solid 1.5-pixel line in the role colour round the silhouette and both end circles |
  | Retaining tab | Sand `#d8b98a` | Opaque and shaded, like the part | Solid 1.5-pixel dark line (`#15171a`) round the silhouette and both end circles |

  The cross-hatch is not the single-direction hatch of avoid surfaces and the travel limit (7.4), and neither role uses dashes, which mean outlines of stock and clearance. A held region is translucent so the part and stock behind it stay visible; a tab is opaque because it is part of the finished surface (decision 0035 item 1).
- **They turn with the part.** Like the part and the stock, they are fixed to the workpiece, so they turn with A in the machine view and with "Turn part to A" *(UI-21)*.
- **Show or hide.** The "Show or hide" control (V, 7.1) has one entry, "Held regions and tabs", on by default. Hiding them changes only the view, never what is machined; a line under the entry says so.
- **Legend.** Whenever the setup has a held region or a tab, the legend of the step lists the roles that are present, with a swatch drawn the way the role is drawn (a hatched violet square, a sand square with a dark border): in the legend strip on Setup, in the path legend on Operations (after "Avoid"), and under the colour scale on Simulate (outline swatches, labelled "Held region (kept)" and "Tab (kept)").
- **Hover.** Pointing at a held region or a tab outlines the whole solid with a 2-pixel line in the hover yellow `#ffe066` (7.4) and opens a tooltip with its name and size, for example "Tab 2 · frustum · X 14.000 to 16.000 mm · Ø 4.000 to Ø 8.000 mm", or "Held region at the start · 6.000 mm · Ø 26.000 mm (stock)". A tab around the axis adds "1 of 3 at 90.00°". The same outline and tooltip appear when the solid's row in the inspector (11.7) has keyboard focus (principle 9).
- **Selection.** Clicking one selects its setup, opens the inspector at the workholding section with that held region or tab expanded and its first field focused, and draws the solid with a 3-pixel `accent` outline while it stays selected. Esc clears it. A tab around the axis is selected as one tab, all its solids together.
- **Widths.** One scale holds across the view: plain lines and feature edges 1 pixel, role outlines 1.5, hovered 2, selected 3, the same as hovered and selected edges (decision 0112 item 3; decision 0114, OI-341).
- **While picking** (Operations, 11.4) they are drawn as outlines only, with no fill, and cannot be picked: clicks and the hover outline go to the part's triangles, so a selection is never made on a solid. A selection that reaches one gets the warning of 11.7.
- **A draft that cannot be drawn.** A held region or tab of the draft that fails a check (11.7) is drawn with its outline only, in `error` red `#ff7b72`, and the others as usual. If the setup's own workholding cannot be built (for example, a held region on box stock with no holding diameter, decision 0045 item 2), nothing of it is drawn and the messages panel says why, with "Edit workholding" as its action.
- **Default tabs offered, not added** (11.7) are drawn as dashed sand outlines with no fill, labelled "Default tab, not added", while the workholding section is open.
- **On Setup** (11.3), the two orthographic panes draw them in section: in the front view a held region is a cross-hatched violet band over the gripped length, under the chuck jaws, with its length dimensioned ("Held 6.000"), and each tab is its outline (a rectangle or a trapezoid) in sand with a dark border, labelled "Tab 1", "Tab 2". In the end view from +X, a tab on the axis is a circle at its largest diameter; tabs around the axis are small circles at their angles, the first one's angle dimensioned from +Y towards +Z ("Start 90.00°").
- **On Simulate** (11.5), the stock is coloured by deviation everywhere, held regions and tabs included, since they count as part of the target (decision 0035 item 8; decision 0045 item 6). Over it, each held region and tab is drawn as its outline only, in its role colour, so the colours under it can be read. The probe *(SIM-11)* names it: "Tab 1 (kept as part) · within tolerance".

## 8. Inspector

The inspector shows the selected item: a model, setup, selection set, tool or operation, or the simulation results on Simulate.

**Order** (principle 3):

1. **Header.** The name in an editable field, and a status chip in the words of the status mark.
2. **Error banner,** when the item is in error. It gives what cannot be done, why, and the actions that fix it (the "Inspector: operation in error" board).
3. **Result summary** of an operation: moves, cut length, A travel, A unwinds and programmed time, in a two-by-two (or two-by-three) grid of large numbers. Programmed time is labelled "programmed time" and is not the SIM-08 estimate (decision 0028 item 3). *(SIM-13)*
4. **Basic and Advanced switch.** *(UI-19)* Without it, every field is shown, grouped.
5. **Sections.** Collapsible, with their open state remembered per item kind. For an operation:
   - Geometry: Cut on, Contain within, Avoid and Check, the roles of decision 0032 item 5, with a list to add from and a button to remove;
   - Tool and feeds;
   - Pattern and tolerance;
   - Tool axis and A;
   - Entry, links and retract (decision 0026);
   - Advanced.

**Fields** (UI-07; decision 0029 item 11):

- **Layout.** The label column is 120 wide, the value to its right. Units sit inside the field on its right. The allowed range is in the tooltip.
- **Hint and source.** An empty field shows the value that applies as a hint, with its source: the operation, the tool's cutting data for the stock material, or the strategy default.
- **Committing.** A value is applied when the field loses focus or on Enter. Esc puts back the last applied text. Each applied value is one undoable command.
- **Refusals.** A refused value stays in its field with a red border, and the reason in red under it ("Must not exceed the tool diameter (3.0 mm). Not applied."). The project and the undo history are unchanged.
- **Reset.** A value the operation sets has a reset button (↺) in the label column. *(UI-19)* It also has a 6-pixel accent dot before its label.
- **Derived values** are read-only, with how they were worked out under them, for example "0.020 mm × 2 flutes × 14000 rpm". *(UI-19)*
- **Typed units.** A field accepts a number with a unit ("0.005 in"), converts it, and says "Stored as 0.127 mm". *(UI-18)*
- **Choices** are drop-down lists. Two to four exclusive choices are a segmented control. Flags are check boxes.

**Status icons in the machine panel** *(UI-13; data from decision 0011 item 8, `provenance`)*:

- **Placement.** Each machine value has a 20-pixel icon at the right end of its label, so the icons line up in one column.
- **The two icons:**
  - green tick in a circle: confirmed on the machine;
  - amber triangle: not confirmed on the machine.
- **Tooltip.** It opens on hover and on keyboard focus, and gives where the value came from (the date it was confirmed, the vendor's profile, or a placeholder) and, when it is not confirmed, how to confirm it. For example: "Unconfirmed placeholder. Measure it by touching the tool tip to the chuck centre and reading G53 Z."
- **The rest of the machine panel** (decision 0029 item 16):
  - the profile chooser;
  - "copy stored in project";
  - the comparison with the library source, with an Update button;
  - "Create the library folder" or "Open the library folder".

## 9. Dock

**Tabs:** Messages, Selection sets and Jobs, on every step. Setup adds "Setup checks". Simulate adds "Playback" (11.5).

**Messages** *(UI-12)*:

- **Rows.** Each row has:
  - a severity icon (error, red triangle; warning, amber triangle; information, blue "i");
  - one or two plain sentences saying what happened and why;
  - the source item, as a link;
  - an action link ("Reselect", "Review setup", "Generate").
- **Order.** Errors first, then warnings, then information. Messages are worked out from the project's state and carry no time, so within a severity they follow a fixed order of their sources, from the latest kind of news to the earliest: job failures, generation, selection sets, setups, models.
- **Filters.** Chips for each severity with their counts.
- **Links.** Choosing the source selects the item in the tree and the view.
- **Where messages come from:**
  - changed and missing model files when a project opens (decision 0029 item 13);
  - the import check;
  - setup checks;
  - generation errors (decision 0032 item 7);
  - selection set problems (decision 0037 item 6);
  - selections that reach a held region or a tab, and a stored workholding that cannot be built (11.7);
  - job failures.
- **Clearing.** A message goes away when its cause is fixed.
- **No pop-up errors.** None, except when a command the user gave fails outright (a failed save, an unreadable file).

**Setup checks:** one row per check, each with a green tick or an amber triangle (SET-06). The checks of the stored workholding against the part and the tools are rows here (11.7).

**Selection sets:** section 12.2 (decision 0037 items 6 and 7).

**Jobs** (UI-09):

- running and queued jobs, with progress, elapsed time and cancel;
- finished jobs, with their outcome, for example "Generated 2, reused 1" (decision 0030 item 6).

## 10. Footer

- **Left:** the current job with its spinner and percentage, or "Ready", then a short note about the step ("edits are applied when you leave a field; undo with Ctrl Z"). After it comes the G-code notice in short form: "Verify G-code before running it" (NFR-12; decision 0029 item 8 shows it in the status bar).
- **Right**, left to right:
  - the display units switch, "mm | in" *(UI-18)*; with P1 only it reads "mm · deg" and cannot be changed;
  - the tolerance of the selected operation or setup;
  - machine warnings in amber, linking to Setup;
  - a separator;
  - the step's actions, with the primary right-most (section 5).
- **Save state** is in the title bar: the project name, then "· saved" or a neutral grey dot and "Edited". Amber is kept for warnings.

## 11. The steps in detail

### 11.1 New project (empty state)

- **The window.** The tree shows each group with "None yet". Steps 2 to 5 are dimmed with their reasons. Undo and Redo are dimmed.
- **The centre card,** "Start with a model":
  - a drop zone for a model file;
  - "Import model…" and "Open project…";
  - the five steps, one line each;
  - recent projects *(UI-23)*.
- **The footer** primary is "Import model…".

### 11.2 Model, and the import dialog

**Import dialog** (IMP-01, IMP-03, IMP-04; decision 0029 item 13; board 1b):

- **Opening.** File > Import model, the Model "+", or a dropped file. It opens in its own window.
- **Size.** The window has a fixed size (460 by 520 points) that fits 1366 by 768 at 120% scale; it cannot be resized or maximised. The path, its button and the step line are pinned at the top and the footer at the bottom; the blocks between them scroll (UI-16).
- **The path.** It is typed, with Browse (`rfd`, decision 0029 item 15) and the button "Check and preview". Under it, a line says where the flow is: "Step 1 of 2: check the file. Nothing is added to the project yet.", then "Step 2 of 2: set the units and placement, then add. Nothing is added until you press Add to project." once the check has run. A status row of fixed height shows the spinner while a check runs, so nothing moves when it appears.
- **Coming back.** When the application returns from the background, the window is brought to the front once (UI-14).
- **The preview** is the main viewport behind the window, not a pane of it, because a tool window draws no 3D (section 3; decision 0029 item 17). It shows the placed model, with findings highlighted:
  - flipped and degenerate triangles outlined in red;
  - holes ringed in amber;
  - open and non-manifold edges in red.

  A legend sits bottom left of the view, and size, largest radius and volume bottom right.
- **The window: three blocks.**
  1. **Units.** The unit used. The unit question, when the file names another unit or the size is implausible (decision 0010 item 4), is in an amber box. Under it, a unit chooser and the largest side after conversion.
  2. **Mesh check.** One row for each kind of finding, with its count. A tick when there are none; selecting a row highlights its triangles. One sentence says the check changes nothing.
  3. **The STL statement,** word for word as `STL_TESSELLATION_NOTICE`.
- **Placement.** An orientation choice that places the model on the rotary axis as it is added (IMP-05), shown in the preview; the Model step's placement steps edit it later.
- **Footer.** It says the file is not changed and that its path and hash are stored. Cancel, and "Add to project" (primary), which stays off until the check has run.

**Model step** (board 1):

- **Viewport.** The placed model; the bounding box with sizes along X and across Y and Z; the rotary axis labelled; a corner triad; the findings when "Show findings" is on.
- **Legend strip.** Rotary axis, bounding box, hole, flipped triangles, model origin.
- **Inspector:**
  - file, format and triangle count, size;
  - unit (a segmented control, with a hint for where the unit came from);
  - placement steps (IMP-05):
    - an ordered list with a drag handle, and a description and value on each row;
    - removing a step is one undoable command;
    - "Add step" offers centre, orient, translate, rotate, scale and mirror;
  - mesh check summary and "Open full report";
  - source: the content hash, "Replace file…" and "Show in folder".
- **Dock.** Messages and Import history.
- **Footer** primary: "Continue to setup".

### 11.3 Setup

Board 2 (SET-01, SET-03, SET-05, SET-06; UI-03).

**Viewport: two panes side by side.**

- **Front view,** orthographic (looking from −Y). It shows:
  - the stock and part, and the rotary axis. No chuck is drawn at this stage: the project holds no chuck geometry, and the held region already shows what the chuck grips (decision 0129 item 4; OI-380, deferred until a data change);
  - the work zero, with X and Z arrows and Y marked as into the screen (a circle with a cross);
  - the clearance cylinder and its radius;
  - the safe Z line, the Z+ travel limit, and the dimensions (stock length, stock diameter, axis to safe Z).
- **End view from +X** (270 wide). It shows:
  - stock, part and clearance circles;
  - +Y to the right, +Z up, −Y labelled "front";
  - the A+ arrow, with the sentence "A+ turns the top toward the front". This matches decision 0013 and the check on the machine (OI-03).

Both panes also draw the held regions and tabs in section (7.5).

A legend strip sits under the panes; it lists the held region and tab roles when the setup has them (7.5).

**Inspector:**

1. **Machine** (section 8): profile; rotary axis; A range; A+ direction; axis height (machine Z); travel and maximum feeds (collapsed). The provenance icons are on the first four and on travel.
2. **Stock** (SET-03): shape (Cylinder, Box, Bounds, where Bounds is the model's bounding box plus an offset), diameter or sizes, length, material. Imported stock (SET-04) is P2.
3. **Part position** (SET-05): offset along X; the model's turn about the rotary axis is set by its placement steps (11.2), and the section says so in a read-only row (decision 0123, OI-366).
4. **Work offset and clearance** (SET-05, SET-06): work offset register; zero position "On the A axis (Y0 Z0)", read-only (decision 0011 item 1); safe radius; safe Z above the axis.
5. **Workholding and tabs** (SET-09, OP-21, UI-25): the held regions, the head and tail zones and the retaining tabs, edited as a draft and applied; section 11.7.

**Dock:** Setup checks and Messages. While the axis height is a placeholder, the first check is "Safe Z cannot be checked against the Z travel" (OI-60).

**Footer** primary: "Continue to operations".

### 11.4 Operations

Boards 3 and 3b.

- **Viewport.**
  - The selected operation's toolpath, and the others by the visibility settings (SIM-01).
  - The working selection, saved sets and avoid surfaces.
  - The held regions and tabs, outlines only while picking (7.5). A selection that reaches one gives the warning of 11.7 under its Geometry role.
  - While picking: the selection tools, using the modes of decision 0037 item 4 (one triangle; connected shell; smooth patch by angle; same direction). Shift adds, and Ctrl (Command on macOS) removes.
- **Generating.**
  - The footer and the "Generate" item in a row's menu start a job. Its row shows progress and cancel.
  - Several operations generate in chain order. An operation served from the cache shows "reused" in the Jobs tab.
  - Cancelling leaves no partial result: the operation stays stale.
- **New operation.** Choose a strategy from the menu; the new operation takes the selection and the last tool used. A strategy that cannot honour a region (decision 0032 item 6) is listed with "ignores selections" and cannot take one.
- **An operation in error** (board 3b):
  - **Banner:** "This operation can't be generated", then the cause in one sentence, for example "The selection 'Lettering' no longer matches the model. 12 of its 40 edges were not found after the model file changed".
  - **Actions:** "Reselect in the view" (primary in the banner) and "Show missing". "Use the N matching edges" is *(UI-24)*.
  - **The broken field** has a red border and shows the count found ("28 of 40").
  - **A closing line** says export is blocked while the operation is in error, and that turning it off lets the others export.
- **Footer** primary: section 5. While generation runs with an operation in error, "Continue to simulate" is disabled, with its reason in the tooltip.

### 11.5 Simulate

Board 4 (SIM-01 to SIM-07).

**Tree:** the same tree. Each operation has an eye button that shows or hides it in the view only. Simulation results do not change when something is hidden; a line under the tree says so.

**Workflow bar:** "Simulate from" (start of operation N).

**Viewport:**

- **The stock,** coloured by deviation from the model (SIM-05). The scale is diverging and ordered by lightness, so it can be read without hue. Decision 0040 (accepted) built six bands by triangle with their own thresholds; which thresholds stand is OI-194 (D12):

  | Range | Colour | Meaning |
  | --- | --- | --- |
  | Below −tolerance | Dark magenta `#a3157f` | Overcut |
  | Within ±tolerance | Grey `#9aa0a8` | Within tolerance |
  | Above +tolerance to +0.10 mm | Light blue `#b9d6ff` | Material left, small |
  | Up to the scale end (+0.50 mm) | Blue to `#3f78d1` | Material left, more |

  The tolerance band is the operation's tolerance. The scale is in a bottom-left box with labelled ends.
- **Held regions and tabs** are coloured by deviation like the rest of the stock, with their outlines over it (7.5).
- **The tool** at the current move, with the part turned by A (SIM-02). Its label gives the tool and A.
- **The probe** *(SIM-11)*. Hovering the stock shows a tooltip: the deviation with its sign and unit ("Overcut −0.004 mm"), the position (X and A), and the operation and move that left it.

**Results inspector:**

- colour by (Deviation, Remaining, Overcut);
- worst values after the current operation: largest overcut, largest material left, the share within tolerance *(SIM-14)*;
- "Go to overcut" and "Go to largest left" *(SIM-11)*;
- a deviation histogram in the same colours;
- a table by operation (overcut and left *(SIM-14)*, status);
- resolution: the cell size, default 0.1 mm and finest 0.025 mm, with the cost stated (SIM-07; decision 0025).

**Playback dock** (SIM-04):

- **Transport:** previous operation, step back, play or pause (Space), step forward, next operation.
- **Speed:** 1×, 10×, 100×, 1000×, 10 000×, or the whole program in 10 s or 60 s.
- **Timeline:** one segment per enabled operation, sized by moves. A segment not generated is dashed amber and labelled. The playhead is a vertical line, with move numbers under it.
- **Go to move** N, and the feed at the current move.
- **G-code panel** (300 wide) *(SIM-12)*. The posted lines around the current move, with the current line highlighted. It uses the post the export will use.

**Footer** primary: section 5.

### 11.6 Post and export

Board 5. Opened from step 5, or with Ctrl E. Built in lane S ([decision 0071](../decisions/0071-slice-19-export-dialogs.md), accepted): until step 5 exists it opens from File, "Post and export…" as well as with Ctrl E; the post, grouping and folder are not remembered between openings (OI-248); a file that fails its own checks also blocks writing; the acknowledgement is kept when the first write succeeds.

**Left column:**

1. **Post-processor.**
   - The post for the machine (GC-02).
   - Feed mode (GC-04), read from the post and shown read-only: units per minute, or inverse time (G93) where the post uses it (decision 0071 item 8).
   - Units, A letter and wrap, read from the post (read-only).
   - Files (GC-05, decision 0028 item 1): Project, Setup (default), Operation or Tool. "Project" is disabled while the project has more than one setup run, with the reason.
   - Output folder.
2. **Tool numbers** (GC-14).
   - One row per tool: library tool, used in, and program number.
   - The machine's range is shown.
   - "Keep as saved", "Renumber in order of use" or "By hand". Duplicates are refused, and the choice is saved in the project.
3. **Checks before writing** *(GC-15)*:
   - Writing is blocked while an enabled operation is stale or in error.
   - Operations not simulated, and unconfirmed machine values, give warnings.
   - Disabled operations are listed as not posted.
   - A sentence states the rule.

**Right column:**

- the files to write, each with its name (decision 0028 item 1), setup, line count and programmed time;
- a preview of the first lines.

**Safety notice band** (NFR-12; GC-16):

- **Always shown** at export, in an amber box. The text is fixed: "Verify this G-code before you run it. It was produced from your project and has not been tested on your machine. Check the origin, axis height, tool lengths, A direction and unwinding at the controller, and run an air cut first."
- **The tick box** "I will verify this G-code before running it on a machine" must be ticked before the first export of a project on a computer.
- **Where it is remembered.** In the user's settings on that computer (section 13), not in the project. Opening the project on another computer asks again, and a sentence under the box says so.
- **Once ticked,** the box stays visible, ticked, with "Acknowledged for this project on this computer".

**Footer:** "Export setup sheet (HTML / PDF)" (GC-12) on the left; Cancel, and "Write G-code" (primary) on the right. "Write G-code" is disabled until the checks pass and the box is ticked, and its tooltip says which is missing.

### 11.7 Workholding and retaining tabs

UI-25, with SET-09 and OP-21 ([decision 0104](../decisions/0104-held-regions-and-tabs-in-the-interface.md); the data is decision 0041, the defaults and checks decision 0035 and its owner's answers, the wiring decision 0045). Board "Workholding and tabs" (section 20). The viewport display is section 7.5.

**Where it lives.**

- **Inspector.** The fifth section of the setup inspector (11.3), "Workholding and tabs", after "Work offset and clearance". Its header has a count chip ("1 held · 2 tabs", or "None") and, while the section has unapplied changes, a second chip "Not applied".
- **Tree.** A child row of the setup (section 6), "Workholding", after "Work offset and clearance", with the same count chip. Choosing it selects the setup and opens the inspector at this section. It carries the Warning mark (section 6) while a setup check (below) finds a problem with a tab, and the Error mark while the stored workholding cannot be built.
- **Steps.** It belongs to Setup. On the other steps the inspector shows the same section when the setup is selected, and editing it works the same way. An applied change marks every operation of the setup stale, as any setup edit does (the operation's key includes its setup, decision 0030 item 1), and the footer primary follows section 5.

**Editing a draft.** Unlike the other inspector sections (section 8, "Committing"), this section edits a draft and stores it with **Apply**, because the checks span several rows and the checks against the part and the tools run as a job (decision 0045 items 4 and 8).

- A field's value goes into the draft when the field loses focus or on Enter. Esc in a field puts back its last value in the draft.
- **Apply** checks the draft against the part and the tools (a job, with a spinner and Cancel in its place), then stores it as one undoable command, "Edit workholding of Setup 1" in the undo history *(UI-22)*. It is disabled while the draft is unchanged or fails a check, with the reason in its tooltip.
- **Revert** discards the draft and shows the stored values again.
- The draft is kept while the user selects other items and comes back, until it is applied or reverted, or the project is closed. While it has unapplied changes, the footer status says "Workholding changes not applied", and the viewport draws the draft (7.5) with a chip "Showing unapplied workholding" at the top left of the view.
- **Undo** (Ctrl Z, UI-08) undoes an applied change. When an undo, a redo or another edit changes the stored workholding while the draft has unapplied changes, the draft is replaced by the stored values and a note under the header says so: "The stored workholding changed (an undo, a redo or another edit), so your unapplied changes were discarded." The note goes when the draft is next edited.
- Apply is a secondary button (section 5): the footer primary stays the step's own.

**Layout, top to bottom.** Fields follow section 8: a 120-pixel label column, the unit inside the field on its right, the allowed range in the tooltip; lengths and positions in mm to 3 places, angles in degrees to 2 (section 14.3). A segmented control chooses between two exclusive choices.

1. **One sentence** at the top: "Held regions are never machined and links keep clear of them. Tabs keep the part joined to the stock. Both count as part of the finished surface." (SET-09, OP-21.)
2. **Held at the start** and **Held at the end**, one block each, for the low-X and high-X ends of the stock (decision 0041 item 3):

   | Field | Kind | Unit | Default when turned on | Notes |
   | --- | --- | --- | --- | --- |
   | Held | Check box | | Off | Turning it on adds the region; off removes it from the draft |
   | Grip length | Number | mm | 10.000 | How far the chuck grips the stock from that end |
   | Holding diameter | Number, may be empty | mm | Empty | Empty shows the hint "Stock diameter (26.000 mm)" (section 8, hint and source). On stock that is not a cylinder it is required (decision 0045 item 2) |
   | Held span | Read-only | mm | | "X −16.000 to −10.000", worked out from the stock and the grip length |

3. **Head and tail zones**, read-only, from the part's extent against the stored held regions: "Head zone X −10.000 to −8.000 mm (2.000 mm)" and "Tail zone X 8.000 to 16.000 mm (8.000 mm)". A zone with no length reads "No room for a tab". They are worked out as a job when the section opens and after each apply, with a spinner meanwhile, and say "Set the stock first" when there is no stock. Tabs may lie only in these zones (decision 0035, owner's answers).
4. **Retaining tabs**, a header "Retaining tabs (2)", then one collapsible row per tab, numbered by its place in the list ("Tab 1", "Tab 2"; removing one renumbers the rest, and the messages use the same numbers). A collapsed row shows its number, its shape and span ("Frustum · X −10.000 to −8.000"), its zone ("Head") and, when it matches the default, the "Default" chip (below). An expanded row has:

   | Field | Kind | Unit | Notes |
   | --- | --- | --- | --- |
   | Shape | Segmented: Cylinder, Frustum | | Changing to a frustum keeps the diameter as the diameter at the start and makes the far end 1.5 times it; changing to a cylinder keeps the diameter at the start |
   | Start X | Number | mm | Model X of the tab's low-X end |
   | Length | Number | mm | Along X |
   | Diameter | Number | mm | Cylinder only |
   | Diameter at start X, Diameter at end X | Number, two rows | mm | Frustum only. Under them a derived line says which end faces the part: "Small end towards the part", or in `warning` amber "Large end towards the part" |
   | Narrowest | Read-only | mm | The smallest diameter, compared with the tools (below) |
   | Placement | Segmented: On the axis, Around the axis | | |
   | Count | Whole number, 1 to 64 | | Around the axis only; default 2 |
   | Start angle | Number | ° | Around the axis only; default 0.00; from +Y towards +Z, evenly spaced after it (decision 0041 item 4); a hint names the angles, "0.00°, 180.00°" |
   | Offset from axis | Number | mm | Around the axis only; default 5.000; the distance from the rotary axis to each tab's centre line |

   The row's menu (right click, the menu key, or a "⋯" button) has Remove (Del when the row has focus) and Duplicate. Both change the draft only.
5. **Add tab** and **Add default tabs**, side by side under the list. Add tab adds a cylinder on the axis, 5.000 mm long and a quarter of the stock's diameter across (at least 1.000 mm), starting at the inner edge of the held region at the start (or the start of the stock), expanded with its Start X focused.
6. **Check against the part and tools**, a secondary button that runs the same checks as Apply without storing anything, then lists what it found under it, or "The tabs are in the head and tail zones and wide enough for the tools."
7. **Apply** and **Revert**, right-aligned, Apply right-most.

**The default tabs shown as defaults** (decision 0035, owner's answers; decision 0045 item 3; OI-198). The defaults are never applied silently, and the stored data stays explicit (decision 0041 item 2). So:

- **When the setup has no tabs,** the section works out the default tabs (a job, as the zones are) and shows them in the tab list as dimmed rows labelled "Default tab, not added", with their shape, span and diameters, read-only. Over them, an amber Warning line: "No tabs are stored. Without tabs the part is not joined to the stock when its last support is cut." The viewport draws them as in 7.5. "Add default tabs" adds them to the draft as ordinary rows, to be applied or edited.
- **Add default tabs** is enabled only while the draft has no tabs and its held regions are the stored ones (the defaults fit the stored held regions). Otherwise its tooltip gives the reason: "Remove the tabs first" or "Apply the held regions first: the default tabs fit the stored ones."
- **No default.** When there is no room in a zone, no default row is shown for it, and a line says why: "No room for a default tab at the head: the held region reaches the part." When the part ends in a point there, "The part comes to a point at X 8.000 mm, so there is no diameter to base a default tab on; add the tab by hand."
- **After they are added,** a tab whose values all equal, to 0.001 mm, the default for its zone carries the "Default" chip, in its collapsed and expanded row. **Overriding** a default is editing it: as soon as a value differs, the chip goes. Its tooltip says "Same as the default tab for the head zone. Editing a value overrides it." A tab that once was a default but no longer matches is an ordinary tab; nothing records where it came from.

**Validation** (NFR-07; decision 0041 item 5; decision 0045 item 4). Checks on the draft alone run as values are entered; the first problem, in the order of the fields, marks its field with a red border and the reason in red under it, and Apply is disabled. The words are plain (section 15): no field paths, and tabs numbered from 1.

| Check | Marked field | Message |
| --- | --- | --- |
| A length or a diameter is zero or less | That field | "Must be more than 0 mm." |
| Text that is not a number | That field | "Not a number." |
| A grip length longer than the stock | Grip length | "Longer than the stock (32.000 mm)." |
| A holding diameter wider than the stock | Holding diameter | "Wider than the stock (26.000 mm)." On box stock: "Wider than the stock's smaller side (20.000 mm)." |
| No holding diameter on stock that is not a cylinder | Holding diameter | "Needed: box stock has no diameter to hold at." |
| The two held regions overlap | Grip length, at the end | "Overlaps the held region at the start of the stock." |
| A tab outside the stock | Start X | "Not inside the stock (X −16.000 to 16.000 mm)." |
| A tab in a held region | Start X | "Overlaps the held region at the start of the stock." |
| A tab reaching beyond the stock's radius | Diameter (on the axis) or Offset from axis (around it) | "Reaches 14.000 mm from the axis, outside the stock (radius 13.000 mm)." |
| Two tabs on the axis overlapping | Start X, of the later tab | "Overlaps tab 1." |
| A frustum with equal diameters (within 0.001 mm) | Diameter at end X | "The same as the diameter at start X; use a cylinder." |
| A count out of range | Count | "From 1 to 64." |
| An offset from the axis below zero | Offset from axis | "Must be 0 mm or more." |

The checks that need the part and the tools run as a job on Apply and on "Check against the part and tools". A finding marks the tab's row header and puts its reason under it: on Apply with the Error mark, in red, and nothing is stored; on a check with the Warning mark, in amber:

| Check | Message |
| --- | --- |
| A tab not wholly in the head or the tail zone (OI-180) | "Tab 3 (X −1.000 to 1.000 mm) is not wholly in the head or the tail zone. Tabs lie between the held region (or the end of the stock) and the part, or between the part and the other end." |
| A tab narrower than a tool of the setup's operations (OI-159) | "Tab 2 is 3.000 mm across at its narrowest, narrower than the tool T2 Flat Ø6 (6.000 mm) of 1 Rough. Make the tab wider or use a smaller tool." |
| The part, the stock or a tool changed while checking | "The part, stock or tools changed while checking; check again." |
| The draft changed while it was being checked | "The draft changed while it was being checked; apply it again." |

The same findings for the stored workholding are rows of the Setup checks tab (section 9; SET-06), with "Edit workholding" as their action.

**The warning when an operation's selection reaches one** (UI-25; decision 0045 item 7). A selection the operation cuts in that has a triangle corner inside or on a held region or a tab is a warning, not an error: those regions are never cut, so the operation cuts less than the selection names.

- **In the operation's inspector,** under the Geometry role that holds the set, an amber Warning line with the triangle outline: "The selection 'Groove floor' reaches tab 1, which is never machined. This operation leaves it uncut there." Actions: "Show in view" (selects the tab, 7.5, and shows the set) and "Edit workholding" (selects the setup at this section).
- **In the messages panel** (section 9), a warning row with the operation as its source and the same actions.
- **In this section,** under the tab or held region, a muted line names the operations whose selections reach it: "Reached by the selections of 2 Helical finish."
- It does not change the operation's status mark, and it does not block generating or exporting. It goes away when the selection or the workholding no longer reaches. Working it out reads the part, so it runs as a job when the operation or the setup is shown, and after each change to either.

**Keyboard** (UI-14; section 16). Every field, row header, menu and button is reached with Tab in reading order. Enter on a collapsed row header expands it, and Left and Right arrows collapse and expand it. Del on a focused tab row removes it from the draft. Ctrl Enter (Cmd Enter on macOS) anywhere in the section is Apply. Revert has no shortcut, so a stray key cannot discard a draft; Esc keeps its meaning of section 16. Tooltips, including the "Default" chip's, open on focus as well as hover; a focused row's solid is outlined in the view (7.5).

**States.**

| State | What is shown |
| --- | --- |
| No stock | The held region and tab blocks work, but the extent checks and the zones wait; a line says "Set the stock first: held regions and tabs are placed on it." Add default tabs is disabled with the same reason |
| No model | As no stock; the zones and the default tabs need the part: "Import a model first" |
| Nothing stored | The default tabs offered, with the warning line (above) |
| Working out the zones or defaults | A spinner in the zones block and in the tab list, "Working out from the part…" |
| Unapplied changes | The "Not applied" chip, Apply and Revert enabled, the footer note, the draft in the view |
| A draft that fails a check | The marked field and its message; Apply disabled with the message in its tooltip; the solid outlined red in the view |
| Checking | Apply replaced by a spinner and Cancel; fields stay editable, and an edit made meanwhile makes the result stale (message above) |
| A check found problems | The tab rows marked, the messages under them |
| The stored workholding cannot be built | An error banner at the top of the section (section 8), with the reason; nothing drawn (7.5); the tree row's Error mark |

## 12. Supporting windows

### 12.1 Tool library and profile editor

TL-01 to TL-07, TL-11; decision 0014; decision 0029 item 12.

- **Left: the tool list.** A search field, the tools grouped by type, a chip "in project", and New tool and Duplicate.
- **Centre top: the tool form.** Name, type, diameter ("from profile" for a custom profile), flutes, flute length, shank diameter, overall length, material.
- **Centre: the profile editor,** for a custom profile.
  - **Preview.** The revolved silhouette to scale on both sides of the axis. The selected segment and its end points are highlighted. The outline is green when valid and red when not.
  - **Segment table.** Number, type, end (r, z), arc radius or centre.
  - **Editing.** "+ Line", "+ Arc", move up, move down, delete.
  - **The verdict.** Live, naming the failing segment, with the same rules as a profile file.
  - **Files.** Import and export of a profile file.
  - **Apply.** One undoable step on a project tool. Library edits save the library file.
- **Right: data for the tool.**
  - Cutting data by material (TL-05): rpm, feed per tooth, plunge, then stepover and stepdown in detail.
  - The machine tool sets (TL-11): the suggested number for each machine, with a note that the project keeps its own number.
- **Footer.** Import library and Export library (TL-01) on the left; Close, and "Add to project" (primary when opened from "+") on the right.

### 12.2 Selection sets

UI-05; decisions 0032 and 0037; a dock tab.

- **Toolbar.**
  - "New set from selection", "Add selection to set", "Remove from set", "Replace set" (the chosen set takes the working selection) and "Clear" (empties the working selection);
  - the angle for selection by angle (default 30°, decision 0037 item 4; the board shows 15° as an example), and the angle for Same direction (default 10°);
  - a note on what can be selected: triangles and edges on meshes (decision 0066), faces and edges on STEP models;
  - for a working edge selection, and for a chosen edge set of a STEP model, "Tangent edges": "Project default (drop)" or "Project default (keep)", "Keep tangent edges", "Drop tangent edges" (decision 0066 item 9);
  - "Keep tangent edges by default", the project's default (off), until a project and software preferences screen exists to hold it *(OI-260)*;
  - "Warn before a click starts a new selection" (on by default), until a project and software preferences screen exists to hold it *(OI-260)*.
- **Table.** Swatch, name, kind, size, used by (operations and roles), status, action.
  - A set with a problem is red, with its reason as a sentence in the row (not the example "12 missing") and "Reselect". "Reselect" starts a fresh pick: it clears the working selection, shows the set's old selection on the model as a dashed grey ghost, and says "Pick the new selection, then Replace set."; "Replace set" makes the set current again (decision 0125, "As built"; OI-374).
  - Rename and delete are in the row menu. Delete is refused while an operation uses the set, naming the operations.
  - A set's colour can be chosen from the eight swatches, or left as the default swatch of its place in the list (decision 0046 item 2).
- **Checking.** Saving or editing a set in a way that changes what it selects checks it against the model as a job: "Checking the selection against the model…" with Cancel, and the other save and edit buttons disabled until it ends (decision 0112 item 1).
- **Footer line:** "Editing a set marks every operation that uses it stale."

### 12.3 Undo history

*(UI-22)*. A popover from the title bar (Ctrl H).

- **Rows,** newest first: the command in words with its item, and the time.
- **Marks.** The current state is highlighted. Undone steps are italic, marked "undone".
- **Jumping.** Choosing a row undoes or redoes to it.
- **Text.** A line at the foot gives the shortcuts and says that a new edit after undoing discards the undone steps.

### 12.4 About window

NFR-16, NFR-10, NFR-12, UI-14; decisions 0151 and 0152 (accepted). Opened from Help, "About OpenChuckCAM…".

- **A modal dialog** in the main window, like the other dialogs (section 3).
- **From the top:** the name; one line on the licence (AGPL, version 3 or later); the NFR-12 notice; **Build**, the facts of one build in monospace (version, commit, uncommitted changes, build date, Rust version, target, and the version of each crate); **Licences**, a read-only monospace box of about ten rows that scrolls, holding the AGPL, the IBM Plex notice, the SIL OFL 1.1 text and the OpenCASCADE notice, with a first line pointing to `THIRD-PARTY-NOTICES.html` for the third-party crates; **Log file** (NFR-17, decision 0155), the path of the file this run writes its log to (or "This run is not writing a log file.") and an "Open log folder" button that shows the folder in the file browser, with a failure written under the button.
- **Footer.** "Copy build information" on the left (secondary; copies the Build block and nothing else; the window stays open), "Close" bottom right (the one primary button, section 5).
- **Keyboard.** Tab visits the Licences box, Open log folder, Copy and Close in that order; the box scrolls with the arrow keys; The tooltips of Open log folder and Copy open on focus (section 16).
- A fact the build could not find reads "unknown".

## 13. Per-user settings

*(UI-15)*

- **Kept per user on the computer, never in the project:**
  - window and panel layout;
  - scale and theme;
  - display units;
  - recent projects;
  - remembered sections;
  - the export acknowledgements of 11.6.
- **Where.** The platform configuration folder of decision 0029 item 15: `OpenChuckCAM/` beside `machines/` and `tools/`, under `$XDG_CONFIG_HOME` (else `~/.config`), `%APPDATA%` or `~/Library/Application Support`.
- **Format.** One `settings.toml`, written with the `toml` crate already in the workspace. Not eframe persistence, which would bring the `directories` crate that decision 0029 declined.
- **When the file is bad.** A missing or unreadable file gives defaults and a message, never a crash (NFR-07).
- **How a project is identified** for the acknowledgement: by its canonical folder path on that computer (OI-165, closed). Moving or renaming the folder asks again; no project file changes.

## 14. Visual design

### 14.1 Tokens

| Token | Dark | Light | Use |
| --- | --- | --- | --- |
| `bg` | `#1b1d21` | `#e6e9ed` | Window background, splitters |
| `panel` | `#23262b` | `#f6f7f9` | Panels, dock, bars |
| `raised` | `#2b2f35` | `#ffffff` | Buttons, add buttons |
| `field` | `#16181b` | `#ffffff` | Inputs, code, readouts |
| `bar` | `#15171a` | `#dde1e6` | Title bar, footer |
| `line` | `#383d45` | `#c9ced5` | Borders |
| `line-soft` | `#2d3137` | `#e2e5e9` | Row dividers |
| `text` | `#e4e7eb` | `#1d2125` | Text |
| `muted` | `#a3abb5` | `#56606b` | Labels, secondary text |
| `heading` | `#c3c9d1` | `#2f363d` | Section headings |
| `accent` | `#5aa9ff` | `#1d6ad0` | Focus, active step, selection bar, links |
| `selected` | `#264466` | `#d4e5fb` | Selected row and segment |
| `primary` | `#2f6fbd`, border `#4a8fe0`, white text | same | The primary button |
| `ok` | `#4cc38a` | `#1b7f53` | Up to date, confirmed |
| `warning` | `#f2b84b` | `#8a5d00` | Stale, warnings, unconfirmed |
| `error` | `#ff7b72` | `#c22b2b` | Errors, refused values |
| `overcut` (text) | `#e27ad2` | `#9c1f86` | Overcut values in panels |
| Warning box | bg `#2a2316`, border `#6b5420`, text `#f2d28a` | `#fff4d6`, `#e0b44c`, `#5c4300` | Unit question, safety notice |
| Error box | bg `#2e1a1a`, border `#7a3330`, text `#ffb4ae` | `#fde8e7`, `#e59a95`, `#7a1d18` | Error banners |

### 14.2 Themes

Dark is the default. The light theme *(UI-20)* changes the panels, bars and dialogs. The viewport keeps its dark gradient (`#2c3139` to `#1b1e23`) and dark overlays in both themes, so the colour roles of 7.4 keep their contrast.

### 14.3 Type and size

- **Typefaces.** IBM Plex Sans for text and IBM Plex Mono for numbers, as in the mockups (accepted 15 Oct 2026, OI-164). They are bundled with the application under the SIL Open Font License 1.1, on these conditions: ship the unmodified font files with IBM's copyright line and the OFL text (`LICENSES/OFL-1.1.txt`, and a line in the release and nightly notices, which `cargo about` does not produce for files that are not crates); do not subset or otherwise modify them, since a modified font may not be called "Plex"; never sell them on their own; load them from the application, never from Google Fonts at run time (NFR-09). egui's own fonts stay as the fallback for characters Plex does not have.
- **Sizes** at 100% scale:
  - text and values: 12 pixels;
  - section headings and group labels: 11, upper case, with letter spacing;
  - the operation name: 13;
  - result figures: 14;
  - empty-state titles: 18.
- **Numbers** are monospaced with tabular figures, with fixed decimal places per quantity: mm to 3 for positions and 1 to 3 for parameters, degrees to 2, feeds whole.
- **Spacing** is on a 4-pixel grid. Corners have a 3-pixel radius (6 for cards and popovers).
- **Targets.**
  - Buttons are 28 high; icon buttons 28 square; small icon and "+" buttons 22.
  - Check boxes are 14 with a 20-pixel hit area.
  - Rows are 28 to 34 high.
  - The focus ring is 2 pixels of `accent`.

## 15. Wording

- **Style.** Plain British English, in sentence case. Labels are nouns ("Stock to leave"); buttons are verbs ("Generate stale (1)").
- **Leave out internal language.** No requirement IDs, decision or open-item numbers, internal type names, "IR" or "schema" in the interface.
- **Messages** say what happened, why, and what to do, in that order.
- **Units.** Every number carries its unit: "mm", "deg" or "°", "mm/min", "rpm". Dates are written "13 Oct 2026".
- **G-code safety.** Never "safe", "verified", "ready to run" or "machine-ready" for G-code (NFR-12).
- **Time.** "Programmed time" until SIM-08 gives an estimate (decision 0028 item 3).
- **Selecting edges.** The filter reads "Pick", then "Both", "Faces" and "Edges". With the filter on Edges, or a working selection of edges, the tools read "Select one", "Chain" and "Smooth chain"; Same direction is hidden with the filter on Edges, and selects the edge alone on an edge. While picking, Q, W, E and R choose Select one, Flood fill, By angle and Same direction (R added by the owner, decision 0046 item 4). A Shift or Ctrl click of the other kind is refused: "The selection holds faces, so an edge cannot join it: a selection holds one kind. Choose Faces in the box on the view to pick only faces, or click without Shift or Ctrl to start a new selection." A plain click of the other kind asks first, in a dialog "Start a new selection?": "This click picks an edge, and the selection holds faces: a selection holds one kind. Starting a new selection with it clears the 12 faces selected now.", with "Start a new selection", "Cancel" and "Don't show me this warning again".
- **The machine.** Machine values are described by provenance ("confirmed on the machine", "not confirmed on the machine"), not by blame.

## 16. Keyboard and accessibility

*(UI-14 and NFR-15)*

| Keys | Command |
| --- | --- |
| Ctrl N, Ctrl O, Ctrl S | New, open, save project |
| Ctrl I | Import model |
| Ctrl E | Post and export |
| Ctrl Z, Ctrl Y, Ctrl H | Undo, redo, undo history |
| Ctrl K | Search commands *(UI-17)* |
| Alt 1 to Alt 5 | Workflow steps |
| Ctrl 1, Ctrl 2, Ctrl 3 | Collapse the project panel, inspector, dock |
| F, V, S, M | Fit; show or hide; section; measure |
| Numpad 1, 3, 7, 0 | Front, right, top, isometric |
| N | New operation |
| G, or F5 | Generate selected; Shift G, or Shift F5, generates every stale operation |
| Ctrl D, Del, F2 | Duplicate, delete, rename the selected item |
| Alt ↑, Alt ↓ | Move the selected operation earlier or later |
| Q, W, E, R, B | Select one, flood fill, by angle, same direction, box (while picking); on an edge, Q, W and E are select one, chain and smooth chain |
| Ctrl Enter (Cmd Enter on macOS) | Apply the workholding draft (in its section, 11.7) |
| Space, ←, →, Shift ←, Shift → | Play or pause, step, previous or next operation |
| Esc | Cancel the running job, or leave picking, or close a popover |

On macOS, Ctrl is Command.

- **Keyboard.** Every control can be reached and used with the keyboard, in reading order. Tooltips and help bubbles open on focus as well as hover (the Setup board shows one open on focus).
- **Screen readers.** Icon-only buttons have accessible names. egui's AccessKit support is used, so controls are exposed to screen readers.
- **Contrast.** Text has at least 4.5:1 contrast with its background, and graphics that carry meaning at least 3:1.
- **Colour.** State is never shown by colour alone (section 6 marks, 7.4 line styles). Colour maps differ in lightness.
- **Scale.** The interface works at 80% to 200%, and text is never smaller than 12 pixels at 100% except headings and chips at 11.

## 17. Differences from the current build

Slice 8 (decision 0029, proposed) and the unmerged lane M (decision 0037, proposed) built or proposed some things differently. The owner agreed these resolutions with decision 0038 (15 Oct 2026); each is work for the lane named in the work breakdown. D13 was added with sections 7.5 and 11.7 (decision 0104) and is open.

| # | Built or proposed | This specification | Resolution |
| --- | --- | --- | --- |
| D1 | The rotary axis is drawn orange (0029, owner's run, observation 3) | Cyan; amber is for cut moves | Change the viewport's role colour (done by lane X3, [decision 0119](../decisions/0119-slice-22-viewport-drawing.md)) |
| D2 | The NFR-12 notice is always in the status bar (0029 item 8) | Kept in short form in the footer, and in full at export with the acknowledgement | Keep both; the mockups omit the footer form and are to be read with this section |
| D3 | Parameters are applied on focus loss, with Reset (0029 item 11) | The same, plus a changed-value dot, derived values and typed units (UI-18, UI-19, P2) | No change to P1 behaviour |
| D4 | Inch display is not part of P1 (0029 item 11) | The "mm \| in" switch is P2 (UI-18); with P1 only the footer reads "mm · deg" | As 0029 |
| D5 | A disabled operation's name is struck through (0029 item 10) | Struck through and muted | Keep strike-through; the mockups show muted only |
| D6 | Placement steps have no enable flag (decision 0010 item 6) | No enable box on a step | The Model board's boxes are dropped |
| D7 | Highlights: working selection orange, chosen set green, hover yellow (decision 0037 item 5, accepted 15 Oct 2026) | Working selection blue, each set its own swatch, hover yellow outline | Agreed (OI-163 closed); decision 0038 amends 0037 item 5, and the built overlay colours change with the slice 8 interface items |
| D8 | The machine panel lists the values not yet confirmed (0029 item 16) | A provenance icon on each value with a tooltip (UI-13) | Replace the list with the icons |
| D9 | A tree mark for partly unconfirmed machine values used the stale diamond (the Setup and Model boards) | A Warning mark (amber triangle outline); the diamond means only stale results | Fix the boards when next edited |
| D10 | Steps, the footer primary action and the messages dock are not built | Sections 4, 5 and 9 | New work for lane A or T (decision 0038) |
| D11 | Size and scale are not remembered (0029, owner's run, observation 1) | Remembered per user (UI-15) | New work |
| D12 | The part is coloured in six bands by the worst deviation per triangle: overcut deeper than one cell, within one cell, up to 0.25 mm left, 0.25 to 1 mm, more than 1 mm or only a bound, not measured, with a legend giving each band's triangle count (decision 0040 item 10, accepted) | A diverging scale ordered by lightness with its thresholds at the operation's tolerance, +0.10 and +0.50 mm (section 11.5) | Open (OI-194). Until it is decided, decision 0040's bands stand; the colours of section 11.5 apply to them (overcut magenta, on the surface grey, material left in blues by depth, not measured hatched) |
| D13 | The workholding panel and the display of held regions and tabs (slice 21c item 2, decision 0045 item 8) predate sections 7.5 and 11.7: among other things the panel stands alone in the inspector, shows messages with field paths, offers the default tabs only by a button, and the two roles are opaque shades of blue and amber with no second cue, no legend and no hover | Sections 7.5 and 11.7 | Partly built by lane 21d (slice 21d item 3; OI-325 closed). What remains, mainly the layout parts and the Setup step panes, is OI-334 and goes with slice 22 |

## 18. Traceability

| Section | Existing requirements | Proposed (decision 0038) |
| --- | --- | --- |
| 3 Window | UI-01, NFR-02 | UI-16, UI-17, UI-22 |
| 4 and 5 Steps and primary action | Requirements section 2 workflow | UI-10 |
| 6 Project panel | UI-06, UI-09 | UI-11 |
| 7 Viewport | UI-02, UI-03, UI-04, SIM-01, SIM-02, NFR-03 | UI-21 |
| 7.5 Held regions and tabs | UI-03, UI-25 (decision 0104), NFR-15 | |
| 8 Inspector | UI-07, UI-08, SET-01, UI-13 | UI-18, UI-19, SIM-13 |
| 9 Dock | UI-05, UI-09, PRJ-02, IMP-04, SET-06 | UI-12 |
| 10 Footer | NFR-12 | UI-18 |
| 11.1 Empty state | | UI-23 |
| 11.2 Model | IMP-01 to IMP-06 | |
| 11.3 Setup | SET-01, SET-03, SET-05, SET-06, UI-03, UI-13 | |
| 11.4 Operations | OP-01, OP-02, OP-16, UI-04 to UI-09 | UI-24 |
| 11.5 Simulate | SIM-01 to SIM-07 | SIM-11, SIM-12, SIM-14 |
| 11.6 Export | GC-02, GC-04, GC-05, GC-06, GC-12, GC-14, NFR-12 | GC-15, GC-16 |
| 11.7 Workholding and tabs | SET-09, OP-21, UI-25 (decision 0104), UI-07, UI-08, UI-14 | UI-22 |
| 12 Supporting windows | TL-01 to TL-07, TL-11, UI-05, UI-08 | UI-22 |
| 13 Settings | PRJ-03 (nothing generated or personal in the project) | UI-15 |
| 14 to 16 Visual design, wording, keyboard | NFR-12 | UI-14, UI-20, NFR-15 |

## 19. Open items

Tracked in [open-items.md](../open-items.md):

- **OI-163 (closed 15 Oct 2026):** selection highlight colours are the ones in section 7.4 (D7).
- **OI-164 (closed 15 Oct 2026):** the typeface is IBM Plex Sans and Mono, bundled under the OFL (section 14.3).
- **OI-165 (closed 15 Oct 2026):** the export acknowledgement is keyed by the project folder path (section 13).
- **OI-166 (closed 15 Oct 2026):** the default scale stays 120%; UI-16 is measured at 100% scale (section 3).
- **OI-194 (open):** the deviation display: decision 0040's six bands by triangle against this specification's continuous scale (D12).
- **OI-340 and OI-341 (closed 18 Oct 2026):** the catch-up edits and the width scale of decision 0114, made here. **OI-342 (open):** the boards re-rendered lane by lane (decision 0114 section E). **OI-345 (open):** UI-13 and section 8 with two provenance states, proposed in decision 0114.
- **OI-325 (closed 18 Oct 2026):** the differences of the workholding panel and display from sections 7.5 and 11.7 (D13), built by lane 21d. **OI-334 (open):** what lane 21d did not build.

## 20. Rendered mockups

Images of the design canvas boards, rendered at 100% scale in headless Chromium (15 Oct 2026; images 03, 13, 14 and 17 again on 18 Oct 2026 by lane X3, [decision 0119](../decisions/0119-slice-22-viewport-drawing.md)), for reference. They show example data; where an image and the text above disagree, the text wins. The board sources (`.dc.html`) are in [mockups/source/](mockups/source/).

| Image | Board | Section |
| --- | --- | --- |
| [00-new-project.png](mockups/00-new-project.png) | New project (empty state) | 11.1 |
| [01-model.png](mockups/01-model.png) | Model | 11.2 |
| [01b-import-dialog.png](mockups/01b-import-dialog.png) | Import dialog | 11.2 |
| [02-setup.png](mockups/02-setup.png) | Setup, with a status tooltip open on keyboard focus | 11.3 |
| [03-operations.png](mockups/03-operations.png) | Operations: one operation generating, one in error | 11.4 |
| [03b-operation-error.png](mockups/03b-operation-error.png) | Inspector for an operation in error | 11.4 |
| [04-simulate.png](mockups/04-simulate.png) | Simulate, with the deviation probe | 11.5 |
| [05-export-dialog.png](mockups/05-export-dialog.png) | Post and export | 11.6 |
| [10-tool-library.png](mockups/10-tool-library.png) | Tool library and profile editor | 12.1 |
| [11-selection-sets.png](mockups/11-selection-sets.png) | Selection sets tab | 12.2 |
| [12-undo-history.png](mockups/12-undo-history.png) | Undo history | 12.3 |
| [13-operations-1366x768.png](mockups/13-operations-1366x768.png) | Operations at 1366 × 768 | 3 |
| [14-operations-light.png](mockups/14-operations-light.png) | Operations, light theme | 14.2 |
| [15-simulate-light.png](mockups/15-simulate-light.png) | Simulate, light theme | 14.2 |
| [16-workholding.png](mockups/16-workholding.png) | Workholding and tabs: the setup inspector section with a draft not applied, a default tab and a refused value (18 Oct 2026) | 11.7 |
| [17-held-regions-and-tabs-view.png](mockups/17-held-regions-and-tabs-view.png) | Held regions and tabs in the view: Setup, Operations with a hovered tab and the selection warning, and Simulate (18 Oct 2026) | 7.5 |

![Operations step](mockups/03-operations.png)

Known gaps in the images: the footer omits the short G-code notice of section 10 (D2), and the tree check boxes are drawn without a tick.
