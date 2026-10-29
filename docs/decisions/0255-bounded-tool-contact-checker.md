# 0255: A bounded tool-contact checker for M2 (lane STEPALL-T; issue #346)

- **Status:** accepted by the project owner on 28 Oct 2026 (Europe/London). The owner read the condensed record in the coordinator session and answered "Approve" there; the acceptance was relayed to this lane session by the coordinator session (decision 0049). Proposed 28 Oct 2026. It changes no product output, default, schema or library API.
- **Date:** 28 Oct 2026 (Europe/London).
- **Requirements:** OP-12, OP-23, NFR-05, NFR-06, NFR-12. Verification plan 4.1 (toolpath deviation budget; the new tool-contact row).
- **Builds on:** [decision 0209](0209-step-geometry-for-every-strategy.md) item 4.1, M2 (accepted: "sampling adds under 0.001 mm"); [decision 0211](0211-step-accuracy-measured.md) (accepted: the flat-cutter counterexample and the unqualified M2); decisions [0115](0115-path-fitting-tests-the-quarter-points.md) and [0133](0133-simultaneous-fitter-tests-the-quarter-points.md) (the stations).
- **Open items:** [OI-1020](../open-items.md) new (trim polylines against exact trim curves). Supplies evidence for OI-834 and OI-835 (lane STEPALL-T, issue #347) and later OI-81 without closing them.

Numbering: on 28 Oct 2026 the highest record on `main` (33a835d) and on every fetched branch was 0252, and the highest open item OI-1003; the lane's allocation is 0255 to 0259 and OI-1020 to OI-1039, and neither 0255 nor OI-1020 is used anywhere. No project schema number: nothing stored changes.

## Context

Decision 0209's M2 judges each tool station against points sampled on the exact faces and asks that sampling add under 0.001 mm. Decision 0211 showed that it does not: on an analytic plane with pencil's 2 mm flat cutter the 0.05 mm grid reads 0.003333 mm of penetration where the exact value is 0.010000 mm (a miss of 0.006667 mm), halving the spacing gives the same miss, and a finer grid is not monotonically better. A ball's sagitta is no bound for a flat cutter, and a uniform UV grid gives no physical spacing on a nonuniform B-spline. M2 was therefore made to fail until a proven bound exists. Issue #346 asks for an independent checker with a proven bound below 0.001 mm (or a bounded replacement), covering ball, tapered-ball and flat cutters, nonuniform B-spline faces, trims and boundaries.

## Decision

Replace M2's sampled clearance with a **bounded** one. For one tool pose, the checker returns an interval `[lower, upper]` proven to hold the exact smallest signed clearance between the tool and the geometry (negative inside the tool), or fails. `upper` is the clearance at a point of the geometry it found; `lower` is proven. M2 qualifies only when every station's interval is narrower than the tolerance.

1. **The tool is convex, so its clearance is convex.** The cutting profile (flat, ball, tapered ball, bull nose) revolved about its axis and capped flat at the profile's top is a convex solid; the checker refuses a profile that is not (a concave arc, a reflex joint, a start off the axis). The signed distance `f` to a convex solid is convex, and for any point `q` on its surface with outward normal `n` of a supporting plane, `f(x) >= n . (x - q)` for every `x`, with equality at the point whose nearest surface point is `q`. The checker computes `q` and `n` exactly from the profile's lines and arcs in the tool's half-plane.
2. **Every piece of geometry lies inside a known hull.** Each face is cut into rational Bézier patches with positive weights: a B-spline face per knot span, its control net for any sub-box found by blossoming (de Boor's algorithm with a different parameter at each level, which is knot insertion); a plane as its bilinear patch; a cylinder, cone, sphere or torus as the exact tensor product of rational quadratic circle arcs of at most 90 degrees with its meridian. The image of a parameter box lies in the convex hull of its projected control points. A tessellation triangle is its own hull. So over a piece with control points `Q_i`, `min f >= min_i n . (Q_i - q)` (taking `q`, `n` at the piece's centre point `c`), and also `>= f(c) - max_i |Q_i - c|` because `f` is 1-Lipschitz; the larger is used, less a slack of 2e-6 mm for rounding and for a validated profile's arcs (which may miss their stated radius by 1e-6 mm).
3. **Branch and bound.** Pieces are split in two along their longer side, smallest lower bound first, and a piece is dropped once its lower bound is within the tolerance of the smallest clearance found so far. When nothing is left, every dropped bound is at least `upper - tolerance`, so `upper - lower <= tolerance`. Pieces are found through a bounding-volume hierarchy built once per geometry (pieces no larger than a 0.5 mm ball). A pose that needs more pieces than its budget, or a piece too small to split, fails with the interval reached.
4. **Trims and boundaries.** A face is the part of its parameter box inside its trim loops, read as `openchuck-path` reads them: even-odd, and one period either side on a periodic surface. A box that no trim segment enters is wholly on or off the face; a box a segment enters is bounded as a whole (it holds the face's part) and split until the face points found in it close the interval: its centre when on the face, otherwise a point just beside a crossing segment that is tested to be on the face (a segment can enter a box without bounding the face there, for example through a hole). The boundary belongs to the face; segments that only run along a box's edge are covered by its neighbours.
5. **Stations, settings and evidence.** M2 checks every fed pose and the quarter, middle and three-quarter points of every fed move, placed on the machine's straight move in X, Y, Z and A (decisions 0025 item 3, 0115, 0133), against the exact faces and against the tessellation the tool was dropped on, at tolerance 0.0005 mm, margin 1 mm and a budget of 4,000,000 pieces a station. It reports the deepest cut and largest gap found and the deepest cut the bounds allow, the drop error at contact stations with its proven bound, and the widest interval; it exits with an error if any station fails or the widest interval is not under 0.001 mm. `--raw DIR` writes one tab-separated file per form and operation, with the source commit, inputs, form and tessellation settings, operation, tool and checker settings in its header and every station's tip, axis and both intervals.
6. **Independence.** The checker is `openchuck_testkit::contact`, in the test-only crate. It evaluates faces from their plain data (its own blossoms and closed forms) and the tool from its profile's pieces; it uses nothing from `openchuck-path` (no `ExactFace`, no drop-cutter, no fitter), so the strategies and their evaluators do not grade themselves and OI-839 cannot reach it.

## Verification

Red, then green: `crates/openchuck-testkit/tests/contact.rs` failed to compile against the missing module, then its tests passed. Each case has a closed-form answer computed in the test from the geometry alone, and each asserts that the interval holds it and is at most 0.0005 mm wide:

- decision 0211's flat-cutter plane, as a face (-0.01 mm, found at x = 0.99) and as its two triangles;
- the same plane as a rational bicubic B-spline with nonuniform knots, uneven pole spacing and unequal weights;
- a ball, and a tapered ball's tip, on a cylinder at five angles and five depths (gouge, touch and gap); a ball on a sphere, a torus and a cone;
- tilted tapered-ball (its tip and, at 88 degrees, its flank), ball and flat tools above a plane, each at the height of its lowest point;
- a rational quadratic cylinder in two uneven spans (0 to 30 and 30 to 90 degrees, knots 0, 0.2, 1);
- trims: the deepest point moved to a trim line (-0.005 mm), a hole whose outer loop runs along the box edge through it (0 mm; this case first failed, exposing a trim point taken off the face, and was fixed), and a periodic face trimmed across its seam;
- several faces, the nearest winning; determinism across threads (bit-identical intervals);
- failures: a budget too small, a tolerance that is zero, negative or not finite, a pose that is not finite, a concave tool, a non-positive weight and a box that is not finite;
- soundness against independent dense sampling: 12 random rational patches (cubic by quadratic, uneven knots, weights 0.5 to 2) with four flat-cutter poses each; the proven lower bound never exceeds the smallest of 160,801 samples evaluated by a separate Cox-de Boor evaluator with a closed-form flat-cutter distance, the found value is within the tolerance of it, and the found value equals the closed-form clearance at its own point.

Review of the released pull request (CodeRabbit, 29 Oct 2026) found that a trim segment lying wholly at the trims' top v was put in no bucket of the v index, so a box that straddled such an edge could be classified without it. A regression test with a trim whose top edge lies inside the parameter box first failed (the face was dropped: the clearance read as the margin), then passed once the bucket bounds were clamped. A diagnostic over the blisk and the corpus found that on every affected face but four the trims' top v equals the parameter box's top v, where an edge along a box boundary never enters a box and the test is unaffected; on the four blisk blade faces 7, 10, 19 and 22 the trim's top lies 4e-11 below the box's top, so at most a sliver 4e-11 wide in v (about a nanometre in space) could have been counted as face, far below the 2e-6 mm slack. The measured results below are therefore unchanged.

The harness tests (`crates/openchuck-app/tests/step_accuracy_bounded.rs`, no kernel) keep the counterexample as a regression: the old sampled check still reads it 0.0067 mm short, and M2 detects -0.01 mm within the tolerance at a pose and a quarter point and qualifies; a station it cannot bound fails M2, naming it; raw files carry their provenance. The kernel tests of `tests/step_accuracy.rs` now check the bounded checker on the imported corpus cylinder: gouge, touch and gap, the quarter stations on the machine's straight move, and its tessellation within the sagitta. All tests run in under 4 s each uninstrumented (`cargo nextest run`; the 18 checker tests 1.5 s in all, the longest the dense-sampling case at 1.4 s), so none is long-running. The checker's own tests cover 94% of `contact.rs`'s lines on a focused instrumented run (the testkit is outside the coverage gate, `xtask/coverage.toml`); the lines left are reasons in error messages for malformed data and defensive fallbacks. The harness changes are example code, outside the product report.

## Results

Measured on 28 Oct 2026 with the release harness, four threads, Intel Xeon @ 2.80 GHz, 4 cores; blisk at source `4f814fc`, corpus at `0560e4f` (the same checker; the later commit adds the drop-error bound column, which for the blisk is computed from its raw files). Inputs: `testdata/step/valid/blisk.step` (SHA-256 `0e472ff5…b646`, as in 0211), the owner's `blisk.stl` from the trial branch (`48351f42…3f37`, outside the repository) and the corpus parts. Full stepovers (no `--quick`), the operations, tools and stock of decision 0211, form (c) at 0.01 mm / 0.02 rad (the original candidate). Commands, hashes, complete output and derived counts are in [the evidence](0255-bounded-m2-evidence.txt). The complete raw observations (about 450 MB for the blisk) are not committed: the evidence lists their SHA-256 hashes, and both the paths (GC-07) and the checker (bit-identical across threads, tested) are deterministic, so the stated command at the stated source regenerates them. [The raw extract](0255-bounded-m2-raw-extract.tsv) keeps, for each blisk form and operation, the provenance header and the 40 deepest stations and the 40 largest-gap contact stations with both intervals.

**Every station of every successful operation was bounded on every part and form, and every interval was at most 0.0005 mm wide**: M2 qualifies its own measurement for the first time. The most pieces any station needed was 28,684 (the blisk's axial finishing, against the exact faces), against a budget of 4,000,000; the mean was 217 to 421 pieces a station. The blisk's 4,073,000 station checks across the three forms took 52.5 minutes at four threads.

The blisk, mm (found deepest cut; proven at most; found at the strategy's poses; largest contact gap; proven drop error at contact stations; stations):

| Form | Operation | Deepest cut | Proven at most | At poses | Gap | Drop bound | Stations |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| a | roughing (0.3) | 0 | 0 | 0 | 0.0593 | 0.0596 | 38,840 |
| a | rest | 0.0734 | 0.0739 | 0.0734 | 0.0624 | 0.0739 | 223,772 |
| a | pencil | 0.0244 | 0.0248 | 0.0244 | 0.0411 | 0.0423 | 12,100 |
| a | helical | 0.0734 | 0.0739 | 0.0734 | 0.0624 | 0.0739 | 223,772 |
| a | axial | 0.0783 | 0.0787 | 0.0783 | 0.0534 | 0.0793 | 517,388 |
| a | indexed | 0.0704 | 0.0709 | 0.0704 | 0.0600 | 0.0709 | 2,344 |
| b | roughing (0.3) | 0 | 0 | 0 | 0.0091 | 0.0268 | 38,868 |
| b | rest | 0.0426 | 0.0430 | 0.0426 | 0.0103 | 0.0431 | 246,096 |
| b | pencil | 0.0342 | 0.0347 | 0.0342 | 0.0065 | 0.0365 | 9,028 |
| b | helical | 0.0426 | 0.0430 | 0.0426 | 0.0103 | 0.0431 | 246,088 |
| b | axial | 0.0428 | 0.0432 | 0.0428 | 0.0095 | 0.0433 | 776,444 |
| b | indexed | 0.0318 | 0.0322 | 0.0318 | 0.0072 | 0.0323 | 2,328 |
| c | roughing (0.3) | 0 | 0 | 0 | 0.0050 | 0.0046 | 37,728 |
| c | rest | 0.0062 | 0.0066 | 0.0051 | 0.0052 | 0.0058 | 227,736 |
| c | pencil | 0.0009 | 0.0014 | 0.0009 | 0.0014 | 0.0018 | 9,092 |
| c | helical | 0.0062 | 0.0066 | 0.0051 | 0.0052 | 0.0058 | 227,728 |
| c | axial | 0.0059 | 0.0064 | 0.0054 | 0.0057 | 0.0058 | 756,352 |
| c | indexed | 0.0052 | 0.0057 | 0.0037 | 0.0028 | 0.0050 | 2,260 |

Simultaneous finishing refuses the blisk on all three forms, as decision 0211 documents (here 0.0203, 0.1368 and 0.3568 mm at the blades' roots); a refused operation has no stations.

What the blisk shows:

1. **The readings agree with decision 0211's sampled ones and are now proven.** The sampled tables were taken on earlier code, so paths differ a little, but every finishing row except pencil (which now follows the B-rep creases) is within 0.0005 mm of its historical full-stepover counterpart, and the found values are attained at real face points, so the sampled maxima did not overstate the cuts. On form (b) the finishing operations cut 0.032 to 0.043 mm into the exact faces, proven at most 0.0432 mm: well over the 0.01 mm tolerance, as 0211 found.
2. **The original finer-mesh candidate (c, 0.01 mm / 0.02 rad) does not meet the 0.005 mm allocation at the tool.** Its proven drop error reaches 0.0058 mm on rest, helical and axial finishing (the tessellation's own share of the cut, from both intervals), which agrees with uncapped M1's 0.00570 mm. Its deepest cut, 0.0062 mm found and 0.0066 mm proven, is within the whole 0.01 mm tolerance, but the allocation, not the total, is the criterion for the mesh (verification plan 4.1). Issue #347 qualifies a candidate against it.
3. **The quarter stations matter.** On (c) the deepest found at the strategy's poses is 0.0051 mm, and between them 0.0062 mm: the fitter's chord adds up to 0.0011 mm, within its 0.004 mm limit (decision 0018 item 12).

The corpus, forms (b) and (c) at full stepovers (complete tables in the evidence): every station of every successful operation was bounded with intervals of at most 0.0005 mm. Pencil clean-up refuses every corpus part (no inside corner of 30 degrees or more), as it should. The worst finishing cut found on (b), proven bound in brackets: cylinder 0.0050 (0.0055), cone 0.0091 (0.0095), sphere 0.0204 (0.0208), torus 0.0161 (0.0165, simultaneous finishing), `bspline_face` 0.0106 (0.0111), `filleted_box` 0.0065 (0.0069), `revolved_spline` 0.0208 (0.0212); on (c): cylinder 0.0030 (0.0035), cone 0.0027 (0.0032), sphere 0.0060 (0.0065), `bspline_face` 0.0084 (0.0089), `filleted_box` 0.0041 (0.0045), `revolved_spline` 0.0059 (0.0063). Form (c)'s proven drop error stays at or under 0.005 mm on the cylinder, cone, sphere, `filleted_box` and `revolved_spline`, but reaches 0.0078 mm on `bspline_face`. The torus could not be imported at 0.02 rad: the STEP worker stopped the read at its 120 s limit, so the torus is measured on form (b) only, and the original candidate's import cost is a further point for #347.

## What it does not establish

- **Trims are polylines.** The bound holds for each face as the B-rep data gives it: the exact surface inside UV trim polylines sampled at the import's tolerance. How far those lie from the kernel's exact trim curves is outside it (decision 0211 reads 0.04282 mm on the blisk's trimmed planes). OI-1020 tracks bounding that gap or checking the exact edge curves too.
- **The tool is its cutting profile.** The checked solid ends at the top of the flutes, as 0209's M2 defines it ("its tool outline only"); the shank and holder are not checked (OP-12's holder check is separate).
- **Clearance is capped at the margin (1 mm)** and the contact window that selects contact stations (within 0.002 mm of the stock to leave on the tessellation) is a classification of found values, as before.
- **The contact gap is a gap at contact stations**, not an exhaustive remaining-material measurement.
- **Convex profiles only.** A concave profile (a corner-rounding cutter) has no bound and is refused; none is used by the measured operations.
- **Double precision.** Rounding is covered by the 2e-6 mm slack; the arithmetic is not interval arithmetic.
- **Toolpath qualification is not decided here.** The bounded M2 reports what the forms cut; whether a finer toolpath mesh meets the 0.005 mm allocation and the time limits is issue #347's.

## Alternatives considered

- **A finer uniform grid.** Not a bound: decision 0211 shows a finer grid can miss more, and a uniform UV grid has no physical spacing on a nonuniform B-spline.
- **A Lipschitz bound alone** (each sample covers a ball of its spacing). Proven, but its error is first order in the piece size, so a 0.0005 mm interval needs pieces of about that size: where a flat end lies on a plane, millions of pieces a station. The convex-tool plane bound is exact on planes and second order on smooth faces.
- **The product's exact queries** (`ExactFace::closest`). Not independent of the strategies, and subject to OI-839.
- **Interval arithmetic on the surface equations.** Possible, but heavier, and the hull property of positive-weight rational patches gives the same guarantee with simpler code.

## Consequences

- **Code:** `crates/openchuck-testkit/src/contact.rs` (new) and its tests; `crates/openchuck-app/examples/step_accuracy/bounded.rs` (new; M2 with the checker, raw output), `sampled.rs` (the old cloud, kept only for the regression), `main.rs` and `measure.rs`; `tests/step_accuracy_bounded.rs` (new) and `tests/step_accuracy.rs`. No product crate, output, default, schema, dependency or library API changes, so the library version is unchanged.
- **Documents:** this record and its evidence; the decision index; the verification plan (a tool-contact row in 4.1, and OP-23's checker wording; no criterion reduced); decision 0211 points here; `docs/open-items.md` (OI-1020 new; OI-834 and OI-835 noted); the work breakdown's STEPALL row.
