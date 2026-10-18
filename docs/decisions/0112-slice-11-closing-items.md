# 0112: Slice 11 closing items: selection checks as jobs, an edge-pick hierarchy, wider selected edges

- **Status:** proposed (lane S11). Items 1 and 2 follow the open items as written (OI-332, OI-243). Item 3 chooses the widths; the owner approved them and the specification row (relayed by the coordinator under decision 0049, 18 Oct 2026; OI-335 closed). The record itself is not yet accepted.
- **Date:** 18 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-332, OI-243 and OI-245 close; OI-335 was raised and closed.

Numbering: 0111 was the highest on `main` and the unmerged branches; OI-334 was the highest open item.

## Context

[Decision 0066](0066-slice-11b-brep-selection.md) (accepted) built B-rep selection and left three items open before slice 11 could close: the check behind saving or editing a set read the model on the caller's thread (OI-332, NFR-04), edge picking had no hierarchy and no measurement (OI-243, with the pick budget of [decision 0037](0037-slice-10-picking-and-selection-panel.md), OI-12), and a selected or hovered edge differed from the others by colour only (OI-245, specification section 16).

## Decision

1. **Saving and editing a set check the model as a background job (OI-332; NFR-04, UI-05).** `Application::start_save_selection` and `start_edit_selection` read and import the model and check the target on the job manager, with progress and cancellation like `start_select_region`; they return a `CheckedSet` that `finish_save_selection` and `finish_edit_selection` store, as one undoable step, on the caller's thread. An edit that does not change what the set selects (rename, stock to leave, colour, tangent choice) needs no model and comes back ready (`EditStart::Done`). `finish_edit_selection` refuses an edit made from a set that has changed or gone since the job started, and `finish_save_selection` one whose model's recorded file changed, so a late result never overwrites a newer edit. `save_selection` and `edit_selection` remain, and wait for the same jobs, so the result is the same and does not depend on the thread count (NFR-06). The selection panel starts the job, shows "Checking the selection against the model…" with Cancel, disables the other save and edit buttons meanwhile and stores the set when the job finishes. No project format change.
2. **Edge picking has a hierarchy (OI-243; UI-04; amends item 5 of decision 0066).** The 1M-triangle benchmark model has 2,000 feature edges, and its edge pick measured 0.8 to 1.0 µs at the 95th percentile (1,024 rays), far inside the 1 ms budget. The worst case of the open item, a rough scan whose 721,200 lattice edges are all feature edges, took 28.4 ms at the 95th percentile with the segment-by-segment scan (release build), 28 times over the budget. `EdgePicker` now keeps a binary tree of boxes over the segments of every polyline (median split on the longest axis, leaves of at most four segments, built in 0.6 to 0.8 s for 721,200 edges). The scan then picks in 69 µs (release) to about 200 µs (the test profile) at the 95th percentile. The answer does not change: of equally near segments the nearer to the eye, then the lower edge, then the lower segment (a test compares the tree with testing every edge). The budget is a test, `req_ui_04_picking_edges_of_a_million_triangle_part_stays_within_its_budget`, with the same relaxation on hosted CI as the surface pick. Because that budget is timed in a test, `openchuck-geom` is optimised in the dev profile like the other crates on a timed path (decision 0018, 0025). The strict figures on the reference machines remain part of OI-173.
3. **A hovered or selected edge differs by width as well as colour (OI-245; specification section 16, principle 4).** A line item gains a `width` in logical pixels; up to 1 it is the hairline as before, wider it is drawn as one quad per segment (an instanced vertex shader, with square ends, cut at the eye plane), so its width does not change with the zoom. The part's feature edges stay hairlines; a **hovered edge is 2 pixels wide** (hover yellow) and a **selected edge 3 pixels wide** (the working selection's blue, or the shown set's swatch). The widths follow the 1.5 and 2 pixel outlines of section 7.5 and were approved by the owner as recommended (relayed by the coordinator under decision 0049, 18 Oct 2026), who agreed to the row for them in specification 7.4, now added (OI-335 closed). Face and triangle hover outlines keep their line; they are an outline mark already.

## Consequences

- `openchuck-viewport` gains `LineItem::width`, `Scene::add_wide_polyline` and `Frame::viewport` (a breaking change to a crate outside the library family, so no `cargo-semver-checks` run applies). `openchuck-app` gains the `start_` and `finish_` pairs and `CheckedSet`, `EditStart`.
- The wide-line shader is exercised by an offscreen render test where an adapter exists (CI sets `OPENCHUCK_REQUIRE_ADAPTER`); run here on a software Vulkan driver.
- Slice 22 (the conformance pass) should check the widths against the mock-ups.

## Alternatives considered

- *Dash the selected edge:* dashes mean outlines of stock and clearance in section 7.4, so they would collide.
- *Check against the geometry the scene already holds* (the second remedy of OI-332): the scene holds the placed mesh and B-rep for display, but its hash is not the file's hash, and a stale scene would accept a changed file. A job that reads the file is the same check as before, off the interface thread.
