# 0030: Slice 9, operation pipeline cache

- **Status:** accepted by the project owner (14 Oct 2026).
- **Date:** 14 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-131 and OI-132 are new.

## Context

Work breakdown slice 9 items 1 and 2 (headless) ask for a job manager with cancellation and progress (UI-09, NFR-04) and for an operation pipeline with caches and chain invalidation. Architecture section 6 fixes the rule: results are keyed by a hash of the operation's inputs and the engine version, and editing operation N invalidates N and every later operation. The job manager already existed from slice 0 (one rayon pool, cancel token, progress events, panics caught). This record covers the choices the architecture leaves open for the cache.

## Decision (accepted)

1. **Chained keys.** An operation's key is the BLAKE3 hash of: the engine version (the crate version), the previous operation's key, the operation, its setup, the machine profile, its tool snapshot, its part's model reference and the current contents of the part file. Because each key includes the one before, an edit to operation N (or to anything N reads) changes N's key and every later key, and no earlier key. This also covers the architecture's "stock left by earlier operations" input without a separate dependency list.
2. **Inputs are serialised with serde_json,** the plain-data form of the shared types; the project types are `Serialize` and use ordered maps, so the bytes are deterministic (NFR-06). Each field is length-prefixed.
3. **The part file is hashed on each run,** not only the hash the project recorded (PRJ-02), so a file changed on disk is never served from the cache.
4. **In memory, a configurable number of results per operation.** The cache is shared with the background job, never written to the project (PRJ-03). It keeps the latest few results of each operation, most recently used first (`Application::set_cache_versions`; default `DEFAULT_CACHE_VERSIONS` = 3: the current state and two earlier ones, enough for a couple of undo steps while a long toolpath is held at most three times; 0 turns caching off). Lowering the number drops older results at once. Entries of removed operations are dropped at each run; registering a strategy clears it (a strategy of the same name could differ). Failed generations are not cached. Posting is not cached: it runs on the whole toolpath each time.
5. **Simulation (slice 7, decision 0025) is not cached.** The stock an operation is generated against comes from the simulation of the operations before it, which the chained key already fixes, so a cached toolpath is valid on carried stock. The simulation itself still runs for cached operations when stock is carried or a report is asked for, because it needs each toolpath in turn (OI-132). The simulation settings are part of the key.
6. **`Regeneration` reports `generated` and `reused`** operation IDs, so the UI can show what was rebuilt and tests can check the chain.
7. **New dependency use:** `serde` and `serde_json`, already pinned in the workspace, are now used by `openchuck-app` (no new crate).

## Amendment after review (15 Oct 2026)

CodeRabbit's reviews of pull request 38 led to five refinements, which refine items 3 and 4 and change none of the decision:

- **Part files are hashed once per run** (not once per operation), so several operations sharing a large model cost one read.
- **A result is stored only if the part file still has the hash it was keyed with** after generation, and only if the file could be hashed at all. Generation reads the file itself, so a file that changed mid-run must not leave a result under the old key (`req_nfr_04_a_result_made_while_the_part_file_changed_is_not_cached`).
- **The number kept and the entries share one lock,** and a lookup returns nothing while the number is 0, so lowering or disabling the cache cannot be overtaken by a concurrent store.
- **A run whose part files changed is refused.** The part files are hashed at the start and checked again at the end; if one changed, the run fails with "a model file changed while regenerating; regenerate again". Otherwise an operation served from the cache (made from the old contents) could sit beside one generated from the new, in the same toolpath and the same carried stock (`req_nfr_04_a_cache_hit_is_not_mixed_with_results_from_a_changed_part_file`).
- **With caching off (0 kept), none of this is done:** no hashing, keys or end check.
- **Disabled operations (slice 8, decision 0029 item 10) are outside the chain.** Regeneration skips them, so disabling operation N changes the previous key of the enabled operation after it, which is then made again (the stock it sees has changed). Their cached results are kept, so enabling one again reuses them (`req_nfr_04_disabling_an_operation_regenerates_those_after_it_and_enabling_reuses_them`).

## Alternatives considered

- **Explicit dependency graph between operations:** rejected; operations form a chain (architecture section 6), and chained keys give the same invalidation with nothing to keep in step.
- **Keep only the latest result per operation:** rejected on review; undo and toggling a parameter back would regenerate, so the number is a setting instead.
- **Trust the recorded model hash:** rejected; a stale cache after an external edit is worse than one extra file hash.

## Consequences

- Verification plan, openchuck-app, pipeline cache invalidation: `req_nfr_04_editing_operation_n_regenerates_n_and_later_operations_only` edits each position of a three-operation chain, a second covers shared inputs and the part file, and a third the number of results kept.
- Cache across sessions ("regeneration after load cheap", architecture section 6) needs an on-disk form: OI-131.
- Simulation runs in the regeneration job but is re-run for cached operations; caching its checkpoints is OI-132 (it matters for playback, SIM-04).

Numbering: first written as 0025 with OI-108 and OI-109, then renumbered as `main` and the unmerged branches took those numbers (slice 7 took 0025, slice 8 took 0029 and OI-127 to OI-130). Record 0030 and OI-131, OI-132 are free on `main` and on every unmerged branch as of 15 Oct 2026; lane G and slice 10 skipped over them. This is lane B of the work breakdown's parallel plan; no project schema number is claimed, because the cache lives in memory and (OI-131) in the cache folder, not in the project files.
