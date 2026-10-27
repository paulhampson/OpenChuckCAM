# Changelog

All notable changes to `openchuck-import` are recorded here.

## Unreleased

- Version 0.22.0 with the library family for FACING (decision0244). Hosts constructing `StrategyInput` supply the new optional `facing` setup context; use `None` to retain legacy whole-stock inputs. No project-schema migration.

- Version 0.21.0 with the library family (decision 0241): `PostJob` and `ConvertOptions` require `a_range` and work-frame `safe_z`; `Step::Unwind` is replaced by the non-moving `RotaryBoundary`. `Regeneration` keeps `canonical_toolpath`, and `Application::planned_files` takes the regeneration. Hosts connect canonical groups through `openchuck_path::program::connect_group` in file order before posting.

- Version 0.19.0 with the library family (decision 0227). The kernel wrapper writes each edge's exact curve (line, circle, ellipse, clamped B-spline) beside its polyline.
- Version 0.18.0 with the library family (`PencilFinishing` in `openchuck-path` becomes a struct with a private field, a breaking change; decision 0226). The STEP import notice says pencil clean-up finds its inside corners on the exact edges and drops the tool on the tessellation.
- The STEP import notice no longer says toolpaths are computed on the exact surfaces: it names the strategies that read them and says the others drop the tool on the tessellation for now (decision 0209 item 4.4, decision 0210 item 7; OI-830).
- Version 0.17.0 with the library family (decisions 0201 and 0202); no change to this crate's items.
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- Version 0.15.0 with the library family.
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.12.0 with the library family (`StrategyInput` in `openchuck-path` gains `keep_out`, a breaking change; decision 0161).
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- Version 0.9.0 with the library family (decision 0076).
- STEP import (slice 11a, IMP-02, IMP-03, IMP-06, NFR-07, NFR-13; decisions 0003 and 0027): `StepImporter`, the `kernel` feature, the OpenCASCADE C ABI wrapper (`cpp/occ_step.cpp`) and the `openchuck-step-worker` child process; B-rep faces as plain data (`openchuck_types::brep`), deterministic face, edge and vertex numbering, units from the file with the user's override (`UnitSource::File`), tessellation at a chosen tolerance (`ImportOptions::tessellation`), `ImportError::Worker`, the test oracle `kernel::Oracle`, and the `step_mutations` example. `ImportOptions` gained a field and lost `Eq`.

- `ModelImporter`, `ImportedModel` and the STL importer over `stl_io` 0.11.0: ASCII and binary, millimetres by default with a `UnitQuestion` when a header names another unit or the size is implausible, the user's unit when set, conversion to millimetres, the STL tessellation statement (slice 2, IMP-01, IMP-03, decision 0010).
- Crate created as a shell (work breakdown slice 0).
