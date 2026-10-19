# 0123: Slice 22 inspectors: the shared field row, the inspector's header, banner and sections, the setup and model inspectors, and the workholding section's part extent

- **Status:** proposed. OI-366 answered by the owner on 19 Oct 2026 (option A, relayed by the coordinator, decision 0049), and specification 11.3 edited to it. Lane X4, slice 22 ([decision 0114](0114-slice-22-conformance-report.md) section E, accepted; it builds on lane X2, [decision 0117](0117-slice-22-shell-frame.md), accepted). The owner accepts it; until then it is guidance.
- **Date:** 19 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-366 to OI-369 are new. OI-366 is closed by the owner's answer (A, relayed). OI-367 to OI-369 are recorded differences. OI-334 items a, c, d, e, i, j, k (the inspector side) and l are built here; its drawing of the draft waits for lane X3's pull request 126 (OI-355). OI-345 stays proposed: the two icons are built on the two states the profile already has, and neither `requirements.md` nor the specification is edited here. OI-342 is updated: this lane re-rendered image 02's machine section.

Numbering: the coordinator assigned this lane records 0123 and 0124 and OI-366 to OI-369 (OI-365, first assigned here, went to lane X3's pull request 126). This lane uses 0123 and OI-366 to OI-369; record 0124 is unused. The disabled-operation Generate fallback that lane X2 found (its proposed OI-352) is lane X3's, by the merge manager's assignment, not this lane's.

## Context

Decision 0114 listed what of the inspector (specification section 8) and the step inspectors (11.2, 11.3, 11.4, 11.7) is not built: C10, C14, C15 and C16, and the workholding items a to e and i to l of OI-334, and it gave them to lane X4. Lane X2 built the window, the tree, the overview query and an inspector frame whose sections were placeholders (decision 0117 item 6). Lane X7a built the tokens. The specification requires that the interface hold no CAM logic: every value it shows comes from a view model in `openchuck-app` (specification section 1; architecture section 5.8). No project format change and no engine change is made (decision 0105 item 4).

## Decision

### 1. One field row for every form (UI-07; section 8)

`apps/openchuck-ui/src/field_row.rs` is the only place a form row is drawn. The operation form, the tool form, the setup sections, the placement steps and the workholding section all use it.

- **Layout.** A 120-pixel label column (`theme::width::LABEL_COLUMN`), the value to its right. The unit is drawn inside the field, on its right, in `muted` (owner's answer 7 to decision 0114: not in a label after it). The allowed range is the field's tooltip ("Allowed: above 0 mm."); the label's tooltip says what the value does.
- **Hint and source.** An empty field shows the value that applies with its source: "1200 · from the tool's cutting data", "0.010 · default", or the machine's clearance ("20.000 · the machine's").
- **Committing.** Typed text is kept in a `FieldState` and committed on Enter or when the field loses focus; Esc puts back the last applied text. The caller applies the value as one undoable command and reports the outcome.
- **Refusals.** A refused value stays in its field with a red border and the reason in red under it, as a sentence ending "Not applied." The project and the undo history are unchanged.
- **Reset.** A value the operation sets has ↺ at the right end of the label column (decision 0029 item 11 makes Reset P1). The 6-pixel accent dot and the Basic and Advanced switch are UI-19 (P2) and are not built.
- **Choices.** Two to four exclusive choices are a segmented control (`selected` fill); more are a drop-down; flags are check boxes.
- **Numbers** have the fixed places of 14.3: mm to 3, degrees to 2, feeds whole. Where the fixed places would show another value (a tolerance of 0.0005 mm), more places are shown, so a stored value is never misread.
- **Sections.** A section header is upper case, in the `heading` colour, collapsible by its arrow or its title, with chips and a status mark after the title. Its open state is remembered per item kind in the per-user settings (UI-15; decision 0117 item 7): "operation", "setup" and "model".
- **Tooltips on focus.** `field_row::tip` opens a tooltip while its widget has keyboard focus as well as on hover (section 16). The provenance icons and the "Default" chip use it.

### 2. The inspector: header, error banner, sections (section 8)

The inspector moves from `shell.rs` to `apps/openchuck-ui/src/shell/inspector.rs`.

- **Header.** The name in an editable field (operations, setups, tools and selection sets; a model is named by its file), and the status chip in the words of the status mark (decision 0117 item 2). Renaming is one undoable command; an empty name is refused under the field. The F2 dialog uses the same call.
- **Error banner** in the error box's colours: what cannot be done, the cause from the item's status, and the actions that fix it. An operation in error (board 3b) shows "This operation can't be generated", "Reselect in the view" (primary in the banner) and "Show missing" when one of its sets has a problem, and the closing line "Export is blocked while this operation is in error. Turning it off lets the others export." A selection set with a problem and a stored workholding that cannot be built have the same banner.
- **The strategy** is shown by its label (`StrategyChoice.label`), not its registry name (C16).
- **Operation sections** (section 8): Geometry, then Tool and feeds, Pattern and tolerance, Tool axis and A, Entry, links and retract, and Advanced. The view model (`openchuck_app::form::Field::section`, `FormSection`) places each parameter by its name; a parameter it does not name is shown with the pattern. Geometry, Tool and feeds, Pattern and tolerance, and Tool axis and A are open by default.
- **Geometry roles** are labelled Cut on, Contain within, Avoid and Check (`SelectionRole::name`; C10); the stored roles keep their names. A set with a problem has a red border and its count ("40 triangles not found"); see OI-369. Under the role that holds a set whose selection reaches a held region or a tab, the warning of 11.7 with "Show in view" and "Edit workholding" (OI-334 l).

### 3. The setup inspector (11.3; C10, C15)

The five sections of 11.3, each with its tree row's status mark in its header.

- **Machine** (UI-13). `openchuck_app::machine_view::machine_values` gives each value in words with whether it is confirmed on the machine: rotary axis, A range, A+ direction, axis height (machine Z), then travel and the maximum feeds under a collapsed "Travel and maximum feeds". Each has a 20-pixel icon at the right end of its label, in one column: a green tick in a circle when confirmed, an amber triangle when not. These are the two icons the owner chose on the two states the profile already has (decision 0114 answer 4, OI-339); the UI-13 wording that follows is still proposed (OI-345). The tooltip says "Confirmed on the machine." or "Not confirmed on the machine.", then the profile's source text, then, for a value not confirmed, how to confirm it. The unconfirmed list by file field names is gone (C10, C23). The header carries "copy stored in project" whenever the project has a copy. The rest of the panel (the profile chooser, the comparison with Update, the library folder) is unchanged; its wording is lane X7b's.
- **Stock** (SET-03). The shape as a segmented control (Cylinder, Box, Bounds); a change keeps the stock's size where it can and is one undoable command. A cylinder's diameter, length and start X, a box's six coordinates, or the bounds' offset; and the material. Imported stock (SET-04, P2) is shown, not offered. The view model is `openchuck_app::setup_form`, which checks each typed value in plain words.
- **Part position** (SET-05): the offset along X. The specification also listed "rotation about X", which a setup does not store: a rotation about X is a model placement step (IMP-05). The section shows a read-only "Rotate about X" row saying it is set by the model's placement steps. The owner chose to edit the specification to match (OI-366, option A, relayed by the coordinator, decision 0049); specification 11.3 item 3 is edited in this change.
- **Work offset and clearance** (SET-05, SET-06): the register, the zero position "On the A axis (Y0 Z0)" read-only (decision 0011 item 1), the zero's X, and the safe radius and safe Z. An empty clearance field uses the machine's clearance, given as the hint; setting one sets the setup's own clearance.
- **Workholding and tabs**: item 5 below. Its header carries the count chip and "Not applied".

### 4. The Model inspector (11.2; C14)

Sections File, Units, Placement, Mesh check and Source, each remembered.

- **File:** the path, the format, the triangle count and the size.
- **Units:** the unit (a drop-down: "From the file" and five units are more than four choices, so section 8 makes it a list, not a segmented control), with where it came from ("Read in mm, the default: the file names no unit") and the unit question in the warning box.
- **Placement steps** (IMP-05): each row has a drag handle, its number and description, up and down buttons and remove; the step's values are field rows under it, so a step can be edited after it is added. Reordering, editing and removing are each one undoable command.
- **Mesh check:** one row per kind of finding with its count, or a tick; the sentence "The check changes nothing in the model or its file."; "Open full report" opens the whole report in a window.
- **Source:** the content hash, "Replace file…" and "Show in folder". Replacing a file is two new calls, `Application::start_replace_model` (a job that reads, hashes, places and checks the new file with the model's unit and placement) and `Application::replace_model` (one undoable command that keeps the model's ID, unit and placement). Selection sets made on the old file then report it, as for a changed file.
- **Not built:** the Import history tab; the project keeps no history of imports, see OI-367. The Model legend strip is lane X3's (`toolbar::legend`).

### 5. The workholding section (11.7; OI-334)

- **Layout (a, c).** The section is the setup inspector's fifth section; its rows are the shared field rows; shape and placement are segmented controls; the count, start angle and offset are field rows with the angles named under them ("0.00°, 180.00°"). Each tab row has a menu on right click and a "⋯" button with Duplicate and Remove; Del on a focused row header removes it; Enter, Left and Right open and close it. Add tab opens the new tab with its Start X focused. egui has no key event for the keyboard's menu key, so that way to the menu is not built (OI-368).
- **The part's extent (c, d, e).** `Application::start_part_extent(setup)` is a job that reads and places the setup's part and returns `PartFacts`: the extent, and the default tab of each zone for the stored held regions (`workholding::zone_defaults`), whatever tabs are stored, so the "Default" chip still shows after the defaults are added. The section works it out when it opens and when the part, the stock or the stored held regions change, with a spinner and "Working out from the part…". From it:
  - the read-only zone lines, "Head zone X −10.000 to −8.000 mm (2.000 mm)" or "Tail zone: no room for a tab";
  - each tab's zone on its collapsed row;
  - a frustum's "Small end towards the part", or in amber "Large end towards the part";
  - the default tabs offered by zone, and for a zone with none, its line: "No room for a default tab at the head: the held region reaches the part." (or "the stock ends at the part"), or "The part comes to a point at X 8.000 mm, so there is no diameter to base a default tab on; add the tab by hand."
  - The Narrowest line compares the tab with the tools of the setup's operations, `workholding::setup_tools`: "3.000 mm, narrower than the tool T2 Flat Ø6 (6.000 mm) of 1 Rough", in amber.
  - `default_tabs` (the command line's) is built on `zone_defaults` and fails as before.
- **Checks (i, j, k).** The check job returns `WorkholdingFinding { tab, text }`, so each finding goes on its tab's row with the Warning mark in amber (a check) or the Error mark in red (Apply). Findings name the operation by its number and name ("of 1 Rough", `workholding::operation_label`). Cancel cancels the job (`JobHandle::cancel`), and a new check cancels a running one. A field's value goes into the draft on Enter or focus loss, and Esc puts back its last value; text that is not a number stays in its field with "Not a number." and Apply is disabled until it is fixed.
- **Selection warning (l).** Under a held region or a tab, a muted "Reached by the selections of 2 Helical finish." from the overview's messages; in the operation's Geometry section, the warning under its role (item 2).
- **The draft in the view (k).** `WorkholdingPanel::draft_view` gives the view what 7.5 draws: the draft while it is unapplied, its held regions and tabs that fail a check, the default tabs offered, the row with keyboard focus, and the part's extent. Lane X3's pull request 126 adds the drawing (`Viewport::set_workholding_draft`, `set_focused_solid`, `set_selected_solid`, `solid_click`); the wiring into the shell is OI-355 and is made when that pull request merges.
- **The stored workholding cannot be built:** the error banner at the top of the section, with the reason from its status.

### 6. Operations (11.4; C16)

- **New operation** takes the setup and tool of the selected operation, or else the last tool used in machining order, and the selection set shown or selected, in Cut on. `StrategyChoice::takes_selections` (the strategy's `honours_region`) marks a strategy that cannot cut in a selection: the list shows "ignores selections" after it and it takes none. `Application::add_operation_on` adds the operation with its set as one undoable step and refuses a set for a strategy that ignores selections.
- **Not built here:** the progress and Cancel on the operation's own row, and the job's place in the Jobs tab, are lane X2's (decision 0117 item 5). "Use the N matching edges" is UI-24 (P2).

## Alternatives considered

- **A field-row widget per form.** Each form had its own rows (a grid, drag values, `add_sized` labels), which is how the differences of C10 arose. Rejected: one widget, drawn once.
- **Store the operation sections in the strategy schemas** (`openchuck-path`). That changes the library family's API for a question of presentation. Rejected; the application places parameters by name and new ones fall in Pattern and tolerance.
- **Work the zones out in the frame from the stored mesh.** The section would read the model file on the interface's thread (UI-09, NFR-04). Rejected: a job, as the defaults and the checks already were.
- **Keep the defaults job and add a separate extent job.** Two jobs reading the same part. Rejected: one job gives both.
- **Show "28 of 40" in a broken field.** The count found needs the set matched against the changed model, which nothing does today (OI-369).

## Consequences

- **`openchuck-app`** gains:
  - the modules `machine_view` and `setup_form`;
  - `form::FormSection` and `Field::section`;
  - `workholding::{ZoneDefault, zone_defaults, PartFacts, SetupTool, setup_tools, WorkholdingFinding, operation_label}` and `workholding_view::{Zone, zone_lines, tab_zone, no_default_line, narrowest_line}`;
  - `Application::start_part_extent`, `start_replace_model`, `replace_model` and `add_operation_on`;
  - `StrategyChoice::takes_selections`.

  Changed: `start_check_workholding` returns `WorkholdingFinding`s; the narrow-tab findings name the operation by its number; `SelectionRole::name` gives the specification's labels. It is an application crate, not in the library family, so `cargo-semver-checks` does not apply.
- **`openchuck-ui`:** `field_row.rs` and `shell/inspector.rs` are new. `form_panel.rs`, `machine_panel.rs`, `model_panel.rs` and `workholding_panel.rs` are rewritten on the field row. `selection_panel::operation_roles` is replaced by the Geometry section. The run panel's new-operation dialog takes the last tool and the selection.
- **Mock-ups (OI-342).** Image 02's machine section shows the two icons, rendered with the bundled Plex files.
- **Specification and requirements.** `requirements.md` is not edited. Specification 11.3 item 3 is edited to the owner's answer on OI-366. OI-345 (UI-13 and section 8 to two icons) waits for the owner.
- **For other lanes:**
  - X3: the viewport takes `WorkholdingPanel::draft_view` (OI-355).
  - X7b: the wording X7b owns is unchanged here, such as the "parameter {name}:" start of a refusal (C23), the choice words of the operation form ("safe_z") and the machine panel's library actions.
  - X9: the tool library window reuses `field_row`.
- **Work breakdown:** the lane X4 row; slice 8 item 9 (UI-13) and its review box.
