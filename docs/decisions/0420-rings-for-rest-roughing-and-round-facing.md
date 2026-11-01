# 0420: Rings instead of a spiral for rest machining, continuous rotary roughing and round Facing (OP-03, OP-09, OP-24, OP-11, OP-12, OP-14, OP-15, GC-07, NFR-06, NFR-07, NFR-12; issue #440; lane RINGS-ALL)

- **Status:** accepted by the project owner on 1 Nov 2026, relayed by the coordinator (decision 0049), including items 2 and 3: D1, round Facing gets the option (option 1 of 2), and D2, rest machining cuts its rings and steps only where material is left. Proposed by lane RINGS-ALL the same day; D1 and D2 were first answered by the coordinator under the owner's request of 1 Nov 2026 (rings wherever a spiral is offered).
- **Date:** 1 Nov 2026.
- **Issue:** #440, "Rings option for every strategy that currently cuts a spiral".
- **Builds on and extends:** [0390](0390-helical-finishing-in-rings.md) (helical finishing in rings, accepted), [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (a turn at each face across the axis), [0044](0044-slice-15-rest-and-pencil.md) (rest machining), [0033](0033-slice-14-rotary-roughing.md) (rotary roughing), [0212](0212-roughing-direction-and-extend-past-the-stock.md) (cut direction, Facing), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (the end turns at a held region), [0239](0239-adaptive-pass-spacing.md) (adaptive spacing). It does not change 0390; it uses its shape, setting and helpers for three more strategies.
- **Open items:** closes OI-1490. OI-1491 (the simultaneous spiral) stays open for the second pull request of this lane, after the crankshaft investigation (#439). No new open item.

## Context

Decision 0390 gave helical finishing a **Passes** setting, Spiral or Rings, and recorded the other strategies that cut a spiral as open items. The owner asked on 1 Nov 2026 for the option wherever a spiral is offered. Lane RINGS-ALL listed every strategy that cuts a helix today:

| Strategy | The spiral | This record |
| --- | --- | --- |
| Helical finishing (OP-04) | one helix over the part | done by 0390 |
| Rest machining (OP-09) | helical finishing's helix, cut only near remaining material | rings, item 2 |
| Rotary roughing, continuous form around the part (OP-03) | one helix a layer, A turning once every pitch | rings, item 4 |
| Facing, Round shape (OP-24) | the same helix engine as continuous roughing, down to a cylinder | rings, item 3 |
| Simultaneous finishing, Spiral pattern (OP-05) | one helix track, A smoothed and leaned | OI-1491, a later record |

No other strategy cuts a spiral: axial, indexed, pencil, swarf, edge following, drilling and the Flat, End and Box Facing shapes cut lines or contours, and the helix entry (decision 0026) is an entry move, not a pass.

## Decision

1. **One setting, as 0390's.** Rest machining, rotary roughing and Facing take the same parameter, `pass_shape`, labelled **Passes**, with the same choices, `helix` (shown as "Spiral") and `rings` (shown as "Rings"). Each strategy's description says what the choice does there. Roughing shows it only for the continuous form around the part, and Facing only for the Round shape, the forms that cut a spiral; the others ignore it. The shape is 0390's: one whole turn of A at a constant X (a ring), then a step along the part at a constant A, a whole number of turns from the start angle, to the next ring. The rings go from one end to the other, evenly spaced no wider than the stepover (0390 item 3), with a ring at each end and a ring at each face across the axis (0297) in X order. One shared helper places them for every strategy.
2. **Rest machining cuts rings only where material is left (D2, accepted, relayed).** The rings and the steps both go through rest machining's own filter: a point is cut only within half a stepover of a sample with material left, as on the spiral. A step with nothing to cut is not cut, and the shared services link the pieces on either side of it (OP-14). The faces' turns of 0297 are rings among the others.
3. **Round Facing has the option (D1, option 1 of 2, accepted, relayed).** Round Facing runs on continuous roughing's engine, so it gets rings with item 4. Its ends are rings, as the spiral's end turns were. The independent completion check of restricted Facing (decision 0252) accepts rings: it measures the turns at the face's radius, and a ring is a turn at one X. Option 2, keeping Round Facing a spiral, was not chosen.
4. **Continuous roughing in rings.** Each layer is cut as before, outside in, with the same levels, stepdown, floor, remaining stock field, smallest cut and region; only the path along which the layer's nodes lie changes. A ring has as many nodes as a turn of the spiral (`max_angle_step`), a step at least one, no further apart than the line spacing of the forms along the axis. A node is cut where material stands above the tool and the region allows it, and the runs are split and carried to a region's edge exactly as for the spiral. A held region's ends (0161) and round Facing's ends are rings, so the stock beside them is cleared all round. A face across the axis inside the range is a ring, and the tool cuts down to the face there as on the spiral's face turn. The node count is limited and refused as for the spiral (NFR-07).
5. **Off by default, for new operations too.** As for helical finishing (0390 item 2), Spiral is the default for new operations and for saved ones, and an operation without the setting or with Spiral is made by exactly the code it was before, so its output is byte-identical and no golden program changes. No project schema number and no library version bump are needed: the setting is a free parameter, and the only API change is two added constants (`rest::parameter::PASS_SHAPE` and `roughing::parameter::PASS_SHAPE`, both equal to `helical::parameter::PASS_SHAPE`).
6. **What is unchanged.** Entries and links (0026, 0242), with an entry for each cut. Rotary handling (OP-15, 0241, 0247): A builds up one turn a ring, as with the spiral, and never reverses within a cut, so range checks, unwinding and counter resets behave the same. Stock to leave and the free ends and layers of finishing (0343 to 0345) for rest machining. Extending past the stock (0212) widens the range the rings cover. Adaptive spacing: continuous roughing around the part, round Facing and rest machining have none today, spiral or rings, and this record adds none.
7. **Notes.** Each strategy says in a note that it cut rings and how far apart they are.
8. **Refused, not guessed (NFR-07).** Any other value is refused, naming the parameter.
9. **NFR-12.** Nothing in the wording says the program is safe or checked; generated G-code remains the user's to verify before running on a machine.

## Documents

- No change to the requirements or the architecture: a setting of strategies OP-03, OP-09 and OP-24 already have, using the shared services.
- UI specification, section 8: the **Passes** entry covers the four strategies.
- Verification plan: "Helical finishing in rings" becomes "Rings instead of a spiral" with the three strategies' cases.

## Consequences

- **Library:** `helical::pass_shape_spec`, `helical::in_rings`, `helical::ring_places` and `helical::ring_passes` are shared within the crate; continuous roughing's layer engine is one function (`Job::layers`) that the spiral and the rings both use, with the spiral's arithmetic unchanged. Two added public constants; no version bump.
- **Tests,** each written before the code and watched to fail: `crates/openchuck-path/tests/spiral_rings.rs` (each strategy: the setting and its default; rings at constant X and steps at constant A, no wider apart than the stepover; rest cuts only where material is left, no further than the spiral on clean stock; no gouge and no more left than the spiral, graded with the test kit's independent gouge and coverage checks; roughing's layer levels as the spiral's, one cut a layer where nothing splits it; round Facing down to the diameter and as far along as the spiral; unchanged when off, and other forms and shapes unaffected; byte-identical at 1, 2 and N threads; a bad value refused by name), `roughing_region.rs` (rings keep to a selected region and start on its edge) and `facing_legacy_scope.rs` (restricted round Facing in rings passes the completion check).
