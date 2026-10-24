# 0211: What the exact STEP surface gains, measured on the blisk: the measurement of decision 0209 item 4.1, and which strategies the exact drop should cover (lane STEPALL-M; issue #257)

- **Status:** accepted by the project owner on 24 Oct 2026 (Europe/London), directly in this lane STEPALL-M session: "Accept 0211", after the full revised record was shown and the substantive choices were selected individually. Proposed 23 Oct 2026 and revised 24 Oct 2026. The owner chose a fixed finer toolpath mesh as a direction to pursue, query foundations before exact drops, an additional per-operation 2-times limit, correction of the posted output while preserving the replay limit, and completion of all existing verification requirements. Simultaneous refusal stays documented without separate tracking; pencil performance is remeasured first. Acceptance is recorded on the owner's direct instruction (decision 0049). Acceptance by itself changed no product output, default or schema; subsequent owner-authorised implementation is documented below. It grants no methodology waiver and does not release PR 321 or start another lane.
- **Date:** 22 to 24 Oct 2026 (Europe/London).
- **Requirements:** IMP-02, IMP-06, OP-03 to OP-06, OP-09, OP-12, OP-23, NFR-04, NFR-05, NFR-06, NFR-12, GC-07, GC-09. Verification plan 4.1 (the 0.005 mm tessellation share of the 0.01 mm toolpath tolerance).
- **Builds on:** [decision 0209](0209-step-geometry-for-every-strategy.md) item 4.1 (accepted), which asks for this measurement and says the numbers go into its *Measurements* "or 0210"; the coordinator assigned this lane record 0211, so they are here and 0209's section points to this record.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-839 is new (item 1). OI-832 was closed by decision 0209 and the file is copied here. Progresses OI-65, OI-76, OI-81, OI-831, OI-834 and OI-835 without closing them.

Numbering: the coordinator assigned this lane record 0211 and OI-839. On 22 Oct 2026 the highest record on `main` and every unmerged branch was 0209 (`claude/docs-stepall-proposal`), and the highest `OI-` number was OI-836 on the same branch, so both were free. Checked again on 23 Oct 2026 after merging `main`: other lanes hold records 0210 and 0212 to 0219 and OI-837, OI-838 and OI-840 to OI-891, and neither 0211 nor OI-839 is used on `main` or any other branch. No project schema number: nothing stored changes.

## Context

Decision 0209 found that every drop-based strategy on a STEP part drops the tool on the import's tessellation (OI-81), and asked that the gain of an exact drop be measured before it is built, strategy by strategy, against the cheap alternative of a finer tessellation. This lane built the measurement and ran it.

## The harness

`crates/openchuck-app/examples/step_accuracy/` (`main.rs`, `measure.rs`), behind the `kernel-tests` feature, as `path_runs` is a measuring example. It changes nothing in the product. Run it with the STEP worker built:

```text
cargo build --release -p openchuck-import --features kernel --bins
OPENCHUCK_STEP_WORKER=target/release/openchuck-step-worker \
cargo run --release -p openchuck-app --features kernel-tests --example step_accuracy -- \
    [--only m1,m2,m3,m4] [--parts ...] [--deflections ...] [--angular RAD] \
    [--part NAME] [--forms a,b,c] [--fine MM] [--ops LIST] [--threads 1,all] [--runs N] \
    [--stl OWNER_BLISK_STL] [--sim-tilt DEG] [--sim-step DEG] [--keep DIR] [--quick]
```

- **Part.** `testdata/step/valid/blisk.step`, copied from the owner's trial file (OI-832; same SHA-256), placed on the rotary axis from X 10 to 60 as the owner's projects place `blisk.stl`. The owner's `blisk.stl` (form a) is read from a path given on the command line (`docs/trial-feedback-projects/blisk.stl` on branch `claude/upbeat-davinci-jk46yd`): it is not in `testdata/`, since OI-832 covered `blisk.step` only. It sits on the STEP part after the owner's own +10 mm X translation; its vertices lie up to 0.031 mm from the exact faces.
- **Operations.** The owner's operations and tools from the trial projects, in this order, each on the stock the ones before it left (SIM-06), Makera Z1, softwood, a 42 mm cylinder of stock 1 mm longer than the part at each end, no workholding: rotary roughing (6 mm flat, stepover 2, stepdown 1, stock to leave 0.3), rest machining (2 mm ball, stepover 0.3), pencil clean-up (2 mm flat), helical finishing (2 mm ball, stepover 0.3, 2 degree step), axial finishing (3.175 mm tapered ball, 0.25 mm tip, 5 degrees, stepover 0.12), indexed finishing (2 mm ball, A 0, 90, 180, 270) and simultaneous finishing (2 mm ball, stepover 0.3). Indexed and simultaneous finishing are not in the owner's projects and take the helical finish's tool. The parameters go through the application's `parameter_values`, so the cutting data fill is the application's.
- **Forms.** (a) the owner's `blisk.stl`; (b) the STEP part as built today, dropping on the import's tessellation (0.01 mm, 0.1 rad); (c) the STEP part dropping on a finer tessellation (item 1 chooses it: 0.01 mm at 0.02 rad). Form (d), the exact drop, is lane STEPALL-X's.
- **Independence.** Nothing measured uses the drop-cutter or the path fitter. M1 measures the tessellation against each face in closed form (planes, cylinders, cones, spheres, tori, from the face's plain data alone) and, for B-spline faces, by `ParametricFace::closest` against the face without its trims; the trimmed distance is reported beside it. M2 judges each tool position with the independent checker's outline (`openchuck_testkit::gouge::Outline`) against points on the exact faces 0.05 mm apart (5.3 million on the blisk), inside their trims and along their trim loops, and against points on the tessellation the tool was dropped on. A 2 mm ball between points 0.05 mm apart misses at most 0.0003 mm; the tapered ball's 0.25 mm tip at most 0.0013 mm.
- **Stations.** Every fed pose, and the quarter, middle and three-quarter points of every fed move, placed on the machine's straight move in X, Y, Z and A (decisions 0025 item 3, 0115 and 0133).
- **Checked first, historically.** `crates/openchuck-app/tests/step_accuracy.rs` (kernel; four tests, 2.9 s in the historical `cargo nextest` run, in the STEP kernel job; refreshed timings below) checks the functions on closed forms: the measured error of the corpus cylinder equals its sagitta from the vertices alone at 0.01 and 0.0025 mm; a ball's gouge and gap read within 0.0002 mm; a move's stations lie on the machine's straight move; a mesh is sampled by its area.
- **Historical machine.** Intel Xeon @ 2.80 GHz, 4 cores, 15 GB, release build, cloud container. The refreshed run below records its own machine and allocation.
- **Code measured.** The historical M2 to M4 tables are on `57772e7` (merged into this branch on 23 Oct 2026); M1 does not depend on it. STEPALL-P, STEPALL-E, SHORTMOVE and OPSPEED merged after that. The owner authorised merging main while keeping this PR draft, relayed by the coordinator on 23 Oct 2026 as an exception to decision 0055's ready-first sequence. The refreshed section below measures source `827fc45` with main `a2605495` merged, at quick stepovers; it does not replace the historical full-stepover tables or supply refreshed M1/M3 timings.

## Limits of this historical report

The tables below retain the measurements recorded by the original lane on `57772e7`; they have not been reproduced at full stepovers in the takeover environment. They inform the owner's choices recorded below, but do not establish current performance or completion of decision 0209's verification. The owner accepted record 0211 separately on 24 Oct 2026. Neither that acceptance nor merging the report releases STEPALL-T/X, approves a new default, closes OI-831/OI-834/OI-835, or waives the verification plan.

- **M1 samples, rather than bounds, the deviation.** It strides faces over 20,000 triangles; B-spline distances use the library evaluator. The known closest-point defect OI-839 remains in STEPALL-X's scope. A sampled maximum is not a certified error bound for an exact-drop search band. No tested setting meets the 0.005 mm share, so form (c) is an approximation outside that share, not the within-share alternative contemplated by decision 0209 item 4.3.
- **M2's sampling limit is not established.** The stated estimate for the tapered tip is 0.0013 mm, above decision 0209's requirement of less than 0.001 mm. Moreover, the face grid is uniform in UV using lengths estimated on a coarse grid; it does not bound physical sample spacing on a nonuniform B-spline. The closed-form cylinder tests do not establish that bound on the blisk's blades. Reported cuts and gaps apply to sampled points and stations; the gap is restricted to positions that touch the tessellation within the harness's 0.002 mm contact window, not an exhaustive remaining-material check.
- **M3 deviates from the approved timing method.** It uses three runs at all threads and one at one thread, rather than five at each, and reports total generation, summed drop time and simulation rather than separate fitting and assembly times. Its 1.24-times generation and 1.10-times generation-plus-simulation totals include simultaneous finishing's long unsuccessful attempt. They are not times for a completed seven-operation job. Summing only the six successful rows gives 38.24 s for (b) and 67.90 s for (c), about 1.78 times; this is arithmetic on rounded historical rows, not a new measurement or a median of complete jobs. The timings do not establish compliance with OI-835 on current `main`.
- **Historical M4 had a narrower scope than decision 0209 requests.** It compares library paths on (a), (b) and (c), but posted G-code only on (a) and (b). It uses quick stepovers and excludes refused operations from the application runs. This does not prove determinism at the owner's full stepovers or posted G-code determinism for form (c). Historically, differences and failed gates were printed without failing the process. The harness now rejects them through shared checks with three requirement-based tests; the native smoke run exercises the integrated checks, and the complete refreshed run is reported separately. Form (a)'s replay gates are now checked too; no historical clean-gate result is claimed for that form.
- **Current-main integration.** The harness uses STEPALL-P's `PencilFinishing::on_brep` dispatch, rebuilding corners inside each operation's generation timing, and includes the operation material field required by current main. Source checkpoint `827fc45dd8736265d8504ab71bc8c5057ce9c664` completes `posting.rs` with the production Makera Z1 post and compares posted bytes at every M4 thread count for each measured form. It also requires form-b library/application equality for the path and posted text. The pinned kernel and full harness compile; all ten native measurement, rejection and posting tests pass. The three posting tests were first rerun against the stub and all failed, then passed after implementation. The full current-source quick run completed M2 and library M4 for all forms. Form-b application posting/replay checks passed at 1, 2 and 4 threads, and its IR matched the library at four threads. It then stopped with exit status 1 on form-a replay gates at one thread; the production CLI and separate two/four-thread application diagnostics reproduce the same gate-2 failure, with byte-identical programs at all counts. The smaller roughing/pencil form-b smoke check passed earlier too. No current pencil performance claim follows from historical rows. An updated five-run M3 and resolution of the other methodology deviations remain outstanding; the owner chose to retain every verification requirement on 24 Oct 2026.

The original trial STL is recoverable from `claude/upbeat-davinci-jk46yd:docs/trial-feedback-projects/blisk.stl` (1,195,782 bytes, SHA-256 `48351f42635f05eea09aaf77dd685e7b2bd564cd087b4a336fb29554a7e33f37`). The similarly named corpus STL is a different fixture. The committed STEP was rechecked against the trial blob: SHA-256 `0e472ff5374c6bc40825a4f7e8885d69f6ac99e53fbb6dbc412104fadb55b646` matches. No STL is added to the corpus by this lane.

## Measurements (historical)

### M1: the tessellation against the exact faces

The largest distance, mm, from the tessellation (corners, edge middles and a grid of four steps on each triangle) to the exact face, by kind; closed form for the analytic kinds. Faces of more than 20,000 triangles are measured on a fixed stride of them (the harness prints how many).

**The blisk** (25 faces), at the default angular deflection of 0.1 rad:

| Linear deflection | Triangles | Blades (10 B-spline) | Cones (5) | Cylinders (3) | Planes (7) | Facet normals, worst | Import |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0.01 (the import's) | 15,936 | 0.0095 | **0.0429** (p99 0.0247) | 0.0069 | 0 | 7.5 deg (cones) | 0.23 s |
| 0.005 | 30,522 | 0.0048 | 0.0218 | 0.0041 | 0 | 4.5 deg | 0.43 s |
| 0.0025 | 55,330 | 0.0023 | 0.0126 | 0.0110 | 0 | 4.3 deg | 0.93 s |
| 0.001 | 157,462 | 0.0010 | 0.0071 | 0.0007 | 0 | 4.2 deg | 3.2 s |

At 0.02 rad:

| Linear deflection | Triangles | Blades | Cones | Cylinders | Planes | Facet normals, worst | Import |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0.01 | 106,928 | 0.0057 | 0.0055 | 0.0007 | 0 | 4.3 deg | 2.3 s |
| 0.005 | 84,798 | 0.0057 | 0.0053 | 0.0008 | 0 | 4.3 deg | 1.8 s |

**The corpus**, worst face, at 0.1 rad:

| Part | 0.01 | 0.005 | 0.0025 | 0.001 |
| --- | --- | --- | --- | --- |
| `cylinder` | 0.0031 | 0.0025 | 0.0012 | 0.0005 |
| `cone` | 0.0091 | 0.0046 | 0.0023 | 0.0009 |
| `sphere` | **0.0208** | 0.0037 | 0.0052 | 0.0015 |
| `torus` | 0.0124 | 0.0099 | 0.0050 | 0.0020 |
| `bspline_face` | 0.0098 | 0.0050 | 0.0025 | 0.0010 |
| `filleted_box` | 0.0030 | 0.0030 | 0.0021 | 0.0007 |
| `revolved_spline` | **0.0211** | 0.0105 | 0.0052 | 0.0009 |

What M1 shows:

1. **The kernel's deflection is not a bound** (decision 0003, OI-65). On the blisk's cones the import's tessellation lies 0.043 mm from the exact surface, four times its 0.01 mm setting and 8.6 times the 0.005 mm share; the corpus sphere and the revolved spline lie twice their setting from theirs. Finer settings do not shrink the error in step: the blisk's cylinders are worse at 0.0025 mm than at 0.005 mm.
2. **The angular deflection is what holds the cones back.** At 0.1 rad the cones need 0.001 mm and ten times the triangles and still lie 0.0071 mm off; at 0.02 rad they lie 0.0055 mm off at 0.01 mm, with 107,000 triangles. Below that the kernel stops improving: 0.005 mm at 0.02 rad gives fewer blade triangles than 0.01 mm and the same 0.0057 mm.
3. **No setting tried meets the 0.005 mm share on every face of the blisk.** The closest are 0.01 mm at 0.02 rad (0.0057 mm, 107,000 triangles) and 0.001 mm at 0.1 rad (0.0071 mm, 157,000). Form (c) uses the first.
4. **Facet normals** are off by up to 7.5 degrees on the blisk's cones and 7 degrees on the corpus fillets at the import's setting, and up to 4.3 degrees on the blisk's blades, which does not fall as the deflection is made finer (2.6 degrees at 0.01 mm, 4.2 at 0.001 mm; OI-76).
5. **Planes read 0.04 mm against their trimmed faces** while lying in their planes: a point of a rim triangle can fall outside the face's trim polyline, which is coarser there than the tessellation. Anything that judges a contact by `contains` near a face's edge inherits that (the region test of OI-833).
6. **A defect in `ExactFace::closest`** (OI-839, new). On the corpus cone, 13 of the 1,112 vertices of its large rim are answered with a point on the seam, up to 4.2 mm away (1.5 mm with the trims removed), where the closed-form distance is under 0.01 mm. The projection of a point on the face's own boundary lands just outside the parameter box, and the boundary search then picks the wrong edge. `BrepSurface::project` and `distance` use it: simultaneous finishing's drive points today, and the exact drop of lane STEPALL-X. It is in `openchuck-path` `brep.rs`, X's file, and is left to that lane.
7. **What one exact query costs.** The mean time of one `ExactFace::closest` on the trimmed face, one thread, at 0.01 mm: on the blisk, 1,762 µs on a blade (B-spline), 703 µs on a plane, 675 µs on a cylinder and 90 µs on a cone; on the corpus, 519 µs on `bspline_face`, about 210 µs on a plane, 5.9 µs on `cylinder`'s side and 1.5 µs on `cone`'s. The trims dominate the analytic faces: the same kind of face costs a hundred times more where its trim loops are long (the blisk's planes and cylinders carry the blades' outlines). A `BrepSurface::distance` visits every face whose box is nearer than the best answer so far, and on the blisk that is several blades, so one query costs 50 to 60 ms (M3).

### M2: each strategy's cut against the exact faces

The deepest any exact-face point lies inside the tool at any station ("deepest cut"; "at ends" counts the dropped poses only), the largest clearance above the exact faces beyond the stock to leave at the poses that touch the tessellation ("largest gap"), and, at those poses, how far the exact faces lie inside and outside the tessellation the tool touched ("drop error"), mm. Full stepovers, all threads.

| Operation | Stock | Form | Stations | Deepest cut | At ends | Largest gap | Drop error in | out |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Roughing | 0.3 | a | 38,840 | 0 | 0 | 0.0593 | 0.0136 | 0.0593 |
| | | b | 38,868 | 0 | 0 | 0.0091 | 0.0264 | 0.0072 |
| | | c | 37,728 | 0 | 0 | 0.0050 | 0.0040 | 0.0042 |
| Rest | 0 | a | 223,772 | 0.0733 | 0.0733 | 0.0625 | 0.0734 | 0.0610 |
| | | b | 246,096 | 0.0425 | 0.0425 | 0.0101 | 0.0427 | 0.0092 |
| | | c | 227,736 | 0.0060 | 0.0049 | 0.0052 | 0.0052 | 0.0055 |
| Pencil | 0 | a | 12,100 | 0.0244 | 0.0244 | 0.0415 | 0.0256 | 0.0403 |
| | | b | 6,292 | 0.0349 | 0.0349 | 0.0073 | 0.0351 | 0.0067 |
| | | c | 8,384 | 0.0029 | 0.0006 | 0.0056 | 0.0023 | 0.0042 |
| Helical | 0 | a | 223,772 | 0.0733 | 0.0733 | 0.0625 | 0.0734 | 0.0610 |
| | | b | 246,088 | 0.0425 | 0.0425 | 0.0101 | 0.0427 | 0.0092 |
| | | c | 227,728 | 0.0060 | 0.0049 | 0.0052 | 0.0052 | 0.0055 |
| Axial | 0 | a | 517,388 | 0.0779 | 0.0779 | 0.0537 | 0.0787 | 0.0536 |
| | | b | 776,444 | 0.0428 | 0.0428 | 0.0103 | 0.0440 | 0.0101 |
| | | c | 756,352 | 0.0059 | 0.0052 | 0.0067 | 0.0069 | 0.0051 |
| Indexed | 0 | a | 2,344 | 0.0702 | 0.0702 | 0.0600 | 0.0704 | 0.0602 |
| | | b | 2,328 | 0.0316 | 0.0316 | 0.0073 | 0.0319 | 0.0074 |
| | | c | 2,260 | 0.0051 | 0.0035 | 0.0031 | 0.0038 | 0.0044 |
| Simultaneous | 0 | a, b, c | | refused by the strategy | | | | |

Simultaneous finishing refuses the blisk on every form, at its 10 degree tilt and 2 degree step and at every setting tried (tilt 0 and 5 degrees, steps of 1 and 2 degrees): "cannot keep the move to X 20.0 to 20.8, A ... within the tolerance: it would cut 0.018 to 0.357 mm into the surface; reduce the largest tilt or the largest angle step", always at the blades' roots (X 20). It refuses the owner's STL the same way, so the refusal is the strategy's on this part, not the STEP part's (NFR-07: a clear error, not a gouge). It was measured instead on the two corpus parts it finishes in decision 0056's kernel tests, with helical finishing beside it (same tool and stepover, quick-run options otherwise as above):

| Part | Operation | Form | Stations | Deepest cut | Largest gap |
| --- | --- | --- | --- | --- | --- |
| `revolved_spline` | helical | b | 96,064 | 0.0196 | 0.0097 |
| | | c | 96,012 | 0.0030 | 0.0009 |
| | simultaneous | b | 96,084 | 0.0189 | 0.0097 |
| | | c | 96,004 | 0.0030 | 0.0051 |
| `cone` | helical | b | 80,152 | 0.0090 | 0.0240 |
| | | c | 76,360 | 0.0006 | 0.0248 |
| | simultaneous | b | 75,792 | 0.0090 | 0.0009 |
| | | c | 72,004 | 0.0006 | 0.0013 |

Simultaneous finishing gains what helical finishing gains: the deepest cut falls by the tessellation's error (0.019 to 0.003 mm on the revolved spline, whose import tessellation lies 0.021 mm off; 0.009 to 0.0006 mm on the cone). The cone's 0.024 mm helical gap is the same on both forms, so it is not the tessellation's; it is left for the strategy's own tests.

What M2 shows:

1. **On the historical STEP part (b), the successful zero-stock finishing operations cut 0.032 to 0.044 mm into the sampled exact faces**, three to four times the whole 0.01 mm tolerance, at the dropped poses themselves (the "at ends" column), not inside the fitted moves. The drop error says why: the exact faces lie up to 0.043 mm inside the tessellation the tool touched, the cones' error of M1. The fitter's quarter points add nothing beyond the drops (b's deepest cut equals its deepest at the ends).
2. **On the owner's STL (a), helical, axial, indexed and rest cut 0.070 to 0.079 mm in and leave contact gaps of 0.054 to 0.063 mm**: the mesh path is worse, because the STL itself lies up to 0.031 mm off the part and its facets are larger.
3. **On the finer tessellation (c) the deepest sampled cut is 0.006 mm and the largest sampled contact gap 0.0067 mm** over the successful finishing operations. These readings are below 0.01 mm, subject to the sampling limits above; M1 still exceeds the 0.005 mm tessellation share at 0.0057 mm. At the dropped poses the deepest is 0.0052 mm; the fitter's quarter points add up to 0.0011 mm (helical and rest), within its 0.004 mm chord limit.
4. **Roughing gains nothing that matters.** It never cuts into the part in any form; what it leaves beyond its 0.3 mm is 0.009 mm on (b) and 0.005 mm on (c), under 3 % of the stock it leaves for the finishing cut, as decision 0209 item 2 expected.
5. **Rest machining reads as helical finishing** on this project: roughing leaves 0.3 mm everywhere, above rest machining's 0.15 mm threshold, so its passes cover the part and its path is helical finishing's but for the first few moves. It shares `wrap_fit` with helical and axial, so it gains what they gain.
6. **Pencil clean-up** follows creases found in the tessellation; on (b) its drops cut 0.035 mm in, on (c) 0.003 mm. Its crease source (decision 0209 item 2, T) is lane STEPALL-P's.

### M3: time

Release build, 4 cores (the machine above). Decision 0209 asks for the median of 5 runs; the cloud container restarted several times during the long runs and a background job is limited to two hours, so the medians here are of 3 runs at all threads and of one run at 1 thread, each run printed as it ended. Seconds per operation, in machining order on the stock the ones before it left. "Drops" is the time inside `drop_tool` summed over the threads (so it can exceed the wall time); "simulate" is the stock simulation of the operation's moves at the project's cell (0.1 mm).

**All threads (4), median of 3 runs**, on `main` as merged on 23 Oct 2026 (after lane BUG-TABDEV, #252). The first runs, on the code before that merge, gave the same paths (M2's figures were identical to the station) but axial finishing took 975 s and pencil 29 s on (b); they are not used.

| Operation | (a) STL: generate | drops | simulate | (b) STEP today: generate | drops | simulate | (c) finer: generate | drops | simulate | (c) over (b) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Roughing | 1.43 | 1.6 | 7.4 | 1.23 | 0.63 | 7.4 | 1.49 | 1.7 | 7.3 | 1.21 |
| Rest | 10.02 | 37.4 | 21.7 | 5.32 | 18.8 | 19.5 | 10.27 | 38.1 | 20.8 | 1.93 |
| Pencil | 12.27 | 37.2 | 0.25 | 2.48 | 3.9 | 0.10 | 8.95 | 19.5 | 0.10 | **3.61** |
| Helical | 9.27 | 36.3 | 14.4 | 5.16 | 19.8 | 13.1 | 9.08 | 35.5 | 13.4 | 1.76 |
| Axial | 20.00 | 76.0 | 129.0 | 24.00 | 91.2 | 335.3 | 38.00 | 139.4 | 348.1 | 1.58 |
| Indexed | 0.12 | 0.35 | 0.07 | 0.05 | 0.14 | 0.07 | 0.11 | 0.33 | 0.07 | **2.2** |
| Simultaneous (until it refuses) | 107.66 | 390.4 | – | 112.84 | 5.3 | – | 119.09 | 10.3 | – | 1.06 |
| Attempted sequence, including refused simultaneous (generation) | 160.8 | | | 151.1 | | | 187.0 | | | 1.24 |
| Attempted sequence, including refused simultaneous (generation and simulation) | 333.6 | | | 526.6 | | | 576.7 | | | 1.10 |

Drop counts per run: helical 234,045 (a), 256,205 (b), 237,977 (c); axial 556,420, 828,608 and 810,532; pencil 410,107, 222,948 and 342,624. One tessellation drop costs about 70 µs on (b)'s 15,936 triangles and 110 to 160 µs on (c)'s 106,928 (the probe's figures below, on one thread).

**One thread, one run each**, on the same merged `main`, and the speed-up of the generation from 1 thread to 4 (NFR-04; decision 0209 item 4.5 asks for it per form):

| Operation | (a): generate | simulate | speed-up | (b): generate | simulate | speed-up | (c): generate | simulate | speed-up |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Roughing | 2.29 | 24.8 | 1.6 | 1.60 | 22.9 | 1.3 | 2.57 | 22.9 | 1.7 |
| Rest | 19.91 | 62.4 | 2.0 | 9.96 | 65.7 | 1.9 | 23.19 | 66.0 | 2.3 |
| Pencil | 10.47 | 0.51 | **0.85** | 2.46 | 0.21 | **0.99** | 8.85 | 0.25 | **0.99** |
| Helical | 19.37 | 41.3 | 2.1 | 9.91 | 43.3 | 1.9 | 21.96 | 42.7 | 2.4 |
| Axial | 26.68 | 491.1 | 1.3 | 35.32 | 1,150.9 | 1.5 | 61.01 | 1,235.9 | 1.6 |
| Indexed | 0.21 | 0.24 | 1.8 | 0.10 | 0.24 | 2.0 | 0.23 | 0.25 | 2.1 |
| Simultaneous (until it refuses) | 232.18 | – | 2.2 | 402.80 | – | 3.6 | 448.85 | – | 3.8 |

Pencil clean-up gains nothing from 4 threads on any form (0.85 to 0.99 times); the rest gain 1.3 to 3.8 times. The simulations scale (axial's 1,151 s on one thread is 335 s on four on (b)). With four threads busy each drop costs about twice what it does alone (helical on (b): 9.6 s of drops on one thread, 19.8 s summed on four), so per-drop costs are compared on one thread.

**What an exact ball drop would cost with today's exact queries.** For each ball-end operation, a fixed stride of the drop lines (up to 200 of those kept by their coordinates, so the same at any thread count) is refined against the exact faces by bisection on `BrepSurface::distance` from the ball's centre, in a band of 0.05 mm about the tessellation's answer, 20 halvings, one thread. This is a measuring probe, not lane STEPALL-X's design: it ignores the flutes above the ball, so where the flutes touch first it reads the exact contact too low.

| Operation | Form | Refined (of sampled) | Tessellation drop | One `distance` | One exact ball drop | Seed offset p50 | p99 | Seed too deep, max | Seed too high, max |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Rest | b | 183 (187) | 77 µs | 59 ms | 1.29 s | 0.0042 | 0.0295 | 0.0431 | 0.0332 |
| Helical | b | 183 (187) | 74 µs | 60 ms | 1.31 s | 0.0042 | 0.0295 | 0.0431 | 0.0332 |
| Indexed | b | 25 (25) | 32 µs | 58 ms | 1.28 s | 0.0032 | 0.0231 | 0.0231 | 0.0188 |
| Simultaneous | b | 189 (191) | 40 µs | 51 ms | 1.10 s | 0.0034 | 0.0226 | 0.0233 | 0.0215 |
| Rest | c | 187 (187) | 160 µs | 60 ms | 1.33 s | 0.0005 | 0.0147 | 0.0247 | 0.0092 |
| Helical | c | 187 (187) | 117 µs | 60 ms | 1.33 s | 0.0005 | 0.0147 | 0.0247 | 0.0092 |
| Indexed | c | 23 (23) | 83 µs | 59 ms | 1.29 s | 0.0003 | 0.0092 | 0.0092 | 0.0023 |
| Simultaneous | c | 169 (169) | 76 µs | 50 ms | 1.11 s | 0.0002 | 0.0081 | 0.0270 | 0.0036 |

**Through the application.** Lane BUG-TABDEV's `operation_times` example (`crates/openchuck-app/examples/operation_times.rs`, issue #252), run on the harness's form (b) project (`--keep DIR`, all threads), times each operation by "generate selected" up to it: its own generation plus the stock simulation of the one before it, which the application needs to carry the stock. Roughing 1.50 s, rest 11.87 s, pencil 22.50 s, helical 3.92 s, axial 27.30 s, indexed 337.33 s (of which axial's simulation, 335 s in the table above), and simultaneous finishing refused as above. They agree with the harness's figures summed the same way (rest 7.4 + 5.3, pencil 19.5 + 2.5, indexed 335 + 0.05); helical and axial read lower through the application (3.9 and 27.3 s against 0.1 + 5.2 and 13.1 + 24.0 s), within the spread of single runs on this machine.

The probe was run on the code before the merge of 23 Oct 2026 for (b) and (c), and again on the merged `main` for (b): the same within 6 % (one `distance` 53 to 64 ms, one exact drop 1.17 to 1.39 s, the same offsets), as the merge did not touch the exact faces (`brep.rs`) or the paths.

What M3 shows:

1. **An exact drop built on today's exact queries is not usable on the blisk.** One exact ball drop costs 1.1 to 1.3 s, 17,000 to 28,000 times a tessellation drop; helical finishing's 256,000 drops would take about 90 hours of one core. Decision 0209 item 4.5 proposes at most 2 times the tessellation job (OI-835). The cost is the exact query itself: one `BrepSurface::distance` takes 50 to 60 ms, because it visits several blades and one `closest` on a blade costs 1.8 ms and one on a long-trimmed plane or cylinder 0.7 ms (M1 item 7). These timings support the recommendation to improve closest-point queries, trim tests and face selection before the exact drop. The owner selected query foundations first and a fixed finer mesh as an interim direction on 24 Oct 2026; no product change or lane release follows from the historical measurements.
2. **The probe agrees with M2.** On (b) the tessellation drop lies up to 0.043 mm too deep (p99 0.030 mm); on (c) the median offset is 0.0005 mm and the p99 0.015 mm. Its largest readings on (c), 0.025 to 0.027 mm, are not in M2's checker, which finds no pose deeper than 0.006 mm on (c): they are the probe's ball-only model where the flutes touch, or `distance` answers of the OI-839 kind, and are not taken as measurements.
3. **The finer tessellation (c) has different historical costs by operation.** Including the unsuccessful simultaneous attempt gives the 1.24 generation ratio (1.10 with simulation); the six successful operations alone give about 1.78 from the rounded table. Import took 2.3 s instead of 0.2 s. Roughing (1.21), axial (1.58), helical (1.76) and rest (1.93) are below 2 times; simultaneous's 1.06 ratio measures time to refusal, not successful generation; **pencil clean-up (3.61 times, 2.5 to 9.0 s) and indexed finishing (2.2 times, 0.05 to 0.11 s) are over it.** Pencil's is the creases: it finds more on the finer mesh (342,624 drops against 222,948), so its path changes, not only its drops' cost (lane STEPALL-P's concern). Indexed is a twentieth of a second. The owner selected both the existing per-job limit and an additional per-operation limit on 24 Oct 2026 (OI-835); these historical timings do not establish compliance.
4. **Where the time goes.** On (b) the drops are almost all of every finishing operation's generation: 19.8 of helical's 20.6 thread-seconds, 91.2 of axial's 96. So an exact drop's cost lands on the whole operation: at 1.3 s a drop (item 1) nothing is left of the 2-times limit, and at the limit an exact drop may cost at most about twice a tessellation drop, some 150 µs.
5. **Pencil clean-up gains nothing from threads** (NFR-04): 0.85 to 0.99 times the speed on 4 threads against 1, on every form, where the other operations gain 1.3 to 3.8 times. That is the strategy's own, the same on the owner's STL as on the STEP part, and outside #257 (item 6 of the decision).
6. **The STEP part is faster than the owner's STL** for helical, rest and pencil (5.2, 5.3 and 2.5 s against 9.3, 10.0 and 12.3 s) and a little slower for axial finishing (24.0 against 20.0 s, with half as many drops again). Axial finishing takes 24 s on (b) at all threads since lane BUG-TABDEV's change (#252); before it, on the same paths, 975 s.

### M4: determinism

On `57772e7`, merged into this branch on 23 Oct 2026. Run with `--quick` (finishing stepovers coarsened to 1 mm, axial's too), which kept a check at 1, 2 and 4 threads within the two-hour limit. These results cover those stepovers only. Simultaneous finishing, which refuses the blisk, is turned off in the application's project (UI-06), so the application generates the rest.

| Check | Form | 1 thread | 2 threads | 4 threads |
| --- | --- | --- | --- | --- |
| The library's toolpath (every group, serialised), against 1 thread | b | – | identical | identical |
| | c | – | identical | identical |
| | a | – | identical | identical |
| The application's posted G-code (Makera Z1), against 1 thread; gates 1 and 2 | b | clean | identical, clean | identical, clean |
| | a (simultaneous left out) | – | identical | identical |
| The library's form (b) path against the application's (`Regeneration::toolpath`), at 4 threads | b | | | identical |

The last row records agreement on the historical project and code. The refreshed quick run below re-establishes form-b IR equality on current dispatch; helper tests alone check rejection policy, not geometric equivalence.

## Refreshed M2/M4 on current source (quick stepovers, 23 to 24 Oct 2026, Europe/London)

**Measured source:** `827fc45dd8736265d8504ab71bc8c5057ce9c664`, based on the owner-authorised draft merge of main `a2605495d4e05d323d4cc3f85a4f4d6be118072f`. Later documentation commits do not change that measured code. These readings are separate from the historical full-stepover tables above. The raw output, native test summary and environment are retained in [the rerun evidence](0211-step-accuracy-evidence.txt).

**Build and allocation:** Rust 1.97.0 (`2d8144b78`, 7 Jul 2026), release with debug information and no coverage instrumentation; static OpenCASCADE 7.8.1 built through pinned `occt-sys`, CMake 4.4.4, GCC/G++ 14.2.0, C++17 wrapper. AMD EPYC 9V74 host; cgroup CPU quota `400000/100000` (four CPUs), five CPUs in the effective cpuset, 16 GiB memory limit. Rust reports four available cores. Build jobs and Rayon all-thread runs use four threads. The initial writable disk had about 30 GB free. No new product dependency, library API, schema, strategy or default is introduced.

**Inputs and settings:** the exact trial STL hash and STEP hash are those stated above; the STL is kept outside the repository and is not committed. The trial STL has 6,386 triangles and, after the +10 mm X translation, a sampled maximum vertex distance of 0.0313 mm from the exact faces. Forms (b)/(c) have 15,936/106,928 triangles. The exact-face cloud has 5,307,226 points at the existing nominal 0.05 mm spacing. `--quick` sets rest, helical, axial, indexed and simultaneous stepovers to 1 mm; roughing stays at 2 mm stepover, 1 mm stepdown and 0.3 mm stock to leave, and pencil's parameters remain at their resolved defaults. Other tools, feeds, stock, 0.1 mm simulation cell, 0.01 mm path tolerance, 10-degree simultaneous tilt and 2-degree angle step are the harness settings described above. Form (b) retains the import's 0.01 mm / 0.1 rad settings; form (c) explicitly uses **0.01 mm / 0.02 rad**.

From `/workspace/OpenChuckCAM`, with Rust and CMake on `PATH`, the actual build and run commands were:

```text
export CARGO_BUILD_JOBS=4
export CMAKE_POLICY_VERSION_MINIMUM=3.5
cargo build --release -p openchuck-import --features kernel --bins --locked
cargo build --release -p openchuck-app --features kernel-tests --example step_accuracy --locked
export OPENCHUCK_STEP_WORKER=/workspace/OpenChuckCAM/target/release/openchuck-step-worker
export RAYON_NUM_THREADS=4
cargo nextest run --release -p openchuck-app --features kernel-tests --test step_accuracy --test step_accuracy_checks --test step_accuracy_posting --locked --test-threads 1
cargo run --release -p openchuck-app --features kernel-tests --example step_accuracy --locked -- --only m2,m4 --forms a,b,c --fine 0.01 --angular 0.02 --stl /workspace/owner-blisk.stl --quick --keep /workspace/stepall-evidence/project
```

### M2: refreshed sampled accuracy

All distances are mm, rounded to the four decimals printed by the harness. The station and contact-window definitions are unchanged; these are sampled readings, not a certified surface or remaining-material bound.

| Form | Operation | Stock | Stations | Deepest cut | At ends | Largest gap | Drop error in | out | Contact poses |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| a | roughing | 0.3 | 38840 | 0.0000 | 0.0000 | 0.0593 | 0.0136 | 0.0593 | 6587 |
| a | rest | 0 | 67104 | 0.0709 | 0.0709 | 0.0614 | 0.0709 | 0.0605 | 55123 |
| a | pencil | 0 | 14060 | 0.0244 | 0.0244 | 0.0415 | 0.0256 | 0.0403 | 9743 |
| a | helical | 0 | 67104 | 0.0709 | 0.0709 | 0.0614 | 0.0709 | 0.0605 | 55123 |
| a | axial | 0 | 62460 | 0.0777 | 0.0777 | 0.0533 | 0.0780 | 0.0533 | 57401 |
| a | indexed | 0 | 2348 | 0.0702 | 0.0702 | 0.0600 | 0.0704 | 0.0602 | 2091 |
| b | roughing | 0.3 | 38868 | 0.0000 | 0.0000 | 0.0091 | 0.0264 | 0.0072 | 7360 |
| b | rest | 0 | 73468 | 0.0421 | 0.0421 | 0.0097 | 0.0422 | 0.0086 | 61140 |
| b | pencil | 0 | 9860 | 0.0340 | 0.0335 | 0.0086 | 0.0346 | 0.0084 | 6775 |
| b | helical | 0 | 73460 | 0.0421 | 0.0421 | 0.0097 | 0.0422 | 0.0086 | 61132 |
| b | axial | 0 | 94016 | 0.0414 | 0.0407 | 0.0089 | 0.0426 | 0.0087 | 86522 |
| b | indexed | 0 | 2328 | 0.0316 | 0.0316 | 0.0073 | 0.0319 | 0.0074 | 2062 |
| c | roughing | 0.3 | 37728 | 0.0000 | 0.0000 | 0.0050 | 0.0040 | 0.0042 | 7616 |
| c | rest | 0 | 68240 | 0.0058 | 0.0047 | 0.0042 | 0.0052 | 0.0051 | 62275 |
| c | pencil | 0 | 10100 | 0.0066 | 0.0007 | 0.0050 | 0.0022 | 0.0042 | 7288 |
| c | helical | 0 | 68232 | 0.0058 | 0.0047 | 0.0042 | 0.0052 | 0.0051 | 62267 |
| c | axial | 0 | 91204 | 0.0054 | 0.0049 | 0.0060 | 0.0062 | 0.0050 | 84572 |
| c | indexed | 0 | 2260 | 0.0051 | 0.0035 | 0.0031 | 0.0038 | 0.0044 | 2003 |

Simultaneous finishing refused all three forms: the tolerance errors name 0.0485 mm on (a), 0.0180 mm on (b), and 0.0184 mm on (c). No successful simultaneous path or completed seven-operation job is claimed. M4 compares the refusal results too; posting uses only successful groups, and the application disables each form's refused operation.

The current quick-stepover observations retain the accuracy concern: the worst sampled successful finishing cut is 0.0777 mm on (a), 0.0421 mm on (b), and 0.0066 mm on (c); (c)'s largest sampled contact gap is 0.0060 mm. Pencil now uses the B-rep creases on (b)/(c), so its historical facet-crease rows are not current pencil results. On (c), pencil's worst sampled cut is 0.0066 mm although its endpoint maximum is 0.0007 mm: endpoint-only checks would miss the larger between-pose reading. Roughing still has no sampled cut into the part. These readings support investigating a finer toolpath mesh but do not establish the 0.005 mm tessellation allocation, a bounded sub-0.001 mm checker error, or performance compliance.

### M4: refreshed determinism, application equivalence and the replay blocker

**The full measurement returned exit status 1.** Library M4 and form-b application checks passed, then the form-a one-thread application replay failed. The main run did not reach form-a application counts 2/4. Separate diagnostics on the saved project subsequently checked those counts and reproduced the failure. No clean form-a replay result is claimed.

| Check | Form | 1 thread | 2 threads vs 1 | 4 threads vs 1 |
| --- | --- | --- | --- | --- |
| Library IR, including refusal results | a/b/c | reference | identical | identical |
| Production Makera Z1 posted text | a/b/c | reference | identical | identical |
| Application posted text vs library reference | b | identical | identical | identical |
| Application replay gates 1/2 | b | clean | clean | clean |
| Application IR vs library IR | b | — | — | identical |
| Application posted bytes, saved-project diagnostics | a | reference (CLI) | identical | identical |
| Application gate 2, including separate diagnostics | a | **fails** | **fails** | **fails** |

The form-b library/application IR equality is directly checked at four threads. Its posted text agrees at every count. The main run also compared form-a one-thread application text with the library before failing its gate check. The saved-project production CLI and two/four-thread diagnostic programs compare byte for byte identical (SHA-256 `9c4d20200e5a70076e1e16c010fc170ae255bcb57f5c104c04d8940b773fc268`). The separate two/four-thread reports contain no gate-1 findings, but gate 2 remains an error. The diagnostic process returning zero means that it completed its investigation, not that the G-code passed.

Form (c) uses the production post directly because the application has no finer-tessellation setting; no application or replay-gate result for (c) is claimed. Its posting-context conclusion is supported by the full successful form-b application/context comparisons, not by the six helper tests alone. These results cover the quick stepovers and six successful operations; the refused simultaneous operation is excluded from posted programs and its refusal is compared in the serialized library results.

#### The form-a failure, reproduced without changing the limit

The production CLI reproduces `tip error 0.000912 mm is above 0.000867 mm` on the saved form-a project. The accepted gate-2 limit is still the verification plan's rounding budget (OI-91); it was not relaxed. Exact commands, diagnostic source and output are in [the evidence](0211-step-accuracy-evidence.txt). The CLI command is:

```text
OPENCHUCK_STEP_WORKER=/workspace/OpenChuckCAM/target/release/openchuck-step-worker RAYON_NUM_THREADS=4 cargo run --release -p openchuck-cli --locked -- --threads 1 regenerate /workspace/stepall-evidence/project.stl --post makera-z1 --tool-numbers keep --out /workspace/stepall-evidence/form-a-diagnostic
```

A temporary diagnostic using the public application, rounding and kinematic APIs reproduces a rounding-only error of **0.0009120345106203881 mm** at pencil's repositioning pose (zero-based group 2 and pose 2295; G-code line 31,243 in this program). Work-frame XYZ before rounding is `(21.558251369535647, -0.016498447475797207, 45.000000000000014)` and A is `120.93751838129586` degrees. Written XYZ is `(21.558, -0.016, 45.000)` and A is `120.938`. The XYZ errors are `(-0.000251369535647, +0.000498447475797, approximately 0)` mm and A error is `+0.000481618704143` degrees: each is within half a unit at three decimals. Inverting the rounded XYZ/A at approximately 45 mm rotary radius produces the larger model-tip error. The matching line is a modal G0 move, following `G0 Z45.000`.

This characterizes the coupled XYZ/A rounding at one real pose; it uses shared kinematics and is not independent certification of the post. The fixed `sqrt(3) * 0.0005 * 1.001` tip limit in `Tolerances::for_decimals` accounts for three linear axes, while this example also incurs angular rounding at a nonzero radius. **The gate remains failed.** A decision about precision, the comparison budget or a product correction belongs through the coordinator and an authorised lane; STEPALL-M does not change the post, the checker or the accepted limit to make the measurement green. After the owner instructed this lane to allocate numbers itself on 24 Oct 2026, the checked next numbers were claimed: **OI-984**, [decision 0234](0234-replay-output-rounding-correction.md) (implementation proposed), and [GitHub issue #324](https://github.com/paulhampson/OpenChuckCAM/issues/324). Decision 0233 and OI-983 remain BUG-EDIT's. The numbering audit is recorded in 0234; implementation and lane assignment are still pending.

Owner choice, 24 Oct 2026: preserve the current tip-error limit and pursue an output correction through the coordinator, including investigation of posting precision and controller support before any precision change. Do not revise the comparison budget to make this case pass. Tracking is now OI-984 / issue #324, with implementation record 0234 proposed; an authorised implementation lane remains to be assigned. The owner superseded central number assignment with lane-selected numbers on 24 Oct 2026. The gate remains failed and PR 321 remains draft; no report acceptance or tessellation direction resolves this blocker.

### Validation and remaining limits

The kernel worker and complete harness build. All ten native nextest tests passed in 5.005 s, uninstrumented with one test at a time; each geometry test was under 2 s and the six posting/rejection tests each under 0.01 s. The six non-kernel contract tests also pass independently. All three posting tests failed against the original stub before implementation. Native harness/test clippy with warnings denied, formatting, dependency rules, SPDX/licence checks, dependency/register records, test-data checks, conflict-marker checks, publishing metadata and traceability pass (`cargo xtask all`: 99/99 P1 requirements have tests). `cargo deny --locked check` passes advisories, bans, licences and sources.

The verification commands (nextest 0.9.149, cargo-llvm-cov 0.9.1, cargo-deny 0.20.2) were:

```text
cargo fmt --all -- --check
cargo clippy --release -p openchuck-app --features kernel-tests --example step_accuracy --test step_accuracy --test step_accuracy_posting --test step_accuracy_checks --locked -- -D warnings
cargo xtask all
cargo deny --locked check
cargo llvm-cov nextest --profile coverage -p openchuck-app --test step_accuracy_posting --test step_accuracy_checks --locked --no-report
cargo llvm-cov report --summary-only --ignore-filename-regex '/(tests|examples|benches)/'
cargo llvm-cov clean --workspace
```

The focused `cargo llvm-cov nextest --profile coverage -p openchuck-app --test step_accuracy_posting --test step_accuracy_checks --locked --no-report` run passed; the standard summary excluded tests/examples/benches and instrumented artifacts were cleaned with `cargo llvm-cov clean --workspace`. This narrow run does not establish 95% whole-crate coverage; the changed implementation is example code, outside the standard product-source summary, and no product-source line is changed.

M1 and M3 were not refreshed. There is no new five-run, per-stage timing measurement or current performance ratio. The smaller form-b smoke overlapped the full run's STL provenance check, and native clippy overlapped initial preparation; printed regenerate times are diagnostics, not M3 evidence. The nonuniform B-spline sampling bound, tapered-tip sampling estimate, historical M1 sampling/closest-point limits, unmet 0.005 mm allocation and historical timing departures remain unresolved and unwaived. Quick-stepover determinism does not establish determinism at the owner's full finishing stepovers. OI-839 remains STEPALL-X's. Decision 0209's existing per-job policy is retained, and the owner has selected the additional per-operation limit described below; compliance with either is unproven.

Record 0211 is **accepted by the project owner**, directly in this lane session on 24 Oct 2026 after the full revised record was shown ("Accept 0211"). The owner has now made the substantive choices directly in this lane session, individually and in the requested order; those choices are recorded below. No methodology deviation is accepted. PR 321 stays draft and is not released to the Merge Manager. The form-a replay gate, sampling/error-budget, five-run timing and full-stepover determinism criteria remain unresolved. Full platform CI and external review remain for the authorised release sequence. Replay tracking and the next-number audit are now recorded in OI-984 / issue #324 / proposed implementation record 0234 under the owner's new lane-allocation instruction. Coordinator lane scheduling and release remain separate; this session starts no other lane.

## Follow-up: successful-run timing and isolated pencil remeasurement (24 Oct 2026)

Source `f7a770d044169ff0e03d8c9feabeb389ff3afe49` makes M3 distinguish successful benchmarks from time to refusal: medians require at least five successful samples, refused attempts remain labelled, and the whole-job generation median is computed from each complete run's total rather than the sum of operation medians. Missing, non-finite and negative samples cannot qualify. Timing-only runs skip unused M2 clouds; `--skip-probe` skips the expensive exact-ball diagnostic explicitly. M2 now describes its grid spacing as a nominal target with no established physical coverage bound.

The owner chose pencil remeasurement before separate performance tracking. Five successful runs at each of one/four threads, for each form, now cover **an isolated pencil operation on initial stock**. No `--quick` was used; the current B-rep preparation remains inside generation timing. This is not the full sequence with stock carried from preceding operations. Same pinned release build, four-CPU quota and 16 GiB allocation as above; no concurrent compilation, tests or coverage during timing. Raw output, exact command and validation are in [the pencil timing evidence](0211-step-accuracy-pencil-timing.txt).

| Form | One-thread median generation, s | Four-thread median generation, s | Speed-up |
| --- | --- | --- | --- |
| a: trial STL | 6.861560 | 3.462898 | 1.98 times |
| b: import STEP mesh | 8.204402 | 3.193163 | 2.57 times |
| c: finer STEP mesh | 12.555629 | 6.320591 | 1.99 times |

All 30 trials succeeded. Form c/b's four-thread generation ratio is **1.98 times**, narrowly below 2 for this isolated case. Current pencil benefits from threads on every measured form; the historical no-speed-up observation is not current evidence for this case. No separate pencil-performance issue is opened by this result; confirm performance in the full sequence before concluding the broader requirement is met. These numbers establish neither whole-sequence performance nor a tessellation default.

Two new requirement-based timing tests failed before implementation, then passed. All twelve native geometry/posting/rejection/timing tests passed uninstrumented in 5.164 s; native example/test clippy with warnings denied, formatting and `cargo xtask all` passed. Focused coverage tests passed; the product-source summary excludes the example helper and supports no whole-crate coverage claim. Instrumented artifacts were cleaned.

**Still outstanding:** full-sequence five-run timings, separately measured fitting and assembly stages, bounded physical sampling error, the 0.005 mm allocation, full-stepover M4 and replay correction OI-984 / issue #324. The public progress messages do not supply complete stage boundaries; no fitting/assembly time is inferred by subtracting summed parallel drop time from wall time. PR 321 stays draft. This evidence update does not amend the accepted choices or waive a criterion.

## M2's sampling criterion fails on an analytic flat-cutter case (24 Oct 2026)

The current nominal 0.05 mm grid is not merely missing a proof for nonuniform B-splines. Against a small analytic plane, with a 2 mm flat cutter (the shape used by pencil), the actual `Cloud` measurement reports 0.003333333 mm penetration where the exact minimum clearance proves 0.010000000 mm penetration: it misses **0.006666667 mm**, above the required sub-0.001 mm error. Halving the nominal spacing to 0.025 mm gives the same miss. A 0.01 mm grid happens to hit the minimum exactly, while 0.005 mm misses 0.001379310 mm; agreement at one resolution is not a bound. See [the exact geometry, proof, raw outputs and diagnostic source](0211-flat-cutter-sampling-diagnostic.txt).

M2 now prints its measurements but exits with an explicit unqualified-sampling error. A rejection-policy test requires a finite, nonnegative, proven clearance-error bound strictly below 0.001 mm, and rejects a missing bound, the demonstrated miss, the exact limit and non-finite values. The current caller supplies no bound; nominal grid spacing is never substituted for one. M1, M3 and M4 remain independently runnable. This guard does not repair the sampler or waive the criterion: a replacement independently bounded clearance check remains required before STEPALL-M release. The blisk readings above remain sampled observations, not certified maxima or remaining-material bounds.

## Measuring every M1 triangle (24 Oct 2026)

The historical 20,000-triangle-per-face cap was a methodological departure from decision 0209. It is removed: M1 now measures every triangle's corners, edge midpoints and barycentric grid. A regression places the only 0.02 mm deviation at a triangle index skipped by the old stride. It failed with a reported maximum of zero before the change, then passed after it. All 13 native harness tests pass uninstrumented in 3.086 s; native clippy also passes. The focused instrumented regression also passes; the changed example code is outside the product coverage report, whose application product-line coverage is zero for this selection. Instrumented artifacts were cleaned. [Commands and red/green evidence](0211-m1-triangle-coverage.txt).

The initially launched tighter-setting investigation used the capped method and was stopped without producing a measurement table. The replacement investigations use the uncapped method at the original candidate (0.01 mm / 0.02 rad) and a tighter setting (0.001 mm / 0.01 rad). Neither setting is adopted by running these measurements. The M2 physical-coverage bound and the exact closest-query limitation OI-839 remain separate; removing the triangle cap does not solve them.

## Optional stage timing for the required M3 rerun (24 Oct 2026)

To perform the owner's retained five-run/per-stage verification, `ProgressSink` gains default methods `wants_timings` (false) and `timing`; `JobContext::time_stage` returns a scope guard. Existing hosts need no change and read no timing clock. The measurement example opts in, collecting durations without changing progress counters, cancellation or path output. This is an additive library API; the library version and schemas are unchanged.

Fitting is measured directly in `wrap_fit::fit_curve` after initial drops, in simultaneous finishing after initial curve pieces, and in pencil after its initial drop pass while constructing/checking each crease path. Roughing has no corresponding fitting scope, so its fitting column is zero. Assembly surrounds the shared `assemble_with_entry` call. Query/drop time includes calls made inside fitting; parallel fitting scopes overlap. Both are inclusive summed durations, not additive wall-time components. Generation includes the remaining preparation/validation, including B-rep pencil corner construction. Simulation is measured separately, and a new job timer measures the sequential job from stock construction through its final simulation. Medians are computed from complete run totals; refused sequences cannot qualify as successful jobs.

The scope test was first compiled against the missing API and failed, then passed with the implementation. Tests cover early error returns, default opt-out and equality of generated paths with timing enabled/disabled. The full uninstrumented path/types suite passed all 382 then-existing tests; the new/default-host/path-invariance selection passed three tests. All twelve native measurement tests pass. Local clippy, format, register, dependency, licence and traceability checks pass (99/99 P1 requirements traced).

Focused coverage covers the timing guard, opt-in/opt-out, callback, shared assembly and helical fitting. Pencil/simultaneous scope lines and diagnostic Debug formatting were not reached by that focused coverage selection; their existing paths passed the full uninstrumented suite. Coverage lower bounds and the exact selection are in [0234's evidence](0234-replay-output-evidence.txt); no 95% claim is made. Three observed long tests are added to both coverage-exclusion and nightly-long profiles, as AGENTS.md requires; none exceeded ten minutes.

A five-run cylinder/helical smoke (`--only m3 --part cylinder --forms b --ops helical --runs 5 --threads 1 --quick --skip-probe --fine 0.01 --angular 0.02`) completed successfully and printed all three new columns and both job medians. It overlapped M1/M4 validation and is not performance evidence. Its raw output is [stage-timing smoke evidence](0211-stage-timing-smoke.txt).

The isolated pencil measurements above remain measurements of the previous harness. Full-sequence benchmarks below must use the instrumented release harness with no concurrent CPU workload; fitting/drop columns must not be subtracted from wall time or summed to invent an exclusive breakdown.

## Replay correction included by the owner (24 Oct 2026)

The owner instructed: "Complete all steps and also include changes for #324 in this work." This authorises the OI-984 correction in PR 321 and supersedes its former pending-lane restriction. [Implementation record 0234](0234-replay-output-rounding-correction.md) remains proposed; the output-correction policy in this record is already accepted.

The post now rounds A first, transforms the original model tip at that written angle, then rounds XYZ. Precision and replay limits are unchanged. The original saved form-a quick project passes production CLI replay: 66,076 moves, maximum tip error printed as 0.0008 mm against the unchanged 0.000867 mm limit. The direct regression covers both axes/senses and both posts, including a fed move; a separate reader test still rejects the original bad output. The two lobed reference goldens are updated for corrected Y/Z rounding and the corresponding feed calculation, and pass the independent golden audit. See [correction evidence](0234-replay-output-evidence.txt).

This fixes the demonstrated output defect; it does not turn the earlier quick measurements into full-stepover evidence or establish the geometric error budget. The full-stepover M4 run, further M1 investigation and full-sequence timings are recorded separately when complete. PR 321 remains draft.

## Owner choices made before record acceptance (24 Oct 2026)

The owner requested: "Take me through the decisions. Ask me about the decision record last." The following answers were given directly in this lane session, rather than relayed. They select the directions and conditions below. The owner subsequently accepted the full revised record directly in this session on 24 Oct 2026: "Accept 0211".

| Choice | Owner answer | Result |
| --- | --- | --- |
| OI-834, interim toolpath mesh | A | Pursue a fixed finer toolpath mesh; keep the display mesh unchanged. The tested 0.01 mm / 0.02 rad setting is a candidate to verify, not an approved default. No stored setting or schema claim. OI-831 remains open. |
| STEPALL-X order | A | Address OI-839 and reliable face/trim selection first, then exact ball-tool drops, then other cutter profiles. |
| OI-835, performance scope | A | Retain the existing per-job 2-times limit and add a 2-times limit for each operation, using medians of five successful runs with equivalent settings. Both must pass. |
| Form-a replay failure | A | Preserve the current tip-error limit and pursue an output correction through the coordinator; check controller support for any precision change. |
| Incomplete verification | A | Retain every verification requirement. Complete the missing evidence and resolve the accuracy allocation before release; record acceptance waives none of it. |
| Simultaneous-finishing refusal | B | Keep it documented in this report without requesting separate tracking or scheduling a separate investigation. |
| Historical pencil thread scaling | A | Remeasure current pencil performance as part of the required five-run timing work, then decide whether separate performance tracking is warranted. |

## Decision

This section incorporates the owner's substantive selections. The owner accepted the complete revised record directly in this session on 24 Oct 2026 after reviewing it in full. The historical full-stepover figures and the refreshed quick measurements retain their stated limits. No criterion is waived, no failed gate is accepted, and no product default or additional lane is released. This accepted record supplements decision 0209 with item 4's additional per-operation requirement; decision 0209 and verification plan 4.1 are reconciled in the same change. All other verification requirements remain in force.

1. **Per strategy, does the exact surface help?** Decision 0209 item 4.1 asks whether (b)'s error goes over the 0.005 mm share, or decides a gouge or a gap over the tolerance, where (c) or (d) would not.

   | Strategy | (b), historical full stepovers | Helps? | Why |
   | --- | --- | --- | --- |
   | Rotary roughing | never cuts the part; at most 0.009 mm over its 0.3 mm | **No** | It leaves 0.3 mm; the error is under 3 % of it. It keeps the tessellation, and its notes say so (decision 0209 item 4.4). Not in lane STEPALL-X. |
   | Helical finishing | 0.043 mm into the part | **Yes** | Over the whole tolerance, at the drops. Ball end. |
   | Axial finishing | 0.043 mm | **Yes** | As helical; a tapered ball, so it needs the outline form of the exact drop (decision 0209 item 4.2), after the ball. |
   | Indexed finishing | 0.032 mm | **Yes** | As helical. Ball end. |
   | Rest machining | 0.043 mm | **Yes** | Its passes are `wrap_fit`'s, as helical's, and gain with them. Its detection is not changed. |
   | Simultaneous finishing | 0.019 mm on `revolved_spline`, 0.009 on `cone` (it refuses the blisk on every form) | **Yes** | Its drops are the tessellation's like the others; its drive points are already exact. |
   | Pencil clean-up | 0.035 mm | **Yes**, through its drops; its creases are lane STEPALL-P's | A flat end here (the owner's 2 mm), so the outline form of the drop. |

2. **Pursue a fixed finer toolpath tessellation first (OI-834, OI-831; owner selected A).** Historically, form (c) reduced the sampled deepest finishing cut from 0.043 to 0.006 mm. The attempted sequence took 1.24 times as long including the refused operation (1.10 with simulation); successful-operation generation alone gives about 1.78 from the rounded rows. Import took 2.3 s instead of 0.2 s; per operation, pencil clean-up (3.6 times, its creases change on the finer mesh) and indexed finishing (2.2 times, a twentieth of a second) are over the 2 times, the others within it (M3 item 3). It does not meet the 0.005 mm share on the blisk (0.0057 mm, M1 item 3): no kernel setting tried does. Alternatives presented to the owner, with A selected subject to verification:
   - **(A, selected) A fixed toolpath tessellation, no stored setting:** pursue a finer mesh for toolpaths, keeping the import's mesh for display, mesh checks and stock comparison. The tested 0.01 mm / 0.02 rad setting remains a candidate; verify the setting against the unchanged accuracy criteria and item 4's performance limits before adoption. No schema claim. Intended for STEPALL-T before X, subject to coordinator scheduling and release. It does not give the user IMP-06's control (OI-831 stays open).
   - (B) As (A), with the setting stored in the project and shown with the model, closing OI-831: needs a schema claim in the work breakdown, and a choice of what the user sets (one tolerance, or linear and angular).
   - (C) The tessellation follows each operation's tolerance: a different mesh per tolerance, cached per part; the most accurate, the most memory and the most code.
   - (D) Neither: wait for X.

   The refreshed quick run on current dispatch measures a 0.0421 mm worst finishing cut on (b) and 0.0066 mm on (c), with a 0.0060 mm contact gap on (c). It supplies no new performance ratio, does not establish the allocation or sampling bound, and leaves the separate form-a replay failure unresolved. The owner selected option A as the interim direction to pursue, with the setting verified before adoption. These observations do not authorise a default or a waiver; STEPALL-T has not been started or released by this session.

   The schema claim OI-834 asks about is needed only for (B).
3. **Query foundations, then ball drops, then other profiles (OI-81, OI-835; owner selected A).** Historical queries made an exact ball drop cost 1.1 to 1.3 s, four orders of magnitude over the approximate allowance inferred from the 2-times limit (M3 item 1). The owner selected correctness and face/trim query foundations before ball drops, followed by other cutter profiles. The proposed engineering sequence within that direction is:
   1. fix `ExactFace::closest` at a face's own boundary (OI-839);
   2. make the trim test and the boundary search cheap (an index on the trim polylines: the blisk's planes and cylinders cost 700 µs a query against 6 µs untrimmed);
   3. a faster closest point on B-spline faces (1.8 ms on a blade), with a per-face seed from the tessellation contact;
   4. search only the faces within the tool's reach of the seed contact, not every face nearer than the best answer;
   5. then the ball drop (helical, indexed, rest, simultaneous), then the outline form (axial's tapered ball, pencil's flat end), checked against the 2-times limit on this harness at each step.

   A fixed finer toolpath mesh is the selected interim direction, subject to item 2's verification conditions and the unmet tessellation share. This refines STEPALL-X's internal order; coordinator scheduling and lane release remain separate, and this session does not start X.
4. **Require both whole-job and per-operation 2-times limits (OI-835; owner selected A).** Retain decision 0209's whole-job limit and add a limit of 2 times the baseline for each operation. Compare medians of five successful runs with equivalent settings, on the same machine at all threads, against the import's tessellation baseline; retain the existing one-thread/all-thread reporting requirements. Both the whole-job and each operation's limit must pass before release. An unsuccessful operation's time to refusal cannot supply a successful job or operation benchmark. No absolute-time exemption was selected for short operations. The current report does not establish compliance: historical timings use fewer runs and include an unsuccessful operation in the 1.24-times total. Four successful historical operations are below 2 times, pencil clean-up (3.6) and indexed finishing (2.2) are above it; the successful rows sum to about 1.78 times. These are historical observations, not current passes or failures under the required method. The estimated 150 µs exact-drop allowance is a historical engineering estimate, not an accepted strategy budget.
5. **The verification plan's 0.005 mm tessellation share (4.1) is not met by any tessellation of the blisk tried** (M1). Form (c)'s historical sampled cut and contact-gap readings are below 0.01 mm (0.006 mm deepest, 0.0067 mm largest gap); the refreshed quick readings are 0.0066 mm and 0.0060 mm, with the sampling limits above. That does not establish the entire tolerance budget. An exact drop has not yet been built or verified by this lane; its accuracy remains to be demonstrated by STEPALL-X. The owner chose to retain all verification requirements: establish the sampling bound, complete the required five-run and per-stage timings, verify full-stepover determinism, and resolve the accuracy allocation before release. Record acceptance documents outstanding gates; it does not certify a setting or waive any criterion.
6. **Disposition of findings outside #257, selected by the owner.** Simultaneous finishing refuses the blisk at every setting tried and on every form, including the STL, at the blades' roots (M2, reproduced in the refreshed quick run). The owner chose B: retain this documented limitation without requesting separate tracking or scheduling an investigation. The refusal protection remains. Historical pencil clean-up gained nothing from 4 threads on any form (M3 item 5; NFR-04), with the later isolated B-rep pencil remeasurement reported above; the full-sequence comparison remains separate. The owner chose A: remeasure it as part of the required five-run timing work, then decide whether separate performance tracking is warranted. No new number is taken for either finding. The separate replay defect is now tracked as OI-984 / issue #324 / proposed implementation record 0234 and is now implemented in this owner-authorised work, preserving its existing limit as selected above; verification is reported in the follow-up section.
7. **The import notice stays as decision 0209 item 4.4 has it** (OI-830, lane STEPALL-F): with form (c) a STEP part's toolpaths are still computed on a tessellation, finer than the display's, and its notes should say which.

## Alternatives considered

- **Measure on the corpus only.** Its analytic parts are smaller and cheaper, but none has the blisk's cones, where the import's tessellation is furthest off (0.043 mm): a measurement without them would have underrated the gain.
- **Measure through the application alone.** The application imports at one setting (OI-831), so form (c) could not be run without a product change. The harness runs the library's `generate` with the application's resolved parameters and checks that its form (b) path is the application's, byte for byte (M4).
- **Take the kernel's setting as the error.** Rejected by decision 0003 and shown wrong again here, by up to four times.
- **Sample a 0.25 mm grid as the kernel tests do** (`tests/step_strategies.rs`). It misses up to 0.005 mm of a 1.5 mm ball's reach, the size of the effect being measured; 0.05 mm gives a 0.0003 mm ball-sagitta estimate under that model. It is not a bound for the full cutter set: the analytic flat-cutter counterexample below demonstrates a larger miss.

## Consequences

- **Code:** a measuring example and its tests, in `crates/openchuck-app` (`examples/step_accuracy/`, `tests/step_accuracy.rs`, `tests/step_accuracy_checks.rs`, `tests/step_accuracy_posting.rs`, `examples/step_accuracy/posting.rs`, an `[[example]]` entry in `Cargo.toml`); CI's STEP kernel job runs the geometric tests and builds the example (`.github/workflows/ci.yml`); the M4 rejection tests also run without the kernel. The original harness changed no product code or output. The owner subsequently included #324 in this work: post/checker rounding is corrected with unchanged limits and precision (record 0234), and optional diagnostic stage callbacks are added to the job context for M3. M4 returns an error on failed equality/replay checks; M2 now returns an error after printing its raw measurements until a proven sub-0.001 mm sampling bound exists.
- **Test data:** `testdata/step/valid/blisk.step`, with its row in `testdata/README.md`.
- **Documents:** this record; the decision index; decision 0209's *Measurements* points here; `docs/open-items.md` (OI-839 new; OI-65, OI-76, OI-81, OI-831, OI-834 and OI-835 noted); the work breakdown's STEPALL row.
