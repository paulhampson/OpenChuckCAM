# 0054: The decision index and the open-items register merge as unions

- **Status:** accepted by the project owner (16 Oct 2026, in the orchestration session, option A of the three offered).
- **Date:** 16 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-231 is new.

## Context

Every lane adds a row at the end of the decision index (`docs/decisions/README.md`) and the open-items register (`docs/open-items.md`). When one pull request merges, each other open pull request that also appended a row is in conflict at the same lines, so its session merges `main`, resolves it by keeping both rows and pushes, which restarts a CI run of about 25 to 55 minutes at the current queue. It happened four times on 16 Oct 2026.

## Decision

1. **`.gitattributes` marks both files `merge=union`.** A local `git merge` then keeps the lines added on both sides, so a lane merging `main` no longer stops on a conflict over appended rows.
2. **Lanes put new rows in number order** and, after a merge, check any row they edited. A union merge can leave two rows out of number order, and it keeps both versions of a row that was changed on both sides.
3. **No other file is changed.** The work breakdown's prose and tables, the requirements and the verification plan still conflict normally, because a union of prose is not a merge.

## Limits

- GitHub's own mergeability check and its merge button do not apply `.gitattributes` merge drivers. A pull request can still show as conflicting on GitHub until its session merges `main` locally; that merge now resolves itself, but it is still a push and a CI run.
- `cargo xtask records` checks that each record has an index row, not the order of the rows or a duplicate row (OI-231).

## Alternatives considered

- **Keep merging by hand and rely on merge order (option B):** no change, and the conflicts continue.
- **Generate the index from the records (option C):** removes the conflict entirely, including on GitHub, but needs a new `xtask` command and a bigger change. Not ruled out if the limits above hurt.

## Open items

- **OI-231:** extend `cargo xtask records` to check the index rows are in number order and not duplicated, so a union merge cannot leave a bad row unnoticed.
