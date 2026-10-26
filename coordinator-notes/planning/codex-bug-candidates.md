# Bugs that can go to Codex manually

Prepared by the task coordinator, 26 Oct 2026, against main `ba10400` (library 0.23.0). Source: the open issues labelled `awaiting-planning` on paulhampson/openchuckcam (12 when reviewed; the six STEPALL-0211 issues #346 to #351 were already moved to `queued`). Planning only: no lane is started and nothing is released. The coordinator hold is still in force.

When a Codex session finishes, it opens a **draft** pull request and the Merge Manager picks it up (decision 0055: draft first, push once).

## Summary

| Candidate | Issues | Separate from the work plan? | Model tier and reasoning | Start |
| --- | --- | --- | --- | --- |
| BUG-NEWPROJ | #338 | Yes | Standard tier, medium | Now |
| BUG-SIMEND | #334, #330 | Yes (one lane: same Simulate display code) | Top tier, high | Now |
| RAMPBUDGET | #343 | Yes, with one condition | Top tier, high (xhigh if the owner wants a proof-cost redesign) | Now, plan first |
| AROT (A accumulation) | #337 | Yes, but needs a design decision first | Top tier, high | Proposal phase only |
| STLPENCIL | #344 | Partly: shares pencil code with STEPALL-Q | Top tier, high | After STEPALL-Q, or fold into it |
| (folded into OUTPUT2) | #331, #332 | **No. Owner accepted the fold on 26 Oct 2026.** | Same lane as OUTPUT2 (Sonnet in my plan; consider Opus because of #332) | With OUTPUT2 |
| (no work) | #147 | Already built | none | Owner verifies |
| (not bugs) | #335, #336 | Enhancements | n/a | Not covered here |

Model note: I do not know which Codex model names your account offers, so the tiers are relative. "Top tier" means the strongest Codex model available. "Standard tier" means the default one. Reasoning levels are low, medium, high and xhigh. If you prefer Claude models for any of these, the equivalents are Opus for top tier and Sonnet for standard.

## The candidates

### BUG-NEWPROJ: #338, the New project window warns about unconfirmed changes

- **What:** typing a project name and pressing OK triggers the unconfirmed-field warning. In this dialog OK and Cancel are the explicit confirm and discard, so no warning is wanted.
- **Area:** `apps/openchuck-ui/src/shell/` (the new-project dialog and its tests, `new_project_tests.rs`). The form-departure guard is decision 0233 (BUG-EDIT, merged in #323); it has to keep working in every other form.
- **Why separate:** it touches only the New project dialog's use of the guard. No planned lane edits it. FOURSTEP changes the workflow steps, not this dialog.
- **Test first:** a requirement-based test that OK with a typed name creates the project with no warning, that Cancel discards with no warning, and that other forms still warn.
- **Model:** standard tier, medium reasoning. It is a small, well-bounded fix; the risk is weakening the guard elsewhere, which the tests cover.

### BUG-SIMEND: #334 and #330, Simulate display faults

- **#334:** skip to end in Simulate does not show the final cut part; the user has to step back and run some steps.
- **#330:** when cut-stock preparation fails (memory bound or a cut-surface job failure), the Deviation colours checkbox stays available but has no effect and does not say why. The Playback dock already shows the error. Related: #293, #300, decision 0238. Do not silently fall back to target-model triangles; that changes accepted semantics and needs an owner decision.
- **Area:** the Simulate display and playback code in `apps/openchuck-ui/` (skip control, stock bands, `update_bands`).
- **Why one lane:** both live in the same display and band code, so two lanes would conflict.
- **Why separate:** SIMUI (#328) and BUG-SIMSYNC (#341) are merged. FOURSTEP changes the workflow bar and the Post and Export entry point, not the playback dock. The agreed SIMUI boundary says the shared dock is untouched by other lanes, so the Codex session should read decision 0238 first and keep to it.
- **Test first:** reproduce #334 first (the cause is not known yet), then a regression. For #330, test both failure paths (checkpoint limit and surface job failure), current and stale states, and no retry loop.
- **Model:** top tier, high reasoning. #334 needs a root cause, and the playback state is easy to break.

### RAMPBUDGET: #343, back-and-forth ramp clearance budget

- **What:** the clearance proof can use up its fixed 100,000-check budget on deep entries (about 7,600 checks per mm at 0.01 mm tolerance and 3 degrees, so roughly 12 mm of descent or less), and it repeats costly drop queries on every pass. Found by CodeRabbit on #340; the design is decision 0242.
- **Area:** `crates/openchuck-path/src/assemble/entry.rs` and `entry_safety.rs`. This is the library family, so a breaking change bumps the workspace minor version with a changelog entry (decision 0048).
- **Why separate:** ROUGHPATH (#340) is merged. The STEPALL chain works on B-rep queries and drops, not on this entry proof. **One condition:** STEPALL-X1 (#348) changes exact face and trim queries in the same crate, so the Codex session should rebase on main before opening its PR and the owner should expect a possible semver-number clash with STEPALL-T. I assign no version; the Merge Manager resolves it in merge order, as before.
- **Constraints the issue sets (do not relax):** full-tool continuous sweep clearance on every leg; a lower start needs a sound stock-clearance proof; do not apply CodeRabbit's tolerance expression as written; keep cancellation, determinism at 1, 2 and N threads, bounded work, the one-diameter minimum and byte identity with the option off. Changing decision 0242's limits needs owner approval.
- **Deliverable:** failing cost and availability cases at and beyond the budget boundary, a plan for the optimisation, then the change.
- **Model:** top tier, high reasoning (xhigh only if the owner wants the proof redesigned from scratch). A wrong clearance proof is a collision risk.

### AROT: #337, continuous A values get excessive

- **What:** on helical strategies the accumulated A position can grow large enough to hit a machine limit.
- **Why a proposal first:** the fix is a design choice, not just a defect (for example, wrapping A at a post boundary, a rewind move between passes, or a limit check and warning). It touches path assembly and the post, so it can change G-code for existing jobs. It needs a numbered decision record and owner acceptance before any build.
- **Area:** `crates/openchuck-path` assembly and `openchuck-post`.
- **Why separate:** nothing in the work plan edits the rotary assembly now (ROUGHPATH is merged). It does overlap CUTOPTS and ROUGHPATH decisions 0241 and 0242, which the session must read.
- **Phase 1 (Codex):** measure the accumulation on the reference jobs and the blisk, list options with their effect on the Z1, and write the proposal. **Phase 2:** build only after the owner picks an option.
- **Model:** top tier, high reasoning for the proposal.

### STLPENCIL: #344, STL pencil shows no threading benefit

- **What:** in the carried-stock sequence, STL pencil clean-up runs 6.62 s at one thread and 6.60 s at four (about 1.004 times), while STEP pencil gains about 2 times. NFR-04.
- **Constraint:** profile the real carried-stock workload first; keep output byte-identical and keep progress and cancellation; do not relax tolerances or the benchmark.
- **Why partly separate:** it is in the pencil code that STEPALL-Q also edits (OI-950, OI-951). Running both at once risks conflicts. Either start it after STEPALL-Q merges, or fold it into STEPALL-Q.
- **Model:** top tier, high reasoning (profiling plus determinism).

### Folded into OUTPUT2 (accepted by the owner, 26 Oct 2026): #331 and #332

Both come from the #329 export review.
- **#331:** `OutputReport::text()` ends on a path, then `sentence()` adds a full stop, so Messages shows `Not attempted: /out/part-setup-sheet.pdf.`. Fix in `crates/openchuck-app/src/overview.rs`.
- **#332:** new destinations use create-new at the final path, so a crash during `write_all` can leave a partial file under the final name. The former writer staged every file before renaming. The recommendation is to restore staged no-clobber publication without weakening the no-overwrite rule.
- **Why not separate:** OUTPUT2 (#308 to #310) builds on the same export manifest, write path and `OutputReport`. A separate Codex lane would conflict with it and with FOURSTEP and OUTPUT3. #331 is a trivial change that OUTPUT2 should include. The owner decided #332 ships with OUTPUT2. OUTPUT2 closes #308, #309, #310, #331 and #332; #332 restores staged no-clobber publication per file, keeping the accepted no-overwrite rule.

### No work needed: #147

The import-dialog layout shift is listed in the work breakdown as built by lane IW (pull request 161, merged): "a spinner row that reserves its space". The issue is still open. The owner should confirm on the Windows build (OI-430 and OI-431) and close it.

### Not bugs, left out

#335 (machine library management) and #336 (clearance and tool length) are enhancements with no plan yet. #322 is lane FOURSTEP. #344 is above.

## Suggested order for you

1. Start BUG-NEWPROJ and BUG-SIMEND now. They are independent and do not use a schema number.
2. Start RAMPBUDGET with a planning phase first; ask me for the number range before it claims any decision or OI numbers.
3. Start AROT only as a proposal.
4. Hold STLPENCIL until STEPALL-Q or fold it in.
5. #331 and #332 are decided: they go into OUTPUT2.

## Rules each Codex session must follow

These apply to every lane (`CLAUDE.md` and `AGENTS.md`):
- Branch `codex/lane-<name>` from current main; open a **draft** PR and push once (decision 0055).
- Commit author and committer must be Paul Hampson (`528002+paulhampson@users.noreply.github.com`), with a Co-Authored-By line for the agent. A default agent identity fails the CLA check.
- Take decision, OI and project schema numbers from the next free ones across main and every unmerged branch; claim a schema number in the work breakdown first.
- Write requirement-based failing tests first, named by requirement ID; run them uninstrumented (`cargo nextest run`); check line coverage for the changed crates locally and run `cargo llvm-cov clean --workspace` afterwards.
- Before pushing: `cargo fmt --all`, `cargo clippy --workspace --all-targets`, the changed crates' tests, `cargo xtask deps`, `licences`, `records` and `trace`.
- Keep `docs/open-items.md` current; do not edit `docs/requirements.md` or `docs/architecture.md` to make a change fit; update any document the change affects in the same PR.
- Use "Closes #..." only when the work is fully done; otherwise "Refs". The owner closes issues after testing, and the `awaiting-fix-feedback` label marks them.
