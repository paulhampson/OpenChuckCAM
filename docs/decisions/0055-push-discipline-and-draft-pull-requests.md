# 0055: Push discipline and draft pull requests for lane sessions

- **Status:** accepted by the project owner (16 Oct 2026, in the orchestration session, on the orchestrator's account of where CI time was going). Written up on 17 Oct 2026 after the rules had been in use for a day.
- **Date:** 16 Oct 2026 (accepted), 17 Oct 2026 (recorded).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none are new.

Numbering: the orchestrator reserved 0055 on 16 Oct 2026. It is recorded after 0056 and 0057 because those lanes' records merged first.

## Context

A full pull request CI run takes about 20 minutes with warm caches (decision 0052) and more when the queue is busy, and every push restarts it. With several lanes open at once, each push, each update from `main` and each pull request that ran the heavy jobs while still being worked on lengthens the queue for all of them (decision 0054 records the same cost for register conflicts).

## Decision

1. **New lane pull requests open as drafts.** The heavy CI jobs (build and test on each platform, Gate 3, fuzz, coverage) skip on a draft, so a draft runs only change detection and the fast checks. The `CI required checks` job fails on a draft on purpose: a draft cannot merge. A lane marks its pull request ready for review only when the orchestrator releases it.
2. **One push after CodeRabbit and CI have reported.** A lane waits for both, then fixes everything in a single push. Nit, minor and optional findings never cause a push on their own: they ride a push that is needed anyway, or a follow-up pull request. A finding that is real and blocking is fixed at once.
3. **Docs-only commits ride the next code push.** A lane keeps them local until a code push is needed.
4. **Update from `main` only when released.** The orchestrator releases one session at a time, in merge order, and releases the next only after the previous pull request has merged. The released lane first marks the pull request ready, then merges `main` (registers merge as unions, decision 0054), pushes once and reports the head commit. (Amended 17 Oct 2026, relayed owner decision: ready first, then push. Marking ready after the push races: the push's event and the ready event start two runs in one concurrency group, and the surviving run can carry the stale draft flag, so the heavy jobs skip and `CI required checks` fails, as on pull request 106. Ready first starts a run on the old head; the push, made with the draft flag off, cancels it and starts the real run.)
5. **The owner sets auto-merge.** Auto-merge needs a pull request that is up to date with `main`, so the orchestrator asks for it when the pull request has been updated from `main` and marked ready, not when it is green. Lanes never merge or set auto-merge.
6. **Each push is reported.** A lane reports the head commit of every push to the orchestrator, so the orchestrator can check the branch against what was agreed.

## Alternatives considered

- **Let every lane update from `main` whenever it likes:** each update restarts CI and invalidates a neighbour's, so the queue grows with the number of lanes.
- **Require green before marking ready:** a ready pull request would then run the heavy jobs twice, once while being fixed and once on release.
- **Cap the lanes only:** the owner's cap of five lane sessions bounds the queue, but does not stop one lane from restarting CI repeatedly.

## Consequences

- **Documents:** `AGENTS.md` gains the rule above. The work breakdown's "Parallel work" section already names merge order and the checks before a push.
- **CI:** no change. The draft gate and the strict `CI required checks` aggregator were added with decision 0052.
- **Cost:** a lane waits for its release before it can merge, and a pull request can sit as a draft for hours. That is intended.
