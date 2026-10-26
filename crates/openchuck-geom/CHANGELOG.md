# Changelog

All notable changes to `openchuck-geom` are recorded here.

## Unreleased

- Version 0.21.0 with the library family (decision 0241): `PostJob` and `ConvertOptions` require `a_range` and work-frame `safe_z`; `Step::Unwind` is replaced by the non-moving `RotaryBoundary`. `Regeneration` keeps `canonical_toolpath`, and `Application::planned_files` takes the regeneration. Hosts connect canonical groups through `openchuck_path::program::connect_group` in file order before posting.

- Version 0.19.0 with the library family (decision 0227). `transform::transform_brep` also places each edge's exact curve.
- Version 0.18.0 with the library family (`PencilFinishing` in `openchuck-path` becomes a struct with a private field, no longer `Copy` or `Eq`, a breaking change; decision 0226); no change to this crate's items.
- Version 0.17.0 with the library family (decisions 0201 and 0202); no change to this crate's items.
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- Version 0.15.0 with the library family.
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- `check::weld_vertices`: a mesh's triangles moved onto the first vertex within a tolerance, with vertex and triangle indices kept, so a STEP model's separately tessellated faces are checked as one mesh (OI-259).
- Slice 11b (lane O, decision 0066): `pick::EdgePicker`, `PickTolerance` and `EdgeHit`: the polyline a ray passes nearest within a reach that may grow with distance, stopped at a surface hit (UI-04 for B-rep and mesh edges).
- `transform::transform_brep` and `brep_placement`: a B-rep model placed by the same steps as a mesh, with exact frames, radii, UV parameters and outward normals kept under rotate, mirror, scale and move (IMP-05; decision 0056 item 1). New items only.
- Version 0.9.0 with the library family (decision 0076).
- Slice 10 (decision 0032): `select` module: flood fill, flood fill by angle, select by direction and `submesh` on triangles (UI-04).
- `display::feature_edges` for the viewport's edge display (slice 8, decision 0029 item 4).
- `part_extent`: bounds and largest radius about the rotary axis, for the setup checks (slice 3, decision 0011).
- Edge map, mesh checks and report (IMP-04), placement transforms and orientation to the rotary axis (IMP-05), mesh summary, and an example (slice 2, decision 0010).
- Crate created as a shell (work breakdown slice 0).
