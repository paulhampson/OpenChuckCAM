# 0106: The Z1 air-cut check is a manual release-checklist step owned by the owner

- **Status:** accepted by the project owner on 17 Oct 2026, in the orchestration session (Decision 28, option B).
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none are new.

Numbering: the orchestrator took the next free record number across `main` and the unmerged branches (0105 is the highest).

## Context

The Makera Z1 conventions were derived and are confirmed on the machine only in part (OI-37, the SP-12 air cut of decision 0007). Slice 20 item 10 prepares the air-cut procedure and item 11 has the hands-on gates 4 and 5 on the Z1 for the reference job set. Nothing said who runs them, what they cover for the release, or whether a release tag can be cut without them. The interface already shows each machine value as confirmed, vendor-supplied or an unconfirmed placeholder (UI-13).

## Decision

1. **The air cut (gates 4 and 5 on the Z1 for the reference job set, slice 20 item 11) is a manual step on the release checklist, owned by the owner.** The owner runs it before the release tag is cut.
2. **`cargo xtask release-gate` does not enforce it.** The gate keeps its automated checks (dependency rules, deny, golden G-code comparison, LinuxCNC replay).
3. **The result is recorded** as a decision record, and the machine profile values it confirms change from unconfirmed to confirmed.
4. **The release notes say which Z1 values are confirmed.** If the air cut has not happened, a release may still be tagged, with the Z1 values labelled unconfirmed as the interface already does (UI-13).

## Alternatives considered

- **A release gate that refuses a tag without a recorded air-cut result.** Safest, but it blocks every release until the owner has run the machine. Rejected by the owner.
- **Not tying 1.0 to the air cut at all.** Rejected by the owner; the air cut stays on the checklist.

## Consequences

- `docs/work-breakdown.md` slice 20 item 11 names this record. No requirement, architecture or code change.
- NFR-12 still applies: generated G-code is the user's to verify before running on a machine.
