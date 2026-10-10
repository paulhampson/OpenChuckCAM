# OpenChuckCAM

@AGENTS.md

## Working in this repository with Claude Code

- The documents in `docs/` are the source of truth (see the table in `AGENTS.md`). Read the relevant ones before planning or editing. They are long, so read the section you need, using the requirement IDs and section numbers they cross-reference, rather than relying on memory.
- When a task or an answer depends on a requirement, architecture rule or verification method, cite the ID or section (for example "OP-05", "architecture section 4").
- Check `docs/decisions/` before planning. Read the records that touch the task. An accepted record that supersedes a document takes precedence over it; a proposed record has open items, so flag any dependence on it instead of treating its provisional choice as settled. Cite records by number (for example "decision 0001").
- Use the work breakdown to find the next task and its exit criteria. If a task there conflicts with the architecture or the requirements, flag the conflict instead of picking a side.
- Before finishing, check that your change matches the documents and that any document it affects is updated in the same change. If your change closes an open item in a decision record, update the record's status and outstanding items too.
- Do not edit `docs/requirements.md` or `docs/architecture.md` to make a change fit. Propose the document change and wait for agreement, then record it (a decision record under `docs/decisions/` where one is needed).
