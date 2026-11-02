# Contributing to OpenChuckCAM

Thank you for helping. Please read this before opening a pull request.

## Contributor licence agreement

Every contributor signs the [Individual Contributor Licence Agreement](CLA.md) once, before their first pull request is merged, by posting the comment given in that document on the pull request. A CLA check (CLA Assistant Lite, `.github/workflows/cla.yml`) prompts you and records signatures on the `cla-signatures` branch; a pull request cannot be merged until everyone who committed to it has signed. If your employer or another organisation holds rights in your work, it signs the [entity agreement](CLA-ENTITY.md) as well, or confirms in writing that it waives those rights; the maintainer records entity agreements.

The check looks at the author of each commit, not at who opens the pull request. Author your commits with an email linked to your GitHub account (a `users.noreply.github.com` address works), or the check will treat them as unsigned. Commits written with an AI assistant are authored as the person directing it, with the assistant credited through a `Co-Authored-By` line.

The agreement lets the project add a licence exception or dual licence later (for example so FreeCAD can use the pathing library, requirement INT-09) without asking every contributor again. In return, the project commits to keep every contribution available under AGPL-3.0-or-later or another open source licence.

## Licence

OpenChuckCAM is licensed under AGPL-3.0-or-later. Every source file starts with:

```
// SPDX-License-Identifier: AGPL-3.0-or-later
```

`cargo xtask licences` checks this.

## How work is done

- The documents in [docs/](docs/README.md) are the source of truth. [AGENTS.md](AGENTS.md) summarises the rules for people and coding agents alike: trace work to requirement IDs, follow the architecture's crate layout and dependency rules, and record decisions under `docs/decisions/`.
- Open items from decision records are tracked in [docs/open-items.md](docs/open-items.md).
- Name each test after the requirements it covers, for example `req_op_05_roundtrip`.
- Dependency updates come from Dependabot as pull requests. Each one fails `cargo xtask records` until its row in [docs/dependencies.md](docs/dependencies.md) is updated, so every bump is reviewed against NFR-14.
- Before pushing, run:

  ```
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo xtask all
  cargo deny check
  ```

- `main` stays releasable. Branches are short-lived and merge only with CI green on Windows, macOS and Linux.

## Safety

Generated G-code must be verified by the user before it is run on a machine (NFR-12). Do not add wording or behaviour that implies otherwise.
