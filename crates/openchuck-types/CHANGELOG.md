# Changelog

All notable changes to `openchuck-types` are recorded here.

## Unreleased

- Slice 3 (decision 0011): `kinematics::RotaryFrame` (machine and work frames of a setup, rotary axis about X or Y as a parameter) with `AsRotaryFrame`, `facing_at` and `model_up`; `machine::Clearance` is now a safe cylinder and a safe Z; `MachineProfile` gains `provenance` (confirmed or unconfirmed per field), `RotaryAxis::Y` (refused by validation until SET-02), and `rotary_origin` is the rotary reference point; new `setup` module (`SetupFrame`, `WorkOffset`, `WorkOffsetRegister`, `PartExtent`, `check_setup`, `validate_setup`, `expected_work_offset`) and `Stock::bounds` and `Stock::radius`.
- `units::LengthUnit` (um, mm, cm, m, in), `model` (`ModelFormat`, `ModelKind`, `ModelGeometry`) and `transform` (`Axis`, `TransformStep`) (slice 2, decision 0010).
- Units, geometry, mesh and B-rep data, stock, tools, machine profile, shared kinematics, neutral toolpath IR, job context and errors (work breakdown slice 0).
