# Changelog

All notable changes to `openchuck-viewport` are recorded here.

## Unreleased

- `bench`: the NFR-03 scripted camera path and frame-time statistics (slice 8 item 7, decision 0029 item 9).
- Slice 8, first step (decision 0029): `camera` (orbit, pan, zoom, standard views, perspective and orthographic, fit, pick ray), `scene` (plain-data meshes with feature edges and lines, display modes) and `renderer` (wgpu, no window toolkit; derivative-normal shading). wgpu is re-exported for front ends.
- Crate created as a shell (work breakdown slice 0).
