# OpenChuckCAM

@AGENTS.md

## Working in this repository with Claude Code

- The documents in `docs/` are the source of truth (see the table in `AGENTS.md`). Read the relevant ones before planning or editing. They are long, so read the section you need, using the requirement IDs and section numbers they cross-reference, rather than relying on memory.
- When a task or an answer depends on a requirement, architecture rule or verification method, cite the ID or section (for example "OP-05", "architecture section 4").
- List `docs/decisions/` and read the records relevant to the task before planning. Note each record's status (accepted wins over the document it amends; proposed is guidance only) and its open items, and mention any open item your task depends on instead of resolving it silently.
- Check `docs/open-items.md` before planning, and keep it current: add each open item you raise, and close the ones your change settles.
- Use the work breakdown to find the next task and its exit criteria. If a task there conflicts with the architecture or the requirements, flag the conflict instead of picking a side.
- Before finishing, check that your change matches the documents and that any document it affects is updated in the same change.
- For a spike, follow the two-branch rule in `AGENTS.md` (code on `spike/...`, never merged; record and registers on `docs/decision-...`). If the session assigns a branch name that does not fit it, say so and ask before pushing elsewhere.
- Keep the spike status current. Before finishing any change that touches a spike (its branch, `RESULT.md`, a decision record, or an open item it waits on), check the Status column of the spike table in `docs/work-breakdown.md` against the current state and update it in the same change. If you can't verify a spike's state (for example the branch is not fetched), say so in the Status cell rather than guessing.
- Before starting, read the work breakdown's "Parallel work" section: confirm your lane's needs are merged, check unmerged branches for the next free decision, `OI-` and schema numbers, and add your schema claim before changing a project file format. Rebase on `main` before opening the pull request, and say in it which lane it is.
- Before pushing, run `cargo fmt --all`, `cargo clippy --workspace --all-targets`, the tests of the crates you changed and `cargo xtask deps`, `licences` and `trace`; CI also runs `cargo-semver-checks` on the library family, so do not add fields to public structs there.
- Before committing, check `git config user.name` and `user.email` against the commit-author rule in `AGENTS.md`; a default agent identity makes the CLA check fail.
- Time every test uninstrumented as you write it (`cargo nextest run`). Follow the long-running test rule in `AGENTS.md`: list tests over about a minute in `.config/nextest.toml`, and flag any test over 10 minutes for a decision before going on.
- Docs are not reviewed by CodeRabbit: `.coderabbit.yaml` excludes `docs/` and Markdown. Keep those exclusions in place. See the rule in `AGENTS.md`.
- Do not edit `docs/requirements.md` or `docs/architecture.md` to make a change fit. Propose the document change and wait for agreement, then record it (a decision record under `docs/decisions/` where one is needed).
