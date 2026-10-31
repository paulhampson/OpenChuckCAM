# 0302: First-run rough edges (lane FIRSTRUN)

- **Status:** accepted (30 Oct 2026) by the project owner, relayed by the coordinator session (decision 0049); the owner's answers on items 1 to 4 were relayed the same way.
- **Date:** 30 Oct 2026.
- **Lane/issue:** FIRSTRUN. It refers to the five findings lane DOCSUSER made while capturing the tutorial's screenshots; no GitHub issues exist for them.
- **Requirements:** UI-11, UI-12, SET-09, OP-21, UI-25, OP-03, TL-05, PRJ-08, GC-07, NFR-12.
- **Builds on:** [0173](0173-status-bar-and-required-values.md) (health badge and required values), [0230](0230-material-preset-per-operation.md) (the stepdown follows the tool's cutting data), [0243](0243-project-recovery-checkpoints.md) (recovery checkpoints), [0282](0282-four-step-workflow-with-separate-simulate-and-export-actions.md) (the four-step bar).
- **Amends:** UI specification sections 6 (step bar), 11.3, 11.7 and 3 (save state), edited in the same change. No requirement or architecture text changes; no project schema change.
- **Open items:** OI-1230, OI-1231, OI-1232.

## Context

A first-time user met five rough edges: a red "Error to fix" chip on an empty project while Messages counted no error; a blank "required" Stepdown; a workholding sentence that was untrue with a held bar; no hint that saving is manual; and a feed of 1000 mm/min that the machine's limits cut to about 673 with only the export dialog saying so.

## Decision

1. **A missing machine is a to-do, not an error (the owner's option A).** The Setup step reads "Needs attention" with the reason "Choose a machine"; red stays for real faults (a setup with an error, a required value missing on an item that exists). The step bar and Messages now agree. Generation and export still refuse without a machine.
2. **No guessed stepdown (the owner's option A).** An empty Stepdown reads, under its field, "Required. Enter the depth of each layer, or add cutting data for this tool and material in the tool library, then choose Update from library." The lists keep the short "Enter stepdown". Nothing is stored or derived, so there is no schema change; a wrong depth of cut is a machine-safety matter (NFR-12).
3. **Workholding wording made true.** With a held region the "No tabs are stored" line says the part stays joined to the stock at the held end and to add tabs if an operation will cut it free of that end; with no held region it keeps the old words. The by-design grip warning reads "The chuck grips N mm of the part here (X a to b mm), so that length is never machined. Shorten the grip if it should be cut." This makes a false statement true, so it needed no decision.
4. **The unsaved marker says what it means (the owner's option A).** "Edited, not saved", with the tooltip "Changes are kept in memory only. File > Save (Ctrl S) writes them to the project."; "· saved" as before. Quit, open and new already ask about unsaved changes, and recovery checkpoints exist (0243). Nothing saves the user's file by itself.
5. **The feed field says what the machine's limits did.** Under Cutting feed, while the operation's path is current, a line reads "Last run: the machine's feed limits slowed N moves to as low as X mm/min, below the Y mm/min set here. The feed set here is unchanged; the machine's limits lower it only where it cannot keep up." It reads the last run's per-move notes (the same notes the export dialog and Messages show); the feed the user typed and every written feed are unchanged (GC-07). The notes name an operation, not its ID, so the line looks the operation up by the name its run's own project gave it (a rename since the run changes nothing) and shows nothing when two operations of that project share a name (CodeRabbit finding on the pull request).

## Alternatives not chosen

- Item 1: keeping the chip red and counting the machine as a Messages error (the user would meet an error on an empty project); changing only the tooltip.
- Item 2: a one-click suggested stepdown (OI-1231), or a silent derived default (advised against).
- Item 4: an amber marker after a delay (a timer, and amber is kept for warnings).

## Verification

Tests named `req_ui_11_req_ui_12_a_missing_machine_is_attention_not_an_error`, `req_ui_25_req_op_21_no_tabs_warning_is_true_when_a_held_region_joins_the_part`, `req_tl_05_req_op_03_a_missing_stepdown_says_where_it_can_come_from`, `req_prj_08_req_ui_03_unsaved_changes_say_they_are_not_saved`, `req_nfr_12_req_op_11_the_feed_summary_names_the_lowest_effective_feed`, `req_nfr_12_req_op_11_the_feed_field_says_what_the_machine_limits_did` and `req_ui_11_req_nfr_12_a_run_report_is_current_only_while_the_operation_is`.
