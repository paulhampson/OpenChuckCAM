# openchuck-geom

Mesh utilities: adjacency, validity checks, transforms, selection algorithms and ray-cast picking.

Part of [OpenChuckCAM](../../README.md). Architecture section 5.4; requirements IMP-04, IMP-05, UI-04.

**Status:** slice 2. The edge map, mesh checks with a report for the user (holes, non-manifold edges, flipped triangles and stated normals, degenerate triangles; IMP-04), and placement: translate, rotate, scale and mirror with the winding kept outward, and orientation to the rotary axis (IMP-05). Decision record 0010. Selection and picking arrive in slice 10.

Example: `cargo run -p openchuck-geom --example check_and_place`.



Licence: AGPL-3.0-or-later.
