# 0151: Build traceability: a requirement, and a small crate that holds the build information (lane BI)

- **Status:** proposed. Owner decisions relayed by the coordinator on 19 Oct 2026: the requirement is NFR-16, P1, with G-code naming the tool and its version (OI-443, agreed with the amended wording, which the owner's go-ahead let this pull request add to `docs/requirements.md` and the verification plan); the placement is option A, the new crate (OI-444, decided); the G-code version line is option A of OI-446 (decided). The record itself is not accepted: the owner accepts it directly in the lane BI session. Lane BI, GitHub issue #139 (owner's nightly-build trial, F-01 in `docs/trial-feedback.md`). Nothing here edits `docs/requirements.md` or `docs/architecture.md`.
- **Date:** 19 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-443 to OI-447 are new.

Numbering: the coordinator assigned this lane records 0151 and 0152 and OI-443 to OI-447, checked against `main` and the unmerged branches. This record takes 0151 and OI-443, OI-444, OI-446 and OI-447; [decision 0152](0152-about-window.md) takes OI-445. No project schema number.

## Context

The owner's trial of a nightly build found that neither the About window nor `openchuck-cli --version` says which build is running. A trial report that names no commit cannot be matched to a source tree. No requirement covers it. Decision 0110 (GC-12 amended) already puts the software version, a project fingerprint and a version-control line on the setup sheet; this record is the same idea for the program itself. The notices tooling (`cargo about`, decisions 0023, 0031, 0091) and the release builds start from a clean checkout of the tagged commit, so the commit and the dirty flag are exact there.

Requirements touched: NFR-06 and GC-07 (output is deterministic: the build date and commit must not reach G-code, goldens or the library crates), NFR-09 (offline: no network client or telemetry), NFR-10 (licence notices), NFR-12, NFR-14 (dependencies), UI-14 (keyboard and tooltips on focus).

## Decision (proposed)

1. **A new requirement, NFR-16, P1** (OI-443), to be added to `docs/requirements.md` by whoever the owner names:

   > **NFR-16.** The About window and the command-line `--version` report which build is running: the version-control commit, whether the build tree had uncommitted changes, the build date and time (UTC), the Rust compiler version, the target triple and the version of each crate in the build. A fact that cannot be determined (for example, no Git directory in a source archive) reads "unknown"; building needs no network. The About window also shows the licence text in a scrollable box and a Copy action that puts the build information, and only that, on the clipboard. The generated G-code names the tool and its version in a comment (for example `(OpenChuckCAM 0.10.0)`), the workspace version only. No other build information (commit, date, uncommitted-changes flag) is written into G-code, golden files or project files, and the library family does not contain it (GC-07, NFR-06).

   The owner added the G-code sentence on 19 Oct 2026 (relayed by the coordinator); item 7 is how it is written. The row is in `docs/requirements.md` and `docs/verification-plan.md` in this pull request, and NFR-16 is listed under slice 22 (the interface conformance pass, where the About window belongs) in the work breakdown's traceability table, because the trace check wants every P1 requirement under a slice. The owner then agreed (relayed 19 Oct 2026) the GC-07 sentence ("the software version is an input: the generator comment is the only text that changes with it") and the architecture rows, and both are edited in this pull request: GC-07 in `docs/requirements.md`, and the `openchuck-buildinfo` row in architecture section 3 and `-buildinfo` in the section 12 `crates/` list. AGENTS.md's "same project and post" sentence and the `openchuck-cli` and `openchuck-ui` dependency columns are left for the SP lane (OI-447).

   Verification-plan row, proposed: *NFR-16, P1, Test.* (a) unit tests of the text from a fixed fixture, the dirty-flag logic, the full-hash check and the fallback without Git; (b) a CLI test that `--version` contains every field; (c) UI tests that the About window renders the build text and the licence box, that Copy puts exactly the build text on the clipboard, and that the box, Copy and Close are reached with Tab; *Inspection:* a nightly archive reports its commit and a clean tree. The tests are named `req_nfr_16_*` and are renamed if the owner chooses another ID.
2. **Where it lives: a new crate `crates/openchuck-buildinfo`** (OI-444, decided by the owner, relayed 19 Oct 2026), with no dependencies. Only `openchuck-cli` and `openchuck-ui` depend on it. Architecture section 3 has one table row for it and section 12 one word in the `crates/` list (agreed by the owner, relayed 19 Oct 2026); the `xtask deps` table in `xtask/src/deps.rs` already has the entry (it may depend on nothing; the CLI and the UI may depend on it) and a test (`req_nfr_16_only_the_front_ends_use_the_build_information`) that the library family and `openchuck-app` cannot. A new crate has no third-party dependency, so NFR-14 has nothing to review; it has the workspace licence and SPDX headers. No allow-list exception is needed (the allow-list is for rule exceptions; none is made).
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

7. **The tool and version in the G-code (OI-446, owner decision A, relayed 19 Oct 2026).** Both shipped posts write `(OpenChuckCAM <workspace version>)` on the line after the title, before the revision comment, inside the existing `comments` option (so it is on in both posts and off where comments are off). `PostJob` in `openchuck-post` gains `generator: Option<String>` (the host's text, made comment-safe like the title), and `openchuck-app` fills it from its own `CARGO_PKG_VERSION`, which is the workspace version; it never reads `openchuck-buildinfo`, so no commit, date or dirty flag can reach a program. A host passing `None` writes no line. Golden files hold `(OpenChuckCAM <version>)`; `openchuck_testkit::golden::check` masks the version in the actual text before comparing (and before blessing), and the CLI test helper `stable` does the same, as `(operation <id>)` is already masked, so a version bump changes no golden file. `xtask golden` and the LinuxCNC replay read the stored files, where the line is an ordinary comment. This is a breaking change to the library family API, so the workspace version goes from 0.10.0 to 0.11.0 with a changelog entry in each of the four crates (decision 0048); `cargo-semver-checks` stays green that way. Tests: `req_nfr_16_both_posts_name_the_tool_and_version_after_the_title`, `req_nfr_16_req_nfr_07_the_generator_comment_is_made_safe`, `req_nfr_16_req_gc_07_every_program_names_the_tool_and_the_workspace_version` (the real output carries exactly the workspace version and no build facts) and the masking test in `openchuck-testkit`.

## Alternatives considered

- **A module in `openchuck-app` with the build script there.** Both front ends use `openchuck-app`, so no new crate. But every commit or edit changes the script's output, which would rebuild `openchuck-app` and everything that links it, test crates included. Rejected.
- **A module and a build script in each front end.** No new crate, two copies that drift. Rejected.
- **Environment variables set by the workflows** (`OPENCHUCK_COMMIT=${{ github.sha }}`). Local builds would show nothing, the workflows would change, and a dirty tree could not be told. Not rejected for later: an override for archives built without Git is in OI-447.
- **Crates from the ecosystem** (`vergen`, `built`, `shadow-rs`). They bring `git2` or `gix`, which the rules allow only in `openchuck-vcs`, and more than this needs. Rejected.
- **The crate versions through a constant in each crate.** Touches the library family and bumps it. Rejected; `Cargo.lock` has them all.
- **For the G-code line: store the literal version in the golden files** and regenerate all 38 on each bump (rejected by the owner: churn in every release). **A line independent of the `comments` option** or an option off by default: not chosen. **Prepend it to the title in the host** (no API change): the title is the project's name, and the line would repeat in any post that drops the title.
- **Make it P2.** Then no test is mandatory. Offered to the owner as an option; not recommended, since a trial report without a commit is the problem.

## Consequences

- The workspace gains a member, `openchuck-buildinfo`, and the two front ends a dependency on it. `Cargo.lock` gains one package.
- The build script reruns when `Cargo.toml`, `Cargo.lock`, `crates/`, `apps/`, `benches/`, `posts/`, `xtask/`, `HEAD`, the current branch's ref or the index change, so the commit and the dirty flag follow the tree. An edit that does not change the program still rebuilds the two front ends (OI-447).
- Output is the same for the same sources and the same software version; a version bump changes the one comment line and no golden file. The 32 golden files the tests write and the 4 STEP-job files (`drilling`, `swarf`, which need the OpenCASCADE kernel tests) gained that one line.
- The workspace and the four library crates move to 0.11.0.

## Open items

- **OI-443:** the requirement's ID, priority and wording (owner).
- **OI-444:** decided (the crate), with the architecture rows.
- **OI-446:** decided (item 7).
- **OI-447:** two notes. The dirty flag and date are computed when the build script runs: it reruns when `Cargo.toml`, `Cargo.lock`, `crates/`, `apps/`, `benches/`, `posts/`, `xtask/`, `HEAD`, the branch ref or the index change; an edit elsewhere (for example `testdata/`) does not rerun it, and an edit inside rebuilds the two front ends (accepted as the price of an exact flag). Archives built from a source tarball show "unknown" for the commit and the dirty flag. An environment override (`OPENCHUCK_BUILD_COMMIT`) would fix it; not built until a packager needs it.
