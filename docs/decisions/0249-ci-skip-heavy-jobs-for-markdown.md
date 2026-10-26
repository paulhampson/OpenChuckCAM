# 0249: Skip the heavy CI jobs when a pull request changes only Markdown

- **Status:** proposed (26 Oct 2026). The project owner approved the task on 26 Oct 2026; the owner has not yet accepted this record. It amends [decision 0052](0052-ci-speed-up.md) (path filters of `ci.yml`) and changes no required-check name, no job condition and no Rust code.
- **Date:** 26 Oct 2026.
- **Lane:** CIFILTER, `claude/ci-markdown-path-filters`.
- **Requirements:** none. No requirement ID covers CI path filters, so none is cited; the change serves the speed goal of decision 0052 without weakening a check (NFR-06 and NFR-08 are not touched: no code, test or output changes).
- **Numbers:** takes 0249, OI-999 and OI-1000: the highest numbers on `main` and on every remote branch on 26 Oct 2026 were 0248 and OI-998. No project schema number.

## Context

Pull request #357 changed nine Markdown files and no other file, and ran the whole heavy CI (Build and test x6, UI smoke x3, Each library alone x3, Gate 3 x3). `Detect changes` in `ci.yml` filters by directory only. Two of the nine files are `testdata/README.md` (matches `rust` through `testdata/**`) and `tools/linuxcnc/README.md` (matches `rust` through `tools/**` and `gate3` through `tools/linuxcnc/**`). The header comment of `ci.yml` said a change to `docs/` alone does not rebuild the workspace, which is true only under `docs/`.

What the Markdown files the Fast checks read are, so that they keep running (read from `xtask/src`):

| Check | Reads | Filter that turns it on |
| --- | --- | --- |
| `cargo xtask records` | `docs/dependencies.md`, and through `registers.rs` `docs/open-items.md` and `docs/decisions/README.md` and the decision file names | `records`: `docs/requirements.md`, `docs/work-breakdown.md`, `docs/dependencies.md`, `testdata/**` |
| `cargo xtask testdata` | sizes of every file under `testdata/` | `records` |
| `cargo xtask trace` | `docs/requirements.md`, `docs/work-breakdown.md` | `records` |
| `cargo xtask conflicts` | every tracked file, Markdown included | `any` (`**`) |
| `cargo xtask deps`, `licences` | `Cargo.toml` files; Rust, TOML and YAML sources (`licences` skips `docs/` and `testdata/` and checks no Markdown) | `rust` or `records` |

## Decision

1. The four heavy filters `rust`, `step`, `gate3` and `fuzz` move to a second `dorny/paths-filter` step (`id: heavy`) that uses `predicate-quantifier: some-with-excludes` and ends each filter with `'!**/*.md'`. A file turns a heavy filter on only if it matches one of its patterns and is not Markdown. With the default `some`, `'!**/*.md'` would match every non-Markdown file and turn every filter on, and the quantifier is a property of the step, hence the second step. The step is pinned to `dorny/paths-filter@v4.0.3`, the first release with `some-with-excludes`, rather than the moving `v4` tag, which could not be verified from the session that wrote this record.
2. `records`, `any` and `linuxcnc_image` stay in the first step, unchanged, so a Markdown file still turns them on. The job output names are unchanged; only the step that supplies four of them changed. Fast checks therefore run on a Markdown-only pull request: always `conflicts`; and `deps`, `licences`, `records`, `testdata`, `trace` when it touches `testdata/**` or the three `docs/` files above (for example `testdata/README.md`).
3. A change to `.github/workflows/**` that is not Markdown still turns `rust` on, and `ci.yml` itself still turns on `step`, `gate3` and `fuzz`. This record's own pull request therefore runs the full CI.
4. `CI required checks` is untouched. Its "due" logic already lets a job that is not due be skipped and fails a due job that did not succeed; with `rust`, `step`, `gate3` and `fuzz` false the four heavy groups are not due, and `fast-checks` is due (`any` is true) and must succeed. A draft still fails the aggregate by design (decision 0052).
5. A push to `main` and a manual run use the same filters, so a Markdown-only push to `main` also skips the heavy jobs. That loses no cache or result worth keeping: the Rust cache is keyed on `Cargo.lock` and the sources, which a Markdown-only commit does not change, and the nightly run (decision 0102) builds and tests everything. A manual run is a request to run the jobs that the filter says; this is as before.
6. The header comment of `ci.yml` states the rule.

## Alternatives considered

- **A first step that lists the non-Markdown files and a second that filters the list.** `paths-filter` reads the pull request's changed files from the API and cannot filter a list passed in, so this needs a script step. Rejected as more code for the same result.
- **A single non-Markdown gate (`every`: `**`, `!**/*.md`) anded into each output.** Simple, but a pull request with Markdown under `testdata/` plus an unrelated non-Markdown file (for example an image under `docs/`) would run the heavy jobs. Rejected for precision.
- **Extglob patterns such as `crates/**/!(*.md)`.** Works but must be repeated for about fifty patterns and is easy to get wrong in a later edit. Rejected.
- **Skipping a Markdown file only under chosen directories.** Leaves the gap for any other directory (the cause of this problem). Rejected.

## Verification

- `actionlint` could not be installed in the session that wrote this record (running a downloaded binary was refused), so it was **not run**; the workflow parses as YAML, and the expressions changed are only `steps.filter.outputs.X` to `steps.heavy.outputs.X`.
- The filter logic was run locally with `picomatch`, the matcher `paths-filter` uses, on the patterns read from `ci.yml`, replicating the `some-with-excludes` rule of the action's source. The table of expected outputs is in the pull request description.
- The proof on GitHub is a probe, not run by this session: after this pull request merges, the Merge Manager watches the next Markdown-only pull request (#357) run only `Detect changes`, `Fast checks` and the CLA check.

## Consequences and limits

- A Markdown file that a build reads would be missed. No `include_str!` or test in `crates/`, `apps/`, `xtask/` or `benches/` reads a `.md` file (checked with a search on 26 Oct 2026); the crate `README.md` files are named in the manifests of the library family and `cargo xtask publish` checks that the file exists, not its content. A future change that makes a build read Markdown must add it to the filter by a pattern that does not end in `.md` or an exception in this step.
- The `fuzz` filter also ignores Markdown under `fuzz/` (corpus notes).

## Open items

- **OI-999:** `.github/workflows/e2e.yml` (CLI end to end, network unavailable, a separate workflow) has its own `rust` filter with `testdata/**` and `crates/**`, so a Markdown-only change under `testdata/` still starts that suite. It is outside this task, which names `ci.yml`; the same `some-with-excludes` step would fix it. Needs the owner's yes.
- **OI-1000:** `cargo xtask records` also runs the open-items register and decision-index checks (`registers.rs`), but the `records` filter does not list `docs/open-items.md`, `docs/decisions/**` or `docs/decisions/README.md`, so a pull request that changes only those does not run it (this was so before this record). Adding them to `records` is a strengthening, not a weakening; not done here because the task says to keep the `records` filter as it is. Needs the owner's yes.
