# 0058: Per-selection stock to leave for roughing leaves at least the largest value

- **Status:** accepted by the project owner (17 Oct 2026, in the orchestration session, option C of the three offered). Built by lane E2 on `claude/lane-e2-roughing-stock-per-selection` (17 Oct 2026; see "As built" below).
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-261 is new.

Numbering: the orchestrator took the next free record number, 0058, and OI-261, across `main` and the unmerged branches (0066, 0071 and 0076 and OI-257 to OI-260 are held by lanes O, S, F and Z).

## Context

OP-11 lists stock to leave "per operation and per selection". Per-selection stock to leave is built for the three finishing strategies (slice 12, [decision 0034](0034-slice-12-baseline-finishing.md) item 4, project schema 9: `SelectionSet::stock_to_leave`). Rotary roughing refuses it: `honours_leave_zones()` is false for it and the application stops with the existing message ([decision 0043](0043-slice-14-roughing-containment.md) item 5). Slice 14 therefore stays open on one checklist box, "Every OP-11 parameter exists and has a test".

Roughing does not ride the surface. It plans its layers from the floor over the whole part with the tool dropped on a surface grown by one stock value ([decision 0043](0043-slice-14-roughing-containment.md) items 1 and 2), and its tests rely on three things: every layer cuts the same sites, no cut is deeper than a stepdown, and a second pass over its own result cuts nothing. Growing the part by a different amount in each zone would change the floor and the tool-drop heights, and put those at risk.

## Decision

1. **Roughing leaves at least the largest stock to leave that applies to the operation.** Option C of three (A: leave each zone exactly its own amount; B: keep the refusal and record an exception). The value used over the whole part is the largest of the operation's stock to leave and the `stock_to_leave` of every selection set the operation uses. Roughing therefore never cuts below what any zone asks for, and the finishing strategies, which already honour the exact per-selection value, cut each zone to its own amount afterwards.
2. **The result says so.** When a selection's value raised the amount, the operation's notes name the value used and the selection that set it, so a user who expects the exact figure is told why more is left (NFR-12 spirit: nothing implies the output is other than it is).
3. **OP-11 for roughing is read as "at least".** `docs/requirements.md` is not edited: its wording is met for roughing by this record, which is the agreed reading. The slice 14 checklist box is ticked when lane E2 has built item 1 and tested it.
4. **Exact per-zone roughing is deferred** (OI-261, deferred). It is built only when a real part needs a zone to keep more stock than the rest during roughing, because bulk removal rarely pays for the extra complexity.
5. **Scope: a future lane, E2.** It changes `openchuck-app` (`operation.rs`, where `honours_leave_zones` is checked) and, if needed, the roughing strategy's parameter handling in `openchuck-path`; it adds no project format change and no public struct field. Decision 0043 item 5 is amended by it.

## Alternatives considered

- **A. Exact per-zone roughing:** the meaning of the requirement, but it changes the floor and tool-drop heights per zone and needs a lane, new tests and simulated-stock checks. Kept as OI-261.
- **B. Keep the refusal and record an exception:** costs no code, but a P1 requirement would close with a known gap and no behaviour behind it.

## Verification

Lane E2 writes the tests, named after OP-11 and OP-03: a selection with a larger stock to leave raises roughing's amount over the whole part and the notes name it; a smaller one changes nothing (byte for byte the same program as the operation's value alone); the simulated leftover after roughing is at least the largest value everywhere; the finishing pass after it leaves each zone its own amount; the second-roughing-pass and stepdown invariants of decision 0033 still hold.

## As built (lane E2, 17 Oct 2026)

- **Where.** `openchuck-app` `operation.rs` only, in `generate_with_part`: when the operation's selections give leave zones and the strategy does not honour them, rotary roughing (`roughing::NAME`) takes the largest of the operation's stock to leave and the `stock_to_leave` of its selected sets as its `stock_to_leave` value and is generated without zones; every other strategy that does not honour zones is still refused with the existing message. `openchuck-path` is unchanged, so no public API, dependency or project format changes, and no number was taken (no schema, no `OI-`).
- **The note** (item 2) is added only when a set's amount is larger than the operation's, after the notes about values, and reads: "stock to leave: roughing leaves 0.8 mm on the whole part, the amount of the selection set 'Top' (the largest of its selections), in place of the operation's 0.3 mm; roughing leaves one amount everywhere, and a finishing operation cuts each selection to its own". Where two sets hold the same largest amount, the first in the operation's list is named.
- **Selections on a STEP part** are unchanged: roughing cannot be limited to faces yet, and a set with a stock to leave on a STEP part is refused as before (`req_op_11_req_nfr_07_a_stock_to_leave_on_a_step_selection_is_refused`, which is about the B-rep selection, not roughing, and stays).
- **Tests** (`crates/openchuck-app/tests/selection.rs`):
  - `req_op_11_req_op_03_a_larger_selection_stock_to_leave_raises_roughing_over_the_whole_part`: a set at 0.8 mm over an operation at 0.3 mm gives the note (operation, value, set and the operation's value named) and the same toolpath and G-code byte for byte as the operation at 0.8 mm with no set amount, so the floor, the tool-drop heights and the stepdown and same-sites invariants of decision 0033 are those of a plain 0.8 mm roughing.
  - `req_op_11_req_op_03_a_smaller_or_equal_selection_stock_to_leave_changes_nothing`: a smaller, a zero and an equal amount give the same toolpath and G-code byte for byte and no note; with two sets the largest is named.
  - `req_op_11_req_op_03_roughing_leaves_at_least_the_largest_and_finishing_each_its_own`: simulated on the dexel stock (0.1 mm cell), the least left anywhere after roughing is 0.749 mm for 0.8 mm (within the cell); a second roughing operation over its result finds no cut to make (decision 0043 item 5's error, so the second-pass invariant holds); helical finishing after it on the same sets leaves the middle of the top face 0.80 to 0.82 mm and the middle of the other side faces 0.30 mm.
  - `req_op_11_req_nfr_07_a_strategy_without_leave_zones_other_than_roughing_is_still_refused`: simultaneous finishing with a set amount is still refused, naming OP-11.
  - `req_gc_07_req_nfr_06_req_op_11_roughing_with_a_selection_leave_is_the_same_at_any_thread_count`: the same toolpath, G-code and notes at 1, 2 and 4 threads.
- **Times** (uninstrumented, `cargo nextest run`, debug build, with the rest of the suite running): 2.4 s, 4.0 s, 47 s, under 0.1 s and 2.6 s. None is long-running.

## Consequences

- **Documents:** the work breakdown (lane E2 in the lane tables, slice 14 status), the decision index, the open-items register. Decision 0043 item 5 is amended when lane E2 lands.
- **Open items:** OI-261 (exact per-zone roughing, deferred).
