# 0641: End facing keeps the approach distance from the end face, covers the face for written motion, and End and Box facing can keep their links low (OP-14, OP-24, SET-06, SET-09, NFR-05, NFR-07, NFR-12, GC-07, NFR-06; issue #499; lane FACEAPPROACH)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049), unchanged. Proposed by lane FACEAPPROACH the same day; item 3 is the owner's decision D1 (option iii), relayed by the coordinator; items 1 and 2 fix defects and needed no choice (the coordinator, 2 Nov 2026).
- **Date:** 2 Nov 2026.
- **Issue:** [#499](https://github.com/paulhampson/OpenChuckCAM/issues/499), "Facing: the toolpath starts from the safe Z instead of just above the top of the material".
- **Builds on:** [0252](0252-facing-ends-sides-and-extents.md) (End and Box facing, their completion proofs and the written-motion budget), [0496](0496-low-links-between-finishing-passes.md) (Links between passes), [0495](0495-minimum-clearance-and-clearance-from-stock.md) (the clearance in use), [0033](0033-slice-14-rotary-roughing.md) item 6 (low links against stock), [0048](0048-pre-1-0-api-changes.md) (pre-release API changes).
- **Open items:** OI-1741 (other strategies that retract between passes) is narrowed; OI-1997 (End facing depth against the flute length) and OI-1998 (the Facing cutter-sweep overlay) are new. No project schema change.

## Context

The owner set up End facing on the Baluster example: box stock 72.4 × 35 × 35 mm, the start end held for 8 mm, a 3.175 mm flat tool and 1 mm stock to leave. The toolpath appeared to start from the safe Z, not just above the material.

The real pipeline was run on that project and every move inspected. The Facing shapes behave as follows.

- **End.** There are 16 flute bands at A 0, each beside the far end face. Each band:
  - rapids from the safe Z (40 mm) straight down to the approach distance above its own height;
  - plunges 1 mm in the air;
  - feeds into the end, across it and out;
  - rapids back up to the safe Z.

  Two things were wrong. First, the point where each band starts and ends kept the complete tool only a quarter of the tolerance (2.5 µm) from the end face. Every rapid descent therefore passed the face 2.5 µm away, down to 35 mm below the stock top. The shared descent rule (OP-14) stops a rapid the approach distance above stock "on its line", but the line missed the stock by 2.5 µm, so the rule never applied. Second, both posts refused the operation: "Facing written-motion verification: Facing completion: the actual end sweeps leave an unfinished required region in an axial layer".
- **Box.** Each row rapids down to the approach distance above the stock and goes back up to the retract height before the next row.
- **Flat and Round.** These shapes run on rotary roughing's layers. They descend to the stock plus the approach distance and link low against their own pass stock (0033 item 6). They behave as expected.

Generation does not depend on other operations. The End facing path was identical with the Rough operation enabled and disabled.

The export refusal had a measurable cause. The lowest band sat exactly on the coverage edge, at Z −17.5, and adjacent bands met with no overlap whenever the band spacing equalled the flute length. The verifier replays the written motion as the controller reads it, so the coordinates it sees are not the planned ones. Through the LinuxCNC frame, −17.5 came back as −17.499999999999993. Through the Z1's single-precision reading it came back as −17.4999985. A word rounded to three places moves by up to 0.0005 mm. Any of these leaves an uncovered sliver at the edge, and the exact completion proof rightly refuses it. Decision 0252 already allots part of the tolerance to written and assembly error, but the End band layout did not reserve that allowance.

## Decision

1. **The approach distance beside the end face (OP-14, NFR-12; a defect).**
   - End facing's air point, where each band's in-feed starts and its out-feed ends, keeps the complete tool the operation's approach distance from the stock's end face and from any protection beyond it. It is never less than a quarter of the tolerance, the old value, which applies when the approach distance is 0.
   - A rapid that descends beside the end face then keeps the same allowance for stock longer than modelled as one that stops above the stock top. The in-feed into the end runs from there at the cutting feed.
   - `EndFacingParameters` gains `approach`, read from the operation's approach distance.
2. **The written-motion allowance in the End bands (OP-24, NFR-05; a defect).**
   - End bands reach one eighth of the tolerance past the requested top and bottom edges. The band spacing is capped at the flute length less twice that allowance, so neighbouring bands overlap by at least the allowance.
   - With the default tolerance of 0.01 mm the allowance is 0.00125 mm. This is the written-motion share of the budget in decision 0252.
   - Where a tab limits a band, the band keeps the tab's separation and is not carried further.
   - The completion proof is unchanged and stays exact: a band raised by more than the overlap still refuses.
3. **Links between passes for End and Box (OP-14, SET-09; the owner's decision D1, option iii, relayed).**
   - Facing offers decision 0496's choice, `link_height`, labelled **Links between passes**, with the values Retract (the default) and Low. The option is shown for the End and Box shapes only: Flat and Round already link low against their pass stock.
   - A saved operation has no value and reads as Retract, so its path is unchanged. A new operation starts at Retract, as in 0496.
   - With Low, the shared assembly links each pass by 0496's rule. A link stays the approach distance above the setup's stock wherever the whole link is clear of the stock, the held regions and the part. Any other link retracts, with the note that counts them. Every move still goes to the collision oracle.
   - Facing's own proofs still apply to the low links: the continuous rapid clearance, protection, shank and completion proofs. A path that fails any of them refuses (NFR-07).
   - For End, a low link runs beside the end, the approach distance clear of the face.
4. **The generated G-code is still the user's to verify (NFR-12).** Nothing here says the path or the G-code is safe or verified. The parameter text is 0496's, which says so.
5. **Library version.** `EndFacingParameters` is a public struct built by literal, and it gains a field. This is a breaking change under 0048: a minor bump, with the number assigned by the coordinator, and a changelog entry.

## Alternatives considered

- **For item 2, treating remainders thinner than the arithmetic resolution as noise.** This was tried first. It covers the LinuxCNC frame's 7e-15 mm sliver but not the Z1's 1.5e-6 mm one, and it would weaken an exact proof. Rejected.
- **For item 2, moving only the outermost band edges.** This does not cover neighbouring bands that meet with no overlap when the spacing equals the flute length. Rejected.
- **For item 3, always low (option i) or a choice defaulting to Low (option ii).** Not chosen by the owner, who keeps existing output unchanged and sets Low where wanted.

## Consequences

- **Output.** Every End facing path changes: its air points and band heights move (items 1 and 2). Box paths are unchanged unless Low is chosen. Flat and Round are unchanged. No reference golden file uses End or Box facing.
- **Code.**
  - `openchuck-path`: `facing/end.rs`, `facing/end/routing.rs` and `facing.rs` (the schema, and marking low links).
  - Tests: `crates/openchuck-path/tests/facing_end_plan.rs`, `crates/openchuck-app/tests/facing_approach.rs` (the Baluster set-up) and `crates/openchuck-app/src/facing/integration_tests.rs`.
- **Documents:** the verification plan rows OP-14 and OP-24, the work breakdown (lane FACEAPPROACH), the open-items register and the decision index.
- **Not in this record.** These were reported on #499 and are left open:
  - End facing reaches 35 mm below the stock top with 25 mm of flute. The owner's expectation is to turn A through 180° at the flute limit (OI-1997).
  - The Facing cutter-sweep overlay is drawn in the Cut colour, at the tip and at the flute top of every fed move, and only when the last run's project equals the current one. This is what made the Operations view look different with Rough enabled and disabled (OI-1998).
  - Box facing refuses on the Baluster with an unfinished strip near the held end: issue #500. It has a different cause: nothing covers X 0 to 9.4 next to the held region.

## Verification

- On the Baluster set-up, End facing posts through both built-in posts (`req_op_24_req_gc_08_baluster_end_facing_posts_through_both_posts`).
- Every rapid below the stock top plus the approach distance stays the approach distance clear of the end face (`req_op_14_req_nfr_12_baluster_end_rapids_keep_the_approach_distance_from_the_end_face`).
- The default retracts as before, and an explicit Retract gives the same path and G-code (`req_op_14_req_gc_07_end_facing_links_retract_by_default`).
- With Low, the tool never returns to the retract height between bands, and both posts accept the path (`req_op_14_req_set_09_end_facing_low_links_stay_beside_the_end`).
- The same holds for Box on the integration project (`req_op_14_req_gc_07_box_facing_links_retract_by_default_and_stay_low_on_request`).
- At path level:
  - the air point keeps the approach distance (`req_op_14_req_nfr_12_end_air_point_keeps_the_approach_distance_from_the_end_face`);
  - the bands cover the edges for read-back errors of 7e-15, 1.5e-6 and 0.0005 mm but not for a band raised past the allowance (`req_op_24_req_nfr_05_end_bands_cover_the_edge_with_the_written_motion_allowance`).
- Not run on a machine.
