# 0440: Inspector label and entry widths; the dock's show button (lane UI-LAYOUT)

- **Status:** accepted (1 Nov 2026) by the project owner, who saw the revised screenshots, including the setup inspector's Stock shape becoming a drop-down list at the default width. The acceptance was relayed to this lane by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7), as decision 0049 allows. Proposed on 1 Nov 2026 and revised the same day after the owner asked for the changes in items 2, 3 and 6 (the default's source stays in the box and the tooltip carries the whole text).
- **Date:** 1 Nov 2026.
- **Lane/issues:** UI-LAYOUT, [#433](https://github.com/paulhampson/OpenChuckCAM/issues/433) (the dock's show and hide buttons are on opposite sides) and [#434](https://github.com/paulhampson/OpenChuckCAM/issues/434) (inspector labels cut short while the entry boxes are wide).
- **Requirements:** UI-07, UI-14, UI-16, UI-19, UI-26.
- **Amends:** UI specification section 8 ("Layout" of the fields) and section 14's setup layout line, which gave a fixed 120-point label column; section 9's dock rail. Edited in the same change. No requirement or architecture text changes. No project schema change; `openchuck-ui` only, so no library version bump.
- **Open items:** OI-1600 (the Windows rendering of the new widths has not been run).

## Context

Every inspector row gave its label a fixed 120 points, of which the reset slot took 28, so labels longer than about 90 points were cut ("Distance past the stock" is 90 wide with the default font; longer labels in other strategies and the Windows font are wider). The entry box took all the rest of the row, so widening the pane (280 to 520) only widened the boxes. The owner finds the inspector hard to use for this reason.

The dock's hide button is at the right end of its tab row, but the collapsed rail drew its show button first, at the left.

## Decision

1. **The dock's show button is at the right end of the rail**, where the hide button is. The tabs stay at the left. Same command (Ctrl 3), same tooltips.
2. **One label column per form.** The column is as wide as the widest label of the whole inspector form needs (the label in the body font, the reset slot and a gap), but not less than 104 points and not more than the smaller of 220 points and 55% of the room the row has. Every row has the same column, so every box of the form starts at the same x. The widths are measured from the rows themselves, from the frame before; a row that needs more than the column it was drawn with draws the frame again at once, so the form never shows the old column.
3. **Entry controls are sized by kind, and the compact boxes are one width.** Every number box (a number row, any row with a unit, and the tool number) is 168 points, never more than the room, so the compact boxes of a form have the same width, the same left edge and the same right edge at every pane width. A drop-down list is as wide as its longest option or its shown text plus the arrow, never more than the room. A segmented control is as wide as its options and falls back to a drop-down list when it does not fit with two points to spare. A text, name or path box fills the rest of the row. A check box is a check box.
4. **Widening the pane gives the width to the labels first.** The column grows to what its labels need; the compact boxes stay 168; what is left stays empty at the right, and a text box takes it.
5. **Narrow panes.** At 280 the column shrinks toward 104 and its share of the pane; a label that still does not fit is cut with an ellipsis and its whole text is added to its tooltip. A box keeps at least 60 points. (Labels are not tab stops: the entry box beside a cut label keeps its own tooltip and its accessible name, which carries the label's whole text, so a keyboard user hears it; the tooltip on focus of section 16 is unchanged.)
6. **A default's source is in the tooltip.** A compact box can cut the hint it shows in an empty box ("12000 · from the tool's cutting data for oak"). The box is not widened for it: the whole hint, then the allowed range, is the tooltip of the box, on hover and on keyboard focus (UI-14). The hint stays in the box as before. No requirement says the source must be visible in the box; specification section 8 ("Hint and source") says an empty box shows the value that applies with its source, which still holds, and the full text is a tooltip away.
7. **Wrapped value text and rows keep one point clear of the pane's edge.** A wrapped value that ran to the edge, or a row that ended a fraction past it, made the inspector widen by a point when the scroll bar came in (the creeping of issues 178 and 179).
8. **Nothing behavioural changes** (the tooltip gains the hint and now also shows on focus). Field identities, focus, the reset slot (issue 243: it never moves the label or the value), committing, refusals, validation, the tab order and the project are as they were. The change is in `field_row.rs`, `theme.rs` (the widths), `machine_panel.rs` (its value rows use the same column), the call sites in `shell/inspector.rs` and `shell/frame.rs` (the rail).

## Alternatives considered

- **B: labels left, boxes pushed to the right edge.** Easier to scan down the values, but a gap opens between a short label and its box on a wide pane. Not chosen by the owner.
- **C: the label above the box below about 320 points.** No cut labels at all, but taller rows and more scrolling at 280. Could follow if 280 stays cramped.
- **A column per section.** Tried in the first build; the owner asked for the boxes of a form to align, which needs one column.
- **Wrapping labels to two lines.** Rows would change height with the width, which the inspector avoids (issue 243).

## Known costs

- In the setup inspector the machine section's long labels ("Tool length measurement") set the column for the whole form, so at the default width the Stock shape is a drop-down list and not a segmented control.
- At 280 and 340 a label of more than about 130 points is still cut; the tooltip has the rest. At 340 this includes the setup inspector's "Tool length measurement", which sets the column and is just over the 55% share (a test found it once the cut detector read the galley's `elided` flag, not the text). The owner decided on 1 Nov 2026 (relayed) to keep the 55% share and accept this cut: the full label shows at 520 and in its tooltip.
- A long default's source is cut in the box and read in the tooltip.

## Verification

`apps/openchuck-ui/src/field_row.rs` (unit tests: the column fits the widest label and is shared; the compact boxes share width and edges and a text box fills; the narrow share; the hint is in the tooltip on focus), `shell/inspector_layout_tests.rs` (the operation and setup inspectors at 340 and 520 with the pane dragged to its width: no label cut, every compact box one width with shared left and right edges, labels first), `shell/dock_buttons_tests.rs` (the show and hide buttons share the right edge); the existing inspector tests (`frame_tests.rs`, `pending_edit_tests.rs`, `numbox_tests.rs`) are updated where they assumed a 120-point column. Screenshots at 280, 340 and 520 points before and after, taken under software rendering (Xvfb with lavapipe), are in the pull request.
