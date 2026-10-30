# 0363: Simultaneous finishing: held ends, walls and jumps, and off-axis sections (lane SIM4AX)

- **Status:** proposed. The owner chose B1 (tracks outside the held bands), B2 (walls and better refusal advice) and D1 (off-axis sections driven round their own centre, item 3) and accepted the A reversals at concave fillets (D4) on 30 Oct 2026, relayed by the coordinator session (session_01CFw3bzew7zizFQ91L5tvT7), decision 0049. This record's text, including the fitter changes item 2 adds beyond the wall test, awaits the owner's acceptance.
- **Date:** 30 Oct 2026.
- **Lane:** SIM4AX (issue #411), branch `claude/lane-sim4ax`.
- **Requirements:** OP-05, OP-06, OP-12, OP-15, SET-09, OP-20 (item 3; its wording is an owner decision, see *Open items*). NFR-06 and GC-07 unchanged: the output stays deterministic.
- **Builds on:** [0036](0036-slice-13-simultaneous-finishing.md) (the strategy), [0050](0050-walls-merge-micro-moves.md) (wall moves), [0133](0133-simultaneous-fitter-tests-the-quarter-points.md) (the fitter's quarter points), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (the held keep-out band), [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (the turns at faces).
- **Open items:** OI-1430 (new). OI-1200 closed. OI-337 and OI-318 progress. OI-1372 (on lane ENDFACE's branch, not yet on `main`) is closed by item 1 once that branch merges.

## Context

Issue #411: on the owner's CrankShaft the simultaneous toolpath looked incoherent. The owner expected the tool to stay as perpendicular to the part as possible, sliding in Y on flats and turning A on rounds. That is the intent of decision 0036 (item 2: the tool axis on the normal's part across X; OP-06). The lane's investigation (Phase 1, numbers on the committed `testdata/stl/crankshaft.stl` and the owner's STEP, Z1 3.175 mm ball, defaults) found:

- On the web the strategy does what was intended: A holds while the tool slides on the flats, and A turns on the rounds.
- The three pins are centred 10 mm off the rotary axis. A ray from outside towards the axis meets a pin over only about 90 degrees of each turn, so each turn was cut as its own pass (about 234 degrees of A over 0.11 mm of X), with a retract to safe Z between turns: 80 passes and 75 retracts. The pin side facing the axis (about 126 degrees) was never cut by the spiral (decision 0036 item 1's accepted limit, OI-148, requirement OP-20). Item 3 addresses this.
- At the web's concave r 2 fillets, following the normal turns A back by about 36 degrees twice on each side per turn. That is the geometry (the owner accepted these reversals, D4). On the STL the ball touched the flat beside a fillet with the tool axis up to 35 degrees off its normal: the smoothed mesh normal lags round the fillet. On the STEP the tool axis is within 1.5 degrees of the normal everywhere.
- OI-1372 (the owner's blisk project refused "would cut 7.5 mm" at X 6.588 beside a held start): reproduced with no owner data. The spiral ran from the drop surface's lowest X, which includes the tab from the held region's inner edge, so it drove inside the held keep-out band and over the held stock's face. There the move test read the step as a cut (the OI-1200 / OI-337 item 2 mechanism).

## Decision

1. **The tracks start and end outside the held bands (B1, OI-1372).** The spiral and the axial passes run over the part's X range less the held keep-out bands that cover its ends (`StrategyInput::keep_out`, decision 0161), as the turns at faces already do (`faces.rs`). Nothing changes without a held region.
2. **Walls and jumps in the move test (B2, OI-337, OI-1200).** Decision 0036 item 7 halves a move until the tool at its middle and quarter points is within the chord, and refuses one that still cuts in after 12 halvings. Removing the refusals at walls exposed places where that test passed a move that cuts. So all of the following come together, because a refusal must not become a gouge (OP-12):
   - **Wall move.** After the last halving, a move whose tip moves across the tool axis, and whose A turn moves the tool's edge, by less than the tolerance is a wall move (decision 0050 item 2) and is kept. A move along the tool's own axis sweeps only the tool's body above its lower end. Shoulders, held faces and a tab meeting the part make the drop height jump at a wall, which no halving removes.
   - **A flat bottom on the normal.** A flat end or bull-nose whose axis is within `FLAT_SEED` (1e-3, as a sine) of the normal is seeded with its centre on the drive point. Before, the rounding of the planned axis (a few 1e-6 rad) put the corner on the drive point on either side at random, a jump of the tool's diameter between neighbouring points (OI-337 item 1, in this narrow form). A round bottom (a ball) is unchanged.
   - **Jumps go through the raised tool.** After the last halving, a move that still cuts in by more than the tolerance, or still moves the tip further than the tool's radius, is not a fitted move: the drive has passed from one surface to another (a concave corner, a blade root). It is kept as two moves through the tool at the straight move's middle, raised along its axis until it touches, and each half is checked the same way, up to `RAISES` (12) more levels. Only what still cuts in after that is refused. The raised pose touches the surface, so the moves through it are checked like any other.
   - **Long moves and blind test poses.** A move whose tip travels further over the part than the tool's cutting radius (the larger of the straight distance and A's arc at the tip's radius) is also tested at a pose every quarter of that radius along it (at most 256), and split if one of them fails. A move is also split while a middle or quarter pose meets no surface. Before, three test poses up to 2 mm apart all missed a blade, and a 54-degree, 9 mm move through it was kept. A long move that holds at every pose is kept whole, so a smooth part's output does not change: at a sphere's poles, where A turns 85 degrees between the first seeds, the path is the same as before.
   - **A cut in is read at no less than half its depth.** The drop at a test pose is taken along the drive normal (decision 0036 item 7). On a face steep along X (a blisk's blade) that normal is nearly across the tool axis, and a cut of tenths of a millimetre read as thousandths. A cut in is now read at no less than `MIN_ALONG` (0.5) of its depth along the axis; a wall the tool only slides on is a wall move.
   - **The refusal's advice.** What still fails names the jump: "the tool jumps between two surfaces there (a sharp edge, or a wall the drive meets), so a smaller largest tilt or angle step does not help: limit the operation to a region away from it, or finish that part with helical finishing".
3. **Off-axis sections (D1, OP-20).** To be written with stage 2, after the plan and the OP-20 wording are agreed.

## Alternatives considered

- **Only the wall test (B2 as first proposed).** It turns refusals into programs, and on the blisk those programs overcut by up to 1.7 mm in simulation. Rejected for OP-12.
- **Seeding every tool from the track position and the planned axis (OI-337 item 1's proposal).** It makes the seed continuous everywhere, but it changes the output of every part, the `lobed` golden included. The narrow flat-bottom rule and the raised middle cover the jumps found here; the general form stays with OI-337.
- **A cap on A's step per move.** The `lobed` job already takes A steps up to 10 degrees with a largest angle step of 2, so a cap tight enough to matter would move its golden file. The tip-travel limit bounds the same thing in millimetres without changing `lobed`.

## Verification

Tests named after the requirements (AGENTS.md), written first and seen failing:

- `openchuck-app/tests/held_region.rs` `req_set_09_req_op_05_simultaneous_finishing_beside_a_held_start_with_tabs_generates`: the cone held at both ends with the default tabs, round and box stock. Before: refused at X -6.623 (the held face at X -8 plus the ball radius). After item 1 alone: refused at X -1.5 (the part's own face, where the tab meets it), then at X 30.905 (the concave corner at the tail tab). After item 2 it generates, keeps clear of the gripped length and the simulation leaves the held regions whole. 77 s.
- `openchuck-path/tests/helical_faces.rs` `req_op_05_req_op_12_simultaneous_finishing_crosses_the_shoulders_of_a_stepped_shaft` (OI-1200's shaft): the ball and the flat generate. Before: refused at X 8.413, 1.94 mm (flat) and 1.21 mm (ball). Nothing is cut into the faces or rounds (independent check against the tool's outline); the flat leaves at most 0.05 mm on the rounds and the ball its scallops. 13 s.
- `simultaneous.rs` unit test `req_set_09_the_tracks_start_and_end_outside_the_held_bands`.
- Unchanged: every simultaneous test (the sphere's path at the poles included: a first version split every long move, which moved the tool up to 0.097 mm off the axis there; testing long moves at more poses instead keeps it at 0.017 mm), the region tests, the turns at faces on the crankshaft, the held-region tests of every strategy, and the reference jobs. The `lobed` golden programs are byte for byte the same, and identical at 1 and N threads.
- Through the command line (not committed; the owner's project data is not used): the blisk at X 9.175 on box stock with a 5 mm held start and the default tabs (the OI-1372 set-up), with a 3.175 mm flat end and a 1 mm stepover, now generates. In simulation the deepest overcut is 0.28 mm, and every overcut place is on the plane X 60.000, the stock's and tail tab's end. Helical finishing on the same project overcuts that plane by 0.25 mm, so it is not from this strategy (OI-1430). The crankshaft, STL and STEP: no overcut beyond one 0.1 mm cell.

## Consequences

- `openchuck-path::simultaneous`: private changes only, plus the new public constant `FLAT_SEED`. That is an addition, so no version bump (decision 0048).
- No golden file changes. Programs change only where the job was refused before, at a held band, or where the new tests apply (a long move that fails at one of its extra poses, a blind test pose, a flat bottom on the normal).
- `testdata/step/valid/crankshaft.step`: the owner's CrankShaft STEP, with its row in `testdata/README.md`.

## Open items

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | The overcut at the stock's and tail tab's end plane (X 60.000 on the blisk set-up of OI-1372): 0.28 mm under simultaneous finishing, 0.25 mm under helical finishing. (OI-1430) | A lane that owns the ends (ENDFACE or after it) | A reproducer in a test |
| 2 | The OP-20 wording for item 3: a new P1 requirement for off-axis sections, or a note under OP-20 (P2). | The owner, through the coordinator | |
