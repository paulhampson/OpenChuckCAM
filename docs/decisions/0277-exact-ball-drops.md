# 0277: Exact ball drops on STEP parts for helical, indexed, rest and simultaneous finishing (lane STEPALL-X2; issue #349)

- **Status:** proposed, 29 Oct 2026 (Europe/London). Draft at stage 1 of the lane: the design and the first measurements. The full measurements (five runs at one and all threads, every in-scope operation, corpus and blisk, determinism and replay) are added before the pull request. Only the project owner accepts a record (decision 0049).
- **Date:** 29 Oct 2026 (Europe/London).
- **Lane:** STEPALL-X2, branch `claude/lane-stepall-x2-ball-drops`, from `main` at `d5d0a74` (library family 0.26.0). Issue #349 (STEPALL-0211 step 4 of 6), parent #257.
- **Requirements:** IMP-02, OP-03, OP-04, OP-05, OP-12, OP-21, OP-23, SET-09, GC-07, NFR-04, NFR-05, NFR-06, NFR-07, NFR-12. Verification plan 4.1 (the 0.005 mm tessellation share of the 0.01 mm toolpath tolerance).
- **Builds on:** [decision 0209](0209-step-geometry-for-every-strategy.md), [decision 0210](0210-step-geometry-follow-ups.md), [decision 0211](0211-step-accuracy-measured.md) (accepted: query foundations, then ball drops; the 2-times limits of OI-835), [decision 0255](0255-bounded-tool-contact-checker.md) (the bounded independent checker), [decision 0260](0260-exact-face-and-trim-queries.md) (accepted: the exact queries this drop builds on). Decision 0256 (proposed in PR #378: no fixed finer toolpath mesh qualifies) is guidance only here.
- **Open items:** progresses OI-81, OI-833, OI-835, OI-1041; numbers OI-1110 onwards are reserved for this lane and are added when raised.
- **Numbers:** record 0277 and OI-1110 to OI-1129 were assigned by the coordinator (0277 to 0281) and checked free on `main` and every unmerged branch on 29 Oct 2026. No project schema number: nothing stored changes.

## Context

Every drop-based strategy on a STEP part drops the tool onto the import's tessellation (OI-81), which lies up to 0.043 mm off the exact faces on the blisk (decision 0211, M1), so the zero-stock finishing operations cut 0.032 to 0.044 mm into the exact faces at the dropped poses (decision 0211, M2). Decision 0260 made the exact queries two to three orders of magnitude cheaper, but measured decision 0211's probe (21 unlimited distance queries a drop, no seed) at 12.6 ms a drop on the blisk against 54 µs for a tessellation drop. Issue #349 asks for an exact ball drop with a justified search and error bound (not the kernel's deflection), within 2 times the import-tessellation generation time for the whole job and for every operation.

## Decision (as built at stage 1)

1. **The drop is a branch and bound on the exact faces, not a search band about the tessellation's answer.** A ball of radius `r` moves along the tool line; a face point `p` is first held by the ball at the tip offset `lift(p) = z + sqrt(r^2 - rho^2)` (`z` along the axis from the ball's centre at offset 0, `rho` from the axis) and never when `rho > r`. The drop is the largest `lift` over the faces. The tessellation is not used: no bound is taken from it.
   - Every point the search evaluates is a point of a face, so its `lift` is a real contact and the best found is never above the answer.
   - Every part the search passes over has an upper bound on its `lift` no higher than the best found plus `DROP_TOLERANCE` (1e-4 mm), so the answer is never more than that above the best found. Where a search would stop at its budget (decision 0260's 4,096 pieces), the tip is raised to the bound it stopped at: never below the true contact, so the error is a gap, not a gouge, and it is reported.
2. **Faces.** Faces are visited highest bound first (the ball against each face's box); a face whose bound cannot beat the best is passed over.
   - **Boundary** (trim loops, or the parameter box of an untrimmed face): the segments are visited highest bound first, after a coarse pass over runs of 16 segments; each segment is searched by halving its parameter, each piece bounded by the ball and the capsule of decision 0260 (the ball's highest reach to a ball about the piece's middle image, and to a capsule about its chord of radius `bend w^2 / 8`).
   - **Inside an analytic face:** where the largest `lift` is at an inside point, the ball's centre lies on the face's normal at distance `r`, that is on an offset surface of the same kind (planes, cylinders, cones and spheres by a quadratic, the torus by a quartic, with the cone's apex as a point of its own); each root is kept when its foot is inside the trims.
   - **Inside a B-spline face:** the hull cells of decision 0260. The distance from a point to a cell's boxes (the axis-aligned box and the box along the cell's frame) is convex, so the highest offset at which the ball reaches them is found by Newton steps from above, each an upper bound. A cell's middle gives a contact; a few Newton steps of the contact condition from a middle that beats the best tighten it.
   - **The normal-cone test.** The face's largest `lift` is at a point where the ball rests tangentially (centre at `p + r n` or `p - r n` on the line) or on its boundary, which the boundary search has already settled. A cell whose normals lie in a cone of axis `m` and half angle `theta` (from the cross products of its net's differences, which hold `S_u x S_v` for a polynomial patch) cannot hold such a point when the line keeps further than `2 r sin(theta / 2)` from both boxes shifted by `+- r m`; it is passed over. Without it, cells along a trim edge near the contact, where the untrimmed surface continues higher, had to be split to the tolerance (up to 1,400 cells a drop on the blisk). A rational patch whose weights differ by more than rounding gets no cone and is searched without the test (still correct, slower). This is the critical-point test decision 0260 left open; it also settles the focal-point case of OI-1041 for drops.
3. **Profiles.** Only a tool whose lower envelope is a ball centred on its axis one radius above its tip (a ball end, or a ball end grown for stock to leave) has an exact drop. Any other profile, or a face with no exact search, falls back to the tessellation drop.
4. **Kernel isolation.** Everything is in `openchuck-path` (`brep.rs`, new `brep/drop.rs`, additions to `brep/hull.rs` and `brep/trim.rs`) on the plain B-rep data; no kernel type or dependency.

## Measurements at stage 1

Machine: Intel Xeon @ 2.10 GHz, 4 cores, 15 GB, cloud container, release build. The blisk placed as decision 0211 places it (X 10 to 60), 2 mm ball.

**One drop, one thread** (a stage-1 benchmark: radial tool lines as helical finishing uses, X every 0.5 mm, A every 6 degrees, 6,900 lines, 6,240 touching):

| | Mean | Median | p90 | p99 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Tessellation drop (today) | 24.2 µs | 23.8 µs | 38.4 µs | 63.3 µs | 426 µs |
| Exact drop, first prototype (no normal-cone test) | 87.1 µs | 16.0 µs | 107 µs | 1,151 µs | 1,721 µs |
| Exact drop, with the normal-cone test | 20.9 µs | 13.0 µs | 44.4 µs | 107.9 µs | 511 µs |

Cells and pieces looked at per drop: mean 15, p99 43, max 60 (before the normal-cone test: mean 99, p99 1,341). No drop reached a budget (none raised). The exact answers differ from the tessellation's by up to 0.071 mm (median 0.0036 mm, p99 0.032 mm); the largest, checked by brute force (a 600 x 600 grid on every face), is the exact answer: the tessellation stands 0.07 mm proud of a blade where the line grazes it.

**Operations** (decision 0211's `step_accuracy` harness, form b, full stepovers, all threads, one run each; not yet the five-run medians #349 requires):

| Operation | Tessellation drop: generate | Exact drop: generate | Ratio |
| --- | ---: | ---: | ---: |
| Roughing (flat tool, unchanged) | 0.959 s | 0.963 s | 1.00 |
| Rest machining | 4.491 s | 5.346 s | 1.19 |
| Helical finishing | 4.232 s | 5.116 s | 1.21 |
| Indexed finishing | 0.054 s | 0.060 s | 1.12 |

**Accuracy** (the bounded checker of decision 0255, quick stepovers, all threads):

| Operation | Drop | Deepest cut | Proven at most | At the poses | Largest gap |
| --- | --- | ---: | ---: | ---: | ---: |
| Helical | tessellation | 0.0422 | 0.0426 | 0.0422 | 0.0087 |
| | exact | 0.0040 | 0.0045 | 0.0001 | 0.0040 |
| Indexed | tessellation | 0.0318 | 0.0322 | 0.0317 | 0.0086 |
| | exact | 0.0039 | 0.0044 | 0.0001 | 0.0036 |

With exact drops the poses are within the checker's resolution; what remains is the path fitter's chord between them (decision 0211 M2 item 3: its quarter points add up to the fitter's 0.004 mm chord limit).

## Still to do in this lane

Wire the drop into the four strategies by an explicit choice (a strategy keeps the tessellated drop unless it meets both 2-times limits), switch region, avoid and check face sets to exact faces with the region test accepting the drop's own accuracy (OI-833), the full five-run measurements at one and all threads, the corpus, simultaneous finishing (its documented blisk refusal kept), determinism at 1, 2 and all threads, application and library equivalence, replay for both posts, notes, version and changelog.

## Alternatives considered

- **A search band about the tessellation's drop** (decision 0211's probe, refined with `project_within` and `project_from`). Its band needs a bound on the tessellation's error, which the kernel's deflection is not (decision 0211 M1), and its cost is in the many whole-part distance queries.
- **A finer fixed tessellation** (decision 0256, proposed): no setting qualifies; the exact drop is cheaper than the import tessellation's drop here.
- **A smaller cell budget** (OI-1041): not needed for drops; the normal-cone test keeps the cells to tens.
