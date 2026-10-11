# 0012: SP-13 result, one revolved-profile tool model

- **Status:** accepted by the project owner (11 Oct 2026), including the two proposals that needed agreement: the monotone rule that excludes undercut tools (item 2, OI-57) and the TOML text format for custom profiles (item 6, OI-58). The spike met pass criteria 1, 2 and 4; criterion 3 (sweep time within the SP-6 budget) cannot be judged until SP-6 sets that budget, and is carried to SP-6 (OI-56, still open).
- **Date:** 11 Oct 2026.
- **Spike branch:** `spike/sp-13-tool-profiles` (throwaway, never merged). The code is in `spikes/sp-13-tool-profiles/`, a separate Cargo project with no third-party dependencies, never a workspace member; the write-up is `spikes/sp-13-tool-profiles/RESULT.md` and the full report `spikes/sp-13-tool-profiles/out/report.md`, both on that branch.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-07 progressed; OI-56 to OI-58 are new.

Numbering: record 0011 and OI-53 to OI-55 are claimed on the unmerged slice 3 branch (`claude/slice-3-machine-setup`), so this record takes 0012 and OI-56 to OI-58.

## Context

SP-13 (work breakdown, Spikes, 3 days) asks: can one revolved-profile tool model drive tool-centre solving, the independent gouge checker and the simulator sweep for flat, ball, bull-nose, tapered ball, V-bit, chamfer, drill and custom-profile cutters? Pass criteria: flat, ball and V-bit results match closed-form values within tolerance; a custom profile agrees with an equivalent standard tool; sweep time within the SP-6 budget; identical results at 1 and N threads. Named fallback: limit custom profiles to line and arc segments with a validity check, or exclude them from simultaneous strategies (OP-05) with a clear warning. It runs before slices 4 and 5 (and feeds slices 7 and 13).

Requirements: TL-02, TL-03, TL-04 (tool parameters), OP-05 and OP-12 (gouge), SIM-03 (sweep), GC-07 and NFR-06 (determinism), NFR-05 (tolerance), NFR-07 (invalid input gives an error). Architecture section 5.1 ("Tool and stock as data": one revolved profile of line and arc segments that path solving, the gouge checker and the simulation sweep share), section 5.2 (`SurfaceSource`, `StockModel`), section 9 (the independent gouge checker, "different code from the strategies"). Verification plan section 4 (the TL-03 row) and 4.1 (0.01 mm toolpath deviation, 0.005 mm of it for tessellation; zero gouge beyond tolerance). Decision 0001 item 7 (OI-07: off-top contact and cutter shape).

The profile *data* already exists: slice 0 put `ToolProfile` (ordered `Line { to }` and `Arc { to, centre }` segments, starting on the axis at the tip) and `ToolShape` in `openchuck-types/src/tool.rs`, and left the mapping to SP-13 and slice 4.

## Method

The spike uses the same input form as `ToolProfile` and builds, behind it:

1. **A validated profile** with one query for the solvers, the lower envelope `h(r)` (lowest point of the tool at distance `r` from its axis), its pieces in radius order, a convexity flag, and for convex profiles the profile point with a given outward normal.
2. **Standard types mapped to the profile** (flat, ball, bull-nose, tapered ball, V-bit, chamfer, drill), each with the impossible-value checks the slice 4 checklist names.
3. **Drop-cutter tool-centre solving** against a triangle mesh, generic over the profile: vertex, facet and edge tests that ask only for `h(r)` and its pieces. The tool axis is +Z; for 4-axis the part is rotated by A first, so an off-centre tool gives off-top contact. It returns the tip height and the contact point.
4. **Normal-offset solving** (surface point and normal to tip) for convex profiles, as a cross-check and a fast path.
5. **An independent gouge checker**: it samples the exact analytic surface (not the mesh), classifies each point against the closed revolved outline by a crossing count and measures depth as the distance to that outline. It shares the profile data and nothing else.
6. **Two stock stand-ins swept by the profile**: a dexel Z-map (tool axis along the dexels) and a rotary grid (material radius per X and part angle, with A rotation). SP-6 has not run, so these are the simplest forms of two of its candidates, not SP-6's implementations.
7. **Test parts**: cylinder (r 20), cone (r 10 at x 0, half angles 20 and 60 degrees) and sphere (r 15) on the A axis, tessellated with vertices on the surface at 0.0005 mm chord, so the mesh is inscribed and its own error is known. Tools 6 mm diameter, 12 mm flute.

14 tests (named after the requirements) and a report binary; the report's numbers are in `RESULT.md` on the spike branch.

## Results

| Criterion | Result |
| --- | --- |
| 1. Flat, ball and V-bit match closed form | Pass. On 1681 positions per tool on the sphere and the cylinder, and 41 along the cone, the dropped tip is never above the exact value and is below it by at most the mesh's own error: 0.00050 mm along the normal on the cylinder (chord 0.0005), 0.00099 mm on the sphere (whose latitude-longitude triangles sit up to twice the edge chord inside), and 2e-15 on the cone's top line, which the mesh holds exactly. Bull-nose and chamfer (flat-tipped V) match their closed forms too. All are inside the 0.005 mm tessellation budget (verification plan 4.1). |
| 2. Custom profile agrees with the equivalent standard tool | Pass. A ball typed as two arcs, a flat with split bottom and side, a V with a split flank, and a bull-nose and tapered ball typed in by hand give bit-identical drops to the standard tools on all three parts (169 positions each), and identical sweeps. A non-convex custom profile (flat, concave arc, side) agrees with a brute-force search of the exact surface within the mesh error. |
| 3. Sweep time within the SP-6 budget | Not assessed: SP-6 has not set a budget. On the stand-ins, one thread: the dexel Z-map takes 16 to 21 ns per cell and pose for every tool type, the rotary grid 23 ns (flat) to 101 ns (ball), the difference being the ray-entry search. Carried to SP-6 (OI-56). |
| 4. Identical results at 1 and N threads | Pass. Drop-cutter grids and both sweeps are bit-identical at 1, 2 and 4 threads (fixed chunks, results in input order, each item computed alone). |
| Gouge checker (OP-12) | At every dropped pose, for all eight tool types on the sphere and the cylinder, the worst gouge against the exact surface is within the mesh error (at most 0.00098 mm). With the tip lowered 0.05 mm, it reports between 0.030 and 0.047 mm for every tool (depth is measured along the outline's normal, so a flank contact reads less than the vertical fault). |
| Validation | Custom profiles are rejected with a message naming the segment: a profile that turns back (self-intersecting or undercut), a downward step, an arc whose ends are not on one circle, an arc beyond a quarter turn, a first segment up the axis, zero length, empty, not finite. Standard types reject a corner radius above the tool radius, an included angle of 0 or 180 degrees, a cutting end taller than the flute length, and a tapered ball with no taper inside the diameter. |

Findings:

- **One query is enough for the solvers and the sweep.** Drop-cutter, the dexel sweep and the rotary-grid sweep all need only `h(r)`; the drop-cutter's facet and edge tests also use the piece boundaries and whether each piece is convex in `r` (lines and convex arcs are, concave arcs are not). Normal-offset solving needs the profile point for a normal, which is unique only on a convex profile.
- **Non-convex custom profiles need no exclusion from OP-05.** The drop-cutter handles them: on concave-arc pieces the edge test samples then refines instead of using golden-section search. The normal-offset fast path refuses them, so a strategy that uses it must fall back to the drop for such a tool.
- **Off-top contact (OI-07, cutter shape half)**: the drop returns the contact point, which is where OP-13's contact feed applies, for any profile and any tool offset from the A axis. Tool axis variation is not part of this spike; it stays with SP-4 and slice 13.
- **Mesh error on tilted surfaces.** A vertical drop onto an inscribed mesh errs by up to the normal error divided by the normal's Z component (0.0013 mm vertically on the sphere for 0.00099 mm along the normal). Slice 5's deviation check should measure along the surface normal, as the 0.01 mm budget intends.
- **Precision**: the edge test first computed horizontal distance from an expanded quadratic, which lost about 1e-8 mm to cancellation near the tool axis and put a sharp V's apex 2e-9 mm *above* the surface; evaluating the distance directly fixed it. The product should evaluate distances directly.
- **Cost**: `h(r)` takes 5 to 8 ns. One drop on a 147,070-triangle sphere with a plain XY grid index takes 0.35 ms (flat) to 3.2 ms (tapered ball) on one thread, after a height cull (triangles visited from the highest down, stopping when none can lift the tool, valid because `h(r) >= 0`) brought it down from about 12 ms. It was not profiled; the edge tests' golden-section searches are the likely cost. That is slow for a dense finishing pass; slice 5 should add a bounding structure (SP-4's choice) and cull edges by their own height before searching. Normal-offset solving takes about 0.1 microseconds. The dexel stand-in sweeps at 16 to 21 ns per cell and pose for every tool, the rotary-grid stand-in at 23 to 101 ns, all on one thread; 4 threads take 2 to 3 times less.

## Decision (accepted)

1. **Keep `ToolProfile` as the one tool description.** Its input form (segments from the tip, on the axis, each running from the previous point; arcs by end point and centre) needs no direction flag under the rule in item 2. Every tool type maps to it, and path solving, the gouge checker and the sweep read only the profile.
2. **Validity rule: monotone profiles.** Along the profile neither the radius nor the height decreases; each segment has non-zero length; an arc's ends lie on one circle within 1e-6 mm and the arc stays within one quarter turn (radius and height both increasing along it); the first segment leaves the axis; every value is finite. This makes a profile connected, non-self-intersecting and revolvable by construction, makes `h(r)` a function, and fixes each arc's direction. It excludes undercut tools (dovetail, lollipop, keyseat, back chamfer); P1 accepts that if the owner agrees (OI-57). This is the fallback's "line and arc segments with a validity check", reached as the design rather than after a failure.
3. **Standard types**: the cutting end (flat, ball, corner, taper, V or point) runs from the tip to the cutting radius D/2, then a vertical side runs to the flute length; a cutting end taller than the flute length is rejected. A tapered ball's diameter is where the taper ends. A drill is a sharp V of its point angle, and a chamfer mill is a V with a tip diameter (the same geometry as a V-bit; both types stay for naming). The silhouettes match their closed forms to 1e-12 mm.
4. **Tool-centre solving is the drop-cutter, generic over the profile**, with the vertex, facet and edge tests above, returning the tip and the contact point. Normal-offset solving is a fast path for convex profiles only, never the sole check. No tool type is excluded from OP-05.
5. **The profile's interface to the solvers** is `h(r)`, its pieces in radius order with a per-piece convexity flag, the profile's convexity, and (convex only) the point for a normal. Slice 4 puts the profile and validation in `openchuck-types`; slice 5 builds the drop on it; slice 7 sweeps with it.
6. **Custom profile text file (proposal for slice 4)**: TOML with the `ToolProfile` fields, read through `toml` (already a dependency, and the format decision 0006 chose for hand-edited libraries). An arc may give `radius` and a side instead of `centre`, so a hand-typed arc is not rejected for a rounding difference in its centre (OI-58).
7. **The gouge checker stays independent** as built here: exact-surface sampling, a crossing count in the profile half-plane, and distance to the outline. It shares only the profile data.

## Alternatives considered

- **A general self-intersection test instead of the monotone rule**: it would admit undercut tools, but then `h(r)` is not a function, drop-cutter misses contacts above the lowest point at a radius, and arc direction needs a flag. Rejected for P1; reopen with OI-57 if undercut tools are wanted.
- **Normal-offset solving as the main method**: rejected; it is local and only defined for convex profiles. Kept as a fast path.
- **Excluding custom profiles from OP-05 (the second fallback)**: not needed; the generic drop handles them.
- **Per-type closed-form solvers (as in OpenCAMLib)**: rejected; one generic solver over the profile pieces matched the closed forms, and per-type code would have to be repeated for custom profiles.

## Consequences

- No change to `requirements.md` or `architecture.md`: architecture section 5.1 already names one revolved profile shared by the solver, the gouge checker and the sweep, and TL-03's verification row already asks for validation and agreement with an equivalent standard tool. The monotone rule narrows what TL-03's "custom profile" admits; that is for the owner to accept (OI-57), not a silent change.
- **Slice 4**: items 1 to 3 take decision items 1 to 3 and 6. `Tool::validate` for `ToolShape::Custom` checks only finiteness today; slice 4 replaces it with the rule in item 2, and adds the TL-04 fields `Tool` lacks (flute count, overall length, material, tool number). The text parser gets corpus tests and a fuzz target (NFR-07).
- **Slice 5**: the drop-cutter of item 4 inside `openchuck-path`, on the `SurfaceSource` of SP-4; deviation measured along the surface normal; distances evaluated directly.
- **Slice 7 and SP-6**: SP-6 sweeps the profile in its candidates (its row already includes a V-bit and a custom profile) and sets the budget criterion 3 is judged against (OI-56). The rotary-grid stand-in finds where a ray enters the tool by stepping and bisection; a closed-form entry per piece (a quadratic for line pieces, a quartic or bisection for arcs) is the obvious improvement to time there.
- **Slice 13**: tool-centre solving for every profile as in item 4.
- **OI-07** progresses: the cutter-shape half (contact offset from the tip, off-top contact) is answered by the drop; tool axis variation stays open for SP-4 and slice 13.
- No dependency enters the product; the spike uses only the standard library.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Sweep time against the SP-6 budget (criterion 3): SP-6 sets the budget and times the profile sweep in its candidates, including V-bit and custom profiles. (OI-56) | SP-6 | SP-6 |
| 2 | Undercut tools (dovetail, lollipop, keyseat, back chamfer) are rejected by the monotone rule. Accepted for P1 on 11 Oct 2026 (OI-57 closed); a general profile with a self-intersection test and a different drop would be a new decision. | Closed | |
| 3 | Custom profile text format: TOML with the `ToolProfile` fields, with an arc by `radius` as an alternative to `centre`. Accepted on 11 Oct 2026 (OI-58 closed); slice 4 fixes the field names and writes the corpus tests and fuzz target. | Closed (slice 4 specifies details) | |
