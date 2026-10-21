# 0175: A swarf cut into the face beyond the tolerance is a warning, not a refusal (lane BUG-SWARF; issue #226)

- **Status:** accepted (21 Oct 2026, by the owner directly in the lane BUG-SWARF session). Proposed 21 Oct 2026. The owner's answers set the behaviour:
  - on issue #226 (20 Oct 2026): the path is produced with a warning and does not wait for an acknowledgement, and the swarf tolerance setting is highlighted when the warning occurs;
  - relayed by the coordinator on 21 Oct 2026 (OI-630): option A, so every overcut above the operation's tolerance is a warning, whatever its cause.

  Only the owner accepts this record (decision 0049).
- **What it amends:**
  - **accepted [decision 0061](0061-slice-17-swarf.md) item 4**: a gouge beyond the tolerance refused the face;
  - **[decision 0021](0021-sp-11-swarf.md) item 3, as built**: the gate refused;
  - **[decision 0111](0111-swarf-through-the-application.md) item 2, last case**: "the gouge and material-left gate ... The strategy still refuses these".

  OP-07 and the requirements are unchanged.
- **Date:** 21 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-630 (which overcuts become a warning) is new and closed by the owner's answer; OI-633 is new.

Numbering: see [decision 0174](0174-step-only-strategies-blocked-on-a-mesh-part.md). OI-631 and OI-632 are the owner questions of issue #227 (a boundary, check and avoid sets on a swarf operation), whose record, 0176, comes with the lane's second pull request.

## Context

Trial finding F-70 (issue #226), on the owner's STEP part. A swarf operation failed with "Swarf on face 7 would cut into the surface by 0.0131 mm at (40.330, -5.370, -9.425), more than the tolerance of 0.01 mm. The normal twists 8.62 degrees along the rulings, so the face is ruled but not developable and a straight flank cannot follow it; a smaller tool radius deviates less." The owner asked for a warning that explains that the tolerance can be raised for this operation, and where.

The cut is into the finished surface (an overcut). Turning the refusal into a warning means a program can be written with a known overcut above the tolerance. That needs care under NFR-12: generated G-code is the user's to verify, so the overcut must be visible wherever the user looks before running the program.

Requirements: OP-07 (swarf), OP-12 (gouge checking), NFR-05 (tolerance), NFR-06 and GC-07 (determinism), NFR-07 (clear errors), NFR-12 (the user verifies), GC-12 (setup sheet), GC-15 (checks before writing), UI-11 and UI-12 (item states, messages). Records read: 0021, 0061, 0111 (swarf), 0048 (API changes before 1.0), 0110 (setup sheet traceability), 0114 (plain wording), 0117 (item states), 0162 (results histogram), and 0173 (proposed; the required-value marks of lane BUG-STALE part 2, issue #224), whose marks the tolerance highlight reuses.

## Decision

1. **Every overcut above the operation's tolerance is a warning (OI-630, the owner's option A).** Swarf's gate still measures how far the tool cuts into each face beyond the stock to leave (decision 0061 item 4). A face cut deeper than the tolerance is no longer refused, whatever the cause (a twisting face, a four-axis tilt, or both): the path is made, with a warning. Material left beyond `max_material_left` is still refused, as before (decision 0061 item 7). The other refusals (no rulings, rulings along the rotary axis, a ruling longer than the flank, the A range) are unchanged.
2. **The library carries warnings beside notes (OP-16).** `openchuck-path` has a new `PathWarning`:
   - `message`: what is wrong, where, and what to do;
   - `parameter`: the operation parameter that, changed, would accept or remove the warning (`tolerance` here).

   `StrategyOutput` and `GeneratedPath` gain a `warnings` field. The shared generation copies a strategy's warnings to the path and adds each to the path's notes as well, first, so a host that shows only notes still says it. `swarf::FacePath::overcut_warning(tolerance)` gives the warning, and `FacePath::refusal` takes only `max_material_left`. This breaks the library API: the workspace goes to 0.13.0 with a changelog entry in each library crate ([decision 0048](0048-pre-1-0-api-changes.md)).
3. **The words (decision 0114, plain wording).**
   - **What and where.** The warning gives the face, the depth to four decimals and the place: "Swarf on face 7 cuts into the finished surface by 0.0131 mm at (40.330, -5.370, -9.425), more than this operation's tolerance of 0.01 mm."
   - **Why.** "The face is ruled but not developable: it is made of straight lines, but its surface turns 8.62 degrees along each of them, so the straight side of the tool cannot lie flat on it everywhere; a smaller tool radius deviates less." For a tilt, the existing sentence names the taper that would remove it.
   - **What to do.** "To accept this cut, raise the Tolerance of this operation to at least 0.0131 mm (in the operation inspector's parameters, where the field is marked); to cut less into the face, use a tool with a smaller tool radius or the taper named here, or machine this face with another strategy."
   - **The check.** "The program is made with this cut in it: check the program and the simulation before running it."

   The swarf Tolerance field's description now says a deeper cut is warned of, and that raising the tolerance accepts it.
4. **The default tolerance is not loosened.** It stays 0.01 mm (the common parameter's default). Raising it is the user's explicit choice for the operation.
5. **Visible wherever the user looks before running the program (NFR-12).**
   - **Operation state.** An up-to-date operation whose path was made with a warning shows the warning state, with the warning's words as the cause (UI-11, decision 0117). The warning goes with the path: an edit that makes the operation stale clears it until the next generation.
   - **Messages.** Each warning is listed in Messages, naming the operation, with a new action, `MessageAction::ShowSetting`, labelled "Show setting".
   - **Output.** The checks before writing G-code repeat each warning, naming the operation (GC-15). Writing is not blocked: the owner chose no acknowledgement.
   - **Results.** The Results window lists the path's notes as warning lines, and the warning is one of them.
   - **The simulation.** The simulation shows the overcut in the deviation colours and the histogram of [decision 0162](0162-simulation-results-histogram-and-probe.md) when it is deeper than the simulation's cell, its own error. A shallower overcut, like the 0.0131 mm of #226 against cells of 0.025 mm or more (0.1 mm by default), is below what the simulation resolves. The gate's own measure, in the warning, is then the record (OI-633).
   - **Setup sheet.** The Operations table has a Tolerance column: the tolerance each operation is generated with, its own value or its strategy's default ("0.01 mm"). This is the traceability of [decision 0110](0110-setup-sheet-traceability.md) for the limit a warning was measured against. The G-code is unchanged (GC-07).
6. **The tolerance setting is marked when the warning occurs (the owner's answer on #226).** `Application::operation_warning_settings(operation)` names the settings the current path's warnings point at. The operation inspector marks each named field with the same marks as the required values of [decision 0173](0173-status-bar-and-required-values.md) (proposed, issue #224):
   - the warning shape and a plain sentence under the field, so the mark does not rest on colour alone;
   - an amber border;
   - the field scrolled into view, with its section opened, when the "Show setting" link is followed.

   The mark goes with the warning.
7. **Deterministic.** The warning is part of the path made from the same inputs, so the same project gives the same warnings and the same G-code at any thread count (GC-07, NFR-06).

## Alternatives considered

- **Keep the refusal:** the program cannot be made at all for a face that is cut 3 micrometres too deep, which is what #226 reports.
- **Only the twist case a warning (option B):** a face usually has some of both causes, and the gate measures the total depth, not each cause. The owner chose A.
- **A warning up to a ceiling, an error beyond (option C):** it is safer against a gross overcut from a badly chosen tool, but it adds a second number the user has to find. The owner chose A. The warning gives the depth wherever the program is looked at.
- **A "Generate anyway" acknowledgement:** the owner chose no acknowledgement (#226).
- **A G-code comment with the tolerance:** a comment changes every program's bytes and its golden files for a value the setup sheet already carries beside the program. The setup sheet was chosen, as decision 0110 records traceability there. A comment can follow if the owner wants one.
- **Warnings only as notes, with a marker word in the text:** the host would have to parse sentences to find a warning and the setting it names. A typed warning keeps both exact.

## Verification

| What | Test |
| --- | --- |
| An overcut above the tolerance is made with one warning that names the face, the depth, the place, the tolerance, where to raise it, the other choices and the check; the warning is a note too; the default tolerance is 0.01 mm; a raised tolerance makes no warning (OP-07, OP-12, NFR-12) | `req_op_07_req_nfr_12_an_overcut_above_the_tolerance_is_a_warning_that_says_how_to_accept_it` (`openchuck-path` `tests/swarf.rs`) |
| The twisting helicoid is made with the warning naming "ruled but not developable" (the #226 case); the small twist has no warning | `req_op_07_a_helicoid_is_accepted_at_a_small_twist_and_refused_with_the_twist_named_at_a_large_one` (revised) |
| A drafted wall with a straight tool: the overcut alone is a warning naming the taper, and the material left beyond the limit is still refused | `req_op_07_a_draft_cone_needs_the_matching_taper_and_a_straight_tool_is_refused_naming_it` and `req_op_07_req_op_12_material_left_is_silent_within_the_tolerance_noted_within_the_limit_and_refused_beyond_it` (revised) |
| Through the application, the warning reaches the operation's path and notes | `req_op_07_req_nfr_12_a_swarf_overcut_above_the_tolerance_is_made_with_a_warning` (`openchuck-app` `operation.rs`) |
| The operation's state is a warning with the words, Messages lists it with "Show setting", the setting is named for the inspector, and an edit clears it with the path (UI-11, UI-12) | `req_op_07_req_nfr_12_a_path_made_with_a_warning_keeps_the_operation_in_warning` (`openchuck-app` `overview.rs`) |
| The checks before writing repeat it without blocking (GC-15) | `req_gc_15_req_nfr_12_a_path_made_with_a_warning_is_said_at_writing_and_does_not_block_it` |
| The setup sheet records each operation's tolerance (GC-12) | `req_gc_12_req_nfr_12_the_sheet_records_the_tolerance_each_operation_is_generated_with`, and the sheet's golden files |

## Consequences

- **Code:**
  - `openchuck-path`: `PathWarning`, the `warnings` fields, the swarf gate.
  - `openchuck-app`: `OperationPath` and `GroupInfo` warnings, the generation record, the overview, `MessageAction::ShowSetting`, `operation_warning_settings`, the export checks and the setup sheet's Tolerance column (`SetupSheet::new` takes the registry).
  - `openchuck-ui`: the "Show setting" link and the inspector mark.
- **Library API:** breaking; the workspace goes to 0.13.0, with changelog entries.
- **Golden files:** the setup sheet's (`testdata/golden/setup_sheet/small_project.html` and `.pdf`) gain the column. No G-code golden file changes: the `swarf` reference job has no overcut.
- **Documents:**
  - the work breakdown's lane row, the decision index, the open-items register;
  - the verification plan's OP-07 row (a helicoid of large pitch is made with a warning naming the twist, not refused), edited when this record is accepted;
  - the interface specification: the "Show setting" action in section 9 and the setup sheet's columns. These are proposed here and edited when this record is accepted.
- **Open items:** OI-630 is closed by the owner's answer (option A). OI-633: an overcut below the simulation's cell is shown only by the gate's warning, not by the deviation colours or histogram.
