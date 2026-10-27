# FACING implementation and draft handoff

Local implementation and validation are complete for the FACING draft handoff.
No owner question is pending. This report supersedes earlier preparation
milestones; decision 0244 is the accepted design authority. Merge Manager
controls release under 0055.

## Scope and baseline

Issues #286, #287 and #288; branch `codex/lane-facing-ends-sides-and-extents`,
started from merged main `e2189f07fe8dee5a1bd15300babed08f89e6e853`.
No unmerged prerequisite or cherry-pick was used. Prerequisite merge commits:

| Prerequisite | Commit |
| --- | --- |
| CUTOPTS #277 | `ba829c1677c74bf90aec61c39aba8201cd5f5350` |
| STEPALL-F #274 | `f9474af0ec1651a663ca7227efc40dbc85b2fb0f` |
| SWARF3 #320 | `e590f4de67bbde573edf30982982d6d0c5d92707` |
| ROUGHPATH #340 | `ab863f6531d1e8fab8c2562e88d92b9277937c3f` |

[0244](../decisions/0244-facing-ends-sides-and-extents.md) was accepted directly
by the owner after full presentation on 26 October 2026. Its substantive text is
unchanged. The old unpublished 0237 claim was superseded. No new open-item or
project-schema number is used. Number allocation was owner-authorised, subject
to Merge Manager conflict review. Regular watches remain stopped; cross-lane
coordination goes through the owner, not comments on other lanes' PRs.

## Implemented behaviour

- **Unheld end:** exactly one held end selects the opposite; both/neither refuse.
  The target is the placed part end plus existing stock allowance outward once:
  start held gives `p1+s`, far held gives `p0-s`. Display target X and
  `Length to opposite model end = p1-p0+s`, excluding tabs and held stock.
  Real XYZA flank cutting uses equal axial slices within the entered maximum,
  finite flute bands and tangent indexed views around a supported cylindrical
  tab. No fictitious tip orientation or tab removal is used.
- **Box sides:** individual +Z,+Y,-Z,-Y faces and All four, plus ordered arbitrary
  A rows. Positive-sense standard A is 0,90,180,270 degrees; shared machine sense,
  frame/range and rotary placement determine actual motion. Standard faces run
  in fixed order; arbitrary rows have Move up/down and stable Sort by A.
  Per-face offsets default to whole-part support plus allowance, distinguish
  explicit edits, and Reset restores derivation. Opposite-plane separation is
  the sum of their offsets. Full selected steps plus a shortened final layer is
  default; Equal steps is selectable. Legacy equal-layer arithmetic is retained.
- **Extents:** Whole stock is default. Part is the smallest facing-axis-aligned
  rectangle enclosing the entire placed part projection, not its silhouette,
  convex hull, target-plane section or projected world AABB. End usesYZ; sides
  useX/transverse. Part plus tabs bounds their union and preserves all tabs.
  Nonnegative margin defaults to zero and grows the four planar edges. Existing
  Round uses the corresponding full-circumference axial span and axial margin.
  Necessary cutter overlap is separate, derived from actual motion and previewed.
- **Reporting and diagrams:** immutable shared FacingDisplay supplies viewport
  and Setup front/end guides, targets/dimensions, coverage, margin and finite
  cutter sweeps. Edited projects suppress stale sweeps. Operation-local completion
  and Intentionally unfaced stock do not suppress whole-part misses, other
  operations' misses or any overcut. Simulation resolution remains explicit.

Coverage, finished targets and protected solids remain separate. Requested
coverage is never silently cropped around an inaccessible region. Independent
checks consume actual assembled, connected and posted motion; candidate pass
counts, clear endpoints and absence of Warn-only warnings are not clearance
proof. Missing required coverage or forbidden removal refuses usable output.

The proofs check complete supported tool bodies, actual preceding flute removal,
maximum layer engagement including overlap outside the rectangle, protected
part/tabs/held regions, XYZ/A travel and rotary/linear feed. Fed entries/links
outside Box coverage must establish complete-tool air separation. The all-angle
radial bound uses analytic cylindrical-stock radius and conservative bounds for
part/protections. Both decimal post words and binary32 modal replay are checked,
including actual counter offsets. The established Z1 M5/G53Z-3/G28/M2 footer is
preserved and checked against whole-setup clearance, safe height and all home
travel axes; arbitrary unverified frame/home changes refuse.

## Compatibility and API migration

Legacy Flat/Round, default Flat/angles 0, size semantics, Whole stock/zero margin,
extension-off and equal depths remain. Whole-stock/zero-margin legacy generation
keeps its original branch. Pre-scope IR/diagnostic fingerprints remain identical
at 1/2/4 threads: Flat `2ed1e05d2c113015`/346775bytes and Round
`9a9c29f3ec238440`/184577bytes. Existing post formats and committed goldens are
unchanged. Same-version legacy output identity is preserved; across releases,
the generator-version comment changes to0.22.0. The previously accepted ROUGHPATH
rotary correction is already in the baseline, not a new FACING exception.

Under 0048 the family/workspace minor version is 0.22.0. `StrategyInput` adds
`facing: Option<&FacingSetup>`; other strategies and legacy default hosts pass
None. New/restricted Facing requires whole placed geometry, stock, configured
tool/machine, held intervals, separate tabs/protections and a source-error bound.
Constructors/examples and changelogs are adapted. No project-format migration:
new settings use existing generic parameter storage, including typed row data
encoded in a text value. Absent keys retain legacy defaults; inactive edits are
retained rather than discarded or applied in the wrong mode.

Shared provided hooks (Result is `openchuck_path::Result`):

```rust
fn schema_for(&self, values: &BTreeMap<String, ParameterValue>) -> ParameterSchema;
fn resolve_parameters(&self, values: &BTreeMap<String, ParameterValue>) -> Result<Parameters>;
fn validate_path(&self, input: &StrategyInput<'_>, parameters: &Parameters,
    path: &GeneratedPath, context: &JobContext<'_>) -> Result<()>;
```

`schema()` remains the union of known names. `schema_for` defaults to it;
resolution rejects unknown names, filters known inactive entries and resolves
active defaults/constraints. Common forms, required fields, edits and generation
use active settings. Final validation defaults to no-op and runs after assembly,
checks and diagnostics on plain, region and zoned generation; errors/cancellation
return no usable path. Existing FINSTEP zoned warnings/results are preserved.
Optional crate-private legacy Facing scope limits per-A Flat endpoints or Round
axial span while retaining original Job/HeightStock bounds; None is unchanged.

**Owner-approved merge order, required in the PR:** FACING delivers
`Strategy::schema_for`/`resolve_parameters` and common host wiring first. Merge
this FACING PR before FINSTEP consumes those hooks from current main; no
cherry-picks or dependency on unmerged FACING commits. FINSTEP owns adaptive
finishing overrides, including its single height control: Scallop default 0 when
adaptive is off; Allowed excess default 0.01mm when on; explicit handling of saved
values when toggling. A test-only consumer checks these defaults and active edit
constraints here; no production adaptive finishing override was changed.
ROUGHPATH's overlapping entry/assembly/rotary changes are already merged.
Merge Manager controls readiness, current-main updates and merge under 0055.

## Verification

Requirement-based failures were observed before fixes, including initial missing
APIs/disabled public modes, missed final coverage, deeper cutter overlap without
prior removal, stepdown relaxed by finishing tolerance, tab work-budget/posted
residual failures, missing rotating sweep preview, active-schema edit constraints,
footer XY home travel, extra outside-extent entry cutting and overly conservative
round-stock indexing. Independent analytic geometry, mutated actual paths and
both real posts supplement simulator checks.

Uninstrumented runs (Linux x86_64, AMD EPYC9V74, five visible CPUs and four-CPU
cgroup quota). Tests use repository optimization; recent builds set command-local
debug symbols/incremental off to fit disk. Timings exclude compilation.

| Check | Result | Seconds |
| --- | --- | ---: |
| Path/G-code broad run before final guards |447passed;13existing long skipped|194.928|
| Final nominal End/Box/motion focused run |28passed|0.830|
| Final extent guard after lint refactor |16passed|0.861|
| Final application broad run |519passed;21existing long skipped|445.322|
| UI broad run |485passed;2existing long skipped|260.660|
| Simulator own regression run |50passed;6existing long skipped|142.186|
| G-code own final observer/timing tests |22passed|0.276|
| Isolated representative cost cases |2passed|1.776|
| Expanded all-mode thread determinism |1passed;30regeneration/post runs|18.828|

All 15 nonempty side combinations, both A senses, both layer choices and both
posts are exercised (120 posting runs), plus ordered fractional/arbitrary angles.
End posting covers both held directions and cylindrical tabs of radii 1 and 5.
Canonical paths and both posts match at 1/2/available-N threads (N=4 in the final
run) for End, Flat, Round and both Box layer choices. A rounded saturated Round feed correctly refuses; the independent
controller fixture uses requested feed 600 instead of relaxing the axis limit.

Five application cases exceeded a minute under contention:75.152,61.950,79.254,
71.006 and64.086s; one existing simulator envelope case took75.124s. They are
listed in both required nextest profiles, remain in default tests, and are
selected by nightly long. The cylindrical-tab case takes 22.985s in isolation.
No individual uninstrumented test exceeded ten minutes. Coverage timings below
are instrumented and are not used for long-test classification.

Representative application regeneration/post/verification costs, isolated.
LinuxCNC runs first; the subsequent Z1 run may reuse canonical operation caches.
These are representative job costs, not a controlled post-to-post comparison:

| Mode | LinuxCNC / Z1 milliseconds | Moves | LinuxCNC / Z1 bytes |
| --- | ---: | ---: | ---: |
| Box |22.6/18.7|143|1887/1897|
| End |35.5/32.0|195|2547/2557|
| Flat |51.5/23.9|183|2583/2593|
| Round |470.2/313.2|2605|44685/40863|

Own-crate coverage uses `cargo llvm-cov nextest --profile coverage -p CRATE
--locked --no-report`, exports LCOV/text without excluding production code,
inspects changed files and runs `cargo llvm-cov clean --workspace` after each.
It is a lower bound from that crate's own selected tests, not nightly whole-suite
coverage. The 95% development target is not a merge gate; below-target results
are reported without filler tests or exclusions. These full-run snapshots precede
the final test-only determinism expansion and lint-only bounded-iteration edit;
the latter also passed a 16-case uninstrumented guard rerun.

| Crate | Own cases / seconds | Covered source lines | Percent |
| --- | --- | --- | ---: |
| path |428/427.311;13skipped|16611/18137|91.586|
| app |514/466.240;26skipped|18561/20247|91.673|
| G-code |22passed|928/979|94.791|
| UI |485/647.861;2skipped|26187/29923|87.515|
| simulator |49/449.633;7skipped|1558/1722|90.476|

Path's text summary warns 313 functions have mismatched LLVM data despite a clean
coverage-directory rebuild; LCOV export succeeds. This warning limits confidence
in that coverage summary; no 95% claim is made. Simulator's text summary likewise warns 21 mismatched functions. App/UI reports
have no such warning. Raw LCOV/text exports were retained for inspection; the
generated instrumentation directory was removed after all reports.
Changed Facing written verification is 88.6%, display 90.476%, strategy 93.158%
in their owning runs. Gaps include unsupported/budget/precision refusals,
malformed metadata/footer/controller cases, display budget/failure branches and
excluded long fixtures. The new UI row controls are 92.5%; UI event/failure
branches and other unrelated headless gaps also remain. End multi-slice verification is 57.931% in path's own
run and 83.448% through the app run; independent app slice/engagement tests exercise
the actual assembled case. Existing unrelated code also contributes to each
crate's aggregate. Full source and line reports were inspected, not just totals.

Final workspace Clippy with warnings denied passes; formatting/diff checks pass;
workspace doctests pass. `cargo xtask all` passes dependencies, licences, records,
test data, conflicts, publishing and 99/99 P1 traceability. `cargo xtask golden`
passes. All four library API checks accept 0.21->0.22 under 0048. cargo-deny passes
advisories/bans/licences/sources; cargo-audit passes 449 dependencies against 1378
advisories; cargo-about generated 312078-byte notices. Earlier disk-exhausted
build attempts were repeated successfully and are not counted as passing runs.

Independent LinuxCNC 2.9.10 replay passes all five programs at the unchanged
repository gate tolerance 0.1 (reported coordinate units, mm/degrees), with no
controller fault:

| Program | Reached moves | Largest reported difference | Motion seconds |
| --- | ---: | ---: | ---: |
| Box |144|0.0013|45.0|
| End |196|0.0013|60.9|
| Flat |170|0.0482|14.9|
| Round |2606|0.1000|218.5|
| Existing small-part reference |29|0.0010|26.7|

The standard image is
`ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10`, digest
`sha256:22d8b9f0acb8a0fd7a02bf47be7fd9e864e671bd510ca27ab35ff214d1489546`.
No tolerance was relaxed. The first attempt failed before producing reports:
image user cnc is UID 1001, while source fixtures were readable only by workspace
UID 1000. Making these public test fixtures readable (no content/Git-mode change)
allowed the complete rerun to pass. Only the successful rerun is counted.
Raw controller reports were retained outside the repository.

Reproduce by retaining programs from
`req_op_24_req_gc_08_controller_programs_cover_all_modes_with_bounded_run_times`
with `OPENCHUCK_FACING_PROGRAMS=/tmp/facing-programs`, then running
`cargo xtask replay --docker ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10
--timeout 540 --keep-reports /tmp/facing-controller-reports` with those four
LinuxCNC programs and `testdata/golden/post_linuxcnc_small_part.ngc`.
Keep the machine otherwise idle for real-time replay. The normal Docker endpoint,
proxy and CA configuration remain in effect; no proxy or network bypass was used.

## Supported-proof limits and remaining checks

- Exact placed triangles are supported. Unknown STEP projection accuracy and
  nonzero unproved source error refuse the affected operation; tessellation
  settings do not stand in for a bound.
- New final proofs require the supported flat cylindrical tool, without a wider
  shank. Unsupported holder/tool geometry is not guessed. End routing supports
  the demonstrated cylindrical-tab construction; inaccessible growing frusta,
  multiple/unproved tab arrangements and unavailable clear entry refuse.
- New End/Box stock-edge extension currently refuses enabled requests. Legacy
  extension keeps its existing stock-edge-to-near-tool-side distances, default
  off and zero-to-diameter range. Margin does not substitute for extension.
- Restricted legacy coverage refuses raised composite geometry, rotary/stock
  arrangements or written precision not covered by its proofs. Legacy default
  geometry is preserved and does not gain a new full-tool certificate.
- Whole-stock Box coverage intersecting held geometry refuses; it is not silently
  cropped. Travel/feed limits, unrepresentable precision and finite work budgets
  can also refuse otherwise geometric requests. No successful partial face is
  published. Round-bar new side scope remains deferred; existing Round remains.
- Optional native STEP-kernel feature suites, all pre-existing nightly long cases,
  other operating systems and hands-on graphics/machine verification were not
  completed locally. Relevant default/headless tests and software rendering run
  here; heavy CI is for Merge Manager release. Existing NFR-12 user verification
  remains required before machining.

## Delivery

The draft references all three issues and includes the shared-hook ordering
above. Human author and committer: Paul Hampson <528002+paulhampson@users.noreply.github.com>, the linked GitHub noreply address for the connected account. GitHub rejected
the private-email push before publishing it; the unpublished commit was amended
to this address without changing account privacy settings. Agent credit:
`Co-Authored-By: Codex <noreply@openai.com>`. Commit/push once for initial delivery;
report the actual head/PR URL in the draft and its Merge Manager handoff comment.
Merge Manager owns readiness, ordered main updates and merge. Remain available
for corrections and batch review fixes under 0055; do not restart regular watches.
