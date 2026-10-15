# Changelog

All notable changes to `openchuck-gcode` are recorded here.

## Unreleased

- `compare` counts no A travel for the move to a group's start pose, as the IR does not, so a toolpath that starts away from A 0 passes gate 2 (decision 0033).
- Version 0.6.0 with the library family (`PostJob` and `ConvertOptions` in `openchuck-post` gain `operation_names`, a breaking change; decision 0028).
- Version 0.5.0 with the library family (the B-rep face record in `openchuck-types` is a breaking change, OI-123).

- Crate created as a shell (work breakdown slice 0).
