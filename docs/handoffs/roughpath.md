# ROUGHPATH handoff — paused by owner, 24 October 2026

## Workspace and authorization

- Repository `/workspace/OpenChuckCAM`, branch `codex/lane-roughpath-unwind-and-ramp`, base `1a90435654c734c05e4fa3f110c4a7f3f8e5859f`. Resume from this branch's handoff checkpoint commit.
- The owner explicitly requested pushing the handoff on this branch after pausing work. This checkpoint includes the unfinished implementation, accepted records and this handoff so a fresh checkout can resume it. **It is not ready for merge and required validation is incomplete.** No ROUGHPATH PR exists. Continue this branch rather than replacing it. No validation processes remain running. The owner's explicit checkpoint-push instruction authorizes this push before the original completion gate; it does not waive the remaining implementation/validation gates.
- Fetched main is `049c70a`; it has NOT been merged. Follow accepted 0055 and Merge Manager release instructions before updating the branch from main.
- Dependencies already in base: OPCREATE #271 `adb07bf1a7abc0a209ea82c4656cbe9484eff204`, CUTOPTS #277 `ba829c1677c74bf90aec61c39aba8201cd5f5350`, SWARF3 #320 `e590f4de67bbde573edf30982982d6d0c5d92707`.
- Decisions 0238 and 0239 were shown completely and **explicitly accepted directly by the owner**. Their local records/index say accepted. One-diameter minimum and refusal wording also accepted beforehand. No owner approval is pending. No schema/OI number claimed; numbers provisional until merge.
- ROUGHPATH owns existing roughing/layers and goes first on shared overlaps. FACING owns new facing planning/coverage/targets/completion; FINSTEP owns adaptive finishing. They consume these interfaces only after merge into main, no cherry-picks or unmerged prerequisites. Shared inventory is in PR320 comment6385263681; accepted0239 follow-up comment6385414469. The initial inventory is comment6383427410.

## Implemented locally

### #292 / accepted0238

- `openchuck-path/src/assemble.rs` retains accumulated A after safe retract; cuts remain unwrapped.
- New public `program.rs`: `ConnectionOptions`, `ConnectedGroup`, `connect_group`. Whole-group constant turn offset, positive-machine-A ties, complete-group A limits, explicit required orientation, cancellation. Explicit timed incoming link. Conservative enclosing-cylinder half-space proves full supported vertical-tool clearance. Linear travel checks apply to new connection/boundaries, not a newly invented cut validator.
- App `connections.rs` supplies whole-part bounds, original stock and protected held/tab geometry. `Regeneration::canonical_toolpath` retains independent paths; simulation/playback use connected groups. Posting and file planning reconnect each exported file from canonical groups. `Application::planned_files` now takes the regeneration; UI call updated.
- `PostJob`/`ConvertOptions` require `a_range` and work-frame `safe_z`. `Step::RotaryBoundary` replaces `Unwind`; a physical full-turn rapid remains physical. Rotate-only posts omit the nonmoving marker. Z1 resets supported safe continuous-axis boundaries, tracking all later coordinates. Replay tracks reset offsets explicitly and catches full-turn errors.
- Fixed the existing ±360 mismatch: firmware does NOT reset exactly ±360. G92.4 A0 S0 changes live numeric A by whole turns, without motion, XYZ reset or saved-work-offset changes. Existing firmware1.1.2 evidence and air-cut record0007 support this; no new hardware trial.
- Library family bumped to 0.21.0; changelogs/migration notes started. Accepted narrow legacy Facing boundary-output exception; planning/defaults/size/equal-depth layers stay unchanged.

### #291 / accepted0239 — in progress

- Added common `ramp_back_and_forth` flag, false by default; `EntryParameters.back_and_forth`; form grouping/label identifies ramp-only option.
- Added `StrategyInput.entry_safety`; literals updated. New public `entry_safety.rs::EntrySafety` separates whole mesh target, protected solids, complete body and machine constraints.
- Safety verifier checks fixed A/vertical orientation/convex body/travel; protected geometry uses conservative full swept body bounds. Whole-target proof grows the body by a ball enclosing each continuous subsegment; midpoint query covers the entire subsegment, not only samples. Shared 100,000 check budget and cancellation.
- Shared `assemble/ramp.rs` plans descending reversals, ≤configured slope, partial final leg, 10,000-leg bound, inclusive one-diameter minimum and cancellation.
- `assemble/entry.rs::repeated_ramp` is wired: proves one-diameter corridor at target depth, plans legs, checks each and existing gouge checks, checks approach/final feed connection, then emits at plunge feed. A stays fixed. Missing safety/direction refuses rather than plunging. Approach uses conservative stock radius. Corridor failure reports the approved wording and zero verified length; cancellation is preserved.
- App operation generation supplies whole STL target, protected solids and extended cutter/shank body through overall length. **Current limitation:** BRep has no certified exact-target safety input and refuses optional mode; a shank larger than the cutting diameter also refuses. Do not treat display tessellation as an exact STEP certificate.

## Remaining work, priority order

1. Review the new ramp implementation against0239 before broadening it. Strengthen integration tests: every-leg slope/feed/final-depth/connection assertions, later-leg obstacle, unsafe reversal, shank collision, missing direction/safety, minimum boundaries, budget/no-progress and cancellation. Ensure original target/refusal checks cover approach and final connection as well as descending legs; current `.gouge` loop is on descending legs, while full safety verifier covers all motions.
2. Add application end-to-end **option-on** tests for roughing and other applicable shared-entry strategies. Exercise complete-body/protected-target inputs and honest unsupported-format refusals. Current application test covers off/absent identity only.
3. Re-run the historical blisk at3 degrees with option on, using the exported owner data below. Report actual generation or specific refusal; do not promise it fits. Bound diagnostics to600 seconds and stop/raise an individual uninstrumented test over ten minutes.
4. Verify determinism at1/2/available-N threads for new ramp and connected/cached/file-split rotary behavior, tool changes, continuous/limited axes. Strengthen simulation/playback/post agreement tests and inspect link segment indices in cached simulation notes/reports.
5. Finish specs/UI wording/WBS/register/changelogs and migration documentation for all new APIs. Audit intentional goldens; preserve cutting geometry and FACING contracts (coverage/target/protection distinct, extension semantics unchanged, incomplete coverage exposed, no warn-only clearance proof).
6. Re-run final tests, long profile, changed-crate own-test coverage, formatting, clippy and xtask after ramp changes. Review uncovered changed lines and clean coverage artifacts. Do not report earlier passes as final passes for newer code.
7. Follow AGENTS human author/committer identity Paul Hampson (GitHub528002) and agent co-author credit. When complete, commit/push once and create/reuse a **DRAFT ROUGHPATH PR** for #292/#291. Coordinate with Merge Manager through PR comments; never mark ready, merge or enable auto-merge. Its merge is the shared milestone unblocking FACING.

## Evidence and test status

Selected verification logs are committed alongside this handoff in `roughpath-evidence/`. The original workspace also has `/workspace/roughpath-evidence/`, including the historical fixture copies and chronological `investigation.txt`; those local paths will not exist in a fresh checkout. Recreate the fixture from the historical branch reference below, using the documented separate migration and diagnostic spindle setting. Historical model files and generated full G-code are not duplicated in this checkpoint.

- Historical data exported from `origin/claude/upbeat-davinci-jk46yd` at `c92fa236ccc8862e5a52e80eade69c73b835d7e0`, under `docs/trial-feedback-projects/` in evidence directory. Original saved entry is plunge. STL SHA25648351f42635f05eea09aaf77dd685e7b2bd564cd087b4a336fb29554a7e33f37. Separately migrated diagnostic copies add spindle13000 purely to permit posting; not a machining recommendation. `blisk_ramp_3` sets explicit3degree ramp and originally refused pass1563 after263.514s, run37.506mm with stock0.078mm above rapid stop. Option-on has NOT been reproduced yet.
- Fixed #292 blisk run:89.733s,84,855segments, safe retract retainsA593275. Removed1,648-turn/9,888s playback return. Makera differs only time comment; LinuxCNC removes terminalA-5 plus time comment. Cutting lines unchanged, both replay reports clean. `fixed-plunge.log`, `fixed-output/`, original `plunge-posted.log`, `plunge-output/`.
- Tests failed first before motion fixes. Broad uninstrumented nextest run:853tests in579.094s (652.001s withbuild),848passed,5failed,12profile-skipped. Five failures were fixed through explicit-link expectations, accepted golden changes and corrected extension-test stock fixture. Targeted reruns passed; final combined rerun remains. Five existing tests >60s were added to coverage exclusions/long selection, timings62.697/96.301/97.045/80.216/76.749s. No individual test exceeded10minutes.
- Rotary unit/integration logs: `nextest-short.log`, `recheck.log`, `travel-recheck.log`. Reference golden test passed43.68s. Updated helical golden and last-cut-angle expectation passed.
- Post own-test coverage:30tests pass; changed convert/render files86.34%/96.40% line coverage. Saved summary `coverage-post.log`; more changed-line inspection/coverage remains. Coverage artifacts cleaned.
- `xtask-small.log`: ALL xtask checks passed,99/99 P1 traceability, **before latest ramp/API wiring**.
- New sweep tests: failed first (`sweep-before.log`), then2passed .01s (`sweep-after.log`); path clippy passed after fixing test-helper lint allowances.
- Ramp integration: failed first (mode ignored, expected repeated descent), then passed .98s; full entry suite11tests passed .85s and sweep2tests passed .01s (`ramp-tests.log`). Smaller36-sided integration fixture now used, still verifies fixed-plane geometry.
- Most recent result: application absent-vs-false test passed on BOTH shipped posts, byte-identical toolpaths/G-code, clean replay reports, .55s (`ramp-off.log`). Most recent path+app all-target clippy passed (`ramp-clippy.log`,8.15s). No process remains active.

## Tooling

Read AGENTS/docs/decisions before changes. `source /workspace/.tooling/openchuck-env.sh` supplies cargo/rustup paths and4build jobs. To avoid exhausting32GB disk, consistently use:

    CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo ...

Nextest0.9.149, llvm-cov0.9.1 and llvm-tools are installed. Full debug UI/test builds previously filled disk; `cargo clean` recovered29GB and smaller artifacts solved it. Coverage cleanup remains required after each crate. Network tool commands require additional network permission; inherit proxy/CA, no bypass. Cloud-environment skill was read; policy restricted/enforced, noVPN. No blocker-monitor automation is active; earlier watch was paused once dependencies cleared. If genuinely blocked later, use hourly monitoring if available or state unavailable.
