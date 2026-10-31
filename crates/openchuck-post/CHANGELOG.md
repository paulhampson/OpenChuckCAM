# Changelog

## 0.29.0

- **Breaking family release (0.29.0, ENDFACE stage 3, decision 0239, accepted):** `openchuck_path::PathWarning` gains `requires_acknowledgement` and `Strategy::passes_in_zones` returns `StrategyOutput`, and adaptive spacing on for a new operation changes its output (decision 0239 item 13). No change in this crate; all family crates advance together under 0048.

## 0.28.0

- **Breaking family release (0.28.0, Z1AXIS, decision 0317):** `openchuck_types::machine::MachineProfile` gains the spindle-height fields and `ProfileField` three variants. No change in this crate; all family crates advance together under 0048. Project schema 22 and machine profile schema 7 in `openchuck-project`.

## 0.27.0

- **Breaking family release (0.27.0, BUG-SWARF4, decision 0272):** `openchuck_path::swarf::FacePath::overcut_warning` takes a checked `Clearing`. No change in this crate; all family crates advance together under 0048. No project-schema migration.

## 0.26.0

- Accept explicit clear pass-boundary metadata for nonmoving continuous-axis counter resets. Z1 rebases subsequent absolute words with existing signed/exact-360 semantics; rotate-only posts omit bookkeeping and add no rewind (accepted 0247, #337). Library family advances together to 0.26.0 under decision 0048.

## 0.23.0

- STEPALL-M release integration: optional host stage timing (types/path) and corrected XYZ rounding at the written rotary angle (post/replay, issue #324); retain main’s GC-17 block-time and rotary-boundary APIs. Library family advances together after 0.22.0.

## 0.22.0

- GC-17 (accepted decision 0222): minimum block-time profile/post fields and deterministic Report mode. The built-in Z1 uses the gate-4 measured 7 ms with window 32. Existing project copies remain Off, and Report leaves G-code unchanged. No Slow down or Merge.


All notable changes to `openchuck-post` are recorded here.

## Unreleased

- **Breaking family release (0.25.0, FACING, decision 0252):** `StrategyInput::facing` adds the optional whole-setup context. Host struct literals supply the new field; use `None` for other strategies and legacy whole-stock Facing. All family crates advance together under 0048. No project-schema migration.


- Breaking (0.24.0): `ConvertOptions` adds `short_blocks` and `feed_decimals`. Hosts use disabled defaults for legacy behaviour. `PostOutput`/`MachineProgram` return demand reports and an adjusted verification path. `PostOptions` accepts optional per-field overrides.

- Version 0.24.0 with the library family (GC-17, decisions 0222/0245): configurable short-block reporting and geometry-preserving slowdown; profile/post/report APIs extended. Legacy project copies retain Off.

- Correct XYZ at the written rotary angle before linear rounding, retaining the existing precision and replay error budget (issue #324).
- Version 0.21.0 with the library family (decision 0241): `PostJob` and `ConvertOptions` require `a_range` and work-frame `safe_z`; `Step::Unwind` is replaced by the non-moving `RotaryBoundary`. `Regeneration` keeps `canonical_toolpath`, and `Application::planned_files` takes the regeneration. Hosts connect canonical groups through `openchuck_path::program::connect_group` in file order before posting.

- Fix the Makera Z1's A-coordinate adjustment after `G92.4 A0 S0` at exactly +360 or -360 degrees (GC-04/09, decision 0241, #292). Firmware retains these values; following absolute A words now retain that offset too, avoiding an unintended full turn. The command remains a non-moving whole-turn counter reset, not a change to the saved work offset.
- Version 0.20.0 with the library family (`MachineProfile` in `openchuck-types` gains `min_spindle_rpm` and `max_spindle_rpm`, and `ProfileField` gains `SpindleRange`, breaking changes; decision 0231).
- Version 0.19.0 with the library family (`BrepEdge` in `openchuck-types` gains `curve`, a breaking change; decision 0227); no change to this crate's items.
- Version 0.18.0 with the library family (`PencilFinishing` in `openchuck-path` becomes a struct with a private field, no longer `Copy` or `Eq`, a breaking change; decision 0226); no change to this crate's items.
- Version 0.17.0 with the library family (`Tab` in `openchuck-types` gains `auto`, and `RotaryFrame::machine` is removed, breaking changes; decisions 0201 and 0202).
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- **Breaking (0.15.0):** `PostJob` and `ConvertOptions` have a new public field, `operation_spindle` (spindle speeds in rpm by the IR's operation tag; decision 0196, issue #249; TL-05, GC-14). An operation with an entry runs at it; any other at its tool's `ToolSetting::spindle_rpm`, as before. A bad entry is a `PostError::Setting` naming the operation. Code that builds a `PostJob` or `ConvertOptions` adds `operation_spindle: BTreeMap::new()`.
- Version 0.14.0 with the library family (`RegionConstraint` in `openchuck-path` gains `boundary_limits` and `watch`, a breaking change; decision 0176).
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.12.0 with the library family (`StrategyInput` in `openchuck-path` gains `keep_out`, a breaking change; decision 0161).
- Ready to publish to crates.io (decision 0138): an `include` list (the library sources and the files they read; the tests stay out of the package), keywords and categories, and a README that links the repository. No API change. The built-in posts are now in this crate's `posts/` folder (decision 0103), so the published crate carries them; their content and the posts' output are unchanged.
- Version 0.11.0 with the library family (`PostJob` in `openchuck-post` gains `generator`, a breaking change; decision 0151).
- **Breaking (0.11.0):** `PostJob` gains `generator`, the tool and its version (such as `OpenChuckCAM 0.11.0`) that both shipped posts write as a comment after the title, inside the `comments` option (NFR-16). A host passes the software version only, never a commit or a date, so one version always gives the same program (GC-07); a host that does not want the line passes `None`. Golden files hold `(OpenChuckCAM <version>)`, and `openchuck_testkit::golden::check` masks the version before it compares.
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- Version 0.9.0 with the library family (decision 0076). New: the template filter `feed` and `feed_str`, which write a feed with at least `decimals.f` places and at least `FEED_SIGNIFICANT_FIGURES` (three) significant figures, so a small F such as `4.237` is `F4.24`, not `F4.2`, and rounding is at most 0.5 % of a move's time. Both shipped posts write F with it; feeds of 100 and more are written as before.
- Version 0.8.0 with the library family (`Pass` in `openchuck-path` gains `rapids`, a breaking change; decision 0051).
- Version 0.7.0 with the library family (`Tool` gains `centre_cutting` and `Pass` gains `stock`, breaking changes; decision 0033).
- Version 0.6.0 with the library family (`PostJob` and `ConvertOptions` in `openchuck-post` gain `operation_names`, a breaking change; decision 0028).
- **Breaking (0.6.0):** `PostJob` and `ConvertOptions` gain `operation_names`, the display names the comments that name each operation use (GC-06, decision 0028). The comment now reads `operation <name>, tool T<n> <tool name>, about <s> s`.

- Version 0.5.0 with the library family (the B-rep face record in `openchuck-types` is a breaking change, OI-123).

- Crate created as a shell (work breakdown slice 0).
- Slice 6: neutral machine program (`to_machine_program`) and the named feed rules `linuxcnc`, `makera`, `inverse_time` and `contact`.
