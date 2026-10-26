# ROUGHPATH released allocation audit

Main `dc610e26dd260d69b5fc859270e7bf5fc913a3bb`; 28 actual remote heads, 27 with a decision index. Audit reads actual record filenames, indexes, OI registers, work-breakdown claims and project/machine schema constants. No branch is merged or checked out for the audit.

| Remote branch | Exact head | Relation | Records >=0234 | Project/machine schema |
| --- | --- | --- | --- | --- |
| cla-signatures | `55d93fb3659b0c57f6a7cf1d106ee10db48d3e0f` | unmerged/auxiliary |  | -/- |
| claude/ci-speed-up-libs-nextest | `af6a09bc6d3c777a069a32154b2790fc582c493c` | unmerged/auxiliary |  | 10/2 |
| claude/coordinator-notes-owner-test-list | `226e082e61f1f4a8e875bf62b276498037582f0f` | unmerged/auxiliary |  | 13/3 |
| claude/docs-gates-4-5-test-instructions | `9573685001117c4e71a236c555f34138fb21cbad` | unmerged/auxiliary |  | 17/3 |
| claude/lane-stepall-m-measure | `b375262526492737ff609a6968a5501fe6d88b8e` | unmerged/auxiliary | 0234-replay-output-rounding-correction.md | 18/4 |
| claude/upbeat-davinci-jk46yd | `c92fa236ccc8862e5a52e80eade69c73b835d7e0` | unmerged/auxiliary |  | 13/3 |
| codex/bug-simsync-333 | `e17e65e4012780f2aa31acb2c9b15c6573f99e9e` | unmerged/auxiliary | 0235-confirmed-selection-overlay.md, 0236-remove-unused-project-tools.md, 0237-strategy-dropdown-and-information-panel.md, 0238-simulate-display-and-operation-navigation.md, 0240-export-write-results-and-replacement-confirmation.md | 18/4 |
| codex/lane-checkpoint-crash-recovery | `b2e4f859a03108036d955f3f249005558b1982d7` | unmerged/auxiliary | 0235-confirmed-selection-overlay.md, 0236-remove-unused-project-tools.md, 0237-strategy-dropdown-and-information-panel.md, 0238-simulate-display-and-operation-navigation.md, 0240-export-write-results-and-replacement-confirmation.md, 0243-project-recovery-checkpoints.md | 18/4 |
| codex/lane-roughpath-unwind-and-ramp | `32d081254489a02709671c2f63c3f2b6f0d07158` | unmerged/auxiliary | 0241-continuous-rotary-operation-boundaries.md, 0242-optional-back-and-forth-ramp.md | 18/4 |
| dependabot/cargo/ui-graphics-49e8759968 | `5f16937630190956027ee9b5f638f8bbdc45c9a0` | unmerged/auxiliary |  | 18/4 |
| docs/decision-0016-sp-9-exact-surfaces | `68789930b1eaebdc91f64f33de6e77de46a2ea31` | unmerged/auxiliary |  | 4/1 |
| main | `dc610e26dd260d69b5fc859270e7bf5fc913a3bb` | main | 0235-confirmed-selection-overlay.md, 0236-remove-unused-project-tools.md, 0237-strategy-dropdown-and-information-panel.md, 0238-simulate-display-and-operation-navigation.md, 0240-export-write-results-and-replacement-confirmation.md | 18/4 |
| management/merge-queue | `86f2194328356774316c3c5fbc6290f8110283b0` | unmerged/auxiliary |  | 18/4 |
| spike/linuxcnc-fixture-checks | `92dd315d11ed0c85c5b4ee50466af5dd595a3bbc` | unmerged/auxiliary |  | -/- |
| spike/sp-1-step-kernel | `f4a7d04db83c33e9cb4ea5f03ded1b9bbaa62834` | unmerged/auxiliary |  | -/- |
| spike/sp-10-template-engine | `a27134ebc0373714b5dcf10a5b879269c0868840` | unmerged/auxiliary |  | 4/1 |
| spike/sp-11-swarf | `d88f11077c1ac41a18115120a5b27e9d173baaf6` | unmerged/auxiliary |  | 4/1 |
| spike/sp-12-z1-conventions | `4b1ea01d5eb772e2663e6e6d8698ae671b2ff898` | unmerged/auxiliary |  | -/- |
| spike/sp-13-tool-profiles | `cc24ea2848920c6fb4d23a40b9fb2a66a445eded` | unmerged/auxiliary |  | 1/- |
| spike/sp-14-short-moves | `69d532f0723b89667ae792a90a4d4a571300fe39` | unmerged/auxiliary |  | 17/3 |
| spike/sp-2-viewport | `d911116d86d5ca369de851991087f933359bd7e4` | unmerged/auxiliary |  | -/- |
| spike/sp-3-wrapped-path-gcode | `0e251d2d40973cbc043d31e8672a6932ff3228d3` | unmerged/auxiliary |  | -/- |
| spike/sp-4-ray-cast-sweep | `e5ffcd8daaaae4797ed69326fa7783409b36215e` | unmerged/auxiliary |  | 3/1 |
| spike/sp-5-gcode-reader | `0fee6129e5a7051a36e49ec26781c2bda493422d` | unmerged/auxiliary |  | -/- |
| spike/sp-6-simulation-stock | `7156bc1d196ab0038fa92389b0cbc176a5d13d96` | unmerged/auxiliary |  | 4/1 |
| spike/sp-7-2d-offsets | `1f54adde2d9dc46ee4ed5dfcea8e513075921ac2` | unmerged/auxiliary |  | 4/1 |
| spike/sp-8-project-format | `db95e1558b0d34aeaa46272012e79674dcc06422` | unmerged/auxiliary |  | -/- |
| spike/sp-9-exact-surfaces | `b0dca8b57621b23245992d5db6e70510ee0ec918` | unmerged/auxiliary |  | 4/1 |

Distinct filenames sharing an identifier across all fetched records: {"0016": ["docs/decisions/0016-sp-7-2d-offsets.md", "docs/decisions/0016-sp-9-exact-surfaces.md"]}

Persisted reservations: `origin/management/merge-queue:MERGE_QUEUE.md` at 86f2194328356774316c3c5fbc6290f8110283b0. Relevant coordinator map: held STEPALL-M 0234/OI-984; merged 0235 overlay, 0236 cleanup, 0237 OPCREATE2/OI-985–986 CLOSED, 0238 SIMUI/OI-987 OPEN/OI-988 CLOSED; FINSTEP reserved 0239; merged OUTPUT-1 0240/existing OI-580; ROUGHPATH 0241/0242, no OI/schema; CHECKPOINT 0243/OI-989 per management reservation and metadata reconciliation. Project18/machine4 unchanged.

Owner open PRs: #284 GATES (held), #321 STEPALL-M (held), #340 ROUGHPATH (sole ready), #341 BUG-SIMSYNC (draft), #342 CHECKPOINT (draft). Every actual remote head is audited, including non-owner/dependabot/spike refs; no new ROUGHPATH allocation is taken. Persisted reservations remain authoritative even without a published record. Prior reported FACING 0237 is not a published new record in this audit and cannot displace merged OPCREATE2 0237.

The local decision-index union is sorted with one row per intended record. ROUGHPATH accepted records retain original acceptance/provenance and unchanged substantive content. A local records pass is a separate check, not the cross-branch uniqueness evidence.
