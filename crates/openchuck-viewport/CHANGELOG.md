# Changelog

## 0.34.0 (provisional; the coordinator confirms the number at merge)

- **Breaking family release (0.34.0, lane CLEARANCE, issue #336; decision 0530, proposed):** no change in this crate; all family crates advance together under 0048.

All notable changes to `openchuck-viewport` are recorded here.

## Unreleased

- Version 0.25.0 with the library family for FACING (decision0252). Hosts constructing `StrategyInput` supply the new optional `facing` setup context; use `None` to retain legacy whole-stock inputs. No project-schema migration.

- Version 0.21.0 with the library family (decision 0241): `PostJob` and `ConvertOptions` require `a_range` and work-frame `safe_z`; `Step::Unwind` is replaced by the non-moving `RotaryBoundary`. `Regeneration` keeps `canonical_toolpath`, and `Application::planned_files` takes the regeneration. Hosts connect canonical groups through `openchuck_path::program::connect_group` in file order before posting.

- Version 0.19.0 with the library family (`BrepEdge` in `openchuck-types` gains `curve`, a breaking change; decision 0227); no change to this crate's items.
- Version 0.18.0 with the library family (`PencilFinishing` in `openchuck-path` becomes a struct with a private field, no longer `Copy` or `Eq`, a breaking change; decision 0226); no change to this crate's items.
- Version 0.17.0 with the library family (decisions 0201 and 0202); no change to this crate's items.
- Version 0.16.0 with the library family (decision 0204); no change to this crate's items.
- Version 0.15.0 with the library family.
- Version 0.13.0 with the library family (`StrategyOutput` and `GeneratedPath` in `openchuck-path` gain `warnings`, a breaking change; decision 0175).
- Lane BUG-PERF (issue 200, decision 0166): lines of one look in a layer are drawn as one buffer and one draw, each keeping its own dash pattern; a hidden layer stays uploaded (`Renderer::set_layer_shown`, `is_layer_shown`); `LayerStats`, `Renderer::layer_stats` and `Renderer::uniform_writes`; the `path_frames` long-toolpath benchmark example. Additive.
- Lane BUG-SIM (decision 0169): `Layer::Trace`, a faint trace of the toolpath drawn before `Layer::Paths`, for the playback window. Breaking: `Layer` gains a variant and `Layer::ALL` has seven layers (`index` of `Paths`, `Tool`, `Overlay` and `Focus` moves up by one). Not in the library family, so `cargo-semver-checks` does not check it.
- Slice 22, lane X3 (decision 0119): `LineStyle` (solid, dashed, dash-dot, dotted, measured on screen at the target) on `LineItem`; a screen-space `Hatch` (one direction or crossed, 6 pixels apart) and an `Outline` (silhouette, creases and boundary as a wide line) on `MeshItem`, with `outline_only`; `Layer::Focus` for the hovered or selected held region or tab; `Frame::pixels_per_point` and `Frame::pixels_per_mm`. Breaking: `LineItem` and `MeshItem` gain fields.
- Version 0.10.0 with the library family (`assemble::smoothed_contacts` in `openchuck-path` takes the rotary mapping, a breaking change; decision 0115).
- Version 0.9.0 with the library family (decision 0076).
- `bench`: the NFR-03 scripted camera path and frame-time statistics (slice 8 item 7, decision 0029 item 9).
- Slice 8, first step (decision 0029): `camera` (orbit, pan, zoom, standard views, perspective and orthographic, fit, pick ray), `scene` (plain-data meshes with feature edges and lines, display modes) and `renderer` (wgpu, no window toolkit; derivative-normal shading). wgpu is re-exported for front ends.
- Crate created as a shell (work breakdown slice 0).
