# Decision records

A decision record captures a choice made after the documents in `docs/` were written, or one that changes them. The architecture's "Open decisions" section (architecture.md, section 13) lists what is still open.

## Format

One file per decision, named `NNNN-short-title.md` (four digits, starting at `0001`). Each record contains:

- **Status:** proposed, accepted, or superseded by `NNNN`.
- **Context:** the requirement IDs and document sections it affects.
- **Decision:** what was chosen, in a sentence or two.
- **Alternatives considered:** with reasons, including any spike results.
- **Consequences:** documents or code that must change, and any rule exceptions (for example an entry in the `xtask` dependency allow-list, which also needs the NFR-14 justification).

When a record is accepted, update the affected document in the same pull request.

## Index

Add a row for each new record. Read the records that touch your task before you start.

| Record | Title | Status |
| --- | --- | --- |
| [0001](0001-sp3-wrapped-path-result.md) | SP-3 result, wrapped path to G-code with A words | Proposed (simulator criterion outstanding) |
| [0002](0002-sp-2-viewport-spike.md) | SP-2 viewport spike, interim result | Proposed (keep or fall back is open until the NFR-03 measurements are recorded) |
| [0003](0003-slice-0-guardrails.md) | Slice 0 design refinement, guardrails and shared data | Proposed; licence, contributor agreement and testkit crate accepted 11 Oct 2026. Open items in [open-items.md](../open-items.md) |
