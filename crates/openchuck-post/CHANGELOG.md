# Changelog

All notable changes to `openchuck-post` are recorded here.

## Unreleased

- Version 0.8.0 with the library family (`Pass` in `openchuck-path` gains `rapids`, a breaking change; decision 0051).
- Version 0.7.0 with the library family (`Tool` gains `centre_cutting` and `Pass` gains `stock`, breaking changes; decision 0033).
- Version 0.6.0 with the library family (`PostJob` and `ConvertOptions` in `openchuck-post` gain `operation_names`, a breaking change; decision 0028).
- **Breaking (0.6.0):** `PostJob` and `ConvertOptions` gain `operation_names`, the display names the comments that name each operation use (GC-06, decision 0028). The comment now reads `operation <name>, tool T<n> <tool name>, about <s> s`.

- Version 0.5.0 with the library family (the B-rep face record in `openchuck-types` is a breaking change, OI-123).

- Crate created as a shell (work breakdown slice 0).
- Slice 6: neutral machine program (`to_machine_program`) and the named feed rules `linuxcnc`, `makera`, `inverse_time` and `contact`.
