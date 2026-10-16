# 0046: Slice 10 items 1 and 6: the selection interface brought to the UI specification (highlights, set swatches, tools box, keys, table)

- **Status:** accepted (owner, 16 Oct 2026), with the owner's decisions on the choices below. It relies on accepted records [0037](0037-slice-10-picking-and-selection-panel.md) (picking, the modes, the overlay, the panel) and [0038](0038-ui-specification-and-requirements.md) (the interface specification, whose D7 amends 0037 item 5 and which the owner accepted). Where this record chooses a detail the specification leaves open, the choice is listed under "Choices a reviewer may change".
- **Date:** 16 Oct 2026.
- **Branch:** `claude/lane-m-slice-10-picking` (lane M). It needs lanes A and C, both on `main`, and the specification of 0038 (pull request 58).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-199 to OI-201 are new and open; OI-202 is new and closed; OI-171 is narrowed (the outline is built).

Numbering: this record takes 0046 and OI-199 to OI-202 from the orchestrator's assignment, and **project schema 11** for the stored set colour (item 2; the owner chose to store it). The claims table in the work breakdown shifts the expected lanes D and O to 12 and lane S to 13.

## Context

Slice 10 is closed and its picking, hover and panel are on `main` (record 0037). Decision 0038 accepted the interface specification and, in D7, changed what 0037 item 5 chose: the working selection is blue and drawn as a fill, each saved set has its own swatch, and the hovered triangle is a yellow outline. The specification also describes the selection tools box over the viewport (section 7.2), the keys (section 15) and the Selection sets panel as a table (section 12.2). Requirements: UI-04, UI-05, UI-14 (every command has a keyboard path), NFR-15 (state never by colour alone). This record is the lane M part of 0038 item 4's "where the work goes".

## Decision

1. **Colours (specification 7.4; amends 0037 item 5 as 0038 says).** Working selection: blue `#3d8bff` at 40%, a fill. Hovered triangle: a yellow `#ffe066` outline, the three sides of the triangle drawn as a closed line over the surface (depth-tested, so the part hides it where it is behind). A saved set that is shown: its swatch at 40%, a fill. The orange and green of 0037 are gone.
2. **Swatches.** The eight swatches are a fixed list (`#2d6cb5`, `#a04a46`, `#7d5bb5`, `#3f9c8a`, `#b58a2d`, `#6f9a3a`, `#b5568f`, `#7a8591`; the first three are the mockup's). A set takes the swatch at its place in the project's list of sets, so up to eight sets are told apart and the ninth repeats the first. None is the working blue, the hover yellow or the avoid red. **The colour can be chosen and is stored (owner's decision, 16 Oct 2026):** the set file gains an optional `colour`, `#rrggbb` in lower case, written only when chosen (project schema 11; older projects migrate with no change and keep the default swatch). The details of a set offer the eight swatches and "Default". A colour does not change what an operation cuts, so editing it does not mark operations stale. A free colour picker is not built (OI-199).
3. **The tools box (specification 7.2).** While picking, a box over the top left of the viewport has Select one, Flood fill, By angle and Same direction, the current one marked, each with its tooltip. These are the four modes of 0037 item 4 (flood fill is the connected shell; by angle is the smooth patch). The panel no longer repeats them as radio buttons. Box selection is not built (OI-201).
4. **Keys (specification 15; UI-14).** While picking, when no text field has the keyboard and no dialog is open: Q Select one, W Flood fill, E By angle, **R Same direction**; Esc cancels a region pick that is running and, if none is, leaves picking. Section 15 of the specification lists only Q, W, E and B; **R is added by the owner (16 Oct 2026)** and the specification is not edited. The tooltip label of each tool shows its key.
5. **The Selection sets panel (specification 12.2).** A toolbar and a table:
   - toolbar: "New set from selection" (with a name field), "Add selection to set", "Remove from set" (both act on the chosen set), "Replace set", Clear, and the angle for select by angle (default 30°, 0037 item 4);
   - table: swatch, name, kind, size, used by (operations and roles), status, action. A set with a problem is shown with a triangle mark and its reason on hover, in red; one without shows a dot and "Matches model". The action is "Select in view": it makes the set the working selection, so it can be edited, and shows it. A set with a problem has no action (the "Reselect" of the specification is UI-24, P2, with OI-172);
   - details of the chosen set (model, problem, uses, rename, delete) and the footer line "Editing a set marks every operation that uses it stale." Delete is refused while an operation uses the set, as in 0037 item 6.
6. **Where the panel is.** Specification section 9 places the panel in a dock tab; the dock (D10) is not built and belongs to lane A or T. The panel stays in the properties panel on the right, and its table is a self-contained function that moves into the dock tab unchanged (OI-200). Changes to shared UI files are three: the `Viewport` reports whether picking is on, the shell passes keys and draws the tools box, and nothing else.

## Choices a reviewer may change

| Choice | Impact | Options |
| --- | --- | --- |
| Swatch by place in the list as the default, a chosen colour stored (item 2) | **Decided by the owner (16 Oct 2026): store it.** A set without a stored colour still shifts when sets before it are deleted | A free RGB picker (OI-199) |
| Panel stays on the right (item 6) | **Decided by the owner (16 Oct 2026): stays on the right.** The seven-column table scrolls sideways in a 320-pixel panel (OI-200) | |
| "Select in view" loads the set into the working selection (item 5) | **Decided by the owner (16 Oct 2026): as built.** | |
| Hover outline is one pixel (item 1) | **Decided by the owner (16 Oct 2026): accepted as built** (OI-202 closed). | Note for later: a wider line needs a quad-based line in the renderer |
| Same direction key (item 4) | **Decided by the owner (16 Oct 2026): R.** An addition to the specification's key table | |
| The specification points Box selection at "OI-160" | OI-160 on `main` is the workholding defaults of 0035 | Orchestrator or owner to correct the specification's reference; this record uses OI-201 |

## Alternatives considered

- **Keep the radio buttons in the panel as well as the box:** two controls for one choice; the specification has the box.
- **Build the dock now:** not this lane's area (D10), and it would collide with lane A or T.
- **Draw the hover as a translucent fill in yellow:** the specification says an outline so it stays visible over a blue selection fill.

## Evidence

`openchuck-ui` tests: `req_ui_04_hover_is_a_yellow_outline_and_the_working_selection_a_blue_fill` (the overlay holds a blue translucent fill and a closed yellow loop of three sides), `req_ui_05_the_swatches_are_eight_and_none_is_a_highlight_colour`, `req_ui_05_a_chosen_colour_replaces_the_default_swatch`, and in `openchuck-project` `req_ui_05_prj_06_a_chosen_set_colour_survives_save_and_load_and_is_optional`, `req_ui_05_nfr_07_a_bad_set_colour_cannot_be_put_in_the_project` and `req_prj_04_a_schema_10_project_loads_with_default_set_colours`, `req_ui_14_keys_q_w_e_r_choose_how_a_click_selects` (the keys, every tool having one, and Esc leaving picking), and the existing `req_ui_05_a_saved_set_is_highlighted_again_after_the_project_is_reopened` now checking the swatch fill.

## Open items

- **OI-199:** a free colour (any RGB) for a set; the stored colour is chosen from the eight swatches.
- **OI-200:** move the Selection sets panel into the dock tab when the dock exists (D10, lane A or T).
- **OI-201:** box selection (the B key and the box tool); the specification calls it OI-160, which is a different item on `main`.
- **OI-202 (closed, owner, 16 Oct 2026):** the hover outline stays one pixel wide, as built. A wider, quad-based line is a possible later change.
