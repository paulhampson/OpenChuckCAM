# OpenChuckCAM documentation

These documents are the source of truth for the project. They live in the repository, so a change to the design or the scope is reviewed in the same pull request as the code that needs it. The copies that began as Claude artifacts are working drafts only.

| Document | What it settles |
| --- | --- |
| [requirements.md](requirements.md) | What the software must do. Every requirement has an ID (for example `OP-05`, `NFR-01`) and a priority (P1 core, P2 secondary, P3 suggested). |
| [architecture.md](architecture.md) | How it is built: workspace layout, dependency rules, component design, crate selection summary, verification architecture, repository structure and open decisions. |
| [work-breakdown.md](work-breakdown.md) | Phase 1 work breakdown: slices, tasks, exit criteria and the order they are built in. |
| [verification-plan.md](verification-plan.md) | How each requirement is verified, by which method and at which level, and the release gates. |
| [crate-selection.md](crate-selection.md) | The survey of Rust crates, with scores out of 100 as of 10 Oct 2026. The architecture's crate choices come from it. |
| [dependencies.md](dependencies.md) | The NFR-14 record of every third-party dependency: pinned version, survey score, licence and justification, and the pin and upgrade policy. |
| [fuzzing.md](fuzzing.md) | Every user-supplied file format with its fuzz target and corpus, and the running total of fuzz CPU hours per parser (NFR-07, decision 0107). |
| [z1-gates-4-5-test-instructions.md](z1-gates-4-5-test-instructions.md) | Step-by-step instructions, checklists and results forms for G-code gates 4 (the Z1 air cut) and 5 (the first cut), with the [air-cut procedure](z1-air-cut-procedure.md) and the [safe-practice checklist](z1-safe-practice-checklist.md) they build on. Owner results and trial-specific waivers recorded; remaining evidence gaps and local verification are itemised in section 10. |
| [templates/](templates/) | Blank templates for records: [z1-gate-results-record.md](templates/z1-gate-results-record.md), the results and review record of a run of Z1 gates 4 and 5. |
| [open-items.md](open-items.md) | The register of open items raised by decision records and slice reviews, with their state. |
| [decisions/](decisions/) | Decision records for choices made after these documents were written. |
| [archive/](archive/) | Raw evidence (logs, run output, scripts) that a decision record cites, in one folder per record (`archive/<NNNN>-<name>/`, each with a README). Not a source of truth: the record's Verification section is. |
| [ui-design/](ui-design/) | The user interface specification: layout, workflow, states, wording and visual tokens of the desktop application. Accepted with decision 0038; it ranks with the decision records (precedence 5). Rendered mockups are in `ui-design/mockups/`. |

The guide for people who use the program is not here: it is [`user-docs/`](../user-docs/README.md), published on GitHub Pages ([decision 0400](decisions/0400-user-documentation-site.md)). It is written from these documents, and where it adapts one (the Z1 air-cut procedure and safe-practice checklist) the document here is the authority.

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
