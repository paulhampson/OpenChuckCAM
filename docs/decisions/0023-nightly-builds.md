# 0023: Nightly development builds of the application binaries

- **Status:** proposed (14 Oct 2026). The project owner asked for the workflow; this record waits on their acceptance of the choices below.
- **Date:** 14 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-103 and OI-104 are new and deferred to slice 20.

Numbering: records up to 0022 and open items up to OI-102 are on `main` or claimed on unmerged branches, so this record takes 0023 and OI-103 to OI-104.

## Context

The owner asked for a nightly build that produces a runnable archive for each platform when the repository has changed since the last nightly build. Nothing in the documents asks for this before slice 20: architecture section 12 ("Application binaries") plans a tag-triggered build matrix and leaves the packaging route open until after the first manual releases, and work breakdown slice 20 items 8 and 10 decide release tooling and platform packaging. A nightly development build is not a release and decides neither.

Requirements touched: NFR-02 (Linux, Windows and macOS), NFR-10 (licence notices shipped with binaries), NFR-12 (the user verifies generated G-code; nothing may imply otherwise). Verification plan section 8 (CI and platforms).

## Decision (proposed)

1. **A separate workflow, `.github/workflows/nightly-build.yml`,** scheduled daily at 01:43 UTC and runnable by hand. It is kept apart from `nightly.yml` (fuzzing and the beta build) so that "the last successful nightly build" means the last run whose binaries built.
2. **Build only when `main` has moved.** A first job reads the newest successful run of this workflow on the same branch through the Actions API (`actions: read`) and skips the build when its commit is the current head. A failed build is therefore retried the next night. A manual run always builds, from the branch it is started on.
3. **Targets:** `x86_64-unknown-linux-gnu` (`ubuntu-latest`), `x86_64-pc-windows-msvc` (`windows-latest`) and `aarch64-apple-darwin` (`macos-latest`), the same runners as pull request CI. `cargo build --release --locked` of `openchuck-cli` and `openchuck-ui`.
4. **One archive per platform:** `openchuckcam-nightly-<date>-<commit>-<target>`, a `.tar.gz` on Linux and macOS (keeps the executable bit) and a `.zip` on Windows, uploaded unwrapped (`archive: false`) as a workflow artifact kept for 30 days.
5. **Contents:** the two executables, `LICENSE.txt` (the AGPL text), `THIRD-PARTY-NOTICES.html` from `cargo about` (NFR-10), and a `README.txt` naming the commit, toolchain and runner image, saying the build is a development build that passed no release gate, and carrying `VERIFY_GCODE_NOTICE` read from `openchuck-types` (NFR-12). No data folders are needed: the built-in posts (`openchuck-post`) and machine profile (`openchuck-project`) are compiled into the binaries, and tool libraries are folders the user names.
6. **A rolling `nightly` pre-release** (owner's request, 14 Oct 2026). When every platform of a build of `main` succeeds, a last job (`contents: write`) deletes the `nightly` release and its tag and creates them again at the built commit, with the three archives attached, marked as a pre-release and never as latest. The notes say it is a development build, not a release, and carry `VERIFY_GCODE_NOTICE` (NFR-12). So the release always holds one complete set from one commit, can be downloaded without signing in, and is replaced by the next build. Manual runs on other branches produce artifacts only.

## Alternatives considered

- **Artifacts only, no pre-release** (the first version of this record): needs no write permission, but artifacts can only be downloaded by a signed-in GitHub user and expire. The owner asked for the pre-release (item 6).
- **Updating the `nightly` release in place** (move the tag, replace the assets one by one): no window without a release, but a failure part way leaves archives from two commits. Deleting and recreating is simpler and never mixes commits.
- **Jobs inside `nightly.yml`:** one schedule, but the fuzz jobs would decide whether the run counts as successful, which breaks the change check.
- **`cargo-dist`:** a candidate for the release workflow and installers (architecture section 12); assessing it under NFR-14 is slice 20 item 8. A plain matrix needs no new tool.
- **Building every night regardless of changes:** simpler, but repeats identical builds and fills the artifact store.

## Consequences

- `verification-plan.md` section 8 (Platforms) names the nightly build job.
- No dependency enters the product. The workflow uses actions already used in `ci.yml` and `nightly.yml` (`checkout`, `rust-cache`, `install-action` for `cargo-about`, `upload-artifact`) plus `download-artifact` (v8, which reads unzipped artifacts), and the `gh` CLI on the runner for the release.
- The `nightly` tag moves every night it builds. It is not an application release tag; the tag schemes of slice 20 item 8 must not use the name.
- Watchers subscribed to releases are notified on each new nightly pre-release.
- Architecture section 12 and slice 20 are unchanged: the tag-triggered release build, packaging, installers and signing are still decided there.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Platform coverage of the binaries: the Linux build links the glibc of `ubuntu-latest` and will not start on older distributions; there is no Intel macOS or Linux arm64 build (OI-103) | Slice 20 | The packaging route |
| 2 | Code signing and notarisation: the nightly binaries are unsigned, so macOS quarantines them and Windows SmartScreen warns (OI-104) | Slice 20 | Signing certificates; the packaging route |
