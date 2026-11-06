# 0528: An Apply of the workholding section is stored when its check finishes, wherever the user is, and Messages says while a draft is not applied (UI-11, NFR-12; issue #558; lane STALETAB)

- **Status:** proposed by lane STALETAB on 6 Nov 2026. Guidance only until the owner accepts it (decision 0049).
- **Date:** 6 Nov 2026.
- **Number:** 0528, with OI-2059 and OI-2060, given to the lane by the coordinator.
- **Issue:** [#558](https://github.com/paulhampson/OpenChuckCAM/issues/558) (sev1). Related: decisions [0172](0172-stale-state-and-removed-paths.md), [0192](0192-simulate-result-out-of-date.md), [0523](0523-tab-checks-follow-the-operations-reach.md).
- **Amends:** nothing in the requirements or the architecture. It adds a message to interface specification section 11.7 (the workholding section) and to Messages, once accepted.
- **Open items:** OI-2059 and OI-2060 new. No project schema change and no library change: `openchuck-ui` only, plus tests in `openchuck-app`.

## Context

The owner shortened the tab at the free (unheld) end of his CrankShaft project and pressed Apply. The operations did not go stale and were not generated again. The view showed the new tab beside a toolpath made for the old one, with no "out of date" mark (NFR-12).

**The key and the stale test are complete.** The diagnosis found no field missing from the pipeline key (decision 0030):

- `cache::operation_key` hashes the whole stored setup except its name and its place in the list. That covers every held region and every tab field, the stock and the frame. It also hashes the machine, the tool, the simulation settings, the model and the selection sets.
- The tree state (0172), the Simulate result's out-of-date test (0192) and the export's stale check (GC-15) all read that key.
- On the public stand-ins (the cone of `tab_scope.rs`, and the CrankShaft with default tabs), shortening the free-end tab makes every operation stale, and the path differs when it is generated again. `crates/openchuck-app/tests/stale_setup_edits.rs` now holds this for every setup, workholding, machine, tool and simulation edit.

**The fault is in the workholding section.** The section edits a draft. Apply starts a check of the draft against the part and the tools as a job (11.7), and stores the draft when the check finds nothing refusing. Two things went wrong:

- **The result was taken only while the section was drawn** (`WorkholdingPanel::show`). Apply followed by a move to an operation, or by closing the section, before the check finished stored nothing. The draft was stored later, without notice, when the section was next shown.
- **Meanwhile the view drew the draft** in place of the stored workholding, whatever was selected (7.5, OI-334 k). Generation, Simulate and export read the stored setup, so the operations were rightly up to date for it. The only signs were the view's "Showing unapplied workholding" chip and the setup section's "Not applied" chip.

A shell test reproduces it: Apply, select an operation, let the check finish, and the tab is never stored.

The owner confirmed he pressed Apply. His other answers (whether the tab was automatic, whether a "Not applied" chip showed, whether he left the section before Apply's spinner finished, and his build) were not known when this was written: OI-2060.

## Decision

1. **The shell takes the check's result every frame.** `WorkholdingPanel::poll_apply` is called by the shell after the inspector, whether the section is drawn or not. An Apply whose check finds nothing refusing is stored as soon as the check finishes, as one undoable edit (UI-08), and the operations go stale in the same frame (UI-11). The section still takes the result first while it is drawn, and a result is taken only once.
2. **The section follows the stored setup when it is not drawn.** `WorkholdingPanel::follow_project` runs every frame, as it does when the section is drawn. A draft that has been stored stops counting as unapplied at once, so the view stops calling it unapplied. An undo while the section is hidden replaces the draft as it would when shown.
3. **Messages says while a draft is not applied.** While the workholding section of a setup holds changes that are not applied, including while Apply's check runs, Messages carries a warning on that setup's workholding:

   > The workholding of '<setup>' has changes that are not applied. The view shows them, but the toolpaths, the simulation and the G-code use the held regions and tabs stored in the setup: apply or revert the changes in its workholding section.

   Its action opens the section. The warning goes when the draft is applied or reverted. It does not change any item's state: the operations are up to date for what is stored, which is what they generate and export from (0172).
4. **Not changed.**
   - The pipeline key and the stale rules of UI-11, 0172 and 0192.
   - Generated output (GC-07, NFR-06).
   - Apply's refusals and warnings (0523).
   - The project file.

## Consequences

- An Apply the user pressed can no longer wait unseen until the section is shown again.
- A hidden check that fails (for example "The part, stock or tools changed while checking; check again", or a refusing finding) still stores nothing. Its message waits in the section, and the Messages warning stays meanwhile.
- **Choosing another setup still drops the section and its draft.** That includes an Apply whose check is still running, with no notice: OI-2059. It cannot show a draft beside a path made for something else, because the view then draws the stored workholding. It can lose an Apply the user pressed.

## Open items

- **OI-2059:** choosing another setup drops the workholding section, and with it an Apply whose check is still running, without notice. Keep the panel until its Apply is taken, or say that the Apply was cancelled.
- **OI-2060:** confirm with the owner's CrankShaft project (numbers only) that #558 was the hidden-check case of this record, and not another path. If it was another path, extend `stale_setup_edits.rs` and fix it.
