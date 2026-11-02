# 0024: Pathing review, two P2 requirements and one open item

- **Status:** accepted by the project owner (14 Oct 2026): the rotary acceleration limit as a P2 requirement (SET-08), the finishing patterns as a P2 requirement (OP-19) and the precision open item (OI-105).
- **Date:** 14 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-105 to OI-107 are new.

Numbering: record 0023 and OI-103, OI-104 are claimed on the unmerged branch `claude/nightly-build`, so this record takes 0024 and OI-105 to OI-107.

## Context

A review of current 4-axis pathing practice (indexed, wrapped, rotary passes, simultaneous, swarf, rotary roughing, rest and pencil, radial drilling; feed, collision, simulation and motion quality) was checked against the repository at `78d2477`. Most of what it proposed was already settled: the G94 and G93 tests by [decision 0007](0007-sp-12-z1-conventions.md), the swarf scope by [0021](0021-sp-11-swarf.md), the stock representation by [0022](0022-sp-6-simulation-stock.md), and the drop-cutter by [0012](0012-sp-13-tool-profiles.md). Three proposals were not settled and the owner accepted them as follows.

## Decision (accepted)

1. **SET-08 (P2): optional rotary acceleration limit in the machine profile.** Maximum feed rates stay in SET-01 (the Z1 holds 3600 deg/min on A, [decision 0013](0013-z1-profile-consensus.md)). An acceleration limit is used by time estimates (SIM-08) and motion smoothing when set; unset means no limit is assumed. It is P2 because P1 emits contact feed and duration and leaves timing to the controller. Verification: a unit row in the verification plan. The Z1 value is not known (OI-107).
2. **OP-19 (P2): geodesic-parallel (constant scallop) and spiral finishing patterns,** selectable per operation, with scallop height as the stepover control. They sit beside the P1 baseline patterns of OP-04 and the simultaneous pass of OP-05, which keep their scope. Verification: a system row in the verification plan. The method is open (OI-106).
3. **OI-105: rotary positioning precision against the 0.01 mm tolerance is recorded, not turned into a requirement change.** NFR-05 is a computation tolerance for geometry and toolpaths, so it stays as written. The question of what the machine can hold, and where the user is told, is logged for gate 4 or 5 or the setup sheet design.

Document edits made with this record: `requirements.md` (SET-08, OP-19), `verification-plan.md` (two rows), `work-breakdown.md` (the requirement counts in the opening note: 122 requirements, 43 outside this phase), `open-items.md` and the index in `decisions/README.md`.

## Alternatives considered

- **SET-08 as P1:** declined by the owner; nothing in P1 needs it.
- **Change NFR-05 to depend on radius:** rejected; the tolerance is about calculation, and the machine's accuracy is a separate matter (OI-105).
- **Fold the patterns into OP-05:** rejected; OP-05 is P1 and the patterns are a P2 extension with their own method question.

## Consequences

- No change to `architecture.md`: both additions are P2 and use the existing machine profile and strategy interface (OP-16). The P2 list in section 11 already names strategy and simulation work in general terms.
- Slice 3 (machine profile) is unchanged; SET-08 adds a field when it is scheduled.
- Requirement IDs SET-08 and OP-19 are new and are never reused.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Rotary precision against 0.01 mm (OI-105) | Gate 4 or 5, or the GC-12 design | Measurement or Makera's figure |
| 2 | OP-19 method (OI-106) | When OP-19 is scheduled | A spike if needed |
| 3 | Z1 rotary acceleration (OI-107) | When SET-08 is scheduled | Makera's figure or a measurement |
