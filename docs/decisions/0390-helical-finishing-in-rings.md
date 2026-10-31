# 0390: Helical finishing can cut rings stepped along the part instead of a spiral (OP-04, OP-11, OP-12, OP-14, OP-15, GC-07, NFR-06, NFR-07, NFR-12; issue #423; lane LINKEDPASS)

- **Status:** accepted by the project owner on 31 Oct 2026, relayed by the coordinator (decision 0049). The owner chose the pass shape (item 1, option 1 of 4) and the default for new operations (item 2, Spiral) the same day, also relayed by the coordinator.
- **Date:** 31 Oct 2026.
- **Issue:** #423, "Helical strategies: option to cut linked parallel passes instead of a continuous helix".
- **Builds on:** [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (one turn at each face across the axis), [0343](0343-end-faces-beside-a-short-tab.md), [0344](0344-finishing-stepdown-and-free-ends.md) and [0345](0345-finishing-layers.md) (end faces, free ends and finishing layers), [0239](0239-adaptive-pass-spacing.md) (adaptive pass spacing; proposed), 0212 (extend past the stock), 0241 (A between operations).
- **Open items:** OI-1454 (the helix's ends) does not arise for rings; it is noted, not closed. New OI-1490 (rings for rest machining and continuous roughing) and OI-1491 (rings for the simultaneous spiral). No project schema change.

## Context

Helical finishing cuts one continuous helix: A turns once every pitch of X, so X moves throughout every turn. The owner asked for an option to cut linked parallel passes instead: one full turn at a fixed place along the part, then a short step to the next one. The pieces already exist. The ring is the face turn of decision 0297 (`helical::turn_passes`), and the step is an axial fit one spacing long, as axial finishing uses.

## Decision

1. **The shape (option 1 of 4, the owner's choice, relayed).** With **Passes** set to **Rings**, helical finishing cuts one whole turn of A at constant X (a ring), then steps along the part at the same A to the next ring. The step is cut on the surface: the tool is dropped at every point, as on the ring. The rings and steps form one continuous cut, with no retract between rings. Every ring turns the way **Rotation** says, and each starts and ends at the **Start angle**, so all the steps lie on that one line of the part.
2. **Off by default, for new operations too (the owner's choice, relayed).** The parameter `pass_shape` is `helix` (shown as "Spiral") or `rings` (shown as "Rings"). An operation with no value, or with `helix`, is made by exactly the code it was before, so its output is byte-identical and the golden programs do not change. New operations also default to Spiral, so no operation's output changes unless the user picks Rings, and the library version is not bumped.
3. **Spacing.** The spacing is the pitch the stepover and scallop give, as for the helix. The part's length is divided into `n = ceil(length / pitch)` equal gaps, with rings at both ends of the part, so the spacing is never wider than the pitch. A note gives the spacing used.
4. **Faces across the axis (0297).** A face's turn is a ring. In ring mode it is placed in X order among the rings and joined by steps, instead of being cut after the spiral with its own links in and out. 0297 item 3 found that splicing a face's turn into the helix left micro-moves at the join. Rings do not splice: a face's ring is fitted on its own and the steps meet it at its start, so the join is a point of both. Measured: the stepped shaft's faces are left at the stock to leave (0 and 0.3 mm) within the tolerance and the whole shaft is one cut; the CrankShaft's four faces are cleaned as the spiral cleans them.
5. **What is unchanged.**
   - Stock to leave, region, leave zones, tabs and protections use the same fit as the spiral. A ring or step is split where the spiral would be, and the shared services link the pieces (OP-14).
   - Entries and links (0026, 0242) are unchanged, with an entry for each pass.
   - Rotary handling (OP-15, 0241) is unchanged: A builds up by one turn per ring, as with the spiral, so range checks, unwinding and counter resets behave the same.
   - Free ends, end faces and finishing layers (0343 to 0345) work on the passes as before. Extend past the stock (0212) adds rings out to the extended ends.
   - Adaptive spacing (0239) adds rings between rings where more than the Allowed excess is left, using its own settings and limits; partial results and the export acknowledgement work as for the spiral.
6. **Refused, not guessed (NFR-07).** Any other value is refused, naming **Passes**.
7. **NFR-12.** Nothing in the wording says the program is safe or checked; generated G-code remains the user's to verify.

## Documents

- No change to the requirements or the architecture: this is a setting of an OP-04 strategy, and the shared services do not change.
- UI specification, section 8: the **Passes** entry.
- Verification plan: "Helical finishing in rings".

## Options not chosen

2. Rings back and forth: each ring turns the other way, so A never builds up beyond one turn, but the cutting direction alternates.
3. Rings with a short spiral step over the last few degrees of each turn: smoother, but each ring is not quite flat at its end.
4. Separate rings joined by the usual retract and entry: simplest, but slower, with an entry mark at every ring.

## Consequences

- **Library:** one added public constant (`helical::parameter::PASS_SHAPE`); no version bump if the default stays Spiral. The `openchuck-path` changelog records the change.
- **Tests,** each written before the code: `crates/openchuck-path/tests/helical_rings.rs`, and rings cases in `helical_faces.rs`, `adaptive.rs`, `region.rs` and the UI's `form_panel.rs`. They cover:
  - constant X on each ring and constant A on each step;
  - spacing never wider than the pitch, with both ends reached;
  - no gouge and full coverage, graded with the test kit's independent gouge and coverage checks on a cylinder and a cone, and the faces of the stepped shaft and the CrankShaft left as the spiral leaves them;
  - output unchanged when off;
  - one cut where nothing splits it, and splits at a boundary's edges as for the spiral;
  - gouge, stock to leave and scallop;
  - faces and adaptive spacing (free ends and layers work on the passes unchanged);
  - A range;
  - byte-identical output at 1, 2 and N threads;
  - a bad value refused.
