# 0176: Swarf honours boundary, avoid and check face sets (lane BUG-SWARF; issue #227)

- **Status:** proposed (21 Oct 2026). Every question is answered; the owner accepts the record directly in the lane's session (decision 0049). Owner answers, relayed by the coordinator on 21 Oct 2026:
  - **OI-631, decision 2:** option C. A boundary limits the pass along the face, across the rulings (limit A), and also limits the cut length along the rulings at the tool tip (limit B). The two compose, the notes say which one limited a cut, this record explains how B relates to the existing Tip extension, and the operation's note says plainly that the far end of a ruling cannot be limited.

  - **OI-635, whether A and B are settings:** yes, but not as two operation-wide parameters. They are tick boxes in the operation inspector's Geometry section, where the boundary, avoid and check sets are chosen, and they belong to each set in its role within the operation: each boundary, avoid and check set carries its own options. For the boundary they are A and B, both on by default. A set whose options leave it limiting nothing keeps the warning "the set 'X' limits nothing", and the options are stored in optional tables of the operation file, project schema 14, where older files take the defaults (item 3).
  - **OI-632, decision 3a:** an avoid or check set cuts the pass short by default (item 4).
  - **OI-632, decision 3b:** face sets only for now; mesh avoid and check sets on a STEP part stay OI-634 (item 5).
  - **The options of an avoid or check set**, as the lane proposed: one choice per set, shown beside it in the Geometry section: Cut short (the default), Warn only or Ignore (item 3).

  Only the owner accepts this record (decision 0049).
- **What it amends:**
  - **accepted [decision 0111](0111-swarf-through-the-application.md)**, which reads a STEP part's sets for swarf as its face set only, and refuses boundary, avoid and check sets on a STEP part;
  - **accepted [decision 0061](0061-slice-17-swarf.md) item 1**: swarf needs no containment because its faces are its input. The faces stay its input; the limits are new.
  - **accepted [decision 0056](0056-brep-path-through-the-pipeline.md) item 3**: boundary, avoid and check sets on a STEP part were refused for every strategy. They are now honoured for swarf, and still refused for the other strategies (OI-233 stays open for them).
  - **accepted [decision 0032](0032-slice-10-mesh-selection.md)** (schema 7): the operation file gains each set's options beside the `boundary`, `avoid` and `check` it already stores (project schema 14, item 3).
  - **The interface specification, section 8** (accepted with [decision 0038](0038-ui-specification-and-requirements.md)): the Geometry section shows the options under the sets of a strategy that takes them. The owner asked for this placement, so the line is edited in this change.

  OP-02 and OP-07 and the requirements are unchanged: OP-02 already asks for "an optional containment boundary and optional avoid or check surfaces" on every operation.
- **Date:** 21 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md):
  - OI-631, OI-632 and OI-635 are closed by the owner's answers;
  - OI-634 (mesh avoid and check sets on a STEP part), OI-636 (how finely an avoid or check face is sampled) and OI-637 (whether the other strategies take per-set options) are new;
  - OI-233 stays open for the other strategies.

Numbering: 0176 and OI-634 to OI-636 are the next free numbers across `main` and the unmerged branches on 21 Oct 2026 (lane BUG-STALE took OI-692 to OI-697 and lane BUG-INSP decision 0187 out of sequence). OI-637 and project schema 14 were assigned by the coordinator on 21 Oct 2026; no branch held a schema number above 13.

## Context

Trial finding F-71 (issue #227, P1): the owner asked that swarf operations take boundary, avoid and check selection sets, as the other strategies do (OP-02). Until now swarf on a STEP part took only its face set, the ruled walls it cuts ([decision 0111](0111-swarf-through-the-application.md) item 1). Any boundary, avoid or check set on a STEP part was refused before generation ([decision 0056](0056-brep-path-through-the-pipeline.md) item 3, OI-233).

For the other strategies, [decision 0032](0032-slice-10-mesh-selection.md) defines the roles on a mesh, against the cutter contact:
- a boundary is a window of X and angle about the rotary axis, and the contact must lie inside it;
- an avoid surface is never cut, but the tool may pass over it;
- a check surface is never touched.

A swarf flank is different: it touches the whole of a ruling at once, and deliberately rubs the face it cuts. So the roles need a meaning of their own for swarf.

Requirements: OP-02 (selections, boundary, avoid, check), OP-07 (swarf), OP-12 (gouge checking), NFR-05, NFR-06 and GC-07 (determinism), NFR-07 (clear errors), NFR-12 (the user verifies), PRJ-06 (selection storage). Records read: 0021, 0061, 0111, 0174, 0175 (swarf), 0032 (the roles and `RegionConstraint`), 0056 (a STEP part through the pipeline), 0066 (B-rep selection sets), 0048 (API changes before 1.0), 0161 (the held region, SET-09).

## Decision

1. **The sets.** A swarf operation on a STEP part takes, besides its face set (the walls it cuts):
   - one **boundary** set;
   - any number of **avoid** sets;
   - any number of **check** sets.

   Each must be a face set made on the setup's part as it is now. This is checked as for the face set: the same model, the same content hash, every face in the model, not empty. An edge set in one of these roles is refused, naming the set. A face that is both in the face set and in an avoid or check set is refused before anything is generated, naming the face and the set. A boundary may hold the cut walls.

   The sets are made and chosen as for every strategy: selection sets, in the operation inspector's Geometry section, which already lists the four roles (#225). The operation file already stores `boundary`, `avoid` and `check` (decision 0032, schema 7); each set's options are new (item 3, project schema 14). Other strategies on a STEP part still refuse these sets, now saying "built for swarf only so far" (OI-233).
2. **How a boundary limits a swarf cut (OI-631, the owner's option C).** The boundary is the window of its faces: the X interval and the arc of angle about the rotary axis that their tessellation spans (`Window::of_mesh`, the boundary of decision 0032). A point of the face is inside when it is in that window. Each ruling is tested at 33 points along it, and its edges are found by bisection.
   - **Limit A, the pass (across the rulings).** Only a ruling that lies inside the boundary along its whole length is cut, and it is cut whole. The pass stops, or splits with a retract, where the rulings leave the boundary. The run's end is found to the bisection's precision, as the face's own trim is. This is the meaning decision 0032 gives a boundary for the other strategies, applied to rulings instead of contact points.
   - **Limit B, the tip (along each ruling).** At the end of each ruling where the tool tip sits, the cut starts where the ruling enters the boundary. The tip is raised along the ruling so the bottom of the flank stops at the boundary; the side of the tool still covers the rest of the ruling.
   - **The far end of a ruling cannot be limited.** The side of the tool runs past it, up to the shank. The operation's note says so plainly whenever limit B is on: "The far end of the rulings is not limited by the boundary: the side of the tool cuts each ruling up to that end."
   - **How the two compose.**
     - **B then A.** With both on, the tip end of each ruling is moved first, and the ruling is kept only when what remains lies inside the boundary. So the pass is limited across the rulings after the tip is raised.
     - **B alone.** A ruling with no part inside the boundary is not cut at all, since the tip cannot be raised into it.
     - **A alone.** A ruling whose tip end is outside the boundary is dropped whole, and the pass is cut short there.
   - **B and Tip extension.** Tip extension moves the tip beyond the end of the ruling, along the tool axis. Limit B moves that end of the ruling itself. The two add: with B raising the end by 2 mm and a tip extension of 0.5 mm, the tip sits 1.5 mm above the old end. The tip end (the Tip end setting, or the automatic rule) is chosen on the whole face before any limit, so a boundary never turns the tool over.
   - **Measuring the cut.** The gate (decision 0021 item 3) measures the face only where it is cut. A part of a ruling below a raised tip is left on purpose. It is not reported as material left, and the note says how far the tip was raised.
   - **Notes say which limit acted.** Each face's note says:
     - with A, how much of the face's length is not cut because its rulings leave the boundary, or that every ruling lies inside;
     - with B, the most the tip was raised;
     - the far-end sentence above.
   - **A boundary that leaves nothing.** A face with nothing left to cut is refused, naming it: "none of its rulings lies inside the boundary set, so the boundary leaves nothing of it to cut".
3. **Each set's options (OI-635, the owner's decision, with the avoid and check choices the lane proposed).** The options belong to a set in its role within one operation, not to the operation and not to the set: the same set may be a boundary in one operation and an avoid set in another.
   - **Where they are shown.** In the operation inspector's Geometry section, under the role that holds the set, for a strategy that takes them (swarf, so far; OI-637). Another strategy shows none.
   - **A boundary set:** two tick boxes, both **on by default**, so the default is the owner's option C:
     - **"Limits the pass"** (limit A);
     - **"Limits the tip"** (limit B).

     Each one's tooltip says what it does.
   - **An avoid or check set:** one choice of three, so no combination means nothing:
     - **Cut short** (the default): the pass stops short of the set and starts again after it (item 4, decision 3a).
     - **Warn only:** the path is kept. The face's note and a warning (decision 0175) name the set, the faces, and the length along which the tool breaks it: "The tool cuts into the avoid set 'Rib' along about 3.000 mm of face 0. The set only warns, so the path is kept: set it to cut the pass short, or check the cut in the simulation before using the path." The set is measured as a cut-short set is (item 4), along what is cut.
     - **Ignore:** the set limits nothing.

     Two tick boxes ("cut short" and "warn") were not chosen: "cut short" off with "warn" off would be Ignore, and both on would mean nothing more than cut short.
   - **A set that limits nothing.** A boundary with both limits off, or an avoid or check set on Ignore, is left out of the path, which is made as if it were not there. A warning (decision 0175) names it, and is said first in the operation's notes too:

     > The boundary set 'Fence' limits nothing, because both Limits the pass and Limits the tip are off: the faces are cut as if there were no boundary. Turn one of them on beside the set in the operation inspector's Geometry section, or take the set out of the operation.

     > The avoid set 'Rib' limits nothing, because it is set to Ignore: the tool is not kept clear of it. Set it to Cut short or Warn only beside the set in the operation inspector's Geometry section, or take the set out of the operation.

     So the warning state, Messages and the checks before writing of decision 0175 apply. The warning names no operation parameter, so its Messages action shows the operation rather than a setting. The Geometry section also says "The set 'X' limits nothing." under the set.
   - **How they are stored (project schema 14).** In the operation file, beside `boundary`, `avoid` and `check`, three optional tables, each written only when an option differs from its default:

     ```toml
     boundary = "…"
     avoid = ["…"]
     check = ["…"]

     [boundary_options]
     limits_pass = true
     limits_tip = false

     [avoid_options."<set id>"]
     on_break = "warn"        # or "cut_short", "ignore"

     [check_options."<set id>"]
     on_break = "ignore"
     ```

     - An older file has none, so every set takes the defaults: today's behaviour. The migration from schema 13 changes only the number, as for schema 12, and a schema 13 file that already holds one of the tables is refused, naming it. No fixture folder.
     - Options for a set that is not in that role, boundary options with no boundary set, an unknown field and an unknown choice are refused on load, naming what is wrong (NFR-07).
     - Taking a set out of a role in the inspector drops its options, and choosing another boundary set, or none, puts the boundary's options back to the defaults. So a set put back starts from the defaults.
     - The options are part of the operation's selection inputs (decision 0032's cache key, UI-05): changing one makes the path again. Options at their defaults leave the key as it was, so no older operation is made again.
     - On another strategy the options are kept but not used, and the inspector does not show them (OI-637). A strategy that does not take them refuses a region that sets any (item 6), but the application never builds one for it.

4. **Avoid and check sets cut the pass short (OI-632 part a, the owner's decision 3a).** This is what the default choice of item 3, Cut short, does.
   - **How each surface is measured.** Each avoid or check surface is sampled, and the tool is measured against every sample, as the gate measures the cut face. This uses the same tool body: the revolved profile continued upward at the cutting radius.
   - **When a tool position breaks a surface.** A position breaks an avoid surface when the tool cuts into it deeper than the operation's tolerance. It breaks a check surface when it comes closer than the stock to leave, less the tolerance. A check surface therefore keeps the operation's stock to leave clear, as the cut face does, and is stricter than an avoid surface whenever there is stock to leave. With none, the two behave alike for swarf, since a flank that touches is a flank that cuts. The tolerance's slack lets a flank meet a neighbouring face along their shared edge, for example a floor at the foot of a wall.
   - **What is left out.** Every pose, and every point of the machine's motion between poses that the gate looks at, is tested.
     - **Poses.** Where the tool goes from clear to not, the last clear pose is found by bisection (16 steps), so the pass stops close to the surface.
     - **What is dropped.** A pose that breaks a surface is left out, and so is a move along which the tool breaks one.
     - **What the pass does.** The pass stops there and starts again after it with a retract. Pieces of fewer than two poses are dropped.
   - **What the note says.** The face's note gives the length along the face left uncut because of each kind of surface.
   - **A face with nothing left.** It is refused, naming it and why: "the tool would cut into the avoid surfaces wherever it cuts it, so nothing of it is left to cut".
   - **The other choices.** Keeping the path and warning (as decision 0175 does for an overcut) is not the default: the user marked these faces as protected. It is the Warn only choice of item 3, for a user who wants it. Refusing the whole face when any position breaks a set was not chosen: a wall that meets a check face at one end could never be cut.
5. **Face sets only, for now (OI-632 part b, the owner's decision 3b).** An avoid or check set on a STEP part is a face set, measured against the exact faces. A triangle set cannot be made on a STEP part today: decision 0111 item 2 picks triangles only on a mesh, and a set must be on the setup's part. Allowing mesh sets would mean picking triangles of a STEP part's tessellation, and they would be only as accurate as that tessellation. That is OI-634, for a later lane. Workholding needs no check set: the held region and tabs are kept clear by SET-09 (decision 0161) for every strategy, swarf included.
6. **How the library takes the limits (OP-16).**
   - **The strategy.** Swarf now honours a region (`honours_region` is true) and its restriction options (`honours_restriction_options` is true). It takes the `RegionConstraint` of decision 0032 through `generate_in_region`:
     - its `boundary` is the window, and its new `boundary_limits` (a new `BoundaryLimits`, `pass` and `tip`, both true by default) say how it limits swarf;
     - its `avoid` and `check` are surfaces, which the application builds from the cut-short face sets as `BrepSurface::with_faces`;
     - its new `watch` holds the Warn only sets, each a `WatchedSurface` with its name, whether it is an avoid or a check surface (a new `Protection`), and its surface;
     - a `region` (a selected region) is refused, since swarf's faces are its selection.
   - **Other strategies.** `Strategy::honours_restriction_options` is a new provided method, false by default. The shared generation refuses a region that turns a boundary limit off or watches a surface (`RegionConstraint::has_restriction_options`) for a strategy that does not honour them, rather than ignoring it.
   - **Where the warnings are made.** The library warns for watched surfaces the tool breaks, by name, and for a boundary with both limits off (without a name, for a host that passes one). The application leaves out a set that limits nothing and makes that warning itself, since it knows the set's name.
   - **The sampling.** An avoid or check face is sampled on a grid of its parameters. The samples are no further apart on the face than an eighth of the tool's cutting radius, at least 33 and at most 400 each way. A surface with no exact faces gives its tessellation's vertices.
   - **API changes.**
     - `swarf::face_path` takes the constraint.
     - `RegionConstraint` gains `boundary_limits` and `watch`; `BoundaryLimits`, `Protection` and `WatchedSurface` are new.
     - `swarf::FacePath` gains `limits` (a new `swarf::Limits`: which boundary limits applied, the length outside the boundary, the most the tip was raised, the lengths left uncut by avoid and check surfaces, and each watched surface the tool breaks, a new `swarf::WatchedBreak`).
     - The two swarf parameters `boundary_limits_pass` and `boundary_limits_tip` that this record first proposed are not added, and `swarf::Settings` is unchanged.

     These break the library API within the 0.13.0 cycle that decision 0175 opened, with changelog entries ([decision 0048](0048-pre-1-0-api-changes.md)).
7. **Deterministic.** The limits are computed from the same inputs in a fixed order, and the only parallel work (the guard tests and the gate) goes through `crate::parallel` in order. The same project gives the same G-code at any thread count (GC-07, NFR-06).

## Alternatives considered

- **Only limit A (the lane's first recommendation):** the owner chose both (option C).
- **A boundary as a limit on the tool's contact point, as for the other strategies:** a swarf contact is the middle of a ruling, so a boundary across a wall would cut half a ruling's worth of tool past it.
- **Fixed A and B, with no settings:** simpler, but a boundary drawn to stop the pass along a wall could also cut across the bottom of its rulings by accident. The tip would then be raised and material left at the foot of the wall, with no way round it but redrawing the boundary.
- **A and B as two operation parameters (the lane's first recommendation):** the owner chose options per set, in the Geometry section beside the sets. Operation parameters are name and value pairs for the whole operation, so per-set options need their own place in the file (schema 14).
- **The options in the selection set's file:** a set may be a boundary in one operation and an avoid set in another, so the options belong to the operation's reference to the set.
- **Avoid and check as a warning, or as a refusal of the whole face:** see item 4.

## Consequences

- **Tests.** `crates/openchuck-path/tests/swarf.rs` and `crates/openchuck-app/src/operation.rs` test, named after OP-02 and OP-07:
  - limit A stops and splits the pass;
  - limit B raises the tip, adds to the tip extension, and says that the far end cannot be limited;
  - the two compose;
  - both off gives the path without a boundary, with a warning; a watched surface keeps the path and warns, naming it, and an unbroken one says nothing; another strategy refuses restriction options (`crates/openchuck-path/tests/region.rs`);
  - an avoid surface stops the pass a tool radius short, a check surface stops it a further stock to leave short, and a rib in the middle splits the pass in two;
  - a floor the flank only meets is kept, and a tip extension into it leaves nothing, refused;
  - the application honours the three roles on a STEP part, warns naming the set with both limits off or a set on Ignore, keeps the path and warns for a set on Warn only, and refuses a face that is both cut and checked;
  - the options are set as undoable steps, refused for a set not in that role, kept in the file, and dropped when a set leaves its role (`crates/openchuck-app/tests/selection.rs`);
  - schema 14 round trips, writes nothing at the defaults, reads a schema 13 file with the defaults, and refuses the tables in a schema 13 file, options for a set not in the role, and unknown fields and choices (`crates/openchuck-project/tests/persistence.rs`);
  - the Geometry section shows the options for swarf only, and says when a set limits nothing (`apps/openchuck-ui/src/shell/frame_tests.rs`);
  - an edge set, a missing face, an empty set and a changed file are refused in each role.
- **Library API.** It changes as item 6 says.
- **Project format.** Schema 14 (item 3), claimed in the work breakdown.
- **Interface.** The options show in the Geometry section, under the sets (item 3); the interface specification's section 8 says so.
- **OI-233.** It stays open for the other strategies.

## Open items

- **OI-632** (closed): the owner's decisions 3a and 3b, items 4 and 5.
- **OI-634** (new): mesh avoid and check sets on a STEP part, which need triangle picking on its tessellation.
- **OI-635** (closed): limits A and B are settings, per boundary set in the Geometry section (the owner's answer, item 3).
- **OI-637** (new): whether the other strategies take options on their boundary, avoid and check sets, and what each means for them.
- **OI-636** (new): an avoid or check face is sampled no further apart than an eighth of the tool's cutting radius. With samples spaced `h` apart, a cut into a flat face shallower than about `h²/(8r)` can fall between them, where `r` is the cutting radius; that is about `r/512` at that spacing, 0.006 mm for a 3 mm radius. The simulation shows such a cut when it is deeper than the simulation's cell.
