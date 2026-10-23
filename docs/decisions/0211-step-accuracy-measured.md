# 0211: What the exact STEP surface gains, measured on the blisk: the measurement of decision 0209 item 4.1, and which strategies the exact drop should cover (lane STEPALL-M; issue #257)

- **Status:** proposed by lane STEPALL-M, 22 Oct 2026. It records measurements and recommends; it changes no output (no golden file, no strategy, no default). The recommendations of item 3 wait on the owner, through the coordinator: OI-834 (a toolpath tessellation setting, and whether it needs a schema claim) and OI-835 (the time limit). Until the owner accepts it, this record is guidance for lane STEPALL-X, not authority (decision 0049).
- **Date:** 22 Oct 2026.
- **Requirements:** IMP-02, IMP-06, OP-03 to OP-06, OP-09, OP-12, NFR-04, NFR-05, NFR-06, NFR-12, GC-07. Verification plan 4.1 (the 0.005 mm tessellation share of the 0.01 mm toolpath tolerance).
- **Builds on:** [decision 0209](0209-step-geometry-for-every-strategy.md) item 4.1 (accepted), which asks for this measurement and says the numbers go into its *Measurements* "or 0210"; the coordinator assigned this lane record 0211, so they are here and 0209's section points to this record.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-839 is new (item 1). OI-832 was closed by decision 0209 and the file is copied here. Progresses OI-65, OI-76, OI-81, OI-831, OI-834 and OI-835 without closing them.

Numbering: the coordinator assigned this lane record 0211 and OI-839. On 22 Oct 2026 the highest record on `main` and every unmerged branch was 0209 (`claude/docs-stepall-proposal`), and the highest `OI-` number was OI-836 on the same branch, so both were free. No project schema number: nothing stored changes.

## Context

Decision 0209 found that every drop-based strategy on a STEP part drops the tool on the import's tessellation (OI-81), and asked that the gain of an exact drop be measured before it is built, strategy by strategy, against the cheap alternative of a finer tessellation. This lane built the measurement and ran it.

## The harness

`crates/openchuck-app/examples/step_accuracy/` (`main.rs`, `measure.rs`), behind the `kernel-tests` feature, as `path_runs` is a measuring example. It changes nothing in the product. Run it with the STEP worker built:

```text
cargo build --release -p openchuck-import --features kernel --bins
OPENCHUCK_STEP_WORKER=target/release/openchuck-step-worker \
cargo run --release -p openchuck-app --features kernel-tests --example step_accuracy -- \
    [--only m1,m2,m3,m4] [--parts ...] [--deflections ...] [--angular RAD] \
    [--forms a,b,c] [--fine MM] [--threads 1,all] [--runs N] [--stl OWNER_BLISK_STL] [--quick]
```

- **Part.** `testdata/step/valid/blisk.step`, copied from the owner's trial file (OI-832; same SHA-256), placed on the rotary axis from X 10 to 60 as the owner's projects place `blisk.stl`. The owner's `blisk.stl` (form a) is read from a path given on the command line (`docs/trial-feedback-projects/blisk.stl` on branch `claude/upbeat-davinci-jk46yd`): it is not in `testdata/`, since OI-832 covered `blisk.step` only. It sits on the STEP part after the owner's own +10 mm X translation; its vertices lie up to 0.031 mm from the exact faces.
- **Operations.** The owner's operations and tools from the trial projects, in this order, each on the stock the ones before it left (SIM-06), Makera Z1, softwood, a 42 mm cylinder of stock 1 mm longer than the part at each end, no workholding: rotary roughing (6 mm flat, stepover 2, stepdown 1, stock to leave 0.3), rest machining (2 mm ball, stepover 0.3), pencil clean-up (2 mm flat), helical finishing (2 mm ball, stepover 0.3, 2 degree step), axial finishing (3.175 mm tapered ball, 0.25 mm tip, 5 degrees, stepover 0.12), indexed finishing (2 mm ball, A 0, 90, 180, 270) and simultaneous finishing (2 mm ball, stepover 0.3). Indexed and simultaneous finishing are not in the owner's projects and take the helical finish's tool. The parameters go through the application's `parameter_values`, so the cutting data fill is the application's.
- **Forms.** (a) the owner's `blisk.stl`; (b) the STEP part as built today, dropping on the import's tessellation (0.01 mm, 0.1 rad); (c) the STEP part dropping on a finer tessellation (item 1 chooses it: 0.01 mm at 0.02 rad). Form (d), the exact drop, is lane STEPALL-X's.
- **Independence.** Nothing measured uses the drop-cutter or the path fitter. M1 measures the tessellation against each face in closed form (planes, cylinders, cones, spheres, tori, from the face's plain data alone) and, for B-spline faces, by `ParametricFace::closest` against the face without its trims; the trimmed distance is reported beside it. M2 judges each tool position with the independent checker's outline (`openchuck_testkit::gouge::Outline`) against points on the exact faces 0.05 mm apart (5.3 million on the blisk), inside their trims and along their trim loops, and against points on the tessellation the tool was dropped on. A 2 mm ball between points 0.05 mm apart misses at most 0.0003 mm; the tapered ball's 0.25 mm tip at most 0.0013 mm.
- **Stations.** Every fed pose, and the quarter, middle and three-quarter points of every fed move, placed on the machine's straight move in X, Y, Z and A (decisions 0025 item 3, 0115 and 0133).
- **Checked first.** `crates/openchuck-app/tests/step_accuracy.rs` (kernel; 3.7 s, in the STEP kernel job) checks the functions on closed forms: the measured error of the corpus cylinder equals its sagitta from the vertices alone at 0.01 and 0.0025 mm; a ball's gouge and gap read within 0.0002 mm; a move's stations lie on the machine's straight move; a mesh is sampled by its area.
- **Machine.** Intel Xeon @ 2.80 GHz, 4 cores, 15 GB, release build, cloud container.

## Measurements

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

1. **On the STEP part as built today (b), every zero-stock finishing operation cuts 0.032 to 0.044 mm into the exact faces**, three to four times the whole 0.01 mm tolerance, at the dropped poses themselves (the "at ends" column), not inside the fitted moves. The drop error says why: the exact faces lie up to 0.043 mm inside the tessellation the tool touched, the cones' error of M1. The fitter's quarter points add nothing beyond the drops (b's deepest cut equals its deepest at the ends).
2. **On the owner's STL (a) the same operations cut 0.070 to 0.079 mm in and leave gaps of 0.054 to 0.063 mm**: the mesh path is worse, because the STL itself lies up to 0.031 mm off the part and its facets are larger.
3. **On the finer tessellation (c) the deepest cut is 0.006 mm and the largest gap 0.0067 mm** for every finishing operation: within the 0.01 mm tolerance, a little over the 0.005 mm share, as M1 predicts for its 0.0057 mm. At the dropped poses the deepest is 0.0052 mm; the fitter's quarter points add up to 0.0011 mm (helical and rest), within its 0.004 mm chord limit.
4. **Roughing gains nothing that matters.** It never cuts into the part in any form; what it leaves beyond its 0.3 mm is 0.009 mm on (b) and 0.005 mm on (c), under 3 % of the stock it leaves for the finishing cut, as decision 0209 item 2 expected.
5. **Rest machining reads as helical finishing** on this project: roughing leaves 0.3 mm everywhere, above rest machining's 0.15 mm threshold, so its passes cover the part and its path is helical finishing's but for the first few moves. It shares `wrap_fit` with helical and axial, so it gains what they gain.
6. **Pencil clean-up** follows creases found in the tessellation; on (b) its drops cut 0.035 mm in, on (c) 0.003 mm. Its crease source (decision 0209 item 2, T) is lane STEPALL-P's.

### M3: time

Release build, 4 cores (the machine above). Decision 0209 asks for the median of 5 runs; the cloud container restarted several times during the long runs and a background job is limited to two hours, so the medians here are of 3 runs at all threads and of one run at 1 thread, each run printed as it ended. Seconds per operation, in machining order on the stock the ones before it left. "Drops" is the time inside `drop_tool` summed over the threads (so it can exceed the wall time); "simulate" is the stock simulation of the operation's moves at the project's cell (0.1 mm).

**All threads (4), median of 3 runs:**

| Operation | (a) STL: generate | drops | simulate | (b) STEP today: generate | drops | simulate | (c) finer: generate | drops | simulate |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Roughing | 1.49 | 1.8 | 6.9 | 1.21 | 0.65 | 7.6 | 1.45 | 1.5 | 6.8 |
| Rest | 9.97 | 35.5 | 21.7 | 6.79 | 19.9 | 20.1 | 10.22 | 35.4 | 19.2 |
| Pencil | 68.78 | 37.0 | 0.25 | 29.34 | 4.5 | 0.09 | 25.70 | 18.4 | 0.09 |
| Helical | 9.31 | 35.4 | 12.2 | 5.33 | 18.4 | 13.0 | 9.01 | 33.2 | 12.2 |
| Axial | 534.76 | 24.0 | 119.0 | 975.31 | 33.5 | 326.5 | 799.69 | 56.8 | 309.8 |
| Indexed | 1.13 | 0.17 | 0.07 | 1.32 | 0.07 | 0.07 | 1.48 | 0.20 | 0.06 |
| Simultaneous (until it refuses) | 114.42 | 439.2 | – | 105.63 | 4.2 | – | 100.83 | 9.6 | – |

Drop counts per run: helical 234,045 (a), 256,205 (b), 237,977 (c); axial 556,420, 828,608 and 810,532; pencil 410,107, 222,948 and 342,624. One tessellation drop costs about 70 µs on (b)'s 15,936 triangles and 110 to 160 µs on (c)'s 106,928 (the probe's figures below, on one thread).

**One thread, one run each**, and the speed-up of the generation from 1 thread to 4 (NFR-04; decision 0209 item 4.5 asks for it per form):

| Operation | (a): generate | simulate | speed-up | (b): generate | simulate | speed-up | (c): generate | simulate | speed-up |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Roughing | 2.28 | 22.5 | 1.5 | 1.55 | 22.4 | 1.3 | 2.47 | 22.3 | 1.7 |
| Rest | 19.75 | 60.3 | 2.0 | 10.82 | 62.1 | 1.6 | 21.91 | 62.1 | 2.1 |
| Pencil | 56.17 | 0.45 | **0.82** | 24.90 | 0.24 | **0.85** | 21.30 | 0.22 | **0.83** |
| Helical | 18.33 | 39.4 | 2.0 | 9.54 | 41.3 | 1.8 | 20.71 | 41.1 | 2.3 |
| Axial | 508.50 | 455.5 | **0.95** | 824.19 | 1,104.9 | **0.85** | 678.24 | 1,107.6 | **0.85** |
| Indexed | 1.36 | 0.22 | 1.2 | 1.36 | 0.23 | 1.0 | 1.04 | 0.20 | 0.7 |
| Simultaneous (until it refuses) | 219.10 | – | 1.9 | 381.07 | – | 3.6 | 362.93 | – | 3.6 |

Pencil clean-up and axial finishing are slower on 4 threads than on 1, on every form; their simulations scale (axial's 1,105 s on one thread is 326 s on four). With four threads busy each drop costs about twice what it does alone (helical on (b): 8.8 s of drops on one thread, 18.4 s summed on four), so per-drop costs are compared on one thread.

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

What M3 shows:

1. **An exact drop built on today's exact queries is not usable on the blisk.** One exact ball drop costs 1.1 to 1.3 s, 17,000 to 28,000 times a tessellation drop; helical finishing's 256,000 drops would take about 90 hours of one core. Decision 0209 item 4.5 proposes at most 2 times the tessellation job (OI-835). The cost is the exact query itself: one `BrepSurface::distance` takes 50 to 60 ms, because it visits several blades and one `closest` on a blade costs 1.8 ms and one on a long-trimmed plane or cylinder 0.7 ms (M1 item 7). Lane STEPALL-X needs a faster closest point on B-spline faces, a faster trim test, and a search of the faces near the seed only, before the drop is worth building; until then form (c) is the way to the accuracy.
2. **The probe agrees with M2.** On (b) the tessellation drop lies up to 0.043 mm too deep (p99 0.030 mm); on (c) the median offset is 0.0005 mm and the p99 0.015 mm. Its largest readings on (c), 0.025 to 0.027 mm, are not in M2's checker, which finds no pose deeper than 0.006 mm on (c): they are the probe's ball-only model where the flutes touch, or `distance` answers of the OI-839 kind, and are not taken as measurements.
3. **The finer tessellation (c) costs little.** Its generation is within 2 times today's STEP path for every operation: helical 1.7 times and rest 1.5 times (their time is mostly drops, each about twice as dear on seven times the triangles), roughing 1.2 and indexed 1.1 times, and pencil and axial faster (0.88 and 0.82 times). Its import takes 2.3 s instead of 0.2 s.
4. **Axial finishing and pencil clean-up do not gain from threads** (NFR-04): on every form they take longer on 4 threads than on 1 (0.82 to 0.95 times the speed). That is the strategies' own, the same on the owner's STL as on the STEP part, and is outside #257; it is reported to the owner with #252 (item 6 of the decision).
5. **Axial finishing's time is not in its drops** (issue #252). It takes 535 s on the STL, 975 s on (b) and 800 s on (c) at all threads, and its drops sum to 24 to 57 s of thread time, 1 to 2 % of the run; its simulation adds 119 to 327 s. An exact drop would barely change its time; what #252 needs is elsewhere in the strategy (its fitting and assembly at 0.12 mm stepover), outside #257.
6. **The STEP part is faster than the owner's STL** for helical and rest machining (5.3 and 6.8 s against 9.3 and 10.0 s) and pencil (29 against 69 s), but slower for axial finishing (975 against 535 s, with half as many drops again).

### M4: determinism

MEASURE_M4

## Decision

These are recommendations on the measurements above. Items 2 and 3 need the owner's choice (asked through the coordinator); the rest follow from decision 0209.

1. **Per strategy, does the exact surface help?** Decision 0209 item 4.1 asks whether (b)'s error goes over the 0.005 mm share, or decides a gouge or a gap over the tolerance, where (c) or (d) would not.

   | Strategy | (b) today | Helps? | Why |
   | --- | --- | --- | --- |
   | Rotary roughing | never cuts the part; at most 0.009 mm over its 0.3 mm | **No** | It leaves 0.3 mm; the error is under 3 % of it. It keeps the tessellation, and its notes say so (decision 0209 item 4.4). Not in lane STEPALL-X. |
   | Helical finishing | 0.043 mm into the part | **Yes** | Over the whole tolerance, at the drops. Ball end. |
   | Axial finishing | 0.043 mm | **Yes** | As helical; a tapered ball, so it needs the outline form of the exact drop (decision 0209 item 4.2), after the ball. |
   | Indexed finishing | 0.032 mm | **Yes** | As helical. Ball end. |
   | Rest machining | 0.043 mm | **Yes** | Its passes are `wrap_fit`'s, as helical's, and gain with them. Its detection is not changed. |
   | Simultaneous finishing | 0.019 mm on `revolved_spline`, 0.009 on `cone` (it refuses the blisk on every form) | **Yes** | Its drops are the tessellation's like the others; its drive points are already exact. |
   | Pencil clean-up | 0.035 mm | **Yes**, through its drops; its creases are lane STEPALL-P's | A flat end here (the owner's 2 mm), so the outline form of the drop. |

2. **The finer toolpath tessellation first (OI-834, OI-831; owner's choice).** Form (c) gives most of the gain now, at a cost decision 0209's 2-times limit allows: the deepest cut falls from 0.043 to 0.006 mm on every finishing operation, generation takes at most 1.7 times as long (helical) and the import 2.3 s instead of 0.2 s. It does not meet the 0.005 mm share on the blisk (0.0057 mm, M1 item 3): no kernel setting tried does. Options, recommended first:
   - **(A) A fixed toolpath tessellation, no stored setting:** the application tessellates a STEP part for toolpaths at 0.01 mm and 0.02 rad, and keeps the import's for display, the mesh checks and the stock comparison. No schema claim. A small lane (STEPALL-T, decision 0209's option) before X. It does not give the user IMP-06's control (OI-831 stays open).
   - (B) As (A), with the setting stored in the project and shown with the model, closing OI-831: needs a schema claim in the work breakdown, and a choice of what the user sets (one tolerance, or linear and angular).
   - (C) The tessellation follows each operation's tolerance: a different mesh per tolerance, cached per part; the most accurate, the most memory and the most code.
   - (D) Neither: wait for X.

   The schema claim OI-834 asks about is needed only for (B).
3. **The exact drop waits on fast exact queries (OI-81, OI-835; owner's choice of the order).** With today's queries an exact ball drop costs 1.1 to 1.3 s, four orders of magnitude over the 2-times limit (M3 item 1). Lane STEPALL-X is recommended to start with the queries, not the drop:
   1. fix `ExactFace::closest` at a face's own boundary (OI-839);
   2. make the trim test and the boundary search cheap (an index on the trim polylines: the blisk's planes and cylinders cost 700 µs a query against 6 µs untrimmed);
   3. a faster closest point on B-spline faces (1.8 ms on a blade), with a per-face seed from the tessellation contact;
   4. search only the faces within the tool's reach of the seed contact, not every face nearer than the best answer;
   5. then the ball drop (helical, indexed, rest, simultaneous), then the outline form (axial's tapered ball, pencil's flat end), checked against the 2-times limit on this harness at each step.
   
   Until then, form (c) is the way to accuracy on STEP parts; decision 0209's order otherwise stands (F and M first, then X, P, E).
4. **The 2-times limit (OI-835) stands as proposed.** It is the measure X is checked against on this harness (M3, all threads, against form (b)); form (c) meets it today.
5. **The verification plan's 0.005 mm tessellation share (4.1) is not met by any tessellation of the blisk tried** (M1). The total 0.01 mm toolpath tolerance is met by form (c) (0.006 mm deepest, 0.0067 mm largest gap). That is a fact for the owner, not a change to the plan: the share is met only by the exact drop.
6. **Two findings outside #257, for the owner through the coordinator:** simultaneous finishing refuses the blisk at every setting tried and on every form, including the STL, at the blades' roots (M2); axial finishing and pencil clean-up are slower on 4 threads than on 1, on every form (M3; NFR-04, issue #252). Neither has an `OI-` number yet: the coordinator assigns one if the owner wants them tracked.
7. **The import notice stays as decision 0209 item 4.4 has it** (OI-830, lane STEPALL-F): with form (c) a STEP part's toolpaths are still computed on a tessellation, finer than the display's, and its notes should say which.

## Alternatives considered

- **Measure on the corpus only.** Its analytic parts are smaller and cheaper, but none has the blisk's cones, where the import's tessellation is furthest off (0.043 mm): a measurement without them would have underrated the gain.
- **Measure through the application alone.** The application imports at one setting (OI-831), so form (c) could not be run without a product change. The harness runs the library's `generate` with the application's resolved parameters and checks that its form (b) path is the application's, byte for byte (M4).
- **Take the kernel's setting as the error.** Rejected by decision 0003 and shown wrong again here, by up to four times.
- **Sample a 0.25 mm grid as the kernel tests do** (`tests/step_strategies.rs`). It misses up to 0.005 mm of a 1.5 mm ball's reach, the size of the effect being measured; 0.05 mm misses 0.0003 mm.

## Consequences

- **Code:** a measuring example and its test, in `crates/openchuck-app` (`examples/step_accuracy/`, `tests/step_accuracy.rs`, an `[[example]]` entry in `Cargo.toml`); the test runs in CI's STEP kernel job. No product code, no library API, no output changes.
- **Test data:** `testdata/step/valid/blisk.step`, with its row in `testdata/README.md`.
- **Documents:** this record; the decision index; decision 0209's *Measurements* points here; `docs/open-items.md` (OI-839 new; OI-65, OI-76, OI-81, OI-831, OI-834 and OI-835 noted); the work breakdown's STEPALL row.
