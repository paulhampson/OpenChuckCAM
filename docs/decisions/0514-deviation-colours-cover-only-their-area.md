# 0514: Deviation colours cover only the area each band applies to, not whole triangles (lane SIMVIS, issue #488)

- **Status:** Accepted (2 Nov 2026, by the project owner after reading the record in full, relayed by the coordinator; decision 0049). Proposed 2 Nov 2026 by lane SIMVIS. The plan (option A), item 5 and item 6 (choice B) are the owner's answers of 2 Nov 2026, relayed by the coordinator.
- **Date:** 2 Nov 2026.
- **Lane:** SIMVIS, `claude/lane-simvis-bite` (#492), stacked on #486 until that merged on 4 Nov 2026, then merged with `main`.
- **Issue:** #488 (the owner's request after accepting decision 0513: "the area highlighted covers the whole face and not just the bite. This makes it confusing to the user. Add the ability to highlight the specific area impacted."). The owner's crankshaft report #523 is the same defect.
- **Requirements:** SIM-04 (still and playing frames), SIM-05, SIM-11 (the probe), SIM-15, UI-05 (selection sets keep the part's triangle numbers), NFR-04, NFR-06, NFR-12, NFR-15.
- **Amends:** decision 0513 item 4 (accepted), by the owner's answer of 2 Nov 2026, relayed by the coordinator (item 5); specification 11.5 and the verification plan's SIM-05 row, to match.
- **Numbers:** 0514, assigned by the Merge Manager at release on 4 Nov 2026 (proposed on the lane as 0638). No open item was needed. No project schema number. The workspace minor version moves to 0.41.0 for `openchuck-sim`'s breaking change (below); no library-family API change.

## Context

The renderer draws each mesh item in one colour, and the deviation bands are sets of whole triangles (decisions 0040, 0238, 0492, 0513). A large triangle therefore takes one colour over its whole area:

- **The cut part** is the dexel stock's mesh. A faced end is made of strips one column wide and as long as the stock is across. A strip that crosses the edge of a bore is coloured "Unreachable" over the solid ring too, as the screenshots of the tube stand-in for #478 showed.
- **The part model** may hold samples of several bands in one large triangle. On a bore made of long strips, each strip takes one colour.

## Decision

1. **Pieces where a triangle holds more than one band** (option A, the owner, 2 Nov 2026, relayed; made adaptive after the measurements). A triangle whose test points fall in more than one shown band is cut into pieces that follow the band edges, down to about two simulation cells; a triangle of one band is drawn whole, as before.
   - **Which triangles.** On the part, the triangle's own samples within a cell of a piece decide, because seven test points missed a rim band 0.1 mm wide. On the cut part, seven test points decide.
   - **How they are cut.** A strip (four or more times longer than it is wide, as the cut part's faced end and the part's bore are made of) is cut across its length. Its length is halved while a stretch holds more than one band, down to two cells or the strip's width. A clean band edge (a short mixed stretch, or two neighbouring ones, with one band on each side) is one cut at the middle of the stretch, within two cells of the edge. At an end of the strip, the band at that end counts as the side beyond it, and a half standing alone there is kept only if its pieces read its band, so a rim narrower than the stretch is kept. Neighbouring stretches of one band are joined, and a stretch still holding more than one band is cut on as a piece. Any other triangle is halved along its longest edge while its test points disagree.
   - **Colour.** Each piece is coloured by its parent's rule at its own centre: on the part, the nearest of the parent's own samples; on the cut part, decision 0513 item 4 as amended by item 5.
2. **The part's triangle numbers do not change.** The pieces are an extra coloured layer and their parents are left out of the bands, so selection sets, picking and the outlines (decision 0216) are untouched. The probe reads the piece under the pointer. The cut part's mesh is not replaced either: its picker is built from the unchanged mesh (decision 0238).
3. **Bounded and deterministic.** The pieces are made off the interface thread in the job that makes the colours, in fixed chunks, the same at any thread count (NFR-06). They are capped at 500,000; past the cap the remaining parents are drawn whole and the colour scale says so (NFR-04). Nothing is drawn finer than about two cells, the simulation's own resolution (NFR-12). The cap bounds the work as well as the pieces kept (CodeRabbit on #492): the parents are cut in batches of eight, in the mesh's order, each only up to the room left in the budget at its batch's start (a parent past it would not fit, so it is drawn whole, as before), and the cutting stops once the pieces cut, kept or not, pass twice the cap. The work is then bounded whatever the mesh, and the result is the same at any thread count.
4. **No renderer change.** Rejected: per-vertex colours or a texture in the shader (a renderer change, and blending smears the band edges and the hatches); dots at the affected samples (they show where, not the extent, and would duplicate the unreached dots of lane VIEW-OVERLAYS, #416).
5. **A cut-part face off the part's surface takes the material behind it** (amends decision 0513 item 4; the owner answered yes on 2 Nov 2026, relayed by the coordinator). A cut-part face, or a piece of one, more than two cells from every counted sample is off the part's surface. It takes the value and class of the sample of any kind nearest to the point one search distance behind it, into the stock, when that is a counted part sample whose value is a bound or whose material left is enclosed, as item 4 of decision 0513 says for faces beyond the search. Otherwise it keeps the nearest counted sample within the search, and beyond the search it is Not measured, as before. Within two cells of a counted sample the face is on the part's surface and takes that sample, as before. Item 4 alone left a cap over a bore within the search of the opening's rim taking the rim's "nothing left": on the tube stand-in only 31% of the bore's disc showed Unreachable.
6. **The cut part is drawn in pieces on a still frame only** (the owner's choice B, 2 Nov 2026, relayed by the coordinator, after the measurements below). A frame shown while playback plays has no picker (decision 0322) and is coloured by whole triangles, as before pieces, so playback costs what it did. A still frame, at a pause or after a step or a jump, is drawn in pieces. The part model, coloured once per run, is always drawn in pieces.

## As built

`openchuck-app`: `pieces.rs` (the cutting, the budget and `with_pieces`); `stock_surface.rs` (the cut part, items 5 and 6) and `stockplay.rs` (the part, from each triangle's own samples) use it; `pathview.rs` `DeviationBands::pieces`, `BandPieces`, `value_at`, `band_at`, `band_of`. `openchuck-sim`: `triangle_samples` (additive). `openchuck-ui`: `viewport.rs` `band_layers` draws the pieces with the bands and the probe reads them; the colour scale's note when the budget ran out; `playback_panel.rs` applies the band filter (SIM-15) to the pieces.

**Also carried: two findings from CodeRabbit's review of #486** (the coordinator's instruction, 4 Nov 2026; decision 0513's Verification lists the tests).
- **A skipped enclosure test claims nothing (NFR-12).** `openchuck-sim` `Reach` and `openchuck-app` `SetupReach` gain `classified`. It is false when the test was skipped: a tool narrower than two grid pitches, or nothing to cast against. The results then read "not classified" for the share no tool reaches (decision 0513 item 7), not "0.0 %". The colours are unchanged: such samples were Undecided and never in the Unreachable band, so this record's colour rule and the legend stay as they are.
- **The tick box that hides unreachable material stays while its filter is on**, after a run with nothing enclosed.
- **Library version.** The new public field is a breaking change to `openchuck-sim`, now that #486 is on `main`. The workspace minor version moves to 0.41.0 (decision 0048), as the Merge Manager assigned at release, with the changelog entry.

## Measurements

All on the owner's carving (kept outside the repository; numbers only) at a 0.1 mm cell, release build. Before any change, a triangle was counted as holding more than one band when its own counted samples (the part) or seven test points (the cut part) fell in more than one band.

| Mesh | Triangles | Holding more than one band | Even split to about 2 cells would add |
| --- | --- | --- | --- |
| Cut part at the end | 1,323,834 | 1,210 (1,475 mm²) | about 85,000 pieces (+6%) |
| Part model | 500,738 | 1,522: the bore's long strips | about 159,000 pieces (+32%) |

On the part model the bore's strips hold two bands because the bore is enclosed along most of its length, but its rim at each end is on the surface (the probe below; the first reading of this record put those samples in "More than 1 mm left"). An even split would add pieces almost all of one colour. Testing the cut part's triangles at seven points took 7.9 s on one thread.

**Longest-edge halving only, the first build.** Measured on the owner's carving (release build, numbers only):

| Mesh | Parents | Pieces | Pieces by band | Colouring job |
| --- | --- | --- | --- | --- |
| Part model | 1,522 | 133,422 | 117,698 Unreachable, 15,724 on the surface | 6.1 s |
| Cut part at the end | 1,210 (longest edge: median 18.8 mm, at most 40 mm) | 132,134 | 68,711 on the surface, 39,234 Unreachable, 24,189 not measured | 8.3 s |

Neither run reached the budget. The adaptive pieces number about as many as even ones. A first reading put this down to bands interleaving inside the triangles, but a probe of the part model's mixed triangles found otherwise. Each of the 1,522 holds exactly three runs of one band along X:

- **On the surface at each end of the bore:** the rim, where the faced end cuts the stock flush. These are 10,056 samples: values 0 to 0.05 mm, not enclosed, within about a cell of each end.
- **Unreachable between them:** 1,304,634 samples, enclosed.

Each strip therefore has two real band edges across it.

**The cut part.** With item 5 (above), the cut part at the end had 1,687 parents and 264,850 pieces. The colouring job took 10.1 to 11.1 s, against 8.3 s under item 4 alone and 3.2 s before pieces. The owner asked for the cause to be found before any cost was accepted (2 Nov 2026, relayed).

The cut part's mixed triangles hold no interleaving either. Every one of them is a strip of the faced end, exactly 0.1 mm wide (one dexel column) and up to 40 mm long (median 25.4 mm). Sampled every quarter cell along its length:

| Runs of one band | Strips |
| --- | --- |
| 1 | 212 |
| 2 | 996 |
| 3 | 282 |
| 4 | 145 |
| 5 to 8 | 52 |

Only 72 runs anywhere are shorter than two cells.

**The cause is the cutting rule, not the bands.** Halving a strip along its longest edge leaves one child as long as the strip, so the pieces spread along the whole strip instead of gathering at the band edge. A model of the rule on a strip 25 to 40 mm long, 0.1 mm wide, with one band edge across it, gave 35 to 151 pieces. The part model's bore strips are slivers too.

**The fix** is the strip rule of item 1. The coordinator relayed the target: about five pieces per parent, and at most seven for a strip with two edges.

Measured on the owner's carving (release build, 4 threads, numbers only):

| Mesh | Parents | Pieces, longest-edge halving | Strips cut across | With one cut per clean edge |
| --- | --- | --- | --- | --- |
| Part model | 1,522 | 133,422 | 16,476 | 10,556 (6.9 per parent: 5,412 on the surface, 5,144 Unreachable) |
| Cut part at the end | 1,687 | 264,850 | 17,801 | 8,975 (5.3 per parent) |

**Timings.** The part model's colouring job takes 3.3 to 3.4 s. For the cut part, the job was run four rounds in one process, alternating with and without pieces:

| Cut-part frame at the end | Without pieces | With pieces |
| --- | --- | --- |
| Still (with its picker) | 4.8 to 5.5 s | 6.2 to 6.5 s |
| Playing (no picker) | 1.1 to 1.6 s | 2.5 to 2.9 s |

Pieces add about 1.3 s either way. Making the picker is most of a still frame's cost. The 3.2 s quoted earlier "before pieces" was not like for like. The rest of the pieces' cost is finding the mixed triangles: 73,558 of the cut part's 1,323,834 triangles are longer than two cells and are tested at seven points.

**As built, with item 6** (pieces on still frames only), over four rounds on the cut part at the end:

- **Playing frames:** 1.2 to 1.6 s, the same as without pieces.
- **Still frames:** 5.6 to 7.3 s on this container, about 1.3 s more than without pieces (see the timings above).
- **Part model:** 3.4 s.

**The tube stand-in.** Under decision 0513 item 4 alone, the faced end showed 47.7 mm² of Unreachable over the bore's 153.9 mm² disc (31%): a cap face within the comparison's search (3 mm plus a cell) of the rim took the rim's value, on the surface. With item 5 it shows 145.1 mm² (94%).

## Verification

Built test first; each test failed before its code, except where noted.

- `openchuck-app` `pieces.rs`:
  - `req_sim_05_a_triangle_holding_two_classes_is_coloured_by_each_within_two_cells`;
  - `req_sim_05_a_triangle_of_one_class_is_drawn_whole`;
  - `req_sim_05_adaptive_pieces_follow_the_band_boundary`;
  - `req_nfr_06_the_split_colours_are_the_same_at_any_thread_count`;
  - `req_sim_11_the_probe_on_a_split_triangle_reads_its_piece`;
  - `req_nfr_04_the_added_triangles_stay_within_their_budget`;
  - `req_nfr_04_the_budget_bounds_the_cutting_work_not_only_the_pieces_kept` (CodeRabbit on #492; failed first, with 1,523,151 calls to the colour rule for a hundred ragged triangles against a budget of 2,000; now within 200 calls per budgeted piece);
  - `req_ui_09_cutting_stops_when_cancelled`;
  - `req_sim_05_the_distance_to_a_triangle_is_the_nearest_point_s`;
  - `req_sim_05_a_band_narrower_than_the_test_points_is_found_by_the_samples`.
- `pieces.rs` `req_nfr_04_req_sim_05_a_long_strip_is_cut_across_its_length`. It failed first, with 124 pieces. A strip 25 mm by 0.1 mm (two triangles) with one band edge across it makes at most 20 pieces, with the edge placed to within two cells; with two band edges, as across a bore, at most 14.
- `pieces.rs` `req_nfr_04_req_sim_05_a_clean_band_edge_across_a_strip_is_one_cut`. It failed first, with 22 pieces for the two-triangle strip. A one-triangle strip with two clean edges makes at most 7 pieces, with each band's area within two cells of the true one.
- `pieces.rs` `req_ui_05_req_sim_05_pieces_leave_the_part_s_triangles_and_their_numbers` (item 2). It was written after the code and passed first time; it stands for the planned `req_ui_05_selection_sets_and_picking_keep_the_part_s_triangle_numbers`. The mesh is unchanged, every piece names a triangle of it, a parent's pieces cover exactly that parent, and the parents are left out of the bands.
- `pieces.rs` `req_sim_05_a_small_area_left_on_a_round_end_face_drawn_as_a_fan_is_coloured_only_there` (2.6 s), for the owner's crankshaft report #523: a round end face drawn as a fan of large triangles, with material left only over a disc of radius 1 mm (3.14 mm²). It is a regression test written after the code, not test-first: it passed first time. With 6 fat wedges, 3.18 mm² is coloured, against 209.4 mm² for a whole wedge. With 64 thin wedges, 3.16 mm² is coloured, against 58.9 mm² for the three whole wedges the spot touches.
- `stock_surface.rs`:
  - `req_sim_05_a_stock_face_across_a_band_edge_is_drawn_in_pieces`.
  - `req_sim_05_a_stock_face_off_the_part_s_surface_takes_the_material_behind_it` (item 5). A cap five cells from the rim takes the enclosed bore behind it. A cap one and a half cells from the rim keeps the rim. With nothing bound or enclosed behind a cap, it keeps the nearest counted sample.
  - Decision 0238's projection test `req_sim_05_req_sim_11_stock_colours_and_probes_use_nearest_existing_samples_with_bounds` places its triangles within two cells of the samples they read. At 0.2 mm (two cells, and a little more at the centroid) the bound behind the triangle was taken, as item 5 intends.
- `openchuck-app` `tests/reach.rs`:
  - `req_sim_05_the_part_s_bore_is_drawn_in_pieces_where_its_strips_hold_two_bands`: Unreachable across the middle of the bore, not at its rim.
  - `req_sim_04_req_sim_05_only_a_still_frame_draws_the_cut_part_in_pieces` (13.6 s; item 6): on the tube, the cut part at the end is drawn in pieces on a still frame but not on a playing frame, and the whole triangles' values are the same on both.
- `openchuck-app` `tests/playback.rs`: every triangle is in a band or among the pieces' parents.
- `openchuck-sim` `tests/reach.rs` `req_sim_05_each_triangle_names_the_range_of_its_samples` (`triangle_samples`).
- `openchuck-ui` `viewport.rs`:
  - `req_sim_05_req_sim_15_a_triangle_in_pieces_is_drawn_and_read_by_its_pieces`: drawn, probed and filtered by its pieces.
  - `req_nfr_04_the_scale_says_when_the_pieces_ran_out`.
- `openchuck-ui` `shell::simulate_tests::req_sim_05_the_faced_end_over_a_bore_is_violet_only_over_the_bore` (11.5 s), through the whole interface on the tube stand-in. It shows 145.1 mm² Unreachable on the faced end against the bore's 153.9 mm² disc (within 20%); before item 5 it showed 47.7 mm².
