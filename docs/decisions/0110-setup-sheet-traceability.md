# 0110: The setup sheet carries traceability data (GC-12 amended)

- **Status:** accepted by the project owner on 17 Oct 2026, in the orchestration session (Decisions 39 and 40, option A on both): the scope below and the wording of GC-12.
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none are new.

Numbering: the orchestrator took 0110, clear of 0107 (lane FZ), 0108 (lane BM) and 0109 (lane BT), the highest claims across `main` and the unmerged branches.

## Context

The setup sheet (slice 19, GC-12) lists tools, operations, stock and times. A printed sheet carries nothing that ties it to the files it came from or to the moment it was made. The owner asked on 17 Oct 2026 for the time and date it was generated and when the project was last saved, with version control information to follow.

## Decision

1. **A Traceability block on the setup sheet** (HTML and PDF) with:
   - **Generated:** the date and time in ISO 8601 with the time zone.
   - **Project last saved:** the modification time of the project file on disk. It is not stored in the project, so project files stay diff-clean and byte-stable (PRJ-01). When there is no saved file the sheet says "not saved yet".
   - **Software version:** the workspace version that produced the sheet.
   - **Project fingerprint:** a hash of the saved project file, so a printed sheet can be matched to the file it came from.
   - **Version control:** a reserved line that reads "not tracked" until `openchuck-vcs` exists, then carries the commit and branch.
2. **Deterministic tests.** The generation time comes from an injected clock, read from the system only at export, so the sheet's golden snapshot is made with a fixed clock. The G-code is unchanged and stays byte-identical (GC-07, NFR-06).
3. **GC-12 is amended** (requirement wording agreed by the owner): "...The sheet also records when it was generated and when the project was last saved, the software version and a fingerprint of the project, and a version-control line that is filled in when version control is available, so a printed sheet can be traced to the files it came from." The verification plan row and the work breakdown (slice 19 item 6) follow.
4. **A small lane builds it** after lane S19's pull request merges, since both change the setup sheet.

## Alternatives considered

- **Only the generated and last-saved times.** Simpler, but a printed sheet could not be matched to a file version. Rejected by the owner.
- **Store a "saved at" timestamp in the project file.** It needs a schema change and adds a changing line to every project diff (PRJ-01). Rejected.

## Consequences

- `docs/requirements.md` GC-12, `docs/verification-plan.md` (the GC-05, GC-06, GC-11, GC-12 row), `docs/work-breakdown.md` slice 19 item 6, and the decision index.
- The golden setup sheet snapshots gain the block (with a fixed clock) when the lane builds it.
