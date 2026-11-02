# 0643: The Facing cutter sweep has its own colour and legend entry, follows its own operation, and shows only the cutting sweep (OP-24, SIM-01, UI-03, UI-11, UI-25, NFR-12; issue #499; lane FACEAPPROACH)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049), unchanged. Proposed by lane FACEAPPROACH the same day; item 3 is the owner's decision S-1, relayed by the coordinator; items 1 and 2 were agreed by the coordinator as following decision 0252's "existing overlay conventions".
- **Date:** 2 Nov 2026.
- **Issue:** [#499](https://github.com/paulhampson/OpenChuckCAM/issues/499).
- **Builds on:** [0252](0252-facing-ends-sides-and-extents.md) ("actual necessary additional cutter sweep … follow existing overlay conventions"; UI specification section 9 on the Facing extent diagram), [0641](0641-facing-approach-written-allowance-and-links.md) and [0642](0642-end-facing-depth-within-the-flutes.md).
- **Open items:** closes OI-1998. No project schema change. Neither the library family nor the toolpath and G-code change.

## Context

On the Baluster example, the owner saw End facing drawn in two different ways in the Operations view:

- with Rough enabled, as straight passes;
- with Rough disabled, as stacked rings up to the safe Z.

The owner read the second as an older, stale toolpath. In fact the generated path was identical both times. The rings were a second drawing, the Facing cutter-sweep diagram of decision 0252, and four things about it caused the confusion:

1. **Its colour.** It was drawn in the Cut colour, with no legend entry, so it looked like toolpath.
2. **What it drew.** For every fed move, plunges included, it drew a tool circle and outline at the tip and again at the top of the flutes. That is why it rose to about Z 41.
3. **When it showed.** It was shown only while the whole project was exactly the one the last run was made from (`Application::start_scene_with_facing`). Enabling or disabling another operation, or any other edit, removed it until the next regeneration.
4. **The eye button.** The eye hid it only as a side effect of that equality test.

## Decision

1. **Its own colour and legend entry.**
   - The sweep is drawn in lime `#c2e07a` as a thin solid line (`roles::SWEEP`). The Cut moves keep amber and the path width, so the two differ in width as well as colour (section 2, principle 4: never by colour alone).
   - The path legend lists **Cutter sweep** after Avoid while a sweep is shown, before the held region and tab roles.
2. **It follows its own operation.**
   - Each Facing operation's sweep is shown while that operation's path is current and its eye is on.
   - Current is the same per-operation key the playback and export use (`export::current_operations`, the per-operation form of `run_is_current`).
   - Enabling, disabling or hiding another operation leaves the sweep as it is. Editing the Facing operation, or hiding its path, removes the sweep.
3. **Only the cutting sweep (the owner's S-1).**
   - The diagram draws the tool's outline along the cutting moves only, at the tool's tip:
     - for a cut at a fixed A, the two edges of the sweep and the tool's circle at each end;
     - for a cut that turns A (Round), the tool's circle along the arc.
   - Plunges, entries, leads and links are not drawn, and neither is the top of the flutes. The toolpath itself shows those moves.
4. **Nothing here says the path is verified (NFR-12).** The diagram is a picture of the cuts, as decision 0252 says. It is not a proof.

## Consequences

- **Code:**
  - `crates/openchuck-app`: `facing/display.rs` (`FacingVerification::display`, `add_generated`), `export.rs` (`current_operations`) and `lib.rs` (`start_scene_with_facing`).
  - `apps/openchuck-ui`: `roles.rs` (`SWEEP`), `viewport.rs` (the role's style) and `viewport_overlays.rs` (`LegendEntry::CutterSweep`).
- **Documents:** the UI specification's colour roles (section 7.4) and path legend (section 7.2). The verification plan's SIM-01 row.
- **Tests:**
  - `req_ui_11_req_sim_01_facing_sweep_follows_its_operation_eye_and_currency`
  - `req_op_24_req_ui_03_facing_sweep_shows_only_the_cutting_moves_at_the_tip` (End, Box and Round)
  - `req_sim_01_req_ui_25_the_facing_sweep_has_its_own_colour_and_legend_entry`

## Verification

- On the integration project, the sweep stays shown when another operation is turned on and when another operation's path is hidden. It goes when the Facing operation's own path is hidden, comes back when it is shown, and goes when the operation is edited.
- Every sweep point lies within the tool's radius of a cutting move, for End, Box and Round.
- The legend lists Cutter sweep after Avoid, and the role's colour is not the Cut colour.
- Not looked at in the running application in this session.
