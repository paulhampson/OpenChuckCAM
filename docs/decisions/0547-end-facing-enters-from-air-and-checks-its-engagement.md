# 0547: End facing enters each band from air and checks its engagement (lane PATHFEED; issue #641)

- **Status:** accepted by the project owner on 8 Nov 2026, relayed by the coordinator (decision 0049), after the owner read it in full. Proposed by lane PATHFEED on 8 Nov 2026; the owner answered questions 1 to 3 the same day (option A for each, as recommended, and the first finding below in scope of this work), and the decision below is written to those answers. Amended the same day by the owner's answer to question 4 (option A, relayed by the coordinator; section "Question 4" below), found while building it. Built in lane PATHFEED's #641 pull request (section "As built").
- **What it amends:** [decision 0252](0252-facing-ends-sides-and-extents.md) section 5 (End-cutting geometry and limits: the entry of each band, "validated engagement", and the meaning of the entered stepdown), and through it the UI specification's Facing controls paragraph and the verification plan's OP-24 row. The entry styles of [decision 0026](0026-entry-moves.md) are not changed; the shared entry (`assemble/entry.rs`, lane RAMPENTRY) is not edited. The requirements and the architecture are unchanged.
- **Requirements:** OP-24 (Facing), OP-11 (stepover, stepdown, entry), OP-14 (clearance), TL-05 (cutting data), NFR-07 (clear refusal), NFR-12, GC-07 / NFR-06 (determinism).
- **Date:** 8 Nov 2026.
- **Open items:** OI-2130 (the answers; closed with them), OI-2131 (schema 28; closed when built), OI-2132 (the second finding, issue #650) and OI-2138 (question 4; closed with the owner's answer). Numbers given by the coordinator; the Merge Manager assigns final numbers at release.

## Context

End facing (0252 section 5) cuts the unheld end of the stock with the flank of a flat end mill. Each pass is built in `crates/openchuck-path/src/facing/end.rs` (`plan_end`, the `points` array in the slice loop, `:291-297` on `main` `1f3675d`) as four points at one A and one tool height:

1. from air beyond the end face (`air_x`) along X to the cutting X (`centre_x`), at the near edge of the band (`band.across.0`);
2. across the band to `band.across.1`;
3. back along X to air.

`routing::bands` sets `band.across` to the edges of the requested rectangle clipped to the band (`end/routing.rs:222-231`), so the tool centre starts **on** the edge, with half the tool over the requested area. The only engagement checks are that the slice thickness (the operation's **Stepdown**) is no more than the cutting diameter (`end.rs:129-131`) and the band spacing (the operation's **Stepover**) no more than the flute length (`end.rs:137-139`).

### What was measured (throwaway probe, not committed)

A probe ran End facing through the application on the public Baluster example (`testdata/examples/baluster`, its 3.175 mm one-flute flat tool, F1000, plunge 300) and swept the actual tool through a 0.1 mm voxel model of the stock, cut by cut, on `main` `1f3675d`. "Cross-section" is the material removed per millimetre of travel; ap is the extent of the cut along the tool; ae is cross-section over ap.

| Set-up | Leg 1 (sideways along X into the face) | Leg 2 (across) |
| --- | --- | --- |
| Cylinder stock 24 mm (the example's, lengthened to 66.3 mm so there is an end to face), Stepdown 1, Stepover 2.2225 (the tool's Hardwood preset) | up to 3.08 mm² (ap 2.2 mm, about 1.4 mm of the tool's width in the face) | up to 1.96 mm² (ap 2.3 mm, ae about 0.85 mm) |
| Box stock 35 x 35 mm (the #499 set-up in `crates/openchuck-app/tests/facing_approach.rs`), Stepdown 2, Stepover 2.2225 | up to 2.15 mm² | up to 0.68 mm² |
| Cylinder stock, Stepdown 3.17, Stepover 25 (both at their current limits), stock 69.33 mm long | up to 13.1 mm² (ap 11.8 mm) | 55.2 mm² mean over the leg: a slot the full tool width (ae = 3.17 mm = D) and **24 mm deep (7.6 x D)**, generated without a warning |

So:

- **Leg 1 cuts.** Where the band's edge is on the stock (every band on a round bar near its centre line, every band on box stock), leg 1 drives the tool sideways along X into the face with up to half its width engaged, through the full band height, and then turns 90 degrees in the material. On the round bar it removes 1.6 times the material per millimetre of the steady cut that follows. Where the requested area is narrower than the stock (Extent Part or Part plus tabs), the centre starts inside the stock and leg 1 is a full-width slot along X.
- **Engagement is not bounded** beyond ae <= D and ap <= flute length: a full-width slot as deep as the flutes passes every check.

### Stepdown and Stepover are filled the wrong way round by name

The tool's cutting data (TL-05; `crates/openchuck-types/src/tool/mod.rs:386-389`) defines **stepover** as the spacing *across the tool* and **stepdown** *along the tool axis*, and validates stepover <= diameter and stepdown <= flute length (`:413-427`). Decision 0230 fills an operation's Stepover and Stepdown from those values by name. End facing reads them the other way: its Stepdown is the slice along X, which is across the tool (a radial width), and its Stepover is the band along the tool (an axial depth); and its checks are the tool's checks swapped (`end.rs:129-139`). The preset's stepdown, an axial depth the tool library allows up to the flute length, therefore becomes End facing's radial width. No public example tool has a stepdown larger than its diameter, so no example is refused today; the meanings are still crossed.

### Found on the way (not in #641)

On the same round-bar set-up the post-time check refuses ordinary settings (both with the generator's own paths, nothing edited):

- **When the stock depth divides exactly into full-size slices** (Stepdown 1 or 1.5 with 3 mm to remove: "Facing written-motion verification: Facing final validation: an actual End cut exceeds the selected maximum axial stepdown"). The written X of the tool centre is rounded to 0.001 mm, and the radius 1.5875 has four decimals, so the read-back slice is 1.0005 mm against a 1 mm maximum (`facing/validation/end_layers.rs:120-123` allows only floating-point arithmetic). 65.5 mm or 66.3 mm of stock posts; 66.0 mm does not.
- **With a larger slice on a round bar** (Stepdown 2.2225 / Stepover 1, and Stepdown 3.1-3.17 / Stepover 10-25, 66.3 mm stock): "Facing completion: the actual end sweeps leave an unfinished required region in an axial layer". Not investigated further.

The owner put the first in scope of this work (item 4 below). The second is out of scope: its reproduction goes to the coordinator for a ticket (OI-2132).

## Decision (accepted)

![Two views looking down the tool axis at one End facing band: today the tool moves sideways along X into the end face with half its width in the material and turns 90 degrees in the material; under decision 0547 it comes down in air beside the stock, the approach distance clear, and cuts across only. Below, a view along Y shows the engagement check: the radial width (slice thickness along X) at most the Stepover and the axial depth (band along the tool) at most the Stepdown.](0547-end-facing-entry.svg)

1. **Each band starts and ends in air beside the stock where the stock allows it (question 1).** The across leg is lengthened at each end to a point where the whole tool is the approach distance clear of the stock (and of any protection) beside the band, the tool comes down there and returns there, and leg 1 and leg 3 along X go. The cut then starts with the tool's flank entering the face from its edge, its engagement growing from nothing to the slice thickness, with no sideways move into the face and no turn in the material. Where there is no air beside the band's edge (Extent Part or Part plus tabs narrower than the stock, or a band split by a tab), the band keeps its sideways entry along X, fed at the operation's plunge feed and written as an entry move (`LeadIn`), and is checked by item 2 as an entry. The lengthened sweep is cutter travel in air, not coverage: it is shown in the cutter-overlap preview (0252 section 3) and the completion check still compares the requested rectangle.
2. **Engagement is checked against the operation's own cutting settings (question 2).** On every across leg the radial width (the slice thickness) is at most the operation's radial setting and the axial depth (the band) at most its axial setting, measured on the assembled path (both after question 3). A sideways entry kept by item 1 is checked as a slot: its axial depth at most the axial setting. Exceeding one refuses the operation with a message in the 0536 form naming the setting, for example "End facing (operation 'End facing'): the band at A 0.000 is 24.000 mm deep along the tool, more than the Stepdown of 3.000 mm. A deeper cut would overload the flutes. Lower the Stepdown, or use a tool with cutting data for this depth." (NFR-07). No fixed engagement constant is invented: the tool's cutting data (TL-05) and the operation's edits are the limits. A split instead of a refusal is not proposed: the band and slice counts already come from those settings, so a correctly read setting never needs a split.
3. **Stepover is the radial width and Stepdown the axial depth (question 3).** End facing's slice thickness along X reads **Stepover** and its band spacing along the tool reads **Stepdown**, as the tool's cutting data defines them. The checks become those of TL-05: Stepover above zero and at most the cutting diameter, Stepdown above zero and at most the flute length. The End-mode label "Axial stepdown" (0252 section 5; UI specification Facing controls) becomes "Stepover (slice thickness along X)", and the band spacing is labelled "Stepdown (band along the tool)". A saved End operation keeps its path: on load, an End operation with an explicitly stored stepover or stepdown has the two values exchanged (a project-format migration, project schema 28, claimed in the work breakdown). Values the operation takes from its tool's preset are not stored, so they take the new meaning.
4. **The slice check allows for the written rounding (the first finding).** The check that an actual End cut is no deeper along X than the slice setting (`facing/validation/end_layers.rs:120-123`, the setting being Stepover after item 3) allows the written-motion share of the tolerance (an eighth of it, as the band edges already do, decision 0501), so a depth that divides exactly into full-size slices posts. The planned slices are unchanged.

## Questions for the owner

*Each question was answered A (8 Nov 2026, relayed by the coordinator); the options are kept as they were put.*

### Question 1: how a band enters the face

- **A. Enter from air beside the band where the stock allows, keep the sideways entry fed at the plunge feed where it does not (recommended).** Removes the sideways entry and the 90-degree turn for Whole stock, the default, on box and round stock. Cost: each band travels about one tool diameter plus two approach distances further, in air (about 5 mm per band for the Baluster tool). Part extents keep a slower, still-sideways entry.
- **B. A lead arc in the plane of the face, from air beyond the face into the cut, for every band.** No sideways entry anywhere, but the arc sweeps stock outside a Part extent (0252 sections 2 and 3 forbid cutting there) unless its radius is limited, and it still turns in the material.
- **C. Option A, and refuse a band with no air beside it.** Simplest proof, but End facing with Extent Part or Part plus tabs on stock wider than the part would be refused, a regression for those set-ups.

### Question 2: the engagement rule

- **A. The operation's radial and axial settings are the limits, checked on the assembled path, refusing with the setting named (recommended).** Uses the cutting data the user or tool library already gives (TL-05, 0230); no new setting or constant.
- **B. A fixed rule, for example axial depth at most 2 x D and a full-width slot refused.** Catches extreme values even when the user enters them, but invents limits no document gives, and refuses settings the user chose deliberately.
- **C. Warn instead of refusing.** Keeps every current path; does not meet 0252's "validated engagement".
- *Consequence common to A and B:* also allowing for the written-motion rounding in the existing slice check (the first "found on the way" item) is about one hour and is needed for the Baluster preset with a whole number of millimetres to remove. *Answered: in scope (item 4).*

### Question 3: which setting is which

- **A. Exchange them, Stepover = slice along X, Stepdown = band along the tool, and migrate saved End operations so their paths do not change (recommended).** Matches the tool's cutting data and 0230 by name. Needs a schema number and a migration test.
- **B. Exchange them without a migration, with a changelog entry.** Cheaper (no schema change), but a saved End operation with its own values changes its path on load, silently apart from the changelog (GC-07 holds only for unchanged inputs).
- **C. Keep 0252's meaning.** No change; the preset keeps filling the radial width with an axial depth.

## Question 4 (found while building; the owner's answer, 8 Nov 2026, relayed: option A)

![Three views looking along X at the end face with the stock 3.0 mm above the requested area and a Stepdown of 1.3 mm: today (and option C) the first band's flutes cut 4.3 mm deep, unflagged under C; option A adds passes of 1.3, 1.3 and 0.4 mm from the stock's top down to the area, across the area's width only; option B refuses. Below, why Whole stock is unaffected and why this is the usual case for a Part extent.](0547-question-4.svg)

Item 2 measures, on the assembled path, how deep each sweep cuts along the tool: from the stock's top at its view, or the bottom of the band above it, down to the tip. With a Part (or Part plus tabs) extent on stock taller than the requested area, the flutes of a slice's first band also cut the stock above the area (they always have), so that band cut (stock top - area top) + band spacing deep: always more than the Stepdown. Item 2 as first written would have refused End facing with a Part extent whenever the stock rises above the area, which is the usual case.

The owner chose **option A**: where the stock rises above the requested area at a view, the planner adds bands from the stock's top down to the area's top, each no deeper than the Stepdown, swept across the area's width only. The same material is removed as before, in steps the Stepdown allows. The user sees an information note in the 0536 form (topic "Stock above the area") saying how many passes were added and about how long they take at the cutting feed. Whole stock is unaffected: its requested area is the stock's outline, so nothing is above it.

The options put were: **A** as above (recommended); **B** refuse as item 2 read, so the user faces the stock down first; **C** measure the depth only within the area's heights, keeping today's cut unflagged. Before choosing, the owner asked whether End facing needs to be rotary at all: it is not (A stays fixed while cutting; on an XYZA machine with a vertical spindle the end face can only be cut with the flank), so the question stood.

## Test plan (written first, each watched failing)

`openchuck-path` (`tests/facing_end_plan.rs`) and `openchuck-app` (`tests/facing_approach.rs`, modules of each crate's `tests/integration.rs`):

- `req_op_24_req_op_11_end_band_enters_from_air_beside_the_stock`: Whole stock, box and round stock: no fed move along X inside the stock; every across leg starts and ends with the whole tool at least the approach distance clear of the stock (analytic, not the generator's helpers).
- `req_op_24_req_op_11_part_extent_band_keeps_a_plunge_fed_side_entry`: Part extent on wider stock: the X entry is a `LeadIn` at the plunge feed.
- `req_op_24_req_nfr_07_end_engagement_over_the_settings_refuses_naming_it`: the 24 mm band of the measurement above refuses, naming Stepdown, and the slice wider than Stepover refuses, naming Stepover; messages pass the 0536 topics check.
- `req_op_24_req_tl_05_end_reads_stepover_across_and_stepdown_along_the_tool`: a tool whose stepdown exceeds its diameter fills End from its preset without refusal; the slice equals the preset's stepover.
- `req_op_24_req_gc_07_saved_end_operation_keeps_its_path_after_migration`: an End operation saved at schema 27 gives the same path after loading at 28; a schema 27 file migrates its stored End values and leaves every other operation alone.
- `req_op_24_req_nfr_05_whole_number_of_slices_posts_with_both_posts` (item 4): the Baluster tool's preset (Stepover 1 after item 3) on 3.0 mm to remove posts through both posts (refused on `main` `1f3675d`).
- An independent sweep check (the voxel method above, added to `openchuck_testkit::engagement` from lane RINGSTEP as a new type with the axial depth, not a second module) that every cut's removed cross-section is at most Stepover x Stepdown.
- The existing End tests and the Baluster End posts (`facing_approach.rs`) updated for the new pass shape; determinism at 1, 2 and N threads with both posts (GC-07, NFR-06).

Every test is timed uninstrumented; none is expected to approach a minute.

## Alternatives considered

- **Use the shared entry styles (0026) for End bands.** They enter along or around the tool axis; an End band is cut by the flank across the axis, so a ramp or helix would plunge the tool's end into the next band.
- **Split a band that exceeds the rule into smaller ones automatically.** The band and slice counts already come from the settings; with them read the right way round (question 3) a split adds nothing a refusal naming the setting does not.

## Consequences

- `crates/openchuck-path/src/facing/end.rs` (pass shape, checks, parameter reading), `end/routing.rs` (air beside the band), `facing/validation/` (engagement on the assembled path); `crates/openchuck-app` (migration, labels); `docs/ui-design/ui-specification.md` (Facing controls: the two End labels); `docs/verification-plan.md` (OP-24 row: the new tests); `docs/decisions/README.md`; `docs/open-items.md`; the changelog. The library family's public API changes only if `EndFacingParameters` fields are renamed (`axial_stepdown`, `flute_step`): a workspace minor bump and a changelog entry (decision 0048).
- Golden G-code: no stored golden program is an End facing job; the End tests' expected paths change.
- Generated G-code remains the user's to verify before running it on a machine (NFR-12). Nothing here was run on a machine.

## Open items

- **OI-2130:** the owner's answers to questions 1 to 3. Closed: A for each (8 Nov 2026, relayed).
- **OI-2131:** project schema 28 for the End migration, claimed in the work breakdown; open until built.
- **OI-2132:** the second post-time refusal found on the round bar (larger slices refused as leaving an unfinished region): the reproduction goes to the coordinator for a ticket. The first is item 4.

## As built (lane PATHFEED, #641)

- **Item 1, the entry.** Where there is air beside a band's edge (the whole stock's projected rectangle at the band's heights does not reach past it), the band still comes down in air *beyond the end face*, as before, then moves in along X *beside the stock* with the whole tool the approach distance clear of it, sweeps across, and leaves along X the same way. Only the sweep across meets the face, entering it at its edge; the turns are in air. The record first drew the tool coming down beside the stock; the first build did that, but it put rapids, low links (decision 0501) and A changes beside the stock below the part's top, which the Facing rapid proof cannot certify. Keeping them beyond the end face keeps every existing link and rapid proof unchanged. The diagram above shows the path as built.
- **Item 1, no air beside the band.** The move along X into the face is kept and goes at the plunge feed as an entry move (`MoveKind::LeadIn`), and so does the move back out along X, which cuts the stock outside the area the same way. `strategy::Pass` gains `plunge_fed`, the indices of points reached so; assembly leaves such a pass unmerged at walls (decision 0050) so the indices keep their points. The shared step that cuts passes short at a held region (`keep_clear` in `strategy.rs`) carries each kept point's index over, so a plunge-fed point stays plunge-fed in the piece it lands in; the owner check of the release candidate found it dropped there (the side entry went at the cutting feed when the setup had a held region). This is a breaking change to the library: workspace 0.51.0 to 0.52.0 (provisional; the Merge Manager sequences versions).
- **Item 2.** `facing/validation/end_layers.rs` checks each across sweep's depth along the tool against the Stepdown, allowing the written-motion share of the tolerance, and refuses a deeper one naming the Stepdown. With question 4's answer, `facing/end/routing.rs` adds the bands above the area and `EndPassPlan::passes_above_area` lists their passes for the note. The note reads, for example, "Stock above the area (operation 'Face end, part only'): the stock rises above the requested area, so 2 passes from the stock's top down to the area are added, each no deeper than the Stepdown (band along the tool); they take about 3 s more at the cutting feed" ("1 pass ... is added" for one).
- **Item 3.** `EndFacingParameters::read` takes the slice from Stepover and the band from Stepdown; the field names `axial_stepdown` and `flute_step` are kept (no public rename). The 27 to 28 step in `openchuck-project` exchanges a saved End operation's stored values.
- **Item 4.** The slice check allows the written-motion share of the tolerance (an eighth), so the Baluster set-up with exactly 3 mm to remove at a 1 mm slice posts with both posts.

## Verification

**Before building.** The measurements in "Context" were taken on `main` `1f3675d` (8 Nov 2026), Linux x86_64 container, release build, with a throwaway example (`crates/openchuck-app/examples/probe_pathfeed.rs`, never committed) using only the public Baluster example and the #499 box set-up: engagement from generation alone (`start_regenerate`); the post-time refusals from `start_regenerate_with_post` with the LinuxCNC post.

**As built (lane PATHFEED, #641; branch `claude/lane-pathfeed-641-wip`, on `main` `645256c3`).** Each test was written from this record before the code and watched failing on the code before it, except where noted. Times are uninstrumented (`cargo nextest run`), Linux x86_64 container.

| Item | Test | What it checks | Seconds |
| --- | --- | --- | ---: |
| 3 | `facing_end_plan::req_op_24_req_tl_05_end_reads_stepover_across_and_stepdown_along_the_tool` | Stepover is the slice along X, Stepdown the band; a Stepdown of 7 mm on a 3 mm tool is not refused | 0.007 |
| 3 | `facing_end_plan::req_op_24_req_op_16_end_labels_name_the_slice_and_the_band` | The two End labels | 0.007 |
| 3 | `req_op_24_req_prj_04_req_gc_07_older_end_facing_operations_keep_their_slice_and_band` (`openchuck-project`) | A schema 27 End operation's stored values are exchanged, one stored value moves alone, Flat and roughing are untouched | 0.014 |
| 3 | `facing_end_plan::req_op_24_req_nfr_07_end_settings_beyond_the_tool_refuse_naming_them` | A Stepdown longer than the flutes, or a Stepover wider than the tool, refuses naming it (pins the new wording; the refusals existed) | 0.008 |
| 1 | `facing_end_plan::req_op_24_req_op_11_end_band_enters_from_air_beside_the_stock` | With air beside the band, both legs along X are the approach distance beside the box stock and only the sweep across crosses it (failed on the old shape) | 0.007 |
| 1 | `facing_end_plan::req_op_24_req_op_11_part_extent_band_keeps_its_side_entry` | With a Part extent the legs stay at the band's edges, inside the stock, marked plunge-fed | 0.007 |
| 1 | `facing_end_plan::req_op_24_req_op_11_part_extent_side_entry_goes_at_the_plunge_feed` | Those legs are written `LeadIn` at the plunge feed; the sweep across keeps the cutting feed | 0.011 |
| 2 | `facing_end_plan::req_op_24_req_nfr_07_end_engagement_over_the_stepdown_refuses_naming_it` | A path planned with a 4 mm Stepdown, checked against 2 mm, refuses naming the Stepdown | 0.017 |
| 2, Q4 | `facing_end_plan::req_op_24_req_nfr_07_part_extent_adds_bands_above_the_area` | Part extent on taller stock: bands step down from the stock's top by at most the Stepdown, with the "Stock above the area" note; Whole stock has no note (failed: a 9 mm first band against 4 mm) | 0.015 |
| 1 | `strategy::tests::req_set_09_req_op_24_plunge_fed_points_survive_the_held_region_check` (unit) | A clear pass keeps its plunge-fed points across the held-region check, and a pass cut short keeps those still in it at their new indices (failed first, as the owner check of the release candidate found) | 0.010 |
| 2, Q4 | `facing_end_plan::req_op_24_req_nfr_07_one_added_band_is_said_in_the_singular` | The note says "1 pass ... is added" for one added pass | 0.007 |
| 4 | `facing_approach::req_op_24_req_nfr_05_whole_number_of_slices_posts_with_both_posts` (`openchuck-app`) | The Baluster on its round stock, exactly 3 mm to remove at a 1 mm slice, posts with both posts (refused on `main`) | 14.4 |

Existing tests changed to the new pass shape or meaning, each still checking what it checked: `req_op_24_end_flank_uses_equal_axial_slices_and_top_first_flute_bands`, `req_op_14_req_nfr_12_end_air_point_keeps_the_approach_distance_from_the_end_face` (now from the stock, not only the end face), `req_op_24_req_op_14_end_slice_starts_and_ends_with_the_complete_tool_outside_stock`, `req_op_24_req_op_14_actual_assembled_end_rapids_have_continuous_complete_tool_clearance` (a rapid may now come down beside the round stock, the whole tool outside it; the deliberate rapid into the stock is still refused), `req_op_24_stored_end_settings_control_axial_depth_flute_spacing_and_start_view`, and `operation::req_op_24_req_ui_07_facing_forms_show_only_the_selected_modes_settings` (the labels).

Results: `openchuck-path`, `openchuck-app`, `openchuck-project` and `openchuck-cli` 1,982 of 1,982 passed (2,707 s, 46 slow, all existing). No stored golden program is an End facing job, so none changed. `cargo fmt`, `cargo clippy --workspace --all-targets` (clean), `cargo xtask deps` (with the message and topic rules), `licences`, `records`, `trace` and `messages` pass. Paths are deterministic at 1, 2 and N threads with both posts (`facing::integration_tests::req_op_24_req_gc_07_req_nfr_06_facing_paths_and_both_posts_are_identical_at_one_two_available_threads`, on the Part-extent project that now has the added bands).

Line coverage of the changed files on these crates' Facing, migration and descent tests (a lower bound): `facing.rs` 92.3 %, `facing/end.rs` 89.1 %, `facing/end/routing.rs` 90.2 %, `facing/validation/end_layers.rs` 84.7 %. Changed lines not run: the defensive branches for an empty point window, the band budget, no stock between a band's heights, a malformed operation file in the 27 to 28 step, and the post-assembly refusal of a slice deeper than the Stepover (the planner never makes one; item 4's allowance is tested through the post). The instrumented build was cleaned afterwards.

Not verified: no LinuxCNC replay was run for the new End programs, and nothing was run on a machine. Generated G-code remains the user's to verify before it is run (NFR-12).
