# 0046: Slice 10 items 1 and 6: the selection interface brought to the UI specification (highlights, set swatches, tools box, keys, table)

- **Status:** proposed. The owner accepts, not the lane. It relies on accepted records [0037](0037-slice-10-picking-and-selection-panel.md) (picking, the modes, the overlay, the panel) and [0038](0038-ui-specification-and-requirements.md) (the interface specification, whose D7 amends 0037 item 5 and which the owner accepted). Where this record chooses a detail the specification leaves open, the choice is listed under "Choices a reviewer may change".
- **Date:** 16 Oct 2026.
- **Branch:** `claude/lane-m-slice-10-picking` (lane M). It needs lanes A and C, both on `main`, and the specification of 0038 (pull request 58).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-199 to OI-202 are new; OI-171 is narrowed (the outline is built).

Numbering: this record takes 0046 and OI-199 to OI-202 from the orchestrator's assignment. It takes **no project schema number**: nothing here is stored in the project (the colour of a set is derived, OI-199).

## Context

Slice 10 is closed and its picking, hover and panel are on `main` (record 0037). Decision 0038 accepted the interface specification and, in D7, changed what 0037 item 5 chose: the working selection is blue and drawn as a fill, each saved set has its own swatch, and the hovered triangle is a yellow outline. The specification also describes the selection tools box over the viewport (section 7.2), the keys (section 15) and the Selection sets panel as a table (section 12.2). Requirements: UI-04, UI-05, UI-14 (every command has a keyboard path), NFR-15 (state never by colour alone). This record is the lane M part of 0038 item 4's "where the work goes".

## Decision

1. **Colours (specification 7.4; amends 0037 item 5 as 0038 says).** Working selection: blue `#3d8bff` at 40%, a fill. Hovered triangle: a yellow `#ffe066` outline, the three sides of the triangle drawn as a closed line over the surface (depth-tested, so the part hides it where it is behind). A saved set that is shown: its swatch at 40%, a fill. The orange and green of 0037 are gone.
2. **Swatches.** The eight swatches are a fixed list (`#2d6cb5`, `#a04a46`, `#7d5bb5`, `#3f9c8a`, `#b58a2d`, `#6f9a3a`, `#b5568f`, `#7a8591`; the first three are the mockup's). A set takes the swatch at its place in the project's list of sets, so up to eight sets are told apart and the ninth repeats the first. None is the working blue, the hover yellow or the avoid red. The colour is derived, not stored: no schema number (OI-199 for a colour the user picks).
3. **The tools box (specification 7.2).** While picking, a box over the top left of the viewport has Select one, Flood fill, By angle and Same direction, the current one marked, each with its tooltip. These are the four modes of 0037 item 4 (flood fill is the connected shell; by angle is the smooth patch). The panel no longer repeats them as radio buttons. Box selection is not built (OI-201).
4. **Keys (specification 15; UI-14).** While picking and when no text field has the keyboard: Q Select one, W Flood fill, E By angle; Esc cancels a region pick that is running and, if none is, leaves picking. Same direction has no key in the specification and gets none.
5. **The Selection sets panel (specification 12.2).** A toolbar and a table:
   - toolbar: "New set from selection" (with a name field), "Add selection to set", "Remove from set" (both act on the chosen set), "Replace set", Clear, and the angle for select by angle (default 30°, 0037 item 4);
   - table: swatch, name, kind, size, used by (operations and roles), status, action. A set with a problem is shown with a triangle mark and its reason on hover, in red; one without shows a dot and "Matches model". The action is "Select in view": it makes the set the working selection, so it can be edited, and shows it. A set with a problem has no action (the "Reselect" of the specification is UI-24, P2, with OI-172);
   - details of the chosen set (model, problem, uses, rename, delete) and the footer line "Editing a set marks every operation that uses it stale." Delete is refused while an operation uses the set, as in 0037 item 6.
6. **Where the panel is.** Specification section 9 places the panel in a dock tab; the dock (D10) is not built and belongs to lane A or T. The panel stays in the properties panel on the right, and its table is a self-contained function that moves into the dock tab unchanged (OI-200). Changes to shared UI files are three: the `Viewport` reports whether picking is on, the shell passes keys and draws the tools box, and nothing else.

## Choices a reviewer may change

| Choice | Impact | Options |
| --- | --- | --- |
| Swatch by place in the list (item 2) | Deleting a set shifts the colour of those after it; no schema number | A stored colour (a project schema number, the user can pick one); a colour from the set's ID (stable, but two sets may share one) |
| Panel stays on the right (item 6) | The seven-column table scrolls sideways in a 320-pixel panel | Wait for the dock; build a minimal bottom dock with this one tab (would collide with lane A or T) |
| "Select in view" loads the set into the working selection (item 5) | The specification does not say what the action does | Only show the set (the row click already does); select the set's operations |
| Hover outline is one pixel (item 1) | Thin on a high-resolution screen | A wider line needs a quad-based line in the renderer (OI-202) |
| Same direction has no key (item 4) | Reached by the box or the panel only | R, or another key; the specification's table lists only Q, W, E, B |
| The specification points Box selection at "OI-160" | OI-160 on `main` is the workholding defaults of 0035 | Orchestrator or owner to correct the specification's reference; this record uses OI-201 |

## Alternatives considered

- **Keep the radio buttons in the panel as well as the box:** two controls for one choice; the specification has the box.
- **Build the dock now:** not this lane's area (D10), and it would collide with lane A or T.
- **Draw the hover as a translucent fill in yellow:** the specification says an outline so it stays visible over a blue selection fill.

## Evidence

`openchuck-ui` tests: `req_ui_04_hover_is_a_yellow_outline_and_the_working_selection_a_blue_fill` (the overlay holds a blue translucent fill and a closed yellow loop of three sides), `req_ui_05_the_swatches_are_eight_and_none_is_a_highlight_colour`, `req_ui_04_keys_q_w_e_choose_how_a_click_selects` (the keys, and Esc leaving picking), and the existing `req_ui_05_a_saved_set_is_highlighted_again_after_the_project_is_reopened` now checking the swatch fill.

## Open items

- **OI-199:** the colour of a set is derived from its place in the list; letting the user choose one needs a project schema number (ask the orchestrator).
- **OI-200:** move the Selection sets panel into the dock tab when the dock exists (D10, lane A or T).
- **OI-201:** box selection (the B key and the box tool); the specification calls it OI-160, which is a different item on `main`.
- **OI-202:** the hover outline is one pixel wide; draw it wider on high-resolution screens if the owner finds it hard to see.
