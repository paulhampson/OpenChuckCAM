# Changelog

All notable changes to `openchuck-import` are recorded here.

## Unreleased

- STEP import (slice 11a, IMP-02, IMP-03, IMP-06, NFR-07, NFR-13; decisions 0003 and 0027): `StepImporter`, the `kernel` feature, the OpenCASCADE C ABI wrapper (`cpp/occ_step.cpp`) and the `openchuck-step-worker` child process; B-rep faces as plain data (`openchuck_types::brep`), deterministic face, edge and vertex numbering, units from the file with the user's override (`UnitSource::File`), tessellation at a chosen tolerance (`ImportOptions::tessellation`), `ImportError::Worker`, the test oracle `kernel::Oracle`, and the `step_mutations` example. `ImportOptions` gained a field and lost `Eq`.

- `ModelImporter`, `ImportedModel` and the STL importer over `stl_io` 0.11.0: ASCII and binary, millimetres by default with a `UnitQuestion` when a header names another unit or the size is implausible, the user's unit when set, conversion to millimetres, the STL tessellation statement (slice 2, IMP-01, IMP-03, decision 0010).
- Crate created as a shell (work breakdown slice 0).
