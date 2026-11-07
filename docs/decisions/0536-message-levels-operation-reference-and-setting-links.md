# 0536: Message levels, the operation reference and links to the setting (lane MSGREVIEW; issue #567)

- **Status:** accepted by the project owner on 7 Nov 2026, relayed by the coordinator (decision 0049), after the owner read it in full. Proposed by lane MSGREVIEW on 7 Nov 2026. Nothing here is built yet; code follows, test first. Section 8 (the lost status lines, with the work-loss reminders as warnings in Messages) is the lane's application of the author's test, not an explicit owner answer: it stands as accepted with the record, and the build pull request for step 5 points it out so the owner can change it.
  - **The owner's answer to question 1** (7 Nov 2026, relayed by the coordinator): the levels and the author's test are accepted in principle, with option B, so information is shown in the Results tab only and Messages keeps errors and warnings. Sections 1, 2 and 5 below follow that answer.
  - **The owner's clarification** (same message): a check whose finding may be a collision becomes a warning to acknowledge. That changes what the export does, so it is [decision 0538](0538-a-possible-collision-is-a-warning-to-acknowledge.md) (proposed), accepted separately.
  - **The owner's answers to questions 2 to 7** (7 Nov 2026, relayed by the coordinator), each the recommended option:
    - 2, A: the one form for every message and every item (section 3).
    - 3, A: a link keyed to the setting, for every field kind, with the new targets tool, setup field and post (section 4).
    - 4, A: one level-carrying library type, one message constructor and the checks, with a library minor version bump (section 6).
    - 5, A: workholding Apply stores and warns on an automatic tab with no room and on tabs that overlap. That is [decision 0539](0539-workholding-apply-warns-on-a-tab-with-no-room-or-tabs-that-overlap.md) (proposed).
    - 6, A: the command line runs the checks before it writes, and prints the export dialog's warnings. That is [decision 0540](0540-the-command-line-checks-before-it-writes.md) (proposed).
    - 7, A: the lost status lines are split by kind, with each level set by the author's test (section 8).

  Decisions 0538, 0539 and 0540 change a check's behaviour and are accepted on their own.
- **What it amends, if accepted:**
  - [decision 0175](0175-swarf-overcut-is-a-warning.md) items 2 and 3 (how a path warning and a note carry their level; "Show setting" opens one setting);
  - [decision 0430](0430-results-feed-limit-summary-and-copyable-results.md) item 1 and [decision 0519](0519-results-in-the-lower-dock.md) (the Results lines take each message's own level);
  - [decision 0522](0522-the-flute-check-warns.md) item 3 (its lead-in form, extended to every message);
  - interface specification sections 9 (Messages rows and action links) and 15 (wording). These edits are proposed here and made only after acceptance.

  The requirements and the architecture are unchanged.
- **Date:** 7 Nov 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-2094 to OI-2101 are new (OI-2100 and OI-2101 belong to decision 0538).
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

Two findings change what a check does, not only its level, so they are kept out of this record as decisions of their own, on the owner's answers to questions 5 and 6:

- **OI-2095** is [decision 0539](0539-workholding-apply-warns-on-a-tab-with-no-room-or-tabs-that-overlap.md). Workholding Apply refuses on findings that generation only warns of: `workholding::findings_of` marks every zone finding other than "tab outside the end zones" as refusing, including an automatic tab with no room and tabs that overlap.
- **OI-2096** is [decision 0540](0540-the-command-line-checks-before-it-writes.md). `openchuck-cli regenerate --out` writes the G-code files before it reports failed checks, and exits with an error leaving them in the folder. The export dialog does not write in that case (GC-15).

## Decision (proposed): levels, form and links

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
2. Is the output made but partial, so the user must accept it before it is written? **Warning, to acknowledge.** If decision 0538 is accepted: is it a finding that may be a collision of the tool, its shank or its holder with the part, the stock, a tab or a held region? Also **warning, to acknowledge**.
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

   This breaks the library API. The workspace version goes up one minor version (0.49.0 on `main` today, so 0.50.0 is expected, or the next free minor version at release), with entries in the changelogs of `openchuck-path` and `openchuck-app` (decision 0048). The Merge Manager sequences the version with the other lanes.
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
6. Decisions 0538, 0539 and 0540, each as its own pull request with its own tests, once the owner accepts it. 0538 comes after step 1, because it uses the library type. 0539 and 0540 can come at any time.

### 8. The lost status lines (the owner's answer to question 7)

The status line of the interface shell is never drawn (OI-2098). Each text it holds is placed by the author's test of section 2. Information is shown in the Results tab only (question 1), and a reminder that loses work if it is missed is not information.

| Text today | Where | Test | Level and place |
| --- | --- | --- | --- |
| "Recovered checkpoint. Save explicitly to keep these changes." | `shell.rs:965` | unread, the recovered work is lost at close (unexpected behaviour) | **Warning** in Messages, on the project, with the action "Save", until the project is saved or the recovered changes are undone |
| "Tool numbers changed. Undo with Ctrl Z; save the project to keep them." | `export_panel.rs:690` | the program just written uses the new numbers; unsaved, the project reopens with the old ones and the next program differs from the tool table (a machining problem) | **Warning** in Messages, on the tools, with the actions "Save" and "Undo", until saved or undone |
| "Open or create a project before importing a model." | `shell.rs:2067` | the command did nothing | **Error dialog**: a command the user gave failed outright (specification section 9) |
| "The selected geometry changed; select it again." | `shell/pending.rs:44` | the pick was dropped, so nothing was made | **Error** in Messages on the selection set, with "Reselect", until it is picked again, as for the other selection-set problems |
| "Saved …", "Opened …", "Added …", "Copied n models into the project folder" | `shell.rs:556-557, 597, 820, 2047` | no compromise and no consequence | **Not a message**: left out. The title bar shows the save state, and the tree shows what was added |
| Tool library and machine library lines ("Removed …", "Imported into …") | their windows | shown in the window's own line already | unchanged |

The shell's unused status field is removed with them. The recovery notices in Messages stay warnings (losing recovery data is unexpected behaviour), except a checkpoint that could not be written, which is an error because the action failed.

## Behaviour changes, in records of their own

- [Decision 0538](0538-a-possible-collision-is-a-warning-to-acknowledge.md): a possible collision is a warning to acknowledge (the owner's clarification with question 1).
- [Decision 0539](0539-workholding-apply-warns-on-a-tab-with-no-room-or-tabs-that-overlap.md): workholding Apply warns on an automatic tab with no room and on tabs that overlap (question 5).
- [Decision 0540](0540-the-command-line-checks-before-it-writes.md): the command line checks before it writes (question 6).

The author's test of section 2 names 0538.

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
- Decisions 0538, 0539 and 0540 change what the export, workholding Apply and the command line do. Each has its own test plan and is accepted on its own.

## Open items

| ID | Item |
| --- | --- |
| OI-2094 | The owner's answers to this record: the level policy, the one form, the links and the mechanism. Closed on 7 Nov 2026: answered (questions 1 to 7) and the record accepted. |
| OI-2095 | Workholding Apply refuses on findings that generation only warns of (an automatic tab with no room, tabs that overlap): decision 0539. |
| OI-2096 | The command line writes G-code before it reports failed checks, and leaves the files when it exits with an error: decision 0540. |
| OI-2097 | NFR-12 wording to remove: "verified path geometry", "certify", "certificate", "Good to go", "No overcut". |
| OI-2098 | The shell's status line is never drawn, so its confirmations and refusals reach nobody. Answered (question 7, A): section 8; built in step 5. |
| OI-2099 | Outside this record unless the owner says otherwise: the command-line help text, the setup sheet (it shows no warnings; GC-12 does not ask for them), and the log, About and crash texts, which keep the log crate's levels. |
| OI-2100 | Decision 0538: the owner's acceptance of the rule that a possible collision is a warning to acknowledge, its "?" rows answered on 7 Nov 2026 (questions 8 to 10). Closed on 7 Nov 2026: decision 0538 accepted. |
| OI-2101 | Decision 0538's tests in the verification plan: the GC-16 entry gains the collision acknowledgements, when 0538 is built. |
