# Check for issue #558: changing the tab at the free end makes the operations stale

**What was fixed** (pull request #565, [decision 0528](../../../docs/decisions/0528-workholding-apply-is-stored-when-its-check-finishes.md)): the workholding section edits a draft, and **Apply** checks the draft as a job before storing it. Before the fix, the result of that check was taken only while the section was shown: if you pressed **Apply** and then clicked an operation (or closed the section) before the check finished, nothing was stored, the operations stayed up to date, and the view drew the new tab next to a path made for the old one. Now the draft is stored when the check finishes, whether or not the section is still shown, and the operations go stale. Also new: while a draft is not applied, the **Messages** list says so.

The lane found nothing missing from what makes an operation stale: every tab and held-region field already did (decision 0528, "Context"). Whether your crankshaft case was this one is open item OI-2060; step C below asks for the answers it needs.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. Do not run the G-code of this project on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

[`project/`](project/), written by [`generate.py`](generate.py) from public material only: the public CrankShaft (`testdata/step/valid/crankshaft.step`, the model you gave the repository) centred on the rotary axis, X 0 to 40, in 40 mm stock from X -13 to 47. The chuck holds 8 mm at the start (X -13 to -5); **nothing holds the other end**. Two cylinder tabs, 10 mm across: **Tab 1** at the head (X -5 to 0) and **Tab 2** at the free end (X 40 to 47, 7 mm long, typed, not automatic). One operation, **1 Rough** (rotary roughing, 3.175 mm flat end mill).

## Steps

Use the nightly build of 8 November 2026 (commit 76796fb24) or later, or a build from `main`.

1. Copy the folder `project` to a folder of your own and open the copy with **File > Open project**.
2. Click **Operations** at the top, then **Generate**. Wait until **1 Rough** shows its path and is no longer stale.

**A. Apply, then leave the section at once**

3. In the project panel, under **Setup 1**, click **Workholding**.
4. In **Tab 2**, set **Length** to `4` and press <kbd>Enter</kbd>. The view draws the shorter tab and shows the chip "Showing unapplied workholding".
5. Click **Apply**, and straight away, before its spinner stops, click **1 Rough** in the project panel.
6. Wait a few seconds and look at **1 Rough** and the view.

**B. A draft left without Apply**

7. Click **Workholding** again. In **Tab 2**, set **Length** to `3`, press <kbd>Enter</kbd>, and do **not** press Apply. Click **1 Rough**.
8. Look at the **Messages** list and at the status bar at the bottom.
9. Click **Workholding** again and press **Revert**.

**C. For OI-2060 (from memory, about your crankshaft project)**

10. When you shortened the free-end tab there: was the tab **Automatic** (Place automatically ticked)? Did you see a "Not applied" chip? Did you click away before Apply's spinner stopped? Which build was it?

## Expected result

- **A:** **1 Rough** becomes **Stale** (the out-of-date mark), and **Generate stale** is offered, although you left the section before the check finished. Tab 2 is drawn 4 mm long (X 40 to 44) and the "Showing unapplied workholding" chip has gone. Generating again gives a new path.
- **B:** **1 Rough** stays as it was (correct: nothing new is stored). The **Messages** list has this warning, with an action that opens the section:

  > The workholding of 'Setup 1' has changes that are not applied. The view shows them, but the toolpaths, the simulation and the G-code use the held regions and tabs stored in the setup: apply or revert the changes in its workholding section

  and the status bar says "Workholding changes not applied". After **Revert** the warning goes.

**Old behaviour (not fixed):**

- **A:** **1 Rough** stayed up to date, with its old path drawn beside the new tab. The edit was stored later, without notice, when you next opened the Workholding section.
- **B:** no line in the **Messages** list; only the "Not applied" chip and the status bar said it.

**If step 5 is too quick to catch.** On this small project the check may finish before you can click away; then part A shows nothing either way (it passes on old builds too). Part B tells old and new apart on any project. On a large project, such as your crankshaft, the check takes longer and part A is easier to hit.

## Report back

Reply on issue #558 with: **PASS**, or **FAIL and what you saw** (for A and B: the operation's mark, the message text), and your answers to step 10 for OI-2060. Numbers and messages only; no project files from your own work.

## What we saw when preparing this

This fix is in the interface, which the command-line tool does not use, and the container this was prepared in has no display, so the steps above were not run in the program here, on either build.

- **What we ran:** `openchuck-cli validate project` and `openchuck-cli regenerate project` on `main` (see below), to make sure the project opens and generates.
- **The fix's own tests** (decision 0528), in `apps/openchuck-ui/src/shell/frame_tests.rs`: `req_ui_11_req_nfr_12_an_apply_that_finishes_after_the_section_is_left_is_stored_and_makes_the_operations_stale` (part A), `req_ui_11_req_nfr_12_a_draft_left_without_apply_is_said_to_be_not_applied` (part B) and `req_op_21_req_ui_11_req_nfr_12_a_refused_apply_stores_nothing_and_says_the_draft_is_not_applied`. Results below. They use the interface code without a window; they do not exist before the fix, so they cannot be run on the old build. Decision 0528 records that the shell test reproduced the fault before the fix.
