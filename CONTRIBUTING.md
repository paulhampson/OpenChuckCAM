# Contributing to OpenChuckCAM

Thank you for helping. Please read this before opening a pull request.

## Contributor licence agreement

Every contributor signs the [Individual Contributor Licence Agreement](CLA.md) once, before their first pull request is merged, by posting the comment given in that document on the pull request. If your employer or another organisation holds rights in your work, it signs the [entity agreement](CLA-ENTITY.md) as well, or confirms in writing that it waives those rights.

The agreement lets the project add a licence exception or dual licence later (for example so FreeCAD can use the pathing library, requirement INT-09) without asking every contributor again. In return, the project commits to keep every contribution available under AGPL-3.0-or-later or another open source licence. The maintainer keeps a record of who has signed.

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
