# 0368: Simultaneous finishing of surfaces hidden from the drive (lane OP20)

- **Status:** proposed. The owner chose, on 31 Oct 2026, relayed by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7; decision 0049):
  - decision 1, option A: passes added for the hidden surfaces, with the drive left as it is (the unified drive is deferred, OI-1450, issue #415);
  - decision 2, option B: the reachable direction nearest the normal, limited by a new parameter, the largest lean on hidden surfaces;
  - decision 3, option B: always on, with no setting to turn it off;
  - decision 4, option B: a warning, plus the unreached areas as structured data on the strategy's output (viewport highlighting deferred, OI-1451, issue #416);
  - decision 5, option A: OP-20's wording and verification row, with OP-20 raised to P1 (*Consequences*).

  This record's text awaits the owner's acceptance.
- **Date:** 31 Oct 2026.
- **Lane:** OP20 (issue #413), branch `claude/lane-op20`.
- **Requirements:** OP-20 (now P1), with OP-05, OP-06, OP-12, OP-15, OP-26, OP-02, SET-09, NFR-06, NFR-07, GC-07 and NFR-12.
- **Builds on:** [0036](0036-slice-13-simultaneous-finishing.md) (the strategy, whose item 1 left these surfaces as a P1 limit, OI-148), [0363](0363-simultaneous-finishing-off-axis-sections-walls-and-held-ends.md) (the section-centre drive, OP-26, and the move test), [0050](0050-walls-merge-micro-moves.md), [0133](0133-simultaneous-fitter-tests-the-quarter-points.md), [0161](0161-tools-keep-clear-of-the-gripped-length.md), [0277](0277-exact-ball-drops.md) and [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md).
- **Open items:**
  - closed: OI-1431 (a section in several separate pieces: its pieces' facing sides are hidden surfaces and are finished or reported here);
  - noted: OI-148 (closed in decision 0036; the P1 limit it records is lifted here);
  - new: OI-1450 and OI-1451 (deferred by the owner), and OI-1452 and OI-1453.

## Context

Simultaneous finishing drives the tool from the first point a ray from outside meets on its way to the rotary axis (decision 0036 item 1), or to a section's own centre where the section does not surround the axis (OP-26, decision 0363 item 3). Anything such a ray does not meet first is never driven:

- the walls of a slot along X, which lie along the rays;
- the far walls and the floors of undercuts;
- the sides of separate pieces of one section that face each other (two pins side by side, blades without a hub).

The owner accepted this as a P1 limit in decision 0036 and added OP-20 (P2) to lift it. The owner asked for a lane to build OP-20 (issue #413) after lane SIM4AX's OP-26, and made OP-20 P1 (decision 5).

The machine can lean the tool only in the plane across the rotary axis: A turns the part, and Y and Z move the tool. So whether the tool can reach a point is mostly a question about the part's sections within one tool radius of that point's X. The design was sent to the owner as lane OP20's phase 1 (31 Oct 2026). These items record what was built, with the owner's five answers.

## Decision

1. **Where (OP-20).** The part, or the tessellation of a STEP part, is cut across X at sections the stepover apart, between the part's ends less the held bands (decision 0363 item 1). The first section is half a step in.
   - Each section is a set of chains of the triangles' edges, running round the material counter-clockwise seen from +X. A vertex on the plane counts as beyond it, and each segment is oriented by its triangle's winding, so the chains close wherever the mesh does, and a mesh with an open edge gives an open chain.
   - Points along each chain, 0.2 mm apart (`REACH_SPACING`), are **hidden** when the drive's ray at their angle round the drive's centre (the axis, or the section's own centre, OP-26) does not meet them first (within 0.1 mm, `VISIBLE_GAP`).
   - On a STEP part each point is first put on its exact face, with the face's normal, since the drive's rays meet the faces, not their tessellation. A chord of the tessellation lies further than 0.1 mm inside a coarse face, and a tessellation's winding need not match the face's normal: without this, an exact cylinder was taken as hidden all round.
   - A point is also hidden when that ray meets it at a **grazing** angle: across X, more than 75 degrees (`GRAZING`) from the facet's normal. There the drive's rays fall far apart on the surface, and the drive's tool, on the normal, lies across them: the sides of a radial blade are an example. A surface of revolution, and the lobed part (whose normal leans up to about 17 degrees across X), are nowhere near it.
2. **The passes (owner's decision 1, A: added).**
   - Each run of hidden points along a chain is grown on both sides by the larger of the stepover and the tool's cutting radius. The drive's own coverage near a grazing edge ends before the points stop being met.
   - The grown run is a **contour**: one pass along the chain at that section's X, cut after the drive's passes and the turns at faces.
   - Contours on neighbouring sections that come within twice the larger of the section spacing and 0.2 mm of each other form one area. Areas are cut one after another, each section after the one before, every other pass reversed, and the shared services link the passes (OP-14).
   - The drive's passes are made by exactly the code they were made by before. A part with no hidden surface gets no contour, no note and no warning, so its program is unchanged. The `lobed` golden programs are byte for byte the same.
3. **The tool axis (owner's decision 2, B).** At each point of a contour the tool is tried with its axis at angles across X.
   - It **reaches** the point when, placed so that its point with the surface's normal is on it and dropped along that axis, it rests on the point to within the tolerance. Where the shank is wider than the cutting part (item 7), the shank must also not rest higher.
   - The tool is placed by the facet's normal (the exact face's on a STEP part), the only one with which it can touch the point. The axis aims by the smoothed normal (decision 0036 item 4), which leans across a convex edge.
   - The search starts at the target, the normal's angle across X plus the lead, within the largest lean on hidden surfaces (the new parameter `max_hidden_lean`: degrees, 0 to 90, default 90).
     - If the target does not reach, the search steps outwards in 5 degree steps, both ways, from the angle that reached at the point before (or from the target when none did). The first angle that reaches is walked back towards the target a step at a time, and the edge is found by 6 halvings (0.08 degrees).
     - The axis is kept 0.5 degrees (`GATE_MARGIN`) inside the edge, because what reaches changes between the points tried.
   - **A is smoothed** as on the drive's passes (decision 0036 item 5), by the taut string, inside a band: the largest tilt either side of the nearest reachable angle, but only angles found to reach, also kept the margin inside their edge.
   - So where the normal can be reached within the tilt, the tool follows it as everywhere else (OP-06). Elsewhere it leans from the normal as little as the part lets it, never beyond the largest lean on hidden surfaces. 0 keeps it on the normal, and 90 lets it cut a wall with its side.
4. **The fitter on a contour.** The path is fitted by the drive's fitter (decisions 0036 item 7, 0133 and 0363 item 2), with four differences, all for contours alone:
   - A tool that does not rest on its drive point (it touches something in the way) is no point of the pass: the pass breaks there.
   - The fitter treats such a point as it treats a point a region forbids, finding the edge between by halving (decision 0115 item 2).
   - A cut into the surface at a test pose is measured as the depth inside the tool, at that pose, of the point the drop came to rest on. The drive's measure, along the drive normal, reads too little where the axis is far from the normal and turns fast at an edge: it read 0.0065 mm for a cut of 0.011 mm at a slot's top edge.
   - A move that cannot be kept within the tolerance after every halving and raise breaks the pass. It never refuses the operation.
5. **What cannot be reached (OP-20's last clause).** Each hidden point the tool does not reach is classed by why:
   - **needing more lean** than `max_hidden_lean`: some angle up to 90 degrees reaches it;
   - **blocked**: no straight line across X, within a right angle of its normal, reaches it from outside without meeting the part, so the tool would have to lean along the rotary axis;
   - **too narrow**: such a line exists, but the tool is too wide for the gap, or its flutes too short or its shank too wide for the depth.

   **Corners are left to rest machining.** A run of points the tool does not reach is a corner when both of these hold:
   - points it reaches lie on both sides, no more than 2.5 tool radii apart along the section (`CORNER_SPAN`): a round end in a square corner leaves two radii;
   - the surface turns inwards by more than 30 degrees round it (`CORNER_TURN`).

   Such corners are not reported, as concave corners on surfaces the drive meets are not reported: rest machining finishes them (OP-09). The tool is tried a tool radius beyond each contour's ends to tell them apart. A slot narrower than the tool is longer than that, so it is reported. The side of a pin facing another turns outwards, so it is reported too.

   **Grouping.** The rest are grouped into areas of neighbouring points with the same reason, on the same or a neighbouring section. Areas of fewer than three points (`MIN_REPORTED_POINTS`, where a grazing ray happens to miss) and areas outside the operation's region, boundary or avoid sets (OP-02) are left out. Each area has:
   - its X range;
   - the range of A at which its normals face the tool (all round when its angles leave no gap over 10 degrees);
   - its area (points times their spacing);
   - its reason, and for "needs more lean" the largest lean any of its points needs.
6. **How it is reported (owner's decision 4, B).**
   - **The warning.** `StrategyOutput` and `GeneratedPath` gain `unreached: Vec<Unreached>`, the areas largest first, and one warning (decision 0175) says them in plain words: "simultaneous finishing cannot reach N hidden areas of the part, which stay uncut: X a to b near A c (s mm2), too narrow for this d mm tool, or deeper than its flutes or shank allow; ...; check them in the simulation before running the program".
     - It lists the largest five and says how many more there are.
     - The reasons read "blocked: the part is in the way of every direction the tool can take across the rotary axis, so it would have to lean along it", "too narrow ...", and "needs a lean of n degrees from the surface normal, more than the largest lean on hidden surfaces (m degrees)".
     - The warning's parameter is `max_hidden_lean` when any area needs more lean, so a host can point at it.
     - Nothing in it implies the rest of the program is safe (NFR-12).
   - **The note.** A note says what was done: "surfaces hidden from the drive (such as the walls of a slot, an undercut, or sides of the section facing each other) between X a and b are finished with passes of their own after the main ones, the tool leaning from the surface normal as little as the part lets it reach them: up to n degrees (at X, A); the shank ... ; the tool holder is not checked".
   - **The viewport.** Highlighting the areas in the viewport and going to each is a later interface lane's (OI-1451, issue #416), reading `unreached`.
7. **The shank.**
   - `StrategyInput` gains `shank: Option<Shank>`: the tool's shank where it is wider than the cutting part, a cylinder of its radius from its height above the tip upwards. `openchuck-app` fills it from the tool (the shank's radius from the flute length) when the shank is the wider. Every other host passes `None`, and the tool above its cutting part is then a cylinder of its cutting radius, as every drop takes it.
   - The passes on hidden surfaces keep the shank clear of the part. This is checked at each point by dropping the tool's outline with the shank (the cutting profile, then the shank's radius from the flute length) and comparing it with the cutting part's drop, and at each test pose of the fitter.
   - The drive's passes do not check the shank, as no strategy did before (COL-02, P2; OI-1452). The holder is not checked anywhere (COL-04, P2), and the note says so.
8. **A drop with a lower bound.**
   - `SurfaceSource` gains the provided method `drop_tool_above(profile, line, at_least)`, the same answer as `drop_tool` for a line on which the tool is known to touch the surface at `at_least` or above. The default ignores the bound. `MeshSurface` uses it as the drop's floor (a hair below, so a tie is decided as without it), when the line's own highest surface point gives none or a lower one.
   - The tool placed on a hidden wall, with its axis along the wall, is on a line that often misses the surface, and then every triangle within the tool's radius of the line was tried. The answer is the same.
9. **Always on (owner's decision 3, B).** There is no setting. A saved project whose part has hidden surfaces the tool can reach gets the new passes when it is regenerated, and one whose part has hidden surfaces the tool cannot reach gets the warning. Every other project's program is unchanged. The user's control is `max_hidden_lean` (0: no lean from the normal, so no side cutting).
10. **Determinism (NFR-06, GC-07).**
    - Sections, chains (joined by edge keys made from the end points' bits, so a welded and an unwelded mesh chain alike) and their points are made in the triangles' order.
    - Sections and contours go through `parallel::map_in_order`, and the search for an axis runs in a fixed order along each contour.
    - Areas and passes are sorted on fixed keys. The output is identical at 1, 2 and N threads.

## Alternatives considered

- **A unified drive along each section's outline instead of rays** (owner's decision 1, option B). Cleaner in the end, with no seam between the drive's passes and these, but it changes every simultaneous program, the `lobed` golden included. Deferred by the owner: OI-1450, issue #415, after OP-20 is proven.
- **Closest-point drive curves** (OP-20's original example). Points projected from a grid near the part reach hidden walls, but their order is not defined: neighbouring grid points jump between surfaces across a thin wall. Used here only to put a point on the exact face of a STEP part.
- **Offset-surface drive curves** (the other example). They are a true offset only for a ball's centre (a flat or tapered tool's depends on its axis), robust 3D offsets of meshes are costly, and they say nothing about the shank. The drop gives the tool's position for any profile instead.
- **Reporting only** (decision 1, option C), and **a setting** for the hidden passes (decision 3, options A and C). Not chosen by the owner.
- **The lean held to the largest tilt** (decision 2, option C). Most slot walls need the tool's side, and would only be reported.
- **A corner rule by distance alone** (unreached points within a tool radius of reached ones). It also hid the walls of a slot narrower than the tool, within a radius of their reached top edges. The turn of the surface tells a corner from a gap.
- **Growing the runs by one stepover only.** On two pins side by side the drive's grazing coverage ended 0.5 mm before the hidden run began, and 0.071 mm was left there. Growing by the tool's radius covers it.
- **Checking the shank on the drive's passes too.** Safer, and unchanged wherever the shank does not touch the part, but it changes a P1 strategy's behaviour beyond OP-20: left to OI-1452 for the owner.

## Verification

Tests named after the requirements (AGENTS.md), written first and seen failing. Times are alone on one test thread, uninstrumented, on this lane's container (4 vCPU):

- `openchuck-path/tests/simultaneous_hidden.rs`. Each test measures points of the exact part against the tool's outline (`openchuck-testkit::gouge::Outline`) at every fed pose, independently of the strategy:
  - `req_op_20_req_op_12_the_walls_of_a_keyway_are_finished_and_nothing_is_cut_in`: a 6 mm slot 3 mm deep in a 20 mm shaft, 3.175 mm ball. The walls are left within 0.004 mm above the corner strip, nothing is cut in beyond the tolerance (deepest 0.002 mm), nothing is reported, and the note is given. Before: the walls were left uncut (the test failed on the note; a 4 mm slot is cleaned by the drop alone, so the test uses 6 mm).
  - `req_op_20_a_slot_narrower_than_the_tool_is_reported_and_not_cut_into`: a 2.5 mm slot. Too-narrow areas over 60 to 150 mm2 at A near the slot, the warning and its closing words, nothing cut in.
  - `req_op_20_under_the_lips_of_a_dovetail_groove_is_blocked_and_reported`: a groove round the axis, wider at its bottom than its opening. The bottom under each lip is reported blocked, all round, and nothing is cut in.
  - `req_op_20_two_pins_side_by_side_are_cut_all_round` (OI-1431): two pins of radius 4, 4 mm apart, at one X. Both are left within 0.011 mm all round, and nothing is cut in or reported.
  - `req_op_20_a_gap_narrower_than_the_tool_between_two_pins_is_reported`: 2 mm apart. A too-narrow area over more than 8 mm of X.
  - `req_op_20_blades_without_a_hub_are_finished_on_both_sides`: three radial blades 2 mm thick. Both sides left within 0.03 mm and nothing cut in. Where the rays aimed at the section's centre meet a side squarely enough, the drive finishes it with its own scallop (up to 0.024 mm measured).
  - `req_op_20_a_wall_needing_more_lean_than_the_limit_is_reported_with_its_parameter`: `max_hidden_lean` 30 on the 6 mm slot. The walls are left, reported as needing 30 to 90 degrees, and the warning names the parameter.
  - `req_op_20_req_op_12_a_shank_wider_than_the_cutter_is_kept_out_of_a_slot`: a 1 mm ball with 1.5 mm flutes on a 3.175 mm shank, in a 3 mm slot. On the passes on hidden surfaces the shank never meets the part (deepest 0.0096 mm), the walls' tops are finished, and the deeper walls are reported too narrow.
  - `req_op_20_req_op_06_on_a_hidden_wall_the_tool_axis_is_the_reachable_one_nearest_the_normal`: with no smoothing, turning the tool 3 degrees nearer the wall's normal about the ball's centre makes it meet the part, at 74 poses on the walls.
  - `req_op_20_req_op_12_the_crankshaft_is_finished_where_reached_and_nothing_is_cut_in` (`stl/crankshaft.stl`, 3.175 mm ball, 0.3 mm stepover): generated in 6 s; nothing is cut in beyond the tolerance at any of its 4,512 vertices away from the ends (deepest 0.006 mm). Three areas are reported, all where the first pin meets the web at X 10, with a combined area of 3.6 mm2: 3.2 mm2 blocked near A 180, and two of 0.2 mm2 too narrow near A 137 and A 223.
  - `req_op_20_req_nfr_06_hidden_surfaces_are_identical_at_1_2_and_n_threads`, and `req_op_20_a_part_with_nothing_hidden_gets_no_hidden_passes_or_reports` (a plain cylinder).
- **Test times** (alone on one test thread, uninstrumented): the blades 79 s and the keyway's walls 69 s, listed as long-running in `.config/nextest.toml`. The others take 0.6 to 58 s (the two pins 4 mm apart 44 s, 2 mm apart 58 s, the crankshaft 13 s), 7.7 minutes in all for the sixteen. Most of each test's time is the independent check, not the generation (6 to 24 s).
- **On exact faces.** `req_op_05_exact_faces_drive_the_path_with_their_own_normals` (an exact B-rep cylinder) found the first version taking the whole cylinder as hidden (item 1), and passes again. No test drives hidden surfaces on a STEP part's exact faces yet: they use the same contours, placed and aimed by the faces' own points and normals.
- Unit tests in `simultaneous/hidden.rs`:
  - sections of a box, closed, counter-clockwise, with outward normals, and through a ring of vertices;
  - an open mesh's open chain;
  - areas all round or not;
  - the outline with a wider shank.
- `openchuck-testkit`:
  - `parts::prism` (a section along X in bands, its ends capped by ear clipping) and `parts::join`;
  - `parts::revolve` documented for a profile that turns back, with tests that each is closed and outward.
- Unchanged: every other test of `openchuck-path`, `openchuck-app` and `openchuck-testkit` (1,249 of 1,250 in one run, the 1,250th the exact cylinder above, since fixed), among them `simultaneous.rs`, `simultaneous_off_axis.rs` (OP-26, the crankshaft's pins), `helical_faces.rs`, the region and wall tests and the reference jobs. The golden programs, `lobed`'s included, are byte for byte the same.

## Consequences

- **Requirements (owner's decision 5, A, with OP-20 raised to P1; agreed 31 Oct 2026, relayed).** `requirements.md` OP-20 now reads:

  > Simultaneous 4-axis finishing of surfaces that a ray from outside the part towards the rotary axis (or towards the section's own centre, OP-26) does not meet first, such as undercuts, the far walls of slots and pockets, and sections in several separate pieces at one X, wherever the tool, with its axis in the plane across the rotary axis, can touch them without its cutting part or shank meeting the part elsewhere. There the tool axis is the reachable direction nearest the normal, within a user-set largest lean. Surfaces the tool cannot reach are reported with where they are, how large they are and why (blocked by the part, too narrow for the tool, or needing more lean than the limit). Off-axis sections are covered by OP-26.

  Its priority is P1. This supersedes "P2" in accepted records 0035, 0036 and 0038, which are left as they are. OP-06 is unchanged.
- **Verification plan.** The OP-20 row becomes P1, T, A, System: "On synthetic parts with a keyway, a dovetail slot along the axis, a dovetail groove round it, two off-axis pins side by side and blades without a hub, and on the crankshaft: every hidden surface point the tool can reach (classed independently of the strategy) is finished within the tolerance plus the stock to leave; the independent checker reports no gouge beyond the tolerance by the cutter or the shank; every unreachable hidden region is named in the operation's warnings with its X range, angle, area and reason, and no reachable region is; the tool axis is within the lead and tilt limits wherever they reach the surface and otherwise at the reachable direction nearest the normal, never beyond the largest lean; output is identical at 1, 2 and N threads, and a part with no hidden surface gives the same program as before."
- **Work breakdown.** 102 of 175 requirements are P1 (OP 21). OP-20 is built by lane OP20 on slice 13's strategy and closes with slice 20, as OP-26 does.
- **Library API (breaking; decision 0048).** The workspace minor goes to 0.29.0, with changelog entries:
  - `StrategyOutput::unreached`, `GeneratedPath::unreached` and `StrategyInput::shank` are new public fields;
  - `Unreached`, `UnreachedReason` and `Shank` are new types;
  - `SurfaceSource::drop_tool_above` and `simultaneous::parameter::MAX_HIDDEN_LEAN` are additive;
  - every host's `StrategyInput` literal gains `shank`.

  No project schema number: `max_hidden_lean` is an operation parameter (decision 0008 item 6; work breakdown, "Project schema ordering", rule 3), and a saved operation without it takes its default, 90.
- `openchuck-path`:
  - the new private module `simultaneous::hidden`;
  - in `simultaneous.rs`: contour tracks, the band smoothing (`smooth_gates`) and the fitter's four contour rules;
  - `cutter::drop_on_mesh_above`.
- `openchuck-app`: `operation.rs` gives the strategies the shank.
- `openchuck-testkit`: `parts::prism` and `parts::join`.

## Open items

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | A unified per-section contour drive in place of the ray drive (owner's decision 1, option B). Deferred by the owner: issue #415. (OI-1450) | After OP-20 is built, merged and proven | Byte-identical output for parts with nothing hidden, or an owner-approved golden change |
| 2 | Highlight the unreached areas in the viewport and go to each, reading `GeneratedPath::unreached` (owner's decision 4). Deferred by the owner: issue #416. (OI-1451) | A later interface lane | This lane merged |
| 3 | The drive's passes of simultaneous finishing (and the other strategies) do not check a shank wider than the cutting part: with a 1 mm ball on a 3.175 mm shank, the spiral took the shank 1.09 mm into a 3 mm slot's edges. The passes on hidden surfaces do check it (item 7). Checking it on every pass changes the program only where the shank would meet the part. (OI-1452) | The owner, or COL-02 (P2) | A decision |
| 4 | Faces square to the rotary axis under an overhang (the side walls of a T-slot groove round the axis) lie in no section, so they are neither driven nor reported; the groove's floor under the lips is reported. (OI-1453) | When such a part is to be finished | A test part and a sampling across X |
