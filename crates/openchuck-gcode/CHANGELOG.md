# Changelog

All notable changes to `openchuck-gcode` are recorded here.

## Unreleased

- Version 0.21.0 with the library family (decision 0241): `PostJob` and `ConvertOptions` require `a_range` and work-frame `safe_z`; `Step::Unwind` is replaced by the non-moving `RotaryBoundary`. `Regeneration` keeps `canonical_toolpath`, and `Application::planned_files` takes the regeneration. Hosts connect canonical groups through `openchuck_path::program::connect_group` in file order before posting.

- Replay `G92.4 A0 S0` with the Z1 firmware's strict `|A| > 360` condition (GC-09, decision 0241, #292). Exactly +360 and -360 remain unchanged, so subsequent absolute A motion is checked against the actual controller convention. XYZ and spindle state are unchanged by this command.
- Version 0.20.0 with the library family (`MachineProfile` in `openchuck-types` gains `min_spindle_rpm` and `max_spindle_rpm`, and `ProfileField` gains `SpindleRange`, breaking changes; decision 0231).
- Version 0.19.0 with the library family (`BrepEdge` in `openchuck-types` gains `curve`, a breaking change; decision 0227); no change to this crate's items.
- Version 0.18.0 with the library family (`PencilFinishing` in `openchuck-path` becomes a struct with a private field, no longer `Copy` or `Eq`, a breaking change; decision 0226); no change to this crate's items.
- Version 0.17.0 with the library family (`Tab` in `openchuck-types` gains `auto`, and `RotaryFrame::machine` is removed, breaking changes; decisions 0201 and 0202).
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- Version 0.15.0 with the library family (`PostJob` and `ConvertOptions` in `openchuck-post` gain `operation_spindle`, a breaking change; decision 0196).
- Version 0.14.0 with the library family (`RegionConstraint` in `openchuck-path` gains `boundary_limits` and `watch`, a breaking change; decision 0176).
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.12.0 with the library family (`StrategyInput` in `openchuck-path` gains `keep_out`, a breaking change; decision 0161).
- Ready to publish to crates.io (decision 0138): an `include` list (the library sources and the files they read; the tests stay out of the package), keywords and categories, and a README that links the repository. No API change.
- Version 0.11.0 with the library family (`PostJob` in `openchuck-post` gains `generator`, a breaking change; decision 0151).
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- Version 0.9.0 with the library family (`MaxFeeds` in `openchuck-types` gains `rotary_in_feed`, a breaking change; decision 0076).
- Version 0.8.0 with the library family (`Pass` in `openchuck-path` gains `rapids`, a breaking change; decision 0051).
- `compare` counts no A travel for the program's first move (from the controller's A to the first group's start), as the IR does not, so a toolpath that starts away from A 0 passes gate 2; moves between groups still count on both sides (decision 0036).
- Version 0.7.0 with the library family (`Tool` gains `centre_cutting` and `Pass` gains `stock`, breaking changes; decision 0033).
- Version 0.6.0 with the library family (`PostJob` and `ConvertOptions` in `openchuck-post` gain `operation_names`, a breaking change; decision 0028).
- Version 0.5.0 with the library family (the B-rep face record in `openchuck-types` is a breaking change, OI-123).

- Crate created as a shell (work breakdown slice 0).
