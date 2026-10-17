# 0103: The built-in posts live inside the `openchuck-post` crate (so the library family can be published)

- **Status:** accepted by the project owner on 17 Oct 2026, in the orchestration session, with the wording of architecture section 12 in item 3 (applied to `docs/architecture.md` in this change).
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none are new. The question this answers is OI-302, which sits on lane TR's branch and is not on `main` yet; it closes when this record is accepted and lane TP has moved the files.

Numbering: the orchestrator took the next free record number across `main` and every unmerged branch (highest seen 0102).

## Context

`openchuck-post` embeds the two built-in posts with `include_str!("../../../posts/...")` (`crates/openchuck-post/src/builtin.rs`), which reaches outside the crate directory to the repository-root `posts/` folder that architecture section 12 defines. `cargo publish` packages only the files inside a crate, so a published `openchuck-post` would not build, and `cargo publish --dry-run` fails for it. Publishing the library family is lane TP's work (work breakdown, planned by the owner's decision of 17 Oct 2026); where the posts live is an architecture question that the owner answers first.

## Decision

1. **The built-in posts move into the crate:** `posts/makera-z1/` and `posts/linuxcnc/` become `crates/openchuck-post/posts/makera-z1/` and `crates/openchuck-post/posts/linuxcnc/`, with their README. `include_str!` paths become `../posts/...` relative to `src/`. There is one copy; nothing is synchronised at publish time.
2. **Lane TP moves them,** in the same change that makes the library family pass `cargo publish --dry-run`. The move covers every reference to the old path: `.gitattributes` (`posts/** -text`), the path filters in `.github/workflows/ci.yml`, the fuzz corpus step that reads `posts/*/`, the tests that read `posts/linuxcnc` (`openchuck-app` `posting.rs` and `round_trip.rs`, `openchuck-ui` `export_panel.rs`), the golden-test locations and the documents that name `posts/`. The user-facing behaviour does not change: a post is still a folder, `openchuck-cli regenerate --post <folder>` still takes any folder, and a user's own post still needs no rebuild.
3. **Architecture section 12, wording agreed by the owner:** the `posts/` row of the repository table is replaced by "(`crates/openchuck-post/posts/`) Built-in post definitions as data files, inside the `openchuck-post` crate so the crate packages them: the Makera Z1 post and the LinuxCNC verification post, each with golden tests." and section 12's mention of the top-level `posts/` folder is removed or pointed to the crate.

## Alternatives considered

- **Keep `posts/` at the root and copy it into the crate before publishing (a build step or `xtask`).** The architecture would stay as written, but there would be two copies, and a forgotten sync would publish stale posts. Rejected by the owner.
- **Keep `posts/` at the root and load posts at run time from the application's data directory.** It would change the crate's behaviour: the library would carry no built-in posts, and the CLI and UI would need to find them. Rejected by the owner.

## Consequences

- The moved files keep their history through `git mv`. CI path filters change from `posts/**` to the crate path, which the `crates/**` filter already covers.
- Lane TP can then run `cargo publish --dry-run` on `openchuck-post` without a path leaving the crate (OI-302). Other reasons the family might fail `--dry-run` are lane TP's to find.
- `docs/architecture.md` section 12 (the repository table and the CI path-filter sentence) and the changes list in section 13 carry the agreed wording.
