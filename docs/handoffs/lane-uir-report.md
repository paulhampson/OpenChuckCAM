# Lane UIR: review of the interface against the specification (wave 3)

7 November 2026. Base: `main` at `e869d93`, rechecked against `1b3db00` (#570 SAFETYNOTICE merged since; it changes no file in `apps/openchuck-ui` or `crates/openchuck-viewport`). This is the review and report phase only: no product code is changed. Public data only: the corpus cube in the test rig (`testdata/stl/cube_ascii.stl`) and `testdata/examples/baluster/baluster-project` in the real application. Nothing here says that generated G-code is safe to run (NFR-12). Wrote the probe G-code only to a temporary folder to check that the export dialog works by keyboard.

## How the review was done

1. **Documents read:** specification sections 3 to 16, UI-01 to UI-16, the verification plan's UI rows, the slice 8 and slice 22 sections of the work breakdown, the open UI items (OI-129, 173, 176, 334, 342, 345, 493, 523, 539, 540, 623, 773) and decisions 0114, 0117, 0123, 0127, 0131 and 0282.
2. **Test rig (headless, egui without a window, 1366 × 768 at 100 %):** a temporary probe module (`shell/uir_probe_tests.rs`, not committed; its source is on `claude/uir-shots`) drives the real `Shell` with keyboard events only:
   - Tab walks on every step, every setup inspector section and every dialog or window, recording each focus stop's role, name and rectangle;
   - a tooltip audit: for each focus stop, is a tooltip shown on hover, and is one shown on keyboard focus;
   - a keyboard-only run from import to export (below);
   - Esc on each dialog;
   - menus opened by keyboard.
3. **Real application:** release build, `Xvfb` 1600 × 900 with Mesa `llvmpipe` (Vulkan) and the `openbox` window manager. The window's client area is sized to 1366 × 768 at 100 %, opened on the baluster project, and stepped through every step and window with `xdotool`, screenshots taken with ImageMagick. Without a window manager the window gets no keys, as lane X8 found (decision 0131), so openbox was needed.
4. **Baseline:**
   - `cargo test -p openchuck-ui`: 708 passed, 0 failed, 1 ignored (the 708 include the 7 probe tests, which return at once unless `UIR_RUN` is set).
   - `cargo xtask deps`: ok.
   - `ui-smoke.yml` (the built archives on Windows, macOS and Linux): run 42353325351 on `main` (9f84345, 7 Nov) succeeded, as did runs 31 to 34 before it.

**Not checked:**
- Windows and macOS by hand.
- Native GPU frame time.
- Contrast ratios measured on a real screen.
- A real screen reader.
- The OS clipboard by hand.
- Several monitors.
- Scales other than 100 % on the real application (the existing layout tests cover 120 %).
- The light theme (P2).
- The `openchuck-app` test suite (I changed nothing in it).

## 1. Slice 22 review boxes

| Box | State | Evidence or what is missing |
| --- | --- | --- |
| Every screen, panel and dialog in the specification exists, matches it or has a recorded difference, and nothing the requirements need is missing | **Unticked** | Differences with no open item: "Simulate from" is not built (spec 3 and 11.5; recorded only as a note in decision 0114), the numpad view keys (spec 16), the New operation dialog's buttons (spec 5) and the shortcuts list (findings F6, F7, F9, F10). Still open: OI-334 and OI-342. |
| The UI imports no engine crate, and only `openchuck-ui` and `openchuck-viewport` use egui or wgpu | **Can be ticked** | `cargo xtask deps`: ok, on `e869d93`. |
| The UI smoke test passes on Windows, macOS and Linux (NFR-02) | **Can be ticked (software rendering)** | `ui-smoke.yml` run 42353325351 (7 Nov 2026, `main`) green, with earlier runs 31 to 34 also green. Native graphics is the slice 8 box below (OWNER). |
| The frame time on 1M triangles still meets NFR-03 on the reference hardware | **Unticked, OWNER** | OI-540: the owner's run with the reworked interface (and OI-173, OI-176). |
| Undo, redo and the inline errors behave as before (UI-08, UI-07) | **Can be ticked** | Whole `openchuck-ui` suite green, including the `req_ui_08_*` (26 names across the crates) and `req_ui_07_*` tests. |
| Anything the specification asks for beyond the requirements is listed as an open item or a requirement proposal, not built | **Unticked** | "Simulate from" and the numpad keys have no open item (F6, F9). Raise them and this can be ticked. |

**Slice 8 boxes that slice 22 builds:**

| Box | State | Evidence or what is missing |
| --- | --- | --- |
| Launches on Windows, macOS and Linux … native graphics on the reference machines | **Unticked, OWNER** | OI-129: native graphics on macOS, Linux and the reference machines are hands-on. |
| Every menu command has a shortcut or menu path, and a keyboard-only run reaches all five steps (UI-14) | **The run passes; tick once the run is a committed test** | The keyboard-only run below reaches import, Setup, Operations, generate, Simulate and Post and export, and writes the files. Every menu command has a menu path, and the menus open by Tab and Enter. Two caveats: UI-14 as a whole is only partly met (F1, F2, F5), and a few commands are not in any menu (F8). Proposed fix 0 commits the run as a test, which closes OI-539. |
| The reference screens at 1366 by 768 at 100 % scale have no overlapping overlays or clipped text (UI-16, NFR-15) | **Unticked** | Small overlaps and cut text, F12 (screenshots 01, 11, 13, 18, 26). OI-523 (Windows dock height) is the owner's. |
| The other slice 8 boxes | Ticked already | Rechecked: their tests pass. |

## 2. Requirement coverage, UI-01 to UI-16

"Tests" counts distinct test names that carry the ID, across `apps/` and `crates/`.

| Req | State | Basis |
| --- | --- | --- |
| UI-01 | Verified (software) | Smoke tests pass (13 tests). `ui-smoke.yml` green on three systems. Native launch: OWNER (OI-129). |
| UI-02 | Verified | 28 tests. Toolbar, views, projections and display modes seen on the real application. |
| UI-03 | Verified | 44 tests. Axis, WCS, stock, part and held region drawn together (screens 01, 11, 13). |
| UI-04 | Verified | 58 tests. Picking keys tested (`keys_tests`). Box selection is OI-201 (spec, not built). |
| UI-05 | Verified | 54 tests. |
| UI-06 | Verified | 17 tests. Tree rows are reached by Tab and selected with Enter. Alt ↑/↓, Ctrl D, Del and F2 are tested. |
| UI-07 | Verified | 105 tests. Hint layout nits in F12. |
| UI-08 | Verified | 26 tests, suite green. |
| UI-09 | Verified | 51 tests. Generation ran in the background on the real application. |
| UI-10 | Partly | 37 tests and the behaviour is right. A step's or a disabled button's reason is a hover tooltip only, so keyboard users get it only from the footer status (F2). "Simulate from" is not built (F9). |
| UI-11 | Verified | 88 tests. The legend stays visible (screens). |
| UI-12 | Verified | 61 tests. Messages, filters and actions reached by Tab. |
| UI-13 | Verified (two states, OI-345 open) | 16 tests. The provenance icons open their tooltip on focus (the tooltip audit confirms it). |
| UI-14 | **Partly** | 34 tests plus this review's run: every step, dialog and command can be reached and used by keyboard. Against the requirement: hover-only tooltips (F2), focus on hidden controls (F1), new windows not taking focus (F5), commands outside the menus (F8). |
| UI-15 | Verified | 32 tests. |
| UI-16 | Partly | 47 tests, panels collapse and resize. Small overlaps and cut text at 1366 × 768 (F12). Windows dock height: OI-523 (OWNER). |

NFR-15 (contrast, scale, colour, text size) is tested by token tests. Contrast on a real screen is the owner's (OI-623, OI-493).

## 3. Keyboard-only run (UI-14, OI-539)

**What worked**, all by keyboard alone through the test rig on the cube project, and on the real application where noted:

| Action | Keys | Result |
| --- | --- | --- |
| Open the steps | Alt 1 to Alt 4 | Each step opens |
| Post and export | Alt 5, Ctrl E | Opens the export dialog |
| Collapse and reopen the panels | Ctrl 1, Ctrl 2, Ctrl 3 | Each panel collapses and reopens |
| Import a model | Ctrl I; Tab to the path field, type the path, Enter (check); Tab to "Add to project", Enter | Model added: 1 → 2 models |
| Use the menus | Tab to the menu bar, Enter opens a menu, ↓ and Tab move through its items | Works |
| Use the tree | Tab to a row, Enter | Row selected; ↓ moves focus to the next row |
| Add an operation | N; Tab to Add, Enter | Operation added: 1 → 2 operations |
| Generate | Shift G | Generates every stale operation |
| Simulate | Tab to the footer's two actions, Enter | Works |
| Play the simulation | Space | Plays and pauses |
| Write G-code | Ctrl E; type the output folder; Tab to the acknowledgement, Space; Tab to "Write G-code", Enter | Files written (`01-setup-1.nc`, setup sheet html and pdf, in a temporary folder) |
| Result dialog | Tab | Cycles between Copy paths and Dismiss |
| Picking keys | Q, W, E, R, Esc | Existing tests pass |

**Where focus is lost, a control cannot be reached or used, the order is wrong, or a key is missing:**

| # | Where | What happens | Spec | Finding |
| --- | --- | --- | --- | --- |
| K1 | Inspector (operation, setup sections), export dialog, machine library | Tab moves focus to fields below the visible area and the panel does not scroll: a field at y 730–758 stays under the footer, and the export acknowledgement is at y 832 in a 768-high window. Nothing on screen shows where focus is. | 16 | F1 |
| K2 | Every step | Tooltips are shown on hover only. Of the focus stops that have a tooltip on hover, the number that also show it on focus: Model 0 of 33, Setup 0 of 19, Operations 4 of 44, Simulate 1 of 47, Setup machine section 6 of 28 (only `field_row::tip` rows and provenance icons). Among the hover-only tooltips are the step tabs and badge (the UI-10 reasons), the view toolbar, tree rows, dock filters, collapse chevrons, "n machine values unconfirmed" and the hide-toolpaths boxes. | 16, UI-14 | F2 |
| K3 | Post and export, Import, New operation, Tool library and Machine library windows (drawn inside the main window on Linux) | Opening one leaves keyboard focus behind it. The first Tab goes to the File menu (real application, screenshot 20), and about 58 to 60 Tabs pass through the main window before the dialog. | 16 | F5 |
| K4 | Whole window | The Tab order is not reading order. Main window: menus → health badge → workflow actions (right) → step tabs (left) → footer → project panel → inspector → dock → view toolbar → view. Dialogs: Cancel and Write before the body. | 16 | F4 |
| K5 | Tree rows, section headers, number boxes, the view, filter fields | A focus stop with no accessible name: 13 of 58 on Model, 30 of 99 on Operations, 20 of 87 on Simulate. The app is also built without AccessKit, so no screen reader sees any of it. | 16 | F3 |
| K6 | Views | Numpad 1, 3, 7 and 0 do nothing: egui 0.37 does not tell numpad keys from digit keys. | 16 | F6 |
| K7 | Modal dialogs (Keyboard shortcuts, About, Settings) and the export, import, New operation and machine library windows | Esc does not close them; only the tool library closes on Esc (OI-536). Every one can be closed by Tab to Close or Cancel. | (16 asks it only of the tool library and the export confirmation and result dialogs) | F11 |
| K8 | Menus | "Add setup" is only on the workflow bar, and deleting a selection set has only the Del key. There is no F10 or Alt key to reach the menu bar; Tab reaches it. | UI-14 | F8 |
| K9 | Help > Keyboard shortcuts | The dialog leaves out F, V, Space and the arrows (playback), Ctrl Enter (workholding), Ctrl + and Ctrl −, and Ctrl Shift Z. | 16 | F7 |

**Shortcut conflicts:** none found. Ctrl 1–3 and Alt 1–5 share digit keys but not modifiers, and the letter keys are left to a focused text field (tested).

## 4. Findings

No sev1 was found (nothing touching safety, data loss or wrong output). None duplicates an open issue: the 36 open issues were checked on 7 Nov 2026, and no UI-14, accessibility or layout issue is open. OI-539 and OI-493 say these things were not checked; they do not record these defects.

### F1, sev2: Tab focus goes to controls that are scrolled out of view

- **Breaks:** UI-14 and spec 16 ("every control can be reached and used with the keyboard"). A keyboard user edits a field they cannot see.
- **Repro:** cube or baluster project, Operations, select an operation, press Tab until focus is in the inspector's lower sections. The focused field's rectangle (y 730–758) is still under the footer ten frames later. Ctrl E, then Tab to the acknowledgement: it is at y 832 in a 768-high window.
- **Cause:** egui 0.37 scrolls to a widget only on an AccessKit request. The app calls `scroll_to_me` only for section headers and dock rows.
- **Fix:** small to medium. One shell-level pass that scrolls the newly focused widget into view (`Response::scroll_to_me` when `gained_focus`), in the inspector, the export dialog, the machine and tool libraries and the dock.
- **Test:** `req_ui_14_tab_scrolls_the_focused_control_into_view`.

### F2, sev2: tooltips are hover-only nearly everywhere

- **Breaks:** UI-14 ("anything shown on hover is also shown on keyboard focus") and spec 16. It hides the UI-10 "why unavailable" reasons from keyboard users, except where the footer repeats them.
- **Repro:** K2's counts. `apps/openchuck-ui/src` has 171 `on_hover_text`, `on_disabled_hover_text` and `on_hover_ui` calls and about 34 `field_row::tip` calls.
- **Caveat:** egui does not show any tooltip for a disabled widget that has focus, and disabled widgets take no focus. Where a reason is only on a disabled control, keep it in the footer status or in visible text.
- **Fix:** medium, mostly mechanical. A focus-aware helper, like `field_row::tip`, used everywhere `on_hover_text` is.
- **Test:** `req_ui_14_every_tooltip_also_opens_on_focus`, a Tab walk asserting that each stop with a hover tooltip shows it on focus.

### F3, sev2 (owner to confirm the severity): no screen-reader support, and many controls have no name

- **Breaks:** spec 16 ("egui's AccessKit support is used, so controls are exposed to screen readers"; "icon-only buttons have accessible names"). NFR-15's text does not mention screen readers, so this is a breach of the specification, not of a P1 requirement.
- **Repro:** `Cargo.toml` builds `eframe` with `default-features = false` and without its `accesskit` feature: `cargo tree -i accesskit_winit` finds nothing. K5's unnamed stops: tree rows, the section header toggles, the number boxes of the field rows, the 3D view, the tool library and export filter fields.
- **Fix:**
  - small: turn on `eframe/accesskit`, which adds `accesskit_winit` (MIT or Apache-2.0) and needs an NFR-14 note and a `cargo xtask licences` run;
  - medium: give each control its label (`labelled_by`, or `WidgetInfo` labels on custom rows).
- **Test:** `req_ui_14_every_focus_stop_has_a_name`. A real screen reader check is OWNER.

### F4, sev3: Tab order is not reading order

- **Breaks:** spec 16 ("in reading order").
- **Repro:** K4.
- **Fix:** medium. Draw order sets egui's focus order. Reorder the layout passes, which matters because right-to-left blocks are drawn first, or set the order explicitly.
- **Test:** `req_ui_14_tab_follows_reading_order`.

### F5, sev2: a window that opens does not take keyboard focus

- **Breaks:** UI-14 and spec 16. It works, but needs about 60 Tabs.
- **Repro:** K3, on the real application under X11 with a window manager (screenshot 20).
- **Fix:** small. Focus the window's first control when it opens; the machine library already outlines its first button.
- **Test:** `req_ui_14_a_new_window_takes_the_keyboard`.

### F6, sev3: numpad view keys not built

- **Breaks:** spec 16 (Numpad 1, 3, 7 and 0).
- **Cause:** egui 0.37 has no numpad key codes. Telling them apart needs `physical_key`, which winit may not report on every platform.
- **Proposal:** the owner chooses a spec change (other keys, or drop them) or an open item to try `physical_key`. No OI exists; decision 0117 deferred these keys to lane X3.

### F7, sev3: the Keyboard shortcuts dialog is incomplete

- **Breaks:** spec 16 and UI-14 (finding commands).
- **Repro:** K9, screenshot 25.
- **Fix:** small. Add the missing rows, ideally generated from the same list the shell uses.
- **Test:** `req_ui_14_the_shortcut_list_names_every_key`.

### F8, sev3: some commands are not in any menu

- **Breaks:** UI-14 ("every command is available from the menus").
- **Repro:** K8. "Add setup" (workflow bar and the tree "+"), deleting a selection set (Del only), Duplicate and Delete of anything but an operation.
- **Fix:** small. Add Project > Add setup and widen Edit > Delete.
- **Test:** `req_ui_14_every_command_is_in_a_menu`.

### F9, sev3: "Simulate from" not built, with no open item

- **Breaks:** spec 3 and 11.5 line 540. It is listed as "not built" in decision 0114 C2 and nowhere else.
- **Proposal:** add an open item, or remove it from the specification.

### F10, sev3: the New operation dialog breaks the primary-action rule

- **Breaks:** spec 5 ("Dialogs follow the rule … bottom right"; one filled primary).
- **Repro:** N on Operations. "Add" is not filled and sits bottom left beside Cancel (screenshot 28).
- **Fix:** small.
- **Test:** `req_ui_10_the_new_operation_dialog_has_its_primary_action_bottom_right`.

### F11, sev3: Esc closes only the tool library

- **Breaks:** consistency only. Spec 16 asks Esc to close only popovers and the tool library window, and spec 11.6 the export confirmation and result.
- **Repro:** K7.
- **Proposal:** Esc closes every dialog and tool window as it does the tool library (OI-536 pattern). Owner's choice.

### F12, sev3: layout nits at 1366 × 768

- **Breaks:** UI-16 and NFR-15 ("no overlapping overlays or clipped text").
- **What:**
  - a. A field's sentence that starts in the value column wraps back under the label: Model > Units "Read in mm, the default: the file / names no unit." (screenshot 26), and the workholding grip warning "⚠ The chuck grips / 7.000 mm of the part…" (screenshot 13).
  - b. Values cut short with "…": Machine > Tool length measurement "Automatic (too…" (screenshot 11), and the Holding diameter placeholder "Stock diamete…" (screenshot 13).
  - c. Setup front pane: the "Held 8.000" leader crosses the "Axis to safe Z" dimension, and "A axis" is written over the part outline (screenshots 11 and 13).
  - d. 3D view: the "A0" mark at the held end sits on the A+ arrow's label (screenshots 01 and 14).
  - e. On Simulate, the tree's last line is cut through the middle of its letters by the legend, with no scroll cue (screenshot 18 and its crop).
- **Fix:** small each.
- **Test:** extend the `inspector_layout_tests` and `frame_tests` overlap checks.

### Recommended order

1. **Fix 0:** commit the keyboard-only run as a test (`req_ui_14_a_keyboard_only_run_reaches_every_step_and_writes_g_code`). It ticks the slice 8 box and closes OI-539's "no test" part.
2. F1 and F5 (small, the biggest gain for keyboard use).
3. F2 (medium, mechanical).
4. F3 (enable AccessKit and name the controls).
5. F8, F7 and F10 (small).
6. F12 (small, ticks the UI-16 box).
7. F4 (medium).
8. Then the owner's decisions on F6, F9 and F11.

## 5. OWNER items

1. Launch the nightly archives on native graphics on Windows, macOS and Linux, on the reference machines (NFR-02, OI-129).
2. The NFR-03 frame time with the reworked interface on the reference hardware (OI-540; with OI-173 and OI-176).
3. Open Simulate at 1366 × 768 on Windows and confirm the dock height (OI-523).
4. Contrast, focus ring visibility and tooltips on a real screen (OI-623, OI-493, OI-987).
5. A real screen reader pass once F3 is fixed (NVDA or Narrator on Windows, VoiceOver on macOS, Orca on Linux).
6. Clipboard (Copy paths) and windows on a second monitor, by hand. Tool windows open inside the main window on this Linux build: the owner says whether they should open as separate OS windows on Windows and macOS (spec 3, "Dialogs").
7. Severity of F3, and decisions on F6, F9 and F11. Each will go to the coordinator as its own message.

## Screenshots

Branch `claude/uir-shots`, folder `docs/handoffs/uir-shots/`. Each is the client area at 1366 × 768, 100 %, llvmpipe, baluster example project:

- 01 Operations;
- 11 Setup machine;
- 13 Setup workholding;
- 14 the Rough operation;
- 16 after generating;
- 18 Simulate with the primary tooltip;
- 20 export after one Tab (focus on File behind the dialog);
- 23 tool library;
- 25 Keyboard shortcuts;
- 26 Model;
- 27 Import;
- 28 New operation;
- 31 Machine library;
- crop-tree-help (F12 e).

The probe logs (`probe_*.txt`) and the probe source (`uir_probe_tests.rs.txt`) are beside them.
