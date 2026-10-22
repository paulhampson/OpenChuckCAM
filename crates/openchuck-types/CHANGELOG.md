# Changelog

All notable changes to `openchuck-types` are recorded here.

## Unreleased

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
