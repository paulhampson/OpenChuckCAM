# Changelog

All notable changes to `openchuck-path` are recorded here.

## Unreleased

- Slice 5, second part (decision 0018): `helical` (helical finishing, `HelicalFinishing`, `CHORD_FACTOR` 0.4) and `Registry::builtin` holding it; `cutter::grow_profile` and `cutter::cutter_contact` for stock to leave; a pruned drop-cutter (`MeshSurface` finds the highest point on the axis line and walks the bounding volume hierarchy with a height bound); `unroll` (regions on the development, `Region::validate`, `ValidRegion::offset` and `passes`, `chord_points`, `PeriodicBand`); the port gains `tessellation`, `position: Option<FacePosition>` on `SurfacePoint` and `RayHit`, and the `ParametricFace` trait; `PathError::Region`. New dependency `cavalier_contours`.
- Slice 5, first step (decision 0018): the `SurfaceSource`, `StockModel` and `CollisionOracle` ports with `MeshSurface`, `AnalyticStock` and `AcceptAll`; the drop-cutter (`cutter`); `RotaryMapping` and the A-axis functions (`rotary`); parameter schemas and the common parameters (`params`); `Strategy`, `Registry` and `generate` (`strategy`); the shared services (`assemble`); deterministic parallel maps (`parallel`); `PathError`; the `host` example. New dependency `parry3d-f64` (bounding volume hierarchy only).
- Crate created as a shell (work breakdown slice 0).
