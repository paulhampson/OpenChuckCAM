# Changelog

All notable changes to `openchuck-gcode` are recorded here.

## Unreleased

- Version 0.8.0 with the library family (`Pass` in `openchuck-path` gains `rapids`, a breaking change; decision 0051).
- `compare` counts no A travel for the program's first move (from the controller's A to the first group's start), as the IR does not, so a toolpath that starts away from A 0 passes gate 2; moves between groups still count on both sides (decision 0036).
- Version 0.7.0 with the library family (`Tool` gains `centre_cutting` and `Pass` gains `stock`, breaking changes; decision 0033).
- Version 0.6.0 with the library family (`PostJob` and `ConvertOptions` in `openchuck-post` gain `operation_names`, a breaking change; decision 0028).
- Version 0.5.0 with the library family (the B-rep face record in `openchuck-types` is a breaking change, OI-123).

- Crate created as a shell (work breakdown slice 0).
