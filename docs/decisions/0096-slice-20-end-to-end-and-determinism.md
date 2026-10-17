# 0096: Slice 20 items 3, 6 and 7 and the offline run: command-line end-to-end suite, determinism on the sample projects, undo, redo and cancellation (lane TE)

- **Status:** proposed (lane TE, 17 Oct 2026). Built on `claude/lane-te-end-to-end-and-determinism`. Only the project owner accepts a record (decision 0049).
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md). New: OI-310 to OI-315; OI-314 is closed by item 9 (owner's decision, relayed).

This is lane TE: work breakdown slice 20 items 3 (determinism at 1, 2 and N threads on all sample projects), 6 (sample projects, including the Z1 reference part, end to end) and 7 (command-line end-to-end scenarios in CI, including undo, redo and cancellation mid-job), and the slice 20 review check "The full run works with no network access" (NFR-09). It claims record **0096** and **OI-310 to OI-315** (from the block of records 0096 to 0100 and OI-310 to OI-319 the orchestrator assigned) and **no project schema number**. Requirements: GC-07, NFR-06, NFR-08, NFR-09, GC-11, UI-08, UI-09, with SIM-03 and SIM-05 for the simulation run. The only production-code change is the fix of item 9, in its own commit.

## Context

The documents ask for:

- **GC-07, NFR-06** (verification plan section 3 and 4.1): byte-identical G-code with 1, 2 and many threads, "on each platform"; every golden-file job at 1, 2 and the maximum thread count (section 5, "Repeatability").
- **NFR-09** (section 5, "Offline use"): the full system suite runs with the network blocked. Section 5 makes the command-line tool the end-to-end harness.
- **Slice 20 item 7:** command-line end-to-end scenarios, including undo, redo and cancellation mid-job.

Before this change, `main` already had the following:

- `crates/openchuck-app/tests/reference_jobs.rs` runs the eight generated reference jobs through the application API. It checks gates 1 and 2 and the golden programs for both posts, and the Makera Z1 output at 1 and 4 threads (the first run is at 2).
- `step_pipeline.rs` does the same for the STEP `drilling` job in the kernel job.
- `apps/openchuck-cli/tests/cli.rs` runs command-line scenarios, all on projects with no operations.
- Nothing ran with the network blocked.

Two gaps in the command-line tool shape the design:

- It has **no command that adds an operation.** It can make a project, import a model, set the machine, add a setup and add a tool, but not an operation.
- It has **no undo or redo.** Each run is one command, and the commands that edit save.

## Decision (proposed)

1. **The sample projects are the reference job set on the Makera Z1 profile** ([reference-jobs.md](../reference-jobs.md)): `groove`, `cone`, `sphere`, `lobed`, `cone_axial`, `cone_indexed`, `cone_axial_lead` and `cone_held_tabs`. No document defines "the Z1 reference part". These are the parts the gates and the air-cut plan run on the Z1, so they are taken to be it (OI-311). Their golden programs already exist, so no new test data is added.

2. **Built as a user would, with the command-line tool** (`apps/openchuck-cli/tests/e2e.rs`). The generated part is written as ASCII STL; shortest round-trip formatting keeps every vertex exact. Then `new`, `import`, `machine makera-z1`, `setup --new` (with the stock, the material, and the held regions and tabs for `cone_held_tabs`), and `tool --add` from a library folder holding the reference 3 mm ball. Only the operation is added through the application API (OI-310). The project this builds gives the committed golden programs byte for byte, so the command-line route and the API route of `reference_jobs.rs` agree.

3. **Item 6 and item 3: end to end at 1, 2 and N threads, both posts.** Each sample has its own module with two tests, `<sample>::req_gc_07_req_nfr_06_req_nfr_08_req_gc_11_runs_end_to_end_at_1_2_and_n_threads` and `<sample>::req_sim_03_req_sim_05_simulates_without_overcut`, so the test runner spreads them over cores and CI shares. `req_gc_07_every_reference_job_is_a_sample_project_with_its_tests` fails if a reference job with golden programs (other than `drilling`) has no sample. For each sample:
   - `validate`.
   - For each post, `openchuck-cli --threads T regenerate --post P --tool-numbers keep --out DIR`, with T in 1, 2 and N, where N is the larger of the core count and 4 (`openchuck_testkit::determinism::thread_counts`). Each run is a separate process, so no cache is shared. Each run must exit 0, report the replay matching the toolpath with no check finding (gates 1 and 2), print the NFR-12 notice and write one program.
   - The programs must be byte-identical across thread counts (GC-07, NFR-06) and equal the golden program, with the operation id replaced as in the golden files (NFR-08).
   - `resave` must leave every project file as it was.

   The Tests job of `ci.yml` runs the suite on Linux, Windows and macOS, which gives "on each platform".

   **Simulation** (the `simulates_without_overcut` test of each sample): `simulate` must exit 0, so there is no overcut beyond a cell and no rapid move through material.

4. **Item 7, cancellation mid-job, at both levels:**
   - **Command-line** (`req_ui_09_req_gc_11_an_interrupted_cli_run_writes_nothing_and_changes_nothing`). On `lobed`, `regenerate --out` at 1 thread is interrupted on an observable event, never on a timer. The tool writes the tool numbers to standard error after it has opened the project and immediately before it generates (GC-14), and the test interrupts as soon as it reads that line. The interrupt is SIGINT, as Ctrl-C sends, on Unix; on Windows the process is ended, which is just as deterministic. About 9 s of work (model loading and generation, debug build) remain after that line, against a signal that lands within milliseconds. The run must fail, write no G-code file and leave every project file byte-identical. The next run must give the golden program. It passed 5 runs in 5 in the lane TE session. It runs on all three platforms.
   - **Application** (`req_ui_09_a_regeneration_cancelled_mid_job_leaves_the_next_run_identical`). On `groove`, `JobHandle::cancel` after the job's first progress report (an event, not a timer) must give an error that `is_cancelled`. The next regeneration must give the golden program, so a cancelled job leaves nothing in the cache. It passed 10 runs in 10.

5. **Item 7, undo and redo, through the application API in the same suite.** Slice 20 item 7's "undo and redo" is met through the application API, because the command-line tool has no undo or redo commands. None is added: no requirement asks for one, and the owner may decide later (OI-313). The test is `req_ui_08_undo_and_redo_on_a_sample_project_restore_the_g_code_and_the_files`, on `cone`:
   - A parameter edit changes the G-code.
   - Undo gives back the original G-code, and once saved the original files byte for byte.
   - Redo gives the edited G-code.
   - Undo again and save, and a fresh `openchuck-cli` process regenerates the golden program.

   The last step, in a fresh `openchuck-cli` process, is the command-line part of the scenario. The property tests of `crates/openchuck-app/tests/undo.rs` stay the unit-level check of UI-08.

6. **NFR-09: the suite with the network unavailable.** New workflow `.github/workflows/e2e.yml`, job "CLI end to end, network unavailable (NFR-09)", on Ubuntu:
   - It fetches and builds the `openchuck-cli` tests with the network up.
   - A canary must fail to reach `github.com` from inside `sudo unshare --net`.
   - The whole `openchuck-cli` suite (`cli.rs` and `e2e.rs`) then runs inside a new network namespace, as the runner user, with `cargo nextest run --offline`. The namespace has only a loopback, and it is down.

   Like the heavy jobs of `ci.yml`, the job skips drafts (decision 0052) and runs only when the workspace, posts or test data change. Making it a required check is the owner's action in the repository settings (OI-315). The same run (48 tests) passed in the lane TE session with `unshare --net`.

7. **Long-running tests and CI cost.** Timed uninstrumented with `cargo nextest run` on a 4-core Linux machine, debug build (17 Oct 2026). Times are alone on one test thread, then in a full `cargo nextest run -p openchuck-cli`:
   - `lobed` end to end: 103 s, then 117 s.
   - `lobed` simulation: 55 s, then 63 s.
   - `cone_held_tabs` end to end: 41 s, then 57 s.
   - Interrupted run: 36 s, then 43 s.
   - The other six samples' end-to-end tests: 12 to 13 s each.
   - The other simulations: 6 to 9 s each.
   - Undo and redo: 6 s. Cancel: 4 s. Sample list: under 0.1 s.

   The new tests take 379 s of test-thread time in all. The whole `openchuck-cli` suite takes 158 s of wall time on 4 cores, against a few seconds before. The two `lobed` tests are over about a minute, so they are listed in both profiles of `.config/nextest.toml`. No test takes over 10 minutes.

   Wall time added to the Tests job of `ci.yml`, estimated, not measured: the job hashes tests over two shares on Linux and Windows and one on macOS. The added wall time is about the new tests' 379 s divided by the runner's cores (4 on hosted Linux and Windows, 3 on macOS), bounded below by the 117 s `lobed` test, so about 2 to 3 minutes per platform. Windows process start-up and file writes are slower, so allow more there. A draft runs no Tests job, so the first ready run of the pull request gives the measured figures.

8. **What is not covered here** (OI-312):
   - The STEP `drilling` job needs the kernel. Its determinism and golden checks stay in `step_pipeline.rs` in the STEP kernel job: first run at 2 threads, then 1 and 4, with the Makera Z1 post.
   - Swarf has no sample project on `main` (its application wiring is not merged).
   - Rotary roughing, rest machining, pencil and edge following have no reference job with golden programs.

   Each joins the suite when its reference job exists.

9. **Fix: stock exactly as wide as the part encloses it (OI-314; owner's decision, option B, 17 Oct 2026, relayed by the orchestrator session, decision 0049).**
   - **The defect.** `stock_encloses` in `crates/openchuck-types/src/setup.rs` compared the part's extent with the stock's exactly. So `setup` warned "some of the part lies outside the stock" for `cone_held_tabs`, whose stock is exactly as wide as the part: the mesh radius is 10.000000000000002 against a stock radius of 10. Measured over the testkit parts, using the same largest-vertex radius as `openchuck_geom::part_extent`, a part exactly as wide as its stock measures outside it by:
     - double-precision coordinates (ASCII STL, STEP): 1.8e-15 mm at radius 10, 2.8e-14 mm at radius 150;
     - coordinates rounded to 32-bit floats, as a binary STL stores them: 3.0e-7 to 4.6e-7 mm at radius 10 to 12, 1.5e-6 mm at radius 50, 6.5e-6 mm at radius 150, about 4.5e-8 × the radius.

     The library's existing 1e-9 mm (`LENGTH_EPSILON`) covers the first case but not the second.
   - **The change.** Every comparison in the function (both X ends, the cylinder radius, the box Y and Z) is one call to `at_most(a, b)`, which tests `a <= b + ENCLOSE_TOLERANCE * max(|a|, |b|, 1)`. The new private constant `ENCLOSE_TOLERANCE = f32::EPSILON as f64` (2^-23, about 1.19e-7) is a relative tolerance, never applied to less than 1 mm of size. It is about 2.6 times the measured worst case: 1.2 µm at 10 mm, 18 µm at 150 mm. Its comment gives the reason and the measurements.
   - **API.** The constant and `at_most` are private, as nothing else needs them. The public API is unchanged: `cargo semver-checks -p openchuck-types --baseline-rev origin/main` reports no update required. This is a non-breaking behaviour change, with no version bump and no changelog entry.
   - **Tests.** `req_set_03_req_nfr_05_stock_as_wide_as_the_part_encloses_it` in `setup.rs` checks four parts: radius 10 and radius 150, each with double-precision and with 32-bit coordinates.
     - Stock exactly as wide as the part, cylinder or box, gives no warning.
     - Cylinder stock 0.01 mm narrower warns, and so does box stock 0.01 mm short along X.
     - A real overhang of 1e-4 mm on a 10 mm part warns.

     Without the tolerance the test fails, which reproduces the defect. The end-to-end suite now requires every sample, `cone_held_tabs` included, to report no setup problem.

## Consequences

- One sample list feeds determinism, the golden comparison, simulation, cancellation and undo. A new reference job is added in `reference_jobs.rs` (its golden programs) and in `e2e.rs` (the command-line route).
- The suite found one defect: a false setup warning when the stock is exactly as wide as the part (OI-314). It is fixed by item 9.
- Slice 20 review check "The full run works with no network access" is ticked for the command-line suite on Linux. The UI is not run offline here: it has no network code, and `cargo xtask deps` refuses network crates on every platform.
