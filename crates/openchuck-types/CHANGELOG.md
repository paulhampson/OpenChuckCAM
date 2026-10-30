# Changelog

## 0.28.0

- **Breaking (0.28.0, Z1AXIS, decision 0317):** `machine::MachineProfile` has three new public fields: `automatic_tool_length` (`bool`, the controller measures each tool on a tool setter), `spindle_to_axis` (`Option<Millimetres>`, from the spindle at the top of the Z travel down to the rotary axis) and `recommended_min_safe_z` (`Option<Millimetres>`, advice, never a limit). `ProfileField` gains `ToolLengthMeasurement`, `SpindleToAxis` and `RecommendedMinSafeZ` (`ProfileField::ALL` has 14 entries); `fields` lists the first two when the machine measures tool lengths and the third when it is given. `validate` refuses a value that is not above zero. Profiles without them read with none and write them only when set. Struct literals add `automatic_tool_length: false, spindle_to_axis: None, recommended_min_safe_z: None`.

## 0.27.0

- **Breaking family release (0.27.0, BUG-SWARF4, decision 0272):** `openchuck_path::swarf::FacePath::overcut_warning` takes a checked `Clearing`. No change in this crate; all family crates advance together under 0048. No project-schema migration.

## 0.26.0

- **Breaking (0.26.0):** `ToolpathGroup` carries explicit `rotary_reset_after` segment indices for certified clear helical-pass boundaries (accepted 0247, #337). Host struct literals supply `Vec::new()` unless they certify eligible boundaries; copies retain the indices. Empty metadata defaults on deserialization and is omitted on serialization. Library family advances together to 0.26.0 under decision 0048.

## 0.23.0

- STEPALL-M release integration: optional host stage timing (types/path) and corrected XYZ rounding at the written rotary angle (post/replay, issue #324); retain main’s GC-17 block-time and rotary-boundary APIs. Library family advances together after 0.22.0.

## 0.22.0

- GC-17 (accepted decision 0222): minimum block-time profile/post fields and deterministic Report mode. The built-in Z1 uses the gate-4 measured 7 ms with window 32. Existing project copies remain Off, and Report leaves G-code unchanged. No Slow down or Merge.


All notable changes to `openchuck-types` are recorded here.

## Unreleased

- **Breaking family release (0.25.0, FACING, decision 0252):** `StrategyInput::facing` adds the optional whole-setup context. Host struct literals supply the new field; use `None` for other strategies and legacy whole-stock Facing. All family crates advance together under 0048. No project-schema migration.


- Breaking (0.24.0): `ShortBlockMode` adds `SlowDown`; the profile window uses `usize` and is bounded to 4096. Confirmation lists omit disabled timing settings and the software mode policy.

- Version 0.24.0 with the library family (GC-17, decisions 0222/0245): configurable short-block reporting and geometry-preserving slowdown; profile/post/report APIs extended. Legacy project copies retain Off.

- Add optional host stage timing through ProgressSink and JobContext; disabled by default.
- Version 0.21.0 with the library family (decision 0241): `PostJob` and `ConvertOptions` require `a_range` and work-frame `safe_z`; `Step::Unwind` is replaced by the non-moving `RotaryBoundary`. `Regeneration` keeps `canonical_toolpath`, and `Application::planned_files` takes the regeneration. Hosts connect canonical groups through `openchuck_path::program::connect_group` in file order before posting.

- **Breaking (0.20.0):** `machine::MachineProfile` has two new public fields, `min_spindle_rpm` and `max_spindle_rpm` (`Option<SpindleSpeed>`), the spindle speed range the machine runs at, a limit and never a default; `ProfileField` gains `SpindleRange` (`ProfileField::ALL` has eight entries), and `unconfirmed_fields` lists it only when the profile gives a range. New: `machine::SpindleLimit`, `MachineProfile::spindle_limit` and `MachineProfile::fields` (the fields the profile gives a value for; the CLI counts those). `validate` refuses a value that is not above 0, and a lowest speed not below the highest (decision 0231, OI-754; GC-14, NFR-12). Profile files without them read with no range.
- **Breaking (0.19.0):** `brep::BrepEdge` has a new public field, `curve: Option<EdgeCurve>`, the exact curve under the edge (decision 0227, issue 257; OP-08, OI-229). New: `brep::EdgeCurve` (a line, circle, ellipse or `BSplineCurve`), `brep::BSplineCurve`; `BrepModel::validate` checks the curve. Models written before the field read as `None`.
- Version 0.18.0 with the library family (`PencilFinishing` in `openchuck-path` becomes a struct with a private field, no longer `Copy` or `Eq`, a breaking change; decision 0226); no change to this crate's items.
- Lane FOLLOW1 (OI-841; OP-21): `setup::SetupWarningKind::TabsOverlap`, a tab placed automatically that lies over a tab placed by hand; `workholding::check_tab_zones` reports it.
- **Breaking (0.17.0):** a setup's part offset moves only the part (decision 0201, issue #175; SET-05). `kinematics::RotaryFrame::machine` is removed: every setup's machine frame is `RotaryFrame::of_profile`, with the model origin at the rotary reference point, and the stock, the workholding, the work zero and the toolpaths are fixed in it. `setup::SetupFrame::place_part` and `moves_part`, and `setup::PartExtent::in_setup`, place the part in a setup; `expected_work_offset` no longer adds the part offset; `SetupCheck::part` is the part as the setup places it.
- **Breaking (0.17.0):** `workholding::Tab` has a new public field, `auto: Option<TabEnd>`, a tab placed automatically at the head or the tail of the part (decision 0202, issues #182 and #186; OP-21). New: `TabEnd`, `MIN_TAB_ROOM`, `auto_tab_span`, `Workholding::resolved` and `Workholding::auto_tabs_without_room`, and `setup::SetupWarningKind::AutoTabNoRoom`; `check_tab_zones` places automatic tabs first, and `validate_workholding` does not hold an automatic tab's stored position against the stock.
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- Version 0.15.0 with the library family (decision 0196, issue #249; TL-05). New: `tool::material_key` and `tool::find_cutting_data`, which match a stock material to a tool's cutting data ignoring case and repeated spaces (an exact name wins), and `CuttingData::cutting_feed_at`, the cutting feed at another spindle speed. `tool::validate_entry` now refuses two cutting data entries whose names differ only by case or spacing. `CuttingTable` is no longer matched exactly. Library files that hold both "Oak" and "oak" on one tool are refused with a message naming both; delete one.
- Version 0.14.0 with the library family (`RegionConstraint` in `openchuck-path` gains `boundary_limits` and `watch`, a breaking change; decision 0176).
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.12.0 with the library family (`StrategyInput` in `openchuck-path` gains `keep_out`, a breaking change; decision 0161).
- Ready to publish to crates.io (decision 0138): an `include` list (the library sources and the files they read; the tests stay out of the package) and a README that links the repository. No API change.
- Version 0.11.0 with the library family (`PostJob` in `openchuck-post` gains `generator`, a breaking change; decision 0151).
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- **Breaking (0.9.0):** `machine::MaxFeeds` has a new public field, `rotary_in_feed` (`RotaryInFeed`: `Separate`, the default, or `Makera`), which says how the controller counts A in the feed word of a move that also moves X, Y or Z, so path assembly can keep that word within the limit (OP-13; decision 0076). Profile files without it read as `Separate`.
- Version 0.8.0 with the library family (`Pass` in `openchuck-path` gains `rapids`, a breaking change; decision 0051).
- Version 0.7.0 with the library family (`Tool` gains `centre_cutting` and `Pass` gains `stock`, breaking changes; decision 0033).
- **Breaking (0.7.0):** `tool::Tool` has a new public field, `centre_cutting: Option<bool>` (TL-04, OI-117; project schema 8), and `Tool::can_plunge()` (decision 0033).
- Version 0.6.0 with the library family (`PostJob` and `ConvertOptions` in `openchuck-post` gain `operation_names`, a breaking change; decision 0028).
- **Breaking (0.5.0):** the B-rep face record (slice 11a, decisions 0017 and 0027). `brep::BrepFace` gains `body`, `geometry` (a `Surface`: plane, cylinder, cone, sphere, torus with a `Frame`, or a clamped rational `BSplinePatch`), `reversed`, `uv_bounds` and `trims`; `BrepEdge` gains `vertices`; `BrepModel` gains `vertices` and `tessellation` (`TessellationSettings`). New: `VertexId`, `BrepVertex`, `Frame`, `UvPoint`, `TrimLoop`, `BrepModel::validate` and `joined_tessellation`. The family version moves from 0.4.0 to 0.5.0 for it, as the semver check requires (OI-123).

- Slice 7 (decision 0025): the `simulation` module, `SimulationSettings` (default cell 0.1 mm, finest 0.025, coarsest 1.0) with its relative time and memory.
- **Breaking (0.4.0):** `machine::MachineProfile` has a new public field, `max_tool_number` (the highest tool number the controller accepts, 1000 by default, `DEFAULT_MAX_TOOL_NUMBER`; GC-14, decision 0020 item 12). Profile files without it read as 1000.
- **Breaking (0.3.0):** `tool::Tool` replaces `stickout` with `overall_length` and gains `flute_count` and `material` (TL-04); `tool` is now a module directory. Slice 4 (decision 0014): `tool::profile` validates a `ToolProfile` by the monotone rule into a `RevolvedProfile` (lower envelope `height_at`, envelope pieces with a convexity flag, `is_convex`, `point_for_normal`) and gives `arc_centre` for arcs given by radius (decision 0012); `Tool::profile` maps every standard type to the profile and checks its values; `ControllerMapping` (TL-06), `CuttingData` and `CuttingTable` (TL-05), `validate_entry`; `units::SpindleSpeed`.
- **Breaking (0.2.0):** `machine::Clearance` is a struct (safe cylinder radius and safe Z) instead of an enum, `MachineProfile` has a new public field, `provenance`, and `MaxFeeds` gains `z` and `rapid_z`. Slice 3 (decision 0011): `kinematics::RotaryFrame` (machine and work frames of a setup, rotary axis about X or Y as a parameter) with `AsRotaryFrame`, `facing_at` and `model_up`; `machine::Clearance` is now a safe cylinder and a safe Z; `MachineProfile` gains `provenance` (confirmed or unconfirmed per field), `RotaryAxis::Y` (refused by validation until SET-02), and `rotary_origin` is the rotary reference point; new `setup` module (`SetupFrame`, `WorkOffset`, `WorkOffsetRegister`, `PartExtent`, `check_setup`, `validate_setup`, `expected_work_offset`) and `Stock::bounds` and `Stock::radius`.
- `units::LengthUnit` (um, mm, cm, m, in), `model` (`ModelFormat`, `ModelKind`, `ModelGeometry`) and `transform` (`Axis`, `TransformStep`) (slice 2, decision 0010).
- Units, geometry, mesh and B-rep data, stock, tools, machine profile, shared kinematics, neutral toolpath IR, job context and errors (work breakdown slice 0).
