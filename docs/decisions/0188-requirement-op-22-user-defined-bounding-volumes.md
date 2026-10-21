# 0188: Requirement OP-22, selection sets from a user-defined bounding volume (lane REQ-VOL; issue 236)

- **Status:** accepted by the project owner (21 Oct 2026), directly in the lane REQ-VOL session ("accep 0188"). The requirement text was first added on the owner's instruction relayed by the coordinator ("Do not implement that new feature, only create a future P2 requirement for it"). Not scheduled and not built.
- **Date:** 21 Oct 2026.
- **Requirements:** adds OP-22 (P2); relates to OP-02, SET-09, GC-07, NFR-05, NFR-06, NFR-07, NFR-12.
- **Open items:** OI-700, OI-701, OI-702 (new).

## Context

Issue 236 (trial feedback F-72, the owner's suggestion): selection sets that are a separately defined bounding volume, a box or a cylinder the user defines, usable within strategies as a boundary, an avoid volume or a check volume, following OP-02. A cylinder fits the 4-axis case (around the rotary axis, with a start and end in X and a radius); a box limits a cut to a region in X, Y and Z. OP-02 covers selected faces and regions, but today's containment boundary is the bounding window of a set's triangles (decision 0032; OI-137 defers other boundaries), so a volume is a new kind of set.

OP-21 is the last ID in the operations group on `main` and on every unmerged branch, so OP-22 is the next free one.

## Decision

1. Add requirement OP-22 (P2) to `docs/requirements.md` and its row to `docs/verification-plan.md`. Only this addition is edited: no other requirement, the architecture or the UI specification.
2. **Not scheduled and not built.** No code, UI, schema claim or lane is added; the work breakdown gets no lane or slice.
3. The owner's open points are recorded in OI-700: the frame (part, stock or machine, and how it follows a part move, with issue 175); how the volume is defined (numbers, handles, from the stock or from faces); a cylinder about the rotary axis only or placed freely, and a box axis-aligned only or not; how volumes combine with face sets and with each other.
4. The build is OI-701 (deferred, no lane scheduled). A stored volume changes the project file, so a schema claim in the work breakdown comes first.
5. A volume must never let a path enter the held region (SET-09; decisions 0160 and 0161; NFR-12). OI-702 records that the build decides and states how a boundary volume that reaches into the held region is handled.

Relationships, as notes and not scope: the selection model (decisions 0032 and 0066; lanes C, M and O), swarf boundary sets (issue 227, decision 0176), selection sets in the inspector (issue 225), and deterministic output (GC-07, NFR-06).

## Alternatives considered

- Treat volumes as part of OP-02. Rejected: OP-02 is P1 and built; a volume is a new kind of set that is not tied to the part's triangles, and a separate P2 entry keeps P1 unchanged.
- Leave the requirement to the build lane. Rejected on the owner's instruction: record the requirement now and build later.

## Consequences

- `docs/requirements.md` gains OP-22 and a note under it; `docs/verification-plan.md` gains its row; `docs/open-items.md` gains OI-700 to OI-702; the decision index gains this record; `docs/work-breakdown.md` lists OP-22 as a future requirement.
- `cargo xtask trace` fails only a P1 requirement with no slice or test, so OP-22 (P2) does not fail it.
- Nothing else follows until a lane is scheduled (OI-700 first).
