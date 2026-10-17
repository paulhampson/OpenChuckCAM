# Changelog

All notable changes to `openchuck-geom` are recorded here.

## Unreleased

- `transform::transform_brep` and `brep_placement`: a B-rep model placed by the same steps as a mesh, with exact frames, radii, UV parameters and outward normals kept under rotate, mirror, scale and move (IMP-05; decision 0056 item 1). New items only.
- Slice 10 (decision 0032): `select` module: flood fill, flood fill by angle, select by direction and `submesh` on triangles (UI-04).
- `display::feature_edges` for the viewport's edge display (slice 8, decision 0029 item 4).
- `part_extent`: bounds and largest radius about the rotary axis, for the setup checks (slice 3, decision 0011).
- Edge map, mesh checks and report (IMP-04), placement transforms and orientation to the rotary axis (IMP-05), mesh summary, and an example (slice 2, decision 0010).
- Crate created as a shell (work breakdown slice 0).
