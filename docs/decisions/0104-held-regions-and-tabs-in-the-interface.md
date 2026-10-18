# 0104: Held regions and retaining tabs are shown and edited in the interface (UI-03 amended, UI-25 added)

- **Status:** accepted by the project owner on 17 Oct 2026, in the orchestration session (the wording of both requirement changes below).
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none are new.

Numbering: the orchestrator took the next free record number across `main` and the unmerged branches (0103 is the highest on `main`; 0104 is free everywhere).

## Context

The owner asked on 17 Oct 2026 whether a requirement says that retaining tabs are shown in the interface. None did. SET-09 and OP-21 describe the data and the machining of held regions and tabs; UI-03 listed the rotary axis, work coordinate system, stock and part; the interface specification does not mention held regions or tabs. The code shows them (the viewport colours the held-region and tab roles, and `workholding_panel.rs` edits a draft of both), built under slice 21c item 2, which cited UI-03 and UI-07. So the behaviour existed without a requirement to test it against.

## Decision

1. **UI-03 is amended** to read: "Display the rotary axis, work coordinate system, stock, part, held regions and retaining tabs together in the viewport." (P1, unchanged priority).
2. **UI-25 is added** (P1): "View, add, edit and remove a setup's held regions and retaining tabs in the interface, with validation, the default tabs shown as defaults, and a warning when an operation's selection reaches one."
3. **The work follows in the work breakdown:** slice 21 gains item group 21d (the interface specification section and mock-up for the workholding panel and the viewport display, and the tests that name UI-25; the verification plan row is added with this record). Slice 22's component inventory names the panel and the display. UI-25 is mapped to slice 21 in the traceability table, so the "P1 requirement with no test" rule applies to it when slice 21 closes. P1 counts become 94 of 148, UI 17.
4. **Nothing in the interface specification or the code is changed by this record.** The specification text and the tests are 21d's work.

## Alternatives considered

- **Leave the requirements as they were and rely on slice 21c item 2.** The behaviour would stay untested against a requirement, and the traceability gate could never fail for it.
- **Only amend UI-03.** It covers the display but not the editing, validation and warning, which the code already has.

## Consequences

- `docs/requirements.md`: UI-03 text, UI-25 row. `docs/verification-plan.md`: UI-02/UI-03 row text, UI-25 row. `docs/work-breakdown.md`: counts, the slice 21 mapping, 21d. Slice 21 cannot close in `xtask/traceability.toml` until a test names UI-25.
- Requirement IDs stay stable: UI-25 is new, no ID is reused or renumbered.
