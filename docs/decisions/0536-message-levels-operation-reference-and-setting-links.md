# 0536: Message levels, the operation reference and links to the setting (lane MSGREVIEW; issue #567)

- **Status:** proposed (7 Nov 2026, lane MSGREVIEW). Only the project owner accepts it (decision 0049). Nothing here is built yet: issue #567 asks for the proposal first, and code follows, test first, only after the owner's answers.
  - **The owner's answer to question 1** (7 Nov 2026, relayed by the coordinator): the levels and the author's test are accepted in principle, with option B, so information is shown in the Results tab only and Messages keeps errors and warnings. Sections 1, 2 and 5 below follow that answer.
  - **The owner's clarification** (same message): a check whose finding may be a collision becomes a warning to acknowledge. Because that changes what the export does, it is part B of this record (below), which the owner accepts separately from part A (sections 1 to 7).
- **What it amends, if accepted:**
  - [decision 0175](0175-swarf-overcut-is-a-warning.md) items 2 and 3 (how a path warning and a note carry their level; "Show setting" opens one setting);
  - [decision 0430](0430-results-feed-limit-summary-and-copyable-results.md) item 1 and [decision 0519](0519-results-in-the-lower-dock.md) (the Results lines take each message's own level);
  - [decision 0522](0522-the-flute-check-warns.md) item 3 (its lead-in form, extended to every message);
  - interface specification sections 9 (Messages rows and action links) and 15 (wording). These edits are proposed here and made only after acceptance.

  The requirements and the architecture are unchanged.
- **Date:** 7 Nov 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-2094 to OI-2101 are new (OI-2100 and OI-2101 are part B).
- **The inventory** is in [docs/message-review/](../message-review/README.md): about 900 rows, one per message or family, read from `main` at `e869d932`.

## Context

The owner asked on 6 and 7 Nov 2026 (issue #567) for a review of every message the application shows, first the warnings and notes, then, on 7 Nov 2026, every kind of message, so that one level policy applies everywhere. The owner's rule:

- **Warning:** an aspect of the operation that may be a problem during machining, or cause unexpected behaviour.
- **Information:** the end goal is still achieved, but a compromise or decision was made to support it (for example a lower feed).
- Every message names its operation in the form of decision 0522 ("Flute length (operation '<name>', tool '<tool name>'): ..."), and is actionable where it can be, with a link to the relevant setting.

The constraints from #567 hold throughout:

- Nothing weakens the safety wording: no message says a path is safe or verified (NFR-12).
- Warnings that need acknowledging stay (GC-16).
- A level change does not change what a check does; changing a check's behaviour is a separate decision.
- Output stays deterministic (GC-07).

Related requirements: UI-11 and UI-12 (states and messages), UI-16 (Results), GC-15 and GC-16 (checks and acknowledgement before writing), OP-16 (notes), NFR-07 (clear errors), NFR-12.

## What the inventory found

The counts are rows per area. One message can appear in two areas (for example, a path warning in the path rows and in the UI rows). Details are in the area files.

| Area | Rows | Stay as they are | Move down to information | Move up to warning | Merge or remove | Wording or link only |
| --- | --- | --- | --- | --- | --- | --- |
| Path strategies ([path.md](../message-review/path.md)) | 19 warnings, about 45 note templates, about 235 refusal sites in 13 families | 19 warnings; every refusal | about 30 notes, 7 more marked "?" | about 15 notes that describe a real problem | 2 | refusals: internal names, no link |
| Setup, workholding, model, import, project ([setup.md](../message-review/setup.md)) | 114 | about 96 | 7 | 3 | 6 | most setup messages: no field link |
| Export, post, G-code check, simulation ([export.md](../message-review/export.md)) | 96 | about 89 | 3 | 1 marked "?" | 1 | post refusals lose the operation |
| Interface warnings and links ([ui.md](../message-review/ui.md)) | 121 | about 105 | 11 (overlap with the above) | 0 | 2 | "Show setting" |
| Interface: prompts, status, empty states, validation ([ui-other.md](../message-review/ui-other.md)) | about 180 | about 165 | 9 | 0 | 0 | lost status lines; error dialogs |
| Library errors ([errors.md](../message-review/errors.md)) | 103 | 98 | 0 | 0 | 0 | internal field names (OI-360) |
| Command line, setup sheet, diagnostics ([cli.md](../message-review/cli.md)) | about 116 | about 100 | 4 | 3 marked "?" | 0 | level words differ by command |

No warning in the Messages panel moves down. Every message proposed for information is today a note, shown as a warning in the Results tab and printed as `warning:` by the command line, where the goal is met. Examples are the ring and pass counts, the passes adaptive spacing added, a feed lowered to the post's maximum, and short blocks lengthened in Slow down mode.

The gaps that cut across areas:

1. **Notes have no level.** A strategy's notes are plain strings. The Results tab shows every one as a warning (`run_panel.rs`, `result_lines`), and so does the command line (`warning:`). Decision 0430's feed-limit summary is the only line marked as information. About 30 notes only describe what was done; about 15 describe a real problem but never reach the Messages panel or a link. Examples: holes left out, chains not cut, a scallop height ignored, material left beyond the tolerance, stock left round a tab, a rest operation with no earlier operation.
2. **Only two warnings name the operation in their text.** The flute-length and tab-length warnings use the 0522 form; every other warning relies on the panel row's source link. The Results lines and the command line use a third form, "operation 'X': move 0: ...". It skips the tool, and "move 0" is wrong for a warning, which belongs to no move. The export checks say the name twice for the flute and tab warnings.
3. **"Show setting" cannot reach the right field.** The action carries the operation, not the setting. It always marks the first setting any of the operation's warnings names, whichever warning was clicked. Only number and text rows can be marked, so the four warnings that name the tick box Face free ends mark nothing. Nothing outside an operation's own settings can be targeted: the tool, the setup's fields, the machine, a selection set, the post.
4. **The same fact is shown at different levels or more than once.** Examples:
   - a step-set or ramp refusal is an error in Messages but an amber line in the inspector;
   - a rapid that cuts is a warning ("mm3") and a problem ("mm³") in the same Results tab;
   - a tab shorter than the tool is a setup warning and an operation warning;
   - "the setup has no stock" is said three ways;
   - the Setup checks tick "raise no warnings" can sit beside failed rows.
5. **Errors say what failed but rarely what to change, and often in internal names.**
   - Validation errors read "invalid tool.flute_length: ...". More than 60 interface call sites show the error's raw text; the machine editor's mapping onto field labels is the one good pattern.
   - Project errors name items by their ID string, not their name.
   - Requirement numbers ("(SET-04)", "(PRJ-02)") appear in the interface.
   - A failed post shows as "Generating failed: cannot post the G-code: ..." with no operation, and the spindle refusal says "cannot post the G-code:" twice.
6. **Some texts reach nobody.** The shell's status line is written in many places but never drawn: "Recovered checkpoint. Save explicitly to keep these changes.", "Open or create a project before importing a model." and others (OI-2098).
7. **NFR-12 wording.** "Facing coverage complete (verified path geometry)" is shown in the Simulate results for every Facing operation. The Facing check's texts say "certify" and "certificate". "Good to go" and "No overcut" read as claims (OI-2097).
8. **The command line labels levels by command.** The same fact is `note:` in one command, `warning:` in another, or has no level word at all. Every run note is printed as `warning:`, one line per move for feed limits, where the interface shows one information line per operation (decision 0430).

Two findings change what a check does, not only its level, so they are kept out of this record and left to the owner as decisions of their own:

- **OI-2095:** workholding Apply refuses on findings that generation only warns of. `workholding::findings_of` marks every zone finding other than "tab outside the end zones" as refusing, including an automatic tab with no room and tabs that overlap.
- **OI-2096:** `openchuck-cli regenerate --out` writes the G-code files before it reports failed checks, and exits with an error leaving them in the folder. The export dialog does not write in that case (GC-15).

## Decision, part A (proposed): levels, form and links

### 1. The levels

| Level | When | What it does | How it looks |
| --- | --- | --- | --- |
| **Error** (a refusal) | The action cannot be done, or its output would not match what the project asks for; nothing is made. | Stops that action only (generate, apply, write, open). | Red triangle; the item's Error mark. |
| **Warning, to acknowledge** (GC-16) | Output is made but partial: the user accepts this result before its program is written. | The export dialog asks for the tick; the result is never called complete. | Amber triangle, with "Needs acknowledging". |
| **Warning** | Output is made, but something about it may be a problem during machining or cause unexpected behaviour (the owner's rule). | Nothing is blocked; the item keeps the Warning mark while the cause stays. | Amber triangle. |
| **Information** | The goal is still achieved, but a compromise or decision was made to get there (the owner's rule). | Nothing is blocked; the item's mark does not change. Shown in the Results tab only, not in Messages (the owner's answer to question 1). | Blue "i" in the Results tab; not counted on its label. |
| **Status** (not a message) | The state of an item or a job: stale, running, finalising, up to date. | Shown on the item's mark, its tooltip, the footer and the Jobs tab, not in Messages. | Marks of specification section 6. |
| **Confirmation** (not a message) | A question before the user's own action that cannot be undone or loses work. | Asks once and names what is lost; the default button is the one that loses nothing. | Dialog. |

### 2. The test an author applies to a new message

Ask in order; the first yes sets the level.

1. Is nothing made, or would what is made not match what the project asks for? **Error.**
2. Is the output made but partial, so the user must accept it before it is written? **Warning, to acknowledge.** If part B is accepted: is it a finding that may be a collision of the tool, its shank or its holder with the part, the stock, a tab or a held region? Also **warning, to acknowledge**.
3. Left unread, could the message lead to a problem on the machine, or to a result the user would not expect from their settings? Examples: material cut or left that the settings did not ask for, a limit passed, a machine value not confirmed, a part of the job skipped, a setting ignored. **Warning.**
4. Was the goal met, with a compromise or a choice the user may want to know? Examples: a slower feed, more passes, another entry, a link raised. **Information.**
5. Otherwise it is not a message: leave it out, or show it as status.

When an author cannot tell a warning from information, it is a warning (NFR-12 leans to the cautious side).

### 3. One form for every message

`<Topic> (<context>): <what happened>. <why, where it is not plain>. <what to change>.`

- **Topic:** a short name for the check in sentence case, the same wherever the message is shown: "Flute length", "Tab length", "Overcut", "Unreached areas", "Feed limit", "Short blocks", "Holes left out". This extends decision 0522's form to every message.
- **Context:** the items concerned, named as they are now (named when shown, not when made: decision 0522 item 3):
  - `operation '<name>'`, with `tool '<name>'` when the tool's size, flutes or number matter;
  - `setup '<name>'`, for a setup's stock, workholding or machine;
  - `model '<name>'`, `post '<name>'`, `tool '<name>'` or `machine '<name>'` for those items.
- **What to change:** a last sentence that starts with a verb and names the setting by the label the inspector shows ("Raise Tolerance", "Use a tool with longer flutes"). Where nothing the user sets would change it, it says what to check instead.
- **Plain words** (decision 0114; specification section 15):
  - no internal names, requirement numbers or decision numbers;
  - every number with its unit;
  - never "safe", "verified", "certified", "ready to run" or "machine-ready" (NFR-12).
- **The library crates do not know names.** They give the topic, the sentences and the setting key. The application adds the context when it shows the message, so a rename is followed.

### 4. The link to the setting

- A message that names one setting links to it. "Show setting" carries the setting's key, not only the operation. It opens the item, scrolls to the setting and marks it, for every kind of field: numbers, text, tick boxes and drop-down lists.
- New targets where the fix lies outside the operation:
  - the tool, in the tool library;
  - a setup's field (stock, clearance, machine);
  - a selection set (Reselect, as today);
  - the workholding (Edit workholding, as today);
  - the post, in the export dialog.
- A message with no setting to change keeps the source link only.

### 5. Where each level is shown

| Place | What it shows |
| --- | --- |
| **Messages panel** | Errors and warnings only, errors first (specification section 9), each with its topic-led text, source link and action link. A path note promoted to a warning is listed there too. Information is not shown in Messages (the owner's answer to question 1). The messages Messages shows as information today move to the Results tab or to their item: a model linked from outside the project folder (the model's row already says so) and a finished export (its report stays in the Results tab and the export dialog). |
| **Results tab** | Every level: errors, warnings and information, each line at its message's own level, with the chips Errors, Warnings and Information (specification section 9). A path warning is no longer repeated as a "move 0" note; it is one warning line in the 0522 form. Information is where a compromise is read, after each run. |
| **Operation mark and status chip** | Error or warning only. Information leaves the mark as it is. The inspector shows a refusal at the Error level, as Messages does. |
| **Export checks** | Blocking, to acknowledge, and warning. Information is not repeated. Each line names its operation once and links to it. |
| **Command line** | `error:`, `warning:` and `note:` (information) on standard error in every command, with the same text and context as the interface. A feed-limit summary replaces the per-move lines, as in the interface. |

### 6. What makes it hold

1. **One library type for what generation reports.** `openchuck-path` gets one type in place of `PathNote` and `PathWarning`. It carries:
   - the level (warning or information);
   - the topic;
   - the sentences;
   - the setting key, if there is one;
   - the acknowledgement flag, warnings only;
   - the move, where a note is about one.

   This breaks the library API. The workspace version goes from 0.48.0 to 0.49.0, with entries in the changelogs of `openchuck-path` and `openchuck-app` (decision 0048). The Merge Manager sequences the version with the other lanes.
2. **One constructor for messages in the application.** It builds a `Message` from the level, topic, context, sentences and link, and writes the lead-in. `Message::new` becomes private to that module. The interface and the command line both print from it.
3. **Checks that fail when the rule is broken:**
   - a unit test that builds the messages of a fixture project with one of each kind and checks each row against the form;
   - a `cargo xtask` check that fails on a message level (`Severity::`) built outside the module;
   - a test that no message text contains the words NFR-12 forbids.

   Each test is named after its requirement (for example `req_ui_12_every_message_names_its_item`).
4. **No project schema change.** Warnings and notes are kept only in memory (the pipeline cache) and in no project file. No golden G-code or reference file holds their text, and G-code output does not change (GC-07). Tests that assert message texts change with the wording.

### 7. How it is built, after acceptance

Each pull request is a draft, test first, pushed once at release (decision 0055).

1. The library type and the message constructor, with the checks of item 6, and the workspace minor version bump.
2. Path notes take their levels. The Results tab and the command line follow, and the notes that describe a real problem become warnings.
3. The lead-in for every message, and "Show setting" with the setting key for every field kind.
4. Setup, model, post and export messages: the form, the links, and the field labels in place of internal names (the machine editor's mapping, reused; OI-360).
5. Interface: the lost status lines (OI-2098), error dialogs that name the action that failed, recovery messages at their level, and the NFR-12 wording (OI-2097).
6. Part B, only if the owner accepts it: the acknowledgement for possible collisions, after step 1, whose library type carries the reason for a tick. It is its own pull request with the tests of B4.

## Decision, part B (proposed; a change of check behaviour, accepted separately): a possible collision is a warning to acknowledge

The owner's clarification of 7 Nov 2026, relayed with the answer to question 1, was this: checks that relate to a potential collision, such as the flute length check, should be upgraded to a warning requiring acknowledgement (the GC-16 level).

This changes what the export and the command line do, not only how a message looks. So it is kept apart from part A, has its own open item (OI-2100) and its own build step, and is accepted on its own. No existing acknowledgement is weakened: the partial-path tick of decision 0239 item 10 and the stock-simulation-skipped tick of decision 0168 stay as they are.

### B1. The rule

A finding that may be a collision of the tool, its shank or its holder with the part, the stock, a tab or a held region is a **warning to acknowledge**, not a plain warning.

That holds whether the check measured the collision or could not rule it out (stock of unknown size, a position that is not a number). A finding that only leaves material uncut, or that the strategy already avoided by changing the path, is not a collision.

### B2. The checks that qualify

"Yes" rows are proposed. "?" rows are for the owner (question B). "No" rows are listed so the reason is on record.

| Check | Where (`main` at `e869d932`) | Level today | Qualifies | Why |
| --- | --- | --- | --- | --- |
| Flute length: the stock stands above the flutes along the path (decisions 0509, 0522, 0524) | `openchuck-path` `flutes.rs:647-672` | warning, no tick | **yes** | the shank or holder may rub the stock |
| Flute length: the stock's size is not known, so the flutes could not be checked | `flutes.rs:640-645` | warning, no tick | **yes** | a collision cannot be ruled out |
| Flute length: a tool position is not a finite number | `flutes.rs:634-639` | warning, no tick | **yes** | as above; a defect, so it also says to regenerate |
| Flute length at a free end that is not faced (decision 0531) | `finishing.rs:296-311` | warning, no tick | no | the end is skipped, so the tool does not go there |
| Tab length: a tab shorter than the tool is wide (decisions 0494, 0523) | `openchuck-app` `workholding.rs` (`TAB_LENGTH_LEAD`) and the setup note at `workholding.rs:221-243` | warning, no tick (shown twice today) | **yes** | the tool may cut through the tab, and the part may come loose |
| Swarf cuts into a face next to it, at the end of its rulings or at an end of the pass (decision 0532) | `swarf.rs:907-953` | warning, no tick | **yes** | the side of the tool meets another face of the part |
| Swarf cuts into its own finished face beyond the tolerance (decision 0175) | `swarf.rs:859-890` | warning, no tick (the owner chose no tick in 0175) | **?** | a gouge, often the face's own error of hundredths of a millimetre, not a collision; the recommendation is to leave it a plain warning |
| A selection's check set is touched, or its avoid set cut, where the set only warns | `region.rs:441-462`, `swarf.rs:3082-3093` | warning, no tick | **?** | a check set often stands for a clamp or fixture; the recommendation is yes for a check set, and no for an avoid set (a gouge) |
| A free end faced down to the rotary axis with nothing holding the part on the other side (decisions 0344, 0500) | `finishing.rs:428-439` (and the stock cut free at `finishing.rs:390-403`) | warning, no tick | **?** | a part or stock piece cut free can be thrown into the tool; the recommendation is yes for the part cut free, no for the stock ring |
| A rapid that removes material, found by the simulation | `openchuck-app` `pipeline.rs:441`; Results line `run_panel.rs:205` | a note in the Results tab (and a "problem" line); not in Messages or the export | **yes** | a rapid through material is a crash. It is known only after a stock simulation; a project that skips it already ticks that (decision 0168) |
| The setup's safe cylinder does not enclose the stock, or the part | `openchuck-types` `setup.rs:466-475` | setup warning, no tick | **yes** | links at the clearance pass through material |
| The safe cylinder is inside the machine's minimum clearance | `setup.rs` (`ClearanceBelowMinimum`) | setup warning, no tick | **yes** | its own text says links may hit the chuck, jaws, headstock or tailstock |
| A tool may be too long to clear the workholding within the Z travel (decision 0318) | `openchuck-app` `setup.rs` (`spindle_height_text`) | setup warning, no tick | **yes** | the tool may meet the workholding |
| A tool may take the spindle above the top of the Z travel | same | setup warning, no tick | no | a travel limit, which the controller stops at; not a collision with the job |
| A point of the job is outside the machine's travel | `setup.rs` (`OutsideTravel`) | setup warning, no tick | no | a travel limit, as above |
| The shank is wider than the cutting part, so simultaneous finishing breaks the path and links round | `simultaneous.rs:1042-1053` | warning, no tick | no | the strategy avoided the collision; the warning says where it changed the path |
| Unreached areas, material no tool can reach, a selection that reaches a held region, links that could not be lowered | various | warning or note | no | material is left; the tool keeps clear |
| The simulation's overcut lines | `run_panel.rs` | a "problem" line | no | a gouge, measured after the fact; it stays where it is |

Entry, ramp, held-region and tab-zone refusals are errors already; nothing is made, so they are unchanged.

### B3. What changes for each check that qualifies

- **Export (GC-16).** Writing waits for a tick, as for a partial path today. The dialog lists each collision warning of the result with its operation or setup, and one tick acknowledges them for this result. Proposed label: "I have read the possible collisions listed for this result and will check them before running the program". It names nothing as safe (NFR-12). A new run asks again (decision 0239 item 10). The partial-path tick stays its own tick, with its own label.
- **Command line.** `regenerate --out` refuses to write without a flag (proposed: `--acknowledge-collision-warnings`), as it does for a partial path today. The refusal names each warning.
- **Operation and setup marks.** The Warning mark, with the cause starting "Needs acknowledging:".
- **Messages.** Warnings to acknowledge come first among the warnings, each with its link. The rapid cut, which today reaches only the Results tab, is listed on its operation while the simulation that found it is current.
- **Not "partial".** Today the only warning that needs a tick is a partial path, and the export reads every such warning as partial (`export.rs`, `partial_operations`). The library type of part A, item 6.1, carries the reason for the tick: partial, or a possible collision. Only a partial path is "not complete". Setup warnings take their reason from their kind in the application.
- **Output.** G-code is unchanged, byte for byte, with or without the tick (GC-07).

### B4. Test plan

Each test is named after its requirement, written first and watched fail.

- **For each check marked yes:** `req_gc_16_<check>_needs_acknowledging`, for example `req_gc_16_flute_length_needs_acknowledging`. Each test checks four things:
  - the export refuses to write without the tick, and writes with it;
  - the written files are byte-identical to a run with no warning (GC-07);
  - the command line refuses without the flag and writes with it;
  - the operation's or setup's mark reads "Needs acknowledging".
- `req_gc_16_partial_path_acknowledgement_is_unchanged`: a partial path still asks its own tick, is still called partial, and a collision warning is never called partial.
- `req_gc_16_a_warning_that_is_not_a_collision_asks_no_tick`: an unreached-areas warning is still a plain warning.
- `req_ui_12_warnings_to_acknowledge_come_first`: the order in Messages.

## Alternatives considered

- **Classify in the application only, by matching message text.** This needs no library change. But every new message must match a prefix list, a reworded message silently changes level, and the command line and the interface could disagree. Rejected as fragile.
- **Write a convention and rely on review.** This costs nothing now, but the inventory shows that conventions drift. The 0522 form has been applied twice in three weeks, and four lead-in styles exist. Rejected as the main mechanism; the written rule stays as guidance.
- **Show information in Messages as well as the Results tab** (question 1, option A). A compromise would be seen beside its operation as soon as it is generated, at the cost of a longer Messages list. Not chosen: the owner chose option B (information in the Results tab only), so Messages stays a list of what needs attention.
- **Keep today's levels and change only the wording and links** (question 1, option C). Not chosen.

## Consequences

- Documents, after acceptance, in the pull requests that build each part:
  - the interface specification, sections 6 (marks), 9 (Messages: the action links, the sources, information) and 15 (wording);
  - the verification plan's entries for UI-12 and UI-16;
  - the changelogs.
- Message texts change in the interface and on the command line. No project file, cache key or G-code changes.
- OI-360 (internal names in library errors) is taken up by build step 4 and closes when it is done.
- OI-373 (Results lines belong in Messages) is not closed, and the owner's answer to question 1 narrows it: warnings and errors reach Messages, while information belongs in the Results tab by design. The information rows Messages shows today move out of it (section 5).
- Part B, if accepted, changes what the export and the command line do for the checks it lists. The export asks for a tick before writing, and the command line needs a flag. Its test plan is in part B.

## Open items

| ID | Item |
| --- | --- |
| OI-2094 | The owner's answers to this record: the level policy, the one form, the links and the mechanism. |
| OI-2095 | Workholding Apply refuses on findings that generation only warns of (an automatic tab with no room, tabs that overlap). This changes a check's behaviour, so it is a separate decision. |
| OI-2096 | The command line writes G-code before it reports failed checks, and leaves the files when it exits with an error. This changes behaviour, so it is a separate decision. |
| OI-2097 | NFR-12 wording to remove: "verified path geometry", "certify", "certificate", "Good to go", "No overcut". |
| OI-2098 | The shell's status line is never drawn, so its confirmations and refusals reach nobody. Where should they go? |
| OI-2100 | Part B: the owner's acceptance of the rule that a possible collision is a warning to acknowledge, and the answers to its "?" rows (the swarf overcut into its own face, check and avoid sets that only warn, a part cut free at a free end). |
| OI-2099 | Outside this record unless the owner says otherwise: the command-line help text, the setup sheet (it shows no warnings; GC-12 does not ask for them), and the log, About and crash texts, which keep the log crate's levels. |
| OI-2101 | Part B's tests in the verification plan: the GC-16 entry gains the collision acknowledgements of B4, when part B is built. |
