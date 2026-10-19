# 0117: Slice 22 shell frame: the view-model queries for steps, item states, messages and the primary action, and the window's regions

- **Status:** proposed. Lane X2, slice 22 ([decision 0114](0114-slice-22-conformance-report.md) section E, accepted). The owner answered its two questions on 19 Oct 2026, relayed by the coordinator (decision 0049): OI-350 A, the four readings agreed and written into the specification; OI-351 B, a run up to the selected operation (item 9). The record itself waits for the owner's acceptance.
- **Date:** 18 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-350 and OI-351 were raised and are closed by the owner's answers. OI-342 stays open: this lane re-rendered images 01 and 02, and other lanes' boards remain.

Numbering: the coordinator assigned this lane records 0117 and 0118 and OI-350 to OI-354. Lane BX-b holds 0115 and 0116 and OI-347 to OI-349, and lane X3 holds 0119 and 0120 and OI-355 to OI-359. This lane uses 0117, OI-350 and OI-351. Record 0118 and OI-352 to OI-354 are unused.

## Context

Decision 0114 found that the layout of specification sections 3 to 5, 9 and 10 was not built (C1 to C4, C11 to C13, C21, and the shell parts of C24). That layout is slice 8 items 8, 11 and 12: UI-10, UI-11, UI-12, UI-15 and UI-16. The owner chose that slice 22's lanes build it (OI-338). Lane X2 builds the frame and the view-model queries it needs in `openchuck-app`. Lanes X4 to X6 and X9 build on those queries, and lane X7b words what they show. The specification requires the interface to hold no CAM logic: every value it shows comes from a view model in `openchuck-app` (specification section 1; architecture section 5.8).

## Decision

### 1. The overview: one query for the state of the job (UI-10, UI-11, UI-12)

`openchuck_app::overview` is a new module. It has two calls, so nothing slow runs on the interface's thread (UI-09, NFR-04):

- **`Application::start_checks(previous: Option<Arc<Checks>>) -> JobHandle<Checks>`** is a job ("project checks") that reads what needs the files:
  - whether each model file still matches its recorded hash;
  - each model's mesh check, unit question and notes;
  - each setup's checks against the placed part (SET-05, SET-06, the tab zones and tab widths of OI-180 and OI-159), and whether its stored workholding can be built;
  - for each operation whose setup holds something, whether its selections reach a held region or a tab (11.7).

  It reuses from `previous` every model whose reference and file stamp (size and modification time) have not changed, and every reach note whose operation's own key has not changed. After an ordinary edit, the job therefore reads no model file.
- **`Application::overview(checks: Option<&Checks>, jobs: &JobLog) -> Overview`** is quick and pure. It combines the project, the latest checks, what generation has recorded and the job log. `Overview` holds:
  - `steps`: the five steps, each with `unavailable: Option<String>` (the reason) and `complete`;
  - `items`: an `ItemStatus { state, cause, progress }` per `ItemRef`;
  - `messages`: severity, text, source and source name, and action;
  - `setup_checks`: the Setup checks tab's rows;
  - `stale`: the stale enabled operations;
  - `machine_warnings`: the footer's machine warnings;
  - `checks_current`: whether the checks are of this revision.

  `Overview::primary(step)` gives the footer's primary action (item 3). `Overview::no_project()` gives the state with no project open. `Overview::push` adds a message the application cannot see, such as a note from the view.
- **`Application::revision()`** changes with each command, undo and redo. A front end restarts the checks, and works the overview out again, when the revision, the checks or the job log change. The shell does that once per change, not once per frame.
- **Smaller additions:**
  - `overview::setup_row(project, setup, part)` gives the words and chip of a setup's child row ("Stock · cylinder Ø26 × 32", "G54", "1 held · 2 tabs");
  - `Application::operation_tolerance(op)` gives the footer's tolerance;
  - `Application::record_export(run)` records that G-code was written from a run;
  - `Application::cancel_job(id)` cancels any running job by its ID.

**`ItemRef`** names a model, a setup, a setup part (`SetupPart`: Machine, Stock, PartPosition, WorkOffset, Workholding), a selection set, a tool or an operation. The setup parts are the setup's child rows in the tree and the sections of its inspector (specification sections 6, 11.3 and 11.7). Lane X4 opens a section from `SetupPart`.

### 2. When an operation is up to date (UI-11)

- **The status key.** An operation is up to date when the latest successful generation made it from inputs that hash the same as its inputs now. The hash is the pipeline cache's key ([decision 0030](0030-slice-9-pipeline-cache.md) item 1), chained from the enabled operations before it. One difference: model files are taken at their recorded hashes, so the key reads no file.
- **What the key gives.** Editing operation N makes N and every later enabled operation stale. Undoing the edit makes them up to date again, because their inputs are again the ones they were made from.
- **Two keys per operation.** Each operation has a chained key and its own key. The own key names which operation changed in the stale mark's tooltip ("Stale: operation 2 changed").
- **The generation record.** The application keeps what generation has made. Each pipeline job updates it, whether started by Generate or by the export dialog:
  - **success** records every enabled operation's keys, and its simulation keys when it simulated;
  - **failure** records the operation it was working on and the message;
  - **cancellation** records nothing.
- **States, by item:**
  - **Operation:** up to date, stale ("not generated yet" when it was never made), running (the operation the pipeline is generating, with the job's progress), error or off.
  - **Operation error.** Its selection set has a problem ("Problems go up the tree"), or its recorded failure still holds: the failure's key equals its key now. The error clears as soon as the cause is edited, without generating again.
  - **Selection set:** an error when it has a problem.
  - **Setup part:** a warning from the setup check that concerns it. Stock covers no stock, bounding-box stock without a part, and the part outside the stock. Work offset and clearance covers clearance and travel. Workholding covers the tab zones and tab widths. Machine profile covers values not confirmed on the machine (D9).
  - **Workholding:** an error while the stored workholding cannot be built (11.7).
  - **Setup:** the worst of its parts.
  - **Model:** an error when its file is missing or unreadable. A warning when the file changed, the mesh check found something, or the unit is in question. Running while the view is built.
  - **Tool:** no state.

### 3. Steps and the primary action (UI-10; specification sections 4 and 5)

- **Availability follows section 4:**
  - Model: always.
  - Setup: a model is in the project.
  - Operations: the project has a machine and a setup has stock.
  - Simulate and Post and export: an enabled operation has been generated.
- **Complete (the green ring).** The specification did not define it. The lane's reading, agreed by the owner (OI-350) and now in specification section 4:
  - Model: there is a model and none is in error.
  - Setup: there is a machine, every setup has stock and a part, and no setup check warns.
  - Operations: every enabled operation is up to date.
  - Simulate: every enabled operation was simulated as it is now.
  - Post and export: G-code was written from every enabled operation as it is now.
- **The primary action** follows section 5:
  - Model: "Import model…" with no model, else "Continue to setup".
  - Setup: "Continue to operations".
  - Operations and Simulate: rules 1 to 4; on Simulate rule 3 is checked before rule 2, since simulating generates (agreed by the owner, OI-350, now in specification section 5). A disabled primary carries the reason as its tooltip and in the footer, and the index of the message it points at.
  - Post and export: "Post and export…", which opens the dialog.
- **What blocks the primary (rule 1):**
  - an error on a model blocks Continue to setup;
  - an error on an enabled operation, or on a set an enabled operation uses, blocks generating, simulating and export.

### 4. Messages (UI-12; specification section 9)

- **Sources:**
  - changed, missing and unreadable model files;
  - mesh findings and unit questions;
  - the import notes, as information;
  - setup checks and the machine missing;
  - generation failures, on their operation, or with no operation for a failure after the operations (posting);
  - selection-set problems;
  - selections reaching held regions or tabs;
  - a stored workholding that cannot be built;
  - failed jobs: building the view, the project checks, adding a model.
- **Each message** has a plain sentence, its source item and name, and an action: Show, Reselect, Review setup, Edit workholding or Generate.
- **Clearing.** A message is worked out from the current state, so it goes when its cause is fixed. A job failure clears when a later job of its kind succeeds.
- **Order.** Messages run errors first, then warnings, then information. Within a severity they come in a fixed order of their sources, from the latest kind of news to the earliest: job failures, generation, selection sets, setups, models. They are worked out from the state and carry no time, so "newest first" cannot be applied as written; agreed by the owner (OI-350) and now in specification section 9.
- **The model-file modal goes.** The modal list of changed model files on opening a project is gone. Those files are messages now (decision 0114 C11).

### 5. The job log and the Jobs tab (UI-09)

- **`openchuck_app::JobLog`** folds the event stream into running jobs (name, the user's words, progress, start) and up to 20 finished ones (outcome, time, a summary such as "Generated 2, reused 1"). Quick interactive jobs (picking a region, the stock at a move) are not listed. The project checks are listed only when they fail.
- **One drain.** The shell drains the events once per frame into the log. The run panel and the overview read progress from it.
- **Cancelled jobs.** `Event::JobFinished` gains `cancelled`.
- **Cancel by ID.** The job manager keeps the running jobs' cancel tokens, so the Jobs tab can cancel any job by its ID.

### 6. The window (UI-16; specification sections 3, 10 and 11.1)

**Regions and bars.**
- **Regions:**
  - the title bar (32 pixels): the menus File, Edit, View, Project, Tools and Help; the project name with "· saved" or a grey dot and "Edited"; Undo and Redo;
  - the workflow bar (44 pixels): the steps; on the right, Import model, New operation and Duplicate, by step;
  - the project panel (264, 200 to 400) with the legend pinned;
  - the inspector (340, 280 to 520), always present;
  - the dock (196; 120 to half the height) under the view;
  - the footer (48 pixels);
  - the 44-pixel viewport toolbar strip.
- **The steps.** Each is a numbered circle and a name. The active step has an accent underline and a filled circle; a complete step has a green ring; an unavailable step is dimmed, and its tooltip gives the reason. Alt 1 to Alt 5 open a step only when it is available.
- **The footer:**
  - left: the job with a spinner and its progress, or "Ready"; the step's note, or the reason the primary is disabled; "Workholding changes not applied" while a draft is unapplied; and "Verify G-code before running it" (D2);
  - right: "mm · deg" (D4), the tolerance of the selected operation, the machine warnings as links to Setup, Cancel while a job runs, and the primary button.
- **The empty state.** With no project or no model, the view shows the card "Start with a model". It offers "Import model…", or "New project…" when no project is open, and "Open project…", with the five steps one per line. Every tree group says "None yet". Recent projects and dropping a file are UI-23, P2.

**Collapsing and the small window.**
- **Collapsing.** The chevrons and Ctrl 1, 2 and 3 collapse a region to a 28-pixel rail. Double-clicking a splitter resets its region to the default size.
- **The small-window rule (OI-350, agreed and now in specification section 3).** Below 1366 × 768 logical pixels, the dock collapses. If the viewport would then be narrower than 420 pixels, the inspector collapses too. Neither is stored as the user's choice, and the user can open either again until the window is large again.

**The tree (UI-06, UI-11; section 6).**
- **Groups and rows.** The groups run Model, Setups (with five child rows), Selection sets, Tools and Operations. Rows are 28 pixels high, with the status mark, a swatch or the tool number, and a chip. Operations have a drag handle and an enable box.
- **The "+" buttons.** Model opens the import dialog. Selection sets starts picking with the dock on Selection sets. Operations opens the new-operation dialog. The Setups "+" is hidden until a second setup exists (SET-07, P2), and the Tools "+" until the tool library window exists (lane X9).
- **The row menu:** Rename, Duplicate, Enable or Disable, Move earlier and later, Delete, Show in view, and Generate.
- **Keys:** Ctrl D, Del, F2, and Alt ↑ and ↓.

**The inspector and the dock.**
- **The setup's sections.** Choosing a setup shows its five sections. Machine is the machine panel, which no longer has a tree row of its own. Workholding and tabs is the workholding panel. Stock, Part position, and Work offset and clearance show their row's words until lane X4 builds their fields (C15).
- **The selection sets** move from the inspector to the dock's Selection sets tab, and playback to its Playback tab on Simulate. A set's inspector shows its size, problem and uses.

**Keys** (shell parts of C24, UI-14):
- Ctrl N, Ctrl O and Ctrl I.
- Alt 1 to 5 and Ctrl 1 to 3.
- N.
- G and F5 generate the selected operation, up to and including it (item 9); with no operation selected they generate every stale one. Shift G and Shift F5 generate every stale operation. F5 and Shift F5 were Generate and Generate and simulate; Generate and simulate is now in the Project menu and is the Simulate step's primary.
- Ctrl D, Del, F2, and Alt ↑ and ↓.
- Esc cancels the running job when nothing is being picked.
- Help > Keyboard shortcuts lists them.
- F, V, the numpad views, and Space and the arrows for playback come with the toolbar and the playback dock (lanes X3 and X6).

**The rest of the window.**
- **The viewport toolbar.** `toolbar::strip` and `toolbar::legend` (a new file, `apps/openchuck-ui/src/toolbar.rs`) are the toolbar strip and the legend strip. For now they hold the View menu's controls and no legend. Lane X3 owns their contents and replaces their bodies; the View menu keeps every command (UI-14).
- **Modal errors** are narrowed to commands that fail outright (section 9): opening, creating, saving, renaming and deleting.
- **The scale** is clamped to 80% to 200% (C1). View > Larger, Smaller and Reset stay; egui's own Ctrl + and − also stay in the range.

### 7. Per-user settings (UI-15; specification section 13)

- **The module.** `openchuck_app::settings` reads and writes the `[interface]` table of the user's `settings.toml`, the file the export acknowledgements use, and keeps the file's other tables.
- **What it holds:**
  - the scale;
  - the theme (dark; light is UI-20, P2);
  - the units ("mm");
  - each region's size and whether it is collapsed;
  - up to 10 recent projects (shown by UI-23, P2; kept now because UI-15 asks for them);
  - the open inspector sections by item kind.
- **Bad values.** A missing file gives the defaults. A file or a table that cannot be read gives the defaults and a message (NFR-07). Values out of range are brought into range.
- **When it is written.** The shell writes the file a second after a change, and on quitting. A failure to write is a message, not a dialog. Nothing is written to the project.

### 8. Tokens

The frame draws with lane X7a's dark tokens (`theme::DARK`, specification 14.1), merged before this lane's pull request. It does not install X7a's style for the whole interface: applying the tokens to the existing panels is lane X7b's work.

### 9. Generating up to the selected operation (OI-351 B, the owner's choice)

- **`Application::start_regenerate_until(operation)`**, and `regenerate_until` which waits, generate the enabled operations in machining order up to and including `operation`, and stop. The operations before it are reused from the cache where nothing they read changed, because it cuts the stock they leave. The operations after it are left as they were. A disabled operation is refused, with "it is turned off", and an unknown one is refused too.
- **The result** carries `stopped_after: Some(operation)` and holds only the groups it made. It neither simulates nor posts.
- **States.** The generation record takes only the operations in the run, so the later ones stay stale. Each recorded operation also keeps the own keys of the operations before it when it was made, so a stale operation's cause names the earlier operation that changed ("Stale: operation 1 changed"), even after a run that remade that operation.
- **Export.** The export check treats an operation that is not in the latest run as out of date (GC-15), so a partial run never lets a later operation through.
- **The footer and the Jobs tab.** The primary stays "Generate stale (n)" (rule 2). The Jobs tab's summary of a partial run ends "stopped after operation N, so later operations stay stale".
- **Keys and menus.** G and F5, the row menu's Generate and Project > Generate selected use it. Shift G, Shift F5 and Project > Generate stale generate every stale operation. Specification section 16 stays as written.
- **Checks.** A run up to an operation is the same at 1, 2 and N threads. A full run afterwards reuses every operation the partial run made, so no cache key changed. Tests: `req_ui_14_req_ui_11_generating_up_to_an_operation_leaves_later_ones_stale`, `req_ui_14_generating_up_to_a_disabled_or_unknown_operation_is_refused`, `req_gc_15_a_partial_run_leaves_later_operations_blocking_at_export`, `req_gc_07_req_nfr_06_a_run_up_to_an_operation_is_the_same_at_1_2_and_n_threads`, and the shell's `req_ui_14_g_generates_the_selected_operation_and_shift_g_every_stale_one`.
- **Floating windows and keys.** The shell's letter keys are no longer blocked by a floating window such as the results, which opens after every run; only a text field with the keyboard, or a modal dialog, takes them.

## Alternatives considered

- **Keep staleness in the interface** by comparing the whole project with the one the latest run was made from, as the run panel did. It cannot say which operations are stale, so editing operation N would mark every operation. Rejected.
- **Work the overview out every frame.** The status keys hash every enabled operation's inputs, including its selection sets, which can be large. Rejected: the shell works it out when the revision, the checks or the job log change.
- **One job per check kind,** for example a job for model files and a job for setups. That means more handles and more ordering between them. The single checks job reuses its previous results, which gives the same cost.
- **Record the failing operation in the error type** (a wrapping `AppError` variant). That changes the variant the command line and the tests match on. Rejected for the generation record.
- **Hide the steps' unavailable reasons in a status line.** The specification asks for the reason on the step itself (tooltip) and on the disabled primary action. Built as specified.

## Consequences

- **`openchuck-app`** gains:
  - the `overview` and `settings` modules;
  - `JobLog`, `JobEntry`, `JobOutcome` and `job_label`;
  - `Application::revision`, `start_checks`, `overview`, `record_export`, `cancel_job`, `operation_tolerance`, `start_regenerate_until` and `regenerate_until`;
  - `Regeneration::stopped_after`, and the export check's treatment of operations a run did not make;
  - `JobManager::cancel`;
  - the `cancelled` field of `Event::JobFinished`.

  It is an application crate, not in the library family, so `cargo-semver-checks` does not apply.
- **`openchuck-ui`:**
  - `shell.rs` is split into `shell/frame.rs`, `shell/tree.rs`, `shell/dock.rs` and `shell/marks.rs`;
  - `toolbar.rs` is new;
  - the run panel reads progress from the job log, and its "generated" and "out of date" badges are replaced by the tree marks;
  - the selection panel gains `poll`, so a save finishes while its tab is closed;
  - the export panel records a write;
  - `main.rs` starts at the user's scale.
- **No project format change** and no engine change (decision 0105 item 4).
- **Mock-ups** (OI-342). Images 01 and 02 and their sources are re-rendered:
  - the setup row carries the Warning mark (D9);
  - Setup has the Workholding row and the fifth section;
  - both footers carry "Ready", a step note, the short G-code notice and "mm · deg".
- **For the other lanes:**
  - X3 fills `toolbar.rs`;
  - X4 builds the inspector sections on `SetupPart`, `Overview::status` and the messages;
  - X6 reworks the Selection sets and Playback tabs where the shell now places them;
  - X7b words the messages and causes;
  - X9 adds the Tools "+" (`tree::Group::Tools`).
- **The work breakdown:** the lane X2 row, slice 8 items 8, 11 and 12, and their review boxes for UI-10, UI-11 and UI-12, and UI-15.
