# 0078: TL-12 reworded for the Z1 tool numbers assigned by the project

- **Status:** accepted by the project owner (17 Oct 2026, in the orchestration session): the TL-12 wording below.
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): no new ones.

Numbering: the orchestrator took the next free record number, 0078, across `main` and the unmerged branches (0081 and later are the new lanes' blocks).

## Context

[Decision 0060](0060-gc16-command-line-reading-and-tl12-default-tool-library.md) added TL-12, which said the default library ships "the reference tools for the Makera Z1 with their tool numbers as the machine's default tool set (TL-11)", with `MakeraInc/CarveraProfiles` named as the source.

Lane TL then found that neither Makera's repository nor the community profiles state a tool number per tool (the community files give one number per category, repeated across tools, and the Makera bit pages give none), and that neither repository has a licence file. The owner decided, in the course of the lane's decision record (0081, accepted), that the Z1 reference tools are taken from Makera's published tool pages (with the community profiles only for what those pages lack) and that the tool numbers are assigned by the project, in the form NXX with N a category number of the project's own and XX counting within it. TL-12 as worded no longer matched what is built and accepted.

## Decision

TL-12 now reads: "A default tool library ships with the application: the reference tools for the Makera Z1, taken from Makera's published tool pages, with tool numbers assigned by the project as the machine's default tool set (TL-11), and a set of common standard tools of each type in TL-02 and TL-03. The user can edit, add and delete tools, and an update never overwrites a tool the user has changed." Priority P1 is unchanged.

## Alternatives considered

- **Leave the wording.** Not chosen: the requirement and the accepted decision 0081 would disagree, and a reader of `requirements.md` would expect Makera's own tool numbers.

## Consequences

- **Documents:** `docs/requirements.md` (TL-12) and the decision index. The verification plan row for TL-12 needs no change: it already says the Z1 tool set and does not mention Makera's numbers.
- **P1 count:** unchanged.
- **Lane TL:** decision 0081 remains the record of the numbers, the categories and the sources.
