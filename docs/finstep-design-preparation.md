# FINSTEP working design preparation

Reconstructed 30 October 2026. This is working preparation, not an accepted
specification or a recovered copy of the earlier file.

See [the reconstruction handoff](handoffs/finstep-handoff.md) for the authoritative
checkpoint inventory, individual owner choices, pending layer-depth default,
source/evidence limitations and complete rebuild/verification checklist.
[Decision0239](decisions/0239-adaptive-axial-pass-spacing.md) is proposed and needs
separate full-text acceptance. OI-1341 tracks off-axis engagement.

## Implementation map to re-audit

- Existing helical/axial finishing shares wrap fitting; indexed finishing uses
  AlongX at explicitly requested A orientations. Reconfirm current exact-drop
  dispatch before adding the adaptive-only bounded route.
- Simultaneous axial passes and roughing/Facing axial forms use other planners.
  Audit them explicitly. Preserve intentional intermediate layers, selected
  orientations and legacy Facing contracts.
- Refine detected excess locally against the correct target/stock allowance;
  observations without a justified bound remain unverified. A vertical wall
  must not cause endless subdivision. Candidate partial arcs are optional where
  continuous cutting is allowed, not a blanket ring requirement.
- Keep complete protected geometry and stock/target uncertainty distinct from
  coverage windows. Final canonical entry/link/protection/replay checks cannot
  be disabled by adaptive limits or export consent.
- FACING's merged mode-aware parameter hooks should be consumed, not duplicated.
  Default height depends on mode only when absent; saved legacy zero requires
  the approved explicit explained conversion before enabling.
- Reconstruct deterministic shared budgets and typed result-bound diagnostics,
  then operation adapters and off-output regressions. Do not add private unused
  stubs solely to resemble missing uncommitted source.

## Layer decisions

The owner approved tool-axis stepdown and indexed inclusion at existing fixed A
orientations. OFF is the default with exact previous output. The final completed
pass reaches the target including stock to leave; invalid depth/flute overrun
refuses. No engagement guarantee is implied by geometric spacing.

Pending depth-default recommendation: explicit operation value first, otherwise
selected tool/material stepdown, with an explicit value required when neither
supplies one. This has not been approved. Preserve accepted0230 preset semantics
if selected. Then establish first-layer/available-stock and layer distribution
individually, and obtain approval of missing requirement/verification wording.

Off-centre example: at across-axis offset3mm and tool height8mm, adding1mm along
machineZ changes rotary radius by sqrt(3^2+9^2)-sqrt(3^2+8^2), about0.943mm.
Adding1mm to rotary radius instead changes Z by about1.060mm. This is analytic
geometry illustrating the approved depth distinction, not an engagement or
implemented-layer measurement. OI-1341 requires separate stock/swept-motion work.
