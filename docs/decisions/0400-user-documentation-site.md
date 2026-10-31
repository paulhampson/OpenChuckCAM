# 0400: The user documentation site: where it lives, how it is versioned and published, and documentation as a release gate (lane DOCSITE)

- **Status:** proposed. The owner answered D0, D2, D3, D4, D5 and D6 on 31 Oct 2026, relayed by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7, decision 0049); those answers are recorded below as the owner's decisions. The owner chose D1 (mdBook) on 31 Oct 2026 after seeing a prototype of mdBook and of Zola, each with a proposed home-page hero, relayed the same way. D7 (who writes `docs-releases`) is open: the planned deploy key cannot be a ruleset bypass actor on github.com (section D7). The record as a whole stays proposed until the owner accepts it.
- **Date:** 31 Oct 2026.
- **Lane:** DOCSITE, branch `claude/lane-docsite`. Prototypes for D1 on `claude/lane-docsite-proto` (never merged; `proto/COMPARISON.md`).
- **Requirements:** NFR-11 (user documentation, P2), NFR-12 (G-code must be verified by the user), NFR-09 (offline, no telemetry), NFR-10 (licences), NFR-14 (dependency health), NFR-15 (legibility and accessibility). New: NFR-19 (D2, agreed by the owner).
- **Builds on:** [0031](0031-slice-20-packaging-and-trial-documents.md) and [0091](0091-release-workflow-and-gate.md) (the release workflow), [0106](0106-air-cut-is-a-manual-release-step.md) (the air cut as a manual release step), [0114](0114-slice-22-conformance-report.md) (plain wording), [0249](0249-ci-skip-heavy-jobs-for-markdown.md) and [0358](0358-ci-time-where-it-goes-and-what-to-change.md) (CI cost), [0055](0055-push-discipline-and-draft-pull-requests.md) (draft pull requests, one push).
- **Open items:** OI-1520 to OI-1528 (new). OI-1520, OI-1521 and OI-1524 are closed by the owner's answers. OI-1220 (lane DOCSUSER, the screenshot capture) stays open; section G proposes its answer.

Numbering: the coordinator assigned decisions 0400 to 0409 and OI-1520 to OI-1539 to this lane on 31 Oct 2026, after checking `main` and the unmerged branches. This record takes 0400 and OI-1520 to OI-1528.

## Context

The owner asked (31 Oct 2026) for the foundation of a documentation website for people who use OpenChuckCAM, published on GitHub Pages and seeded with lane DOCSUSER's tutorial. The style is to be clean, clear and plain: easy for someone new to CAM, without talking down to them, and without the look and wording that mark text and pages as machine-made. Separately, the owner asked that the documentation be kept up to date as a condition of each release, and that readers can switch between the documentation of each release and the development version.

`docs/` stays the project's engineering source of truth and is not the user guide. NFR-11 covers user documentation, but nothing covered how it is published, versioned or kept current.

## Decision

### D0. Where the documentation lives (owner's decision: C)

The source of the user guide is a directory on `main`, edited in the same pull request as the behaviour it describes, with CI that runs only when that directory changes. Built HTML is never committed to `main`. The built documentation of each application release is kept on a publish-only branch, `docs-releases`, written only by the release workflow, never by hand (section V).

Rejected: a long-lived source branch for the documentation (B). It separates the documentation from the change that makes it necessary, so it drifts from the code by construction, needs a second merge queue, branch protection and CodeRabbit configuration, and breaks the pairing between a screenshot and the build that drew it.

### D1. The site generator (owner's decision: mdBook)

**mdBook** 0.5.4 (MPL-2.0, maintained by the rust-lang organisation), with the project's own theme replacing mdBook's completely, no preprocessor and no plugin. The owner chose it on 31 Oct 2026 after seeing two prototypes with the same pages and stylesheet, mdBook and Zola 0.23.6, each with a proposed home-page hero (`claude/lane-docsite-proto`, never merged; `proto/COMPARISON.md` and `proto/screenshots/`). The reasons the comparison gave: one contents file (`SUMMARY.md`) that sets the order and shows unwritten pages as drafts; a print page for the whole guide; built-in offline search; relative links, so one build works at any address; Markdown that also reads correctly on GitHub; the least of our own code. Zola's own advantages (links checked at build time, a free landing page) did not outweigh that Zola 0.23.0 (5 August 2026) had just replaced its template language and removed shortcodes. The hero is an HTML block at the top of `src/index.md` plus three template lines, with the contents sidebar kept.

Hugo, MkDocs with Material, Docusaurus and a hand-written generator were not prototyped: a larger toolchain or supply chain, a recognisable template look, or code we would maintain forever.

### D2. A requirement for the published site (owner's decision: A)

A new requirement, NFR-19 (P2), added to `docs/requirements.md` in the pull request that builds the site, with this agreement quoted:

> NFR-19. The user documentation is published as a static website built from the repository: the development version from main and one fixed version per application release, which the reader can switch between. The site can be read without JavaScript, makes no request to any other site, sets no cookies and collects no analytics (as NFR-09), meets the contrast and keyboard rules of NFR-15, and states on every page that covers generating or exporting G-code that the user must verify it before running it on a machine (NFR-12). (P2)

Verification plan row: `NFR-19 | P2 | T, I | PR and Release | The docs check job (links, anchors, HTML, no third-party request, the NFR-12 line) passes; contrast and keyboard use inspected at release`.

### D3. The directory (owner's decision: B, `user-docs/`)

The directory is `user-docs/`, not `site/`. Architecture section 12 gains this row, with this agreement quoted, in the pull request that creates the directory:

> | `user-docs/` | The user documentation website: the source (`user-docs/src/`), its theme, the tutorial images and their capture script, and the docs check and assembly scripts. Written for people using the program, not for contributors: `docs/` stays the project's source of truth, and a page that adapts a `docs/` document (the Z1 air-cut procedure and safe-practice checklist) names it and defers to it. Built HTML is never committed to `main`; released versions are kept on the `docs-releases` branch, written only by the release workflow (decision 0400). |

and the section 12 CI convention gains: "A change only to `user-docs/` runs the fast checks and the docs check, not the heavy jobs."

### D4. Order with lane DOCSUSER (owner's decision: A, DOCSITE first)

1. DOCSITE pull request A: the foundation and versioning (sections S, V, P, R below), with the tutorial as a stub page.
2. DOCSITE pull request B: lane DOCSUSER's tutorial text, one page per step under `user-docs/src/tutorial/`, its images moved to `user-docs/src/tutorial/images/`, `capture.py` moved to `user-docs/tools/` with its output set there, and a visible note that the screens are from an earlier build (they show five steps; [decision 0282](0282-four-step-workflow-with-separate-simulate-and-export-actions.md) made four).
3. Lane DOCSUSER phase 2 rebases on `main` and: drops `TUTORIAL.md` and `docs/tutorial/` from its branch (they never reach `main`); keeps `testdata/examples/baluster` and OI-1220; rewrites `README.md` to link to the published guide and to `user-docs/src/tutorial/`; references the hero images from `user-docs/src/images/`; keeps "Main limitations" and "Getting it and running it" only in the guide, with a summary and a link in the README; re-captures into `user-docs/src/tutorial/images/` and edits the tutorial for four steps, removing the note; follows `user-docs/STYLE.md` and runs the docs build and check before pushing.

One copy of every page and image exists: in `user-docs/`.

### D5. Documentation as a release gate (owner's decision: b)

NFR-11 and NFR-19 stay P2. Verification plan section 9 gains an explicit first-release exit criterion, and `docs/releasing.md` gains an application release checklist; the release workflow enforces the items that can be checked. The wording is in section R.

### D6. The site's front address (owner's decision: the newest release)

The root of the site redirects to the newest release once one exists, and to `/latest/` until then.

### S. Structure, style and privacy

- **Pages** (`user-docs/src/SUMMARY.md`). Written: Home (with the hero), Install and first run, Known limitations, four concepts pages (rotary machining, stock and the held region, roughing and finishing, stepover and stock to leave), the three safety pages (checking G-code; the Z1 air cut and the safe-practice checklist, adapted from their `docs/` sources, which they name as the authority), the glossary, the release notes (the development version only), and contributing. Stubs, each opening with "This page is not written yet." and saying what it will cover: the tutorial (until pull request B), three concepts pages, five how-to guides, seven reference pages (the post-processor guide among them, OI-1522), and troubleshooting (with its log-file section written).
- **Style.** `user-docs/STYLE.md`: plain words (decision 0114), each CAM term defined once and linked to the glossary, UI labels as the program shows them, numbered procedures, Note and Warning callouts only, figures with numbered captions and alt text, the NFR-12 wording rules, and a 12-point review checklist. `user-docs/tools/check.py` enforces what can be checked: marketing words, placeholder text, wording that calls output safe, alt text and captions, the program's own G-code notice in every footer (read from `crates/openchuck-types/src/notice.rs`), and requests to other sites.
- **Design.** A warm off-white or near-black page, near-black or off-white text and one oxidised-steel accent, with the tokens and their contrast ratios in `STYLE.md` (every text colour meets WCAG AA, most AAA); the interface's own IBM Plex files, unmodified, with their OFL licence and notice beside them; 17 px text, 1.65 line height, a measure of about 68 characters; a light and a dark theme following the system, with a switch; print styles; the contents fold away at phone width when JavaScript runs.
- **Privacy.** No analytics, no cookies, and no request to any other site: fonts, scripts, the search index and images are served from the site. The theme choice is kept in `localStorage`, not a cookie. Pages read without JavaScript; scripts add search, the theme switch and the version list.

### V. Versions

- **Paths.** `/latest/` is built from `main` on each change to `user-docs/`. `/v<semver>/` is one fixed copy per application release (`app-v<semver>` tag). `/versions.json` and `/versions.html` list them. `/` redirects as D6 says.
- **Making a release's copy.** In `release.yml`, a job `docs` (after `plan` and `gate`; `app-v` tags only) builds `user-docs/` at the tag, stamped with the version, tag and commit, runs the checks of section R, and keeps the result as a workflow artifact. A job `publish-docs` (after `docs` and `publish-app`) checks out `docs-releases` (creating it as an orphan branch on the first release), refuses if `v<version>/` exists, adds the folder with a `MANIFEST.sha256` of its files, commits once and pushes a fast-forward. The release notes give the manifest's SHA-256. A dry run builds and checks but does not push.
- **Keeping released copies fixed.** Only the release workflow writes `docs-releases`, with a credential held in an environment `docs-release` that only `app-v*` tags may use (which credential is D7). A ruleset on the branch allows updates only from that writer, and blocks force pushes and deletion. Every deploy checks each version folder against its `MANIFEST.sha256`. The workflow never replaces or deletes a version folder. Withdrawing a released copy (for example for an error with safety consequences) is an owner action under the ruleset's bypass, recorded in a decision record, and the folder is replaced by a page saying why.
- **Publishing.** The Pages source is GitHub Actions. On a push to `main` that changes `user-docs/`, after a successful release run (`workflow_run`), or by hand, a deploy job builds `/latest/`, fetches `docs-releases` read-only, checks every version folder against its manifest (a mismatch fails the deploy), writes `versions.json`, `versions.html` and the root redirect, and deploys the whole as one Pages artifact. `/latest/` is never committed anywhere.
- **The version switch and banners.** Every page names its version in a `<meta>` element and in its footer. The header's "Version" link opens `/versions.html` (works without JavaScript); with JavaScript it becomes a list that opens the same page in the chosen version where that page exists. Pages in `/latest/` carry a fixed note that they describe the development version and may describe unreleased features. A released copy is frozen, so its note that a newer release exists is shown by a small script that reads `versions.json`.
- **Nightly builds.** The rolling `nightly` pre-release is built from `main`, so its documentation is `/latest/`. No copy is kept for nightly builds. Pointing the archives' README at the guide (`/latest/` for a nightly build, `/v<semver>/` for a release) is not built yet (OI-1528).

### P. Pull request checks and cost

A separate workflow, `docs-site.yml`, runs on pull requests and pushes to `main` that change `user-docs/` or the workflow, after a successful release run, and by hand. Its check job (permission `contents: read` only) runs the tools' tests (`python3 -m unittest`, test names `test_req_nfr_19_*` and `test_req_nfr_12_*`), builds and assembles the site, and runs `check.py`, a link and anchor check (lychee, offline) and an HTML check (the Nu HTML Checker). mdBook and lychee are installed with `cargo install --version ... --locked` from crates.io, whose index records each crate's checksum, using the runner's own Rust, outside the checkout so the project's toolchain is not fetched; the Nu HTML Checker comes from the npm registry and is checked against a pinned SHA-256. The three are cached by version, so only the first run pays the install (several minutes); later runs take about two. The job is not one of the required checks of `ci.yml`. The deploy job runs only on `main` with `pages: write` and `id-token: write`, and skips with a notice when Pages is not switched on. Every action is pinned by full commit SHA; a tool pin that could not be verified would be written `UNVERIFIED`, which the check fails on (none is). A change only to `user-docs/` turns on none of `ci.yml`'s heavy jobs (decision 0249); the pull request that adds the workflows turns them on once, because `.github/workflows/**` is in their filter.

### R. The release gate

1. **Verification plan section 9**, first-release exit criteria, new bullet:
   > The user documentation for the release is current: the release workflow's documentation check passes and the documentation items of the application release checklist (`docs/releasing.md`) are ticked in the verification report.
2. **`docs/releasing.md`**, new section:
   > ## Each application release
   >
   > Before pushing the tag `app-v<version>`, the project owner ticks these items and records them in the release's verification report (verification plan section 9, Records). The release workflow checks the items marked *(checked)* and refuses to publish if they fail.
   >
   > - [ ] The Makera Z1 air cut has been run, or the release notes say which Z1 values are unconfirmed (decision 0106).
   > - [ ] The tutorial screenshots match this release: re-captured with `user-docs/tools/capture.py` at the release commit, or checked against the release build and marked reviewed in `user-docs/src/tutorial/images/CAPTURED.toml`. *(checked: the capture or review commit is not older than the last change to the screens)*
   > - [ ] Every setting, operation, strategy, machine-profile field and command-line command added or changed since the previous release is described in the reference pages.
   > - [ ] The release-notes page has a section for this version: what changed, the known limitations, the unconfirmed Z1 values and the notice that generated G-code must be verified before it is run on a machine. *(checked: a section headed with the version exists)*
   > - [ ] The safety pages still agree with `docs/z1-air-cut-procedure.md` and `docs/z1-safe-practice-checklist.md`.
   > - [ ] The documentation builds with no broken link or anchor, valid HTML and no request to another site. *(checked)*
   > - [ ] The footer of every page names this version and tag. *(checked)*
   >
   > After the release, the documentation for `app-v<version>` is on the `docs-releases` branch and listed in the version switch.
3. **`release.yml`**: the `docs` job fails, and so stops `publish-app` and `publish-docs`, when a check of section P fails, when `CAPTURED.toml`'s capture and review commits are both older than the newest commit that changes `apps/openchuck-ui/`, `crates/openchuck-viewport/` or `crates/openchuck-app/src/`, when the release-notes page has no section for the version, when the stamp does not equal the tag, or when a placeholder pin remains. The existing `gate` job and `cargo xtask release-gate` do not change. Library tags (`lib-v`) make no documentation copy.
4. **Who:** the owner ticks the checklist, as for the air cut. A lane that changes a screen updates the pages it affects and either re-captures or leaves the staleness warning for the gate to catch.

### G. Screenshots (proposed answer to OI-1220)

Pull request B moves `capture.py` to `user-docs/tools/` and has it write `CAPTURED.toml` (`captured_at`, the commit; also the date, renderer and window size) beside the images; the owner or a lane that checks the screens without re-capturing adds `reviewed_at`. `check.py` (built in pull request A) compares both with the newest commit that changes `apps/openchuck-ui/`, `crates/openchuck-viewport/` or `crates/openchuck-app/src/`: the release workflow refuses stale screens, pull requests only warn. A weekly check that opens an issue when the screens have changed is proposed, not built. Running the capture itself in CI stays out of scope (it needs the release build and a virtual display). OI-1220 stays open for the owner to accept this answer.

### D7. Who writes `docs-releases` (open, OI-1527)

The plan was a deploy key with write access as the ruleset's only bypass actor. GitHub's documentation (its source, `github/docs`, `data/reusables/repositories/rulesets-bypass-step.md`, read on 31 Oct 2026) lists deploy keys as bypass actors only on GitHub Enterprise Server; on github.com they are repository roles, teams, GitHub Apps and Dependabot. Proposed: a private GitHub App with **Contents: Read and write** only, installed on this repository only, as the ruleset's only bypass actor; its client ID and private key are held only in the `docs-release` environment, and `publish-docs` gets a short-lived token from it with `actions/create-github-app-token`. The alternative, a deploy key with a ruleset that does not restrict updates, would let anyone with write access push to the branch (the manifest check would still catch a changed folder whose manifest was not changed with it). The owner's answer settles `publish-docs` and the one-time setup in `docs/releasing.md`.

## Owner actions (not done by this lane)

The numbered steps are in `docs/releasing.md`, "One-time setup for the documentation site". In short: switch GitHub Pages to **GitHub Actions**; create the writer of `docs-releases` (D7) and the `docs-release` environment limited to `app-v*` tags; add the ruleset on `docs-releases`; and check the result with a dry run and a hand push that must be refused.

Until Pages is switched on, the deploy job skips with a notice. Until the rest is done, `publish-docs` fails at the first application release and says what is missing; the program release itself is published by then.

## Consequences

- New: `user-docs/`, `.github/workflows/docs-site.yml`, two jobs in `release.yml`, a section in `docs/releasing.md`, NFR-19 and its verification-plan row, the architecture section 12 row, rows in `docs/dependencies.md` (CI tools) for the generator and the check tools.
- `.coderabbit.yaml` keeps its exclusions and adds `user-docs/src/` and `user-docs/theme/`; the tools in `user-docs/tools/` are code and are reviewed.
- No change to the Rust workspace, the library family, a project, machine or library schema, or `xtask`.

## Open items

| ID | Item | State |
| --- | --- | --- |
| OI-1520 | The requirement for the published site | Closed by the owner's answer to D2 (NFR-19, P2) |
| OI-1521 | The directory and the architecture section 12 row | Closed by the owner's answer to D3 (`user-docs/`) |
| OI-1522 | The guide to writing a post-processor (part of NFR-11) is not written | Open: a reference page stub until written; not a release blocker (D5) |
| OI-1523 | The versioning design of section V | Built in pull request A; open until the first release has run it |
| OI-1524 | Where the documentation source lives | Closed by the owner's answer to D0 (C) |
| OI-1525 | Pin the remaining GitHub Actions in the other workflows by full SHA (most are pinned by tag) | Open, low priority; not this lane |
| OI-1526 | Documentation as a release gate (section R) | Open until it has run on a release; the wording is the owner's answer to D5 |
| OI-1527 | Who writes `docs-releases` (D7): deploy keys cannot bypass a ruleset on github.com | Proposed: a GitHub App; waits for the owner |
| OI-1528 | The archives' README does not point to the user guide (`/latest/` for a nightly build, `/v<semver>/` for a release) | Open; a small change to the archive README template |
