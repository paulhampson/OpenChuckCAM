# Archive: decision 0211 (STEPALL-M), raw evidence

Cited by [decision 0211](../../decisions/0211-step-accuracy-measured.md) ("Released-main integration" and "Paused checkpoint of 24 Oct 2026"). Moved here from `docs/handoffs/` on 1 Nov 2026 (DOCS-TIDY). Produced on branch `claude/lane-stepall-m-measure` (PR #321). Nothing here is a product API or a verification of generated G-code (NFR-12).

| Path | What it is | Commit |
| --- | --- | --- |
| `release-validation-2026-10-26.txt` | Commands and raw test, coverage, clippy, M4 and build output for the released-main integration, including the deliberately interrupted contended determinism run | parent `0b4daa3`, merged main `0620cd0` |
| `m3-2026-10-24-paused/` | Full-sequence M3 as paused on 24 Oct 2026: three completed four-thread batches (`full-m3-{a,b,c}-all.log`, `.done` markers, manifest, status, summary) and the orchestration, watch and summary scripts (they keep the original `/workspace` paths) | source `eee448609348e7ecc5bbfe71e8b3377d91170b32` |

Superseded by the 25 Oct rerun in `docs/decisions/0211-full-sequence-m3-evidence.txt`. Deleted as noise: the partial single-thread log `full-m3-b-1.log` with its `.paused-by-owner` copy, and the three `.incomplete-quiet-timings` files (an attempt whose timing callbacks were lost; never counted).
