# 0531: A free end past the flutes is warned of and not faced, not refused (OP-04, TL-04, NFR-07, NFR-12; lane FREEEND)

- **Status:** accepted by the project owner on 6 Nov 2026, relayed by the coordinator (decision 0049). The owner chose this behaviour ("option B") on 6 Nov 2026, relayed by the coordinator.
- **Date:** 6 Nov 2026.
- **Number:** 0531, reserved by the Merge Manager on 6 Nov 2026.
- **Amends:** [0344](0344-finishing-stepdown-and-free-ends.md) item 7 ("A face deeper than the flutes refuses the operation") and [0509](0509-flute-length-against-the-deepest-cut.md) item 4 (free ends use the same measure for their refusal). [0522](0522-the-flute-check-warns.md) item 5 (third bullet), which kept the free-end refusal until the owner chose, is settled by this record. The measure ([0509](0509-flute-length-against-the-deepest-cut.md) items 1 and 2, with the reach profile of [0524](0524-flute-check-reads-the-stock-in-the-tools-reach.md)) is unchanged.
- **Open items:** OI-1396 edited in place (its refusal is replaced). OI-2047 (free ends ignore the operation's region) is not touched. No new open item.
- **Library:** no public API change. `finishing::free_ends` is crate-private; the operation now returns a path with a warning where it returned an error.

## Context

With Face free ends on (the default), the tool turns down beside each free end face in layers to the rotary axis. Under 0344 item 7, a face whose stock beside it is deeper than the tool's flute refused the whole operation. Under 0522 the flute check warns in every case, so this was the one flute-depth rule left as a refusal. Since 0524 the measure reads the stock within the tool's reach, so most cases no longer reach it; those that do have stock really past the flutes.

## Decision

1. **Warn and skip that end.** Where the stock beside a free end is deeper than the flute, no ring is cut at that end, the end keeps what earlier operations left, and generation continues with the rest of the path (helical, axial or rest, as the strategy makes it). A turn flagged as deeper than the flutes is never added.
2. **What the warning says.** It starts "Flute length: " (`flutes::WARNING_LEAD`), so the application names the operation and the tool as it does for 0522 ("Flute length (operation '<name>', tool '<tool name>'): ..."). It says:
   - that the end face at X is not faced, and why: the stock beside it is (or "up to", when the depth is a bound, with why) so many mm deep, more than the tool's flute;
   - what the stock is;
   - the options: a tool with longer flutes, a Facing operation for that end, or turning off Face free ends for this operation;
   - that the path must be checked before it is run.

   It carries the Face free ends parameter, needs no acknowledgement, and is one of the path's notes (0175). Nothing in it says the program is safe (NFR-12).
3. **Unchanged.** A free end within the flutes is faced as before, with byte-identical output (GC-07). The other free-end warnings (no Stepdown, stock of no known size, held-part and cut-free warnings) are unchanged. Facing (0252, 0504) keeps its own refusal.

## Consequences

- An operation is no longer refused for a free end past the flutes; the user sees one warning and decides.
- That end is left unfaced, so it keeps whatever earlier operations left.

## Verification

In `crates/openchuck-path/tests/helical_faces.rs`, the free-end refusal tests of 0344 and 0509 become warning tests with the same numbers (10.300 mm against 8.000 mm of flute; the bound case "up to 11.300 mm"):

- `req_nfr_07_req_nfr_12_a_free_end_deeper_than_the_flutes_is_warned_and_not_faced`: helical, axial and rest finishing each make a path with no ring at that end and one warning with the wording above.
- `req_nfr_07_req_nfr_12_a_free_end_skipped_on_a_bound_says_it_is_a_bound`.
- `req_gc_07_a_free_end_within_the_flutes_has_no_free_end_warning`; the existing tests for a face within the flutes (`req_op_04_req_op_09_...`, `req_nfr_07_req_tl_04_...flush...`) are unchanged.
