# OpenChuckCAM

@AGENTS.md

## Working in this repository with Claude Code

- The documents in `docs/` are the source of truth (see the table in `AGENTS.md`). Read the relevant ones before planning or editing. They are long, so read the section you need, using the requirement IDs and section numbers they cross-reference, rather than relying on memory.
- When a task or an answer depends on a requirement, architecture rule or verification method, cite the ID or section (for example "OP-05", "architecture section 4").
- List `docs/decisions/` and read the records relevant to the task before planning. Note each record's status (accepted wins over the document it amends; proposed is guidance only) and its open items, and mention any open item your task depends on instead of resolving it silently.
- Check `docs/open-items.md` before planning, and keep it current: add each open item you raise, and close the ones your change settles.
- Use the work breakdown to find the next task and its exit criteria. If a task there conflicts with the architecture or the requirements, flag the conflict instead of picking a side.
- Before finishing, check that your change matches the documents and that any document it affects is updated in the same change.
- Before committing, check `git config user.name` and `user.email` against the commit-author rule in `AGENTS.md`; a default agent identity makes the CLA check fail.
- Do not edit `docs/requirements.md` or `docs/architecture.md` to make a change fit. Propose the document change and wait for agreement, then record it (a decision record under `docs/decisions/` where one is needed).
