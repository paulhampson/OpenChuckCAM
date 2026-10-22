# 0213: Requirement OP-25, climb, conventional and meander cutting (lane CUTOPTS; issue 251, F-81)

- **Status:** accepted (22 Oct 2026, by the owner directly in the lane CUTOPTS session: "Accept 0212 and 0213"). The owner agreed OP-25 as worded below at P2, with the spindle kept at `M3` only, climb and conventional made by the feed direction and `M4` later as a tool property (option A, relayed by the coordinator, 22 Oct 2026; OI-855 closed). OP-25 and its verification-plan row are added to the documents in this change, on that agreement. Nothing is built (the owner's decision on issue 251: "P2 requirement proposal only, nothing built").
- **Date:** 22 Oct 2026.
- **Requirements:** adds OP-25 (P2); relates to OP-03, OP-04, OP-05, OP-07, OP-08, OP-09, OP-10, OP-11, GC-07, GC-12, NFR-06, NFR-07, NFR-12; issues 246 (pass direction, decision 0212) and 249 (spindle speed, decision 0196).
- **Open items:** OI-855 (closed, option A), OI-856 (the build, deferred).

## Context

Issue 251 (trial feedback F-81, the owner's request) asks for an operation option that sets the cutting direction relative to the tool's rotation: **climb** (the cutter turns with the feed, the tooth enters at its thickest chip), **conventional** (the cutter turns against the feed) and **meander** (passes alternate direction, climb on one and conventional on the next, with no return move between them). The current behaviour stays the default, so existing projects and their G-code do not change (GC-07, NFR-06).

What the code does today, for the proposal's baseline:

- **Spindle direction.** Both built-in posts write `M3` (clockwise seen from the spindle) for every tool (`openchuck-post/posts/*/main.post`); nothing in the project, the tool or the machine profile can ask for `M4`. The setup sheet (GC-12, decision 0110) does not show the spindle direction.
- **Pass direction.** Each strategy fixes the order and direction of its passes: rotary roughing cuts its continuous helix one way and its indexed lines all in +X; helical finishing turns one way; axial and indexed finishing take a `direction` choice of `zigzag` (alternate) or `climb`, where `climb` means only "every pass the same way" and is not checked against the spindle or the side the material is on.
- **OP-21 (P1) is the last ID on `main`; OP-23 is proposed on the STEPALL branches (face sets) and OP-24 by decision 0212 (extend past the stock), so this record took OP-25,** which the owner agreed at P2.

Climb and conventional are defined by the side the uncut material lies on, relative to the feed direction, and by the spindle direction: with `M3`, a cut with the material on the tool's right (looking along the feed, from the spindle) is climb, on its left conventional; `M4` swaps them. On a part turning about X, the "feed direction" is the tool's motion relative to the part, which combines X, Y and the turning of A, so a strategy that reverses A reverses the cut.

## Requirement (as the owner agreed it, at P2)

> **OP-25 (P2).** Cutting direction: an operation can ask for **climb**, **conventional** or **meander (both)** cutting. For each pass, the side of the tool the uncut material is on, the feed direction relative to the turning part, and the spindle direction decide which it is. Strategies that can honour the choice order and direct their passes to meet it; a strategy, or a part of a path, that cannot (the tool cuts on both sides, or on neither) says so in the operation and does not claim a direction. The spindle direction (`M3` or `M4`) and the cutting direction of each operation are shown in the operation inspector and on the setup sheet. Each strategy's default is the direction it cuts today, so existing projects and their G-code do not change.

Verification-plan row (added to `docs/verification-plan.md`):

> | OP-25 | P2 | T, I | Unit, system | On an analytic cylinder and a stepped bar, for each strategy that honours the choice: climb and conventional give passes whose feed direction relative to the turning part puts the uncut side on the right (climb with `M3`) or left (conventional) of the tool, checked from the IR and the simulated engagement; meander alternates the side pass to pass with no return link; `M4` swaps the result; a strategy that cannot honour it reports so and its path is unchanged; the default leaves every golden file byte for byte unchanged (GC-07); the setup sheet and the inspector show the spindle and cutting direction (inspection) |

## Proposed rules per strategy

How the direction is decided, and which strategies cannot honour it (NFR-07: said plainly, never silently ignored):

| Strategy | How the direction is decided | Climb / conventional | Meander |
| --- | --- | --- | --- |
| Rotary roughing, continuous, around the part (the helix) | The uncut material is on the side of the next turn (the X step direction); the turning direction of A sets the cut | Yes, by the direction A turns | No: each layer is one helix, so there is nothing to alternate; said in the operation. (Alternating whole layers is offered only if the owner wants it, OI-855) |
| Rotary roughing, along the length (indexed lines, or continuous lines along X, decision 0212) | The uncut material is on the side of the next line (the step across the sector, or the A step); the X direction of each line sets the cut | Yes | Yes (zigzag) |
| Rotary roughing, indexed, around the part (decision 0212) | The step is along X; the direction across the sector sets the cut | Yes | Yes |
| Facing (proposed OP-24, decision 0212) | As roughing along its passes | Yes | Yes |
| Helical finishing | The step is along X; the direction A turns | Yes, by A's direction | No (one helix); said |
| Axial and indexed finishing | The step is in A (axial) or across (indexed); the X direction of each pass | Yes | Yes: today's `zigzag` is meander. Today's `climb` value means only "one way"; under OP-25 it would be renamed by meaning (with the stored value kept, so no project changes) |
| Simultaneous 4-axis finishing | Its passes follow a pattern with a step direction; the pass direction sets the cut | Yes, where the pattern has a step side | Yes |
| Rest machining | As the finishing pattern it uses | Yes | Yes |
| Pencil clean-up | The tool runs along a valley with material on both sides | **No**: the cut is on both sides; said | No |
| Edge and curve following | The tool runs on the edge, cutting a groove on both sides | **No** for a groove on the edge; said | No |
| Swarf (flank) machining | The flank cuts one side of a wall; the direction along the chain sets the cut | Yes | Yes, where the wall is cut in more than one pass |
| Radial drilling | No feed sideways | **Not applicable**; said | Not applicable |

Rules common to every strategy:

1. **Defaults keep today's behaviour.** The option's default for each strategy is the order it cuts today; the build names which of climb, conventional or meander that is for each strategy and form and shows it, rather than a new value. No golden file changes (GC-07, NFR-06).
2. **Spindle direction (the owner's choice, option A).** `M3` stays the only spindle direction written until a tool can say it turns the other way (a left-hand cutter). Climb and conventional are made by the feed direction, not by reversing the spindle. `M4` comes later as a property of the tool (TL-04), with its own project schema number, and the post then writes it; the direction rule above swaps for it.
3. **Visibility (NFR-12).** The inspector shows the operation's cutting direction and its spindle direction, and the setup sheet (GC-12, decision 0110) lists both per operation, so the user checking the program sees what will be cut.
4. **The first pass of a layer, slots and full-width cuts.** A pass with the tool engaged across its whole width (the first pass of a layer, a slot) is neither climb nor conventional; it is cut in the direction the option gives for the passes after it, and the operation does not claim more.
5. **Links and safety.** Changing direction changes the links: they still keep clear of the held region (SET-09), the tabs (OP-21), the travel limits and the clearance geometry (OP-14), and the F word limit (decision 0076). A meander's turn-around is a link at feed only where it is clear of the stock, as today's links.

## Decision

OP-25 (P2) with the wording, verification row and rules above, as the owner agreed. Nothing is built and no lane is scheduled (OI-856).

## Alternatives considered

- **Reverse the spindle (`M4`) to make climb or conventional.** Rejected: most tools cut one way only, and reversing the spindle on a right-hand cutter breaks it. The feed direction is the safe control.
- **One global setting per project.** Rejected: the right direction depends on the strategy, the material and the tool; per operation matches issue 251.
- **Build it now with roughing (issue 246).** Rejected on the owner's decision: proposal only. The pass-direction option of decision 0212 is a different choice (which way the passes run, not which side the tool cuts on) and does not need this.

## Consequences

- `docs/open-items.md` gains OI-855 and OI-856; the decision index gains this record.
- `docs/requirements.md` gains OP-25 (P2) and `docs/verification-plan.md` its row, on the owner's agreement; `cargo xtask trace` fails only a P1 requirement with no test, so a P2 entry does not fail it.
- The build (OI-856) needs no project schema number: a strategy parameter is a free map entry (work breakdown, "Project schema ordering" item 3), unless the spindle direction becomes a tool field, which is a tool file change and takes a schema number.
