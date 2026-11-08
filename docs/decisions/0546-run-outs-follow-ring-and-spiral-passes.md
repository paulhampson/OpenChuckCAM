# 0546: Extend past the stock carries a ring or spiral pass on in its own motion, not along X (OP-24, OP-04, OP-11, OP-12, OP-14, OP-15, GC-07, NFR-06, NFR-07, NFR-12; issue #644; lane RINGSTEP)

- **Status:** proposed by lane RINGSTEP on 8 Nov 2026. Not yet accepted: the owner accepts the record when they have read it in full (decision 0049). The owner's answers were relayed by the coordinator (session_01CFw3bzew7zizFQ91L5tvT7) on 8 Nov 2026, each the recommended option: Q1 option 1 (continue the pass at its own spacing out to the extended end, with no corner; the longer run-out is accepted, and is stated in the note and in the user guide) and Q2 option 1 (saved helical finishing operations with Extend on get the new run-out, with a changelog and release-notes line). The build follows these answers. The number is provisional: the Merge Manager assigns the final number at release.
- **Date:** 8 Nov 2026.
- **Issue:** #644, "Extend past the stock on ring and spiral passes: the run-out goes along X, across the direction of cut, then turns 90° into the cut at depth" (item 8 of the #638 survey).
- **What it amends, if accepted:** [0212](0212-roughing-direction-and-extend-past-the-stock.md) part B, design item 3 ("Helical, axial and indexed finishing: … past the end of the part the pass runs on at the height of its last point, flat, until the tool clears the stock end") and "As built" item 4 (the straight run-out along X), **for helical finishing only**. Axial and indexed finishing, whose cut runs along X, keep the run-out along X.
- **Builds on:** [0390](0390-helical-finishing-in-rings.md) item 5 ("Extend past the stock (0212) adds rings out to the extended ends", which the code does not do: see Context), [0545](0545-blended-steps-between-rings.md) (blended steps between rings, proposed with this record), [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md), [0345](0345-finishing-layers.md), [0521](0521-low-links-for-helical-rest-and-simultaneous-finishing.md).
- **Related:** #639 and #640 (entries and leaves that follow the toolpath, lane RAMPENTRY, its proposed records 0543 and 0544): the run-out is cut before the entry and after the leave, so they meet the pass where this record leaves it. Lane RAMPENTRY reads a run-in that continues the helix as a spiral (its ramp then runs round the axis before the first point) and an added ring as a ring.
- **Open items:** new OI-2113 (acceptance of the record; the owner's answers Q1 and Q2 were given on 8 Nov 2026, relayed). It also records, and if accepted closes, an unrecorded divergence: 0390 item 5 says the rings go out to the extended ends; as built they do not.

## Context

With **Extend past the stock** on, helical finishing carries a pass that starts or ends at an end of the part on to the extended end (`extend.rs`, `Extension::run_out`, called from `helical.rs` for both the spiral and the rings). The run-out (`Extension::line`) goes **along X**, at the A and the height along the tool axis of the pass's end point, raised over the target where it stands higher (a tab, a held solid). The extended end is the stock's end plus the tool radius plus the set distance (`Extension::read`), so the run-out covers the stock past the part's end as well as the clearance.

That suits axial passes, whose cut runs along X. On a ring or spiral pass, X is across the direction of cut:

- **At the start:** the tool runs along X from the extended end toward the part at the first ring's depth, side-milling whatever stock is left between the stock's end and the part, then turns 90 degrees into the ring or spiral in material. That is the corner of [0545](0545-blended-steps-between-rings.md), at the pass's first ring.
- **At the end:** the same in reverse.
- **It leaves the overhang at every other A.** The line cuts one strip about a tool diameter wide round the part, at one A: on a 10 mm radius with a 3.175 mm tool, about 5 % of the circumference. Past the reach of the end ring (the part's end plus the tool radius) the rest of the stock between the part and the stock's end is not cut by finishing at all. OP-24 asks that the tool "leaves the stock fully and leaves no skin or witness line"; on a ring or spiral pass today it does so only along that strip. (Read from the code, not measured.)
- **0390 item 5 says otherwise.** It records that Extend past the stock "adds rings out to the extended ends". As built, `ring_places` covers the part only and the run-out is the X line, so that sentence does not match the code. This was not recorded as a divergence until now.

Rest machining and simultaneous finishing do not extend past the stock (0212 part B, item 3), so they are not affected. Rotary roughing and round Facing widen their range to the extended end (0212 "As built" item 4), so their rings or spiral already continue in their own motion, and are not affected.

## Decision (proposed; the owner's answers Q1 and Q2 relayed on 8 Nov 2026)

### Q1: the shape of the run-out

**Option 1, continue the pass at its own spacing (the owner's choice, relayed 8 Nov 2026; the lane's recommendation).**

1. **Spiral.** The helix is carried on past each end of the part it reaches, at its own pitch and the Rotation's sense, until the tool's tip reaches the extended end: the run-in starts at the extended start, `360·g/p` degrees before the pass's first point (`g` the gap from the part's end to the extended end, `p` the pitch), and ends at that point; the run-out at the far end is the same continued forward. A turns one way throughout (OP-15) and builds up `g/p` more turns at each end.
2. **Rings.** Rings at the operation's own ring spacing are added past each end it reaches, out to the extended end, the last exactly at it, joined by the operation's own steps: blended or sharp, as **Steps between rings** says (0545). This is what 0390 item 5 says.
3. **Height.** Each point of the run-out is at the height along the tool axis of the pass's end point (today's rule, unchanged), raised over the target where it stands higher, so it rises over a tab or a held solid rather than cutting it (0212 "As built" item 4). An added ring is at the end ring's height at the same A, which on a part whose end is not round keeps the shape of the end ring.
4. **The checks are unchanged.** A run-out towards a held end is refused as today (SET-09); the application's travel check covers the longer X range unchanged; low links (0521) and finishing layers (0345) treat the run-out as part of the pass, as today. A note gives the turns or rings added at each end and the length of path they add, and the user guide's Extend past the stock section says that on a ring or spiral pass the run-out is that much longer (the owner accepted the time).

Consequences of option 1: no corner in the cut, and the stock between the part and the extended end is cleared all round at the pass's own stepover, as on the part. It is longer: for a 3.175 mm tool at the default distance (0.3 mm), with 2 mm of stock past the part's end, the gap is about 3.9 mm, which at a 0.3 mm finishing pitch is 13 turns at each end, about 820 mm of path at a 10 mm radius (about 49 s at 1000 mm/min), where today's line is 3.9 mm long.

The options not chosen:

- **Option 2, one turn out, then one helix piece into the pass.** A whole turn (a ring) at the extended end, then a single helix piece over one turn from it to the pass's first point (on rings, the 0545 blend, but over the whole gap). Short (about two turns per end) and no 90-degree corner (about 3.5° off the ring for the example above), but the overhang between the extended end and the part is cut only along that one helix piece, so it keeps a skin at the other angles, much as today.
- **Option 3, continue at a coarser spacing.** As option 1, but past the part the turns or rings are no further apart than the tool's radius (or the pass's own pitch where that is wider): the overhang is cleared all round with a scallop no deeper than about 0.13 of the tool's corner radius on a ball end, in fewer turns (3 instead of 13 in the example, about 11 s). It adds a rule of its own and a change of pitch where the run-out meets the part's end.
- **Option 4, keep the line along X and add a note or a warning.** Documents the corner and the skin but changes neither.

### Q2: operations already saved with Extend on

**Option 1 (the owner's choice, relayed 8 Nov 2026; the lane's recommendation): the new run-out for every helical finishing operation with Extend on, saved or new**, with a changelog and release-notes line. Today's line misses most of the overhang (Context), which OP-24 asks to clear, and no golden, reference or example program uses Extend (a search of `testdata/` on `main` `1f3675d`), so no golden file changes. GC-07 promises the same G-code for the same project, post and software version; the version changes.

The option not chosen: keep today's line for an operation saved without a new marker, and write the marker into new operations as 0545 item 1 does. That needs a parameter the user would see only to keep an old behaviour (for example **Run-out past the stock**, `run_out = line | follow`); not recommended.

### Common to every option

5. **Strategies.** Helical finishing only (spiral and rings). Axial and indexed finishing keep the run-out along X. Rest machining and simultaneous finishing still do not extend (0212). Rotary roughing and round Facing are unchanged.
6. **Determinism (GC-07, NFR-06).** The run-out is arithmetic on the stock's size, the distance, the pitch and the pass's end points, and each drop of the tool is independent, so the output is identical at 1, 2 and N threads.
7. **NFR-12.** The wording says the run-out carries the pass past the stock in its own direction; it does not say the program is safe or checked: generated G-code remains the user's to verify before running it on a machine.
8. **Library (0048).** No public API change is expected (`Extension` is crate-internal). The output of saved operations with Extend on changes (Q2, recommended), so it shares 0545's minor version bump and changelog entries if they merge in one release; otherwise it takes its own.

## Consequences

- **Code** (lane RINGSTEP): `crates/openchuck-path/src/extend.rs` (a run-out that continues a helix or adds rings, beside the existing `run_out` and `line`, which axial finishing keeps); `helical.rs` (the call site, the ring places past the part and the note). Lane RAMPENTRY's entry code is not touched: the pass it receives now starts and ends past the stock in its own motion.
- **Documents** (proposed here, edited in the build pull request after acceptance; the requirements and architecture are unchanged): 0212's "As built" gains a pointer to this record for helical finishing; the verification plan's OP-24 row adds "on a ring or spiral pass the run-out continues the pass's own motion and clears the stock past the part all round"; the UI specification is unchanged (no new control under the recommended options).

## Verification

### Done for this proposal (8 Nov 2026)

- Read on `main` `1f3675d`: `extend.rs` `run_out` and `line`, the call sites in `helical.rs` and `axial.rs`, `Extension::read` (the extended end is measured from the stock's end), and `ring_places` (the part only). The figures in the Context and under Q1 are arithmetic on those rules (tool 3.175 mm, distance 0.3 mm, 2 mm of overhang, 10 mm radius), not measurements of the product.
- Not done: no product code changed or run; nothing run on a machine.

### To do when built (tests written first, each watched to fail), in `crates/openchuck-path/tests/extend.rs`

- `req_op_24_a_spiral_run_out_continues_the_helix_at_its_pitch` (option 1: the X/A relation of the run-out points is the helix's, through the extended end)
- `req_op_24_a_ring_run_out_adds_rings_out_to_the_extended_end` (the rings at the ring spacing, joined by the operation's steps, sharp and blended)
- `req_op_24_ring_and_spiral_run_outs_leave_no_skin_past_the_part` (the simulated stock between the part's end and the stock's end is cleared all round, graded with the test kit's dexel stock; today's line fails it)
- `req_op_15_run_outs_turn_one_way`
- `req_op_12_run_outs_rise_over_a_tab_and_never_gouge`
- `req_op_24_axial_run_outs_are_unchanged` (byte-identical)
- `req_gc_07_req_nfr_06_run_outs_identical_at_1_2_and_n_threads`
- `req_set_09_a_run_out_towards_a_held_end_is_still_refused`

Each timed uninstrumented; the local coverage check of `openchuck-path`; `cargo xtask deps`, `licences`, `records`, `trace` and `messages`.
