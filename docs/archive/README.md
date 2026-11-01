# Archive: raw evidence cited by decision records

A task's verification and validation notes live in its decision record's Verification section. Raw evidence (logs, run output, generated reports, helper scripts) lives only here, in `docs/archive/<NNNN>-<name>/`, with a `README.md` that says what the files are, which record cites them and which commit produced them. Nothing here is a source of truth, a product API or a verification of generated G-code (NFR-12).

| Folder | Record | Contents |
| --- | --- | --- |
| [0211-stepall-m/](0211-stepall-m/) | [0211](../decisions/0211-step-accuracy-measured.md) | Released-main validation output (26 Oct 2026), the paused 24 Oct 2026 M3 batches with their scripts, and the ten `0211-*.txt` evidence files |
| [0241-0242-roughpath/](0241-0242-roughpath/) | [0241](../decisions/0241-continuous-rotary-operation-boundaries.md), [0242](../decisions/0242-optional-back-and-forth-ramp.md) | Fail-first and completion logs, coverage reports, released-integration logs |
| [0244-z1-gates-4-5/](0244-z1-gates-4-5/) | [0221](../decisions/0221-gate-5-first-cut-project-and-limit.md), [0222](../decisions/0222-short-moves-minimum-block-time.md), [0232](../decisions/0232-block-rate-program-checked-as-sp-12-was.md), [0244](../decisions/0244-bounded-first-cut-linuxcnc-replay.md), [0317](../decisions/0317-z1-measured-axis-height-and-safe-z.md) | The results and review record of the first runs of Z1 gates 4 and 5, moved verbatim out of the procedure |
| [0247-arot/](0247-arot/) | [0247](../decisions/0247-long-operation-rotary-accumulation.md) | Independent first-cut comparison script and the AROT raw measurements |
| [0234-replay-output/](0234-replay-output/) | [0234](../decisions/0234-replay-output-rounding-correction.md) | Replay output correction evidence |
| [0237-strategy-explanations/](0237-strategy-explanations/) | [0237](../decisions/0237-strategy-dropdown-and-information-panel.md) | The approved strategy explanation texts (OI-986) |
| [0255-bounded-m2/](0255-bounded-m2/) | [0255](../decisions/0255-bounded-tool-contact-checker.md) | Bounded M2 evidence and raw extract |
| [0256-toolpath-mesh/](0256-toolpath-mesh/) | [0256](../decisions/0256-no-fixed-toolpath-mesh-qualifies.md) | Toolpath-mesh candidate evidence |
| [0260-exact-face-queries/](0260-exact-face-queries/) | [0260](../decisions/0260-exact-face-and-trim-queries.md) | Benchmark evidence |
| [0248-rampbudget/](0248-rampbudget/) | [0248](../decisions/0248-ramp-clearance-budget.md) | Baseline, red/green, coverage and released-integration logs |
