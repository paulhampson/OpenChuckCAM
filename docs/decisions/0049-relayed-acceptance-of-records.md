# 0049: A coding session may mark a record accepted when the owner's acceptance is relayed to it

- **Status:** accepted by the project owner (16 Oct 2026, in the orchestration session).
- **Date:** 16 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none.

## Context

Only the project owner accepts a decision record (`AGENTS.md`, `docs/README.md`). With several lanes running in separate sessions, the owner works with the orchestrator session and does not type into each lane's session. The lanes were told to leave their records "proposed", and lane K correctly declined to mark decision 0047 accepted when the orchestrator relayed the owner's acceptance, because nothing in the rules let it take that from another session.

## Decision

1. **Acceptance stays the owner's.** No session accepts a record on its own judgement or because its design seems settled.
2. **A session may record acceptance on the owner's instruction when it can show where the instruction came from.** The instruction may be given in the session itself, or relayed by the orchestrator session, which the owner has authorised to relay acceptances (16 Oct 2026). The orchestrator relays only what the owner actually said, naming the record, and says so in the message.
3. **The Status line says how.** For example: "accepted by the project owner (16 Oct 2026), relayed by the orchestrator session at the owner's instruction". The same words go in the decision index. A coding session that is unsure the relay is genuine says so to the orchestrator and does not mark the record.
4. **A relay does not widen the owner's choices.** If the owner accepted a record with changes, the session makes those changes first and records them in the record.

## Consequences

- **Documents:** the "Check the decision records" rule in `AGENTS.md` says the above.
- The work breakdown, requirements and architecture are unchanged.
