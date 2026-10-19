# 0151: Build traceability: a requirement, and a small crate that holds the build information (lane BI)

- **Status:** proposed. The requirement wording, its ID and priority, and the placement of the build information await the project owner's agreement (OI-443, OI-444). Lane BI, GitHub issue #139 (owner's nightly-build trial, F-01 in `docs/trial-feedback.md`). Nothing here edits `docs/requirements.md` or `docs/architecture.md`.
- **Date:** 19 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-443 to OI-447 are new.

Numbering: the coordinator assigned this lane records 0151 and 0152 and OI-443 to OI-447, checked against `main` and the unmerged branches. This record takes 0151 and OI-443, OI-444, OI-446 and OI-447; [decision 0152](0152-about-window.md) takes OI-445. No project schema number.

## Context

The owner's trial of a nightly build found that neither the About window nor `openchuck-cli --version` says which build is running. A trial report that names no commit cannot be matched to a source tree. No requirement covers it. Decision 0110 (GC-12 amended) already puts the software version, a project fingerprint and a version-control line on the setup sheet; this record is the same idea for the program itself. The notices tooling (`cargo about`, decisions 0023, 0031, 0091) and the release builds start from a clean checkout of the tagged commit, so the commit and the dirty flag are exact there.

Requirements touched: NFR-06 and GC-07 (output is deterministic: the build date and commit must not reach G-code, goldens or the library crates), NFR-09 (offline: no network client or telemetry), NFR-10 (licence notices), NFR-12, NFR-14 (dependencies), UI-14 (keyboard and tooltips on focus).

## Decision (proposed)

1. **A new requirement, NFR-16, P1** (OI-443), to be added to `docs/requirements.md` by whoever the owner names:

   > **NFR-16.** The About window and the command-line `--version` report which build is running: the version-control commit, whether the build tree had uncommitted changes, the build date and time (UTC), the Rust compiler version, the target triple and the version of each crate in the build. A fact that cannot be determined (for example, no Git directory in a source archive) reads "unknown"; building needs no network. The About window also shows the licence text in a scrollable box and a Copy action that puts the build information, and only that, on the clipboard. The build information is not written into G-code, golden files or project files, and the library family does not contain it (GC-07, NFR-06).

   Verification-plan row, proposed: *NFR-16, P1, Test.* (a) unit tests of the text from a fixed fixture, the dirty-flag logic, the full-hash check and the fallback without Git; (b) a CLI test that `--version` contains every field; (c) UI tests that the About window renders the build text and the licence box, that Copy puts exactly the build text on the clipboard, and that the box, Copy and Close are reached with Tab; *Inspection:* a nightly archive reports its commit and a clean tree. The tests are named `req_nfr_16_*` and are renamed if the owner chooses another ID.
2. **Where it lives: a new crate `crates/openchuck-buildinfo`** (OI-444), with no dependencies. Only `openchuck-cli` and `openchuck-ui` depend on it. Architecture section 3 needs one table row for it and section 12 one word in the `crates/` list, proposed here for the owner to agree; the `xtask deps` table in `xtask/src/deps.rs` already has the entry (it may depend on nothing; the CLI and the UI may depend on it) and a test (`req_nfr_16_only_the_front_ends_use_the_build_information`) that the library family and `openchuck-app` cannot. A new crate has no third-party dependency, so NFR-14 has nothing to review; it has the workspace licence and SPDX headers. No allow-list exception is needed (the allow-list is for rule exceptions; none is made).
3. **Where it is generated: a build script, not environment set by the workflow.** `crates/openchuck-buildinfo/build.rs` runs `git rev-parse HEAD` and `git status --porcelain` in the workspace root, `$RUSTC --version`, reads `TARGET`, and reads the workspace packages (no `source` line, named `openchuck-*`, not `openchuck-testkit` or `openchuck-benches`) with their versions from `Cargo.lock`. It uses `std::process::Command` only. Where `git` is missing or the folder is not a checkout, the fact is empty and shows as "unknown"; the build never fails for this. The date is `SOURCE_DATE_EPOCH` when set (a reproducible build), else the clock, in UTC. The nightly and release workflows (decisions 0023, 0091) build from clean checkouts and need no change; `THIRD-PARTY-NOTICES.html` is downloaded after the build, so it cannot make the tree look dirty.
4. **Dirty means `git status --porcelain` lists anything**, untracked files that are not ignored included, since a new source file changes the build.
5. **The text** is the same for the About window's Copy button and `--version`:

   ```
   OpenChuckCAM 0.10.0
   Commit: 50fd04ed041e1896002c4870904394b51108ba09
   Uncommitted changes: no
   Built: 2026-10-19T21:07:07Z
   Rust: rustc 1.97.0 (2d8144b78 2026-07-07)
   Target: x86_64-unknown-linux-gnu
   Crates:
     openchuck-app 0.10.0
     ...
   ```
   `--version` prints it after clap's program name; `-V` still prints the one-line version.
6. **Determinism.** Nothing in the library family, `openchuck-app`, G-code, goldens or project files reads this crate (the `deps` check enforces it), so golden files, semver and NFR-06 are unaffected. Setup-sheet traceability (decision 0110) stays as accepted; it could read this source later, which is not done here.

## Alternatives considered

- **A module in `openchuck-app` with the build script there.** Both front ends use `openchuck-app`, so no new crate. But every commit or edit changes the script's output, which would rebuild `openchuck-app` and everything that links it, test crates included. Rejected.
- **A module and a build script in each front end.** No new crate, two copies that drift. Rejected.
- **Environment variables set by the workflows** (`OPENCHUCK_COMMIT=${{ github.sha }}`). Local builds would show nothing, the workflows would change, and a dirty tree could not be told. Not rejected for later: an override for archives built without Git is OI-447.
- **Crates from the ecosystem** (`vergen`, `built`, `shadow-rs`). They bring `git2` or `gix`, which the rules allow only in `openchuck-vcs`, and more than this needs. Rejected.
- **The crate versions through a constant in each crate.** Touches the library family and bumps it. Rejected; `Cargo.lock` has them all.
- **Make it P2.** Then no test is mandatory. Offered to the owner as an option; not recommended, since a trial report without a commit is the problem.

## Consequences

- The workspace gains a member, `openchuck-buildinfo`, and the two front ends a dependency on it. `Cargo.lock` gains one package.
- The build script reruns when `Cargo.toml`, `Cargo.lock`, `crates/`, `apps/`, `benches/`, `posts/`, `xtask/`, `HEAD`, the current branch's ref or the index change, so the commit and the dirty flag follow the tree. An edit that does not change the program still rebuilds the two front ends (OI-446).
- Output is the same for the same sources except in the two front ends' own text.

## Open items

- **OI-443:** the requirement's ID, priority and wording (owner).
- **OI-444:** the crate, with the architecture section 3 and 12 rows (owner).
- **OI-446:** the dirty flag and date are computed when the script runs; an edit outside the watched folders (for example `testdata/`) does not rerun it, and edits inside them rebuild the two front ends. Accepted as the cost of an exact flag.
- **OI-447:** archives built from a source tarball show "unknown" for the commit and the dirty flag. An environment override (`OPENCHUCK_BUILD_COMMIT`) would fix it; not built until a packager needs it.
