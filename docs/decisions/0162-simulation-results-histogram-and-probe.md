# 0162: Simulation results: a histogram that reaches the worst values, the overcut count, and a value probe (lane BUG-RESULTS; issues 217 and 208)

- **Status:** accepted by the project owner (20 Oct 2026), directly in the lane BUG-RESULTS session. Item 4 (the probe now, ahead of the rest of SIM-11) is option A, which the coordinator relayed first. The UI specification 11.5 edits under "Specification entries" are made in the same change. Item 6 (the pencil moves) was added after the acceptance, at the owner's instruction in this session ("fix the pencil error in this lane"). It amends [decision 0044](0044-slice-15-rest-and-pencil.md) item 5.
- **Date:** 20 Oct 2026.
- **Requirements:** SIM-05 (P1), SIM-11 (P2, in part), OP-09 (P1, item 6), NFR-12, NFR-05, UI-11.
- **Open items:** OI-560 to OI-566 (new); OI-561 and OI-566 closed by this change.

## Context

Issue 217 (trial feedback F-61, NFR-12): the Simulate results showed a largest overcut of 1.020 mm beside a deviation histogram whose axis read from -0.1 to +0.5 mm, so one of the two looked wrong. The owner did not know whether the 1.020 mm was one point or many, or which operations the histogram covered. The owner's example project (`blisk_1_overcut_mismatch.zip`, on issue 217) could not be downloaded in the cloud session: the session's GitHub proxy refuses user-attachment URLs. The cause was found in the code and reproduced in tests instead.

**Root cause.** Neither number was wrong; the histogram hid the overcut.

1. The worst values and the histogram come from the same samples: the setup's stock after all of its operations, measured against the part (and the held regions and tabs where they show) by `openchuck_sim::compare`. The largest overcut is the most negative sample.
2. The old histogram (`results_panel.rs`, 11 fixed bins) put every sample cut deeper than the tolerance into bin 0, however deep. Its axis read "overcut | -0.100" and "+0.5 mm", so the 1.020 mm sample sat in the leftmost bar with nothing to say how far that bar went. The last bin clamped the same way: material left above +0.5 mm, up to the 3 mm search limit, was drawn as 0.5 mm.
3. Two smaller mismatches. The equal bins (0.06 mm wide at the 0.1 mm default cell) straddled +tolerance and the band thresholds, so the grey bars did not match the "Within ±0.100 mm" share or the part's colours. "Largest overcut" was also shown in the overcut colour when the deepest cut was within the tolerance, which the bands and the share count as on the surface.
4. Scope: the histogram and the worst values are per setup, after its last operation; they are not for the selected operation. Per-operation overcut and material left (SIM-14, P2) are not built (OI-371).

**The 1.020 mm is real.** The owner's project was later committed to `claude/upbeat-davinci-jk46yd` (52b87b5) and run with `openchuck-cli simulate` (77 s; OI-561, closed). The deepest overcut is 1.0195 mm, at 19378 samples beyond the 0.1 mm cell, on the blade tips at radius 20 mm between X 20.4 and 22.5 mm. Run without its later operations, the project gives no overcut after Roughing and 0.049 mm after Helical Finishing. So the overcut is tool motion of Operation 3, pencil finishing with the 2 mm flat end mill, about the tool's radius deep. That is a toolpath defect (OI-566), fixed by item 6. The comparison also includes the exposed surfaces of held regions and tabs, which the part's colours do not show (OI-562); that is not the cause here.

Issue 208 (F-52) asks to read the value at a point on the part in the deviation, remaining or overcut view. This is part of SIM-11 (P2), whose specification entry (11.5, "The probe") gives the signed deviation, the position (X and A), and the operation and move that left it. The owner answered on issue 208: the tooltip shows the value only, with no position; a click keeps the value on screen; a click off the model hides it. The coordinator assigned issue 208 to this lane. Whether to build it now, ahead of the rest of SIM-11, was put to the owner through the coordinator (OI-560).

## Decision

1. **The histogram reaches the worst values.** `results_panel::histogram` makes a bin for the surface band (-tolerance to +tolerance, closed), and beyond it bins split at the part's colour thresholds (0.25, 0.5 and 1.0 mm, mirrored below the surface for the overcut, as in `deviation_bands`). The outer bins run to the deepest sample and to the largest sample, so the first row starts at the largest overcut and the last ends at the most material left. An edge that the tolerance or the extremes cover is left out. A bin below the surface holds `low <= v < high`; one above holds `low < v <= high`, as the colour bands are bounded. Each bin is drawn in its band's colour.
2. **Each bin is a labelled row.** The range in millimetres at three decimals (header "Range, mm"), a bar in the band's colour scaled to the largest bin, and the count and share of samples ("12 (0.3%)"). A note above the rows says that below zero is cut under the surface and above zero is material left. An end that is only a bound of the search (the 3 mm limit) is marked "≤" or "≥". The rows fit the inspector at its narrowest width (280); a test checks this.
3. **The overcut count is shown.** A row "Overcut samples" gives the samples cut deeper than the tolerance and the separate places (2 mm apart, the run lists at most ten: "3 in 10+ places"), so the largest overcut reads as one point or many. "Largest overcut" says "within ±0.100 mm" and drops the overcut colour when the deepest cut is within the tolerance. Material left and an overcut at the search limit read "or more".
4. **The value probe (SIM-11 in part).** On Simulate, with the part coloured by deviation:
   - once the pointer has rested for 0.3 s (moved less than 2 points), a tooltip gives the value for the view chosen. Deviation: "Overcut 1.020 mm", "Material left 0.350 mm", or "On the surface, 0.000 mm". Remaining: the material left, or "No material left". Overcut: the overcut, or "No overcut". Values are in millimetres at three decimals, with ", within ±0.100 mm" when inside the tolerance and "or more" at the search limit. There is no position;
   - a primary click on the part keeps the value on screen, at the point clicked, with a small marker. It follows the view as the camera moves and hides when the point is out of view. A click off the part, a new part, or the colours turned off clears it. With the part hidden, nothing is read and a kept value is not drawn;
   - the value is the one the triangle is coloured by: its worst sample, held with the bands (`DeviationBands::values`). On a large triangle it is the worst over the triangle, not at the exact point;
   - it costs nothing on the GPU. The pick is the CPU ray pick the readout already makes each frame (`ScenePicker`), and the probe only looks up the triangle's value once the pointer rests. No new rendering pass and no read back.
5. Items 1 to 4 change no toolpath, simulation or G-code (GC-07, NFR-06). They are in `openchuck-ui` and one field of `DeviationBands` in `openchuck-app`, which is not in the library family, so semver is not affected.
6. **Pencil moves are checked, not only its placements** (OP-09, NFR-12; amends decision 0044 item 5, "the pass cannot gouge").
   - The cause of the blisk overcut: each placement is a drop, but where the corner narrows below the tool the drop rests it on the blade tips. The straight move between a rest in the root and a rest on the tips then went through the blade.
   - `pencil.rs` now checks each move between two placements, as the machine cuts it (linear in X, across, height and A, with A turning the short way to the next angle, as assembly unwraps it). It looks at points a quarter of the tool's radius apart (or the largest step, if shorter), counting both the tip's travel and the arc the point of the cutting part furthest from the rotary axis sweeps as A turns, so a turn with little tip travel is still sampled closely, and compares each with a drop there, within the operation's tolerance.
   - A move that fails gets a point dropped half way, and each half is checked the same way, up to 8 halvings. The tool then rides over the blade on its surface. Only if that does not clear the move does the pass end there; the assembler's retract and link join it to the next.
   - Ending the pass at every failing move was tried first. It also removes the overcut, but its links made the blisk's pencil pass seven times as many simulated poses and the run three times as long (236 s against 77 s). Halving keeps it at 83 s.
   - It changes the G-code of a project with pencil finishing wherever a move used to cut into the part. It is still deterministic (GC-07, NFR-06), and `openchuck-path`'s API is unchanged.

## Alternatives considered

- **Keep the 11 equal bins and label the open ends** ("below -0.100", "above +0.500"). Rejected: the extremes would still not be on the axis, and the equal bins would still straddle the tolerance and the colour thresholds.
- **Vertical bars with tick labels under them.** Rejected: at the inspector's 280 minimum width there is no room for a range label per bin, which issue 217 asks for.
- **A probe read back from the GPU (an ID buffer).** Rejected for slow graphics (issue 200, Intel UHD 620): it adds a pass and a read back. The CPU pick already runs for the readout.
- **The probe showing the exact value at the point** (a new measurement under the pointer). Rejected: it needs the stock, which is not held on the user-interface side. It could also disagree with the colour the user sees.

## Specification entries (made in 11.5 on acceptance)

UI specification 11.5, Results inspector, replace "a deviation histogram in the same colours;" with:

> - "Overcut samples": the samples cut deeper than the tolerance and the separate places, so the largest overcut reads as one point or many;
> - a deviation histogram in the same colours, one labelled row per bin: the range in millimetres, a bar, and the count and share. The bins are the surface band ±tolerance and the colour thresholds (0.25, 0.5, 1.0 mm, mirrored for the overcut); the first row starts at the largest overcut and the last ends at the most material left, so the worst values are always on it. An end at the search limit is marked ≤ or ≥.

UI specification 11.5, Viewport, replace the probe bullet with:

> - **The probe** *(SIM-11, in part)*. With the part coloured by deviation, resting the pointer on the part (0.3 s) shows a tooltip with the value for the view chosen ("Overcut 1.020 mm", "Material left 0.350 mm", "No overcut"), in millimetres at three decimals, without the position. A click keeps the value on screen at the point; a click off the part hides it. The value is the one the triangle is coloured by. Picking is on the CPU from the data already held, only when the pointer rests. The operation and move that left the point are added with the rest of SIM-11.

## Consequences

- Tests (all `openchuck-ui` unless named):
  - `req_sim_05_the_histogram_range_covers_the_reported_extremes`, the test issue 217 asked for. The old fixed axis (-tolerance to +0.5 mm) fails it on the issue's case and on the search-limited cases.
  - `req_sim_05_the_histogram_the_worst_values_and_the_shares_agree`: every sample is in one bin; the surface bin is the share within the tolerance; the overcut bins hold the overcut count; each bin's band is the band its values colour the part in.
  - `req_sim_05_the_bins_are_labelled_and_the_overcut_spread_is_stated` and `req_sim_05_the_labelled_histogram_fits_the_narrowest_inspector`.
  - `req_sim_11_the_probe_gives_the_value_of_the_view_without_a_position`, `req_sim_11_the_probe_reads_the_value_under_a_resting_pointer_and_a_click_keeps_it`, `req_sim_11_the_probe_reads_nothing_over_an_overlay` and `req_sim_11_the_probe_reads_nothing_on_a_hidden_part`.
  - `openchuck-app` `req_sim_11_the_bands_keep_each_triangles_value_for_the_probe`.
- Test `openchuck-sim` `req_op_09_req_nfr_12_pencil_never_cuts_through_a_tooth_between_two_rests`, a toothed wheel whose gap narrows below a 2 mm flat end along the crease. Before item 6 the simulated stock was cut 1.8 mm into a tooth; after it, nothing. On the owner's project, `openchuck-cli simulate` now reports a deepest cut of 0.049 mm, within the cell (from Helical Finishing).
- `openchuck-path` unit tests `req_op_09_req_nfr_12_a_move_is_checked_on_the_arc_the_machine_turns` (A turns the short way across the end of the range) and `req_op_09_req_nfr_12_a_turn_is_sampled_by_the_arc_the_tool_sweeps` (a turn with little tip travel is sampled by the arc the tool sweeps).
- The rest of SIM-11 (the operation and move that left a point, "Go to overcut" and "Go to largest left") needs the engine to attribute each sample to a move; it stays with SIM-11 (OI-560).
- Keyboard and screen-reader access to the probe's values is not built (OI-563). The worst values, the overcut count and the histogram rows are text in the inspector, so the same facts can be read without a pointer.

## Open items

- OI-560: whether to keep the probe ahead of the rest of SIM-11 (owner, through the coordinator), and the rest of SIM-11 itself.
- OI-561 (closed): the 1.020 mm on the owner's blisk, checked with the CLI; it is pencil finishing's tool motion.
- OI-566 (closed by item 6): pencil finishing cut about the tool's radius into the blisk's blade tips.
- OI-562: an overcut on a held region's or tab's exposed surface counts in the worst values and the histogram but is not coloured on the part, and the probe cannot read it.
- OI-563: keyboard and screen-reader access to the probe.
- OI-564: the UI specification 11.5 edits above.
- OI-565: the band visibility requirement of issue 216 (decision 0163).
