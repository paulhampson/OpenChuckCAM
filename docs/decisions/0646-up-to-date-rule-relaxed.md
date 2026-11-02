# 0646: Relax "Require branches to be up to date"; the merge queue is not available here (NFR-06; supersedes in part decision 0498; amends decision 0055 items 4 and 5; lane MERGEQUEUE)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049), after reading it in full. Proposed by lane MERGEQUEUE the same day, at the coordinator's instruction recording the owner's decision. Drafted as 0646; renumbered at release under the owner's numbering rule (main's highest decision plus one).
- **Date:** 2 Nov 2026.
- **Builds on:** [0498](0498-merge-queue.md) (accepted; this record supersedes its enablement steps), [0055](0055-push-discipline-and-draft-pull-requests.md) (push discipline, update from `main` only when released), [0052](0052-ci-speed-up.md) (`CI required checks`), [0358](0358-ci-time-where-it-goes-and-what-to-change.md) (the light run on `main`).
- **Open items:** OI-1999 (reworded), OI-2001 (deferred), OI-2010 (new).

## Context

Decision 0498 prepared CI for a GitHub merge queue and listed the owner's steps to enable it. The repository belongs to a personal account, and GitHub's merge queue is not available there (it needs an organisation-owned repository); I could not read GitHub's documentation from the session, so this is the coordinator's report of what the owner found. The problem 0498 set out to solve remains: with "Require branches to be up to date" on, every merge makes the other ready pull requests behind, and each must merge `main`, push and wait for a full CI run of about 45 minutes.

## Decision

1. **The owner relaxes the rule.** In the ruleset "Main branch protection" the owner clears **Require branches to be up to date before merging** (the strict required-status-checks policy). The required check `CI required checks` stays. Nothing else in the ruleset changes in this record.
2. **The queue steps in 0498 and `docs/releasing.md` are withdrawn.** 0498 stays as accepted, with a one-line pointer to this record. The `merge_group` triggers in `ci.yml` and `cla.yml` stay: they are harmless without a queue and are what a future organisation-owned repository needs.
3. **The Merge Manager serialises merges instead of GitHub.** One pull request merges at a time, in the order the Merge Manager sets; a pull request that is behind `main` no longer has to be brought up to date and re-run before it can merge (this amends decision 0055 item 4: the "update from `main` only when released" step is needed only for a real conflict or a register check, not to satisfy the rule). The process rule that no pull request merges without a CodeRabbit review of its final head, and that `CI required checks` is green on that head, stays.
4. **CI runs on `main` after each merge.** The push to `main` already runs `ci.yml`; the Merge Manager looks at that run before the next merge. A pull request merged while behind has never been tested together with the pull requests merged after it was last run, so this run is the first test of the result.
5. **If `main` goes red:** the Merge Manager stops merging, finds the pull request that broke it (the last merge or the pair of merges), and either asks its lane for a fix pull request or has the owner revert it; merging resumes when `main` is green again. A red `main` is fixed before any other merge.

## Alternatives considered

- **Move the repository to an organisation to get the queue:** a larger change than the owner wants now; the workflow side is ready (0498) if it happens.
- **Keep the rule and update every branch:** the cost that started this work.
- **Auto-merge on up-to-date branches only (decision 0055 item 5):** same serial cost.

## Consequences

- **Risk accepted by the owner:** two pull requests that are green alone can break `main` together; item 4 catches it after the merge, not before.
- **CI:** no change to workflows.
- **Light run on `main`** (decision 0358, Option 4): it applies only when the merged tree equals a pull request's tested tree, which is rare now, so the push to `main` is normally a full run (OI-1999).
- **Documents:** `docs/releasing.md` replaces "Merge queue" with "Up-to-date rule relaxed"; 0498 gets a pointer line; the open-items rows are updated. `docs/requirements.md` and `docs/architecture.md` are untouched.
