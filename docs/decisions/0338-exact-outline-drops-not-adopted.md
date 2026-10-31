# 0338: Exact drops for the tapered ball and the flat end, built but not adopted (lane STEPALL-X3; issue #350)

- **Status:** accepted by the project owner, relayed by the coordinator, decision 0049 (30 Oct 2026, Europe/London). The owner read the condensed record in the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7) and approved it there. The owner chose the outcome before it (option A on OI-1351, also relayed): the outline drop is merged as a tested library capability and no strategy adopts it. Proposed 30 Oct 2026.
- **Date:** 30 Oct 2026 (Europe/London).
- **Lane:** STEPALL-X3, branch `claude/lane-stepall-x3`, from `main` at `4dbccee` (library family 0.27.0). Issue #350 (STEPALL-0211 step 5 of 6), parent #257.
- **Requirements:** IMP-02, OP-04, OP-09, OP-11, OP-12, OP-21, OP-23, SET-09, GC-07, NFR-04, NFR-05, NFR-06, NFR-07, NFR-12. Verification plan 4.1 and the *Exact surfaces* note of section 5.
- **Builds on:** [decision 0209](0209-step-geometry-for-every-strategy.md) (item 4.2, the outline form of the exact drop), [decision 0211](0211-step-accuracy-measured.md) (the 2-times limits of OI-835), [decision 0255](0255-bounded-tool-contact-checker.md) (the bounded checker), [decision 0260](0260-exact-face-and-trim-queries.md) (the queries), [decision 0277](0277-exact-ball-drops.md) (the exact ball drop, whose branch and bound this extends).
- **Open items:** OI-1350 and OI-1351 new. Progresses OI-81 (the outline drop exists; adoption is OI-1351) and OI-835 (measured for the outline: not met). OI-1110, OI-1111 and OI-1112 are unchanged (no strategy's contact changed, so the harness and the containment boundary were not touched).
- **Numbers:** record 0338 and OI-1350 to OI-1351 from the coordinator's allocation (0338 to 0342, OI-1350 to OI-1369), checked free on `main` and every unmerged branch on 30 Oct 2026. No project schema number. No library version change: the API is additive (`BrepSurface::drop_exact`, `has_exact_drop`); see *Verification* for `cargo semver-checks`.

## Context

Issue #350 asks for the outline form of the exact drop for axial finishing with a tapered ball and pencil clean-up with a flat end, measured against the bounded checker and both 2-times generation limits (OI-835), and adopted per strategy only where both limits are met. Decision 0277 built the exact drop for a ball: a branch and bound on the exact faces with proven bounds, offset-surface roots in closed form for the analytic faces, and hull cells with a normal-cone test for the B-spline faces. On the tessellation, decision 0277 measured pencil clean-up cutting 0.0342 mm and axial finishing 0.0428 mm into the blisk's exact faces.

## Decision

1. **The outline drop (built).** A tool whose lower envelope is `h(rho)` (height above the tip at `rho` from the axis, out to its radius `R`) first holds a face point `p` at the tip offset `lift(p) = z - h(rho)`. For a convex envelope that never falls, the lift is a concave function of `p` inside the cylinder `rho <= R`. `BrepSurface::drop_exact` finds the largest lift over the faces by the branch and bound of decision 0277: every point it evaluates is a real contact, and every part it passes over has a proven upper bound within `DROP_TOLERANCE` (1e-4 mm) of the best found, or the tip is raised to the bound where a search reaches its budget (a gap, never a gouge; `ExactDrop::raised`).
   - **Bounds.** For any `v` across the axis, `lift(p) <= h*(|v|) + (a - v) . (p - o)` on a box, `h*` the conjugate of the envelope over `0..R`. Its largest value over the box is convex in `v`, and its least value is the largest lift over the box (the minimax theorem), so the bound is exact for each box. The tangent plane at the box's middle with a multiplier for the rim gives the first `v`. The search for the least `v` (the stationary point of each piece, the creases where two pieces meet, and a short search along each crease) runs only where it can pass the box over: within 0.0004 mm of the best. Boxes are the B-spline hull cells of decision 0260 (along their frames), boxes about pieces of an analytic face's parameters (from second-derivative bounds, split across the way the face and the tool bend most, at most four times longer than wide), and boxes about pieces of a trim loop's image.
   - **Where the highest contact can be.** Inside a face, at a tangency (the face's normal along the tool's normal at that radius), or on an edge circle of the outline (the rim, or a joint where the slope jumps) where the face's normal has no part about the axis. A cell whose normal cone allows neither, and that is not along a crease, is passed over: the normal-cone test of decision 0277 for the outline. Where a cell crosses an edge circle, its middle is moved onto it (Newton steps on the distance from the axis); where the trim loop's image crosses one, the crossing is found by false position.
   - **The rim.** A part that reaches the tool only within `RIM_SLACK` (0.0001 mm) of its radius may be passed over. Without it, a face grazing the rim far up the line kept the search to its budget and left tips up to 17 mm high on the blisk during development. The answer is then never below the drop of the tool made 0.0001 mm narrower, and the tool cuts at most 0.0001 mm into such a part, across the axis.
2. **Which outlines** (`has_exact_drop`, the owner's scope, relayed). Supported: the ball (decision 0277's drop), the tapered ball (also grown for stock to leave), the flat end at zero stock to leave, and any convex outline of lines of finite slope and arcs centred on the axis that end before they turn upright and leave the axis level (a V-bit or chamfer with a flat at its tip passes the same tests in the library, but is not measured on a part). Not supported, staying on the tessellation: a bull nose (an arc off the axis, upright at the rim), a flat end grown for stock to leave (which is a bull nose), a sharp V-bit and a concave custom profile (OI-1350).
3. **Not adopted (the owner's choice, option A on OI-1351).** Both 2-times limits fail by far for pencil clean-up and axial finishing (*Measurements*). `Strategy::exact_contact` stays false for both; `SurfaceSource::drop_tool` on a `BrepSurface` with exact drops still drops only a ball on the exact faces; the application, its face sets and its notes are unchanged. No strategy's output changes. The outline drop is reached only by `BrepSurface::drop_exact`. Closed-form contacts for the outline on analytic faces (as decision 0277 has for the ball) and cheaper bounds are OI-1351, for a later lane or STEPALL-Q (#351), to be measured against both limits before any adoption.
4. **Not built yet.** STEPALL-P's crease selection and the between-rest move protection are untouched (this lane changes contact only, and changes no strategy). The containment boundary is still the tessellation's window (OI-1112). Decision 0209 item 4.2 suggested seeding the outline drop from the tessellation and refining by Newton; this lane follows decision 0277's bounded search instead, since the tessellation's error has no proven bound (decision 0211 M1).
5. **Kernel isolation and API.** Everything is in `openchuck-path` on the plain B-rep data: the new `brep/outline.rs`, `brep.rs` and `brep/hull.rs` (`Para`, `Bound::paras`). Additive public API: `BrepSurface::drop_exact`, `has_exact_drop`.

## Verification

Tests written from the requirements, without the kernel (`crates/openchuck-path/tests/brep_outline_drop.rs`, `src/brep/tests.rs`), on five outlines: the owner's tapered ball (3.175 mm, 0.25 mm tip, 5 degrees), a wider taper (25 degrees), flat ends of radius 1 and 2.5, and a flat-tipped V-bit:

- every kind of trimmed face (plane, cylinder, cone, sphere, torus, a rational, a polynomial and a creased B-spline), 24 random lines each, against the highest lift of a brute-force sampling written out independently: never below a sample (less the tolerance), and the contact is a point of the face that the tool holds at the answer (`req_op_12_exact_outline_drop_of_every_kind_is_at_or_above_brute_force`, 38 s alone uninstrumented);
- closed forms: a plane tilted from square to 85 degrees (flat bottom, flat rim, tapered tip, tapered flank, the flank lying along the plane), within the tolerance; a tapered ball grown for stock to leave on a tilted plane; a flat end across a cylinder's seam; a flat end and a taper over a trim edge; a flat end on the point of a cone;
- the same answers on four threads and in reverse order; a line that is not a number gives no answer; the bull nose, the grown flat end, the sharp V-bit and a concave profile have no exact drop, and a surface with exact drops still drops every outline on the tessellation (not adopted);
- region and avoid face sets judge outline contacts as exact faces (OI-833 for the outline);
- the bound is never below the lift of any point of its box, on 400 random boxes and lines per outline, and is exact for a box shrunk to a point (`req_op_12_the_outline_bound_is_never_below_a_lift_in_its_box`).

During development, the application was also run with pencil and axial adopting the drop: its face-set tests (decision 0210, the axial tapered ball and the helical flat end) passed, and were removed again with the adoption.

## Measurements

Machine: Intel Xeon @ 2.10 GHz, 4 cores, cloud container, release build. Part: `testdata/step/valid/blisk.step`, placed as decision 0211 places it. Harness: decision 0211's `step_accuracy` with pencil and axial temporarily adopting the drop (form d), against the import tessellation (form b), at quick stepovers, one run each, all threads: indicative only, not the five-run medians OI-835 requires (the result was far enough from the limit that the owner decided on these). Source: the lane's commit `63002e67` and the commits just before it.

**Accuracy** (the bounded checker of decision 0255, the carried-stock sequence roughing, rest, pencil, axial; every fed pose and the quarter, middle and three-quarter points of every fed move):

| Operation | Form | Deepest cut mm | Proven at most | At the poses | Largest gap mm | Within 0.01 mm (cut, gap) |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Pencil clean-up (2 mm flat) | b | 0.0342 | 0.0347 | 0.0342 | 0.0065 | **no**, yes |
| | d | 0.0066 | 0.0071 | 0.0001 | 0.0022 | yes, yes |
| Axial finishing (3.175 mm tapered ball) | b | 0.0414 | 0.0419 | 0.0414 | 0.0101 | **no**, **no** |
| | d | 0.0041 | 0.0046 | 0.0001 | 0.0045 | yes, yes |

**Generation** (seconds, four threads, quick stepovers, one run):

| Operation | b | d | d/b |
| --- | ---: | ---: | ---: |
| Pencil clean-up | 1.95 | 60.5 | 31 |
| Axial finishing | 1.51 | 11.0 | 7.3 |

**One drop, one thread** (microseconds). Lines kept from those runs (every 97th drop: 3,000 pencil lines, 900 axial lines) replayed on the placed blisk:

| Tool | Tessellation | Exact |
| --- | ---: | ---: |
| Flat end (pencil's lines) | 8.4 | 230 |
| Tapered ball (axial's lines) | 49 | 216 |

On decision 0260's lines (`step_queries`: along the normal at every eighth vertex moved 1 mm off), the per-part table of the final source is printed by `step_queries` in its `flat-mesh`, `flat-exact`, `taper-mesh` and `taper-exact` rows; on the blisk it is in the same range (*Consequences*).

**Why it is slow.** A drop bounds 100 to 130 cells on average, at about 2 microseconds each. Most are near the contact, not on far faces: starting the search from the right answer less 0.04 mm saved about 7 %. A flat end in a pencil crease rests along a line of the hub (its bottom on a cylinder's generator) and against trimmed faces (the hub under the blade roots), where many cells lie within the tolerance of the answer and straddle a trim. Decision 0277 has no cells on analytic faces at all: their interior contacts are roots in closed form. The outline needs the same for each piece of the outline (the ball tip, the cone, the flat) against each kind of face, and for the stationary points of the rim along a face (OI-1351).

## Alternatives considered

- **Adopt for the accuracy and waive the limits** (option C): the owner declined; both limits would fail (pencil and axial dominate the job).
- **Continue in this lane with the closed forms** (option B): the owner declined; the speed needed for pencil (about 15 times) is not assured, and STEPALL-Q (#351) shares the files.
- **Seed the search from the tessellation's drop** (decision 0209 item 4.2): no proven bound on the tessellation's error; and as a starting floor it saved only about 7 %.
- **A tangent-plane bound only:** loose to the second order, and where the taper's flank lies along a face (a line contact) the search reached its budget and stood 0.0002 mm high; the exact dual settles it.
- **Splitting cells only across the way the face bends:** a flat end along a cylinder made cells 1e-10 wide and 0.9 mm long, never settling where a trim crossed them; the length is capped at four times the width.

## Consequences

- **Output:** none. No strategy, note, face set or golden file changes; STL parts and every other strategy are unchanged.
- **Library:** `BrepSurface::drop_exact` and `has_exact_drop` are available for a later lane to adopt (OI-1351).
- **Documents:** this record; the decision index; `docs/open-items.md` (OI-1350 and OI-1351 new; OI-81 and OI-835 noted); the verification plan's *Exact surfaces* note; the work breakdown's STEPALL row; the changelog of `openchuck-path`.
- **Later lanes:** STEPALL-Q (#351) and the lane that takes OI-1351 edit the same files this lane changed: `crates/openchuck-path/src/brep.rs`, `brep/outline.rs` (new), `brep/hull.rs`, `brep/tests.rs`, `src/lib.rs`, `tests/brep_outline_drop.rs` (new), and `crates/openchuck-app/examples/step_queries.rs`. `surface.rs`, `region.rs`, `composite.rs`, `strategy.rs`, `operation.rs`, `selection.rs` and `step_accuracy` are unchanged.
