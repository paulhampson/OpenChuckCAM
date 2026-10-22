# Trial feedback: triage of the issues awaiting planning (for the owner to check and approve)

Prepared 23 Oct 2026 from the 28 open issues labelled `awaiting-planning` (#286 to #314; #301 is a pull request, not an issue). Builds named in the issues: 0.13.0 to 0.17.0. **Nothing here has been sent to the coordinator or started.** On the owner's approval, the approved lanes go to the coordinator, the label is changed to show the issue is planned, and the owner's open questions are asked when each lane starts (the owner's instruction of 23 Oct).

Lane names with `BUG-` are bug fix lanes (the owner's convention); the others are features, assessments or mixed. All names are proposals; the coordinator takes decision, `OI-` and schema numbers. Requirement and specification changes are proposed in the lane and the owner decides; none is edited to fit.

## 1. Triage table

Risk: **S** = safety or correctness (NFR-12), **C** = changes generated G-code or default results, **U** = UI only, **D** = needs a decision or requirement from the owner.

| Issue | Type | Summary | Risk | Proposed lane |
| --- | --- | --- | --- | --- |
| #296 | Bug | Swarf: raising the tolerance changes the path so the warning grows (0.01 gives 0.0131, 0.0132 gives 0.0182) | S | BUG-SWARF4 |
| #294 | Bug | Swarf Check option: tooltip on "Warn only" glitches between two positions | U | BUG-SWARF4 |
| #304 | Bug | An uncommitted edit left in an inspector box (stepover) is set but does not mark the operation stale; warn and confirm on any leaving | S, D | BUG-EDIT |
| #298 | Bug | Faces added with "Add selection to set" are not coloured as part of the set (Operations view) | U | BUG-SELSET2 |
| #297 | Bug | Out-of-date message in three places: one place (the viewport), no operation number on the button | U, S | SIMUI |
| #293 | Bug | The deviation colour mapping cannot be hidden | U | SIMUI |
| #299 | Bug | The Show tick boxes and the Show drop-down are not in step | U | SIMUI |
| #300 | Feature | Cut part solid or translucent (personal setting) | U | SIMUI |
| #302 | Feature | Clicking an operation in Simulate goes to Operations with its inspector | U | SIMUI |
| #305 | Feature | Progress bar under the playback bar; the two check box groups with a separator | U | SIMUI |
| #303 | Feature | Helical and axial finishing: reduce the stepover automatically where it would miss material, on by default, honouring the stock to leave | C, D | BUG-FINSTEP |
| #314 | Feature | Axial and helical finishing: optional stepdown | C, D | BUG-FINSTEP |
| #292 | Bug | Roughing ends with a full unwind of the part (continuous A axis, no limit needed) | C, D | ROUGHPATH |
| #291 | Feature | Ramp entry: zigzag when there is not enough material for one line, as an option | C, D | ROUGHPATH |
| #286 | Feature | Facing: the unheld end (a tab there is left) | D | FACING |
| #287 | Feature | Facing: the sides of a part (box first, user-chosen stepdown) | D | FACING |
| #288 | Feature | Facing: restrict to the part (bounding outline, optional margin) or the part plus tabs | D | FACING |
| #289 | Bug | Add operation: strategy drop-down with an information panel; fix the layout and alignment | U | OPCREATE2 |
| #290 | Bug | Swarf explanation wording (a curved wall, straight across its height); draft agreed | U | OPCREATE2 |
| #306 | Task | Assess whether Post and Export needs its own step; Simulate's primary action becomes Post and Export | D | OUTPUT-1 |
| #307 | Bug | Write G-code gives no confirmation: show the path and file name of each file | U, S | OUTPUT-1 |
| #311 | Bug | Export dialog scroll bar clashes with the text | U | OUTPUT-1 |
| #308 | Feature | NC file names with templating, a per-user setting | D | OUTPUT-2 |
| #310 | Feature | Setup sheet name with the same templating | D | OUTPUT-2 |
| #309 | Feature | The setup sheet lists the NC files and their hashes, as split | S, D | OUTPUT-2 |
| #312 | Feature | Scroll through the generated G-code (read only) | U | OUTPUT-3 |
| #295 | Feature | Recovery checkpoints after a crash (every few minutes, last 3, project folder, project only) | D | CHECKPOINT |
| #313 | Feature | Remove unused tools from the project | U | TOOLLIB2 |

## 2. The proposed lanes

### Wave 1: correctness, start first (at most 3 lanes at once; the file overlaps are low)

**BUG-SWARF4** (Opus): #296, #294. #296 is the priority: a tolerance that does not bound the real error is a false assurance (NFR-12). Measure first with `blisk.step` with an avoid set and the blades, at several tolerances, with and without the avoid; find whether the tolerance drives both the fitting and the check; tests at several tolerances. #294 is a hover feedback loop (the tooltip sits under the pointer). *Overlaps:* the swarf lane's earlier work (#206, #226, #227); check against what has merged.

**BUG-EDIT** (Opus): #304. One navigation guard asked by every leaving action; a value is committed (and marks the operation stale) or not set; a warning with keep, discard or cancel; the owner's stepover case reproduced; check what the project file, the path and the simulation used. *Overlaps:* the number box lane (NUMBOX, merged as #268; open items OI-803 to OI-805 about the step bar) and the revert slot of BUG-REGEN (#243).

**BUG-SELSET2** (Sonnet): #298. A small follow-up to the selection sets lane (SELSET, #282 merged): the Operations viewport does not redraw when a set changes (check Model and Simulate too). *Could ride with* BUG-EDIT or SIMUI if a slot is short.

### Wave 2

**SIMUI** (Opus): #297, #293, #299, #300, #302, #305. All in the Simulate screen and its dock. **Starts after the open pull request #301 (BUG-SIMSTALE follow-up: the banner below the controls, "Generate toolpaths" when the stock simulation is skipped) merges**, because it changes the same dock and the stale banner that #297 consolidates (the owner decided: one message, in the viewport). One layout test for the dock with every state (banner, progress bar, groups). Decision (the stale rule is already settled in decision 0192; #297 amends where the message goes).

**BUG-FINSTEP** (Opus): #303 then #314. Changes the default results of helical and axial finishing (the automatic stepover reduction is on by default): a decision record, a changelog entry and a minor version bump (decision 0048), byte-identical output with the option off, an investigation of the other strategies that share the pass spacing. Cost on generation time measured (the owner closed #252 on 0.17.0, so a baseline exists). **Owner decisions:** how "miss material" is measured (leftover height, recommended, or spacing across the face), defaults, and the meaning of the stepdown for a part that turns about its axis.

**ROUGHPATH** (Opus): #292, #291. The end of roughing (no full unwind on a continuous A axis: shortest way to the next start; a G-code change, a decision record) and the zigzag ramp as an option with a minimum line length the lane suggests. Shares the entry and link modules (decision 0026). *Overlaps:* OPCREATE, which holds #199 (the ramp error wording).

### Wave 3 (sequenced after the lanes they depend on)

**OUTPUT-1** (Sonnet/Opus): #306 first (an assessment: a written recommendation on the Post and Export step, and a separate build issue if the owner agrees), then #307 and #311 (the write confirmation and the export dialog's scroll bar). The dialog layout is fixed with the same test as the other dialogs (#222, #262). **Owner decision:** #306. **Starts before OUTPUT-2 and OUTPUT-3**, because the assessment may change the dialog.

**OUTPUT-2** (Opus): #308, #310, #309. One small template engine for the NC file names and the setup sheet name (a per-user setting), then the setup sheet that lists the files and their hashes as the project is split. *Overlaps:* lane TB (the setup sheet traceability block, GC-12 item 6) and lane GATES (draft pull request #284 touches the setup sheet): sequence after both. **Owner decisions:** the hash algorithm, the field list, a format choice for the sheet.

**OUTPUT-3** (Sonnet): #312. A read-only, lazily loaded G-code view with line numbers, per file when split, jump to a line and search. After OUTPUT-1 (the same dialog).

**FACING** (Opus): #286, #287, #288. Three facing options (the unheld end, the sides of a block, the extent: whole stock, part, or part plus tabs). **Starts after CUTOPTS merges #211** (extend the tool beyond the stock), because side facing needs it, and takes a requirement entry for facing, which the lane proposes. *Overlaps:* the STEP lanes (pull request #274 face sets), the held region work (SET-09).

**OPCREATE2** (Sonnet): #289, #290. The strategy drop-down and information panel, and the swarf wording. After OPCREATE merges (it holds #195 and #242). The owner reviews the final text of all strategy explanations.

**CHECKPOINT** (Opus): #295. Recovery checkpoints: automatic, every few minutes (a setting), the last 3 in the project folder, project only; never overwrites the saved project; a damaged checkpoint gives a message. A requirement entry and a decision record; a schema claim if a marker is stored; a fuzz target for the reader. Independent: can fill an idle slot.

**TOOLLIB2** (Sonnet): #313. Remove unused tools from the project (the project's copy only, TL-07). Joins TOOLLIB if that lane is not complete.

## 3. Order and limits

Wave 1: BUG-SWARF4, BUG-EDIT, BUG-SELSET2. Wave 2: SIMUI (after pull request #301), BUG-FINSTEP, ROUGHPATH, OUTPUT-1. Wave 3: OUTPUT-2 and OUTPUT-3 (after OUTPUT-1; OUTPUT-2 after lane TB and GATES), FACING (after CUTOPTS #211), OPCREATE2 (after OPCREATE), CHECKPOINT, TOOLLIB2. No more than three lanes run at once; lanes that touch the same files are sequenced. The statuses of CUTOPTS, OPCREATE, TOOLLIB and the swarf lane are as of the coordinator's last report and should be confirmed by the coordinator when it takes the proposal.

## 4. Decisions only the owner can make (asked when each lane starts)

| Decision | Issue | Lane |
| --- | --- | --- |
| Remove or keep the Post and Export step | #306 | OUTPUT-1 |
| How "miss material" is measured; defaults; stepdown meaning | #303, #314 | BUG-FINSTEP |
| The end-of-roughing move and its decision record (G-code changes) | #292 | ROUGHPATH |
| Minimum line length for the zigzag ramp | #291 | ROUGHPATH |
| Hash algorithm, template fields, setup sheet format | #308, #309, #310 | OUTPUT-2 |
| Overwrite rule for existing files; the G-code view's place | #307, #312 | OUTPUT-1, OUTPUT-3 |
| Facing requirement entry; the unheld end rules | #286 to #288 | FACING |
| Checkpoint requirement entry, marker and schema | #295 | CHECKPOINT |
| Final strategy explanation text | #289, #290 | OPCREATE2 |
| Dock layout details and groups | #297, #305 | SIMUI |

## 5. Not covered here

- The in-flight work the owner already approved (BUG-SIMSTALE, SELSET, OPCREATE, CUTOPTS, SETUPPOS, TOOLLIB, MSGPROG, STEPALL and the housekeeping lanes) is unchanged.
- #183 (math in number boxes) and #216 (bands) remain requirement proposals; #285 was closed as not a defect; #252 was closed by the owner.
