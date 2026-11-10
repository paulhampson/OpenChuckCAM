# 0551: The held region's own end is not named as a shoulder (SET-09, OP-04, OP-05, OP-09, NFR-12; issue #765; lane BALUSTER-WARN)

- **Status:** accepted by the project owner on 10 Nov 2026, relayed by the coordinator (decision 0049). Proposed by lane BALUSTER-WARN the same day; the owner had decided the fix itself on 10 Nov 2026 (issue #765, an owner-approved exception to the release freeze), relayed the same way. The number is provisional: the Merge Manager assigns the final number at release.
- **Date:** 10 Nov 2026.
- **Found by:** the owner, on the release build, generating the public tutorial project (`testdata/examples/baluster`); investigated by lane BALUSTER-WARN.
- **What it amends:** none. It carries out [0343](0343-end-faces-beside-a-short-tab.md) item 6 as written ("A held region keeps the tool from a face": a face of the part), which the code applied to every face the search found.
- **Builds on:** [0035](0035-workholding-and-retaining-tabs.md) items 1 and 2 (the held region is part of the target), [0161](0161-tools-keep-clear-of-the-gripped-length.md) items 3 and 7 (the keep-out and its warning), [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (turns at faces across the axis), [0344](0344-finishing-stepdown-and-free-ends.md) (free ends leave out the held region's own end), [0536](0536-message-levels-operation-reference-and-setting-links.md) step 2 (material left is a warning).
- **Open items:** OI-2140 (the owner's acceptance of this record), new, closed on 10 Nov 2026 with the acceptance.

## Context

On the tutorial's baluster (stock held 8 mm at the start, X -1 to 7; Finish is helical finishing with a 3.175 mm ball end), Finish carried two warnings:

1. "the tool does not turn at a step across the rotary axis (a shoulder facing along X) at X 8.589: there it would reach over the gripped length of the stock, so that shoulder keeps what earlier operations left";
2. "nothing within one tool radius of the held region at the start of the stock is cut: no part of the tool reaches over the gripped length of the stock".

X 8.589 is 7 + 1.5875 + 0.001: the turn one tool radius (and a tenth of the tolerance) from a step down at X 7, the end face of the held bar. The part there is a plain tenon of radius 7; its first shoulder is at X 10, which a ball end of this radius rolls over without a jump. With grips of 2, 4, 6 and 10 mm the warning moved to X 2.589, 4.589, 6.589 and 10.589, and with the part 3 mm clear of the grip it still appeared, at X -1.411.

**The cause.** The face search (`faces::find`) drops the tool on the target, which holds the held region's solid (0035). The held cylinder's end face is therefore found as a face across the axis, and it always lies inside the keep-out interval (0161), so every such turn went to the list that 0343 item 6's warning names. The free-end search in the same function already leaves the held region's own end out (0344). The warning became a warning, and the operation and project marks with it, in #625 (0536 step 2); before that it was information.

Warning 2 is correct by design (0161 items 3 and 7, SET-09): the generated paths keep the tip at X 9.088 or beyond, and the simulation shows up to 0.92 mm left at the foot of the shoulder at X 10.

## Decision

1. **A face named by 0343 item 6 is a face of the part.** A turn that a keep-out interval stops is named only where the part itself makes the face: on a line of A the face was found on, the tool dropped on the part alone (`SurfaceSource::drop_on_part`), two tenths of the tolerance across the jump from the turn, stands as high as the face's top, less the tolerance.
2. **The held region's own end face, and a tab's, are not named.** They are never cut (SET-09, OP-21); what the keep-out leaves beside a held region is said by the held region's own warning (0161 item 3), which is unchanged.
3. **Nothing else changes.** No turn is added or removed, so every path and program is byte-identical; only the warning list differs. Helical finishing, the simultaneous spiral, rest machining and axial finishing all take the list from `faces::find`, so all four follow.

## Options not chosen

- **Compare the face's X with the held intervals' inner ends.** It needs the raw held intervals in the strategy input (only the widened keep-out is there) and the reach used to widen them, and it would still name a tab's end face. The drop on the part asks the question directly, as the free-end search does.
- **Drop the warning altogether.** A real face of the part inside the keep-out (0343's case: the part's end face beside a short tab under a held region) keeps material, and the user should be told.

## Consequences

- The tutorial's Finish shows only warning 2. The project still reads "Needs attention": warning 2 stays, and so does the setup's warning that the grip covers 7 mm of the part (expected, as the tutorial's "Hold the work" page says).
- A face of the part that the held region's own end shadows is no longer named either. The keep-out zone it lies in is covered by warning 2.
- Figures that show the old warning, or a warnings count that includes it, are retaken by the release recapture (lane SHOTS), not here.

## Verification

### As built (lane BALUSTER-WARN, 10 Nov 2026)

On branch `lane-baluster-warn/held-end-face`, from `main` `7045731`. Tests, each written before the code and watched to fail (both failed with the X 8.589 warning), run uninstrumented with `cargo nextest run`:

- `crates/openchuck-path/tests/helical_faces.rs`: `req_set_09_req_op_04_the_held_regions_own_end_is_not_named_as_a_shoulder`. A tenon of radius 7 to X 10 and a collar of radius 8, joined to a held cylinder of radius 12 from X -1 to 7, with the application's keep-out. For a ball end and a flat end: no "does not turn at" warning, and the held region's warning is present. About 20 s.
- `crates/openchuck-app/tests/held_region.rs`: `req_set_09_req_op_04_the_baluster_finish_names_no_shoulder_at_the_held_end`. The public baluster example, Finish only: no "does not turn at" warning, and "Nothing within one tool radius of the held region" is present. About 28 s.
- `helical_faces::req_set_09_req_op_04_a_face_the_held_region_keeps_the_tool_from_is_named` (0343's case, a face of the part inside the keep-out) still passes, warning at X 31.589.
- **Output unchanged.** `openchuck-cli regenerate` of the baluster with `--post makera-z1` gives a G-code file byte-identical to `main`'s (SHA-256 `4394e8cf4856527a6da142e57c86172e70170d3715262e7ce9cbfde627253cfb`), and the standard error now matches `user-docs/src/reference/command-line.md`, updated here.

- **Suites.** `cargo nextest run -p openchuck-path -p openchuck-app` on `7045731` with the change: 1,797 tests, all passed (2 skipped). The two new tests took 29 s and 33 s under the full parallel load; neither is long-running. `cargo fmt --all`, `cargo clippy --workspace --all-targets` (no warnings) and `cargo xtask deps`, `licences`, `records` and `trace` pass, and `records` again after rebasing on `main` `c57555bd` (#763, registers only).
- **Coverage.** `cargo llvm-cov nextest --profile coverage -p openchuck-path` on the face, free-end and held-region tests: `faces.rs` 95.27 % of lines. Every new line runs; the one part not reached is the rising-face side (`t.up`) of the "just across the jump" X, a part face rising into a held region at the far end, which no requirement test builds yet.
- **API.** Only `pub(crate)` code changes, so the library's public API is unchanged and no version bump is needed.

Not done: nothing was run on a machine (NFR-12).
