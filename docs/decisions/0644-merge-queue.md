# 0644: CI runs on the GitHub merge queue (NFR-06; amends decision 0055 items 4 and 5; lane MERGEQUEUE)

- **Status:** proposed by lane MERGEQUEUE on 2 Nov 2026. Only the project owner accepts it. The owner decided on 2 Nov 2026 to enable a merge queue; this record is the workflow change and the settings that go with it.
- **Date:** 2 Nov 2026.
- **Builds on:** [0055](0055-push-discipline-and-draft-pull-requests.md) (draft pull requests, push discipline, update from `main` only when released), [0052](0052-ci-speed-up.md) (draft gate, concurrency, `CI required checks`), [0358](0358-ci-time-where-it-goes-and-what-to-change.md) (the light run on `main`).
- **Open items:** OI-1999 (the light run on `main` after a queue merge), OI-2001 (the first-run checks of the queue).

## Context

The ruleset "Main branch protection" requires the checks `CI required checks` and `CodeRabbit` and sets "Require branches to be up to date" (strict). With about 14 pull requests waiting, every merge makes the others behind: GitHub refuses the merge (405, "Repository rule violations found. 2 of 2 required status checks are expected"), and the lane must merge `main`, push and wait for a full CI run of about 45 minutes. A merge queue builds each entry on top of `main` and of the entries ahead of it, and runs the checks on that commit, so the up-to-date rule is met by the queue and no lane pushes to catch up.

## Decision

1. **CI runs on `merge_group`** (`types: [checks_requested]`) in `ci.yml`. A merge_group run has no pull request, so `github.event.pull_request.draft` is empty: the draft condition does not skip the heavy jobs, and `CI required checks` does not fail on it. The run is a ready-pull-request run on the queue commit. **Whenever the group's diff changes any code path** (the `rust` filter: `crates/`, `apps/`, `xtask/`, `testdata/`, `.github/workflows/` and the other paths in `ci.yml`), a merge_group run always runs the full heavy set that a ready pull request runs (build and test on all three systems, Gate 3, each library alone, UI smoke, the STEP kernel job when its filter is on, fuzz smoke, semver); there is no light mode on merge_group. Only a group with no code change (Markdown only) skips them. `dorny/paths-filter` has no merge_group handling, so both filter steps are given `base: github.event.merge_group.base_sha`, and the diff covers every entry in the group (conservative). The fuzz smoke, the user guide check and `cargo semver-checks` (against the same base sha) run on merge_group as on a pull request.
2. **Draft rule unchanged.** A draft cannot be queued: `CI required checks` fails on the pull request run while it is a draft, and GitHub only lets a pull request join the queue when its required checks pass. Marking ready starts the full pull request run as before (decision 0055 item 1).
3. **Concurrency.** `ci.yml` and `e2e.yml` key a pull request run on its ref and any other run on its own run id, never cancelled. A merge_group run is "any other run", so a pull request run cannot cancel or replace it. No change to the expression, a comment says so.
4. **CLA.** The CLA check is not in the ruleset's required list. `cla.yml` gains a `merge_group` trigger on which the job `CLA signatures` passes at once without calling the action (the signatures were checked on the pull request), so requiring it later does not block the queue. The commit-author rule in `AGENTS.md` is unchanged and is still checked on the pull request.
5. **CodeRabbit** is a required check on the pull request head and never reports on a merge-group commit, so the queue would wait on it until it timed out. The owner removes `CodeRabbit` from the ruleset's required list. This PR does not touch the ruleset. I could not read GitHub's merge queue documentation from the session (docs.github.com is not in its network allow-list), so whether GitHub can require an external status for a queued commit is not verified: as I understand it, every required check must report on the queue commit itself, and CodeRabbit posts only on the pull request head. If the owner finds that GitHub supports it, the check may stay; the first-run check in OI-2001 settles it. The rule that no pull request merges without a CodeRabbit review of its final head stays, as a process rule for the Merge Manager (it is no longer enforced by GitHub).
6. **Decision 0055 items 4 and 5 are amended.** Lanes no longer merge `main` into a ready pull request to catch up: the queue builds on the current `main`. A lane still merges `main` for a real conflict or when a register check (`cargo xtask records`) needs it. The owner (or the Merge Manager, when the owner delegates) adds a ready, green pull request to the queue instead of setting auto-merge on an up-to-date branch. Items 1 to 3 and 6 stand.

## Alternatives considered

- **Keep strict and update each branch** (status quo): about 45 minutes and a push per pull request per merge ahead of it.
- **Drop "up to date" without a queue:** pull requests that pass alone could break `main` together; the queue keeps the check on the real merge result.
- **Force the full matrix on every merge_group run:** a Markdown-only pull request would run the 45-minute matrix again. The filters on the group diff keep it cheap, and a filter error on a code change is the same risk as on a pull request.
- **Light run for queue merges now** (the pushed tree is the tested queue tree): left to OI-1999, because it changes `tools/ci/main-run-mode.sh` and its test.

## Consequences

- **Workflows:** `ci.yml` and `cla.yml` as above. Not run: no merge queue can be run from a branch, so the merge_group paths are checked only by YAML parsing and reading. The first queued pull request is the test (OI-2001).
- **Cost:** each queue entry runs one full CI run on the queue commit, in addition to the pull request's own. Entries queue and build in sequence or in parallel as the owner sets the queue's build concurrency (docs/releasing.md, "Merge queue").
- **Main after a merge:** the push to `main` still runs; until OI-1999 closes, `main-run-mode.sh` finds no pull request tree that matches the queue commit, so it falls back to a full run (safe, not cheap).
- **Documents:** docs/releasing.md "Merge queue". `docs/requirements.md` and `docs/architecture.md` are untouched.
