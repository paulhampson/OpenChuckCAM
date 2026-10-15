# 0039: Cache the OpenCASCADE install on its own

- **Status:** proposed (15 Oct 2026). Built on branch `claude/occt-install-cache` at the owner's request; the first CI runs on the branch and on `main` are the check, and the record becomes accepted when they show the hit rate in item 6.
- **Date:** 15 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-169 is new (first written as 0034 and OI-150, then 0035 and OI-153 as lanes H and E took those; 0039 and OI-169 are the next free across `main` and the unmerged branches on 15 Oct 2026). This amends the CI route of [decision 0027](0027-slice-11a-step-import.md) item 9 (closed OI-20) and the cache advice of [decision 0003](0003-step-kernel-route.md); it changes no requirement and no crate. Lane D (kernel CI); no project schema number.

## Context

The STEP kernel jobs build OpenCASCADE 7.8.1 through `occt-sys` (decision 0003), 23 to 35 minutes cold. Decision 0027 item 9 cached it with `Swatinem/rust-cache` inside the cargo target directory. The run of `main` at 44be8c4 (15 Oct 2026, run 38773408074) shows the cache did not help: the Linux job logged `No cache found` and spent 23 min 30 s in `occt-sys`; macOS took 23 min and Windows 35 min.

Reasons found in the logs and in the `occt-sys` and `rust-cache` sources (the log shows the first; the others are read from the code and the workflow files, and not yet confirmed by a run):

1. The `rust-cache` key hashes `Cargo.lock` and every `Cargo.toml` (the log lists them all), though the OpenCASCADE build depends on none of them. A version bump or a dependency change anywhere in the workspace starts the kernel from scratch.
2. `ci.yml`, `nightly.yml` (mutation run, sanitizer) and `nightly-build.yml` each keep their own cache, per platform, so OpenCASCADE is built and stored up to four times.
3. A cache written on a pull request is readable by that pull request only, the kernel job is path-filtered so `main` may not refresh it, and GitHub drops caches unused for 7 days.

`occt-sys` always builds OpenCASCADE in Release into `<target>/OCCT`, whatever the cargo profile. `crates/openchuck-import/build.rs` already accepts an installation through `OCCT_ROOT` and skips `occt-sys` and CMake.

## Decision (proposed)

1. **Cache the install, not the target directory.** The composite action `.github/actions/occt` restores `occt-install/` (`lib/` and `include/`, about the 260 MB of static libraries of decision 0003) with `actions/cache`, under the key `occt-<occt-sys version>-<OS>-<arch>-<variant>-r<revision>`. The version is read from the `occt-sys` pin in `Cargo.toml`, so a bump of the pin is a new key and nothing else is. A hit exports `OCCT_ROOT`.
2. **A miss builds once and saves at once.** The action runs a cargo build of `openchuck-import` with the `kernel` feature (`--release`, so the cached copy is the optimised build), copies the install out, deletes the copy in the target directory (the install is not stored twice), and saves the cache in the same job, before any later step can fail. It then exports `OCCT_ROOT` as on a hit, so the job's own cargo steps rebuild only the wrapper.
3. **One cache for the plain jobs.** `ci.yml` (`step-kernel`), `nightly.yml` (`step-mutations`) and `nightly-build.yml` share the `plain` variant. The AddressSanitizer job sets `CXXFLAGS=-fsanitize=address`, which the `cmake` crate passes to OpenCASCADE, so it has the `asan` variant and builds with its own command (`cargo +nightly build -Zbuild-std --target ...`).
4. **Seeded and kept alive from `main`.** `.github/workflows/occt-cache.yml` runs the action on the three platforms every day, on a push to `main` that changes the action or the `occt-sys` pin, and by hand. Running on `main` writes the cache every pull request can read, and the daily read keeps it inside the 7-day limit. It also runs on a pull request that changes the action or the workflow, writing a cache for that pull request only.
5. **Escape hatch.** The action's `revision` input (default 1) discards every cached install, for a runner toolchain change that breaks linking against old libraries. The key does not include the runner image version, which changes weekly: static libraries built by an older compiler link with a newer one, and a rebuild each week is what this record removes.
6. **`rust-cache` stays** for the Rust dependencies of the same jobs. They no longer carry OpenCASCADE, so their caches shrink by about the 280 MB seen in the log.

## What was checked

- On Linux, the cold build, the copy of the install and a rebuild and test run with only `OCCT_ROOT` set ("Local check" below): 24 min against 17 s.
- The workflow and action files parse as YAML. `actionlint` is not installed in the session, so the expressions are unchecked beyond that.
- Not run: the Windows and macOS paths (`/usr/bin/find`, `cp -R`, the `OCCT_ROOT` path with backslashes on Windows) and the cache save and restore themselves, which only a runner can do.

## Local check

On Linux (the session container, 15 Oct 2026), following the action's steps by hand:

1. `cargo build --release --locked -p openchuck-import --features kernel --lib` with a fresh target directory: 24 min 10 s.
2. Copy `OCCT/lib` and `OCCT/include` out (301 MB uncompressed) and delete the target directory.
3. `cargo test --locked -p openchuck-import --features kernel` in a new target directory with only `OCCT_ROOT` set: it compiled the wrapper and the tests and passed all 44 tests in 16.7 s. `occt-sys` is still compiled as a build dependency (a few seconds) but no CMake build runs.

So the `OCCT_ROOT` route, unused by any CI job until now, links and runs the importer correctly. The release build of OpenCASCADE is what the plain cache will hold; the dev-profile jobs link the optimised libraries, which is faster and not a change in behaviour.

## Consequences

- A kernel job whose key hits starts its cargo build in about a minute instead of 23 to 35 minutes (an expectation, to be measured; item 6 of the open item).
- Any run that changes the `occt-sys` pin, the revision, or the action's key builds OpenCASCADE once per platform, on the first run that misses.
- The first pull request after this one merges, and any pull request before `occt-cache.yml` has run on `main`, may still build cold. Run the workflow by hand after the merge to seed `main`'s cache.
- GitHub's 10 GB cache limit is used less: one install per platform and variant replaces four target directories that churned with every manifest change.

## Alternatives considered

- **Keep `rust-cache` and loosen its key** (`cache-directories`, a fixed `key`): its key still hashes the manifests, and the OpenCASCADE sources are unpacked with new timestamps each run, so CMake may rebuild a restored build directory.
- **Prebuilt libraries published as release assets:** no eviction and usable for local builds, but it adds a publishing workflow and a download that must be checksummed. Kept as the next step if the hit rate in OI-169 is poor.
- **`sccache` around the C++ compiler:** needs compiler wrappers inside the `cmake` crate and is awkward with MSVC.
