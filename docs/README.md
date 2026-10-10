# OpenChuckCAM documentation

These documents are the source of truth for the project. They live in the repository, so a change to the design or the scope is reviewed in the same pull request as the code that needs it. The copies that began as Claude artifacts are working drafts only.

| Document | What it settles |
| --- | --- |
| [requirements.md](requirements.md) | What the software must do. Every requirement has an ID (for example `OP-05`, `NFR-01`) and a priority (P1 core, P2 secondary, P3 suggested). |
| [architecture.md](architecture.md) | How it is built: workspace layout, dependency rules, component design, crate selection summary, verification architecture, repository structure and open decisions. |
| [work-breakdown.md](work-breakdown.md) | Phase 1 work breakdown: slices, tasks, exit criteria and the order they are built in. |
| [verification-plan.md](verification-plan.md) | How each requirement is verified, by which method and at which level, and the release gates. |
| [crate-selection.md](crate-selection.md) | The survey of Rust crates, with scores out of 100 as of 10 Oct 2026. The architecture's crate choices come from it. |
| [decisions/](decisions/) | Decision records for choices made after these documents were written. |

## Precedence

1. `requirements.md` decides *what*. The other documents refer to its IDs.
2. `architecture.md` decides *how*, and records the crate choices made from `crate-selection.md`.
3. `verification-plan.md` decides how a requirement counts as met.
4. `work-breakdown.md` decides the order of work and follows the other three.
5. A record in `decisions/` that supersedes part of a document above wins until that document is edited to match. Edit the document in the same change.

## Keeping the documents current

- If code and a document disagree, one of them is wrong. Do not leave the disagreement in place: fix the code, or change the document in the same pull request and say why.
- Keep requirement IDs stable. Never reuse or renumber one; mark it withdrawn instead.
- The traceability report (an `xtask` command) reads requirement IDs from test names, so an ID that changes here changes there.
