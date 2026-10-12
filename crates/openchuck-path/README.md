# openchuck-path

Reusable 4-axis pathing library: strategies, tool-axis control, A-axis management, linking, gouge protection and feed intent.

Part of [OpenChuckCAM](../../README.md). Architecture section 5.2; requirements OP-01 to OP-17, INT-01 to INT-04.

**Status:** the pathing core (work breakdown slice 5, first step; decision record 0016, proposed). It has:

- the ports `SurfaceSource` (with `MeshSurface` on a `parry3d-f64` bounding volume hierarchy), `StockModel` (with `AnalyticStock`) and `CollisionOracle` (with `AcceptAll`);
- tool-centre solving: the drop-cutter generic over the tool profile (decision record 0012);
- the rotary mapping with the rotary axis as a parameter, and A-axis management (OP-15);
- the `Strategy` trait, self-describing parameter schemas and a `Registry` (OP-16), with the common parameters (OP-11);
- the shared services that turn a strategy's passes into the neutral IR: A placement, linking through the clearance geometry (OP-14), and contact feed with segment durations (OP-13).

No strategy ships yet: the first, helical finishing, waits on spike SP-7. `examples/host.rs` is a small host that brings its own strategy and uses only the public API (`cargo run -p openchuck-path --example host`).

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.
