# Owner checks

Projects and step-by-step checks for the project owner, one folder per issue whose fix is merged on `main` and waits for the owner to confirm it (label `awaiting-fix-feedback`, milestone 2026.1-alpha). Written by lanes OWNERCHECKS and OWNERCHECKS2 from public material only: the user guide's baluster ([`testdata/examples/baluster`](../testdata/examples/baluster/)), the blisk starting project ([`testdata/examples/blisk-start`](../testdata/examples/blisk-start/)) and the public STEP files of [`testdata/step/valid`](../testdata/step/valid/). Nothing here comes from a private project.

This folder lives only on the branch `owner-checks/2026.1-alpha`, which is never merged. Each check folder has:

- `<folder>.zip`: the finished project and a one-page `README.txt`; unzip it anywhere and open `<folder>/project` with **File > Open project**. Nothing needs to be run first;
- `CHECK.md`: the numbered steps, the expected result, the old behaviour, and what we saw on the build before the fix and on `main`;
- `project/`: the same project as files, to read here (not for #556, whose model is about 25 MB: it is in the zip only);
- `generate.py`, which writes `project/` with fixed ids, and `COMMENT.md`, the text posted on the issue.

> [!WARNING]
> Nothing in these checks shows that a program is safe to run. They look at messages and paths only. Do not run their G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

| Issue | What to check | Time | Steps | Project zip |
| --- | --- | --- | --- | --- |
| #555 | Axial finishing limited to a selection stops at the selection's edge and no longer runs down onto the tab | 5 min | [`555-axial-into-tab`](555-axial-into-tab/CHECK.md) | [`555-axial-into-tab.zip`](555-axial-into-tab/555-axial-into-tab.zip) (62 KB) |
| #558 | Shortening the free-end tab and pressing Apply makes the operations stale, even if you click away at once; an unapplied draft is listed in Messages | 10 min | [`558-tab-edit-makes-stale`](558-tab-edit-makes-stale/CHECK.md) | [`558-tab-edit-makes-stale.zip`](558-tab-edit-makes-stale/558-tab-edit-makes-stale.zip) (13 KB) |
| #556 | Zooming stays smooth with a selection set of about 470,000 triangles shown | 10 min | [`556-zoom-large-selection`](556-zoom-large-selection/CHECK.md) | [`556-zoom-large-selection.zip`](556-zoom-large-selection/556-zoom-large-selection.zip) (9.4 MB) |
| #548 | A tab shorter than the tool no longer stops an operation that cannot reach the tab; a whole-part operation gets a "Tab length" warning | 5 min | [`548-tab-length-limited-op`](548-tab-length-limited-op/CHECK.md) | [`548-tab-length-limited-op.zip`](548-tab-length-limited-op/548-tab-length-limited-op.zip) (67 KB) |
| #547 | The flute check no longer refuses because of stock the operation cannot reach, and still warns where it can | 5 min | [`547-flute-check-outside-selection`](547-flute-check-outside-selection/CHECK.md) | [`547-flute-check-outside-selection.zip`](547-flute-check-outside-selection/547-flute-check-outside-selection.zip) (66 KB) |
| #477 | A free-end ring that would part the part from the chuck is warned of | 5 min | [`477-free-end-ring`](477-free-end-ring/CHECK.md) | [`477-free-end-ring.zip`](477-free-end-ring/477-free-end-ring.zip) (62 KB) |
| #442 | Swarf stops short of the faces at the ends of the blade (Keep off the end faces), and warns when the setting is off | 10 min | [`442-swarf-past-blade-end`](442-swarf-past-blade-end/CHECK.md) | [`442-swarf-past-blade-end.zip`](442-swarf-past-blade-end/442-swarf-past-blade-end.zip) (69 KB) |
| #512 | Face-flow finishing on the blisk's hub floor, with a guide edge | 15 min | [`512-face-flow-finishing`](512-face-flow-finishing/CHECK.md) | [`512-face-flow-finishing.zip`](512-face-flow-finishing/512-face-flow-finishing.zip) (70 KB) |
| #351, #257 | Exact STEP surface contact, on and off, for axial finishing with a tapered ball and pencil clean-up with a flat end | 15 min | [`351-257-exact-step-contact`](351-257-exact-step-contact/CHECK.md) | [`351-257-exact-step-contact.zip`](351-257-exact-step-contact/351-257-exact-step-contact.zip) (10 KB) |

### Second set (lane OWNERCHECKS2, 8 Nov 2026)

Fixes merged on `main` after the first set was checked: the message work of [decision 0536](../docs/decisions/0536-message-levels-operation-reference-and-setting-links.md) step 2 (#625) and decisions [0538](../docs/decisions/0538-a-possible-collision-is-a-warning-to-acknowledge.md), [0539](../docs/decisions/0539-workholding-apply-warns-on-a-tab-with-no-room-or-tabs-that-overlap.md) and [0540](../docs/decisions/0540-the-command-line-checks-before-it-writes.md) (#623, #591, #584). Each was run with the command-line tool on the build before the fix and on `main` at `1f3675d52`; the interface steps could not be run without a display, so they are given as click sequences.

| Issue | What to check | Time | Steps | Project zip |
| --- | --- | --- | --- | --- |
| #611 (and decision 0539) | A refused workholding Apply marks only the refusing tab red; a warning found with it stays amber; Apply stores an automatic tab with no room, with a warning | 5 min | [`611-refusal-and-warning-levels`](611-refusal-and-warning-levels/CHECK.md) | [`611-refusal-and-warning-levels.zip`](611-refusal-and-warning-levels/611-refusal-and-warning-levels.zip) (62 KB) |
| #567 (decisions 0538 and 0540) | A possible collision (a tab shorter than the tool) reads "Needs acknowledging" and the export waits for its own tick; the command line checks before it writes and needs `--acknowledge-collision-warnings` | 10 min | [`567-collision-acknowledge`](567-collision-acknowledge/CHECK.md) | [`567-collision-acknowledge.zip`](567-collision-acknowledge/567-collision-acknowledge.zip) (62 KB) |
| #600 | One feed-limit line per operation instead of one per move; no "0.0 mm/min" for a turn of A at the rotary axis; F words unchanged | 5 min | [`600-feed-limit-summary`](600-feed-limit-summary/CHECK.md) | [`600-feed-limit-summary.zip`](600-feed-limit-summary/600-feed-limit-summary.zip) (63 KB) |
| #622 | A note about the whole path no longer reads "move 0:" | 3 min | [`622-whole-path-notes`](622-whole-path-notes/CHECK.md) | [`622-whole-path-notes.zip`](622-whole-path-notes/622-whole-path-notes.zip) (63 KB) |
| #634 (fixed in #653) | The "Exact STEP surface contact" tooltips end at "More accurate ..." and "No effect on a mesh part.", with no measurements or decision numbers | 2 min | [`634-exact-contact-tooltip`](634-exact-contact-tooltip/CHECK.md) | [`634-exact-contact-tooltip.zip`](634-exact-contact-tooltip/634-exact-contact-tooltip.zip) |
| #635 (fixed in #648) | The warning under a setting is one block across the Inspector's width | 3 min | [`635-inline-warning-alignment`](635-inline-warning-alignment/CHECK.md) | [`635-inline-warning-alignment.zip`](635-inline-warning-alignment/635-inline-warning-alignment.zip) |
| #636 (fixed in #653) | File > Open project and New project: OK opens a typed or pasted folder and Cancel abandons it, with no "Unconfirmed changes" dialog | 3 min | [`636-open-no-unconfirmed-dialog`](636-open-no-unconfirmed-dialog/CHECK.md) | [`636-open-no-unconfirmed-dialog.zip`](636-open-no-unconfirmed-dialog/636-open-no-unconfirmed-dialog.zip) |
| #637 (fixed in #652) | The areas a strategy could not reach are hatched on the surface (1 mm lines that move with the part), as one closed area | 3 min | [`637-unreachable-pattern`](637-unreachable-pattern/CHECK.md) | [`637-unreachable-pattern.zip`](637-unreachable-pattern/637-unreachable-pattern.zip) |
| #643 (fixed in #654) | A steep drop inside a roughing pass goes down no faster than the plunge feed (193 such moves before on the public blisk, 0 now) | 5 min | [`643-roughing-descent-feed`](643-roughing-descent-feed/CHECK.md) | [`643-roughing-descent-feed.zip`](643-roughing-descent-feed/643-roughing-descent-feed.zip) |
| #641 (fixed in #656) | End facing starts each band in air beside the stock, checks Stepover and Stepdown, posts a whole number of slices, and adds bands where the stock rises above a Part-only area | 5 min | [`641-end-facing-from-air`](641-end-facing-from-air/CHECK.md) | [`641-end-facing-from-air.zip`](641-end-facing-from-air/641-end-facing-from-air.zip) |
| #604 (fixed in #675) | After a stock change, the default tab line and Add default tabs follow at once, without save and reopen | 5 min | [`604-workholding-tab-defaults-follow-stock`](604-workholding-tab-defaults-follow-stock/CHECK.md) | [`604-workholding-tab-defaults-follow-stock.zip`](604-workholding-tab-defaults-follow-stock/604-workholding-tab-defaults-follow-stock.zip) |

The #600 and #622 projects are unchanged copies of the tutorial project [`testdata/examples/baluster/baluster-project`](../testdata/examples/baluster/baluster-project/). Each zipped project of the second set was checked with `openchuck-cli validate` on `main` at `1f3675d52` (all valid).

### Release candidate checks (drafts, lane OWNERCHECKS2)

Fixes not merged when written (8 Nov 2026), and the message-work checks that wait for their tracking issue. They are in [`rc/`](rc/), each with its project, its zip and a CHECK.md marked **Draft, to run on the release candidate**, giving today's behaviour on `main` at `1f3675d52`. When a fix merges, its folder is finished (the expected texts copied from the merged build, the pull request named), moved up beside the others, and given its COMMENT.md.

| Issue | What to check | Time | Steps | Project zip |
| --- | --- | --- | --- | --- |
| 0536 step 3 (merged in #648; to post on the tracking issue) | Every message has its topic and context; Show setting goes to every kind of field | 10 min | [`0536-step3-lead-in-show-setting`](rc/0536-step3-lead-in-show-setting/CHECK.md) | [`0536-step3-lead-in-show-setting.zip`](rc/0536-step3-lead-in-show-setting/0536-step3-lead-in-show-setting.zip) |
| 0536 step 4 (merged in #659; to post on the tracking issue) | Setup and export messages in the same form, with field labels; command-line help without requirement numbers | 10 min | [`0536-step4-setup-export-cli-messages`](rc/0536-step4-setup-export-cli-messages/CHECK.md) | [`0536-step4-setup-export-cli-messages.zip`](rc/0536-step4-setup-export-cli-messages/0536-step4-setup-export-cli-messages.zip) |
| 0536 step 5 (merged in #653; to post on the tracking issue) | The lost status lines reach Jobs, Messages or a dialog; "Nothing to report" and the new Facing wording | 10 min | [`0536-step5-status-and-wording`](rc/0536-step5-status-and-wording/CHECK.md) | [`0536-step5-status-and-wording.zip`](rc/0536-step5-status-and-wording/0536-step5-status-and-wording.zip) |
| 0536 steps 3 to 5 | The body of the tracking issue that lists the three checks above for you to tick through | — | [`0536-tracking-issue/ISSUE.md`](rc/0536-tracking-issue/ISSUE.md) | — |
| #639, #640 (decisions 0543, 0544, #646; code on lane branches) | A ramp follows the pass (around a ring, back and forth on a line), with Allow single-point plunge and Steeper ramp near the axis; a lead on a ring stays in the plane of the cut | 15 min | [`639-640-ramp-and-lead-follow-the-path`](rc/639-640-ramp-and-lead-follow-the-path/CHECK.md) | [`639-640-ramp-and-lead-follow-the-path.zip`](rc/639-640-ramp-and-lead-follow-the-path/639-640-ramp-and-lead-follow-the-path.zip) |
| #638 (decision 0545, #647) | Steps between rings can be Blended (a short helix piece); saved operations keep Sharp; new ring operations start Blended | 10 min | [`638-blended-ring-steps`](rc/638-blended-ring-steps/CHECK.md) | [`638-blended-ring-steps.zip`](rc/638-blended-ring-steps/638-blended-ring-steps.zip) |
| #644 (decision 0546, #647) | Extend past the stock on ring and spiral passes continues the pass instead of a line along X | 5 min | [`644-run-outs-follow-the-pass`](rc/644-run-outs-follow-the-pass/CHECK.md) | [`644-run-outs-follow-the-pass.zip`](rc/644-run-outs-follow-the-pass/644-run-outs-follow-the-pass.zip) |

Shared helpers: [`ownercheck.py`](ownercheck.py) (writes the project files), [`make_zips.py`](make_zips.py) (writes the zips: sorted entries, fixed times, so the same projects give the same bytes), [`gcode_extent.py`](gcode_extent.py) (reads where a program's fed moves go), [`gcode_past_x.py`](gcode_past_x.py) (how far A turns past an X) and [`gcode_descent.py`](gcode_descent.py) (fed moves going down the tool axis faster than a feed) and [`blake3_pure.py`](blake3_pure.py) (the model hash, when the `blake3` package is not installed).

Each zipped project was checked with `openchuck-cli validate` on `main` at `ae608e28f` (all valid).

Reply on each issue with **PASS**, or **FAIL and what you saw**. Numbers and messages only: never paste or attach files from a private project.
