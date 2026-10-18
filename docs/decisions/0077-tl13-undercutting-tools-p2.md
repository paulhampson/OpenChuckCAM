# 0077: undercutting tools and thread mills as a P2 requirement (TL-13)

- **Status:** accepted by the project owner (17 Oct 2026, in the orchestration session): the new requirement TL-13 at priority P2, as worded below.
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): no new ones. Lane TL's OI-281 (thread mills and other undercutting tools, left out of the default library) is on lane TL's branch; at its release push the lane makes it cite TL-13.

Numbering: the orchestrator took the next free record number, 0077, across `main` and the unmerged branches (0081 and later are lane TL's, lane E2's, lane TR's, lane TE's and lane TS's blocks).

## Context

Lane TL builds the default tool library ([decision 0060](0060-gc16-command-line-reading-and-tl12-default-tool-library.md), TL-12). Makera's bit pages list thread milling bits (M3 to M5 in the 1/8 in set, M6 and M8 in the 6 mm set). Neither TL-02 nor TL-03 has a tool type that fits them: a thread mill is an undercutting tool, with a pitch and a profile that neither source gave, so lane TL left them out of the default library.

The project owner directed that thread mills are an undercutting tool and should be handled as a P2 item.

## Decision

1. **New requirement TL-13 (P2):** "The tool library supports undercutting tools, starting with thread mills: a profile that cuts below an overhang or in a bore, with its thread pitch and diameter, so operations can use it for thread milling and undercut features. The default tool library does not ship thread mills until this is built."
2. **Priority P2.** It is not part of the first release. It needs no slice yet; a future slice adds the tool type or profile, the operations that use it, and the tests. The verification plan row for the P2 tool library requirements (TL-08, TL-09) gains TL-13.

## Alternatives considered

- **No requirement, an open item only.** Not chosen: the owner asked for a requirement, and an open item would not show in the requirements or the traceability.
- **P1.** Not chosen: the first release does not need undercutting tools, and no source gives the thread mill geometry.

## Consequences

- **Documents:** `docs/requirements.md` (TL-13), the verification plan (the TL-08, TL-09 row) and the decision index.
- **P1 count:** unchanged; TL-13 is P2, so the traceability check does not require a test for it.
- **Lane TL:** OI-281 and decision 0081 cite TL-13 once this record is on `main`.
