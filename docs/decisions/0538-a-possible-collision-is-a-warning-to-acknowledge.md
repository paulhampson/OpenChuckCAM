# 0538: A possible collision is a warning to acknowledge (lane MSGREVIEW; issue #567)

- **Status:** accepted by the project owner on 7 Nov 2026, relayed by the coordinator (decision 0049), as written: 11 checks, one tick per result, a flag on the command line. Proposed by lane MSGREVIEW on 7 Nov 2026. It records the owner's clarification of 7 Nov 2026, relayed by the coordinator with the answer to question 1 of [decision 0536](0536-message-levels-operation-reference-and-setting-links.md): checks that relate to a potential collision, such as the flute length check, become a warning requiring acknowledgement (the GC-16 level). It changes what the export and the command line do, so it is a record of its own, accepted separately from 0536. The owner's answers to questions 8 to 10 (7 Nov 2026, relayed, each option A) settle which rows qualify.
- **What it amends, if accepted:**
  - [decision 0175](0175-swarf-overcut-is-a-warning.md) item 2: a path warning needs no tick, unless it is a possible collision;
  - [decision 0522](0522-the-flute-check-warns.md) item 1 and [decision 0523](0523-tab-checks-follow-the-operations-reach.md) item 4: the flute and tab warnings need a tick;
  - [decision 0532](0532-swarf-keeps-off-the-faces-at-its-ends.md) (the warnings when swarf cuts into a face next to it).

  The requirements are unchanged. GC-16's own confirmation (that the user will verify the G-code) stays as it is. The export already asks for ticks of this kind, for a partial path (decision 0239) and for a skipped stock simulation (decision 0168). GC-16's rule that the command line asks for no confirmation is kept, because a flag asks nothing.
- **Date:** 7 Nov 2026.
- **Depends on:** decision 0536 item 6.1. The library type carries the reason for a tick, so a collision is never called partial.
- **Open items:** OI-2100 (the owner's acceptance; closed on 7 Nov 2026 with this record) and OI-2101 (the verification plan's GC-16 entry). Both are new, raised with 0536.

## Context

A warning that needs acknowledging exists today for one case: a partial path from adaptive spacing (decision 0239 item 10). The export asks for a tick before writing, and the command line asks for `--acknowledge-partial-path`. A project that skips the stock simulation asks for a tick of its own (decision 0168). Every other warning, including those that point at a possible collision of the tool with the stock or the part, needs no tick: the flute length (0522), a short tab (0523), and swarf cutting into a face next to it (0532).

No existing acknowledgement is weakened by this record.

## Decision (proposed)

### 1. The rule

A finding that may be a collision of the tool, its shank or its holder with the part, the stock, a tab or a held region is a **warning to acknowledge**, not a plain warning.

That holds whether the check measured the collision or could not rule it out (stock of unknown size, a position that is not a number). A finding that only leaves material uncut, or that the strategy already avoided by changing the path, is not a collision.

### 2. The checks that qualify

"Yes" rows need a tick. "No" rows stay plain warnings, listed so the reason is on record. The owner settled the three rows left open (questions 8 to 10, 7 Nov 2026, relayed): the swarf overcut into its own face stays a plain warning, a touched check set needs a tick and an avoid set does not, and a part cut free at a free end needs a tick while the stock ring does not.

| Check | Where (`main` at `e869d932`) | Level today | Qualifies | Why |
| --- | --- | --- | --- | --- |
| Flute length: the stock stands above the flutes along the path (decisions 0509, 0522, 0524) | `openchuck-path` `flutes.rs:647-672` | warning, no tick | **yes** | the shank or holder may rub the stock |
| Flute length: the stock's size is not known, so the flutes could not be checked | `flutes.rs:640-645` | warning, no tick | **yes** | a collision cannot be ruled out |
| Flute length: a tool position is not a finite number | `flutes.rs:634-639` | warning, no tick | **yes** | as above; a defect, so it also says to regenerate |
| Flute length at a free end that is not faced (decision 0531) | `finishing.rs:296-311` | warning, no tick | no | the end is skipped, so the tool does not go there |
| Tab length: a tab shorter than the tool is wide (decisions 0494, 0523) | `openchuck-app` `workholding.rs` (`TAB_LENGTH_LEAD`) and the setup note at `workholding.rs:221-243` | warning, no tick (shown twice today) | **yes** | the tool may cut through the tab, and the part may come loose |
| Swarf cuts into a face next to it, at the end of its rulings or at an end of the pass (decision 0532) | `swarf.rs:907-953` | warning, no tick | **yes** | the side of the tool meets another face of the part |
| Swarf cuts into its own finished face beyond the tolerance (decision 0175) | `swarf.rs:859-890` | warning, no tick (the owner chose no tick in 0175) | no (owner, Q8) | a gouge, often the face's own error of hundredths of a millimetre, not a collision; it stays a plain warning, with "Show setting" on Tolerance |
| A selection's check set is touched, where the set only warns | `region.rs:441-462`, `swarf.rs:3082-3093` | warning, no tick | **yes** (owner, Q9) | a check set often stands for a clamp, a fixture or another part |
| A selection's avoid set is cut, where the set only warns | same | warning, no tick | no (owner, Q9) | a gouge of a surface of the part, not a collision |
| A free end faced down to the rotary axis with nothing holding the part on the other side, so the part is cut free (decisions 0344, 0500) | `finishing.rs:428-439` | warning, no tick | **yes** (owner, Q10) | the part cut free can be thrown into the tool or the machine |
| A free end faced down to the rotary axis, cutting the stock past the facing ring free | `finishing.rs:390-403` | warning, no tick | no (owner, Q10) | an offcut ring is expected when facing an end |
| A rapid that removes material, found by the simulation | `openchuck-app` `pipeline.rs:441`; Results line `run_panel.rs:205` | a note in the Results tab (and a "problem" line); not in Messages or the export | **yes** | a rapid through material is a crash. It is known only after a stock simulation; a project that skips it already ticks that (decision 0168) |
| The setup's safe cylinder does not enclose the stock, or the part | `openchuck-types` `setup.rs:466-475` | setup warning, no tick | **yes** | links at the clearance pass through material |
| The safe cylinder is inside the machine's minimum clearance | `setup.rs` (`ClearanceBelowMinimum`) | setup warning, no tick | **yes** | its own text says links may hit the chuck, jaws, headstock or tailstock |
| A tool may be too long to clear the workholding within the Z travel (decision 0318) | `openchuck-app` `setup.rs` (`spindle_height_text`) | setup warning, no tick | **yes** | the tool may meet the workholding |
| A tool may take the spindle above the top of the Z travel | same | setup warning, no tick | no | a travel limit, which the controller stops at; not a collision with the job |
| A point of the job is outside the machine's travel | `setup.rs` (`OutsideTravel`) | setup warning, no tick | no | a travel limit, as above |
| The shank is wider than the cutting part, so simultaneous finishing breaks the path and links round | `simultaneous.rs:1042-1053` | warning, no tick | no | the strategy avoided the collision; the warning says where it changed the path |
| Unreached areas, material no tool can reach, a selection that reaches a held region, links that could not be lowered | various | warning or note | no | material is left; the tool keeps clear |
| The simulation's overcut lines | `run_panel.rs` | a "problem" line | no | a gouge, measured after the fact; it stays where it is |

Entry, ramp, held-region and tab-zone refusals are errors already; nothing is made, so they are unchanged.

### 3. What changes for each check that qualifies

- **Export (GC-16).** Writing waits for a tick, as for a partial path today. The dialog lists each collision warning of the result with its operation or setup, and one tick acknowledges them for this result. Proposed label: "I have read the possible collisions listed for this result and will check them before running the program". It names nothing as safe (NFR-12). A new run asks again (decision 0239 item 10). The partial-path tick stays its own tick, with its own label.
- **Command line.** `regenerate --out` refuses to write without a flag (proposed: `--acknowledge-collision-warnings`), as it does for a partial path today (decision 0239 item 10). The refusal names each warning. A flag is not a confirmation: GC-16 says the command line runs unattended and asks for no confirmation, and a flag given on the command line asks nothing, as the partial-path flag already shows.
- **Operation and setup marks.** The Warning mark, with the cause starting "Needs acknowledging:".
- **Messages.** Warnings to acknowledge come first among the warnings, each with its link. The rapid cut, which today reaches only the Results tab, is listed on its operation while the simulation that found it is current.
- **Not "partial".** Today the only warning that needs a tick is a partial path, and the export reads every such warning as partial (`export.rs`, `partial_operations`). The library type of decision 0536 item 6.1 carries the reason for the tick: partial, or a possible collision. Only a partial path is "not complete". Setup warnings take their reason from their kind in the application.
- **Output.** G-code is unchanged, byte for byte, with or without the tick (GC-07).

### 4. Test plan

Each test is named after its requirement, written first and watched fail.

- **For each check marked yes:** `req_gc_16_<check>_needs_acknowledging`, for example `req_gc_16_flute_length_needs_acknowledging`. Each test checks four things:
  - the export refuses to write without the tick, and writes with it;
  - the written files are byte-identical to a run with no warning (GC-07);
  - the command line refuses without the flag and writes with it;
  - the operation's or setup's mark reads "Needs acknowledging".
- `req_gc_16_partial_path_acknowledgement_is_unchanged`: a partial path still asks its own tick, is still called partial, and a collision warning is never called partial.
- `req_gc_16_a_warning_that_is_not_a_collision_asks_no_tick`: an unreached-areas warning is still a plain warning.
- `req_ui_12_warnings_to_acknowledge_come_first`: the order in Messages.

## Alternatives considered

- **Leave collision warnings as plain warnings** (today). Not chosen: the owner's clarification asks for the tick.
- **One tick per warning instead of one per result.** It is more deliberate, but a result with many tabs would ask for many ticks, and a tick per result is how the partial path works today. Left as it is unless the owner prefers it.
- **Refuse instead of warn.** That is a different decision. The owner made the flute and tab checks warnings on purpose (0522, 0523), because their measures are conservative bounds (OI-2044).

## Consequences

- The export dialog gains a second acknowledgement tick, shown only when the result has a possible collision.
- The command line gains a flag.
- The marks and Messages show "Needs acknowledging".
- Interface specification section 11.6 (the checks before writing) and the verification plan's GC-16 entry (OI-2101) change when it is built.
- The library type of 0536 carries the reason for a tick. It is part of 0536's version bump; this record adds no second one.
- G-code is unchanged (GC-07).
- Built as its own pull request after 0536's first build step, with the tests of item 4.
