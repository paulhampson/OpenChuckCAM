# 0157: The user-action log (lane UAL; issue 156; requirement NFR-18; OI-476 to OI-480)

- **Status:** accepted by the project owner (20 Oct 2026); the acceptance was relayed to the lane UAL session by the merge manager at the owner's instruction (decision 0049). The owner agreed the requirement wording, the choices of content and switch, and the place of the switch (item 5; all relayed by the coordinator session).
- **Date:** 20 Oct 2026.
- **Open items:** OI-476 (closed: NFR-18), OI-477 (what counts as an action), OI-478 (where the switch is, closed), OI-479 (labels), OI-480 (later windows). OI-457 (the seam) is closed by this record.

Numbering: 0157 and 0158 were assigned to lane UAL by the coordinator session.

## Context

Trial finding F-15 (issue 156): a bug report is easier to act on when the log shows the steps taken. [Decision 0155](0155-local-log-file.md) built the log file and left a seam: `logging::line` for the text and `Sink::write_line` for the file (OI-457). The owner agreed the requirement **NFR-18** (P2) and answered the open questions: content is action names, item kinds and names and parameter names, never values; the recording is on by default with one switch in Settings to turn it off.

## Decision

1. **A line is one action, in the log's own layout.** `[  12.345 INFO  action] <text>`, written by `logging::action` through `line`, to the file only (not standard error: it is not a diagnostic for the terminal). Nothing is added to the `log` facade, `RUST_LOG` does not affect it, and the rotation and folder choice are 0155's.
2. **One place per source, none sprinkled through the handlers.**
   - *Commands, undo and redo*: inside `Application::execute`, `undo` and `redo` (`openchuck-app`), because every one of the 44 call sites in the UI goes through them. The application keeps the lines (bounded to 256 undrained) and the front end takes them with `Application::drain_actions`; `openchuck-app` logs nothing itself and gains no dependency (architecture section 7: libraries do not log). The text is built by `action_log::describe` from the command and the project it is applied to: `command: SetParameter operation 'Rough 1' parameter='stepover'`, `undo: Remove setup 'Setup 2'`.
   - *Buttons, menu items and links, and check boxes*: read once per frame from egui's output events (`OutputEvent::Clicked`) at the end of `Shell::draw`, by the widget's label: `click: 'Save'`. This covers every menu, toolbar and dialog button without a call in each handler.
   - *Dialogs and windows*: the set open at the end of each frame is compared with the last, giving `window opened: About` and `window closed: About` (dialogs, the import window, the export window).
   - *Jobs*: in `overview_frame`, where the job events are read: `job started: Generating`, `job done|failed|cancelled: Generating`, for the jobs the Jobs list shows only (the quick background jobs that run most frames are left out). A failed job's message is not logged.
3. **Names, never values.** A command line names the command, the kind and name of the item and the parameter's name. It never holds a parameter value, a geometry value, a typed text (a rename's new name, a project name) or file contents. Clicks are logged for buttons, links and check boxes only, by label (truncated to 60 characters); text fields, drag values, sliders, radio buttons, selectable choices and combo boxes are never logged because what they show is a value. A button whose label carries a value would log it (OI-479; none is known). Item names are the user's own names (a setup, tool, operation or selection set) and a model's file name within the project.
4. **Not recorded (OI-477).** Selection and hover, camera moves, scrolling, typed text, and quick background jobs. Selection awaits lane BUG-SEL's design.
5. **The switch.** `UserSettings::record_actions` (`openchuck-app`, `settings.toml` `[interface]` table), `true` by default. It is **not a project format change**: the file has no version number and a file written before the field existed reads as on (serde default), so it is additive. The shell sets the process-wide flag at start and when the user changes it; with it off nothing is written and the application still collects and drops its lines (bounded). The control is a checkbox "Record my actions in the log" in a small Settings dialog with a sentence saying what is recorded and the log's path (OI-478): the UI had no settings window, and one checkbox did not justify a larger one; the owner decided the place: a "Settings…" item in the File menu (before Quit) and the same item in the Help menu, both opening the same dialog, two added items in `menu()`. The specification's section 13 gains a line.
6. **Local only.** The lines go to the same local file; no network client is added (NFR-09).

## Alternatives considered

- **Log calls in each menu and button handler**: the owner asked for a single place; it also misses what the next lane adds.
- **The `log` facade with a target `action`**: it would write to standard error as well and obey `RUST_LOG`; the action log is not a terminal diagnostic and must not vanish with `RUST_LOG=off`.
- **Logging the clicked widget's value for choices** (so "Climb" shows): that is a parameter value; the owner excluded values. The command line for the resulting edit still names the parameter.
- **A callback observer in `Application`**: needs `Debug` and lifetime plumbing; a drained buffer is the same shape as `drain_events`.

## Consequences

- Tests, named `req_nfr_18_*`: the text of every kind of command and that no value appears; the application's lines for command, undo and redo, and the bound; the settings default, round trip and a file without the field; click text for buttons and its refusal for values; window and job lines; the Settings dialog switching the flag and keeping the choice. P2: no mandatory CI test.
- Windows and macOS behave as Linux; nothing is platform-specific.
- `docs/requirements.md`, the verification plan, the specification (section 13), the work breakdown, the register and the decisions index are updated in the same pull request.
