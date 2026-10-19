# 0130: Register checks, and how the registers are edited

- **Status:** proposed. Only the project owner accepts it. The number is provisional: it was the next free one on `main` and every remote branch on 19 Oct 2026, and the orchestrator did not assign it.
- **Date:** 19 Oct 2026.
- **Lane:** RH (register hygiene).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-231 progresses (the check is built; it closes when this record is accepted). No new open item.
- **Amends:** [decision 0054](0054-union-merge-for-registers.md) item 2 and its *Limits*.

## Context

Decision 0054 made the decision index and the open-items register merge as unions, so that lanes appending rows stop conflicting. It named the cost: a union merge keeps both versions of a row two branches changed, and nothing warns about it (OI-231). `cargo xtask records` did not check the registers at all; the *Limits* of 0054 say it checks that each record has an index row, and it does not.

By 19 Oct 2026, three days after the union merge was switched on, the register held:

- **Duplicate rows.** OI-334 five times and OI-342 four times (each copy a different stage of the same row), OI-210 twice, in the open table; OI-311 twice in the closed table.
- **Rows in both tables.** OI-80, OI-317, OI-325 and OI-331 were closed in one branch and edited in another, and the edited open copy came back beside the closed row.
- **Closed items in the open table.** Eleven rows whose state said *closed* (OI-144, 158, 159, 161, 209, 210, 247, 251, 257, 381, 382) sat in the open table, so the count of open items was wrong and the next reader had to read each row to know.
- **Open items in the closed table.** OI-131, 132, 149 and 169 were appended at the end of the file, which is the closed table, and were lost to anyone reading the open one.
- **A decision index out of order.** Seven rows were out of number order, and record 0020 had no row.

None of this broke a build. It cost review time, and the 19 Oct triage had to read the whole file to find what was still open.

## Decision

1. **`cargo xtask records` also checks the registers** (`xtask/src/registers.rs`, run with the dependency checks, so CI's existing *Dependency pins and records* step runs it). Each finding names the file and line.
   - Open-items register: every row has the cells of its table (six in the open table, five in the closed one); an ID is used once in a table; no ID is in both tables; no row in the open table has a state beginning *closed*; no six-cell row, which is an open-table row, is in the closed table.
   - Decision index: one row for each record file, each linking to a file that exists; rows in number order; no record twice.
   - The IDs `OI-C1` to `OI-C4` of the first closed items are allowed.
2. **The registers are edited in place** (stated in `docs/open-items.md`, `AGENTS.md` and the work breakdown's *Working in parallel*):
   - Change a row where it is. Do not copy it to a new line and edit the copy; a union merge keeps both.
   - To close an item, move its row from the open table to the closed table in the same commit, in the five closed columns. Never leave a closed state in the open table.
   - Put a new open item at the end of the **open** table, above the *Closed* heading, never at the end of the file.
   - Put an index row in number order, not at the end.
   - After merging `main`, run `cargo xtask records` and fix what it reports before pushing. A merge that edits a row another branch also edited is the case to look at.
3. **The register is cleaned in this change** (the list under *Context*): the newest version of each duplicated row stands, with the progress notes of OI-342's four versions combined in one; closed rows move to the closed table; the four open items return to the open table with their text unchanged; the index is sorted and gets its row for 0020.
4. **Two items are closed on evidence** (the owner can reopen them): OI-221, because `ci.yml` cancels superseded pull request runs and never a `main` run, as decided on 16 Oct 2026; and OI-232, because lane O stores the keep-tangent-edges choice per edge set and for the project (schema 12) and `operation::edge_options` reads it (decision 0066 item 9, decision 0057).

## Alternatives considered

- **Generate the decision index from the records** (option C of 0054): removes the index conflicts completely, but needs a generator, and the open-items register would still need this check. Not ruled out.
- **One file per open item:** no row can be duplicated or misfiled, and a close is a move. A bigger change to every lane and to the triage; not taken now.
- **Leave the union merge off for the register:** back to conflicts on every appended row, which 0054 was made to stop.
- **Check the number order of the open table too:** the open table is grouped by slice, not numbered in order, so it would mean reordering 170 rows for no reader's benefit. Not checked.

## Consequences

- **Lanes in flight.** A lane that adds an index row or a register row and merges `main` after this lands may find the check failing on its merge (for example a row appended at the end of the index). That is the check working: move the row to its place.
- **CI path filter.** The *records* filter in `ci.yml` covers `docs/requirements.md`, `docs/work-breakdown.md`, `docs/dependencies.md` and `testdata/**` only, so a docs-only pull request that edits only the register does not run this check. Lane XC's pull request widens the filter to `docs/**`; merge it with or before this one so the guard covers register-only changes.
- **Decision 0054's *Limits*** line on `xtask records` is out of date; this record is its correction.
- **Not covered:** the check cannot tell which of two versions of a row is the newer, and it does not read the meaning of a row. A closed item still wanted open, or an open one that is done (OI-14, 46, 131, 132, 169, 224 are candidates), is for the owner to decide.
- The `.gitattributes` comment and the register's header point to this record.
