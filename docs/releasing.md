# Publishing the library family to crates.io

What the project owner does to publish `openchuck-types`, `openchuck-gcode`, `openchuck-path` and `openchuck-post` to crates.io ([decision 0138](decisions/0138-library-family-publish-step.md), OI-303). The release workflow (`.github/workflows/release.yml`, [decision 0091](decisions/0091-release-workflow-and-gate.md)) does the publishing, and only when the owner asks for it. A tag push alone never publishes a crate.

Publishing cannot be undone. A published version can be yanked (new projects stop resolving it), but it cannot be deleted or replaced, and its source stays downloadable.

## What is published, and what is not

- **Published:** the library family only, the four crates above, all at the workspace version (lockstep, decision 0031 item 2). Each package holds `src/`, `README.md` and `CHANGELOG.md`, plus `examples/` (`openchuck-types`, `openchuck-path`) and `posts/` (`openchuck-post`: the built-in posts, [decision 0103](decisions/0103-built-in-posts-live-in-the-post-crate.md)). The tests stay out: they read the repository's `testdata/` and the test-only `openchuck-testkit`.
- **Never published:** `openchuck-testkit`, `openchuck-buildinfo`, `openchuck-cli`, `openchuck-ui`, `xtask` and `openchuck-benches` say `publish = false`. The library family does not depend on any of them, nor on the application crates: `cargo xtask deps` and `cargo xtask publish` check this on every pull request. The other application crates (`openchuck-geom`, `-import`, `-sim`, `-project`, `-app`, `-viewport`) are not published by the workflow either.
- **Names:** the owner reserved the four library names on 11 Oct 2026 with `0.0.0` placeholder versions (and `openchuck-geom`, `openchuck-import`, `openchuck` and `openchuckcam` the same day). The first real publish becomes each crate's latest version; the placeholder stays in the version list.

## Once: set up the credential

1. Sign in to [crates.io](https://crates.io) with the GitHub account that owns the crates (`paulhampson`).
2. **Account Settings, API Tokens, New Token.** Name it for this repository (for example `OpenChuckCAM release workflow`), give it an expiry, tick the scope **publish-update** only, and restrict it to the crate pattern `openchuck-*`. Copy the token: crates.io shows it once.
3. In the GitHub repository: **Settings, Secrets and variables, Actions, New repository secret**, named `CARGO_REGISTRY_TOKEN`, holding the token. (It can be an environment secret of `crates-io` instead, so only that job can read it.)
4. Optional but recommended: **Settings, Environments, `crates-io`** (the first run creates it if it does not exist), add yourself as a **required reviewer**. The publish job then waits for your approval before it runs.

Renew the token before it expires and replace the secret. Revoke it on crates.io if it may have leaked.

## Each library release

1. On `main`, with the release work merged: the workspace version in `Cargo.toml` is the version to publish, and each crate's `CHANGELOG.md` has its entry. (`cargo-release` may bump and tag locally, without publishing.)
2. Push the tag `lib-v<version>`, for example `lib-v0.11.0`. The tag's version must be the workspace version, or the release gate fails. The tag push runs the gate and the publish dry run; it publishes nothing.
3. **Actions, Release, Run workflow:** choose the tag `lib-v<version>` (not a branch), **untick** `dry_run`, **tick** `publish_crates`. A run on a branch, with `dry_run` ticked or on an `app-v` tag, stops before the gate and publishes nothing.
4. The run passes the release gate and the publish dry run, waits for your approval if `crates-io` has a reviewer, and then publishes `openchuck-types`, `openchuck-gcode`, `openchuck-path` and `openchuck-post` in that order (the last three depend on `openchuck-types` and nothing else in the family). Cargo waits for each crate to reach the index before the next.
5. If the run stops part way (a network failure, an expired token), fix the cause and run it again the same way: a crate already on crates.io at that version is skipped.
6. Check the crates' pages on crates.io and their documentation on docs.rs.

Generated G-code is the user's to verify before running it on a machine (NFR-12). Publishing a crate does not change that, and nothing in a crate's page or README may say otherwise.
