# 0215: Selection sets move from the dock to the inspector, and Shift and Ctrl clicks change a set directly (lane SELSET; issues 225, 254)

- **Status:** proposed. The owner's answers on issue 225 (everything in the dock for selection sets moves to the inspector and the dock tab is removed) and issue 254 (Shift and Ctrl clicks add to and take from the set directly; a plain click asks first, as in issue 152) are recorded here as given. The specification wording under "Specification entries proposed" waits for the owner's agreement (route A, relayed by the coordinator): the specification is not edited, and nothing is built, until then (OI-870).
- **What it amends:** the interface specification ([decision 0038](0038-ui-specification-and-requirements.md)), sections 1 (board table), 6, 8, 9, 11.4, 12.2 and 15; [decision 0153](0153-selection-usability.md) item 5 (the dock tab kept the sets table and buttons); [decision 0046](0046-slice-10-selection-ui-to-specification.md) item 3 (the table and its "Select in view" action); [decision 0125](0125-slice-22-simulate-and-import.md)'s "Reselect" flow (it becomes "Pick again" for every set). UI-04, UI-05, UI-06, UI-11, UI-14 and NFR-15 are unchanged.
- **Date:** 22 Oct 2026.
- **Open items:** OI-870 and OI-872 are new.

## Context

Trial feedback F-69 (issue 225) and F-83 (issue 254).

- **Issue 225.** Decision 0153 moved the picking options to the inspector's "Selection" section and left the table of sets and the set buttons in the dock's Selection sets tab. The owner's answer: everything in the dock for selection sets moves to the inspector (the instructions, the options, the buttons and the list), and the dock tab is removed. The tree (section 6, UI-06 and UI-11) already lists every set with its swatch, name, size and status mark, and choosing a row already selects the set, shows it on the part and opens its inspector.
- **Issue 254.** Choosing a set shows it in its swatch, but the working selection stays empty, so a Shift click starts a separate selection shown over the set. "Replace set" then gives the set only the new picks, and the faces the user saw are lost. The owner's answer: Shift click adds to the set and Ctrl click takes from it, directly, so what is shown is what the set holds; a plain click asks before it starts a new selection (issue 152); Replace, if it stays, says what it will do and how many faces it drops.

What the tab holds today (`selection_panel.rs`, `SelectionPanel::show`), and where each goes:

| In the tab now | Goes to |
| --- | --- |
| The note on what can be picked (triangles and edges on a mesh, faces and edges on STEP) | Inspector, New selection |
| "Click: …", the keys, "Shift adds. Ctrl removes. Esc leaves" | Inspector, New selection and the set inspector (as instructions) |
| "Pointing at triangle 12" | Dropped: the box on the view already says what the pointer is over (decision 0153 item 2) |
| Working selection count, name field, "New set from selection", "Clear" | Inspector, New selection ("Save as new set") |
| "Add selection to set", "Remove from set" | Dropped: Shift and Ctrl clicks do this directly |
| "Replace set", "Reselect" | Inspector, the set: "Pick again", then "Replace" with a question |
| "Checking the selection against the model…" with Cancel | Inspector, wherever the check was started |
| The table: swatch, name, kind, size, status | Tree (already there) |
| The table: used by | Inspector, the set (already there) |
| The table: "Select in view" | Tree: choosing the row (already there) |
| Row menu: Rename, Delete | Tree row menu (Rename is there; Delete is added) and the inspector header (rename) |
| Set details: colour, tangent edges | Inspector, the set |
| "Editing a set marks every operation that uses it stale." | Inspector, the set |

## Decision

1. **The dock's Selection sets tab is removed** (issue 225). The dock's tabs are Messages and Jobs on every step, with Setup checks on Setup and Playback on Simulate. Everything that opened the tab (the tree's Selection sets "+", choosing a set, "Show missing", the link of a selection warning) opens the inspector instead.
2. **The tree is the list of sets.** It already shows each set's swatch, name, size chip and status mark with its reason. Its row menu gains **Delete** (and the Del key on a chosen set row), refused while an operation uses the set with the sentence the tab used ("“Pocket” is used by Roughing. Take it out of Roughing first."). Choosing a row selects the set, shows it on the part and opens its inspector with picking on, as now.
3. **The inspector while picking with no set chosen: "New selection"** (the Selection sets "+", or a plain click that starts a new selection). From the top:
   - the heading **New selection**;
   - **How to pick**, as plain lines: "Click the part to select. Shift-click adds. Ctrl-click takes away. Esc stops picking." and "Choose the tool in the box on the view, or press Q, W, E or R." and the note on what can be picked ("A mesh is picked by triangle or edge; a STEP model by face or edge.");
   - the working selection: "12 triangles selected" (or "Nothing selected yet"), a name field (hint "Selection 3"), **Save as new set** and **Clear**;
   - while a new set is checked: "Checking the selection against the model…" with **Cancel**;
   - the **Selection** section of decision 0153 item 5 (the two angles, tangent edges, "Keep tangent edges by default", "Warn before a click starts a new selection"), unchanged.
4. **The inspector of a chosen set.** From the top:
   - the header (the name in an editable field, the status chip) and "40 faces on part.step";
   - the error banner with **Pick again** for a set with a problem (in place of "Reselect in the view");
   - **Used by**: each operation and its roles, or "No operation uses it.";
   - **Change this set on the part**, as plain lines: "Shift-click adds to this set. Ctrl-click takes away from it. A click without Shift or Ctrl asks before it starts a new selection." and "Editing a set marks every operation that uses it stale.";
   - **Pick again** and, once something is picked, **Replace** (item 6);
   - while a change is checked: "Checking the selection against the model…" with **Cancel**;
   - **Colour**: the swatches and "Default" (decision 0046 item 2);
   - **Tangent edges**, for a set of a STEP model's edges (decision 0066 item 9);
   - the **Selection** section, as in item 3.
5. **Shift and Ctrl clicks change the chosen set directly** (issue 254). While a set is chosen and picking is on:
   - **Shift-click adds** what the click picks (one triangle, face or edge, or the region of Flood fill, By angle or Same direction) to the set; **Ctrl-click takes it away**. No question is asked: the change is the one asked for. The set is drawn as it now is (in its own colour; decision 0216), so what is shown is what the set holds. The working blue is not used while a set is chosen.
   - Each change is one undoable step ("Add 3 faces to 'Pocket'"), checked against the model as every set edit is (decision 0112 item 1), and marks the operations that use the set stale. Clicks made while a check runs are joined, in order, into the next change when it ends, so none is lost and the set is never written twice at once. Cancel drops the change being checked and those waiting.
   - A click of a kind the set cannot hold (an edge on a face set) is refused with the sentence of section 15, and the set is unchanged.
   - A set with a problem (made on an older version of the model) cannot be added to or taken from (decision 0125): a Shift or Ctrl click is refused with "This set no longer matches the model. Use Pick again to make it match." and nothing changes.
   - **A plain click asks first** (issue 152; decision 0153 item 3): "Start a new selection?", "This click starts a new selection. 'Pocket' stays as it is. Hold Shift to add to the set, or Ctrl to take from it.", with **Start new selection**, **Cancel** and "Don't show me this warning again" (the project setting "Warn before a click starts a new selection"; with it off, the click starts the new selection at once). Starting one leaves the set as it is, un-chooses it and shows the inspector of item 3 with the click's pick.
   - **Esc** stops picking and leaves the set chosen and unchanged.
6. **"Replace set" becomes "Pick again", for every set.** "Pick again" keeps the set chosen, shows its current selection as the dashed grey ghost of decision 0125 and starts an empty pick for it; the inspector says "Pick the new selection, then Replace." Clicks, Shift and Ctrl now build that pick (drawn in the working blue over the ghost), not the set. **Replace** asks first, naming what changes: "Replace 'Pocket'?", "It holds 40 faces. The new selection has 12: 31 are dropped and 3 are added." with **Replace** and **Cancel** (this question is not turned off by the warning setting: it is an explicit button that drops faces, issue 254). Replace makes a set with a problem current again, as before. Esc or "Cancel pick" ends the pick and leaves the set as it was.
7. **"Add selection to set" and "Remove from set" are removed**; Shift and Ctrl clicks replace them. "New set from selection" is renamed "Save as new set".
8. **Keyboard** (UI-14). Every button of items 3 to 6 is in the inspector and reached with Tab in reading order; the tree rows are reached as now, with the menu key for Rename and Delete. Q, W, E, R and Esc work as before (section 16).
9. **No project format change** and no library change: the set edits are the existing `SelectionEdit::Add`, `Remove` and `Replace` of `openchuck-app`, and the warning setting is the existing project setting. No schema number is claimed.

## Specification entries proposed

To be made in `docs/ui-design/ui-specification.md` when the owner agrees (OI-870), and not before.

- **Section 1, board table:** "Selection sets (dock tab)" reads "Selection sets (now in the inspector, decision 0215; the board predates it)".
- **Section 6, "+" table:** Selection sets: "Opens Operations with picking on and the inspector's New selection (12.2)." **Context menu:** "… Delete (operations and selection sets; a set is refused while an operation uses it, naming them) …".
- **Section 8, "While picking" paragraph** is replaced by: "**While picking** (Operations, 11.4) the inspector shows New selection, or the chosen set, with how to pick, the set's actions and the "Selection" section of the options of picking: "select by angle within" (default 30°, decision 0037 item 4), "same direction within" (default 10°), "Tangent edges" for a working edge selection, "Keep tangent edges by default" and "Warn before a click starts a new selection" (decision 0153). Section 12.2 gives the order."
- **Section 9, Tabs:** "Messages and Jobs, on every step. Setup adds "Setup checks". Simulate adds "Playback" (11.5)." The "**Selection sets:** section 12.2" line is removed.
- **Section 11.4, Viewport, While picking:** "… Shift adds, and Ctrl (Command on macOS) removes: to the working selection, or, when a set is chosen, to the set itself (12.2)." **Generating:** "Jobs tab" stays.
- **Section 12.2** is replaced by:

  > ### 12.2 Selection sets
  >
  > UI-05; decisions 0032, 0037, 0153 and 0215. In the inspector; the tree (section 6) is the list of sets.
  >
  > - **The list.** The tree's Selection sets group: each set's swatch, name, size and status mark with its reason. Choosing a row selects the set, shows it on the part and turns picking on. Rename and Delete are in the row menu; Delete is refused while an operation uses the set, naming the operations.
  > - **New selection** (the "+", or a plain click that starts a new selection). The inspector shows, from the top: "New selection"; how to pick ("Click the part to select. Shift-click adds. Ctrl-click takes away. Esc stops picking.", the keys Q, W, E and R, and what can be picked: triangles and edges on meshes, faces and edges on STEP models); the count selected, a name field, "Save as new set" and "Clear" (empties the working selection, without asking); the options of picking (below).
  > - **A chosen set.** The inspector shows, from the top: the header; the size and model; the error banner of a set with a problem; Used by (operations and roles); how to change it ("Shift-click adds to this set. Ctrl-click takes away from it. A click without Shift or Ctrl asks before it starts a new selection."); "Pick again"; the colour, from the swatches or "Default" (decision 0046 item 2); the tangent edges choice of a set of a STEP model's edges, "Project default (drop)" or "Project default (keep)", "Keep tangent edges", "Drop tangent edges" (decision 0066 item 9); the options of picking; and "Editing a set marks every operation that uses it stale."
  > - **Changing a set on the part.** While a set is chosen, Shift-click adds to it and Ctrl-click takes from it, directly, each one undoable step, so what is shown is what the set holds. A plain click asks "Start a new selection?" first, unless the warning is turned off (section 15). A set with a problem cannot be added to or taken from.
  > - **Pick again.** Shows the set's current selection as a dashed grey ghost and starts an empty pick for it: "Pick the new selection, then Replace." "Replace" asks first, with the counts it keeps, drops and adds ("Replace 'Pocket'? It holds 40 faces. The new selection has 12: 31 are dropped and 3 are added."), and makes a set with a problem current again (decision 0125).
  > - **Options of picking.** The "Selection" section: the angle for selection by angle (default 30°, decision 0037 item 4; the board shows 15° as an example) and the angle for Same direction (default 10°); "Tangent edges" for a working edge selection; "Keep tangent edges by default", the project's default (off), until a project and software preferences screen exists to hold it *(OI-260)*; and "Warn before a click starts a new selection" (on by default; it also turns off the question before a Pick filter switch clears the selection), until such a screen exists *(OI-260)*.
  > - **Checking.** Saving or changing a set in a way that changes what it selects checks it against the model as a job: "Checking the selection against the model…" with Cancel, and the other save and change buttons disabled until it ends (decision 0112 item 1). Clicks made on a chosen set while a check runs are joined into the next change.

- **Section 15, Selecting edges:** the last sentence reads "The "Clear" button of New selection and Esc (which ends picking and keeps the working selection, or the chosen set) do not ask (decision 0153). While a set is chosen, a plain click asks "Start a new selection?": "This click starts a new selection. 'Pocket' stays as it is. Hold Shift to add to the set, or Ctrl to take from it." (decision 0215)."
- **Section 20:** the row of `11-selection-sets.png` reads "Selection sets tab (before decision 0215)".

## Alternatives considered

- **Keep the Add, Remove and Replace buttons and colour the pending change** (the alternative in issue 254). Rejected: the owner prefers the direct behaviour, and two selections on screen at once is what caused the lost faces.
- **Direct Shift and Ctrl only on a new selection, with buttons kept for a set.** Rejected: the owner's answer gives the direct rule for editing a set.
- **Load the set into the working selection and write it on a "Save" button.** Rejected: what is shown would again differ from what the set holds until saved, which is the confusion of issue 254.
- **Keep a slim dock tab with only the table.** Rejected by the owner's answer on issue 225 (everything moves); the tree already lists the sets.
- **Drop Replace entirely.** Rejected: a set made on an older model version can only be made current by replacing it (decision 0125), and starting a fresh pick for a set is quicker than taking every face away.

## Consequences

- `apps/openchuck-ui`: `DockTab::SelectionSets` and `Shell::sets_tab` go; `SelectionPanel::show` is split into the inspector's New selection and set views, called from `shell/inspector.rs`; the click handling gains the chosen-set path (Shift and Ctrl become `SelectionEdit::Add` and `Remove` on the set, queued while a check runs) and the "Start a new selection?" question; "Pick again" and the Replace question replace the Reselect message flow; the tree's set rows gain Delete. Tests named for UI-04, UI-05 and UI-14 (for example `req_ui_05_shift_click_adds_to_the_chosen_set`, `req_ui_05_ctrl_click_takes_from_the_chosen_set`, `req_ui_04_a_plain_click_on_a_chosen_set_asks_before_a_new_selection`, `req_ui_05_replace_asks_with_the_counts_it_drops`, `req_ui_05_the_dock_has_no_selection_sets_tab`, `req_ui_05_a_set_is_deleted_from_the_tree_and_refused_while_used`).
- `openchuck-app`: no change expected; if the edit queue needs a helper it is added without a breaking change.
- The mockup board "Selection sets (dock tab)" and its image no longer match; redrawing them is OI-872.
