# 0272: Swarf's tolerance check measures the written path, and the tolerance it advises is checked (lane BUG-SWARF4; issue #296; closes OI-1090)

- **Status:** accepted by the project owner on 28 Oct 2026, relayed by the coordinator session (decision 0049): the owner read the condensed record in the coordinator session and answered "Approve". The lane brief's owner rule that a record is accepted directly in the lane session was waived by the owner for this record, also relayed by the coordinator on 28 Oct 2026. Proposed 28 Oct 2026 by lane BUG-SWARF4. The owner chose option A on OI-1090, with option D as a follow-up (OI-1091), relayed by the coordinator on 28 Oct 2026.
- **What it amends:**
  - **[Decision 0061](0061-slice-17-swarf.md) item 7** (accepted): "Samples on every pose's ruling and every mid-move ruling (25 along each) are measured against the union of the tool at every pose and at three points of each move between poses". The gate now samples the motion and the face finely enough that its own error is at most 0.0005 mm (item 1).
  - **[Decision 0175](0175-swarf-overcut-is-a-warning.md) item 3** (accepted): the "What to do" sentence. The tolerance the warning advises is now checked by making the face again at it, and the warning says when the cut is the face's own error with this tool (items 3 and 4).

  OP-07, OP-12, NFR-05 and NFR-12 are unchanged. The path, the pose spacing (decision 0061 item 6) and the project file are unchanged. The avoid and check guards keep their own sampling (decision 0176, OI-636; decision 0229, OI-970).
- **Date:** 28 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): closes OI-1090; opens OI-1091 (option D); narrows OI-240 (the gate's sampling is settled here, detection and the directrix scan are not).

Numbering: the coordinator gave lane BUG-SWARF4 records 0272 to 0276 and OI-1090 to OI-1109. On 28 Oct 2026 the highest record on `main` and on every pushed branch was 0252, and the highest `OI-` number was OI-1003.

## Context

Trial finding F-103 (issue #296): on the owner's blisk, raising a swarf operation's tolerance after an overcut warning changed the reported cut. The owner reported tolerance 0.01 mm giving a cut of 0.0131 mm, and 0.0132 mm giving 0.0182 mm. The tolerance could not be set to clear the warning.

The lane measured `testdata/step/valid/blisk.step` with a checker of its own (`crates/openchuck-app/examples/swarf_tolerance.rs`). The checker rebuilds the machine's straight moves from the IR's poses with its own rotary kinematics, uses the testkit's independent tool outline, samples the exact face every 0.03 mm and cuts each move into 0.002 mm steps. Face 7 is the owner's #226 face (twist 8.62 degrees). A 3 mm flat tool reproduces the owner's "0.0131 mm at (40.330, -5.370, -9.425)" exactly, 10 mm along X because of the placement.

| Face 7, 3 mm tool | Tolerance 0.01 | 0.0131 and 0.0132 | 0.0182, 0.02 and 0.05 |
| --- | --- | --- | --- |
| Swarf reported (main) | 0.0131 | 0.0131 | 0.0006 |
| Written path, measured | 0.0163 | 0.0163 | 0.0163 |
| Warned (main) | yes | no | no |

The same holds with an avoid set of every face that is not a blade. With a 6 mm tool, swarf reported 0.0258 to 0.0283 mm and the written path measured 0.0330 mm at every tolerance. Splitting the cut by source:

- **Placement** is the whole cut on face 7. The poses alone, at a tolerance of 0.0005 mm, measure 0.0163 mm. The closed form for a straight flank tangent along a ruling whose normal turns by φ at its end is r(1 - cos φ) = 0.0169 mm. It does not depend on the tolerance.
- **Fitting** (the straight moves between poses) adds nothing measurable on face 7. On face 9 (twist 8.91 degrees) it adds 0.0008 mm at 0.01 and 0.0032 mm at 0.02: the placement alone is 0.0167 mm, and the written path measures 0.0175 and 0.0199 mm. This is within decision 0061 item 6's 0.4 times the tolerance.
- **Avoid handling** changes which poses are cut, but not the deepest cut on face 7.
- **Checking** is the fault. The gate sampled the face only on the rulings at each pose and mid-move, 25 points along each. So the deepest cut, beside a ruling and just inside its tip end (0.1 mm from it on the test helicoid), was found or missed depending on the pose spacing, which the tolerance sets. It under-reported by up to 27 times, and gave a false all-clear at 0.0131 and 0.0132 mm (OP-12, NFR-12).

The owner's exact sequence (0.0132 giving 0.0182) is not reproduced on `main`; the owner's builds were 0.14.0 and 0.17.0. The mechanism is the same: the reported figure followed the spacing, not the cut.

Records read: 0021, 0048, 0049, 0055, 0061, 0111, 0114, 0175, 0176, 0228, 0229. Requirements: OP-07, OP-12, OP-16, NFR-05, NFR-06, GC-07, NFR-07, NFR-12.

## Decision

1. **The tolerance limits the true deviation of the written path (the owner's option A).** The gate measures the machine's motion against the face finely enough that its own sampling error is at most `swarf::GATE_RESOLUTION`, 0.0005 mm, whatever the tolerance.
   - Near its deepest, the cut beside a ruling varies as the square of the offset over twice the tool's cutting radius r. So samples of the face and of the motion no further apart than h = 2 √(2 r × 0.0005) (0.077 mm for r = 1.5 mm, 0.11 mm for r = 3 mm; at least 0.005 mm) find the deepest cut to within 0.0005 mm.
   - Each move is cut into steps no longer than h at the tool tip, at the contact and at the far end of the ruling. The face is sampled on the ruling at each step, every h along the ruling, and at least 25 times. A move takes at most 4096 steps (NFR-07).
   - The material left is measured with the same samples. Denser samples can only find more of it, never less.
   - The argument is a sampling bound for a smooth cut, not a proof for any geometry. The tests check it against the independent checker on a closed-form helicoid and on the blisk.

   The reported figure is then a property of the face, the tool and the placement: 0.0158 mm on face 7 at every tolerance, against 0.0163 mm measured.
2. **The path is unchanged.** The poses, their spacing (decision 0061 item 6), the G-code and the golden files do not change (GC-07). The avoid and check guards and the watched surfaces keep their sampling (OI-636, OI-970).
3. **The warning stays a warning (decision 0175), and says when the cut is the face's own.** When the face twists along its rulings or the four axes tilt the tool, the warning adds: "Most of this cut is the face's own error with this tool, so a tighter tolerance does not remove it." It says "most", not "all", because on face 9 a looser tolerance adds a little (Context).
4. **The tolerance advised is checked (`swarf::clearing_tolerance`).**
   - The face is made again at the cut, rounded up to the four decimals shown (as the user would type it). While the cut there is still beyond it, the face is made again at that cut, up to `swarf::CLEARING_TRIES` (4) times.
   - When one clears, the warning says, for blisk face 9 at 0.01 mm (3 mm flat tool): "To accept this cut, raise the Tolerance of this operation to at least 0.0198 mm (in the operation inspector's parameters, where the field is marked): this was checked, and at 0.0198 mm the face is cut 0.0197 mm deep, within it." Face 9 shows why the check is needed: at 0.01 mm it is cut 0.0172 mm deep, but at 0.0173 mm (the cut, rounded up) it is cut 0.0197 mm deep, because the poses are further apart. Without the check, the advice would not have cleared the warning.
   - When none clears, it says: "Raising the Tolerance does not clear this warning: at X mm the face is still cut Y mm deep, because a looser tolerance also puts the tool's positions further apart."
   - Only cancellation fails the operation (UI-09). A face that cannot be made at a tolerance tried is reported as not cleared there.
5. **Library API (decision 0048).** In `openchuck_path::swarf`:
   - New: `Clearing` (`At` and `NotFound`, each with the tolerance and its cut), `clearing_tolerance`, `CLEARING_TRIES` and `GATE_RESOLUTION`.
   - Breaking: `FacePath::overcut_warning(tolerance)` becomes `overcut_warning(tolerance, clearing)`, so a warning cannot be built without a checked advice. The workspace goes to 0.27.0, the number the Merge Manager gave (AROT takes 0.26.0 ahead of this lane), with a changelog entry in each library crate.
6. **Deterministic.** The rulings' samples are made through `parallel::map_in_order` and flattened in order. Each sample's clearance is the least over the tool positions near it, which does not depend on the order they are visited in. The advice's regenerations are made in order. The same project gives the same path, warnings and G-code at 1, 2 and N threads (GC-07, NFR-06).
7. **Cost.** Generation is about 3.5 times slower. Ten blades of the blisk with a 3 mm tool at 4 threads take 4.3 s, against 1.2 s on `main` (release build). The prototype took 8.7 s; the lane cut it by:
   - computing the rulings' samples in parallel;
   - seeding each sample's clearance with the tool at its own ruling;
   - skipping any position whose cheap bound (the half cylinder that holds the tool) cannot beat that seed;
   - indexing the positions in a grid.

   A face that warns is made up to four times more, by item 4. Evaluating the face between exact samples was not chosen: it is within 0.0005 mm only on an exactly ruled face, and a near-ruled face may stray by up to the tolerance.

## Alternatives considered

- **B: the same dense check, with the cut shown as a range** ("between 0.0158 and 0.0163 mm") and no checked advice. Not chosen: the user would still have to guess a tolerance.
- **C: a fixed fine pose spacing, whatever the tolerance.** Not chosen: the measurements show the spacing is not the cause, and the under-report would remain.
- **D: a separate per-operation "accept overcut up to" setting beside the tolerance.** Deferred by the owner as a follow-up (OI-1091). Under A, the only way to clear the warning on a face whose own error the tolerance cannot reduce (0.0163 mm on face 7) is to loosen the tolerance, which coarsens the path (0.0008 to 0.0032 mm deeper on face 9). D needs:
  - a project schema number, claimed through the coordinator;
  - an amendment of decision 0061's one-number rule;
  - the measured figure always shown, even when accepted (NFR-12);
  - a default that reproduces today's behaviour.

  The owner will revisit it after using the corrected warning on real parts.
- **A gate measured at the poses alone, or at their quarter points (decisions 0113 and 0115 for the fitter).** Not chosen: the cut lies between the rulings sampled, and only the sample density finds it.

## Verification

| What | Evidence |
| --- | --- |
| The reported cut is the written path's, at every tolerance; a cut beyond the tolerance is warned of; the advised tolerance clears the warning in one step and the written path is then within it | `req_op_12_req_op_07_req_nfr_12_the_reported_cut_is_the_cut_of_the_written_path_whatever_the_tolerance` (`crates/openchuck-path/tests/swarf.rs`; helicoid of pitch 3 mm per radian over 24 degrees, 6 mm flat tool, tolerances 0.01, 0.0131, 0.02 and 0.05 mm; independent checker over the motion every 0.01 mm and the face every 0.03 by 0.1 mm). It failed on `main` (reported 0.0246 mm, measured 0.0309 mm, on the 40 degree helicoid it was written on); 13 s alone, uninstrumented |
| The blisk, with and without an avoid set | The harness `crates/openchuck-app/examples/swarf_tolerance.rs` (Context and items 1 and 7) |
| The advice search: tried again while beaten, not cleared where the face cannot be made, failed only by cancellation | `req_op_12_req_op_07_req_nfr_07_the_advised_tolerance_is_searched_by_making_the_face_again` (a helicoid, and a helicoid bent to be ruled only within about 0.002 mm) |
| The warning's words and checked advice | `req_op_07_req_nfr_12_an_overcut_above_the_tolerance_is_a_warning_that_says_how_to_accept_it`, and the test above |
| Identical at 1, 2 and N threads | `req_nfr_06_req_op_07_swarf_is_identical_at_1_2_and_n_threads`; the application's swarf reference job tests |
| Coverage | `swarf.rs` 94.52 % of lines on the crate's library and swarf tests alone (the nightly run counts every crate's tests). The one changed line not reached is the `?` that passes a cancellation of the advice search out of the strategy (UI-09, NFR-07) |
| Path unchanged | The swarf golden programs (`req_op_07_req_gc_09_req_gc_10_the_swarf_reference_job_passes_gates_1_and_2_and_matches_its_golden_files`) are unchanged |

## Consequences

- **Code:** `openchuck-path` (`swarf`: the gate, `Clearing`, `clearing_tolerance`, the warning). The application and the interface are unchanged: the warning keeps pointing at the tolerance (decision 0175 item 6).
- **Documents:** decision 0061 and 0175 are amended by this record; the verification plan's OP-07 row; the open-items register; the decision index; the library changelogs.
- **Open items:** OI-1090 closes (the owner's option A). OI-1091 is new (option D). OI-240 is narrowed to detection and the directrix scan.
