# 0148: Post and export in the File menu, the View menu and the view directions (lane MV)

- **Status:** proposed. The owner's choices below were given in the trial feedback (issue 140) and relayed by the coordinator session on 20 Oct 2026 (items 1 to 3, owner decisions 1 and 2 of the lane); the owner accepts the record in the lane MV session.
- **Date:** 20 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-435 (closed), OI-436 and OI-437 (open).

Numbering: the coordinator assigned this lane records 0148 and 0149 and OI-435 to OI-439; this record uses OI-435 to OI-437 and record 0149 holds the gadget and cube.

## Context

GitHub issues 140, 141 and 143 (trial findings F-02, F-03, F-05). Requirements: UI-02 (standard views), UI-10 and UI-14 (steps, every command in the menus), UI-15 (scale kept per user), UI-16 and NFR-15 (1366 × 768 at 100%, 80% to 200%, legibility). Specification sections 3 and 7.1 and decision 0105.

## Decision

1. **File > Post and export is enabled when the project has a model, a setup, a machine, at least one tool and at least one operation** (owner decision A on issue 140). Otherwise it is greyed out with a tooltip naming the first missing item in that order: "Import a model first", "Add a setup first", "Choose a machine first", "Add a tool first", "Add an operation first". Operations need not be generated first: the export dialog makes its own files and keeps its own blockers (output folder, the verification box, an empty toolpath; `write_blocker`). The rule is `Overview::post_blocker`; Ctrl E follows it. No G-code is produced by opening the dialog (NFR-12 unaffected). A tool is still required for each operation while lane X9 moves the tools out of the tree (UI-06).
2. **One rule for the view directions** (owner decision 1 of the lane, option A). Every 3D view has the same Front, Top and Iso selector on the viewport toolbar, and all seven directions in View > View direction and on the view cube. Setup's two fixed panes (front from −Y, end from +X; section 11.3) have no camera to turn, so no direction applies there: the View direction items are greyed out with "Setup shows fixed front and end views"; with no model they are greyed with "Import a model first". The rule is one function, `viewport_toolbar::directions_unavailable`, used by the menu and the toolbar. Specification 7.1 changes from "Model, Setup" to "every 3D view; not Setup's panes".
3. **The View menu is in submenus**: the steps, Panels (Project, Inspector, Dock), View direction, Projection, Display, Fit all and Scale. The Scale submenu offers 80, 90, 100, 110 and 120%, ticking the current one. **The default scale for new users becomes 100%** (owner decision 2 of the lane, option B; it reopens OI-166, closed 15 Oct 2026 at 120%, and closes it again as OI-435). A scale already saved is kept, also one outside the list (which shows no tick); Ctrl plus and Ctrl minus keep working in the 80% to 200% range of NFR-15; "100 %" in the menu is the reset. Larger, Smaller and Reset are removed. `DEFAULT_SCALE` in `openchuck-app` (not a library-family crate) is 1.0; the saved setting is the existing settings-file field, so there is no project format change.

## Alternatives considered

- **Keep 120% as the default (option A of the decision).** Rejected by the owner: the menus were too long at 1920 × 1080, and at 100% a 1366 × 768 screen needs no change of scale.
- **Migrate every user to 100%.** Rejected: it overrides a saved choice.
- **Front and End shortcuts in Setup, or a selector that does nothing there.** Rejected: extra work for little value, and a dead control.

## Consequences

- Code: `Overview::post_blocker` (`openchuck-app`); `Shell::post_blocker` and `open_export`; the File and View menus of `Shell::menu` in `shell/frame.rs`; `viewport_toolbar::directions_unavailable`; `DEFAULT_SCALE`.
- Specification sections 3 (scale, smallest window), 7.1 and the OI-166 line are edited in this change.
- Tests: `req_ui_10_req_ui_14_post_and_export_names_the_first_thing_missing`, `req_ui_10_req_ui_14_post_and_export_opens_once_the_project_has_what_it_needs`, `req_ui_02_the_view_directions_apply_to_every_3d_view_and_not_to_the_setup_panes`, `req_nfr_15_req_ui_16_the_scale_menu_offers_fixed_steps_within_the_range`, and the updated settings tests (`req_ui_15_*`).
