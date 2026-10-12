# Changelog

All notable changes to `openchuck-types` are recorded here.

## Unreleased

- **Breaking (0.3.0):** `tool::Tool` replaces `stickout` with `overall_length` and gains `flute_count` and `material` (TL-04); `tool` is now a module directory. Slice 4 (decision 0014): `tool::profile` validates a `ToolProfile` by the monotone rule into a `RevolvedProfile` (lower envelope `height_at`, envelope pieces with a convexity flag, `is_convex`, `point_for_normal`) and gives `arc_centre` for arcs given by radius (decision 0012); `Tool::profile` maps every standard type to the profile and checks its values; `ControllerMapping` (TL-06), `CuttingData` and `CuttingTable` (TL-05), `validate_entry`; `units::SpindleSpeed`.
- **Breaking (0.2.0):** `machine::Clearance` is a struct (safe cylinder radius and safe Z) instead of an enum, `MachineProfile` has a new public field, `provenance`, and `MaxFeeds` gains `z` and `rapid_z`. Slice 3 (decision 0011): `kinematics::RotaryFrame` (machine and work frames of a setup, rotary axis about X or Y as a parameter) with `AsRotaryFrame`, `facing_at` and `model_up`; `machine::Clearance` is now a safe cylinder and a safe Z; `MachineProfile` gains `provenance` (confirmed or unconfirmed per field), `RotaryAxis::Y` (refused by validation until SET-02), and `rotary_origin` is the rotary reference point; new `setup` module (`SetupFrame`, `WorkOffset`, `WorkOffsetRegister`, `PartExtent`, `check_setup`, `validate_setup`, `expected_work_offset`) and `Stock::bounds` and `Stock::radius`.
- `units::LengthUnit` (um, mm, cm, m, in), `model` (`ModelFormat`, `ModelKind`, `ModelGeometry`) and `transform` (`Axis`, `TransformStep`) (slice 2, decision 0010).
- Units, geometry, mesh and B-rep data, stock, tools, machine profile, shared kinematics, neutral toolpath IR, job context and errors (work breakdown slice 0).
