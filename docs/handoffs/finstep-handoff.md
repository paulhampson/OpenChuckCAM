# FINSTEP reconstruction handoff

Date: 30 October 2026. Repository: paulhampson/OpenChuckCAM.
Branch: `codex/lane-finstep-adaptive-spacing-and-layers`.
Scope: #303 adaptive pass spacing, followed by #314 optional finishing layers.

## Checkpoint status

This is a reconstruction checkpoint, not a completed implementation or release.
It preserves the recoverable owner decisions and implementation inventory.
The earlier FINSTEP source changes were uncommitted and are not present in the
published repository. Their exact source, raw logs, coverage files and timing
artifacts have not been recovered. This checkpoint does not recreate or claim
to validate those missing artifacts.

The checkpoint starts from merged main `4dbccee8a6e07d387c5b9c5a3b62cc2ae4149f95` (library family 0.27.0).
No unmerged prerequisite is adopted. Product source remains that of this base.
No FINSTEP PR is opened, release version chosen or project schema claimed.
The owner authorized publishing the reconstruction checkpoint. The eventual
delivery remains ONE FINSTEP draft PR for both issues, with implementation,
documentation, decisions and verification together. Do not mark ready, merge or
enable auto-merge.

## Owner decisions to preserve

These are individual choices, not acceptance of the complete proposed
[decision 0239](../decisions/0239-adaptive-axial-pass-spacing.md).

### Adaptive spacing (#303)

- Measure excess/cusp height above the operation's own intended target, including
  stock to leave and intentional intermediate layers. Intended allowance is not
  missed material. Never refine below that allowance.
- Apply the adaptive treatment to helical finishing and every operation that
  generates axial passes. Audit indexed finishing, simultaneous axial pattern,
  both roughing axial forms and legacy flat Facing, not only the original
  helical/axial implementations. Configured stepover is a maximum, reduced
  locally where needed. Preserve legacy Facing defaults/equal-depth arithmetic.
  Do not silently extend non-axial strategy patterns.
- Preserve explicitly fixed A orientations. Do not add cutting angles or rotary
  cutting to an indexed/chosen-side operation.
- Circular cuts are possible cleanup outcomes, not mandatory rings. Consider
  starts/ends, before/after depth changes and vertical faces. Partial cleanup
  arcs are approved where rotary cutting is permitted; retain allowance,
  region/protection, cutting direction, travel and entry/link checks.
- One visible height control: legacy Scallop with adaptive OFF, Allowed excess
  with adaptive ON. Preserve legacy Scallop behavior exactly when off.
  The ON default is 0.01 mm; OFF Scallop default remains 0 mm.
- Defaults apply only to absent values. Reuse the existing helical/axial height
  value and provide equivalent controls for newly applicable axial strategies.
- For saved legacy Scallop zero, enabling adaptive requires deliberate height
  confirmation. Suggest 0.01 mm, allow deliberate zero, and explain: legacy zero
  meant disabled; adaptive zero is a strict zero-excess target and may remain
  unverified or hit limits. Cancel changes nothing. Confirmation applies mode
  and height atomically in one undo step. One value is shared: switching off
  retains the newly chosen height as Scallop, without restoring the old zero.
- Per-operation only. Adaptive ON for NEW applicable operations; migrate existing
  operations to explicit OFF and preserve explicit settings. No global default.
- Minimum further-refinement spacing defaults to 0.01 mm, editable. Do not enlarge
  a configured maximum that is already smaller than that floor.
- Initial per-operation ceilings, SHARED ACROSS ALL LAYERS: 1,000,000 adaptive
  geometry queries; 1,000,000 emitted points; 10,000 added passes; 20 subdivision
  levels; 65,536 pending intervals. These bound work, not elapsed time or all RAM.
- The query ceiling covers adaptive planning, assessment and candidate fitting.
  Mandatory final protection/entry/clearance/replay checks run fully under their
  own contracts and limits. Report their costs separately; total generation
  queries may exceed one million. Point accounting includes canonical operation
  motion with entries and links, not just raw candidate points.
- Retain valid partial paths on unmet/unverified adaptive coverage, show a
  prominent warning and require acknowledgement of the CURRENT result before
  export. Never label an acknowledged result complete. Invalid input,
  cancellation, gouge/protection, entry, clearance and replay failures stay hard
  errors. Consent never waives FACING's strict completion checks.
- Finite observations meeting the requested height without a justified bound
  remain unverified. Stop locally rather than blindly subdividing uncertainty;
  refine detected excess within the limits. Narrow features can be missed.
- Use the operation's existing validated cutter model, including supported flat,
  tapered and custom tools. Ideal scallop formulas are seeds only. Unsupported
  assessment stays explicitly unverified, never silently ignored.
- Report refinement count, smallest spacing, reasons/affected areas where known,
  uncertainty, progress and generation-stage time.
- Workspace MINOR bump for the intentional default/output change, extending
  decision 0048 beyond API breaks. OFF output must be byte-identical at the SAME
  version identifier; across releases only the required generator-version
  comment may differ. Public API changes still follow 0048.

### Optional finishing layers (#314)

- OFF by default; OFF preserves prior output exactly.
- Include helical, axial AND indexed finishing. Indexed layers retain existing
  requested A orientations and positioning-only transitions; no extra cutting
  angles or rotary cutting is authorized.
- Stepdown is geometric separation ALONG THE TOOL AXIS (machine Z), approaching
  the final target with stock to leave. It is not radial-distance spacing.
- The final completed pass reaches that target. Reject invalid depths and depths
  exceeding flute length. Preserve held regions, tabs and other protections;
  check complete entry/link/clearance/direction/flute implications across layers.
- No maximum material-engagement/removal-depth or uniform-final-allowance promise
  without sufficiently qualified incoming/remaining-stock information.
- The owner requested a follow-up on off-axis engagement, including simultaneous
  XYZA finishing: OI-1341. This does not by itself authorize simultaneous layers.

## Next owner decision: still pending

The last recommendation awaiting an answer is the finishing-layer depth-default
policy: explicit operation depth wins; otherwise use the selected tool/material
preset's existing stepdown; if neither exists, require an explicit positive finite
depth before generating with layers enabled. Do not invent a universal fallback
or silently clamp an invalid depth. Preserve accepted 0230 preset semantics,
including live inherited values and dropping the preset on cutting-value edits.

No answer to that recommendation has been recorded. Authorization to reconstruct
and publish this checkpoint is not that answer.

After that, settle the first-layer/available-stock contract and layer distribution
individually. Distinguish geometric spacing from actual engagement. Present any
missing layer requirement/verification wording before dependent implementation.
Present each complete proposed record separately for acceptance.

## Dependency and coordination state

- SWARF3 #320 is merged: e590f4de67bbde573edf30982982d6d0c5d92707.
- Complete ROUGHPATH #340, GATES #284 and STEPALL-M #321 are merged. ROUGHPATH
  owns roughing/the existing layer engine; preserve canonical group, pass-stock,
  linking and controller-reset contracts.
- FACING #365 is merged at 33a835db7d0c4748f20ee79c5065f1552bdf0ab3.
  Its shared hooks are now available from main. FINSTEP must consume these
  merged interfaces rather than reconstruct conflicting host wiring:
  `schema_for(&self, &BTreeMap<String, ParameterValue>) -> ParameterSchema`,
  `resolve_parameters(&self, &BTreeMap<String, ParameterValue>) -> Result<Parameters>`.
  Full schema recognizes all names; active schema supplies active controls;
  resolution rejects unknown names, filters known inactive fields, then
  validates/defaults active fields. Hosts retain inactive stored edits.
- FINSTEP's two height modes both allow finite nonnegative millimetres; no
  differing numeric constraint is required for their shared stored height key.
- FACING's strict completion/whole-tool proof is separate from adaptive coverage
  warnings. Coverage extents are not protective target geometry. Warn-only
  sampling does not establish full-envelope clearance.
- Later merged STEP work matters to reconstruction: accepted 0255 supplies a
  bounded tool-contact test checker; accepted 0256 records that no tested fixed
  finer toolpath mesh qualifies. Main includes exact-query foundations 0260 and
  exact ball drops 0277. Re-read current source/contracts before replacing the
  old budgeted fitting route; do not force new exact contacts through a mesh
  approximation or call the failed historical sampler a qualified checker.
- Historical STEPALL-M failures were not waived: M1 candidate 0.00570 mm versus
  0.005 allocation; physical M2 sampler miss 0.006666667 mm versus <0.001; rest
  per-operation ratio 2.006793 versus 2x. Later exact/checker evidence has its
  own source and runtime provenance; neither blanket failure nor blanket
  success should be inferred from the historical values.

Coordinate actual new shared edits through the owner; no blanket wait on
unrelated lane work. Do not coordinate through closed/merged lane PRs.
Use the eventual open FINSTEP PR for authorized Merge Manager handoff.

## Earlier implementation inventory to rebuild and reverify

This inventory is recovery guidance, NOT source recovered into this checkpoint.

1. `openchuck-path::adaptive`: deterministic checked resource accounting with
   atomic candidate-group reservations and shared per-operation counters.
2. A local interval scheduler: qualified bounds versus observations; left-first
   deterministic work order; spacing/depth/query/point/pass/queue limits;
   local failure does not abandon unrelated regions; bounded diagnostic count;
   cancellation/error cleanup retires owned reservations only.
3. An adaptive-only bounded fitter reusing wrap fitting, region/leave-zone and
   actual surface/tool models. Charge target, selected, avoid and check queries.
   Reserve every clipped candidate component and end extension atomically.
   A budget-exhausted query must never look like an absent surface.
4. Streamed extension helpers preserving legacy endpoints and exact OFF output.
5. A private point/motion observer following machine-linear XYZ and unwrapped A,
   preserving full turns. Cutting-tool profile distance and conservative motion
   enclosures; rapid motion is not cutting evidence. Boundary/precision
   uncertainty is explicit. A cut point is not an entire-face excess bound;
   an uncut point alone does not prove incoming material exists.
6. Typed adaptive diagnostics through `passes_in_zones` returning
   `StrategyOutput`; `PathWarning::requires_acknowledgement` distinguishes
   adaptive consent from ordinary advisory warnings.
7. App-level immutable current-output-plan consent, UI consent bound to actual
   generated-result identity and an invocation-local CLI
   `--acknowledge-partial-path`. Hard file/replay failures refuse before writes.
   Preserve later merged output naming/reporting/connection work.
8. Explicit legacy-zero conversion model with fresh-state revalidation,
   validation before either edit, an atomic batch/one undo, Cancel/no-op
   behavior and preserved unrelated/inactive values.
9. Requirement tests, legacy spacing example/pilots, source manifests and docs.

The original two adaptive-control integration tests were still RED (unknown
adaptive parameter); product strategy/default/migration integration was unfinished.
The bounded fitter/observer were private and unwired. Strict Clippy was blocked
by 27 unused-private-code diagnostics. No finishing-layer implementation existed.
Do not reintroduce unused stubs merely to imitate that incomplete state.

## Verification provenance and required work

Earlier reported results at preparation base 008e38f:
76 selected path cases passed in 4.362 s; 11 app toggle/export cases in 2.110 s.
Focused coverage reported path 5359/15241 and app 3103/14520 lines, lower bounds
below the 95% development target. The private modules had narrower reported
coverage; these figures are not whole-product coverage or new-code evidence.
Trace reportedly registered tests for 100/100 P1 requirements after a disk-space
retry. Formatting/records/whitespace reportedly passed. Original raw artifacts
are unavailable here; none of these claims verifies this reconstructed head.
Earlier 160/120 fixed-spacing pilot runs were legacy-only, not adaptive or
current-main performance/accuracy results.

No Rust build, test, Clippy, coverage, xtask or timing run is claimed for this
checkpoint. The only reconstructed changes are documentation. Repeat verification
after source reconstruction; do not fabricate red/green logs, hashes or manifests.

Before the eventual single draft handoff:
- Write requirement-based tests first and observe failure before implementing.
- Independently measure emitted AND posted excess, allowance preservation,
  steep/vertical coverage, protections/tabs, layer order/final target,
  entry/link motion and flute reach. Separate tessellation/contact/fitting/
  posting uncertainty; finite probes are not full-envelope certificates.
- Exact OFF bytes at the same version, and deterministic IR/posted bytes at
  1/2/N thread counts, including partial-result resource exhaustion.
- Validate invalid settings, all ceilings, local stopping, progress/cancellation,
  honest incomplete reporting and stale-consent refusal.
- Uninstrumented baselines and five equivalent successful timing runs for
  adaptive, layers and their combination. Do not overlap timing runs with
  builds/tests/lints/coverage. Report stage costs and source/runtime provenance.
- Classify >1-minute cases in both required nextest profiles. Stop and raise
  individual uninstrumented tests exceeding ten minutes.
- Run each changed crate's own coverage; inspect missed changed lines and report
  gaps without filler tests or exclusions; clean instrumentation afterward.
- Formatting, strict lint, records, trace, required xtask/security/semver and
  repository-required checks. Recheck current main, post rounding and quiet-worker
  diagnostic propagation at the actual handoff head.
- Audit/claim any project schema in work breakdown BEFORE using it; preserve old
  explicit settings. Select release numbers only at the appropriate release step.
- Human author/committer Paul Hampson; agent coauthor. One final draft PR
  referencing #303/#314; Merge Manager controls readiness and merge under 0055.

## Numbering

FINSTEP decision 0239 was allocated by Merge Manager in #326 comment6385503719
and acknowledged in comment6404831340. It remains proposed and unaccepted.

The previously selected LOCAL engagement number OI-1003 was never published by
FINSTEP. Current main uses it for an unrelated kernel/runner follow-up; that row
is preserved unchanged. The reconstructed FINSTEP engagement item uses OI-1341,
selected after checking current main, all published branch registers and open
PR descriptions/comments. Follow subsequent Merge Manager corrections. No other
decision, project-schema or version allocation is made here.
