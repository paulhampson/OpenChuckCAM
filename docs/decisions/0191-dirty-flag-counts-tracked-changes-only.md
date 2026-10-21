# 0191: The dirty flag counts tracked changes only; the nightly's untracked cache no longer marks it (lane BUG-BI; issue #241)

- **Status:** proposed. Only the owner accepts a record.
- **What it amends:** [decision 0151](0151-build-traceability.md) item 4 (dirty means `git status --porcelain` lists anything, untracked files included). NFR-16 is unchanged.
- **Date:** 21 Oct 2026.
- **Open items:** OI-710.

## Context

The owner's nightly build 0.13.0 (commit `eced9a1`, Windows) reported "Uncommitted changes: yes", though the installer came from the nightly workflow's artifacts, built on a clean checkout. A flag that is "yes" on every nightly means the commit hash cannot be trusted to identify the code (NFR-16).

Cause, read from the workflows: `build-archives.yml` (shared by the nightly and release workflows, decision 0091) runs the `occt` action before `cargo build`. That action restores the cached OpenCASCADE install into `occt-install/` in the checkout, or builds and copies it there on a cache miss. `.gitignore` listed only `/target` and the fuzz folders, so `git status --porcelain` showed `?? occt-install/` and the build script (decision 0151 item 4, which counted untracked files) wrote "yes". The same applied on all three platforms, not only Windows. The licence notices are downloaded after the build and the notices job builds nothing, so they are not involved. A Windows line-ending cause (`core.autocrlf`) is not excluded, since the job could not be run here; OI-710.

## Decision

1. **The cause:** `/occt-install` is added to `.gitignore`.
2. **The rule:** the build script asks `git status --porcelain --untracked-files=no`. A modified, staged, removed or renamed tracked file always marks the build dirty. An untracked file never does. *Recommendation and reason:* an untracked file is not part of the commit's code unless a tracked file refers to it (a new module needs a tracked `mod` line; a tracked `include_str!` of an untracked file does not compile on a clean checkout), so a tracked change is the signal that the commit no longer identifies the program. Counting untracked files lets any stray tool output, cache or editor file mark every build, and the flag stops meaning anything. Ignore rules alone would leave the next unlisted cache with the same effect.
3. **A diagnostic:** `build-archives.yml` prints `core.autocrlf`, `git status --porcelain` and `git diff --stat` before the build, for every platform and for the release and dry-run builds as well.
4. The G-code and setup sheet are unaffected: the version line there is the workspace version only and never reads the build information (decision 0151 item 7, decision 0110).

## Tests

`req_nfr_16_dirty_flag_on_a_real_checkout` (a temporary Git repository): a clean checkout is "no"; untracked files, including a restored `occt-install/lib.a`, stay "no"; a modified tracked file, a staged change and a removed tracked file are "yes". `req_nfr_16_dirty_flag_logic` still covers the text test.

## Alternatives considered

- **Keep counting untracked files and only ignore `occt-install`.** Fixes tonight's cause, leaves the flag fragile. Rejected.
- **Count untracked files under `crates/`, `apps/` and `posts/` only.** Closer to decision 0151's wording but more code for a case a tracked change already covers. Not chosen; can be added if the owner wants it.

## Consequences

An untracked source file that is not yet referenced by a tracked file does not mark a local build dirty. Release builds are unaffected (clean checkouts of a tag).
