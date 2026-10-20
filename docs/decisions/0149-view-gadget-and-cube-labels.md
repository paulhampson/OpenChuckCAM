# 0149: The axes gadget at the bottom left and the view cube's flat labels (lane MV)

- **Status:** proposed. The gadget's place and the colour scale's move were decided by the owner on issue 157 and in owner decision 3 of the lane (option A, relayed by the coordinator session on 20 Oct 2026); the cube labels follow issue 158. The owner accepts the record in the lane MV session.
- **Date:** 20 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-438 (closed).

## Context

GitHub issues 157 and 158 (trial findings F-16, F-17). UI-02 and UI-03 (the 3D view and its axes), specification 7.2 (overlays) and decision 0119 (viewport drawing, lane X3).

## Decision

1. **The axes gadget is at the bottom left of every 3D view and has no box.** It was a boxed triad under the view cube. The box is gone and the gadget keeps its axes (X red, Y green, Z blue).
2. **On Simulate the colour scale stays at the bottom left but sits above the gadget**, 8 pixels clear of it. Specification 7.2's rows change accordingly; the overlays still never overlap (tested at 1100 × 760 and 900 × 480).
3. **The view cube's labels lie flat on the faces** (Top, Back, Front, Left, Right, Bottom): each is drawn as text mapped onto its face, reads from outside the cube (upright from the front view for the side faces, along X with Y up for the top), and turns and foreshortens with the face. A label is not drawn when its face is within 15 degrees of edge-on (owner, owner decision 3 of the lane): the face is a sliver then, and a label there cannot be read (OI-438, closed).

## Alternatives considered

- **The colour scale to the bottom middle, or the gadget hidden on Simulate.** Rejected: a new place near the readout at 1366 × 768, and a gadget that is not in every view.
- **Billboard text that faces the user.** What the cube did; the trial reported it snapping in and out of view.

## Consequences

- Code: `cube_faces` returns a `CubeFace` with the text directions, `flat_text`, `boxes` and `paint` in `viewport_overlays.rs`.
- Specification 7.2 is edited in this change.
- Tests: `req_ui_02_the_cube_labels_lie_flat_on_their_faces_and_read_from_outside`; the overlay placement check in `req_ui_02_req_ui_16_the_overlays_never_overlap_on_any_step`.
