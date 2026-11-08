# 0548: A steep descent inside a pass goes down no faster than the plunge feed (lane PATHFEED; issue #643)

- **Status:** proposed by lane PATHFEED on 8 Nov 2026. Not accepted: questions 1 and 2 below are the owner's, put through the coordinator (decision 0049). Nothing in this record is built until they are answered.
- **What it amends, if accepted:** the shared path assembly's feed for moves inside a pass (`crates/openchuck-path/src/assemble.rs`), alongside [decision 0050](0050-walls-merge-micro-moves.md) (wall moves, which it leaves merged and timed as they are) and [decision 0026](0026-entry-moves.md) (entries, already at the plunge feed). The verification plan's OP-11 row gains the tests below. The requirements and the architecture are unchanged.
- **Requirements:** OP-11 (feeds), OP-03 (rotary roughing), GC-07 / NFR-06 (golden programs and determinism), NFR-12.
- **Date:** 8 Nov 2026.
- **Open items:** OI-2133 and OI-2134, proposed below (provisional numbers from the coordinator; the Merge Manager assigns final numbers at release).

## Context

Inside a pass the shared assembly writes every move as `MoveKind::Cutting` at the operation's cutting feed (`assemble.rs:596-603` on `main` `1f3675d`). That includes a move that goes down along the tool axis: a wall step `walls::merge_at_walls` keeps (`assemble/walls.rs:187`), or the drop where rotary roughing comes down off a raised floor to the layer (`roughing.rs` `run_points`, `:976`, the tip is `floor.max(level)`). Entries already go down at the plunge feed (0026: plunges, ramps and lead moves; `assemble/entry.rs`).

The feed of a cutting move is the feed at the contact point (decision 0050: a move is timed by its end contacts). When the contact jumps further than the tip, as it does down a wall, the tip already moves slower than the programmed feed. So the question is what the tool's speed **along its axis, downwards** is, as written.

### What was measured (throwaway probe, not committed)

The probe ran each job through the application on `main` `1f3675d` and, for each cutting move that goes down, took the tip's descent along the tool axis and its travel across, the angle of the move from the across direction, and the written tip speed (tip length over the move's time). Makera Z1 profile, 4 threads.

| Job | Descending cuts steeper than 45 degrees | 30-45 degrees | Descending cuts whose speed along the axis exceeds the plunge feed | Largest such speed |
| --- | ---: | ---: | ---: | ---: |
| Baluster example, Rough (rotary roughing, 3.175 mm flat, F1000, plunge 300; programmed 804.8 s) | 5 | 7 | **0** | 278 mm/min (the drops off the raised floor already come down at 231 mm/min or less, timed by their contacts) |
| Public `testdata/stl/blisk.stl` in a 42 x 52 mm cylinder, the same Rough operation (programmed 3,128 s) | 80 | 46 | **193** | **600 mm/min** (the Z axis limit, twice the plunge feed), 18 mm of near-radial descent (> 80 degrees) at up to 599 mm/min |
| Baluster example, Finish (helical finishing, 3.175 mm ball, plunge 200) | 0 | 0 | 212 | 254 mm/min (all shallower than 30 degrees) |

Feeding every descending blisk cut steeper than 45 degrees at the plunge feed adds 7.8 s (0.25 %); steeper than 30 degrees, 10.8 s (0.35 %). The probe does not know whether the material under each descent is uncut; on a roughing layer it is by construction where the drop ends at the layer, so the counts are an upper bound on the moves the issue describes.

The stored golden programs (LinuxCNC set, counted from the files: descending G1 moves that are not pure plunges, against each program's plunge F word):

| Program | Steeper than 45 degrees | Axial speed above the plunge feed |
| --- | ---: | ---: |
| `golden/blisk/axial`, `helical_2`, `helical_5` (finishing) | 1,355-1,859 | 1,859-2,541 |
| `golden/reference/cone_indexed` | 36 | 58 |
| `golden/reference/lobed` | 0 | 47 |
| `golden/post_linuxcnc_small_part.ngc` | 6 | 12 |
| `first_cut`, `cone`, `cone_axial`, `cone_axial_lead`, `cone_held_tabs`, `drilling`, `face_flow`, `groove`, `sphere`, `swarf` | 0 | 0 |

No stored golden program is a rotary roughing job. A rule for every strategy changes six of them (twelve files with both posts) and slows finishing where the material ahead is only the stock to leave.

## Decision (proposed)

1. **Where.** In the shared assembly, on the moves inside a pass (after the wall merge of 0050 and the contact smoothing), for the strategies question 1 selects. Entries, exits, links and a pass with rapid points (drilling, 0050 item 1) are unchanged.
2. **The rule (question 2).** A cutting move that goes down along the tool axis is fed so that its tip's speed along the axis is no more than the operation's plunge feed: its time is at least its descent divided by the plunge feed. A move already that slow (most drops on the Baluster, timed by their contacts) is unchanged; a move that is not would be written with a lower F word. It stays a `Cutting` move with its contact, so the simulation, the contact smoothing and 0050's wall merge are unchanged. Only ever slower, never faster.
3. **Golden programs.** Each program that has such a move is re-blessed in the same pull request, with the count of changed moves per program stated there (GC-07: the same project and version still give byte-identical G-code; the version changes).

## Questions for the owner

### Question 1: which strategies

- **A. Roughing only: rotary roughing, and Facing's Flat and Round modes, which build on its passes (recommended).** That is where a descent goes into stock this layer has not cut. No stored golden program changes (none is a roughing job); the blisk roughing above changes 193 moves, +7.8 to 10.8 s. Implemented as a provided method on the `Strategy` trait (default off) that those strategies turn on, so no public struct gains a field.
- **B. Every strategy.** One rule everywhere; finishing descents too. Changes the six golden programs above (twelve files) and slows finishing by up to several percent on the blisk, where the load is only the stock to leave (the issue's own note).
- **C. Roughing now, finishing later if the owner wants it** (A, plus an open item to revisit finishing after the release).

### Question 2: which moves count as "mostly along the tool axis"

- **A. No angle threshold: cap the speed along the axis at the plunge feed for any descending move (recommended).** Continuous; a move changes only if it really goes down faster than a plunge; with F1000 and plunge 300 only moves steeper than about 17.5 degrees can be affected. Matches "at the plunge feed, or no more than it" in the issue.
- **B. Steeper than 45 degrees (more along the axis than across): the whole move at the plunge feed,** as a ramp is (0026). A simple rule to state, but a jump in feed at 45 degrees, and a 44-degree move still goes down at up to 0.7 x the cutting feed (700 mm/min here, above the plunge feed).
- **C. Steeper than the operation's ramp angle (0026): the whole move at the plunge feed.** Ties the rule to an existing setting, but the default ramp angle is 3 degrees, so nearly every descending roughing move would be slowed to the plunge feed.

## Test plan (written first, each watched failing)

`openchuck-path` (unit tests in `assemble.rs`'s test module and `tests/`, as modules of `tests/integration.rs`):

- `req_op_11_req_op_03_roughing_drop_off_a_raised_floor_goes_down_no_faster_than_the_plunge_feed`: a synthetic cylinder with a raised band, rotary roughing: every descending cutting move's axial tip speed is at most the plunge feed (checked from the IR's times and positions, not from the assembler's helper).
- `req_op_11_shallow_descent_and_level_moves_keep_the_cutting_feed`: moves below the limit are byte-identical to before.
- `req_op_11_finishing_descent_is_unchanged` (question 1 A or C): the same surface finished keeps its feeds.
- `req_op_13_req_op_14_wall_moves_still_merge_and_pass_gates_1_and_2`: the 0050 wall tests still pass, with both posts (the F word never exceeds a limit after the change).
- `req_gc_07_req_nfr_06_descent_feed_is_deterministic_at_1_2_n_threads`: both posts.
- The blisk roughing count above as an application test (`crates/openchuck-app/tests/`), if it runs in well under a minute uninstrumented (it generated in 2.7 s in release).

## Alternatives considered

- **Write such moves as `Plunge` moves.** A plunge has no contact (0026), so the contact path, its smoothing and 0050's merge would change for every strategy, and the simulation would treat the move differently. A feed cap on a `Cutting` move changes the time and the F word only.
- **Clamp in the post.** The post sees no plunge feed per move and no pass structure; 0050 already rejected fixing feeds in the post.
- **Time a wall move by its tip (OI-211).** Separate question; this rule only ever slows a move and works with either timing.

## Consequences

- `crates/openchuck-path/src/assemble.rs` (the per-pass loop and a helper), `strategy.rs` (with question 1 A or C, a provided method, default off: an addition, not a breaking change), the roughing strategy's implementation of it; golden programs as question 1 decides; `docs/verification-plan.md` (OP-11 row); `docs/decisions/README.md`; `docs/open-items.md`; the changelog.
- Coordinated with lanes RAMPENTRY (owns `assemble/entry.rs`, not edited here) and RINGSTEP (owns ring steps in `roughing.rs`, `helical.rs`, `simultaneous.rs`, `extend.rs`, not edited here; a ring step that goes down steeply inside a roughing pass falls under the rule).
- Generated G-code remains the user's to verify before running it on a machine (NFR-12). Nothing here was run on a machine.

## Open items

- **OI-2133:** the owner's answers to questions 1 and 2.
- **OI-2134:** with question 1 C, whether finishing descents get the rule after the release.

## Verification

Measured on `main` `1f3675d` (8 Nov 2026), Linux x86_64 container, release build, with a throwaway example (`crates/openchuck-app/examples/probe_pathfeed.rs`, not committed) on the public Baluster example and the public `testdata/stl/blisk.stl` (placed in a copy of the Baluster project with a 42 x 52 mm cylinder and no workholding), and a throwaway script over the stored LinuxCNC golden programs. Nothing was run on a machine.
