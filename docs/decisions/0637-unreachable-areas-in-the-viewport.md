# 0637: The viewport marks the surfaces simultaneous finishing cannot reach, and Messages goes to each (OP-20, UI-11, UI-12, NFR-12, NFR-15; issue #416; lane VIEW-OVERLAYS)

- **Status:** accepted by the project owner on 2 Nov 2026, relayed by the coordinator (decision 0049). The owner's answers to questions 1 to 4 (1 Nov 2026) were relayed the same way. Its number is provisional: under the owner's numbering rule (1 Nov 2026) it takes the next free number after `main`'s highest when it is released.
- **Date:** 2 Nov 2026.
- **Issue:** #416 (OI-1451).
- **Requirements:** OP-20, UI-11, UI-12, SIM-05, NFR-06, NFR-12, NFR-15, GC-07. No requirement changes.
- **What it amends:** [decision 0368](0368-simultaneous-finishing-of-hidden-surfaces.md) item 6 (the unreached areas as structured data; the viewport left to a later lane) and its open item 2; the interface specification ([decision 0038](0038-ui-specification-and-requirements.md)), sections 11.4 and 11.5 and the Show menu. It builds on [decision 0216](0216-selection-set-colours-and-deviation-scale.md) (the colours, hatches and outlines already taken), [decision 0238](0238-simulate-display-and-operation-navigation.md) item 3 (the Show menu) and [decision 0492](0492-deviation-colours-on-the-part-model.md) (the part's deviation colours, which the areas lie over).
- **Open items:** OI-1451 closes when this is built. OI-1991 (new): a reduced-motion setting, so that Show in view's turn can be animated; until then the turn is instant (owner, option A). OI-1457 (walls square to the axis under an overhang are not reported) stays open and is a known limit here: what the strategy does not report is not marked.
- **Library:** `openchuck-path` changes (a new field on a public struct), so the library family takes the next minor version at release (decision 0048; provisionally 0.36.0, the Merge Manager assigns it).

## Context

Simultaneous finishing reports the areas of the part it cannot reach (OP-20; decision 0368 item 6): a warning lists up to five, largest first, with their X range, the A at which each faces the tool, area and reason, and `GeneratedPath::unreached` carries all of them as data. The owner asked for them to be highlighted on the part, and for each to be reached from its warning, so the user can check them against the simulation before running the program (NFR-12).

An area is given as an X range by an A range. That box also takes in surfaces the tool did reach: under a dovetail's lip, the lip's outside lies in the same X and A range as the floor below it. Marking every triangle in the box would mark finished surfaces as unreachable.

## Decision

1. **Exact points (owner, question 1, option A).** `openchuck_path::Unreached` gains `points`: the surface points the strategy sampled and could not reach, in its own order, taken evenly to at most `MAX_UNREACHED_POINTS` (4,096) per area. The X range, A range, area, reason and the warning are unchanged. Output stays the same whatever the thread count (NFR-06, GC-07).
2. **The part's triangles.** The application keeps each operation's areas with its toolpath and, off the interface thread, finds the part triangles the points lie on (the nearest triangle within a fixed 0.5 mm). Those triangles are what is marked. An area's marker sits at its point nearest the area's middle.
3. **Where and when (owner, question 2, option A).** On Operations and on Simulate, whenever the operation's toolpath is shown (its tree eye is on). The Show menu gets "Unreachable areas", on by default, menu-only like "Held regions and tabs" (decision 0238 item 3). Nothing is shown from a generation that is out of date or failed: the areas go with the toolpath they came from.
4. **Look (owner, question 3, option A).** A two-tone dot pattern on the area's triangles, drawn over whatever colours lie beneath them (the part's grey, the deviation colours of decision 0492), and a numbered marker per area: the numbers follow the warning's order, so "area 2" in Messages is "2" in the view. The legend gets a row "Unreachable (operation name)". Dots are a pattern no other role uses; the colours, dot size and spacing are measured against the checks of decision 0216 (3:1 against every band and the part's grey, each simulated colour vision type, 100% and 120% scale) and given under "As built".
5. **Show in view (owner, question 4, option A).** Each area the warning lists gets "Show in view N" in Messages. On Simulate it stays on Simulate; from any other step it goes to Operations. It selects the operation, orbits the camera to look straight at the area (at the middle of its A range, from the side it faces), frames its X range with a margin, and pulses its marker. The part stays in its setup placement and Simulate's playback does not move. The move takes about 0.3 s, and is instant with reduced motion; it is one camera step that can be undone, like Fit. Areas past the fifth have their markers in the view only, as the warning counts them.

## Specification entries proposed

- **11.4, Viewport,** and **11.5, Viewport:** "**Unreachable areas** *(OP-20; decision 0637)*. The surfaces an operation could not reach are dotted (two-tone dots) with a numbered marker each, in the warning's order, while its toolpath is shown and its result is current; the legend names them 'Unreachable (operation name)'."
- **The Show menu:** "Unreachable areas", on by default.
- **The View menu:** "Previous view" steps the camera back through Fit, the standard views and Show in view (owner, 2 Nov 2026, relayed: kept).
- **12, Messages:** "A warning listing unreachable areas offers 'Show in view N' for each: the view turns to the area and frames it; the part and the playback do not move."

## Alternatives considered

- **Mark every triangle in the X and A box** (question 1, option C, or drawing the box as a wedge, option B). Rejected: it marks reached surfaces as unreachable (the lip above an undercut), or shows where an area is but not which surface.
- **Only on demand from Messages** (question 2, option B). Not chosen: easy to miss, and the part can look clean when it is not.
- **A fill or an outline** (question 3, options B and C). Not chosen: a fill hides the deviation colours and few colours remain; outlines and hatches already mean sets, avoid surfaces, held regions and stale sets.
- **Turn the part to the area, or move the playback** (question 4, options B and C). Not chosen: the part would sit in a pose the program never gives it on Operations, and the user would lose their place in the playback.

## Consequences

- `crates/openchuck-path`: `Unreached::points`, `MAX_UNREACHED_POINTS`; a minor version and a changelog entry.
- `crates/openchuck-app`: the regeneration keeps each operation's areas; the mapping to the part's triangles as a job.
- `crates/openchuck-viewport`: a dot pattern beside the single and cross hatch.
- `apps/openchuck-ui`: the areas in the scene, the markers and legend row, the Show menu tick, "Show in view" in Messages.

## As built

Built on `claude/lane-view-overlays-416`. Where the build chose something the items above leave open, it is said here.

- **Points (item 1).** `Unreached::points` is sampled evenly by a stride of `ceil(n / 4096)`, so an area keeps its points in the strategy's order. Test: `req_op_20_unreached_areas_carry_their_surface_points` (openchuck-path), with the NFR-06 thread test unchanged.
- **Matching (item 2).** Each point is taken to the nearest triangle within a fixed 0.5 mm (`openchuck_app::unreached::MATCH_TOLERANCE`), found through a uniform grid ordered by cell (no hash), ties to the lower index. A point near no triangle marks nothing and is counted. 0.5 mm is used in place of "one sample spacing": it is of the order of the strategy's spacing and well above a STEP part's tessellation deflection. On the dovetail fixture with the part offset by 3 mm, every point lands on the scene's mesh and no lip outside is marked (`req_op_20_areas_map_to_the_triangles_they_lie_on`, `req_op_20_the_areas_of_a_run_lie_on_the_part_the_view_draws`). The job is refused with `PartChanged` when the model's file is not the run's, and then no areas are drawn, as for the colours of decision 0492. The marker is the area's point nearest the mean of its points.
- **When (item 3).** An operation's areas are drawn on Operations and Simulate while the Toolpaths choice and the operation's eye are on and its path is current (`Application::operation_unreached` is empty for a path edited or failed since the run). The tick "Unreachable areas" is in the Show menu only, on by default, and is kept for the session only, like "Held regions and tabs". Messages finds the warning by its start, "simultaneous finishing cannot reach "; a test pins that against the strategy's real warning.
- **Look (item 4).** `openchuck_viewport::HatchKind::Dots { ring }` beside `Single` and `Cross`, drawn in a new layer `Areas` between the tool and the overlay, over whatever the part shows. The dots are 12 px apart in rows staggered by half a spacing; the core is 4 px across, the ring 7 px (logical pixels, so 14.4, 4.8 and 8.4 physical pixels at 120%). The colours are a core of `#0c0e12` and a ring of `#ffffff`, 19.3:1 against each other. A dot reads by the better of its two tones on each ground; the worst case is the hidden-band slate at 3.28:1:

  | Ground | Core contrast, normal / worst simulated | Ring contrast, normal / worst | Core ΔE00, normal / worst |
  | --- | --- | --- | --- |
  | Overcut | 4.96 / 3.73 | 3.90 / 3.36 | 48.1 / 38.5 |
  | Within tolerance (and not measured) | 7.33 / 7.29 | 2.64 / 2.61 | 50.7 / 50.5 |
  | Up to 0.25 mm | 16.92 / 16.47 | 1.14 / 1.10 | 91.9 / 89.5 |
  | 0.25 to 1 mm | 8.72 / 8.09 | 2.22 / 1.98 | 61.2 / 58.1 |
  | More than 1 mm | 4.48 / 4.25 | 4.31 / 3.85 | 42.4 / 42.0 |
  | Part grey | 10.41 / 10.35 | 1.86 / 1.83 | 64.8 / 64.5 |
  | Hidden-band slate | 3.28 / 3.25 | 5.90 / 5.80 | 27.9 / 27.7 |

  Contrast does not depend on scale. Tests: `req_nfr_15_unreachable_dots_read_on_every_band_and_the_part` (roles) and `req_nfr_15_a_dot_pattern_is_drawn_in_two_tones_over_what_lies_beneath` (render, at two zooms and pixel density 2). Markers and the legend row "Unreachable (operation name)" are drawn at 12 points or more.
- **Show in view (item 5).** Messages offers "Show in view N" under the warning for each listed area. On Simulate it stays on Simulate; from any other step it goes to Operations. It selects the operation, and looks along the tool's direction at the middle of the A range (an area all round is looked at from its marker's side), framing its X range, and pulses the marker. The part and the playback do not move. When the operation is in another setup, the scene is rebuilt for it first; a request waiting for the job is dropped if the result goes stale. The view keeps a camera history of 32 steps, recorded by Fit, the standard views and Show in view, and the View menu gets "Previous view" to step back (no shortcut). This menu entry is not in the specification; the owner kept it (2 Nov 2026, relayed) and it is proposed with the entries above. **The turn is instant.** The 0.3 s orbit is built and tested, but no setting for reduced motion exists, so it is not turned on: the owner chose to keep the turn instant (OI-1991, option A, 2 Nov 2026, relayed). A reduced-motion setting is left to a later issue (OI-1991 stays open for it).
- **Not run:** the coverage check (the instrumented build did not fit the container's disk).
