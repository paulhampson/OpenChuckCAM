# OpenChuckCAM: agent instructions

OpenChuckCAM is a Rust CAM application that turns a 3D model into G-code for true 4-axis (XYZA) CNC machining. The first reference machine is the Makera Z1, with a LinuxCNC post used for verification.

## Source of truth

The documents in `docs/` define the project. Read the relevant ones before you design, implement or review anything, and do not work from memory or from assumptions about what a CAM tool "usually" does.

| Question | Document |
| --- | --- |
| What must the software do? Which requirement ID covers this? | [docs/requirements.md](docs/requirements.md) |
| Where does this code go? What may depend on what? | [docs/architecture.md](docs/architecture.md) |
| What do I build next, and when is a task done? | [docs/work-breakdown.md](docs/work-breakdown.md) |
| How is this requirement verified? What tests are needed? | [docs/verification-plan.md](docs/verification-plan.md) |
| Which crate should I use for this? | [docs/crate-selection.md](docs/crate-selection.md), and the crate selection section of the architecture |
| Was this decided after the documents were written? Is a spike result or an open decision recorded? | [docs/decisions/](docs/decisions/) (read the records before relying on a document) |

Start at [docs/README.md](docs/README.md) for the precedence order. In short: requirements say *what*, the architecture says *how*, the verification plan says what counts as done, and the work breakdown follows the others.

## Rules

- **Trace work to requirement IDs** (for example `OP-05`, `GC-07`, `NFR-06`). If a change has no requirement behind it, say so and ask before building it. Do not implement P3 items unless the requirements document marks them Include.
- **Follow the architecture.** Put code in the crate the workspace layout names for it. Respect the dependency rules: the library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`) must not depend on the application crates, the UI, GPU, Git or the C++ kernel. Only `openchuck-ui` and `openchuck-viewport` may use egui or wgpu, only `openchuck-import` may use the STEP kernel, and only `openchuck-vcs` may use `git2`. Nothing may depend on a network client or telemetry library.
- **Choose dependencies from the crate selection**, and declare each version once in `[workspace.dependencies]`. A dependency that is not in the selection needs a recorded justification (NFR-14) and a licence compatible with AGPL (NFR-10). A deliberate rule exception is recorded and added to the `xtask` allow-list.
- **Write the tests the verification plan names.** Name each test after the requirement IDs it covers (for example `req_op_05_roundtrip`). A P1 requirement with no test fails CI. Output must be deterministic (GC-07, NFR-06): the same project and post always give byte-identical G-code, whatever the thread count.
- **Never silently diverge from a document.** If the documents conflict with each other, or with what the code needs, stop and raise it. When a change is agreed, update the document in the same pull request. Add a decision record under `docs/decisions/` for choices made after the documents were written, and for the result of each spike.
- **Check the decision records before you start.** Read the index in `docs/decisions/README.md` and every record that touches your task. An accepted record that supersedes part of a document wins until the document is edited to match. A record that is *proposed* has open items: do not build on its provisional choice as if it were settled, and say so if your task depends on it. When you close an open item, update that record's status in the same change.
- **Keep requirement IDs stable.** Never reuse or renumber one.
- Geometry and toolpaths use double precision and millimetres internally (NFR-05). Invalid input gives a clear error, never a crash (NFR-07).
- Generated G-code is the user's to verify before running on a machine (NFR-12). Do not add wording or behaviour that implies otherwise.

## Repository layout

Defined in architecture section 12 (a single monorepo with one Cargo workspace): `crates/` (libraries), `apps/` (`openchuck-ui`, `openchuck-cli`, wiring only), `bindings/` (later), `posts/` (built-in post definitions), `testdata/`, `fuzz/`, `benches/`, `docs/` and `xtask/`. Not all of these exist yet; create them as the work breakdown reaches them, and not before.

## Licence

Every crate is AGPL, with SPDX identifiers in files. The STEP kernel stays isolated in `openchuck-import`.
