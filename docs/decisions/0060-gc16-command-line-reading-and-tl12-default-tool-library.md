# 0060: GC-16 for the command line, and a default tool library (TL-12)

- **Status:** accepted by the project owner (17 Oct 2026, in the orchestration session): GC-16 option A, and TL-12 as worded below at priority P1.
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-268 to OI-271 are new.

Numbering: the orchestrator took the next free record number, 0060, and OI-268 to OI-271, across `main` and the unmerged branches (0059 and OI-263 to OI-267 are lane Gfx's).

## Context

Two agreed changes to `docs/requirements.md`, recorded together.

**GC-16.** It said the user confirms before the first export of a project on a computer. Lane S built the confirmation in the export dialog and, as [decision 0071](0071-slice-19-export-dialogs.md) item 10 (accepted) records, left the command-line tool printing the notice at every run with no confirmation, because it runs unattended in the reference jobs and CI (OI-251, closed). The requirement had no exemption for it.

**Default tool library.** Slice 4 ([decision 0014](0014-slice-4-tool-library.md)) built the library as a folder, import and export, snapshots and machine tool sets, and a sample corpus in `testdata/tools/library/` (decision 0014 item 12: sample numbers, not the machine's layout). Nothing ships to users, so a new user starts with an empty library. No requirement covered shipping one.

## Decision

1. **GC-16 option A.** The requirement gains: "The command-line tool runs unattended: it prints the notice at every run and asks for no confirmation." NFR-12 is met on the command line by the notice. Not chosen: leaving the wording (the code and the requirement would disagree), and making the command line ask (every unattended job would need a flag or a pre-seeded settings file, for little safety since the notice is printed each run). The GC-16 row of the verification plan gains the same sentence.
2. **New requirement TL-12 (P1):** "A default tool library ships with the application: the reference tools for the Makera Z1 with their tool numbers as the machine's default tool set (TL-11), and a set of common standard tools of each type in TL-02 and TL-03. The user can edit, add and delete tools, and an update never overwrites a tool the user has changed." The owner named `MakeraInc/CarveraProfiles` as the source of the reference tools (the repository decision 0013 read for the machine profiles).
3. **Slice 23, lane TL** (work breakdown) builds it. It starts with a decision record on the source and licence of the Makera data, the list of standard tools, whether cutting data ships, delivery and the update policy. TL-12 appears in the traceability table at slice 23 and has a verification plan row.

## Consequences

- **Documents:** `docs/requirements.md` (GC-16, TL-12), the verification plan (GC-16 and TL-12 rows), the work breakdown (slice 23, lane TL, the traceability table), the decision index and the open-items register.
- **P1 count:** the first-release P1 requirements gain one, TL-12.
- **Open items:** OI-268 (the list of standard tools, for the owner to approve), OI-269 (the licence of the Makera data, which decides whether it can be copied), OI-270 (whether cutting data ships, and its source), OI-271 (the update policy for installed defaults in detail).
