# 0209: STEP (B-rep) geometry for every strategy that can use it: what applies per strategy, the order, how the gain is measured, and the build lanes (lane STEPALL, proposal; issue #257)

- **Status:** proposed (21 Oct 2026, lane STEPALL, proposal phase: no product code). Only the project owner accepts this record (decision 0049). It proposes a new requirement (OP-23) and verification plan wording; neither document is edited here (item 9). The owner's answers on issue #257 (21 Oct 2026) say every strategy that can use the exact geometry gets it, and the lane proposes the order. **Owner decision on the order (21 Oct 2026, relayed by the coordinator):** option A of item 5. Face sets for every strategy come first, with a measuring lane beside it; then the exact surface for finishing, where measured to help; then pencil; then the engraving depth. That also answers the second question (face selection before exact-surface accuracy, item 6), which was not asked separately. With that answer the owner asked for the wording fixes and the warning before Generate of item 10. The record as a whole is still proposed.
- **Date:** 21 Oct 2026.
- **Requirements:** IMP-02, IMP-06, OP-02 to OP-13, OP-16, INT-02, NFR-04 to NFR-07, NFR-12, GC-07; proposes OP-23.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-830 to OI-836 are new. It starts from OI-233 and proposes how it is closed (item 3). It depends on OI-81, OI-76, OI-65, OI-77, OI-229 and OI-234 without closing any of them. OI-634 and OI-637 are lane SWARF3's and are not touched.

Numbering: the coordinator assigned records 0209 to 0211 and OI-830 to OI-839 to lane STEPALL. On 21 Oct 2026 the highest record on `main` was 0191 and on every unmerged branch no higher, and the highest `OI-` number in use was OI-710, so all of them were free. This record takes 0209 and OI-830 to OI-836. 0210 and 0211 are held for the owner's two answers if either changes this record enough to need its own; otherwise they are unused. No project schema number is needed (item 8).

## Context

Issue #257 (trial feedback F-86, the owner's request): add STEP geometry support to every strategy it can apply to. For each strategy, say plainly whether STEP support applies and why. Measure the accuracy gain instead of assuming it, since where the mesh is already within tolerance the exact surface adds only cost. Report generation time against the mesh path (#200, #252), and keep the output deterministic at any core count (GC-07, NFR-06). The STEP kernel stays in `openchuck-import`, so the path library gets B-rep data only through the surface-source interface. Verify on `blisk.step` (`docs/trial-feedback-projects/blisk.step` on branch `claude/upbeat-davinci-jk46yd`).

**What a STEP part already gets.** Decision 0056 item 2 (accepted) gives every strategy on a STEP part a `BrepSurface`. Its queries do not all have the same accuracy:

| Query (`SurfaceSource`) | On a `BrepSurface` today | Used by |
| --- | --- | --- |
| `project`, `distance`, `cast_ray` | Exact, on the trimmed faces (decision 0017; 0027 item 8) | Simultaneous finishing's drive points and normals (decision 0036), edge following's depth ray, the region test |
| `drop_tool` | **The tessellation** (OI-81: "no exact solver yet") | Roughing, helical, axial, indexed, simultaneous, rest and pencil: every pose they cut |
| `tessellation` | The import's triangles, 0.01 mm and 0.1 rad by default (decision 0027 item 7), not set from the application (OI-831) | Rest detection, pencil's creases, the region's development index (`wrap_fit::region_index`), swarf's fallback samples |
| `exact_faces`, `holes` | Exact | Swarf (decisions 0061, 0111, 0176), radial drilling (decisions 0051, 0056) |

So on a STEP part the drop-based strategies cut the tessellation, which at the default is up to 9.2e-3 mm from a 15 mm sphere (decision 0027 item 7) and up to 0.011 mm from a torus (decision 0017). The tessellation's share of the 0.01 mm budget is 0.005 mm (verification plan 4.1). Decision 0056's tests found nothing deeper than 0.0120 mm into the exact surface with a 3 mm ball, against a limit of 0.03 mm.

The import's notice says "toolpaths are computed on the exact surfaces. The tessellation … is for display and mesh checks only" (`openchuck-import` `step_tessellation_notice`). That is not yet true of the drops (OI-830; NFR-12).

**Face selection.** On a STEP part, a face set is honoured only as swarf's walls and boundary, avoid and check sets (decisions 0111, 0176), and as drilling's holes (decision 0056 item 3). Edge sets are honoured only by edge following. Every other face set is refused with "cannot yet be limited to faces of a STEP model" or "built for swarf only so far" (OI-233). So is a stock to leave on a STEP selection set (`operation.rs`). On a mesh part, five strategies honour a region (`honours_region`): roughing, helical, axial, indexed and simultaneous. Rest, pencil, edge following and drilling take none on any part.

**The region test.** `RegionConstraint::allows` (`openchuck-path` `region.rs`) accepts a cut whose contact lies within `ON_SURFACE` (1e-4 mm) of the region surface. A contact found by a tessellation drop lies on a triangle. Measured against the exact faces of `BrepSurface::with_faces`, it is up to the deflection (0.01 mm) away, so it would be refused. Measured against the triangles of those faces, it is on them. This is why the order of items 3 and 4 matters.

**The blisk.** `blisk.step` (Open CASCADE 7.9, 257 KB) has 25 faces: 7 planes, 5 cones, 3 cylinders and 10 rational B-spline faces (the blades), with 69 edges. The owner's projects on the same branch use `blisk.stl` (a different file from `testdata/stl/blisk.stl`; see that folder's README) with rotary roughing, helical finishing (2 mm ball, 0.3 stepover), axial finishing (3.175 mm tapered ball, 0.12 stepover, about 20 minutes on the owner's laptop, #252) and pencil finishing. That folder's README says not to build tests on these files without copying what a test needs into `testdata/` (OI-832).

Records read: 0003, 0015, 0017, 0018, 0027, 0032, 0036, 0044, 0047, 0048, 0051, 0053, 0055, 0056, 0066, 0076, 0111, 0113, 0115, 0133, 0174, 0175, 0176, 0188, 0190. Open items read: OI-28, OI-65, OI-76, OI-77, OI-81, OI-146, OI-229, OI-233, OI-234, OI-236, OI-634, OI-637.

## What a user sees today on a STEP part (main, 21 Oct 2026)

**In short:** all ten strategies already run on a STEP part. Neither the owner's setup nor a configuration setting is at fault. What is refused is narrower than the messages suggest: five strategies can't yet be limited to chosen faces, and none but swarf takes boundary, avoid or check face sets. Those refusals are a real missing capability (OI-233), but they come late and are worded badly (OI-836).

Read from the code on `main` (`openchuck-app` `operation.rs` `brep_selection`, `selection.rs`, `overview.rs`; `openchuck-ui` `shell/inspector.rs`, `selection_panel.rs`) and from the tests that run every strategy on STEP parts (decision 0056 *Verification*, `tests/step_strategies.rs`, and `req_op_02_req_nfr_07_selections_a_step_part_cannot_use_are_refused`).

**No refusal appears without a selection set.** I traced every path from import to the written G-code; none refuses a STEP part when the operation uses no face set, edge set or per-set stock to leave:
- adding the model;
- the strategy list;
- adding an operation;
- generating with no sets;
- the stock simulation, workholding and inspection, which all use the part's tessellation.

If the owner sees a message on an operation that has no selection sets, that is a bug to fix first, outside the order of item 5: its exact text is needed.

**Where the messages appear.** All of them appear only when the operation is generated, never when the set is added:
- the operation turns red in the operations overview, with the message;
- the same line is added to Messages, with a Show link.

The Geometry section of the operation inspector accepts a face set in any of its four roles (Machine, Boundary, Avoid, Check) for any strategy, without a word. So the application offers something it then refuses, the problem #206 fixed for swarf on an STL part (decision 0174).

**The messages.** Each starts "operation '*name*' cannot be generated: ". The *strategy* below is the strategy's internal name, for example `helical_finishing`.

| Ref | When | The rest of the text, exactly |
| --- | --- | --- |
| R1 | Any boundary, avoid or check set, on any strategy but swarf (checked first) | boundaries, avoid and check surfaces on a STEP model are built for swarf only so far; remove them from the operation |
| R2 | A face set as the Machine selection, on any strategy but radial drilling and swarf | the strategy '*strategy*' cannot yet be limited to faces of a STEP model (OP-02); only radial drilling, to choose its holes, and swarf, to choose the walls it cuts, use selected faces |
| R3 | A selection set with its own stock to leave, on any strategy | selection set '*set*' cannot be used: a stock to leave per selection set is not built for STEP parts yet; clear it on the set and set the operation's stock to leave |
| R4 | An edge set, on any strategy but edge following | the strategy '*strategy*' does not use selected edges; only edge following does |
| R5 | A triangle set, made while the part was an STL and kept after the part became a STEP file | selection set '*set*' cannot be used: it selects triangles, and a STEP model is selected by face and edge. The set itself is marked in error: "a STEP model is selected by face and edge, not by triangle, because its triangles change with the tessellation tolerance" |

**Per strategy.**
- Column (a): does the strategy run on the STEP part with no sets?
- Column (b): which refusals it can give.
- Column (c): whether those refusals are right.
- Column (d): what the user can do after which step.

| Strategy | (a) | (b) | (c) | (d) |
| --- | --- | --- | --- | --- |
| Rotary roughing | Yes, on the tessellation | R1, R2, R3, R4, R5 | R1 to R3 are a real gap (on an STL it takes a region, boundary, avoid and check sets and per-set stock). R4 and R5 are right. | After **STEPALL-F**: rough only chosen faces, inside a boundary, clear of avoid and check faces, with a stock per set. No new action after X: roughing is expected to keep the tessellation, measured in M. |
| Helical finishing | Yes; drops on the tessellation | as roughing | as roughing | After **F**: finish only chosen faces (for example the blisk's blades), with boundary, avoid and check sets and stock per set. After **X**: the same operations cut the exact faces, within the 0.005 mm tessellation share instead of up to 0.01 mm. |
| Axial finishing | Yes | as roughing | as roughing | as helical |
| Indexed finishing | Yes | as roughing | as roughing | as helical |
| Simultaneous finishing | Yes; drive points exact, drops on the tessellation | as roughing | as roughing | as helical |
| Rest machining | Yes | R1, R2, R3, R4, R5 | Rest machining takes no region on an STL either ("cannot be limited to a region"). The refusals are right, but on STEP they say "STEP model", which suggests the cause is STEP. | Nothing new to do: its passes get X through the shared fitter. A region for rest machining would be new for both kinds of part (outside #257). |
| Pencil clean-up | Yes, if the part has a concave crease. Otherwise "pencil clean-up has no inside corner to follow", as on an STL | as rest | as rest | After **P**: it follows the part's real inside edges, not creases found between facets. |
| Edge following | Yes, on the B-rep edges, with edge sets | R1, R2, R3, R5 | R2 is right: it follows edges, not faces. R1 and R3 are as on an STL. | After **E**: the depth is exact on curved edges (OI-229). |
| Swarf | Yes, on its face set, with boundary, avoid and check face sets | R3, R4, R5; and on an STL part it is disabled with the reason (decision 0174) | Right | Nothing more under #257 (OI-634 and OI-637 are SWARF3's) |
| Radial drilling | Yes; a face set chooses the holes | R1, R3, R4, R5; disabled on an STL part (decision 0174) | Right | Nothing more under #257 |

**Wrong or misleading.** None of R1 to R5 refuses something that is built. Four things are wrong in the way they are shown, and STEPALL-F fixes them (OI-836):
- they appear only after Generate, while the Geometry section accepts the set with no warning;
- R2 and R4 name the strategy by its internal name (`helical_finishing`), not its label (Helical finishing);
- R2 shows a requirement ID (OP-02) to the user;
- the STEP import notice says toolpaths use the exact surfaces, which the drops do not yet (OI-830).

STEPALL-F makes R1 to R3 go away for the five strategies that take regions. Fixing the late refusal on its own first would be work F then deletes, so it is proposed as part of F, not a separate fix. The exception is if the owner wants the wording corrected before F lands.

## Decision (proposed)

1. **Three kinds of STEP support, kept apart.** For each strategy, "STEP support" means one or more of:
   - **F, face selection (OP-02, OI-233):** a face set is honoured wherever a triangle set is honoured on a mesh: as the region, the boundary, an avoid or a check set, and as a selection with its own stock to leave (OP-11).
   - **X, exact contact (IMP-02, OP-12, OI-81):** the tool is dropped onto the exact faces, not their tessellation, so the cut follows the real surface. The normals used for the contact feed (OP-13) and the tool axis (OP-06) come from the exact faces too.
   - **T, B-rep topology:** the strategy reads the part's real faces and edges instead of features found in triangles: pencil's concave creases, and edge following's edge curves (OI-229).

2. **Per strategy: what applies, and why.** "Done" means already built and accepted. "Measure" means it applies only if the measurement of item 4 shows a gain. Otherwise the strategy keeps the tessellation and says so (item 4.4).

   | Strategy | F | X | T | Why |
   | --- | --- | --- | --- | --- |
   | Rotary roughing (OP-03) | **Applies** | **Measure** (expected not to apply) | No | It honours a region on a mesh, so it must on a STEP part. It leaves stock (0.3 to 0.5 mm in the owner's projects), 30 to 50 times the tessellation's error, so an exact drop is expected to change nothing that matters. If item 4 confirms that, it keeps the tessellation and its note says so. |
   | Helical finishing (OP-04) | **Applies** | **Applies** | No | A finishing cut at zero stock: the tessellation's 0.01 mm is the whole budget, against a 0.005 mm share. It shares `wrap_fit` with axial and rest, so one exact drop serves all three. |
   | Axial finishing (OP-04) | **Applies** | **Applies** | No | As helical. #252 is on this strategy, so the cost of X is measured here first (item 4.3). |
   | Indexed finishing (OP-04) | **Applies** | **Applies** | No | As helical, at fixed A. |
   | Simultaneous finishing (OP-05, OP-06) | **Applies** | **Applies** | No | Its drive points and normals are already exact (decision 0036); its drop and gouge check use the tessellation. |
   | Rest machining (OP-09) | Does not apply | **Applies** through `wrap_fit` | No | It takes no region on any part, so F is not a STEP gap: a region for rest machining would be a new feature for meshes and STEP alike, outside #257. Its detection samples the tessellation against the earlier stock, at a threshold of 0.15 mm by default (more than a simulation cell), so the tessellation is enough there. Its cutting passes are `wrap_fit` drops and gain X with helical. |
   | Pencil clean-up (OP-09) | Does not apply | **Applies** | **Applies** | No region on any part, as for rest. It finds concave creases in the tessellation (`concave_creases`). On a STEP part, the concave B-rep edges are the real creases: no false creases between the facets of a fillet, and no creases missed under the angle threshold. Its drops gain X. A test needs a corpus part with a concave crease (OI-234 (b)). |
   | Edge following (OP-08) | Done (edge sets) | Does not apply | **Applies** (OI-229) | It follows B-rep edges and takes edge sets (decisions 0053, 0056). It drops no tool: its depth comes from an exact ray. Its points are chords of the edge's tessellation, not points on the exact edge curve, so the depth is exact only to the tessellation (OI-229). Snapping to the edge curve closes that. |
   | Swarf (OP-07) | Done | Done | Done | Swarf reads the exact faces' rulings and takes face sets in every role (decisions 0021, 0061, 0111, 0176). Nothing is left for #257. Mesh avoid and check sets (OI-634) and per-set options for other strategies (OI-637) are lane SWARF3's. |
   | Radial drilling (OP-10) | Done | Done | Done | Holes come from the exact cylindrical faces, chosen by face set (decisions 0051, 0056). Nothing is left for #257. The fixture gap OI-234 (a) is a corpus item, not STEP support. |

   No strategy that can use a STEP part refuses one. Swarf and radial drilling, which need one, are disabled with the reason on an STL part, by the rule of decision 0174 (#206). For a strategy where F or X does not apply, its notes on a STEP part say which form it used and why (item 4.4). For example: "Roughing drops the tool on the part's tessellation (0.01 mm): it leaves 0.5 mm of stock, so the exact faces would change nothing."

3. **Face selection on every strategy, closing OI-233 (lane STEPALL-F).** The host maps each face set to the triangles of its faces in the part's joined tessellation (`BrepSurface::face_of_triangle`). It builds the same surfaces a triangle set gives on a mesh:
   - the region, avoid and check sets as a `MeshSurface` of those triangles;
   - the boundary as `Window::of_mesh` of them, as decision 0176 item 2 already does for swarf;
   - a set's stock to leave as a zone of those triangles, as on a mesh.

   While drops use the tessellation, this is exact to the cut. The contact lies on the triangles the region holds, so `allows` judges it as it would on an STL. A face's boundary in the tessellation is the chord of its edges, at most the deflection from the true edge.
   - **Code.** No library change: `RegionConstraint` already takes `&dyn SurfaceSource`. Every refusal in `brep_selection` and the stock-to-leave check is replaced by the mapping, except an edge set in a role that takes faces, which stays refused, naming the set.
   - **Options.** A set's options (schema 14) stay swarf's. Another strategy refuses a region that sets them, as decision 0176 item 6 says. Whether other strategies take them is OI-637, lane SWARF3's.
   - **Format.** Face sets are already stored (schema 1; decision 0066, schema 12), so there is no schema change.
   - **What changes when X lands.** X changes where the contact lies, so the X lane changes the region surfaces to `BrepSurface::with_faces` and the region test to the exact faces, within the drop's own accuracy (item 4, OI-833). That is part of X's size, not extra work later.

4. **Exact contact, built only where it is measured to help (lanes STEPALL-M and STEPALL-X).**
   1. **The measurement comes first (lane STEPALL-M), with nothing changed.** A committed, repeatable program measures `blisk.step`, copied into `testdata/step/` (the owner agreed on 21 Oct 2026, relayed by the coordinator; OI-832 closed), and the closed-form corpus parts of `openchuck_testkit::brep`. It is an example of `openchuck-app` behind the `kernel` feature, as `path_runs` is a measuring example. It reports:
      - **M1, the tessellation's error.** For each face type and at deflections of 0.01, 0.005, 0.0025 and 0.001 mm: the largest and 99th-percentile distance from the tessellation to its exact face. The tessellation is sampled at its vertices, edge midpoints and a barycentric grid of each triangle, never by the kernel's setting (decision 0003, OI-65). Also: the facet normals' error in degrees (OI-76), the triangle count, and the time to tessellate.
      - **M2, each strategy's cut against the exact surface.** For roughing, helical, axial, indexed, simultaneous, rest and pencil on the blisk, with the owner's tools and steps: the deepest cut into the exact faces (OP-12) and the largest gap left above them beyond the stock to leave. Both are measured by the testkit's independent checker, its tool outline only, against points sampled on the exact faces inside their trims, as decision 0056's *Verification* does. The sample spacing is fine enough that sampling adds under 0.001 mm. They are taken at every fed pose and at the middle and quarter points of every move (decisions 0115 and 0133), since that is where a fitted move strays. Three or four forms are compared:
        - (a) the owner's `blisk.stl`;
        - (b) the STEP part as built today, dropping on the import's tessellation;
        - (c) the STEP part dropping on a finer tessellation (the deflection M1 finds within 0.005 mm);
        - (d) later, in lane STEPALL-X, the exact drop.

        The gain is (b) against (d), and the cheap alternative is (c).
      - **M3, time.** A release build, the median of 5 runs, at 1 thread and at all threads, with the CPU model and core count recorded. Each run reports the total time and the time per stage: surface queries (the drops), fitting, assembly and simulation (#252's breakdown). Every form above is compared against the mesh path (a), on the operations of #252 and #200.
      - **M4, determinism.** Each form's G-code, byte for byte, at 1, 2 and all threads (GC-07, NFR-06).

      The lane writes the numbers into this record's *Measurements* (or 0210, if it is accepted first). For each strategy it says whether the exact surface helps: whether (b)'s error goes over the 0.005 mm share, or decides a gouge or a gap over the tolerance, where (d) or (c) would not.
   2. **The exact drop (lane STEPALL-X, OI-81).** Recommended form: **seed and refine**.
      - The tessellation drop gives the seed. The exact first touch lies within the tessellation's measured distance of its faces (M1, kept per face when the part is built), so the exact answer is searched only in that band along the tool line.
      - The faces within the tool's reach are tested exactly in that band.
      - For a ball end, the test is bisection on the distance from the ball's centre to the faces (exact `closest`), as SP-4 showed (decision 0015).
      - For flat, bull-nose and tapered ends, the test is the tool's outline against exact points on the faces near the seed contact, refined by Newton on the contact condition. If it does not converge, it falls back to the seed and gives a note (NFR-07).
      - The ball comes first: the finishing tools in the owner's projects and in decision 0056's tests are balls.
      - Alternatives considered: the climb of decision 0015 on exact ray hits (a local search that can miss a contact, OI-146), and an exhaustive exact drop (no seed, much slower).
   3. **The cheap alternative is a real option, not a fallback.** If M1 shows a finer tessellation within the 0.005 mm share at a modest cost in triangles and time, then for toolpaths only (display keeps the import's) form (c) may give most of the gain at a fraction of X's cost. The choice is put to the owner with M1 to M3 (OI-834). IMP-06 already asks for a user-controlled tolerance "for display and toolpath computation", which the application does not yet expose (OI-831).
   4. **A strategy that keeps the tessellation says so.** On a STEP part, the operation's notes name the surface the tool was dropped on: the exact faces, or the tessellation at its deflection with the reason (item 2). The import notice says the same, rather than "toolpaths are computed on the exact surfaces", until X is built for every drop (OI-830; NFR-12: nothing may suggest the program needs no checking).
   5. **Performance.** An exact drop is slower than a triangle drop. Decision 0017 measured exact ray and projection on analytic faces as faster than the kernel, and its B-spline projection at 122 µs a call. A ball drop on the tessellation is about 45 µs a pose at 150,000 triangles (OI-146). The proposed starting limit for 4.1 is an exact-surface job within 2 times the time of the same job on the import's tessellation (the median of 5 runs, all threads, same machine). Over that, the result and the cost are put to the owner before the exact drop becomes the default for that strategy (OI-835). Parallel work goes through `crate::parallel` in order, so the speed-up from 1 thread to all is reported for each form (NFR-04).

5. **The order to build (owner question 1; the owner chose this order, option A, on 21 Oct 2026).** Every strategy that can use the exact geometry gets it (the owner's answer on #257). The proposed order:
   1. **STEPALL-M (measure) and STEPALL-F (face sets) together.** They are independent, and neither changes a path on an existing project. F lifts the refusal on roughing, helical, axial, indexed and simultaneous at once, and M gives the numbers every later step is judged by.
   2. **STEPALL-X (exact drop, ball first) on helical, axial and indexed** (one change, `wrap_fit`, which also serves rest machining), **then simultaneous.** These are the zero-stock finishing cuts, where the tessellation is the whole budget. Roughing follows only if M2 shows a gain.
   3. **STEPALL-P (pencil on concave B-rep edges)**, which needs the OI-234 (b) corpus part. It can run beside X: it touches `pencil.rs` and `brep_edges.rs`, not the drop.
   4. **STEPALL-E (edge following on the exact edge curve, OI-229)**, a small lane that can run at any time.
   5. **X for flat, bull-nose and tapered ends**, after the ball, if M2 shows a gain for them.

   Swarf and radial drilling need nothing. Rest machining gets X through `wrap_fit`.

6. **Face selection or exact-surface accuracy first (owner question 2; answered by the choice of option A on 21 Oct 2026: face selection first).**
   - **Face selection first.**
     - **What it fixes.** Today a face set on five strategies is an error, so a user cannot finish only the blades of a STEP blisk at all. The exact surface, in contrast, moves a finishing cut by at most the deflection (0.01 mm; the deepest gouge decision 0056 measured was 0.0120 mm).
     - **Cost.** F is medium-sized and host-only, with no library API change and no schema. X is large, and needs M's numbers to know where it pays.
     - **No rework.** F, built on the tessellation's triangles, matches today's drops exactly. X then moves both together.
   - **Accuracy first.** The other order gives the better cut sooner on whole-part finishing. It leaves the refusals in place for the whole of X, and it builds the region change for exact contacts before there is a region to test it on.

7. **Determinism and isolation.**
   - **Determinism.** Every new query is a pure function of the part and the pose. The exact drop's band, bisection depth and Newton iterations are fixed numbers, not time limits. There is no shared mutable state, and parallel work goes through `crate::parallel` in a fixed order. The same project and post give byte-identical G-code at 1, 2 and N threads (GC-07, NFR-06), on every form of item 4.
   - **Cross-platform.** Bit identity of the pure-Rust evaluators across platforms stays OI-75. Golden files from STEP parts keep the tolerance or per-platform policy of OI-28.
   - **Isolation.** The STEP kernel stays in `openchuck-import`: the path library sees only the plain-data face record (decision 0017 item 2) behind `SurfaceSource`, and the exact drop is pure Rust in `openchuck-path` (`brep.rs`). The library family gains no dependency. A host with its own kernel still implements `SurfaceSource` itself (decision 0017 item 5).

8. **No project file change.** Face sets and edge sets are already stored, and the per-set options are schema 14's. A toolpath tessellation setting (item 4.3), if the owner chooses it, would be stored and would then need a schema claim in the work breakdown first (OI-834). Schemas 16 and 17 are taken (lanes BUG-SPINDLE and SETUPPOS).

9. **Proposed requirement and verification wording (not edited here).** `docs/requirements.md` and `docs/verification-plan.md` are not edited in this change. On acceptance, the first build lane (STEPALL-F) adds them together with its first tests, so `cargo xtask trace` never sees a P1 requirement without one.
   - **Requirement OP-23 (P1)**, in the operations group, after OP-22 (the next free ID on `main` and on every unmerged branch):

     > OP-23 | On a STEP (B-rep) part, every strategy that works on a mesh also works, takes face selections wherever it takes triangle selections on a mesh (as the region, boundary, avoid and check sets, and with a stock to leave per selection), and uses the part's exact faces and edges wherever that measurably improves the result; a strategy that keeps the tessellation for a STEP part says so in its notes, with the reason. | P1

     Alternatives: P2, if the owner wants it outside the first release, or a wider IMP-02 instead of a new ID. IMP-02 is about import and already asks that "selection and surface evaluation use exact geometry"; a separate operations requirement keeps one testable statement per strategy.
   - **Verification plan, section 4 (a new row):**

     > OP-23 | P1 | T, A | Integration, system | For each strategy, on the STEP corpus and the blisk: a face set as region, boundary, avoid and check set keeps the cut where it does on the same faces of an STL; the deepest cut into the exact faces and the largest gap above them, at every fed pose and at the middle and quarter points of each move (decisions 0115, 0133), are within the tolerance, measured by the independent checker against the exact faces, never the tessellation; a strategy that keeps the tessellation names it in its notes; identical G-code at 1, 2 and N threads

   - **Section 4.1, two rows:** "Tessellation error on STEP faces | IMP-06, OP-23 | 0.005 mm, measured on the tessellation (vertices, edge midpoints and a grid of each triangle) against the exact face, never taken from the kernel's setting" and "Exact-surface generation time | OP-23, NFR-04 | within 2 times the same job on the import's tessellation, median of 5 runs, all threads; over that, the owner decides before it becomes the default (OI-835)".
   - **IMP-06's row** gains "and, for toolpaths, the deviation that M1 measures at the setting in use".

10. **Wording, and the warning before Generate (the owner's request with decision 1, 21 Oct 2026; OI-836).**
   1. **One check, two places.** The check that refuses a selection set on a STEP part is moved out of generation (`brep_selection`) into a function both callers use:
      - the operation inspector's Geometry section shows its answer under the role that holds the set, as soon as the set is added;
      - generation keeps it as the fallback (NFR-07).

      Since both read the same function, they never disagree. Before anything is generated, the operation is in the error state, with the message in Messages and a Show link, as decision 0174 item 5 does for a strategy on an STL part.
   2. **In the Geometry section.** Under the set, an amber Warning line with the triangle outline, as 11.7 of the interface specification does for a set that reaches a tab: "*Strategy label* can't use the set '*set*' here: *reason*." The *reason* and the fix are worded per case (below). Generate stays enabled for the other operations. This one is skipped with its error, as an operation in error is today.
   3. **Wording rules.**
      - Strategies are named by their label (Helical finishing), never by their internal name (`helical_finishing`).
      - No requirement IDs or decision numbers in what the user reads (decision 0114, plain wording).
      - Each message says what the user can do.
      - A refusal that is the same on an STL part does not say "STEP".
   4. **The messages after STEPALL-F** (R1 to R3 are gone for the five strategies that take regions). Each still starts "operation '*name*' cannot be generated: " when it reaches generation:

      | Case | Text |
      | --- | --- |
      | A face set as the Machine selection on rest machining, pencil clean-up or edge following (they take no region on any part) | *Strategy label* works on the whole part and cannot be limited to chosen faces. Take the set '*set*' out of the operation. |
      | A boundary, avoid or check set on rest machining, pencil clean-up, edge following or radial drilling | *Strategy label* does not use a boundary, avoid or check set. Take the set '*set*' out of the operation. |
      | An edge set on any strategy but edge following (R4) | Only edge following follows chosen edges. Take the edge set '*set*' out of the operation, or use edge following. |
      | A triangle set kept after the part became a STEP file (R5) | The set '*set*' selects triangles, but this setup's part is now a STEP file, which is selected by its faces and edges. Select the faces again on the STEP part. |

   5. **Which lane.** **STEPALL-F**, not OPCREATE or CUTOPTS. The check, the messages and the face-set mapping are the same function in `operation.rs`, and F removes R1 to R3 for five strategies. A separate earlier fix would reword messages F then deletes, and putting it in OPCREATE or CUTOPTS would bring a second lane into the same function. The owner confirmed on 21 Oct 2026 (relayed) that STEPALL-F carries it, with no separate earlier lane.
   6. **Interface specification.** The Geometry line of the operation inspector (`ui-specification.md`, the Geometry item of the operation inspector) gains: "A set the strategy cannot use on this part shows an amber Warning line under its role, with the reason and what to do, before anything is generated." It is edited when this record is accepted, by the lane that builds it.

## Build lanes

Sizes: **S** is part of one lane session, **M** is one lane session and one pull request, **L** is more than one session or needs its measurement first. Each lane takes its decision, `OI-` and schema numbers from the coordinator, as the work breakdown's *Parallel work* says. "Touches" is the first guess at the files, which the lane confirms before it starts.

| Lane | Scope | Size | Needs | Touches | Stays out of |
| --- | --- | --- | --- | --- | --- |
| STEPALL-M | Item 4.1: measurement program, M1 to M4 on the blisk and the corpus, numbers into this record; `blisk.step` into `testdata/step/` (OI-832) | M | Nothing (the owner agreed to copying `blisk.step`, OI-832) | `crates/openchuck-app/examples/` (a new `step_accuracy.rs`, `kernel`), `crates/openchuck-testkit` (the checker's sampling of exact faces, if it needs a finer grid), `testdata/step/`, docs | Every strategy file; changes no output |
| STEPALL-F | Item 3: face sets as region, boundary, avoid and check sets and a stock to leave per face set for roughing, helical, axial, indexed and simultaneous (closes OI-233); notes naming the drop surface and the import notice (item 4.4, OI-830); the refusals that remain (edge sets on other strategies, options on other strategies) shown in the Geometry section before generation and worded with the strategy's label (item 10, OI-836); OP-23 and its verification row (item 9) | M | Nothing on `main` (decision 0176 merged); merges after CUTOPTS and OPCREATE if they touch `operation.rs` | `crates/openchuck-app` (`operation.rs`: `brep_selection`, the region build, the stock-to-leave check; tests), `crates/openchuck-import` (`step_tessellation_notice` wording), `docs/requirements.md` and `docs/verification-plan.md` on acceptance | `openchuck-path` strategy files (roughing is CUTOPTS's and OPCREATE's); swarf's options (OI-637, SWARF3); mesh sets on STEP (OI-634, SWARF3) |
| STEPALL-X | Item 4.2: exact ball drop, then the other profiles; the region test on exact faces (OI-833); helical, axial, indexed, rest, then simultaneous; roughing only if M shows a gain; golden files of STEP jobs re-blessed; 4.1's time limit checked (OI-835); closes OI-81 and, with M1, OI-76 | L | STEPALL-M and STEPALL-F merged | `crates/openchuck-path` (`brep.rs` `drop_tool`, `cutter.rs` if the profile test is shared, `region.rs` if the test takes the drop's accuracy), `crates/openchuck-app` (`operation.rs` region surfaces), kernel tests (`tests/step_strategies.rs`) | Strategy parameters and creation (CUTOPTS, OPCREATE); `wrap_fit` and `simultaneous` fitters, except through the surface |
| STEPALL-P | Pencil clean-up on concave B-rep edges (item 2), with the corpus part of OI-234 (b) | M | STEPALL-F merged (its notes); OI-234 (b) part, which this lane may write | `crates/openchuck-path` (`pencil.rs`: the crease source; `brep_edges.rs`: concave classification), `testdata/step/generate`, tests | `brep.rs` (X) |
| STEPALL-E | Edge following on the exact edge curve (OI-229) | S | Nothing | `crates/openchuck-path` (`brep_edges.rs`, `edge_following.rs`) | `pencil.rs` (P), `brep.rs` (X) |
| (option) STEPALL-T | Item 4.3: a toolpath tessellation setting, if the owner chooses form (c) over or before X (OI-834, OI-831) | M | STEPALL-M; a schema claim if stored | `crates/openchuck-app` (`model.rs` placed part), `crates/openchuck-project` (if stored), the operation or project inspector | `brep.rs` (X) |

Lanes M and F can start at once and run side by side. P and E can run beside X. None of these lanes touches the files of lanes CUTOPTS, OPCREATE or SWARF3 as far as this proposal can see. Where a file is shared (`operation.rs`), the later lane merges `main` first, as decision 0055 orders.

## Alternatives considered

- **Exact contact for every drop-based strategy at once, without measuring.** It is what "STEP support" sounds like. It is rejected because the issue asks for the gain to be measured, and roughing at 0.3 to 0.5 mm of stock almost certainly gains nothing for the cost.
- **Face sets built as exact `BrepSurface::with_faces` regions now (as swarf's avoid and check sets are).** With drops still on the tessellation, the region test at 1e-4 mm would refuse contacts up to 0.01 mm off the exact face, leaving a cut full of gaps. A wider tolerance in the test would change it for meshes too. Triangles of the faces match today's drops, so this form waits for X.
- **One lane for all of #257.** A single pull request touching the app, the path library and the corpus would conflict with CUTOPTS, OPCREATE and SWARF3 and take weeks. Split by kind of support, each lane is one pull request with one measurable result.
- **A kernel-backed drop** (OpenCASCADE's distance queries through the worker). It is rejected by decision 0017 items 2 and 5 and AGENTS.md: the kernel stays in `openchuck-import`, and it is 7 to 74 times slower on the queries SP-9 measured.
- **Only a finer tessellation, no exact drop.** It may be enough (item 4.3), but the normals stay facet normals (OI-76), the contact length per move still depends on the facets (OI-77), and the triangle count grows as the deflection shrinks. It is kept as option (c) for the owner, decided on M's numbers.

## Measurements

None yet. Lane STEPALL-M fills this section (item 4.1) before STEPALL-X starts. The figures in *Context* are from decisions 0017, 0027, 0056 and 0115 and OI-146, measured on other parts.

## Consequences

- **Documents, in this change:**
  - this record;
  - the decision index;
  - `docs/open-items.md`: OI-830 to OI-836 added, and OI-233's row noting this record as its proposal;
  - `docs/work-breakdown.md`: lane STEPALL in *Parallel work*, proposal only, with its sub-lanes queued.
- **Documents on acceptance:** OP-23 and the verification rows of item 9, added by STEPALL-F with its tests; the import notice's wording; the interface specification's Geometry line (item 10.6).
- **Code:** none in this change. The lanes above name theirs.
- **Library API:** STEPALL-F changes none. STEPALL-X may add to `BrepSurface` (a per-face deflection bound), and if it changes a public item it bumps the workspace minor version with a changelog entry (decision 0048).
- **Open items:**
  - OI-830: the import notice overstates exactness while drops use the tessellation.
  - OI-831: no tessellation setting in the application (IMP-06).
  - OI-832: `blisk.step` into `testdata/` needed the owner's agreement; given on 21 Oct 2026 and closed in this change.
  - OI-833: the region test once contacts are exact.
  - OI-834: a toolpath tessellation setting as an alternative or complement to X.
  - OI-835: the 2-times time limit, and what happens above it.
  - OI-836: STEP refusals appear only at generation and name the strategy by its internal name.
