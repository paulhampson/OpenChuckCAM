# 0239: Adaptive spacing for axial-pass operations

- **Status:** proposed, not accepted. Reconstructed on 30 October 2026 from the
  owner's individual decisions. This complete reconstructed record has not been
  presented for separate acceptance. Individual implementation/policy approvals
  do not accept the full record.
- **Date:** 26 October 2026; reconstruction 30 October 2026.
- **Scope:** FINSTEP #303 followed by #314, one delivery PR.
- **Requirements:** OP-03/04/05/11/12/14/15/21/24, SET-09, GC-07,
  NFR-04/05/06/07/12; TL-05 for the pending layer default policy.
- **Allocation:** FINSTEP0239 assigned by Merge Manager in #326
  comment6385503719, acknowledged6404831340. OI-1341 tracks off-axis engagement.
  No project schema or release version selected.

## Context

Fixed pass spacing and ideal ball/bull-nose scallop formulas alone do not establish
actual excess material on steep or vertical profiles. Fitting samples within one
pass does not choose neighboring passes. The owner expanded adaptive treatment
to all axial-pass operations while preserving intentional layer/stock targets
and explicitly chosen A orientations. Circular motion is a candidate cleanup
outcome, not a mandatory ring or permission to change a fixed-angle operation.

## Individually approved policy

1. Control excess/cusp height above the applicable intended target, including
   stock to leave and intentional intermediate layers. Never refine below the
   allowance or into protected geometry.
2. Configured stepover is a maximum; refine locally where useful. Include helical
   finishing and every axial-pass form, including indexed finishing, simultaneous
   axial pattern, roughing axial forms and legacy flat Facing. Preserve legacy
   Facing defaults/equal-depth layers; do not implicitly change other patterns.
3. Preserve fixed A. Consider starts/ends, before/after depth changes and vertical
   faces. Partial cleanup arcs are permitted only where rotary cutting is allowed,
   preserving direction, allowance, selections/protections, travel, entry and links.
4. One visible height: legacy Scallop with adaptive OFF (default0mm), Allowed excess
   with adaptive ON (default0.01mm). Defaults apply only if absent. Supported tools
   use each operation's validated cutter model, including flat/tapered/custom where
   supported; closed-form scallop estimates are only seeds.
5. Enabling from explicitly saved legacy zero requires explained, deliberate
   height confirmation, suggesting0.01mm. Legacy zero disabled Scallop; adaptive
   zero requests strict zero excess and may remain unverified/limited. Cancel
   changes nothing. Confirm mode/height atomically in one undo step. Keep one
   shared value: switching OFF retains the chosen height as Scallop without
   automatically restoring old zero. Preserve unrelated/inactive edits.
6. Per operation only. New applicable operations default ON; existing operations
   migrate to explicit OFF; preserve explicit settings.
7. Minimum further-refinement spacing defaults0.01mm, editable. Never enlarge a
   configured maximum already below the floor.
8. Per-operation deterministic ceilings shared across all layers: 1M adaptive
   geometry queries,1M emitted points,10k added passes,20 subdivision levels,
   65536 pending intervals. No wall-clock output cutoff. Charge before work/
   allocation and use checked arithmetic.
9. Query accounting includes adaptive planning, assessment and candidate fitting.
   Mandatory final protection/entry/clearance/replay checks run fully under their
   existing limits, with costs separately reported. Total generation queries may
   exceed1M. Point accounting includes canonical entry/link motion.
10. Retain valid partial motion for unmet/unverified adaptive coverage, show a
    prominent warning and require acknowledgement of the CURRENT result before
    export. Acknowledged does not mean complete. Invalid input, cancellation,
    protection/gouge, clearance, entry and replay failures remain hard errors.
11. When finite observations meet height without a justified bound, stop locally
    as unverified rather than blindly subdividing all uncertainty. Refine detected
    excess within limits. Unsupported assessment cannot silently ignore the target.
12. Report refinement count, smallest spacing, limits/unverified reasons and
    locations where available. Progress and generation-stage time include refinement.
13. Preserve exact OFF behavior and posted bytes at the same version identifier;
    across releases only the required generator-version comment may differ.
    Workspace minor bump for the intentional default/output change extends0048.
    Public API changes still follow0048.
14. #314 layers are OFF by default with exact OFF output. The owner approved
    tool-axis (machineZ) geometric stepdown and inclusion of indexed finishing at
    existing fixed A orientations. The completed final pass reaches the stock-offset
    target. Reject invalid depths or depths exceeding flute length. Preserve
    protections and check complete entry/link/direction/clearance/flute implications.
    This is not a material-engagement or uniform-final-allowance guarantee.

## Coordination and implementation requirements

FACING owns the provided schema_for/resolve_parameters hooks and common host
wiring; its PR365 is merged. Consume merged main and provide FINSTEP overrides.
Full schema recognizes known inactive names; active schema supplies active labels,
constraints/defaults; resolution rejects unknown keys, filters inactive fields and
validates/defaults active fields. Hosts retain inactive stored edits. Both height
modes allow finite nonnegative millimetres, so no different host numeric constraint
is selected for their shared stored key.

ROUGHPATH retains roughing/layer ownership. Preserve canonical groups, pass-start
stock, linking and rotary-reset metadata. FACING's complete-tool/strict completion
proofs remain separate from adaptive coverage consent. Coverage is not protective
target geometry; Warn-only samples are not clearance certificates.

Reconstruct typed diagnostics through the zoned path result and result-bound
acknowledgement across application, desktop and CLI. Preserve hard safety and file
checks before writes. Never grade geometry only with the strategy's own sampler.
Requalify against current exact-contact and post/connection interfaces; do not
adopt an unmerged prerequisite or revive historical mesh/sampler assumptions.

## Pending owner decisions

- Layer depth default/material linkage: recommendation is explicit operation depth
  first, otherwise selected tool/material stepdown, requiring an explicit value
  if neither exists. This recommendation has NOT been approved.
- First layer and available incoming stock, layer distribution and associated
  requirement/verification wording. Present choices individually before dependent
  implementation.
- Separate acceptance of this full record after full-text presentation.

OI-1341 tracks material engagement for off-axis paths, including simultaneous
XYZA finishing. Qualified incoming/remaining-stock and swept-motion evidence are
needed before engagement guarantees. This item does not extend layer scope to
simultaneous strategies by implication.

## Alternatives and risks

Keeping only the ideal scallop formula is cheap but does not address general
residual material; retain it on the legacy OFF route. Arbitrary extra cutting
angles or mandatory rings change fixed-orientation intent. Two competing visible
height fields were rejected in favor of one shared value. Wall-clock termination
would make partial output depend on load/thread scheduling. Refusing every partial
adaptive result was not selected; warned, current-result-acknowledged export was.

Closer spacing/arcs increase work, output size and potentially engagement.
Unreachable faces, contact/tessellation/trim uncertainty and finite limits can
leave unverified regions. No warning acknowledgement or extra circle proves
coverage, clearance or machining suitability.

## Verification and delivery

Use the approved requirement/verification wording and the detailed checklist in
[the handoff](../handoffs/finstep-handoff.md). TDD, independent emitted/posted
measurements, exact OFF bytes,1/2/N determinism, bounded termination, protections,
invalid settings, flute/entry/link checks and controlled generation timings are
required. Follow long-test/ten-minute-stop, changed-crate own coverage/cleanup,
strict lint and required repository checks. No historical uncommitted test result
validates this reconstruction.

One eventual FINSTEP DRAFT PR covers both issues, all code/docs/decisions/
verification. No separate preparatory PR. Merge Manager controls readiness and
merge under0055. Human author/committer with agent coauthor. Update specifications,
work breakdown, changelogs and registers in the same delivery.
