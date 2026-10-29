# openchuck-path

Reusable 4-axis pathing library: strategies, tool-axis control, A-axis management, linking, gouge protection and feed intent.

Part of [OpenChuckCAM](https://github.com/paulhampson/OpenChuckCAM). Architecture section 5.2; requirements OP-01 to OP-17, INT-01 to INT-04.

**Status:** the pathing core and its first strategy (work breakdown slice 5; decision record 0018, proposed). It has:

- the ports `SurfaceSource` (with `MeshSurface` on a `parry3d-f64` bounding volume hierarchy), `StockModel` (with `AnalyticStock`) and `CollisionOracle` (with `AcceptAll`);
- tool-centre solving: the drop-cutter generic over the tool profile (decision record 0012);
- the rotary mapping with the rotary axis as a parameter, and A-axis management (OP-15);
- the `Strategy` trait, self-describing parameter schemas and a `Registry` (OP-16), with the common parameters (OP-11);
- the shared services that turn a strategy's passes into the neutral IR: A placement, linking through the clearance geometry (OP-14), and contact feed with segment durations (OP-13).

- helical finishing of the whole model (OP-04) with stock to leave, path fitting by bisection and the registry entry `helical_finishing`;
- regions on the unrolled development and their offsets (`unroll`, spike SP-7): direct offsets, refusal of touching input, a result check, chording to (X, A), and tiling at the seam;
- analytic solids for the held region and retaining tabs (`Solid`: cylinder and frustum with exact queries) and `CompositeSurface`, which joins them to the model's surface as one `SurfaceSource` (slice 21b);
- the `SurfaceSource` port shaped for exact faces (`ParametricFace`, spike SP-9), with the exact evaluators in `brep` (`ExactFace`, `BrepSurface`; slice 11).

`examples/host.rs` is a small host that brings its own strategy and uses only the public API (`cargo run -p openchuck-path --example host`).

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.

### Helical pass counter reset metadata (0247)

Library hosts initialize `Pass::helical_pass_start` to false for ordinary cuts/fragments, and true only on the first retained fragment of a complete new helical traversal. Rotary roughing sets it at layer starts. Assembly preserves physical motion and records existing clear transition segments in `ToolpathGroup::rotary_reset_after`; an index means **after** that segment. It does not authorize a mid-cut reset or add motion. Hosts creating IR directly must certify boundary identity and clearance; empty metadata preserves previous behavior. `connect_group` shifts indices when prepending a physical connection, without mutating canonical paths. Do not drop metadata when caching/copying groups or reuse connected groups as canonical file inputs. Public struct literals need the new fields; this is a breaking family change assigned 0.26.0 in Merge Manager order.

Optional back-and-forth ramp clearance uses adaptive continuous-envelope certificates
(decisions 0242/0248). An inconclusive coarse bound subdivides deterministically;
all accepted intervals enclose the complete tool sweep. Each attempted interval,
including a cache hit or unsuccessful bound, consumes the shared 100,000-attempt
entry budget. Shared assembly retains exact whole-target drop results across passes
in one immutable target/tool scope, with at most 100,000 FIFO cache entries.
Standalone `EntrySafety::verify` calls retain no cache after returning. Machine and
protected-solid checks always run independently. This does not lower the global
stock-clear approach or alter ramp motion or the 10,000-leg limit.
