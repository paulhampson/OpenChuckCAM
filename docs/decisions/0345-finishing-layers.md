# 0345: Finishing layers with a light final pass (OP-04, TL-05, NFR-12; issue #314; lanes FINSTEP and ENDFACE)

- **Status:** proposed (30 Oct 2026). The owner's answers on OI-1391 (option 1), OI-1392 (option 3) and OI-1395 (option 1) were relayed by the coordinator session (decision 0049). The record awaits the owner's acceptance.
- **Date:** 30 Oct 2026.
- **Builds on:** [0344](0344-finishing-stepdown-and-free-ends.md) (the finishing Stepdown and the Finishing layers switch), the FINSTEP handoff's owner decisions for #314 (`docs/handoffs/finstep-handoff.md` on branch `codex/lane-finstep-adaptive-spacing-and-layers`): off by default with exact previous output; helical, axial and indexed finishing; Stepdown along the tool axis; the final pass on the target; invalid depths and depths past the flutes refused; protections kept; no engagement promise (OI-1373).
- **Open items:** OI-1391, OI-1392 and OI-1395 closed by this record. OI-1373 (engagement off the rotary centreline, formerly FINSTEP's OI-1341) stays open.

## Decision

1. **Finishing layers** (helical, axial and indexed finishing; off by default).
   - With them on, the operation cuts layers above its final passes, then the final passes.
   - With them off, the path is exactly as before (tested).
2. **Where the layers start (OI-1391, option 1).**
   - **From what stock:** at each point of the final passes the stock is read along the tool axis: the stock earlier operations of the setup left (SIM-06), or the setup's own stock when there is none (decision 0344, OI-1394).
   - **Where a layer cuts:** only where the stock holds material above it, so no layer cuts air.
   - **Accuracy:** the stock is known to a simulation cell (0.1 mm by default), so a layer within a cell of the material may be cut or left by one. No uniform engagement is promised (OI-1373).
3. **How the depth is split (OI-1392, option 3: a light final pass).**
   - Target T is the surface plus the stock to leave, as the final pass cuts it. The layers stand at T + F + k·S along the tool axis (k = 0, 1, 2 …), where F is the Finish allowance and S the Stepdown, cut top first.
   - The final pass at T follows, and takes only what the lowest layer left, at most F.
   - The heights are fixed above the target, so the layers of neighbouring passes line up.
   - Where the material over T is no more than F, only the final pass cuts there. A layer is the final pass lifted along the tool axis, so it stays above the target and keeps the final pass's A: indexed finishing gains no cutting angle.
4. **Finish allowance (OI-1395, option 1).**
   - The parameter `finish_allowance` (mm) on helical, axial and indexed finishing, shown only with Finishing layers on.
   - Its default is 0.1 mm (the owner's example), shown in the field and named in the note. It is not a depth of cut: the Stepdown is, and it still comes only from the operation or the tool's cutting data (decision 0344, NFR-12).
   - Validation: finite, at least 0, and less than the Stepdown. Otherwise the operation is refused, naming both values.
   - Zero means no light pass: the lowest layer is the final pass.
5. **Refused, not made.** A Stepdown longer than the tool's flutes, since a layer that deep would cut with the shank.
6. **Free-end turns are not affected.** Their layers go down beside the face, which the side of the tool finishes in every layer; a light pass along the tool axis would add a turn without improving it.
7. **The note:** "finishing layers: up to 3 layers of 0.8 mm above a finish allowance of 0.1 mm, then the final pass on the surface; the layers start from the stock the earlier operations left, known to a simulation cell, so the depth of each cut is not uniform and no engagement is promised". With no earlier operation it names "the setup's own stock" instead.

## Consequences

- **Saved projects and goldens.** Unchanged, since layers are off by default and absent in saved operations. Project schema 23 needs no further migration for them.
- **Determinism.** Tested at 1, 2 and N threads (GC-07, NFR-06).
- **Library.** Additive: `finishing::parameter::FINISH_ALLOWANCE` and `finishing::DEFAULT_FINISH_ALLOWANCE`.
- **Not done here:** layers for the simultaneous spiral (the FINSTEP handoff does not authorise them; OI-1373), an engagement limit, and adaptive spacing across layers (decision 0239, stage 3).

**Tests** (`crates/openchuck-path/tests/finishing.rs`, on a radius 10 cylinder in stock of radius 12 or 10.5):
- `req_op_04_finishing_layers_step_down_to_a_light_final_pass` (helical, axial and indexed: levels at 10.1, 10.9 and 11.7, the final pass at 10 last)
- `req_op_04_no_layer_cuts_air`
- `req_op_04_a_zero_finish_allowance_puts_the_last_layer_on_the_target`
- `req_op_04_layers_off_is_the_path_as_before`
- `req_nfr_07_a_finish_allowance_not_below_the_stepdown_or_a_stepdown_past_the_flutes_is_refused`
- `req_nfr_06_req_gc_07_finishing_layers_are_identical_at_1_2_and_n_threads`
