# 0520: Simulate shows the deviation colours on the part model as well as on the cut part (SIM-05, UI-05, UI-11, UI-12, NFR-15; issue #430; lane VIEW-OVERLAYS)

- **Status:** proposed. The owner's answers to questions 1 to 3 (1 Nov 2026) were relayed by the coordinator (decision 0049); the record itself waits for the owner's acceptance.
- **Date:** 1 Nov 2026.
- **Issue:** #430 ("Deviation colours should show on the part model").
- **Requirements:** SIM-05, SIM-15, UI-05, UI-11, NFR-12, NFR-15. No requirement changes.
- **What it amends:** the interface specification ([decision 0038](0038-ui-specification-and-requirements.md)), section 11.5 (the viewport and the probe on Simulate). It builds on [decision 0238](0238-simulate-display-and-operation-navigation.md) items 2 to 4 (deviation colours on the cut part, the shared Show state, opacity), [decision 0216](0216-selection-set-colours-and-deviation-scale.md) (the colours and the set outline), [decision 0217](0217-band-visibility-and-the-kept-comparison.md) (band visibility) and [decision 0162](0162-simulation-results-histogram-and-probe.md) (the probe). It keeps the rule of issue #330: no colours on the target model as a fallback when the cut part cannot be prepared.
- **Open items:** OI-1760 is new (the timing on the reference hardware). No project schema change; no library version change.

## Context

Since decision 0238 item 4, Simulate colours the cut part: each stock triangle takes the value of the nearest counted sample of the final comparison. The owner asks for the colours on the part model too, so it is clear which elements of the part are affected (#430).

The comparison already measures on the part. Its samples lie on the target's triangles, in triangle order ([decision 0108](0108-deviation-samples-follow-the-area.md), [decision 0194](0194-deviation-samples-say-what-they-lie-on.md)), and `openchuck_sim::triangle_values` gives each part triangle its worst counted value. `Application::start_deviation_bands` already sorts the part's triangles into bands, and the viewport could already draw a part in its bands; it stopped doing so on Simulate when the cut part took the colours. On the part the colours are therefore the measurement itself, not a projection.

## Decision

1. **Where (owner, question 1, option A).** On Simulate only. "Deviation colours" in the results inspector colours what is shown: the **Part** by its own measured values, and the **Cut part** by the projected values of decision 0238 item 4. When both are shown, the Part carries the colours and the Cut part is drawn plain at the 30% opacity of plain stock, so the coloured part shows through it, in Translucent and in Solid. Other steps do not show deviation colours.
2. **No fallback (#330).** When the cut part cannot be prepared, "Deviation colours" stays disabled with its reason, and neither the part nor the cut part is coloured. The part's colours are their own display, never a substitute for the cut part's.
3. **Per triangle.** Each part triangle is drawn in the band of its worst counted sample, as `triangle_values` gives it. A summary per B-rep face is not part of this record.
4. **Legend (owner, question 3, option A).** The accepted Show defaults stay (decision 0238 item 3: Cut part). The colour scale box names what it colours: "Part · final result" while the part is coloured, "Cut part · nearest target sample" while only the cut part is. While only the Cut part is shown it adds "Tick Part to see these colours on the part model."
5. **Probe.** On the coloured part the probe reads "Final result · …" with the triangle's own value, without "(nearest target sample)"; on the coloured cut part it reads as decision 0238 says. The cut part drawn plain over the part is not probed.
6. **Selection sets and band visibility.** A shown set keeps its fill and framed outline over the coloured part (decision 0216 item 4); the bands never get an outline. Band visibility (SIM-15), "Colour by", the hidden-band slate and the not-measured hatch apply to the part as to the cut part.
7. **Out of date (owner, question 2).**
   - The toolpath, the simulation settings or a setup setting that is not the part's placement (stock, material, clearance, holding) changed since the run (the existing test, `simulation_out_of_date`): the part keeps its colours, not dimmed, under the one "Simulation out of date" banner, as the cut part does (decision 0238 items 1 and 2). Dimming would break the margins of decision 0216.
   - The part itself changed since the run: the part is drawn plain, and the colour scale box says "Deviation colours are not shown on the part: it has changed since this simulation." The cut part keeps its colours under the banner. "Changed" means the setup's part reference differs from the run's in content hash, units or transform; or the setup names another part or places it differently on the machine (its frame); or the model file read for the colours does not match the run's recorded hash. A reimport or replacement already marks the run out of date, because the pipeline's key includes the part reference with its recorded hash (`cache.rs` `operation_key`). Only a file edited outside the application under an unchanged recorded hash escapes that key, and opening the project reports it (PRJ-02); the file check covers it here.
8. **Cost.** The part's bands are made by a job, off the interface thread, once per run and setup, and kept while the run is shown. The timings are in "As built". OI-1760 covers the check on the reference hardware, as OI-987 does for the cut part.

## Specification entry proposed

Section 11.5, Viewport, a new item after "The stock":

- **The part** *(SIM-05; decision 0520)*. With "Deviation colours" on and Part shown, the part is coloured by its own measured final result (the band of each triangle's worst counted sample); with Cut part shown too, the cut part is drawn plain at 30% over it. The colour scale box names what it colours ("Part · final result", "Cut part · nearest target sample") and, while only the cut part is shown, adds "Tick Part to see these colours on the part model." The probe on the part reads "Final result · …". After a change to the toolpath or the settings the colours stay under the stale banner; after a change to the part itself (reimport, replacement, placement) the part is drawn plain and the scale box says "Deviation colours are not shown on the part: it has changed since this simulation." When the cut part cannot be prepared nothing is coloured (#330).

## Alternatives considered

- **Operations as well (question 1, option B),** or every step (option C). Not chosen: on Operations the colours would compete with the highlights used to pick faces for selection sets.
- **Dim the colours of an out-of-date run.** Rejected: dimming breaks the contrast and colour-vision margins of decision 0216, and the banner already says the run is out of date.
- **Keep a copy of the old part's mesh and draw the old colours on it** when the part changed (question 2, option B). Rejected: more memory, and a ghost part beside the real one is harder to read than a plain part with a note.
- **Hide every colour whenever the run is out of date** (question 2, option C). Rejected: it removes useful information when only the toolpath changed, and goes against decision 0238 item 2.
- **Show the Part by default when colours are on, or add a "Show on the part" button** (question 3, options B and C). Not chosen: the first changes the accepted default of decision 0238 item 3; the second repeats a quick box that already exists.
- **Project the cut part's colours onto the part.** Rejected: the part has the measured values; a projection would be less exact.

## Consequences

- `apps/openchuck-ui`: the part's bands kept apart from the cut part's in the viewport; the cut part drawn plain while the part is coloured; the colour scale box's title and lines; the probe text; the part bands job and its cache in the playback panel.
- `crates/openchuck-app`: a check that the setup's part has changed since a run, and the file-hash check in `start_deviation_bands`. Additive; no library-family change, no schema change.
- Specification 11.5 gets the entry above when the record is accepted.

## As built

- `crates/openchuck-app`: `stockplay::part_changed_since(project, run, setup)` (item 7: the setup's part and frame, the part model's hash, units and transform; a setup or model missing from either project counts as changed); `start_deviation_bands` reads the part in the run's setup and returns the new `AppError::PartChanged` when the file read does not match the model's recorded hash (the enum is `#[non_exhaustive]`, so the addition is not breaking). The private `model::placed_mesh` is removed (no longer used).
- `apps/openchuck-ui`:
  - `viewport.rs`: the part's bands (`set_part_bands`) are kept apart from the cut part's; `part_coloured` (Simulate, Part ticked, bands at hand) decides the scene; the cut part is drawn plain and translucent while the part is coloured; the scale's title and notes; the probe reads the part's triangles when the part is coloured and drops a kept value when its source changes.
  - `viewport_overlays.rs`: the title and the notes in the colour scale box, and the note alone in a box when there are no colours to scale.
  - `playback_panel.rs`: the part bands job, started only when colours are on, Part is ticked and the cut part was prepared; cached per (run, setup); `part_changed_since` checked each update, so an undo brings the colours back without a new job.
- Choices made in the build, within the decision:
  - Until the part's job has finished (74 ms on the blisk mesh, below), the cut part keeps its colours; it then turns plain. Both states are real.
  - The cursor readout still reads the cut part; only the probe reads the part.
  - A run made when the file already differed from its recorded hash gets no part colours: the check is against the recorded hash.
- Tests:
  - `openchuck-app` (`tests/playback.rs`): `req_ui_11_the_part_guard_names_what_changed_since_the_run` (3.0 s; a rename and a material change are not a change), `req_ui_11_deviation_bands_refuse_a_part_file_changed_since_the_run` (3.8 s).
  - `viewport.rs`: `req_sim_05_the_part_and_the_cut_part_keep_their_own_bands`, `req_ui_05_req_nfr_15_a_shown_set_keeps_its_outline_over_the_coloured_part`, `req_sim_05_the_part_probe_says_final_result_without_nearest_sample`, `req_nfr_15_the_colour_scale_names_what_it_colours` (each under 0.3 s).
  - `playback_panel.rs`: `req_sim_05_the_part_is_coloured_by_its_own_measured_values` (7.6 s), `req_sim_05_with_part_and_cut_part_shown_the_cut_part_is_plain_and_translucent` (9.8 s), `req_sim_05_no_colours_anywhere_when_cut_part_preparation_failed` (3.5 s), `req_sim_15_band_filters_apply_to_the_part` (2.6 s), `req_ui_11_a_stale_toolpath_keeps_the_part_colours` (2.5 s), `req_ui_11_a_changed_part_is_drawn_plain_with_the_note` (2.6 s), `req_ui_11_a_part_file_edited_behind_the_application_is_drawn_plain_with_the_note` (2.9 s).
  - Shell: `req_ui_11_a_stale_toolpath_keeps_the_part_colours_with_the_banner` (10.5 s), `req_ui_11_a_changed_part_is_drawn_plain_in_the_shell_with_the_note` (14.7 s), `req_sim_11_req_sim_05_a_coloured_part_is_probed_through_the_plain_cut_part` (8.7 s).
  - Three existing probe tests in `viewport.rs` moved their pointer positions only: the taller colour scale box covers the old spots in the 400 x 400 test window.
  - The `openchuck-app` tests were seen failing first; the interface tests were written after the code and checked by breaking the code (the plain cut part, the hash check, the probe wording) and watching them fail.
- Timings (debug build, `testdata/stl/blisk.stl`, 6,386 triangles): the part's bands job 74 ms, its scene 48 ms, a band filter 0.4 ms, the rebuild 14 ms. The STEP blisk and the reference hardware are OI-1760.
- Uncovered new lines (the new interface tests alone): the part job's cancellation paths and its error arm in `playback_panel.rs`, and the "missing setup or model" returns of `part_changed_since`.
