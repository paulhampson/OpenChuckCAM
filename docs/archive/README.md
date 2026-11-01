# Archive: raw evidence cited by decision records

A task's verification and validation notes live in its decision record's Verification section. Raw evidence (logs, run output, generated reports, helper scripts) lives only here, in `docs/archive/<NNNN>-<name>/`, with a `README.md` that says what the files are, which record cites them and which commit produced them. Nothing here is a source of truth, a product API or a verification of generated G-code (NFR-12).

| Folder | Record | Contents |
| --- | --- | --- |
| [0211-stepall-m/](0211-stepall-m/) | [0211](../decisions/0211-step-accuracy-measured.md) | Released-main validation output (26 Oct 2026) and the paused 24 Oct 2026 M3 batches with their scripts |
| [0241-0242-roughpath/](0241-0242-roughpath/) | [0241](../decisions/0241-continuous-rotary-operation-boundaries.md), [0242](../decisions/0242-optional-back-and-forth-ramp.md) | Fail-first and completion logs, coverage reports, released-integration logs |
| [0247-arot/](0247-arot/) | [0247](../decisions/0247-long-operation-rotary-accumulation.md) | Independent first-cut comparison script |
| [0248-rampbudget/](0248-rampbudget/) | [0248](../decisions/0248-ramp-clearance-budget.md) | Baseline, red/green, coverage and released-integration logs |

Raw evidence that predates this folder still sits beside its record in `docs/decisions/` (for example `0211-*.txt`).
