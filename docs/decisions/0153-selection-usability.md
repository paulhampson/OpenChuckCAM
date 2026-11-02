# 0153: Selection usability from the nightly-build trial (lane BUG-SEL)

- **Status:** accepted (20 Oct 2026) by the project owner, in the lane's own session. Items 3 and 5 record the owner's earlier answers of 20 Oct 2026, relayed by the coordinator session (issue 153: option B; OI-450 and OI-451 confirmed).
- **Date:** 20 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-448 to OI-451 are new and closed (owner-decided, 20 Oct 2026). OI-452 and record 0154 are not used.

## Context

The owner's trial of the nightly build found four selection problems: issues 151 (edge and triangle modes are unintuitive), 152 (the "start a new selection" notice fires only when the kind changes), 153 (selection options are in the lower window) and 159 (the Pick control overlaps the toolbar and does not look like a label with options). The owner decided 151 (each thing its own colour) and 152 (option A, confirm with a warning) before this lane started. Requirements: UI-04 and UI-05 (selection), NFR-15 (state not by colour alone). The specification sections are 7.1, 7.4, 12.2 and 15 (decisions 0037, 0066, 0119). No project format change: the setting "Warn before a click starts a new selection" is the existing project setting.

## Decision

1. **Colours of their own (issue 151).** Roles in `roles.rs`: the triangle or face under the pointer stays yellow `#ffe066`; the edge under the pointer is magenta `#ff4fd8` (`EDGE_HOVER`); a selected triangle or face stays the working blue `#3d8bff` fill; an edge in the working selection is orange `#ff6a00` (`EDGE_SELECTED`). Orange and blue is a pair the common colour blindnesses keep apart; the two hovers differ in lightness. A test requires each edge colour to stand out from the part grey by a contrast ratio of at least 1.4 and the two hovers to differ by the same. Widths still carry state (2 pixels hovered, 3 selected). A saved set keeps its own swatch.
2. **The mode is named (issue 151).** The box on the view shows "Picking: Triangles and edges", "Triangles only" or "Edges only", and under it a line in the hovered thing's own colour: "Over a triangle", "Over a face", "Over an edge" or "Over nothing". The cursor shape is not changed (OI-449).
3. **The question before anything is cleared (issue 152, option A).** A click without Shift or Ctrl, when the working selection is not empty (any kind, any model), and a switch of the Pick filter to one that cannot hold the working selection's kind (Edges while it holds triangles or faces; Faces while it holds edges; Both always can), open "Clear 1,240 triangles?" with the number and kind, a sentence saying what the click or switch does, "Clear" and "Cancel" and "Don't show me this warning again". The explicit "Clear" button, Esc (which ends picking and keeps the working selection) and Shift or Ctrl clicks are exempt. The existing project setting (on by default) turns the question off. A switch of the tool does not clear anything, so it does not ask (OI-450, confirmed by the owner). A region pick that runs as a job asks when its result arrives, naming the count (OI-451, confirmed by the owner).
4. **The Pick control (issue 159).** The box is placed under the toolbar strip (`tools_corner`), so it never overlaps it, at 1366x768 or any size. "Pick" is a muted label and Both, Faces and Edges are one framed segmented control.
5. **The options are an inspector section (issue 153, owner's decision: option B).** While picking, the right-hand inspector shows a "Selection" section (`SelectionPanel::options`, called from the inspector's Picking arm) with the two angles (30° and 10°), the tangent edges choice of a working edge selection, "Keep tangent edges by default" and "Warn before a click starts a new selection". The dock's Selection sets tab keeps the sets table and the set buttons (New set from selection, Add, Remove, Replace, Clear), and a line pointing to the inspector. The sets' own per-set tangent choice stays with the set's details in the tab. Rejected: A keep in the dock, C both (two controls for one setting), D move the table too.

## Specification edits (same change)

7.4: edge colours added to the colour roles. 7.1: the Pick control is a label and a segmented control, below the strip. 8 and 12.2: the options move to the inspector's Selection section while picking. 12.2 and section 15: the warning is "Clear N <kind>?" with Clear and Cancel, and also asks for a plain click on a selection of the same kind and for a filter switch.

## Alternatives considered

- Edge colours near the yellow (orange, amber): rejected, amber is the colour of cut moves and orange-yellow pairs fail for red-green colour blindness.
- Asking for a mode switch only when the tool changes: rejected, a tool switch loses nothing.

## Consequences

- `SelectionPanel` gains `choose_filter`, `set_filter`, `mode_line`, `tools_corner` and the warning helpers; `Reset` becomes an enum of a click and a filter switch; `answer_reset`, `reset_dialog` and `join` change. `shell/frame.rs` passes `tools_corner(...)`.
- Tests: `req_ui_04_*` in `selection_panel.rs` and `roles.rs`.
