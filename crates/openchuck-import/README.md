# openchuck-import

Model and curve readers behind a `ModelImporter` trait: STL, STEP (the C++ kernel is wrapped here and nowhere else).

Part of [OpenChuckCAM](../../README.md). Architecture section 5.4; requirements IMP-01 to IMP-03, IMP-06, IMP-08, NFR-13.

**Status:** slice 2. The `ModelImporter` trait and the STL importer (ASCII and binary, through `stl_io`), read in millimetres unless the user sets a unit, with a question for the user when the file names another unit or the part's size is implausible in millimetres; conversion to millimetres happens here (IMP-01, IMP-03). Decision record 0010. 

**Slice 11a (STEP import):** `StepImporter` reads STEP through the `openchuck-step-worker` program, a child process that wraps OpenCASCADE 7.8.1 (a thin C ABI in `cpp/occ_step.cpp`, decision record 0003). The reader segfaults on damaged files, so a crash, a hang or a missing worker is an `ImportError::Worker`, never a crash of the application (NFR-07). The worker numbers faces, edges and vertices in the kernel's traversal order, so the same file always gives the same IDs; converts every face to plain data (an analytic frame and size, or a clamped, unwrapped rational B-spline patch, with UV bounds, trim loops and a tessellation, decision record 0017); reads the file's unit and applies the user's override (IMP-03); and tessellates at the requested tolerance (IMP-06). The kernel is behind the `kernel` feature, so the rest of the workspace needs no C++ toolchain:

```text
cargo build -p openchuck-import --features kernel --bins        # builds OpenCASCADE: 8 to 71 minutes the first time
cargo test  -p openchuck-import --features kernel
```

Install `openchuck-step-worker` next to the application (or set `OPENCHUCK_STEP_WORKER`). `cargo run -p openchuck-import --features kernel --example step_mutations -- 200` damages the corpus at random and imports each file through the importer. The corpus is in `testdata/step`.




Licence: AGPL-3.0-or-later.
