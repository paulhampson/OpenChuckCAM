# 0058: Per-selection stock to leave for roughing leaves at least the largest value

- **Status:** accepted by the project owner (17 Oct 2026, in the orchestration session, option C of the three offered). The build is not done: it is the scope of a future lane, E2 (work breakdown, Parallel work).
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

## Consequences

- **Documents:** the work breakdown (lane E2 in the lane tables, slice 14 status), the decision index, the open-items register. Decision 0043 item 5 is amended when lane E2 lands.
- **Open items:** OI-261 (exact per-zone roughing, deferred).
