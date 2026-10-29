# 0294: egui and eframe 0.37: widget roles, ids, the symbol font and the look of fields

- **Status:** accepted (29 Oct 2026). The project owner accepted this record on 29 Oct 2026; the acceptance was given to the merge manager session and relayed to lane EFRAME37 under [decision 0049](0049-relayed-acceptance-of-records.md). The owner had earlier chosen a separate lane for the migration (option (a) for Dependabot #131). It amends no accepted record; it follows the dependency policy in `docs/dependencies.md` ("fast-moving crates are upgraded deliberately, one family per pull request") and keeps the UI smoke setup of [decisions 0101](0101-ui-smoke-of-the-built-archives.md) and [0250](0250-ui-smoke-linux-leg-pinned-to-ubuntu-24-04.md) as it is.
- **Date:** 29 Oct 2026.
- **Lane:** EFRAME37, `claude/lane-eframe37`.
- **Requirements:** NFR-02 and NFR-03 (the platforms), NFR-09 (no network client), NFR-10 and NFR-14 (licences, justified dependencies), NFR-15 and NFR-18 (the log never records values), UI-05, UI-07, UI-14 and UI-16 (the interface tests this record keeps passing).
- **Numbers:** takes 0294 and OI-1224: the highest numbers on `main` and on every remote branch on 29 Oct 2026 were 0293 and OI-1223. No project schema number.

## Context

`eframe` 0.37.0 (with `egui` and `egui-wgpu` 0.37.0) leaves `wgpu` at 30.0.1. Bumping it on its own compiles with 21 errors and 90 warnings in `apps/openchuck-ui` (`openchuck-viewport` compiles unchanged), and after the mechanical edits 11 of the 509 interface tests failed. The causes are in the table below; they are not all renames.

## Decision

1. **Widget roles.** `WidgetType` is replaced by `egui::Role` (accesskit's), and `WidgetInfo.typ` by `.role`. The mapping: `Button`, `Link`, `Label` keep their names; `Checkbox` becomes `CheckBox`; `TextEdit` becomes `TextInput`; `DragValue` becomes `SpinButton`; `Other` becomes `Unknown`. `SelectableLabel` no longer exists: a choice among values is a `Button` that carries a selected state. The click log (`shell/actions.rs`, NFR-18) therefore ignores a `Button` with `selected` set, so it still logs a button, a menu item, a link or a check box by label and never a chosen value.
2. **Ids.** All 61 `Id::new` calls are `Id::unique`, which hashes exactly as `Id::new` did, so no id changes. Every one is a root id (`Area`, `Modal`, `Window::id`, memory and temp-data keys). The rule: a call that takes an `Id` is given `Id::unique(<literal or (literal, stable model id)>)`; a call that takes a salt (`push_id`, `id_salt`, `Panel::left/right/top/bottom`) is given the same plain literal or tuple, never an `Id`, a formatted string, an address or an index that reorders; both hash with fixed seeds, so ids are stable from frame to frame and from run to run. `IdSalt::new` is not used directly. `Ui::id` is `Ui::scope_id`.
3. **Panel state ids (tests only).** egui 0.37 keys a panel's saved state by its parent's scope id and the panel's salt. The tests that read it (`PanelState::load`) go through `Rig::panel_state`, which uses the root `Ui`'s scope id and, for the dock (shown inside the central panel), the scope id the shell records in test builds. The product does not read panel state back.
4. **Mouse wheel.** The tests' `Event::MouseWheel` carries `MouseWheelSource::Unknown`, which is what Wayland, X11 and Windows report.
5. **The symbol font.** egui 0.37 stopped bundling the fonts that drew ⏭ ⏮ ⏴ ⏵ ⏶ ⏷ ⏸ and ☰ (the playback buttons, the dock and "Show" chevrons, the drag handle); the glyph test (NFR-09, UI-16) caught it. `openchuck-ui` declares `egui` 0.37.0 directly with `default-features = false` and `features = ["monochrome_emoji_fonts"]`, which restores the two files egui 0.36 bundled: `NotoEmoji-Regular` (OFL-1.1) and `emoji-icon-font` (MIT). Both are data inside `epaint_default_fonts`, whose OFL-1.1 and Ubuntu-font-1.0 exception `deny.toml` already holds; no licence is added. The icons are unchanged, by the owner's decision: the owner accepted this result and dropped the idea of a new icon set. Not chosen: painter-drawn icons and another symbol font.
6. **Fields.** egui 0.37 pads a text field like a button (8 by 4 here) and always sets its text at the top of a box as tall as a button. `theme::single_line` and `theme::multi_line` make every field of the interface with the padding it had (4 and 2) and the text placed as the layout of its row places it (centred in a row that centres its widgets, at the top otherwise). The box is now as tall as a button (28), as egui 0.37 intends, so a field that was 18 high is 28 high; rows are 28 to 34 high (`theme::target::ROW_MIN`) and the layout tests pass unchanged. A `DragValue` and a `ComboBox` draw their own edits and are not touched.
7. **Colour swatches.** egui 0.37 raises a button to the style's minimum size and pads it, so the 18 by 18 colour swatches of the selection-set inspector came out 22 by 20; the swatch scopes its spacing to keep 18 by 18.
8. **Tab order in a drop-down.** The popup's scroll area is no longer a Tab stop in egui 0.37 (it senses drag only); the keyboard test (UI-14) presses Tab three times to reach the third choice where it pressed it four times. Every choice remains reachable by Tab, Space and Enter.
9. **Deprecations** are all fixed; none is allowed.
10. **No project, library or file format change.** The library family is untouched, so no `cargo-semver-checks` bump or crate changelog entry is needed.

## What the tests observed

| Change | Before the fix | After |
| --- | --- | --- |
| Selectable choice as `Button` | NFR-18 test: a chosen value was logged | not logged, chosen or not (test updated to build the case with `WidgetInfo::selected(Role::Button, ..)`) |
| Panel ids | 5 simulate tests found no dock or inspector state | pass through `Rig::panel_state` |
| Symbol glyphs | glyph test: 8 characters without a glyph | passes with `monochrome_emoji_fonts` |
| Swatches | 22 by 20, the swatch test found none of 18 by 18 | 18 by 18 |
| Fields | text 3 px above its label and 4 px further right than before (add-operation dialog: label and text centres equal on `main`, 3 apart on 0.37) | as on `main` |
| Drop-down Tab stops | helical finishing chosen with 4 Tabs no longer reached | reached with 3 |

No requirement threshold of a test was changed.

## Alternatives considered

- **`#[allow(deprecated)]`** for the old names: leaves the next egui bump to fail; refused by the owner.
- **egui's `experimental` theme API** to restore the field style globally: unstable API behind a feature; the per-field helper is small and in our code.
- **Painter-drawn icons or another symbol font**: not wanted by the owner; see item 5.

## Verification

`cargo test -p openchuck-ui -p openchuck-viewport`, `cargo clippy --workspace --all-targets`, `cargo fmt --all --check`, `cargo xtask deps licences records trace` and `cargo deny check` pass; the results are in the pull request. What CI cannot show: real glyph drawing on each operating system, copy and paste on Wayland (the clipboard crate changed), high-DPI layout, keyboard focus in a real window and input methods: OI-1224.

## Consequences and limits

- The binary carries the two emoji fonts again, as it did on 0.36.
- `egui` appears in `docs/dependencies.md` as a direct entry, and is upgraded with `eframe`.
- OI-1224 (native check) opens with this record.
