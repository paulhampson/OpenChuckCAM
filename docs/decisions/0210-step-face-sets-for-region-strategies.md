# 0210: Face sets on a STEP part for roughing, helical, axial, indexed and simultaneous finishing, and one check for the sets a strategy cannot use (lane STEPALL-F; issue #257)

- **Status:** accepted by the project owner (22 Oct 2026, directly in the lane STEPALL-F session: "Accept 0210"). Proposed 21 Oct 2026 by lane STEPALL-F. It builds items 3, 4.4 and 10 of [decision 0209](0209-step-geometry-for-every-strategy.md) (accepted) and records the choices the build made that 0209 does not.
  - **OP-23, agreed by the owner (21 Oct 2026, relayed by the coordinator):** option A. Lane STEPALL-F adds requirement OP-23 (P1) to `docs/requirements.md` as worded in 0209 item 9, with the verification plan's section 4 row, the two section 4.1 rows and the addition to IMP-06's row, in this change with its first tests, which carry the OP-23 name. The other options were P2, or a wider IMP-02 with no new ID.
- **Date:** 21 Oct 2026.
- **Requirements:** OP-23 (added here), OP-02, OP-03 to OP-05, OP-11, IMP-02, IMP-06, NFR-07, NFR-12, UI-07, GC-07, NFR-06.
- **Open items** ([docs/open-items.md](../open-items.md)): closes OI-233, OI-830 and OI-836. Opens OI-837 and OI-838. Depends on OI-81, OI-833 and OI-637 without closing them.

Numbering: the coordinator gave lane STEPALL-F record 0210 and OI-837 and OI-838, and no project schema number. On 21 Oct 2026 the highest record on `main` was 0191 and on the unmerged branches 0209 (lane STEPALL's proposal, pull request 269), and the highest `OI-` number in use was OI-836, so these were free. No project file changes: face sets are already stored (decision 0209 item 8).

## Context

Decision 0209 item 3 says how a face set is honoured on a STEP part by every strategy that takes a region on a mesh: map the set to the triangles of its faces in the part's joined tessellation, and build what a triangle set builds. Item 10 asks for the refusals that remain to be one check, read before Generate by the Geometry section and the operations overview and at generation as the fallback, worded with the strategy's label and what to do. Item 4.4 asks for each operation's notes on a STEP part to name the surface the tool was dropped on, and for the import notice to stop saying that toolpaths are computed on the exact surfaces (OI-830).

Records read: 0032, 0034, 0056, 0058, 0066, 0114, 0174, 0176, 0209. Open items read: OI-81, OI-233, OI-634, OI-637, OI-830, OI-833, OI-836.

## Decision

1. **Face sets as the triangles of their faces (0209 item 3).** On a STEP part, a strategy that honours a region (`Strategy::honours_region`), except swarf, which reads the exact faces itself, resolves its sets through the same code a mesh part's sets go through (`selection::resolve`). A face set is taken as the triangles its faces have in the joined tessellation (`BrepModel::joined_tessellation`), the triangles the tool is dropped on. So:
   - the selected sets are the region (Cut on), as a `MeshSurface` of those triangles;
   - the boundary set is `Window::of_mesh` of them (Contain within);
   - the avoid and check sets are `MeshSurface`s of them;
   - a set's own stock to leave is a zone of them, as on a mesh (OP-11): helical, axial and indexed cut each zone to its amount, and roughing leaves the largest amount wherever it cuts, with its note (decision 0058).

   The tool is still dropped on the whole part (`BrepSurface::new`), as a mesh part's tool is dropped on the whole mesh. The checks are those of a triangle set, worded for faces: a face that is not in the model, a set that selects no faces, a set made on another model or another version of the file. This is roughing, helical, axial, indexed and simultaneous finishing; rest machining, pencil clean-up, edge following and radial drilling take no region on any part (0209 item 2).

2. **The cut against an STL's.** 0209 item 3 says the mapping is exact to the cut while drops use the tessellation. The contacts are: they lie on the region's triangles, so the region test judges them as on an STL. The moves are not the same as an STL's of the same triangles, even with no sets: a `BrepSurface` gives the strategies its exact bounds and, where a strategy asks, its exact normals and projections (simultaneous finishing's drive points, decision 0036). So the tests do not compare moves. They run each case on the STEP part and on an STL of the same triangles, and check that both keep to the same faces: with Cut on, Contain within or Avoid every cutting move lies over the chosen face, with Check none touches the checked face, and with a stock to leave per set the tip is that much higher over that face and not elsewhere.

3. **Set options stay swarf's.** On a STEP part as on a mesh, the five strategies take their sets with the default options: a boundary limits the pass and the tip, and an avoid or check set cuts the pass short. Options stored on the operation (schema 14) are kept and not used, and the inspector does not show them, as decision 0176 item 3 says for another strategy. Whether other strategies take them is OI-637 (lane SWARF3).

4. **One check (0209 item 10.1).** `step_sets::refusals` judges every set of an operation on a STEP part from the project alone: the strategy, each set's kind, its role and its stock to leave. It never reads the model, so it gives the same answer before and at generation. Three places read it:
   - the operation inspector's Geometry section shows each answer under the role that holds the set, as an amber Warning line (`Application::set_refusals`);
   - the operations overview puts the operation in error with the first answer, and each answer is a Messages line with a Show link, as decision 0174 does for a strategy on an STL part. When a generation has already failed with the same words, the line is not said twice;
   - generation refuses with the first answer, before the model is read (NFR-07).

   Generate stays enabled. The operation is skipped with its error, as an operation in error is today. A set that does not fit the part (another version of the file, a face not in the model) is the set's own problem, found where it is today (the set's red border, and generation).

5. **The wording (0209 items 10.2 to 10.4).** Each answer names the strategy by its label, shows no requirement ID or decision number, and says what to do. The Geometry line and the message use the same text, so they never disagree. That text already names the set and the strategy, so the line is the text itself, not "*Strategy label* can't use the set '*set*' here: *reason*." as 0209 item 10.2 sketches. The cases, in the order they are judged for each set:

   | Case | Text |
   | --- | --- |
   | A triangle set, or a set of triangle edges, kept after the part became a STEP file (any role) | The set '*set*' selects triangles (or: edges of triangles), but this setup's part is now a STEP file, which is selected by its faces and edges. Select the faces again on the STEP part. |
   | A boundary, avoid or check set on a strategy that takes no region (rest machining, pencil clean-up, edge following, radial drilling) | *Strategy label* does not use a boundary, avoid or check set. Take the set '*set*' out of the operation. |
   | An edge set as the boundary, an avoid or a check set on a strategy that takes them (new: 0209 does not list it) | *Role* takes faces, not edges, on a STEP part. Take the edge set '*set*' out of *role*, and select faces instead. (*Role* is the Geometry section's name: Contain within, Avoid or Check.) |
   | An edge set as the selection on any strategy but edge following | Only edge following follows chosen edges. Take the edge set '*set*' out of the operation, or use edge following. |
   | A face set as the selection on rest machining, pencil clean-up or edge following | *Strategy label* works on the whole part and cannot be limited to chosen faces. Take the set '*set*' out of the operation. |
   | A selection set with its own stock to leave, on a strategy that does not leave one per set (new: what remains of 0209's R3, for swarf, radial drilling, edge following and simultaneous finishing) | *Strategy label* does not leave a stock of its own on a selection set. Clear the stock to leave on the set '*set*', and set the operation's stock to leave instead. |

   Each still starts "operation '*name*' cannot be generated: " when it reaches generation. Simultaneous finishing does not leave a stock per set on a mesh either; there it is still refused at generation, in the words it had.

6. **On a mesh part.** The check is for STEP parts, as 0209 item 10 says. The one refusal on a mesh part that broke the wording rules is reworded in place: a triangle set on a strategy that takes no region now reads "*Strategy label* works on the whole part and cannot be limited to chosen triangles. Take the selection sets out of the operation." in place of "the strategy '*name*' cannot yet be limited to selections (OP-02); …". It is still said only at generation; showing a mesh part's refusals before Generate as well is OI-838.

7. **The surface the tool was dropped on (0209 item 4.4; OI-830).**
   - On a STEP part, every strategy but radial drilling, swarf and edge following adds to its notes: "*Strategy label* drops the tool on the STEP part's tessellation, made at a deflection of 0.01 mm, not on its exact faces: dropping on the exact faces is not built yet". The deflection is the import's (the application has no setting, OI-831). It does not say how far the tessellation is from the faces: the kernel's setting is not a bound (decision 0003), and lane STEPALL-M measures it. A strategy added later gets the note unless it is named among those that read the exact faces, so the notes never claim more than is built (NFR-12).
   - The import notice now reads: "STEP keeps its exact faces and edges, which radial drilling, swarf and edge following read. The other strategies drop the tool on the tessellation (0.01 mm, 0.1 rad) for now, and each operation's notes say so; the display and the mesh checks use it too."
   - Lane STEPALL-X changes both when the exact drop is built.

8. **Determinism and isolation.** The mapping is a pure function of the placed part and the sets, in triangle order; nothing new runs in parallel. The same project gives the same G-code at any thread count (GC-07, NFR-06). The kernel stays in `openchuck-import`: the mapping reads only the plain-data B-rep (`openchuck-types`). The library family is unchanged, so there is no version bump (decision 0048).

## Alternatives considered

- **Region surfaces as `BrepSurface::with_faces` (the exact faces).** Rejected by 0209 item 3 while drops use the tessellation: the region test at 1e-4 mm would refuse contacts up to the deflection off the exact face. It is OI-833, lane STEPALL-X's.
- **Comparing the STEP part's moves with an STL's move for move.** Tried first; the moves differ with no sets at all, because a `BrepSurface` answers some queries from the exact faces (item 2). Comparing where the cut lies tests what the requirement asks.
- **The Geometry line in the form "*Strategy label* can't use the set '*set*' here: *reason*."** It would repeat the label and the set's name, which the case texts already carry, and it would make the line and the message differ in words.
- **The check for mesh parts too.** It would change what a mesh part's user sees beyond 0209's scope, and the mesh refusals have their own wording (a triangle set, a mesh edge set). Left as OI-838.

## Verification

OP-23's tests so far cover its face-selection and notes parts; its exact-surface parts are lanes STEPALL-M's and STEPALL-X's. Unit tests in `openchuck-app` (`operation.rs`), on a closed-form STEP part with no kernel (`openchuck_testkit::brep::banded_cylinder`: a round bar whose side is three cylinder faces):
- `req_op_23_req_op_02_req_op_11_face_sets_limit_{helical,axial,indexed,rotary_roughing}_on_a_step_part_as_on_a_mesh`: Cut on, Contain within, Avoid, Check and a stock to leave per set, each on the STEP part and on the same triangles as an STL, each keeping to the same faces (item 2). Uninstrumented debug times on the lane's container, one test at a time: axial 27 s, helical 21 s, roughing 4 s, indexed 2 s; all are under the long-test line of a minute.
- `req_op_23_req_op_05_simultaneous_finishing_is_limited_to_chosen_faces_of_a_step_part`.
- `req_op_23_req_nfr_07_a_face_set_that_does_not_fit_the_step_part_is_refused`, in all four roles.
- `req_op_23_req_op_11_req_nfr_07_one_check_refuses_sets_before_and_at_generation`: every registered strategy, every case of item 5 that applies to it, the check's text equal to generation's, no internal name or requirement ID, the label first where the text names the strategy.
- `req_op_23_req_nfr_12_an_operation_on_a_step_part_names_the_surface_it_dropped_on`, and in `openchuck-import` `req_op_23_req_imp_02_req_nfr_12_the_notice_says_which_strategies_use_the_tessellation`.

Integration tests: `tests/operation.rs` `req_op_23_req_nfr_07_a_set_a_strategy_cannot_use_on_a_step_part_is_an_error_before_generating` (the overview's error state and Messages line before Generate, the same words at generation, and the line said once after that generation fails), and in `openchuck-ui` `req_op_23_req_ui_07_the_geometry_section_warns_of_a_set_the_strategy_cannot_use`.

Not run in the lane's container: the kernel tests (`--features kernel-tests`), which need OpenCASCADE. They do not test the changed refusals or the new note, but CI runs them. `blisk.step` is not used here: it reaches `testdata/step/` with lane STEPALL-M, and its blades as a face set are a test for that lane or a later one, on a part with real faces.

## Consequences

- **Code:** `openchuck-app` (`operation.rs`, `selection.rs`, the new `step_sets.rs`, `overview.rs`), `openchuck-ui` (`shell/inspector.rs`), `openchuck-import` (`step_tessellation_notice`), `openchuck-testkit` (`brep::banded_cylinder`). The application gains `Application::set_refusals` and `step_sets::SetRefusal`.
- **Documents:** this record and the decision index; `docs/requirements.md` (OP-23) and `docs/verification-plan.md` (OP-23's row, two rows of 4.1 and IMP-06's row), as the owner agreed; `docs/open-items.md` (OI-233, OI-830 and OI-836 closed; OI-837 and OI-838 added); the interface specification's Geometry item (0209 item 10.6); the work breakdown's STEPALL row.
- **Open items:**
  - OI-837: the warning of 11.7 (a set reaching a held region or a tab) is not given for a face set on a STEP part: `workholding::selection_reach_notes` reads triangle sets only.
  - OI-838: on a mesh part, a set the strategy cannot use is still said only at generation, not in the Geometry section before Generate.
