# 0091: The release workflow and the release gate (lane TR)

- **Status:** proposed (17 Oct 2026). Lane TR, built on `claude/lane-tr-release-workflow-and-gate`. Nothing has been tagged, published or dispatched: the workflow has not run (OI-304).
- **Date:** 17 Oct 2026.
- **Work breakdown:** slice 20 items 8 (the workflow still to build after [decision 0031](0031-slice-20-packaging-and-trial-documents.md)) and 9 (the release gate in `xtask`), and the review checklist box "A deliberately failing check blocks the release tag".
- **Open items** are tracked in [open-items.md](../open-items.md): OI-300 to OI-305 are new. OI-29 has progressed. OI-103, OI-104 and OI-135 are unchanged.

Numbering: the orchestrator assigned record numbers 0091 to 0095 and OI-300 to OI-309 to this lane. This record takes 0091 and OI-300 to OI-305. It takes no project schema number.

## Context

Decision 0031 (accepted) settled the packaging route: a plain build matrix extended from the nightly build, `lib-v<semver>` and `app-v<semver>` tags, archives attached to a GitHub release (not a pre-release) once the release gate passes. It left the workflow and the gate to build. Verification plan section 7 (first-release exit criteria) says: "the xtask runs the dependency-rule check, cargo-deny, the golden G-code comparison and the LinuxCNC replay before a tag is cut, and a release with a failing gate is not published". Architecture section 12 ("Application binaries", "Names and tags") describes the route. Section 8 of the verification plan names `xtask` as the home of the release gate.

Requirements touched: NFR-01 and NFR-09 (the dependency rules), NFR-10 and NFR-14 (cargo deny: licences, advisories, sources), NFR-08, GC-09 and GC-10 (golden files and gate 3), NFR-02 (three platforms), NFR-12 (the G-code notice in each archive and in the release notes).

## Decision (proposed)

1. **`cargo xtask release-gate`** (`xtask/src/release_gate.rs`) runs four checks in order, every one even after one fails, and fails if any finds a problem or cannot run (a missing tool, a failed build):
   - `deps`: the dependency rules (`cargo xtask deps`).
   - `deny`: `cargo deny --all-features check` with `deny.toml`, as pull request CI runs it.
   - `golden`: every test whose name contains `golden`, across the workspace with `openchuck-app/kernel-tests`. That covers the byte comparisons of generated G-code and toolpaths with the committed files, the golden audit against the exact parts, the output grouping files, and the STEP `drilling` reference job. A run that matches no test fails, so a renamed test cannot make the gate pass by matching nothing. Test names already carry `golden` wherever they compare with a golden file. A new golden test that leaves the word out is not in the gate. The gate does not run the full suite: pull request CI does that on every merge to `main`.
   - `replay`: gate 3, every LinuxCNC golden program in the LinuxCNC simulator (`cargo xtask replay` with no programs and no share), about 22 minutes in one job.
   Its options are `--docker IMAGE` (where LinuxCNC comes from, as for `replay`), `--keep-reports` and `--tag`. With `--tag`, a first check refuses a tag that is not `lib-v<version>` or `app-v<version>`, or whose version is not the workspace version (lockstep, decision 0031 item 2). **No option skips or softens a check.** The tests inject failing steps only inside `#[cfg(test)]`.
2. **`.github/workflows/release.yml`** runs on a push of a `lib-v*` or `app-v*` tag, and by hand (`workflow_dispatch`) with `dry_run`, a boolean that defaults to true. Its jobs:
   - `plan` reads the tag and the workspace version, and decides what the run builds and whether it publishes. A tag push publishes. A manual run publishes only with `dry_run` unticked **and** when started on a release tag. Started on a branch with `dry_run` unticked, it fails before the gate. A manual dry run on a branch builds both the application archives and the library dry run.
   - `gate` runs `cargo xtask release-gate --docker <LinuxCNC image> [--tag <tag>]`, with the OpenCASCADE cache action (decision 0039), cargo-deny from `taiki-e/install-action` and the LinuxCNC image of CI's gate 3. Every later job `needs:` it, so a failing gate builds and publishes nothing.
   - `archives` (application) calls the shared build (item 3). `check-archives` checks that all three archives are there and hold the README and the desktop executable, on a dry run as before a release.
   - `publish-app` (only for a publishing `app-v` run) creates the GitHub release for the existing tag (`--verify-tag`) as a draft, checks it holds three archives, then publishes it, not as a pre-release. The notes carry `VERIFY_GCODE_NOTICE` (NFR-12). If a release for the tag already exists, the job changes nothing and fails.
   - `library` (for `lib-v` runs and branch dry runs) runs `cargo publish --dry-run --locked` on the four family crates together. It does not publish to crates.io (item 4).
3. **One build for both workflows.** The notices, build, assemble and pack jobs of `nightly-build.yml` move unchanged into a reusable workflow, `.github/workflows/build-archives.yml` (`workflow_call`), with inputs for the archive name and the kind (`nightly`, `dry-run`, `release`). The kind changes only the first lines of the README: a nightly README is word for word as before, a dry run says it was not published, and a release names its version and says it passed the release gate. The nightly workflow keeps its change check and its `nightly` pre-release job, and calls the shared build in between. The archive names are `openchuckcam-nightly-<date>-<commit>-<target>` (unchanged), `openchuckcam-dry-run-<commit>-<target>` and `openchuckcam-<version>-<target>`.
4. **Publishing the library family to crates.io is not automated here.** The `openchuck-*` names are not reserved, the project has no registry token, and the crates do not yet pass the dry run (OI-302). A `lib-v` tag therefore runs the gate and the dry run only, and its summary says the crates are published by hand with `cargo-release` (decision 0031 item 2). A publish job with a `CARGO_REGISTRY_TOKEN` secret is the owner's later choice (OI-303).
5. **Out of scope, recorded:** the macOS `.app` bundle command (architecture section 12; OI-300), building the Linux release archive on the oldest runner image with its glibc floor in the README (architecture section 12; OI-301, with OI-103), a Windows installer and signing (not proposed for the first release by decision 0031; OI-104, OI-135), and protecting the release tags (OI-305). Until OI-300 and OI-301 are done, the release archives are the nightly ones, with a release README. That falls short of architecture section 12 on those two points. The architecture is not edited: it describes the route as accepted, and these open items track the gap.

## Verification

- `cargo nextest run -p xtask`: `req_gc_09_req_gc_10_req_nfr_08_a_failing_check_blocks_the_release_gate`. Each of the four checks in turn finds a problem, or fails to run, and the gate fails, names that check alone, and still runs the others. With every check passing, the gate passes. `req_gc_09_req_gc_10_req_nfr_08_the_release_gate_runs_every_check` checks that the gate's steps are exactly `deps`, `deny`, `golden` and `replay`, with `tag` first when a tag is given. `req_nfr_08_the_golden_check_fails_on_a_failed_test_or_on_no_test` covers a failed test, a build failure, and a filter that matches nothing. `release_tag_must_name_the_workspace_version` covers the tag check.
- The real gate was run in the lane's session without LinuxCNC (the session's network policy blocks the LinuxCNC image). The results, including the replay failing to run and so failing the gate, are in the pull request.
- Not run: the workflow itself, a dispatch, a tag, a release or a crates.io publish (OI-304). The YAML was linted with `actionlint`.

## Alternatives considered

- **Copy the nightly build jobs into the release workflow:** simpler to read, but two copies of the archive layout drift apart. The brief asked for reuse.
- **A composite action for the build steps:** it cannot hold the matrix or the notices job, so each caller would repeat both. A reusable workflow holds them.
- **Gate checks as separate workflow jobs** (sharded replay, as in CI): faster, but then the gate is no longer one `xtask` command a maintainer can run before cutting a tag, which is what slice 20 item 9 and the verification plan ask for.
- **A `--skip` or `--only` option on the gate:** convenient when developing, but it is exactly the flag a release could carry. The individual checks remain available as their own commands.
- **Run the whole test suite in the gate:** it would also cover golden tests that do not say `golden`, but it duplicates CI, triples the gate's time and is not one of the four named checks.
- **Publish the crates to crates.io on a `lib-v` tag:** item 4. It needs a token and reserved names, and the crates must package first.

## Consequences

- New: `xtask/src/release_gate.rs`, `.github/workflows/release.yml` and `.github/workflows/build-archives.yml`. Changed: `nightly-build.yml` calls the shared build; `xtask/src/main.rs` adds the command; `xtask/src/replay.rs` names its default tolerance and timeout so the gate and `replay` share them.
- No dependency change. The workflow uses actions already used in CI and the nightly build, plus `cargo-deny` and `cargo-about` through `taiki-e/install-action`, as CI does.
- The work breakdown's slice 20 items 8 and 9 note what is built, and the review checklist box "A deliberately failing check blocks the release tag" is ticked: the unit test above proves it. No tag has been cut. `requirements.md`, `architecture.md` and the verification plan are unchanged.
- Nothing in the workflow, the README or the release notes says that passing the gate makes generated G-code safe to run. Each carries the NFR-12 notice.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | The macOS `.app` bundle for `openchuck-ui`, built by an `xtask` command from a template (architecture section 12), is not built; release archives carry the plain executable (OI-300) | Slice 20 (lane T) | Whether the first release waits for it (OI-135) |
| 2 | The Linux release archive is built on `ubuntu-latest`, not on the oldest runner image, and its README states no glibc floor (architecture section 12; OI-301, with OI-103) | Slice 20 (lane T) | A choice of image or container |
| 3 | The library family fails `cargo publish --dry-run`: `openchuck-post` reads `posts/` outside its crate with `include_str!`, and `openchuck-path` and `openchuck-post` dev-depend on the unpublished `openchuck-testkit` with a version. The release workflow's `library` job fails until this is fixed (OI-302) | Slice 20 (checklist: publish dry run) | A lane that changes those crates |
| 4 | Publishing the library family to crates.io: reserve the `openchuck-*` names, then publish by hand with `cargo-release` or add a workflow job with a registry token (OI-303) | Slice 20 | Project owner |
| 5 | The workflow has not run: a first dry-run dispatch, and its results checked, before the first tag (OI-304) | Before the first release | The orchestrator or owner's go-ahead (runner minutes) |
| 6 | Protect the `lib-v*` and `app-v*` tags with a tag ruleset, so only the maintainer can create them (OI-305) | Before the first release | Project owner (repository settings) |
