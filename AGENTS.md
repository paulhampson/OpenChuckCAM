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
| How should a screen look and behave? | [docs/ui-design/ui-specification.md](docs/ui-design/ui-specification.md) (decision 0038) |
| Which crate should I use for this? | [docs/crate-selection.md](docs/crate-selection.md), and the crate selection section of the architecture |
| Was this decided after the documents were written? | [docs/decisions/](docs/decisions/) |

Start at [docs/README.md](docs/README.md) for the precedence order. In short: requirements say *what*, the architecture says *how*, the verification plan says what counts as done, and the work breakdown follows the others.

## Rules

- **Trace work to requirement IDs** (for example `OP-05`, `GC-07`, `NFR-06`). If a change has no requirement behind it, say so and ask before building it. Do not implement P3 items unless the requirements document marks them Include.
- **Check the decision records.** Before planning or editing, read the records in `docs/decisions/` that touch your area. An accepted record wins over the document it amends until that document is edited (see `docs/README.md`). A *proposed* record is guidance, not authority: say when you rely on one. Check a record's open items before building something it defers, and add a new record for any choice made after the documents were written. Open items are tracked in `docs/open-items.md`: add a row for each new one and update its state when it closes, in the same pull request.
- **Keep spike code and spike docs on separate branches.** A spike (work breakdown, Spikes) is throwaway: its code goes on `spike/sp-<n>-<short-title>`, in `spikes/<name>/` as a separate Cargo project with an empty `[workspace]` table (never a workspace member), with its write-up in `RESULT.md` beside it. That branch is never merged. The decision record and the register changes it brings (`docs/decisions/`, `docs/open-items.md`, `docs/dependencies.md`, and any agreed document edits) go on their own branch from `main`, named `docs/decision-<NNNN>-<short-title>`; that branch is the one opened as a pull request. The record names the spike branch and where the write-up is on it. Before taking a record number or `OI-` number, check the unmerged branches as well as `main`, and take the next free one.
- **Keep the spike status current.** The Status column of the spike table in `docs/work-breakdown.md` must match the spike branches, `RESULT.md` files, decision records and open items. Any change that starts a spike, pushes a spike branch, writes or updates a `RESULT.md`, adds or changes a decision record for a spike, or closes an open item a spike waits on, updates that spike's Status cell in the same pull request. Cite the record by number and link it, and name any open items still waiting (`OI-` numbers). Do this before the change is committed, not as a later clean-up.
- **Follow the architecture.** Put code in the crate the workspace layout names for it. Respect the dependency rules: the library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`) must not depend on the application crates, the UI, GPU, Git or the C++ kernel. Only `openchuck-ui` and `openchuck-viewport` may use egui or wgpu, only `openchuck-import` may use the STEP kernel, and only `openchuck-vcs` may use `git2`. Nothing may depend on a network client or telemetry library.
- **Choose dependencies from the crate selection**, and declare each version once in `[workspace.dependencies]`. A dependency that is not in the selection needs a recorded justification (NFR-14) and a licence compatible with AGPL (NFR-10). A deliberate rule exception is recorded and added to the `xtask` allow-list.
- **Write the tests the verification plan names.** Name each test after the requirement IDs it covers (for example `req_op_05_roundtrip`). A P1 requirement with no test fails CI. Output must be deterministic (GC-07, NFR-06): the same project and post always give byte-identical G-code, whatever the thread count.
- **Work in parallel by the plan.** The work breakdown's "Parallel work" section lists the lanes that may run in separate sessions and what each needs merged first. Start only a lane whose needs are on `main`, and name the lane in the branch and pull request. Take decision, `OI-` and project schema numbers from the next free one across `main` and every unmerged branch, and claim a schema number in the work breakdown's table before using it. Add a type or function beside a public library struct rather than a field to it: `cargo-semver-checks` fails the pull request otherwise.
- **Run tests uninstrumented, and flag long-running ones.** Run and time every test without coverage instrumentation (`cargo nextest run`, not `cargo llvm-cov`): instrumentation slows hot loops 6 to 11 times, so coverage runs are not a measure of a test's cost. A test that takes more than about a minute uninstrumented is long-running: list it in both profiles of `.config/nextest.toml` (excluded from `coverage`, selected by `long`, which the nightly run executes) and give the time in the comment there. The pull request Tests job still runs it. If a test takes longer than 10 minutes uninstrumented, stop and flag it for a decision during development (shrink the case, split it, or accept it in the nightly run only); do not add or keep it silently.
- **Never silently diverge from a document.** If the documents conflict with each other, or with what the code needs, stop and raise it. When a change is agreed, update the document in the same pull request. Add a decision record under `docs/decisions/` for choices made after the documents were written.
- **Keep requirement IDs stable.** Never reuse or renumber one.
- **Author commits as the human you work for, not as the agent.** Set the git author and committer to the person whose account runs the session, using an email linked to their GitHub account (the `<id>+<login>@users.noreply.github.com` address works, and the GitHub integration's `get_me` gives the id and login), and keep the `Co-Authored-By` line for the agent. The CLA check (`.github/workflows/cla.yml`) looks at commit authors, not at who opens the pull request, and an author with no linked GitHub account, such as the default agent identity, is treated as an unsigned contributor. The person signs off on the work as their own; the agent is credited as co-author. If the session has no human account to author as, say so and ask before committing.
- Geometry and toolpaths use double precision and millimetres internally (NFR-05). Invalid input gives a clear error, never a crash (NFR-07).
- Generated G-code is the user's to verify before running on a machine (NFR-12). Do not add wording or behaviour that implies otherwise.
- **Documentation is not reviewed by CodeRabbit.** `.coderabbit.yaml` excludes `docs/` and Markdown files from review, so docs-only pull requests (the `docs/decision-...` branches) are skipped. Keep those exclusions in place, and do not request a CodeRabbit review on a docs-only pull request.

## Repository layout

Defined in architecture section 12 (a single monorepo with one Cargo workspace): `crates/` (libraries), `apps/` (`openchuck-ui`, `openchuck-cli`, wiring only), `bindings/` (later), `posts/` (built-in post definitions), `testdata/`, `fuzz/`, `benches/`, `docs/` and `xtask/`. Not all of these exist yet; create them as the work breakdown reaches them, and not before.

## Licence

Every crate is AGPL, with SPDX identifiers in files. The STEP kernel stays isolated in `openchuck-import`.
