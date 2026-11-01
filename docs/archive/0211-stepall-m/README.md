# Archive: decision 0211 (STEPALL-M), raw evidence

Cited by [decision 0211](../../decisions/0211-step-accuracy-measured.md) ("Released-main integration" and "Paused checkpoint of 24 Oct 2026"). Moved here from `docs/handoffs/` on 1 Nov 2026 (DOCS-TIDY). Produced on branch `claude/lane-stepall-m-measure` (PR #321). Nothing here is a product API or a verification of generated G-code (NFR-12).

| Path | What it is | Commit |
| --- | --- | --- |
| `release-validation-2026-10-26.txt` | Commands and raw test, coverage, clippy, M4 and build output for the released-main integration, including the deliberately interrupted contended determinism run | parent `0b4daa3`, merged main `0620cd0` |
| `m3-2026-10-24-paused/` | Full-sequence M3 as paused on 24 Oct 2026: three completed four-thread batches (`full-m3-{a,b,c}-all.log`, `.done` markers, manifest, status, summary) and the orchestration, watch and summary scripts (they keep the original `/workspace` paths) | source `eee448609348e7ecc5bbfe71e8b3377d91170b32` |

Superseded by the 25 Oct rerun in `0211-full-sequence-m3-evidence.txt` (in this folder). Deleted as noise: the partial single-thread log `full-m3-b-1.log` with its `.paused-by-owner` copy, and the three `.incomplete-quiet-timings` files (an attempt whose timing callbacks were lost; never counted).

## Evidence files cited by decision 0211 (moved from `docs/decisions/` on 1 Nov 2026, DOCS-TIDY)

All on branch `claude/lane-stepall-m-measure` (PR #321); the source commit of each is at the top of the file.

| File | What it is |
| --- | --- |
| `0211-step-accuracy-evidence.txt` | Refreshed M2/M4 evidence at measured source `827fc45` (quick stepovers, 23 to 24 Oct 2026), including the blocked form-a gate 2 |
| `0211-step-accuracy-pencil-timing.txt` | Isolated pencil timing at source `f7a770d`, five runs at one and four threads |
| `0211-flat-cutter-sampling-diagnostic.txt` | Analytic flat-cutter plane case showing the M2 sampling miss (helper source `88f8d5c`) |
| `0211-m1-triangle-coverage.txt` | M1 triangle coverage regression (20,002 plane triangles; parent `fcbec11`) |
| `0211-uncapped-m1-evidence.txt` | M1 with every triangle measured (routines at `88f8d5c`), original and tighter candidates |
| `0211-stage-timing-smoke.txt` | Stage timing integration smoke on a cylinder, contended, not performance evidence (parent `f052ee8`) |
| `0211-quiet-pass-timing-evidence.txt` | Timing forwarding through the quiet axial and indexed context (parent `a0eb00a`) |
| `0211-integrated-validation.txt` | Validation of the 24 Oct merge of main `1a90435` into source `1248937` |
| `0211-full-stepover-m4-evidence.txt` | Full-stepover M4 after the #324 correction (24 Oct 2026) |
| `0211-full-sequence-m3-evidence.txt` | Full-sequence M3, five runs on each form, source `eee4486` (25 Oct 2026) |
