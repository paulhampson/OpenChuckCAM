# 0525: Face-flow finishing, passes parallel to a guide edge of selected STEP faces (OP-28; issue #512; lane FACEFLOW)

- **Status:** accepted. The owner approved the requirement and the four points of item 2 on 2 Nov 2026 in the originating session, relayed by the orchestrator (decision 0049). Proposed by lane FACEFLOW the same day. Written as 0645; the number moved from 0645 to 0525 at the merge (6 Nov 2026), assigned by the Merge Manager as the next free number after main's highest (0523; #554 holds 0524).
- **Date:** 2 Nov 2026.
- **Issue:** #512.
- **Builds on:** [0174](0174-step-only-strategies-blocked-on-a-mesh-part.md) (a strategy that needs a STEP part is blocked on a mesh), [0210](0210-step-face-sets-for-region-strategies.md) (face sets on a STEP part, OP-23), [0053](0053-slice-16-edge-following-brep.md) and [0227](0227-edge-following-on-the-exact-edge-curve.md) (B-rep edges and their exact curves), [0017](0017-sp-9-exact-surfaces.md) (exact surfaces in the library), [0036](0036-slice-13-simultaneous-finishing.md) (the tool-axis policy and the smoothing of A), [0368](0368-simultaneous-finishing-of-hidden-surfaces.md) (unreached regions reported, OP-20), [0048](0048-pre-1-0-api-changes.md) (library API before 1.0).
- **Decisions on the build:** D1 to D4 below were confirmed by the project owner on 6 Nov 2026, relayed by the coordinator (decision 0049); they close OI-2005, OI-2006, OI-2008 and OI-2009 and raise OI-2050; pull request A raises OI-2051.
- **Open items:** raises OI-2004 to OI-2009 ([open-items.md](../open-items.md)). Depends on OI-81 (exact tool-centre solving on parametric faces) for the accuracy of the stepover and scallop measured on the exact face.

Numbering: on 2 Nov 2026 `main` ended at OP-26 and record 0497; unmerged branches held OP-27, records 0498 and 0636 to 0643, and RESTCORNERS had reserved 0644; the highest `OI-` number on any branch was OI-2002. This lane took OP-28, record 0645 (renumbered 0525 at the merge, see the Status line) and OI-2004 to OI-2009 (OI-2003 left free as the coordinator advised; still free when checked at the merge). No project schema number (item 6).

## Context

The owner wants a strategy where the user selects a STEP face and the passes run parallel to one of its edges, for surfaces such as a blisk's blade floor. Nothing in the documents covers it:

- Edge following (OP-08, decisions 0053, 0056, 0227) cuts along edges only, not across a face.
- Face sets (OP-23, decision 0210) only limit where another strategy cuts; the passes keep that strategy's own pattern (helical, axial and so on), which follows the machine's axes.
- OP-19 (geodesic-parallel and spiral patterns) is P2 and unbuilt, and is defined for a whole finishing operation, not from a chosen edge.

Requirements: OP-28 (new), OP-02, OP-05, OP-06, OP-12, OP-14, OP-15, OP-16, OP-19, OP-20, OP-23, OP-25, GC-07, NFR-06, NFR-07, NFR-12. Architecture: section 4 (crate responsibilities; strategies in `openchuck-path`, the STEP kernel in `openchuck-import` only) and the dependency rules. No architecture change.

## Decision

1. **The requirement.** OP-28 is added to `docs/requirements.md` at P1 with the owner's wording, and its row to the verification plan (unit, system and reference job tests). The owner approved both edits.
2. **The four points the owner took as proposed.**
   1. **Guide edge.** The default guide is the longest boundary edge chain of the selection, by length along the exact edge curves (decision 0227). The user can override it with a chosen boundary edge or edge chain. The tie-break and what counts as one chain are OI-2005.
   2. **Blend.** Where the opposite boundary differs from the guide in length or shape, each pass is a linear blend by arc-length fraction between the two boundaries: the point at fraction *s* along pass *k* lies between the guide's point at fraction *s* and the opposite boundary's point at fraction *s*, at the pass's blend weight, taken onto the face. The blend weights are chosen so that the spacing between neighbouring passes, measured on the face, is no wider than the stepover anywhere (or the scallop between them no higher than the set height). How the opposite boundary is found on a face that is not four-sided, and what happens where a boundary collapses to a point, are OI-2006.
   3. **OP-19 stays P2 and unchanged.** Face-flow finishing shares OP-19's scallop-height stepover control but is its own strategy; it does not build OP-19.
   4. **Cutter reach on narrow floors** (the tool cannot reach a blade-to-blade floor narrower than the cutter without touching a wall) is OI-2004. It is reported as an unreachable stretch (OP-28 point 4) and becomes a spike only if the Blisk reference job shows the reports leave the floor unusably uncut.
3. **Where it goes.** A new strategy in `openchuck-path`, behind the common `Strategy` interface (OP-16), registered like the others. It reads faces and edges through `BrepModel` and the exact-surface evaluators of decision 0017; the STEP kernel stays in `openchuck-import`. It declares `needs_brep_part` with a reason, so on a mesh part it is shown disabled with that reason and adding it is refused (decision 0174).
4. **Tool axis, protection and reports.** The tool axis uses the OP-06 policy (lead and tilt limits) with A varying along each pass (OP-05) under the A-axis rules of OP-15, as simultaneous finishing does (decision 0036). Every move is checked against the part, including the faces beside the selection, and against the operation's avoid and check sets (OP-12, OP-02). A stretch that cannot be cut without gouging or leaving the machine's limits is left out and reported with where it is and why, through the strategy output's unreached list (decision 0368), never cut silently (NFR-07).
5. **Order and linking.** Passes are linked by the shared services (OP-14) and directed by OP-25. A selection of several faces is cut face by face in an order that follows their shared edges, so neighbouring faces are joined without a retract where the link is clear. Whether a pass continues across a shared edge as one curve is OI-2008.
6. **Settings and storage.** The strategy's settings (stepover or scallop, lead, tilt, direction) are operation parameters, a free map (decision 0008 item 6), so they need no project schema number. The guide override refers to a B-rep edge selection set (schema 12, decision 0066), so no new stored kind is expected; if the implementation finds otherwise it claims a schema number first (work breakdown, "Project schema ordering"). The interface for picking the guide is OI-2009.
7. **Determinism.** Passes, their order and the G-code are byte-identical at 1, 2 and N threads (GC-07, NFR-06).
8. **System test.** The Blisk reference job in `docs/reference-jobs.md` uses the STL (`testdata/stl/blisk.stl`); OP-28 needs exact faces, so the lane adds a face-flow job on the STEP blisk already in the corpus (`testdata/step/valid/blisk.step`), cutting a blade floor. Its definition, and the tolerance it is checked to while OI-81 stands, are OI-2007.
9. **NFR-12.** Nothing in the strategy's wording says the program is safe or checked; generated G-code remains the user's to verify before running on a machine.

## Decisions on the build (6 Nov 2026)

Confirmed by the project owner on 6 Nov 2026, relayed by the coordinator (decision 0049), on lane FACEFLOW's build plan:

1. **D1, the cutting direction (option 1 of 3).** OP-28 point 5 asks for the direction OP-25 sets, but OP-25 is P2 and has no shared code: the lane flagged the conflict rather than pick a side. Face-flow finishing takes its own `direction` choice, Meander (the default), Climb or Conventional, as swarf and axial finishing have their own, and moves onto OP-25's shared choice when that exists; its description says so. Climb with `M3` puts the uncut material, the side of the next pass, on the tool's right looking along the feed from the spindle (decision 0213). Options 2 (build OP-25's shared helper in this lane) and 3 (meander only) were not chosen.
2. **D2, the guide, the opposite boundary and several faces (closes OI-2005, OI-2006 and OI-2008).**
   - The outer boundary is the face's longest loop of edges, joined end to end at their vertices (by position within 0.001 mm where an edge has none). It is cut into sides where it turns by more than 30 degrees (`face_flow::CORNER_ANGLE`); a loop with fewer than two such corners is cut at its two points furthest apart.
   - The guide is the longest side, measured along the edges' exact curves (decision 0227), the side with the lowest edge ID winning a tie within one part in a million.
   - The opposite boundary is the loop without the guide and the two sides next to it (a four-sided face: the side that shares no vertex with the guide). With two sides left it is the point where they meet: the passes close on it, and those nearest it that would be shorter than the tool's diameter are left out, with a note. With one side left (a two-sided face) it is that side.
   - Several faces: each face is its own field with its own guide; the face with the longest guide is cut first (the lowest face ID on a tie), then a face sharing an edge with the face just cut, then one sharing an edge with any face cut, lowest ID first. No pass crosses from one face to the next.
3. **D3, the user's guide (no schema change).** The user's guide is an optional B-rep edge set in the operation's selections, beside its face sets, as edge following stores its edges. The library takes the edges (`FaceFlow::on_brep(model, faces, guide)`); edges that bound a selected face make its guide and must be joined end to end on its boundary, and an edge on no selected face is refused by name. The application wiring is pull request B.
4. **D4, the interface (closes OI-2009).** The Geometry section lists the faces and an optional **Guide edge** set (empty means the longest side); the Results note names the guide edges each face was cut from (the library's note: "face N is cut from its guide, edge E (L mm long), towards the opposite side in K passes ..."). A viewport overlay that draws the guide is later: OI-2050. The UI specification entry comes with pull request B.

### Built in pull request A (the library)

- `crates/openchuck-path/src/face_flow.rs`, behind the common interface (OP-16): needs a B-rep part (decision 0174), honours a region. The passes are the blend of item 2.2; the spacing is measured along the face at 49 places along the guide, each step no wider than the stepover and, with a scallop height and a ball or bull-nose, leaving no higher cusp, then evened out. A scallop height with any other tool is ignored with a warning, as for the other finishing strategies (`scallop::warning`), not refused as the plan first said.
- The tool axis is simultaneous finishing's policy (decision 0036): the normal's direction across the rotary axis, leaned by the lead angle times the share of the travel that goes round the axis, then smoothed along the pass by a taut string within the tilt of that. The tool is placed touching the pass point and dropped along its axis onto the whole part with the exact ball drop (decision 0277): a lift of more than the tolerance means a neighbouring face is in the way, and the stretch is left out and reported as unreached (too narrow where the face is narrower across than the tool, blocked otherwise), joined into areas across neighbouring passes. Each move is checked at its middle and quarter points and split where it leaves the face by more than the tolerance; one that still cuts in after eight halvings is broken. Avoid and check sets (OP-02) leave stretches out without reporting them as unreached.
- It always drops the tool on the exact faces, so it offers no Exact STEP surface contact setting (OP-27, decision 0526): `Strategy::exact_contact` is `None`. Whether OP-27 should name such strategies is OI-2051 (proposed).
- Not yet in `Registry::builtin`: a host builds the strategy with its selection, and the application wiring, the guide set, the interface and the STEP blisk reference job (OI-2007) are pull request B.

## Alternatives considered

- **Extend edge following (OP-08) to offset its edge across the face.** Rejected: edge following's passes sit on edges with a tool-side offset; stepping across a face with blending, scallop control and a tool axis that leans with the surface is a different strategy, and mixing them would complicate an accepted one.
- **Build OP-19 (geodesic parallel) first and seed it from an edge.** Rejected by the owner (point 3): OP-19 is P2, and a true geodesic offset diverges from the opposite boundary on a floor whose boundaries are not parallel, which is the blisk's case; the blend keeps the passes following both boundaries.
- **Pass curves at constant face parameter (iso-parametric).** Not chosen: a face's parameter lines need not run along the chosen edge, and a trimmed face's parameters say nothing about its boundary. The arc-length blend works from the boundaries the user sees.

## Documents

- `docs/requirements.md`: OP-28 added (owner's approval). No other change.
- `docs/verification-plan.md`: the OP-28 row.
- `docs/work-breakdown.md`: lane FACEFLOW (needs, tasks, exit criteria); OP-28 listed with slice 20, closing with it, as OP-20 and OP-26 are. No spike row (item 2.4).
- `docs/open-items.md`: OI-2004 to OI-2009.
- UI specification: not changed here; the form and the guide pick are proposed with OI-2009 when the lane reaches them.
- Architecture: no change.

## Consequences

- **Library:** a new strategy module and public parameter constants in `openchuck-path`, an addition; a version bump only if the lane changes an existing public type (decision 0048).
- **Application:** registration and the `needs_brep_part` reason; the form; the guide override as an edge set reference.
- **Tests,** named by requirement ID (`req_op_28_...`) and written before the code: on a ruled face and a cylinder the passes lie on the face, run parallel to the guide at the chosen stepover and meet the scallop height; on the STEP blisk's blade floor the passes follow the floor's boundaries, never gouge the blade walls beyond tolerance, and unreachable stretches are reported; the mesh block; byte-identical output at 1, 2 and N threads.
