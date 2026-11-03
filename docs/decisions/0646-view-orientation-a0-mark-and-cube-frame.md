# 0646: The viewport marks A0 on the part, names the frame of the view cube, and shows the part as designed outside Simulate (UI-02, UI-03, SIM-02; issue #531; lane VIEW-ORIENT)

- **Status:** proposed by lane VIEW-ORIENT on 3 Nov 2026. The owner's answers (A0 line on the part plus the arc start relabelled; the cube caption always shown) were relayed by the coordinator. Not accepted until the owner says so (decision 0049).
- **Date:** 3 Nov 2026.
- **Number:** provisional 0646; assigned by the Merge Manager at release.
- **Issue:** [#531](https://github.com/paulhampson/OpenChuckCAM/issues/531).
- **Builds on:** [0149](0149-view-gadget-and-cube-labels.md) (view cube), [0168](0168-simulate-controls-and-stock-simulation.md) ("Turn part with A"), [0238](0238-simulate-display-and-operation-navigation.md) item 5 (the cursor is kept on leaving Simulate).
- **Open items:** none opened. No project schema change; no change to toolpaths or G-code. One new `SceneRole` variant in `openchuck-app` (the enum is `#[non_exhaustive]`).

## Context

After a simulation, with the part turned with respect to the machine, it is easy to lose orientation. The owner asked for (1) a mark of where A0 is, and (2) an indication on the view cube of whether its faces refer to the machine or to the part. The owner also noted that the part seems to stay in its simulated orientation outside Simulate.

## Decision

1. **A0 mark on the part (UI-03).** A solid line in the rotary-axis colour runs from the rotary axis along the model's up direction (the side of the part that faces the tool at A = 0) out past the part and the stock, at the part's far end, labelled "A0". It is part of the workpiece, so in the machine view it turns with the part (SIM-02), and the A+ arrow does not. It hides with the rotary axis and the other guides (Show or hide, axes). It needs no stock geometry, so it works for every stock shape.
2. **The arc start is labelled "A0" too.** The fixed A+ arrow starts at machine A = 0; its start carries the label "A0". In the part view, or at A = 0, the two marks coincide; in the machine view they are apart by the current A.
3. **Caption under the view cube (UI-02).** A small box under the cube always says "Machine" or "Part", with a tooltip in plain words. "Machine" while the part turns inside the view (Simulate with "Turn part with A" on): "Machine view: the faces are the machine's, and Top is toward the tool. The part turns inside the box as A changes." "Part" otherwise: "Part view: the faces are the part's and stay with it. Where the tool moves round the part, the part does not turn ("Turn part with A" is off, or this is not Simulate)."
4. **The part is shown as designed outside Simulate (SIM-02).** This was a defect: the playback head's A was applied to the part on every step, so Model, Setup and Operations showed the part, stock and path turned to the last simulated angle, while the fixed guides stayed at design orientation. The turn now applies on Simulate only; every other step puts the part at A = 0. The cursor is still kept (0238 item 5), so returning to Simulate shows the same angle. One rule (`orient::part_turn_shown`, `orient::cube_frame`) feeds the turn, the caption and the tooltip.

## Alternatives considered

- **A0 tick on the part's end face or rim.** Needs end-face geometry; fails for imported stock.
- **A dashed A0 plane or ring.** Heavy, and reads like the clearance cylinder.
- **Caption on Simulate only.** The owner chose always shown.
- **A different frame colour for the cube.** Colour alone does not carry meaning (specification section 16); not used.

## Consequences

- Specification 7.2 (cube row, what is drawn, line table) is edited in this change.
- Code: `apps/openchuck-ui/src/orient.rs` (new), `playback_panel.rs` (`frame`), `viewport.rs`, `viewport_overlays.rs`; `openchuck-app` `scene.rs` (`SceneRole::AZero`, the "A0" labels).

## Verification

Tests, uninstrumented, written first:

- `req_sim_02_the_part_is_at_its_design_orientation_outside_simulate` (`orient.rs`, pure; and in `shell/simulate_tests.rs` with a run that turns A: 300.8 degrees were applied outside Simulate before the fix, 0 after; returns on Simulate; "Turn part with A" off gives 0 and "Part").
- `req_ui_02_the_cube_names_the_machine_only_where_the_part_turns`, `req_ui_02_the_cube_caption_and_tooltip_are_plain_words`.
- `req_ui_03_req_sim_02_the_a0_mark_runs_from_the_axis_up_past_the_stock` (both senses of A; the two "A0" labels), in `openchuck-app/tests/scene.rs`.
- `req_ui_03_req_sim_02_the_a0_mark_turns_with_the_part_and_hides_with_the_axes`.
- `req_ui_02_req_ui_16_the_overlays_never_overlap_on_any_step` now counts the caption box (at 1100 x 760 and 900 x 480).

Not verified by a person looking at the screen: the owner views the application to check the labels' look and position (crowding with "Y" in an end view is possible).
