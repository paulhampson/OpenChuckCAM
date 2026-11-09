# 0550: Helical finishing's and rest machining's rings start on a whole ring at a held region's edge (OP-04, OP-09, OP-11, OP-15, SET-09, GC-07, NFR-06, NFR-12; lane RINGSTEP)

- **Status:** proposed by lane RINGSTEP on 9 Nov 2026. The owner chose (a) on 9 Nov 2026, relayed by the coordinator (decision 0049): the rings start on a whole ring at the held region's keep-out edge, as roughing's do. The owner accepts the record after reading it. The number is provisional: the Merge Manager assigns the final number at release.
- **Date:** 9 Nov 2026.
- **Found by:** lane RAMPENTRY's second session (RAMPLEAD, issue #640), on the owner-check project for #639/#640: a ring operation beside the chuck started with a short cut along X, and a lead-in there was refused. The cause was traced by lane RINGSTEP and put to the owner as a parked question (options (a), (b) and (c) below).
- **What it amends:** [0390](0390-helical-finishing-in-rings.md) item 3 ("The part's length is divided into `n = ceil(length / pitch)` equal gaps, with rings at both ends of the part") and [0420](0420-rings-for-rest-roughing-and-round-facing.md) item 1 ("The rings go from one end to the other"), for helical finishing and rest machining when a held region's keep-out band covers an end of the part: the rings then go from the band's edge.
- **Builds on:** [0161](0161-tools-keep-clear-of-the-gripped-length.md) (the keep-out bands and the shared filter that drops what reaches into them), [0363](0363-simultaneous-finishing-off-axis-sections-walls-and-held-ends.md) item 1 (simultaneous finishing's range already stops at the bands), [0420](0420-rings-for-rest-roughing-and-round-facing.md) item 4 (roughing's rings start on a ring at a held region's end), [0545](0545-blended-steps-between-rings.md) (steps between rings, Sharp or Blended).
- **Open items:** new OI-2114 (the owner's acceptance of this record).

## Context

Helical finishing and rest machining place their rings over the whole part (`helical::ring_places` over the surface's X bounds, 0390 item 3), whatever the workholding holds. The shared filter `strategy::keep_clear` (0161) then drops every point whose tip reaches into a held region's keep-out band (the held length widened by the tool's reach), and keeps the runs that stay clear. So beside a held end:

- every ring inside the band is dropped whole;
- the step from the last ring inside the band to the first ring outside it keeps only its part beyond the band's edge, so the pass starts with a short cut **along X with A held** (a stub), then turns 90 degrees into the first ring. With Blended steps (0545) the filter leaves a piece of a blend instead; the pass still does not start on a ring.

On the public baluster example (held 8 mm at the start) in rings, the Finish operation started with a 0.09 to 0.27 mm cut along X at A 0 from X 9.137, the band's edge, into stock beside the chuck. A lead-in there runs back along X into the band, so the operation was refused with a lead entry (the case lane RAMPENTRY found).

Rotary roughing and round Facing do not have this: `Job::cut_range` shortens their range at a band and puts a whole ring at that end (0161, 0420 item 4). Simultaneous finishing does not either: its range is cut short at the bands by `simultaneous::outside_held` (0363 item 1).

![Rings beside a held region. Today: the rings go over the whole part; the keep-out band drops the first rings and cuts the step into the first ring outside it short, so the pass starts with a stub along X with A held. Decision 0550: the rings go from the band's edge, with a whole ring there, then the step or blend to the next ring.](0550-rings-start-at-the-keep-out-edge.svg)

## Decision (proposed; the owner chose (a) on 9 Nov 2026, relayed)

1. **The range (option (a), the owner's choice, relayed).** Helical finishing and rest machining place their rings over the part's X range cut short at the keep-out bands that cover its ends, with a ring at each band's edge, as roughing and simultaneous finishing do. The places are otherwise 0390 item 3's: equal gaps no wider than the pitch (or stepover) over that range, and a ring at each face across the axis (0297) that lies inside it. A face inside a band is not a ring place (the filter dropped it before).
2. **Both step shapes.** The pass starts and ends on a whole ring whether the steps are Sharp or Blended (0545).
3. **Unchanged.** Without a held region, or with one that does not cover an end of the part, the ring places and the output are as before, to the byte. The shared filter (0161) still runs on every pass and still refuses anything that reaches into a band. Simultaneous finishing, roughing and round Facing are unchanged. The spiral is unchanged.
4. **Output.** Ring operations of helical finishing and rest machining with a held region at an end change, Sharp included, saved operations too: their rings move to the band's edge and the pass no longer starts with the stub. No golden, reference or example program uses rings (checked on `main` `7825c79`), so none changes. No project schema change.
5. **NFR-12.** The wording says the rings start at the held region's edge. It does not say the program is safe or checked: generated G-code remains the user's to verify before running it on a machine.
6. **Library (0048).** No public API change; the output change for ring operations beside a held region is in the changelog, in the same minor version as 0545 if they release together.

## Options not chosen

- **(b) No change, documented:** a ring pass beside a held region keeps starting with the stub, and the user guide says so. Not chosen: the stub is the corner into stock that 0545 removes elsewhere, and it blocks a lead entry there.
- **(c) Drop the stub only:** start on the first whole ring outside the band and leave the band's edge to that ring's reach. Not chosen: up to one ring spacing next to the band is left to the tool's side, where (a) cuts it with a whole ring as roughing does.

## Consequences

- **Code** (lane RINGSTEP, a pull request of its own after #655): `crates/openchuck-path/src/helical.rs` (`ring_span`, used by the rings of `passes`), `rest.rs` (its ring call), `simultaneous.rs` (`outside_held` shared within the crate).
- **Documents:** the verification plan's "Rings instead of a spiral" gains the case; the user guide's **Passes** entry says rings start at a held region's edge; the changelogs of `openchuck-path`; no UI change.

## Verification

### As built (lane RINGSTEP, 9 Nov 2026)

On branch `claude/lane-ringstep-heldring`, from #655's prepared head (`f43c1ea`, `main` `eb2ff5f` merged). Tests, each written before the code and watched to fail (before the change the first three failed with "the cut starts with a Step" and the baluster tests with "the pass cuts along X with A held from X 9.137 to 9.228 before its first ring"), run uninstrumented with `cargo nextest run`:

- `crates/openchuck-path/tests/ring_steps.rs`: `req_set_09_req_op_04_helical_rings_start_on_a_whole_ring_at_a_held_regions_edge` (a cone with a keep-out band to X 3: Sharp and Blended, the first cutting move turns A, no move along X before it, the first ring at X 3 and a whole turn, no ring inside the band), `req_set_09_req_op_09_rest_rings_start_on_a_whole_ring_at_a_held_regions_edge` (the stepped shaft, band to X 2.5, between two ring places), `req_gc_07_req_nfr_06_rings_beside_a_held_region_are_identical_at_1_2_and_n_threads`.
- `crates/openchuck-app/tests/held_region.rs`: `req_set_09_req_op_04_the_baluster_finish_in_sharp_rings_starts_on_a_whole_ring_beside_the_chuck` and `..._in_blended_rings_...` (the public baluster example, held 8 mm at the start, its Finish in rings: no cut along X before the first ring, and that ring a whole turn at the band's edge, between X 7 and 12). 32 s and 34 s each; first written as one test (68 s), split by case under the one-minute rule.
- **Lane RAMPENTRY's lead:** a local trial merge (not pushed) of this branch with `claude/lane-rampentry-lead-wip` (`938fa89`), on the owner-check project `owner-checks/rc/639-640-ramp-and-lead-follow-the-path` with 'Finish rings, lead' on its held setup: it now generates (before: refused, "the lead-in ... would run over the held region"), and the program starts at X 9.088, the band's edge, with the lead turning A into a whole ring.

Not done: nothing was run on a machine.
