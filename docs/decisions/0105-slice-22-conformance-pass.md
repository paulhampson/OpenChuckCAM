# 0105: Slice 22 becomes a conformance pass against the interface specification

- **Status:** accepted by the project owner on 17 Oct 2026, in the orchestration session (Decision 26, option A).
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none are new.

Numbering: the orchestrator took the next free record number across `main` and the unmerged branches (0104 was the highest).

## Context

Slice 22 was added on 15 Oct 2026 as a redesign of the interface to the owner's updated design, run once after the screens existed. Since then the interface specification and its mock-ups (decision 0038) have become that design, and the lanes that built screens (S, O, W and 21d among them) built to it. A redesign pass would repeat that work.

## Decision

1. **Slice 22 is a conformance pass.** It reviews every screen, panel, dialog and overlay against `docs/ui-design/ui-specification.md` and its mock-ups, fixes the differences or records them as open items, updates the screenshots and the UI smoke test, and reruns the 1M-triangle frame benchmark. It is not a redesign.
2. **Rework is expected and accepted.** Earlier screens were built to the specification as it stood when their lanes ran; the specification has changed since. Some rework of earlier interface work is planned in this slice and is not a defect of the slices that built it.
3. **It starts after lane O (#83) and lane 21d have merged,** because both change the viewport, the selection panel or the workholding panel that the pass reviews.
4. **First step:** a conformance report as a decision record. A difference that needs a requirement or specification change is raised for the owner, not built. No project format change and no engine change; if one is needed it is a separate lane with its own schema claim.

## Alternatives considered

- **Cancel slice 22.** The conformance checks would be spread over the slices that built the screens, with no single review and no screenshots pass. Rejected by the owner.
- **Keep it as a redesign waiting for a new design.** Nothing in the repository or the plan asks for a design beyond the specification. Rejected by the owner.

## Consequences

- `docs/work-breakdown.md` slice 22 is rewritten (goal, status, needs, design refinement, checklist wording).
- No requirement, architecture or specification text changes here.
