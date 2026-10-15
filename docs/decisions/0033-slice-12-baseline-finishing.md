# 0033: Slice 12 design refinement: baseline finishing (helical, axial, indexed)

- **Status:** proposed (15 Oct 2026). Guidance only until the project owner accepts it; items 4 and 6 need an answer before they are built (see "Questions for the owner"). It relies on accepted records [0018](0018-slice-5-pathing-core.md) (strategy interface, helical finishing, shared services), [0026](0026-entry-moves.md) (entry moves), [0016](0016-sp-7-2d-offsets.md) (2D offsets) and [0032](0032-slice-10-mesh-selection.md) (region constraints). Lane H of the work breakdown's parallel work; its needs (lane C, the entry module) are on `main`.
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): this record takes OI-145 to OI-148 and proposes to settle OI-137 and OI-138 (item 5).

Numbering: 0032 is the last record on `main`; the unmerged branches hold 0028 (and OI-141 to OI-144), so this record takes 0033 and OI-145 onward. No project schema number is claimed yet (item 4).

## Context

Work breakdown slice 12 (OP-04, continues OP-11 and OP-02). Slice 5 built helical finishing on the whole model, with stepover, tolerance, stock to leave and the shared entry styles of decision 0026 as operation parameters, and slice 10 gave it a `RegionConstraint` that is seen only where the tool is dropped (decision 0032 item 6). What the slice still lacks:

- axial and indexed finishing do not exist (items 2 and 3);
- stock to leave is one value per operation; OP-11 and item 4 also ask for it per selection;
- the region constraint is sampled (OI-138) and the containment boundary is only a bounding window (OI-137);
- `lead` entry is available through the common parameters but no strategy test or golden file covers it on a finishing strategy (item 5);
- no property test, checker run or LinuxCNC replay exists for the three strategies (review checklist).

Verification plan row OP-04: "Helical, axial and indexed finishing on analytic cylinders and cones match exact path and stepover".

## Proposal

1. **Shared stepover meaning (item 4).** For every wrap-family strategy `stepover` is the distance, in mm, between neighbouring passes **measured on the surface at the part's largest radius in the development** (x, s = R·A), as for helical finishing today (pitch along X). Helical: pitch in X. Axial: spacing in arc length at the largest radius, converted to a whole number of A positions so the passes close the circle exactly (the step is rounded down, never up, and the rounding is reported in a `PathNote`). Indexed: no stepover; the A positions are given. A second form, `scallop` (maximum cusp height for the tool in use), is an alternative way to set stepover: when given, the strategy derives the stepover from the tool profile at the contact and the local curvature, and takes the smaller of the two. `scallop` ships for ball and bull-nose tools; flat and V-bit tools refuse it with a message, since a cusp height has no closed form there.
2. **Axial finishing (item 2).** The tool stays at one A and moves along X over the part (or the region), then A steps by the axial stepover, alternating direction (zig-zag) or always the same way (parameter `direction`: `zigzag` default, or `climb`). Each pass is dropped with the same `drop_tool` and fitted by the same bisection as helical finishing (the fit moves into a shared module so both use one copy). A is continuous between passes by the shared A-axis management (OP-15); a full circle is not unwound.
3. **Indexed finishing (item 3).** For each requested angle in `angles` (a list, degrees; or `count` equally spaced starting at `start_angle`) the strategy emits one pass along X at exactly that A, with a positioning move to the angle between passes (a `Rapid` or linking move through the clearance geometry, never a feed move across the stock). The angle is written to the IR exactly as requested (no accumulated stepping), which is what the checklist item "indexed positions land on the exact requested angles" tests. The A values may be given in any order; they are cut in the order given, so the user controls the sequence.
4. **Stock to leave per selection (item 4; OP-11).** Open: the project holds one stock-to-leave value per operation and no value per selection set. Options in "Questions for the owner" below. Whichever is chosen, the strategy side is the same: the `RegionConstraint` carries, for the selected region, a list of (surface, leave) pairs, the tool is dropped with the grown profile of the largest leave among the regions at that point, and a point on no region uses the operation's value.
5. **Region offsets replace sampling (OI-137, OI-138).** The unrolled region (the selection's outline on the development) is offset inward by the tool radius plus the stock to leave with the `unroll` offsets of decision 0016 and the guards of decision 0018 item 14. For axial and indexed finishing the passes are the intersections of the lines A = const with the offset region, so a narrow selection is found exactly and no 0.1 mm sampling is needed. For helical finishing the helix is intersected with the offset region by the same code. The sampling rule and `CONSTRAINED_SPACING` stay as a fallback for a selection whose outline cannot be unrolled (a closed ring on the axis, say), recorded as such. A boundary drawn as a curve is then the same offset operation on the boundary set's outline (settling OI-137). OI-82 (the weak result check on very dense noisy loops) is on this path, because a mesh section is the input: the membership check proposed there is built first.
6. **Entry moves (item 5).** The three strategies take the common `entry` parameters. `lead` is wired in: the lead arc is tangent to the first and last cut of each pass, on the side away from the region boundary. `ramp` and `helix` apply to the first cut of the operation only for finishing (a finishing pass never needs to re-enter the material mid-pass), and the shared checks of decision 0026 item 7 refuse a style that does not fit. Question: whether `lead` is allowed with an axial pass whose ends lie on the part's end faces (nothing to lead onto); proposed: yes, in the air beyond the end, and refuse if there is no room.
7. **Parameter schemas and golden files (item 6).** Each strategy declares its schema through `ParameterSchema::with_common` so the form is generated (OP-16). Golden files: one per strategy on the reference cylinder-and-cone part (G-code through the LinuxCNC post, byte-identical at 1, 2 and N threads) and one for an axial pass with `lead`, one for a region cut by two selections with different stock to leave. Names follow `req_op_04_*`, `req_op_11_*` and `req_op_02_*`.
8. **Verification.** (a) Property test: for random cylinder and cone parts, random stepovers and random regions, every point of the region's surface is within the stepover of a path point, measured in the development at the largest radius. (b) The independent checker (slice 5 item 9) reports no gouge beyond tolerance on the reference parts. (c) Indexed positions are compared to the requested angles bit for bit. (d) Per-selection leave is measured by the checker as the distance from the path's contact to the surface on each region. (e) Each strategy is replayed in the LinuxCNC simulator (gate 3) with an unchanged post.

## Where the code is

`openchuck-path` only, plus parameter wiring in `openchuck-app` and, if option A or B of question 1 is chosen, `openchuck-project`. New modules `axial.rs`, `indexed.rs` and a shared `wrap_fit.rs` (the bisection moved out of `helical.rs`, behaviour unchanged, golden files unchanged). No new dependency (`cavalier_contours` is already declared; `i_overlay` is declared with its first use, which is region booleans in item 5, as decision 0018 item 5 deferred).

## Questions for the owner

1. **Per-selection stock to leave (item 4).** Where is the value stored? A: a field on the selection set (`stock_to_leave`, optional), which is a project format change and takes the next free schema number (8 and 9 are expected by other lanes, so this would be claimed as 10); B: on the operation, as a table from set id to value, which also changes the format; C: an operation parameter holding a list of `set_id:value` text pairs, which is not a format change (decision 0008 item 6) but is awkward to edit and validate. Recommended: A, because the value belongs to the region it describes and then survives the set moving between operations.
2. **Is `scallop` in this slice,** or only `stepover`? The slice item lists "tolerance, stepover and scallop controls". Recommended: in, for ball and bull-nose, as in item 1.
3. **Axial `direction` default:** `zigzag` (shorter paths; mixes climb and conventional cutting) or `climb` (one direction, more retracts). Recommended: `zigzag`, with the choice a parameter.

## Alternatives considered

- **Axial and indexed as modes of helical finishing:** one registry entry fewer, but the schemas differ (rotation, pitch against angles, direction) and a form that hides half its fields is worse than three short ones.
- **Keep sampling for regions:** simple, and misses features under 0.1 mm of path (OI-138); the offsets are already built and proven (SP-7), so use them.
- **Indexed finishing as a flag on axial finishing with a large step:** cannot hit arbitrary requested angles exactly.

## Consequences

- Registry gains `axial_finishing` and `indexed_finishing`; strategy names are stable once saved.
- The verification plan's OP-04 row is met by the tests above; if the answer to question 1 changes the format, the work breakdown's schema table gets a row first (claim before use).
- Work breakdown slice 12: status line and the review checklist are updated as items are built.
- Nothing here changes `docs/requirements.md` or `docs/architecture.md`; if `scallop` or the stepover meaning needs wording there, propose it and wait (CLAUDE.md).

## Open items

- **OI-145:** meaning of `stepover` for axial finishing (arc length at the largest radius, rounded to a whole number of steps), and whether a tool with a taper should be measured at the contact radius instead.
- **OI-146:** where per-selection stock to leave is stored (question 1).
- **OI-147:** `scallop` closed form for ball and bull-nose only; flat, V-bit, tapered and custom profiles refuse it (question 2).
- **OI-148:** `lead` on an axial pass that starts on the end face of the part: needs air beyond the end (item 6).
- **OI-137, OI-138:** proposed to close by item 5 when it is built and accepted.
