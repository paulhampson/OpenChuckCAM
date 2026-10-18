# 0114: Slice 22 conformance report and component inventory (lane X1)

- **Status:** proposed. Lane X1, slice 22 step 1 ([decision 0105](0105-slice-22-conformance-pass.md), accepted). The owner answered the eight questions of section F (18 Oct 2026). The coordinator session relayed the answers (decision 0049), and the owner confirmed them directly in the lane X1 session. Section F records the answers, and sections D and E are revised to them. Accepting this record is a separate question to the owner.
- **What it changes:** no code, no project format, no requirement and no architecture text. The specification edits the owner agreed (section F, answers 6 and 7) are made in the same change: sections 7.5, 11.2, 11.5, 11.6, 12.2, 15 and 16, and the status line. The UI-13 change of answer 4 is a proposal for the owner, in section F; neither `requirements.md` nor specification section 8 is edited for it.
- **Date:** 18 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md). OI-338 to OI-345 are new. OI-338, OI-339, OI-340, OI-341, OI-343 and OI-344 are closed by the owner's answers. OI-342 and OI-345 are open. OI-334 stays open, and its items are mapped to lanes here (section C).

Numbering: 0113 (lane BX, `claude/lane-bx-cross-platform-f-words`) was the highest record, and OI-337 (the same branch) the highest open item, across `main` and the unmerged branches on 18 Oct 2026.

## Context

Decision 0105 made slice 22 a conformance pass. Its first step is this report. For every screen, panel, dialog and overlay of `docs/ui-design/ui-specification.md` (the specification) and its mock-ups, the report says whether the built interface matches. Each difference gets a proposed fix: change the code, or change the specification with the owner's agreement. A difference that would need a requirement change is raised as an open item, not built.

**What was compared.**
- The code is `main` at `6743c93` (lane 21d merged in pull request 119): `apps/openchuck-ui`, `crates/openchuck-viewport`, and the view models in `crates/openchuck-app`.
- The specification is read as it will stand once pull request 121 merges (lane S11, `claude/lane-s11-closing-items`, [decision 0112](0112-slice-11-closing-items.md), accepted). That pull request adds a row to section 7.4: a hovered edge 2 pixels wide, a selected edge 3 pixels, feature edges 1 pixel. Its code was read on its branch.
- Pull requests 120 (swarf wiring) and 122 (cross-platform F words) touch no interface and do not change this report.

**How it was checked, and what could not be.**
- The comparison is by reading the code against the specification text and the mock-up sources (`docs/ui-design/mockups/source/*.dc.html`) and images. Nothing was built or run, and no screenshot was taken.
- These need a display and were not verified:
  - pixel widths as drawn;
  - contrast ratios;
  - tooltips opening on keyboard focus;
  - AccessKit names;
  - the layout at 1366 × 768;
  - whether Esc in a text field restores or applies the typed text;
  - overlays colliding;
  - the look of the translucent held region.
- Each such item is marked *(display)* below. Step 3 of the slice (screenshots and the smoke test) checks them.

## Decision

### A. Summary

The built interface is the slice 8 shell: a menu bar, the project tree on the left, a properties panel on the right, a status bar, the view in the centre, and floating dialogs. The layout of specification sections 3 to 5, 9 and 10 is not built. That layout is the title bar, the workflow bar with the five steps, the viewport toolbar, the dock, the footer with its primary action, and the tokens and type of section 14.

That gap is not a slice 22 discovery. It is slice 8 items 8 to 12 (UI-10 to UI-16 and NFR-15, all P1). The specification's section 17 records them as D10 and D11, and they have no test today (`cargo xtask trace` shows no test naming UI-10, UI-11, UI-12, UI-13, UI-15 or UI-16).

Within the shell, the pieces vary:

- **Matches or nearly matches:** the export dialog (11.6), the selection tools box and edge-picking wording (7.2, section 15), and the selection-set table (12.2).
- **Partly matches:** the camera and scene (7.3), the inspector fields (8), the import dialog (11.2) and the workholding section (11.7, OI-334).
- **Not built:**
  - the toolbar and four of the five overlays;
  - the dash and hatch line styles;
  - most of the colour table of 7.4 (D1 is still open);
  - the error banner;
  - the Simulate results inspector;
  - the Setup step's two panes;
  - the tool library window;
  - the empty state;
  - the tokens and fonts.

| Spec section | Verdict | Main differences |
| --- | --- | --- |
| 3 Window structure | Not built | C1 |
| 4 Workflow steps (UI-10) | Not built | C2 |
| 5 Primary action | Not built | C3 |
| 6 Project panel (UI-06, UI-11) | Partial | C4 |
| 7.1 Viewport toolbar | Not built (controls in the View menu) | C5 |
| 7.2 Overlays | One of five (selection tools) | C6 |
| 7.3 Scene and camera | Matches, apart from colours | C7 |
| 7.4 Colour roles | Mostly differs; no dash or hatch styles | C8 |
| 7.5 Held regions and tabs | Partial (OI-334) | C9, section C |
| 8 Inspector | Partial | C10 |
| 9 Dock (UI-12) | Not built | C11 |
| 10 Footer | Not built (a status bar) | C12 |
| 11.1 New project | Not built | C13 |
| 11.2 Import dialog, Model step | Partial | C14 |
| 11.3 Setup | Not built (only the workholding panel shows) | C15 |
| 11.4 Operations | Partial | C16 |
| 11.5 Simulate | Partial | C17 |
| 11.6 Post and export | Matches, apart from tokens and wording | C18 |
| 11.7 Workholding and tabs | Partial (OI-334) | Section C |
| 12.1 Tool library | Not built as a window (the profile editor is) | C19 |
| 12.2 Selection sets | Matches, apart from Reselect and a row menu | C20 |
| 12.3 Undo history (UI-22, P2) | Not built; P2 | – |
| 13 Per-user settings (UI-15) | Partial (acknowledgements only) | C21 |
| 14 Visual design (NFR-15) | Not built | C22 |
| 15 Wording | Several breaches | C23 |
| 16 Keyboard (UI-14) | Partial | C24 |

### B. The differences and their proposed fixes

**Kinds of fix:**
- **Code:** change the code to the specification.
- **Spec:** change the specification; needs the owner's agreement.
- **Data:** needs a data or format change, so it goes to a separate lane with its own schema claim (decision 0105 item 4).
- **P2:** the element serves a P2 requirement and stays unbuilt.

Line references are to `main` at `6743c93`.

**C1. Window structure (section 3; UI-01, UI-16).** Code.
- **What is built:** `shell.rs` lays out a top menu panel, a bottom status panel, an optional bottom playback panel, a left tree panel (240 wide, resizable), a right properties panel that shows only when something is selected (320 wide), and the view. Splitters are egui's.
- **Not built:**
  - the title bar with the project name and save state;
  - the menus File, Edit, View, Project, Tools and Help (built: File, Edit, Operations, View);
  - the workflow bar;
  - the 44-pixel viewport toolbar strip;
  - the legend strip;
  - the dock;
  - the footer;
  - the inspector at 340 (280 to 520);
  - collapsing to a 28-pixel rail with Ctrl 1, 2 and 3;
  - reset on double-click;
  - the small-window rule;
  - the remembered layout.
- **Dialogs:**
  - The modal dialogs are inside the window (`egui::Modal`), as specified.
  - Import, export, results and add-operation use the `floating` helper, as specified.
- **Built but not specified:** the menu's "Text size" control, Larger, Smaller and Reset. Their range, 50% to 300%, is wider than the 80% to 200% of section 16 and NFR-15. Code: clamp the range to 80% to 200%.

**C2. Workflow steps (section 4; UI-10).** Code.
- None is built: no steps, no availability with reasons, no green ring, and no Alt 1 to 5.
- It needs a view-model query in `openchuck-app` for each step's availability and reason.

**C3. Primary action (section 5).** Code.
- **What is built:** generating is in the Operations menu and under F5. Export is in File and under Ctrl E.
- **Missing:** the single filled button bottom right and its rule. The rule needs a view-model query that gives the next required step, with its reason when it is disabled.

**C4. Project panel (section 6; UI-06, UI-11).** Code.
- **What matches:** operations can be enabled, moved up and down, and duplicated through undoable commands, and a disabled operation is struck through and weak. These follow UI-06 and D5.
- **Group order and names:**
  - Built: a "Machine" row first, then Models, Setups, "Selections", Operations, and Tools collapsed.
  - Specified: Model, Setups, Selection sets, Tools, Operations, with the machine profile, stock, part position, work offset and clearance, and Workholding as child rows of the setup.
- **No "+" on the group headers.** There is an "Add operation…" button inside the group, and a "Select on the part…" row inside Selections. The specification puts that action on the Selection sets "+".
- **Rows:**
  - not built: the 28-pixel rows; the status-mark column; the chips (tool number, triangle count, work offset); the drag handle; the context menu;
  - not built: Ctrl D, Del, F2, Alt ↑ and Alt ↓;
  - the selected row is egui's selectable label, not a tint with the 2-pixel accent bar.
- **Status marks (UI-11), none built:**
  - Operation state shows as text, "(generated)", "(out of date)" or "(reused)".
  - A set with a problem is red text with a ⚠ tooltip, and its operations carry a red ⚠.
  - There is no up-to-date circle, stale diamond, running ring with a progress bar, warning outline or off ring, and no pinned legend.
  - Models and setups carry no mark.
- **Tools** show the name only; the project tool number chip is missing.

**C5. Viewport toolbar (7.1).** Code.
- **Partly built, in the View menu instead of a strip:**
  - display mode, named "Shaded", "Shaded with edges" and "Wireframe" (specified: Shaded, Edges, Wire);
  - projection, named "Perspective" and "Orthographic" (specified: Persp, Ortho);
  - all seven standard views;
  - "Fit all".
- **Not built:**
  - the F and V keys;
  - Show or hide (part, stock, axes, held regions and tabs with its line, selections, toolpaths);
  - Show findings: problem outlines are always drawn (`viewport.rs:133-159`).
- **On Simulate,** "Show: Stock, Part + path, Both" is a "Stock" check box in the playback panel (`playback_panel.rs:680-695`).
- Section, Measure and Turn part are P2 (UI-21).

**C6. Overlays (7.2).** Code.
- **Selection tools: built, top left, dark, only while picking** (`selection_panel.rs:768-845`).
  - Pick: Both, Faces and Edges; Select one, Flood fill, By angle and Same direction; Chain and Smooth chain on edges. These match.
  - Box is OI-201. R is shown as the Same direction key (C24).
- **Path legend:** not an overlay.
  - It is text in the playback panel (`playback_panel.rs:637`), without Selected and Avoid, and does not collapse under 480 pixels.
- **Not built:** the view cube, the axis triad and the readout (X, Y and Z to 3 places, A to 2, frame and source).
- **Colour scale:** a coloured text row with triangle counts in the playback panel (`playback_panel.rs:654-667`), not a bottom-left box.
- **The held region and tab legend** is drawn bottom left on every step (`viewport.rs:601-654`).
  - That is the colour scale's place. Section 7.5 puts the legend in the Setup legend strip, in the Operations path legend after Avoid, and under the colour scale on Simulate.
  - Its text takes the egui theme's text colour, so a light theme would draw dark text on the dark view (14.2).

**C7. Scene and camera (7.3).** Matches, apart from colours.
- **What matches:**
  - the rotary axis past both ends;
  - the A+ arc from the shared kinematics, with its label an eighth of a turn along;
  - the work coordinate system at the work zero;
  - orbit, pan and zoom on the specified buttons;
  - fit on open and on each standard view;
  - projection switching that keeps the size (`openchuck-app/src/scene.rs:341-445`, `viewport.rs:456-511`, `camera.rs:208-212`).
- **Differences:** the work axes are plain lines, not arrows (code), and the colours are those of C8.

**C8. Colour roles and line styles (7.4).** Code.

| Role | Specification | Built | Fix |
| --- | --- | --- | --- |
| Part | `#b7bfca` | `#9eadc7` | Code |
| Feature edges | `#d3d9e0`, thin | `#141419`, near black (`viewport.rs:24`) | Code |
| Stock outline | `#7fb4ea`, dashed | `#737373`, solid | Code |
| Rotary axis | `#35d0e0`, dash-dot | `#f2991a`, solid (D1 still open) | Code |
| A+ arrow | `#35d0e0` | `#d933bf` | Code |
| X, Y, Z | `#ff7b72`, `#6fd3a8`, `#6fb2ff` | `#e62626`, `#1ab333`, `#2659f2` | Code |
| Cut, link, rapid, entry | Amber solid, green thinner, grey dotted, lilac | Green, yellow, red, cyan, all solid at one width (`playback_panel.rs:49-56`) | Code |
| Tool | Grey body, lilac tip | Grey cutter and shank, no lilac | Code |
| Working selection, hovered face, saved sets | As specified | As specified | – |
| Hovered or selected edge (pull request 121) | 2 px hover yellow; 3 px blue or the set's swatch; 1 px feature edges | As specified on the branch | – (see D) |
| Avoid and check surfaces | Red, hatched | Not drawn in the view | Code |
| Mesh findings | Red outline; holes ringed amber | `#f23333` outlines, no amber ring | Code |
| Clearance and safe Z, travel limit | Green dashed; red hatched band | Not drawn (they belong to the Setup panes, C15) | Code |
| Held region, tab | As specified | As specified | – |
| Interface error red | `#ff7b72` | `#e65050` and other ad-hoc reds in several panels | Code (C22) |

`LineItem` has a colour, an on-top flag, a slot and, from pull request 121, a width. The renderer has no dash pattern, so the dashed, dash-dot and dotted styles of 7.4 and 7.5 and a screen-space hatch need a line pipeline that can draw them (code in `openchuck-viewport`).

**C9. Held regions and tabs in the view (7.5).** Code.
- **What matches:**
  - the colours of both roles;
  - the translucency;
  - the dark outline of the tab;
  - turning with A;
  - not being pickable.
- **What remains** is OI-334's viewport part:
  - the hatch is eight world-space lines each way, not a 6-pixel screen hatch;
  - the outlines are the two end rings only, at hairline width, with no silhouette and not 1.5 pixels;
  - no hover outline or tooltip;
  - no click selection with its 2-pixel accent outline (but see D);
  - not outlines only while picking;
  - on Simulate they are shaded solids, not outlines over the deviation colours;
  - the draft is not drawn, and there is no red outline for a failing tab;
  - default tabs are not drawn as dashed sand outlines;
  - a workholding that cannot be built is a note in the view, not a message.

**C10. Inspector (section 8; UI-07).** Code, apart from the items marked Spec.
- **Header:** the name is a heading, not an editable field, and there is no status chip.
- **Error banner:** none. A failed command opens a modal error dialog, and section 9 allows that only for a command that fails outright.
- **Sections:** the operation form is one flat grid (`form_panel.rs:139-154`), with no collapsible sections and no remembered open state.
- **Geometry roles:** listed first under "Selections", named Machine, Boundary, Avoid and Check (`openchuck-app/src/selection.rs:131-136`). The specification names them Cut on, Contain within, Avoid and Check. Code: rename the labels; the stored roles are unchanged.
- **Fields:**
  - **Matches:**
    - commit on focus loss and Enter;
    - the reason in red under a refused field;
    - an empty field showing the value that applies.
  - **Differences:**
    - no 120-pixel label column;
    - the unit is a label after the field, not inside it;
    - the range is in the label's tooltip, not the field's;
    - the source ("default", "from the tool's cutting data") is a third column, not part of the hint;
    - Reset is a text button in that column, not ↺ in the label column (↺ and the dot are P2, UI-19; a Reset is P1 through decision 0029 item 11, so its place is the code fix);
    - no red border, and no "Not applied." ending;
    - the refusal starts "parameter {name}:" (`openchuck-path/src/error.rs:14`), against section 15;
    - two-to-four choices are drop-downs, not segmented controls;
    - numbers are shown with `{v}`, not the fixed places of 14.3;
    - Esc has no handler, so egui's own focus release may apply the typed text instead of restoring it *(display)*.
  - Units inside an egui text field need a custom widget. The owner keeps the unit inside the field, as specified (OI-340 item 8 not agreed), so X4 builds that widget.
- **Machine panel (UI-13, D8):**
  - It is not a section of the setup inspector. It is a separate "Machine" row (C4, C15).
  - It shows only "A direction". The profile's rotary axis, A range, axis height and travel are not shown.
  - It still lists the unconfirmed values, by their file field names ("rotary_axis, a_sense", `machine_panel.rs:65-74`).
  - There are no provenance icons. Section 8 asks for three states, and the data has two. The owner chose two icons (OI-339 closed). That needs the UI-13 change proposed in section F (OI-345), and X4 builds the icons on the two existing states.
  - **Built and specified:** the profile chooser, the library folder actions, and the comparison with Update.
  - **Built but not specified:** Refresh, "Use this machine", and "Give it an ID (rewrites the library file)". The last one's wording breaches section 15.
  - The "copy stored in project" chip shows only when the copy differs.

**C11. Dock (section 9; UI-12).** Code.
- **Not built:**
  - the Messages tab, with severity, sentence, source link, action link, order, filters and clearing;
  - the Jobs tab;
  - the Setup checks tab;
  - the dock that holds the Selection sets and Playback tabs.
- **Instead:**
  - changed model files open a modal list when the project opens (`shell.rs:212-218`), where section 9 makes them messages;
  - job progress is in the status bar;
  - the selection sets are in the properties panel when "Select on the part…" or a set is chosen;
  - playback is a bottom panel switched by "Show playback".
- It needs a view-model message collection in `openchuck-app` that gathers import, setup check, file check, generation, selection-set, workholding and job messages, each with its source and action.

**C12. Footer (section 10).** Code.
- **The status bar has:**
  - a spinner with "Building the view…";
  - the run's progress;
  - "Unsaved changes.";
  - "Workholding changes not applied.";
  - the last status text;
  - the A direction;
  - the scene's notes;
  - the G-code notice in full (`VERIFY_GCODE_NOTICE`).
- **Section 10 asks for:**
  - "Ready" or the job;
  - a step note;
  - the short notice "Verify G-code before running it" (D2 keeps the short form);
  - "mm · deg";
  - the tolerance;
  - machine warnings linking to Setup;
  - the step's actions.
- The save state belongs in the title bar ("· saved", or a grey dot with "Edited").

**C13. New project (11.1).** Code.
- **Not built:** the centre card, the drop zone, "Import model…" and "Open project…", the step lines, the "None yet" groups, and the dimmed steps and Undo.
- With no project, the tree says "No project open. Use File, Open project or New project."
- Recent projects and dropping a file are P2 (UI-23).

**C14. Import dialog and Model step (11.2).**
- **Path entry:** the path is asked for first in a modal dialog with Browse and "Check", and the floating window opens afterwards. The specification has the path inside the import window. Code.
- **The preview: Spec.** It is drawn in the main view behind the window, as decision 0029 item 17 and section 3 ("A tool window draws no 3D. The import preview stays in the main viewport") say. Section 11.2's "Left: the preview" contradicts its own section 3. Proposed: edit 11.2 to say the preview is the main viewport, with the legend and the size, radius and volume as overlays there (OI-340).
- **Findings:**
  - holes are not ringed amber;
  - no legend;
  - size and radius are text lines, and volume is missing.
  - Code.
- **Units block:** the unit question is amber text, not the amber box. Code.
- **Mesh check:** bullets per problem, not one row per kind with its count, a tick and row selection, and the "changes nothing" sentence is missing. Code.
- **The STL statement:** shown, as specified.
- **Footer:**
  - no sentence that the file is unchanged and its path and hash are stored;
  - "Add to project" is not the filled primary;
  - its place is left of Cancel.
  - Code.
- **Built but not specified:** a Placement orientation choice, which serves IMP-05. Its text says "the orange line is the rotary axis", which becomes wrong with D1. Code: change the words with D1. Spec: add the choice to 11.2 (OI-340).
- **The Model step's inspector** (`model_panel.rs`):
  - **Built:** the size; the unit as a drop-down; the placement steps as an ordered list with remove as one command and all six kinds to add (no enable boxes, as D6); the mesh summary.
  - **Missing:**
    - format and triangle count;
    - the unit's source hint and a segmented control;
    - a drag handle and reordering;
    - editing a step's value after adding it;
    - "Open full report";
    - the source block (hash, "Replace file…", "Show in folder");
    - the legend strip and the Import history tab.
  - Code.

**C15. Setup (11.3).** Code.
- **The inspector:**
  - Choosing a setup shows only the workholding panel (`shell.rs:738-756`). The five sections of 11.3 (Machine, Stock, Part position, Work offset and clearance, Workholding and tabs) are missing, and only the machine has a panel, elsewhere (C10).
  - Stock, part position, work offset and clearance have no interface: they are set through the project files and the command line. UI-06 and SET-01 to SET-06 are met by the application's commands, but section 11.3 needs their sections.
- **The two orthographic panes are not built:**
  - the front view with the chuck, clearance, safe Z, travel limit and dimensions;
  - the end view from +X with "A+ turns the top toward the front";
  - the held regions and tabs drawn in section (7.5);
  - the legend strip.
- **The dock's Setup checks tab** (C11) is not built. Its first check, "Safe Z cannot be checked…" (OI-60), is not shown anywhere in the interface.

**C16. Operations (11.4).**
- **Board 3b, an operation in error, is not built:**
  - no banner, "Reselect in the view" or "Show missing";
  - no "28 of 40" in the broken field;
  - no closing export line.
  - "Use the N matching edges" is P2 (UI-24). Code.
- **The strategy name:** shown by its registry name, "Strategy: axial_finishing" (`shell.rs:881`), where `StrategyChoice.label` holds the words. Code (C23).
- **New operation** (`run_panel.rs:350-405`):
  - takes the first tool, not the last used;
  - does not take the selection;
  - does not mark strategies that ignore selections.
  - Code.
- **Generating:**
  - Progress and Cancel are in the status bar, not on the row. Code (C4).
  - The keys are F5 and Shift F5, not G and Shift G (C24).
- **The selection warning (11.7)** is above the form, not under its Geometry role (OI-334).

**C17. Simulate (11.5).**
- **Results inspector: not built.** Results are text lines in a floating "Results" window (`run_panel.rs:467-519`). Missing:
  - colour by Deviation, Remaining or Overcut;
  - the share within tolerance;
  - the histogram;
  - the table by operation;
  - the resolution control.
- **SIM-07 (P1) has no control in the interface.** The cell size can be set only through the project file. `openchuck-app` has `DEFAULT_CELL` and `FINEST_CELL`, but no setter is offered in the interface. Code, needed for a P1 requirement.
- **Deviation colours:**
  - Decision 0040's six bands stand until OI-194 is decided (D12), drawn in 11.5's colours.
  - They are drawn instead in red, green, yellow, orange, purple and an unhatched grey (`viewport.rs:69-77`). Code.
- **The tree's eye buttons** are "Operations shown" check boxes in the playback panel, with no line saying results do not change. Code.
- **Not built:** "Simulate from" (C2), and the tool's label with A.
- **The playback dock (SIM-04)** is a bottom panel shown when "Show playback" is ticked:
  - **Transport:** to-start and to-end replace previous and next operation, with an operation drop-down to jump.
  - **Speed:**
    - built: 1×, 10×, 100×, 1000× and 10 000×, plus "whole program in 10 s or 60 s";
    - specified: 1×, 10×, 100× and Max.
    - The owner agreed to keep the extra speeds, now in 11.5 (OI-340 item 7); the code adds Max.
  - **Timeline:** a plain slider with a clock, with no segments per operation, no dashed amber for an operation not generated, and no move numbers.
  - **Keys:** Space and the arrows are not bound.
  - **Built and specified:** Go to move, and the feed at the current move.
  - The G-code panel and the probe are P2 (SIM-12, SIM-11).
- **Built but not specified:** "Turn the part with A", Tool and Toolpaths visibility, and a point-budget line. Spec: Turn the part with A is the machine view's A (SIM-02), and the others can go under Show or hide (7.1) once that is built. Code.

**C18. Post and export (11.6).** Matches (lane S, decision 0071).
- **Differences:**
  - The safety band's colours are not the warning-box token (`export_panel.rs:33,815-820`).
  - "Write G-code" uses egui's selection fill, not the `primary` token.
  - Both are Code (C22).
- **Feed mode** is shown read-only (decision 0071 item 8, accepted), and the specification still reads as a choice. Spec: say read-only (OI-340).
- **Wording:** the unconfirmed-values warning lists file field names (`openchuck-app/src/export.rs:251-263`). Code (C23).

**C19. Tool library and profile editor (12.1).**
- **No tool library window exists.** A project tool is edited in the inspector (`shell.rs:781-843`) with its dimensions and the profile editor.
- **The profile editor (TL-03) is built:**
  - the preview on both sides, green or red;
  - the table;
  - add, delete and change of segment kind;
  - the live verdict;
  - Apply as one undoable step.
- **Missing from the editor:**
  - the selected segment highlighted;
  - "+ Line", "+ Arc", move up and move down;
  - profile file import and export;
  - the (r, z) and arc radius columns as specified.
- **Missing from the window:**
  - the list with search and type groups;
  - the "in project" chip;
  - New tool and Duplicate;
  - name, type and material in the form;
  - the cutting data by material (TL-05);
  - the machine tool sets (TL-11);
  - Import library and Export library;
  - "Add to project" from the Tools "+".
- **Requirements:** TL-01, TL-05 and TL-11 are met in the library (files, round trip, the default tool set), not in the interface. No requirement asks for the window, but 12.1 does.
- Code. The owner decided it is built in slice 22 and blocks slice 20 (OI-343 closed). Lane X9 builds it.

**C20. Selection sets (12.2).** Mostly matches.
- **What matches:** the toolbar's three actions, the 30° default angle, the tangent-edge choices and the two project boxes, the table's columns, and the footer line (`selection_panel.rs:861-1170`).
- **Differences:**
  - A set with a problem shows "▲ Problem" with the reason in a tooltip only, and no "Reselect". The reasons are full sentences, not "12 missing". Code.
  - Rename and delete are in a details block, not a row menu. A refused delete says "Take the set out of its operations first" but does not name the operations. Code.
- **Built beyond the specification:** "Replace set", "Clear", a "same direction within" angle (default 10°), a colour choice per set (decision 0046 item 2), and the "Pointing at …" line. The note says meshes are selected "by triangle or edge" (mesh edges, decision 0066). Pull request 121 adds the checking line with Cancel. Spec: add them to 12.2 (OI-340).

**C21. Per-user settings (section 13; UI-15).** Code.
- **Built:** `settings.toml` in the configuration folder holds the export acknowledgements, and a bad file gives a message.
- **Not kept:** layout, scale, theme, units, recent projects and remembered sections (D11).
- OI-248 (the export choices) stays deferred.

**C22. Visual design (section 14; NFR-15).** Code.
- **Tokens:** none set. The interface is egui's default dark theme, with ad-hoc colours in each panel.
- **Fonts:** IBM Plex is not bundled, and `LICENSES/OFL-1.1.txt` and its notice are missing. These are slice 8 item 10, OI-164 (closed).
- **Not applied:** the sizes, the 4-pixel grid, the 28-pixel targets and the focus ring.
- **Scale:** the default of 120% matches (OI-166). The range is C1.
- The light theme is P2 (UI-20).

**C23. Wording (section 15).** Code. Each breach is an internal name or ID in the interface.
- **File field names:**
  - the machine panel (`machine_panel.rs:65-73`);
  - the library comparison (`openchuck-app/src/setup.rs:217-220`);
  - the export warning (`export.rs:251-263`);
  - the parameter refusal (`openchuck-path/src/error.rs:14`), worded in a library crate; the fix is a plain message in the view model, without changing the library error.
- **Registry name:** the strategy (`shell.rs:881`).
- **Requirement IDs:** "(SIM-05)" (`playback_panel.rs:618`), "(TL-05)" (`openchuck-app/src/tool_edit.rs:129`) and "(UI-05)" (`selection_panel.rs:930`).
- **"ID":** in the machine panel.
- **G-code wording is clean.** No safety wording is used, and a test enforces it (`export_panel.rs:1103`).

**C24. Keyboard (section 16; UI-14).** Code, apart from the items marked Spec.
- **Bound:** Ctrl S, Ctrl Z, Ctrl Y or Ctrl Shift Z, Ctrl E, F5 and Shift F5, and Q, W, E, R and Esc while picking.
- **Missing:**
  - Ctrl N, Ctrl O and Ctrl I;
  - Alt 1 to 5;
  - Ctrl 1 to 3;
  - F and V;
  - the numpad views;
  - N;
  - G and Shift G;
  - Ctrl D, Del and F2;
  - Alt ↑ and ↓;
  - Space and the arrows for playback;
  - Esc to cancel a running job.
- **Spec:**
  - R for Same direction was added by the owner (decision 0046 item 4, accepted) and the table was not edited.
  - Proposed: add R to sections 15 and 16, and keep F5 and Shift F5 as alternatives to G and Shift G (OI-340).
- B is OI-201. Ctrl H and Ctrl K are P2 (UI-22, UI-17).
- Keyboard focus order and tooltips on focus are *(display)*.

**Requirement changes.** None of these differences needs a requirement change in itself: each is code or a specification edit. One requirement change follows from the owner's answer 4: two provenance states instead of three. Its wording is proposed in section F (OI-345).

### C. Component inventory

What exists today, what it becomes in the specification, and whether it stays, moves, goes or is new. "Kept" means the logic stays and the drawing may change.

| Built today (file) | Specification place | Action |
| --- | --- | --- |
| Menu bar (`shell.rs` `menu`): File, Edit, Operations, View | Title bar menus File, Edit, View, Project, Tools, Help (3) | Moved and regrouped; "Operations" items go to the Project menu, the footer and the workflow bar |
| Project tree (`shell.rs` `tree`, `operation_row`) | Project panel (6) | Kept and reworked: order, "+" headers, status marks and legend, chips, context menu, setup child rows. The top "Machine" row is removed and becomes the setup's "Machine profile" child |
| "Select on the part…" tree row | The Selection sets "+" (6) | Removed as a row; its action moves to the "+" |
| Properties panel (`shell.rs` `properties`), only when something is selected | Inspector (8), always present, collapsible | Kept and reworked: header, error banner, sections |
| Operation form (`form_panel.rs`) | Inspector sections and field rows (8, 11.4) | Kept; field rows reworked; sections added |
| Operation roles (`selection_panel::operation_roles`) | Geometry section of the operation inspector (8) | Moved into the Geometry section; labels renamed |
| Selection-warning notes (`shell.rs` `Reach`) | Under the Geometry role, in the workholding section, in Messages (11.7) | Moved, and shown in three places |
| Machine panel (`machine_panel.rs`) | Machine section of the setup inspector (8, 11.3) | Moved into the setup inspector; the unconfirmed list is replaced by two provenance icons (D8; OI-339, OI-345) |
| Workholding panel (`workholding_panel.rs`) | Fifth section of the setup inspector, "Workholding" tree row (11.7) | Moved from the whole inspector to its fifth section; the rest is OI-334 below |
| – | Stock, Part position, Work offset and clearance sections (11.3) | New |
| Model panel (`model_panel.rs`) | Model inspector (11.2) | Kept; source block and report link added |
| Tool properties and profile editor (`shell.rs` `tool_properties`, `tool_panel.rs`) | Tool inspector; the profile editor also in the tool library window (12.1) | Kept; the editor is reused in the window |
| – | Tool library window (12.1) | New (OI-343; lane X9) |
| Selection panel (`selection_panel.rs`): tools box, sets table, details | Tools box overlay (7.2); Selection sets dock tab (12.2) | Tools box kept; the table moves from the properties panel to a dock tab, with a row menu |
| Viewport widget (`viewport.rs`) | Viewport with toolbar strip and overlays (7) | Kept; the toolbar, view cube, triad, readout, path legend and colour-scale box are new; the held region legend moves (C6) |
| Renderer (`openchuck-viewport`) | 7.4 line styles | Kept; dash, dash-dot, dotted and screen hatch are new (C8) |
| Status bar (`shell.rs` `status_bar`) | Footer (10) and title bar save state | Replaced by the footer; A direction and scene notes go to Messages or the Setup checks |
| Playback panel (`playback_panel.rs`), a bottom panel | Playback dock tab (11.5); visibility moves to the tree's eye buttons and Show or hide | Moved into the dock; the timeline gains segments |
| Results window (`run_panel.rs` `results_window`) | Results inspector on Simulate (11.5) | Replaced |
| Add-operation dialog (`run_panel.rs`) | New operation menu from the workflow bar and the Operations "+" (11.4) | Kept as a menu; last tool and selection taken |
| Run status (`run_panel.rs` `status`, `badge`) | Row progress and marks (6), Jobs tab (9), footer job (10) | Moved |
| Import: path modal (`shell.rs` `Dialog::ModelFile`) and window (`import_panel.rs`) | One import window (11.2) | Path modal removed; path field and Browse go into the window |
| Export dialog (`export_panel.rs`) | Export dialog (11.6) | Kept; tokens and wording (C18) |
| Model-file warnings modal (`shell.rs` `set_app`) | Messages (9) | Replaced by messages |
| Error modal (`shell.rs` `Dialog::Error`) | Error banner (8) or Messages (9); modal only for a command that fails outright | Narrowed |
| Folder, offer-create and unsaved-changes modals | Modal dialogs (3) | Kept |
| Held region and tab legend overlay (`viewport.rs:601-654`) | Setup legend strip, Operations path legend, Simulate colour scale (7.5) | Moved |
| – | Workflow bar and steps (4), footer primary (5), dock with Messages, Jobs and Setup checks (9), empty state card (11.1), Setup's two panes (11.3), settings beyond acknowledgements (13), tokens and Plex (14) | New |
| "Text size" in the View menu | View > Larger, Smaller, Reset (3) | Kept; range clamped to 80% to 200% |

**Workholding items (specification 7.5 and 11.7; OI-334, the remainder of OI-325).** Each item of OI-334 maps to a lane of section E.

| OI-334 item | What remains | Lane |
| --- | --- | --- |
| Layout (a, b, c) | Fifth section of the setup inspector with the count and "Not applied" chips in its header; "Workholding" tree row with chip and Warning or Error marks; section 8 field rows; a row menu (right click, menu key, "⋯") with Remove and Duplicate; Add tab focusing Start X | X4 (needs X2's inspector and tree) |
| Part extent (c, d, e) | Head and tail zone lines; "Small end towards the part"; per-zone "No room for a default tab" lines and the point message in sentence case; Narrowest compared with the tools. Needs an `openchuck-app` call returning the placed part's extent for a setup as a job (a view-model addition, allowed by slice 22 item 2) | X4 |
| Checks (i, j, k) | Findings on the tab's row with the Warning or Error mark, naming the operation's number; Cancel cancelling the job; Esc restoring a field; storing on focus loss or Enter, not every change | X4 |
| Draft in the view (k) | Draw the draft with the "Showing unapplied workholding" chip; a failing tab outlined red; a focused row outlining its solid | X3 (drawing) with X4 (passing the draft) |
| Selection warning (l) | "Show in view"; under the Geometry role; "Reached by the selections of …" in the section; a Messages row | X4 (inspector) and X2 (Messages) |
| Viewport (m, n) | Screen hatch, 1.5-pixel silhouette outlines, show or hide entry, hover outline and tooltip, click selection opening the inspector, outlines only while picking, outlines on Simulate, legends in their three places, the cannot-be-built message with "Edit workholding" | X3 (Messages part X2) |
| Setup step (o) | The front and end panes with held regions and tabs in section | X5 |
| Verification | The UI smoke test opens the workholding section | X8 |

### D. Edge and outline widths against the mock-ups (decision 0112's check)

Decision 0112 item 3 sets feature edges at 1 pixel, hovered edges at 2 and selected edges at 3. It says these widths follow the 1.5- and 2-pixel outlines of section 7.5.

**What the mock-ups draw.**
- **Feature edges** have no stroke width, which is 1 pixel (`Main.dc.html:173-174`, `Model.dc.html:152-153`, `Setup.dc.html:171,194`, `Simulate.dc.html:167-168`), or `stroke-width="1"` (`HeldRegionsView.dc.html:43,48,53`). **The 1 pixel of 0112 matches.**
- **Hovered and selected edges: no board shows one.** The Operations board's tools box predates decision 0066: it has no Pick filter or Chain, and its only selection is a face fill, `#3d8bff` at 42%, with no outline (`Main.dc.html:175,189-192`). So the 2- and 3-pixel widths have no mock-up to compare with.
- **The nearest marks are in "Held regions and tabs in the view"** (`HeldRegionsView.dc.html`, image 17):
  - a held region's and a tab's own outline is 1.5 pixels (lines 43, 48, 53), **as 7.5 says**;
  - the hatch is a 6-pixel pattern at 1 pixel (lines 39, 44, 49, 54), **as 7.5 says**;
  - the **selected** held region's accent outline is `stroke-width="2.5"` (line 43), where 7.5 says 2 pixels;
  - the **hovered** tab's yellow outline is `stroke-width="2.5"` (line 48), where 7.5 gives no width.
- **The path legend in that board** draws Link at 1.2 pixels against Cut at 2, which fits "thinner". The Operations board draws both at 2 (`Main.dc.html:195-196`).

**Finding.** Between them, the mock-up, 7.5 and 0112 use four widths for one idea: a hovered or selected mark is wider than the plain line.
- The mock-up uses 2.5 pixels for both hover and selection of a solid.
- Section 7.5 uses 2 pixels for a selected solid.
- 0112 uses 2 pixels for a hovered edge and 3 for a selected one.

The rationale in 0112 ("follow the 1.5 and 2 pixel outlines") holds against the specification text but not against the mock-up.

**Agreed by the owner** (answer 6, relayed; OI-341):
- one scale for the whole view: plain lines 1 pixel, role outlines 1.5, **hovered 2**, **selected 3**;
- section 7.5 now gives a selected solid a 3-pixel accent outline and a hovered solid a 2-pixel yellow outline, with the scale as a line of its own (edited in this change);
- the owner re-renders image 17 when lane X3 plans its work (OI-342);
- decision 0112's built widths are unchanged.

### E. How steps 2 to 4 split into lanes

The owner accepted this split and its waves (answer 2, relayed), with two changes. X7 is split so that its wording edits wait for the `shell.rs` rewrite. The data lane is dropped, because provenance stays at two states (answer 4). The tool library window is its own lane, X9 (answer 5 left the choice to this record).

The model column gives the class of model recommended and accepted. The coordinator holds the model names.

| Lane | Scope (differences) | Needs merged first | Boards the owner re-renders before it starts (OI-342) | Model class, and why |
| --- | --- | --- | --- | --- |
| **X2 shell frame** | C1 to C4, C11, C12, C13, C21, the shell parts of C24; slice 8 items 8, 11 and 12 (UI-10, UI-11, UI-12, UI-15, UI-16; answer 1). View-model additions in `openchuck-app`: step availability and reasons, the primary action, item status, the message collection. | Pull request 121 (it changes `selection_panel.rs` and `viewport.rs`, which X2 moves into the dock and the toolbar) | Board 1 Model and board 2 Setup: tree marks (D9: the warning mark, not the stale diamond); board 2's "Workholding" tree row and fifth section | The largest: it restructures `shell.rs` (1,600 lines) and adds the view-model queries the other lanes build on |
| **X3 viewport drawing** | C5, C6, C7, C8, C9 and OI-334 m, n and the drawing of k: the toolbar contents as a widget, the overlays, the colour roles (D1), a dash, dash-dot, dotted and screen-hatch line pipeline in `openchuck-viewport`, the held region and tab drawing, hover and selection at the OI-341 widths, the deviation colours of D12 | Pull request 121 (wide lines) | Board 3 Operations, image 13 (1366 × 768) and image 14 (light): the selection tools box with the Pick filter, Chain and Smooth chain (decision 0066); image 17 at the OI-341 widths | The largest: wgpu and WGSL shader work with offscreen render tests, and NFR-03 frame time to keep |
| **X7a theme and fonts** | The new files of C22 only: the tokens as one theme module, IBM Plex bundled unmodified, `LICENSES/OFL-1.1.txt` and the archive notices (slice 8 item 10) | Nothing | None | Mid-size: mechanical and well specified; the licence and notice steps follow existing patterns (`cargo xtask licences`) |
| **X7b wording and tokens applied** | C23 (plain messages in the view models and panels), the token parts of C18 and C22 applied to existing panels | X2 and X7a (answer 2: no edits to shared interface files during the `shell.rs` rewrite) | Board 5 Export (feed mode read-only, OI-340 item 3) | Mid-size |
| **X4 inspectors** | C10 (machine icons on the two existing states, answer 4), C14 (Model inspector), C15 (inspector sections), C16, OI-334 a to e and i to l, the part-extent call in `openchuck-app` | X2 (inspector frame, tree rows, Messages) and X7a (tokens, so the field rows are drawn once) | Board 2 Setup's machine section (two icons, after the UI-13 change, OI-345) | The largest: the field-row widget is shared by every form, and the workholding draft rules (11.7) are intricate |
| **X5 Setup panes** | C15 panes: the front and end orthographic panes with clearance, safe Z, travel limit, dimensions, and held regions and tabs in section (OI-334 o) | X2 (the Setup step) and X3 (line styles) | Board 2c (image 16): its legend's "Refused in the draft", which 7.5 does not list | Mid-size: new 2D drawing over existing scene data, specified in detail by 11.3 and 7.5 |
| **X6 Simulate and import** | C14 (one import window, the Placement choice), C17 (results inspector, the SIM-07 control, the playback dock), C20 | X2 (dock, inspector); C17's colours after X3 | Board 1b (the preview in the main view, OI-340 item 4; Placement, item 5); board 4 and image 15 (speeds, item 7); the Selection sets board (12.2 additions, item 6) | Mid-size |
| **X9 tool library window** | C19: the 12.1 window, with the list, search, New tool and Duplicate, the form with name, type and material, the profile editor reused with its missing controls and profile file import and export, cutting data by material (TL-05), machine tool sets (TL-11), library import and export (TL-01), "Add to project". View-model additions in `openchuck-app` for editing the library, which must keep the default-tool ledger's rules (decision 0081, TL-12) | X2 (window and Tools "+") and X4 (the field-row widget) | Board "Tool library and profile editor" (image 10), if the owner finds it behind 12.1 when the lane plans | The largest: application-side library editing with persistence rules, plus a large new window |
| **X8 verification** (step 3) | The UI smoke test opening every step, panel and dialog, including the workholding section; screenshots of the reference screens at 1366 × 768 at 100% (UI-16); the *(display)* checks of this report; the 1M-triangle frame benchmark rerun (NFR-03), whose reference-hardware run is the owner's | X2 to X7b and X9 | None (it compares with the boards as re-rendered) | Mid-size: running and recording; the judgement calls are already made |

**Waves:**
1. X2, X3 and X7a once pull request 121 has merged.
2. X4, X5, X6 and X7b once X2 has merged; X5 also needs X3. X9 starts once X2 and X4 have merged.
3. X8 last.

**Mock-ups (answer 8).** Before each lane starts, the boards in its column are re-rendered by the owner, from the private design canvas, as part of that lane's planning. They are not one batch up front, and they do not hold back the other lanes of the wave. Until a board is re-rendered, the specification wins over it (`docs/ui-design/README.md`).

**Step 4 of slice 22:** no project format change and no engine change. It holds for every lane.

### F. The owner's answers (relayed by the coordinator, 18 Oct 2026)

1. **OI-338, who builds the shell: A.** Slice 22's lanes build slice 8 items 8 to 12 (UI-10 to UI-16, NFR-15), and slice 8's review boxes are ticked when they land. The work breakdown's slice 22 Requirements line now says it closes UI-10 to UI-16 and NFR-15 with slice 8. OI-338 is closed.
2. **The lane split: accepted,** with X7's wording edits after X2 and the recommended models (section E).
3. **OI-344, what blocks the first release: A.**
   - Everything serving a P1 requirement blocks slice 20: C1 to C17, C20 to C24 and the workholding items.
   - The tool library window also blocks (answer 5).
   - Specification-only polish follows the release as recorded differences: exact hex on non-role chrome, and the extra playback speeds.
   - OI-344 is closed.
4. **OI-339, provenance: B.**
   - Two icons only, confirmed and unconfirmed, on the existing `Confirmation` states. No data change is made, and the data lane is dropped.
   - OI-339 is closed as two states by the owner's choice, pending the requirement change.
   - The change is proposed here and tracked as OI-345. Neither `requirements.md` nor the specification is edited by this record.
   - **Proposed UI-13 (P1, unchanged priority):**
     - Now: "Show, for each machine profile value, whether it is confirmed on the machine, taken from a vendor profile, or an unconfirmed placeholder, with how to confirm it."
     - Proposed: "Show, for each machine profile value, whether it is confirmed on the machine or not, with where the value came from and how to confirm it."
   - **Proposed specification section 8, "Status icons in the machine panel".** Replace the list "The three icons" with:
     - "**The two icons:** green tick in a circle: confirmed on the machine; amber triangle: not confirmed on the machine."
     - Change the tooltip line to: "**Tooltip.** It opens on hover and on keyboard focus, and gives where the value came from (the date it was confirmed, the vendor's profile, or a placeholder) and, when it is not confirmed, how to confirm it. For example: …" (the example is unchanged).
     - Mock-up board 2 then shows two icons (X4's column).
   - Section 15's machine wording ("from Makera's profile", "unconfirmed placeholder") stays as the tooltip's source text.
   - The verification plan row for UI-13 needs no change beyond the count of states.
5. **OI-343, the tool library window: A.** It is built in slice 22, as its own lane, X9, and blocks slice 20. OI-343 is closed.
6. **OI-341, widths: A.** One scale of 1, 1.5, 2 and 3 pixels; section 7.5 is edited in this change; image 17 is re-rendered by the owner (section D). OI-341 is closed.
7. **OI-340, catch-up edits: items 1 to 7 agreed, item 8 not.** The unit stays inside the field, as specified. The agreed edits are made in this change, and OI-340 is closed:
   1. Sections 15 and 16: R chooses Same direction while picking (decision 0046 item 4); the section 16 row reads "Q, W, E, R, B".
   2. Section 16: F5 and Shift F5 do the same as G and Shift G.
   3. Section 11.6: feed mode is read from the post and shown read-only (decision 0071 item 8).
   4. Section 11.2: the import preview is the main viewport behind the window, not a pane of it (section 3; decision 0029 item 17). The legend and the size, radius and volume are at the view's corners.
   5. Section 11.2: a Placement choice places the model on the rotary axis as it is added (IMP-05); the Model step's placement steps edit it later.
   6. Section 12.2: "Replace set" and "Clear"; the Same direction angle (default 10°); "triangles and edges on meshes" in the note; a colour choice per set (decision 0046 item 2); the checking line with Cancel (decision 0112 item 1).
   7. Section 11.5: speeds 1×, 10×, 100×, 1000×, 10 000× or Max, or the whole program in 10 s or 60 s.

   Plus the 7.5 widths of answer 6, a line in the specification's status naming decision 0114, and its section 19 open items.
8. **OI-342, the mock-ups.** They are re-rendered as part of each applicable lane's planning (section E, the boards column), not as one batch. OI-342 stays open until the last board in that column is re-rendered.

## Alternatives considered

- **One lane for the whole rework,** as the work breakdown had for lane X. Simple, but it puts some 6,000 lines of interface change in one pull request, with no parallel work, and the viewport shader work waits behind the shell. Not recommended.
- **Fix the code to the mock-ups where they differ from the text.** The specification wins over the images (`docs/ui-design/README.md`). Rejected.
- **Report only what is not in section 17.** D10 and D11 are recorded already, but they are most of the gap, and the lanes need them itemised. Rejected.

## Consequences

- `docs/open-items.md`:
  - OI-338 to OI-345 are added;
  - OI-338, OI-339, OI-343 and OI-344 are closed by the owner's answers;
  - OI-340 and OI-341 are closed, their specification edits made in this change;
  - OI-342 and OI-345 are open;
  - OI-334 stays open, with its items mapped to lanes;
  - the stale OI-325 row in the open table is removed (its closed row stays).
- `docs/work-breakdown.md`:
  - slice 22 records the report, the owner's answers, the requirements it closes with slice 8, and lanes X1 to X9;
  - slice 8 says its items 8 to 12 are built by slice 22's lanes;
  - the parallel-work tables and the traceability row follow.
- `docs/ui-design/ui-specification.md` takes the agreed edits of OI-340 items 1 to 7 and OI-341. No requirement, architecture or code text changes here, and the UI-13 wording waits for the owner (OI-345).
