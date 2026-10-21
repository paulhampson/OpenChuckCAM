# 0174: A strategy that needs a STEP part is disabled, with the reason, on a mesh part (lane BUG-SWARF; issue #206)

- **Status:** accepted (21 Oct 2026, by the owner directly in the lane BUG-SWARF session). Proposed 21 Oct 2026. The owner's answers on issue #206 (20 Oct 2026) set the behaviour: every strategy that needs a B-rep (STEP) part is blocked the same way on an STL project, and is shown disabled with the reason, not hidden; the rule comes from each strategy's registration, not a fixed list. The coordinator relayed on 21 Oct 2026 that the owner agrees with the lane's reading of which strategies that is (item 2). Only the owner accepts this record (decision 0049). **It amends accepted [decision 0111](0111-swarf-through-the-application.md) item 3** (the B-rep-only message was a form note and a generation error; now the strategy cannot be added either) and item 6 (no interface change).
- **Date:** 21 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): none is new.

Numbering: 0174 to 0176 and OI-630 to OI-639 were assigned to lane BUG-SWARF by the coordinator; on 21 Oct 2026 no other branch held them (the highest record on an unmerged branch was 0187, lane BUG-INSP, and the highest `OI-` number in use was OI-693). No project schema number.

## Context

Trial finding F-50 (issue #206). On a project whose part is an STL, a swarf operation could be added. It then went through some generation steps and failed with decision 0111 item 3's error ("swarf is B-rep only ..."). The owner asked that it is not offered as usable, so no wasted generation runs.

Requirements: OP-07 (swarf), OP-10 (radial drilling), OP-16 (the strategy interface), UI-06 (operations), NFR-07 (clear errors). Records read: 0021 (swarf is B-rep only), 0051 (drilling), 0047 and 0053 (edge following), 0056 (the B-rep path through the pipeline), 0111 (swarf through the application), 0048 (API changes before 1.0), 0114 (plain wording).

## Decision

1. **Each strategy says in its registration whether it needs a B-rep part (OP-16).** The `Strategy` trait in `openchuck-path` has a new provided method, `needs_brep_part(&self) -> Option<&str>`. It gives the reason in a few words, or `None` (the default) for a strategy that works on a mesh. A strategy added later says it the same way, and the host reads it from the registry. Nothing in the host lists strategies by name.
2. **Which built-in strategies need one.**
   - **Swarf** needs one: "it reads the rulings of the exact faces of a STEP model, and a mesh has none".
   - **Radial drilling** needs one: "it finds round holes in the cylindrical faces of a STEP model, and a mesh has none".
   - **Edge following** does not. On a mesh it follows the mesh's feature edges; on a STEP part it follows B-rep edges (decisions 0047, 0053, 0066). It stays available on an STL project.
   - **Every other strategy** works on a mesh.

   The owner accepted this reading on 21 Oct 2026, relayed by the coordinator.
3. **The strategy list shows them disabled, with the reason.**
   - **The list.** `Application::strategy_choices_for(setup)` marks a strategy unavailable on a setup whose part is a mesh, with the reason as `StrategyChoice::unavailable`: "Needs a STEP model: *reason*. This setup's part '*file*' is a mesh (STL); add the part as a STEP file to use it". The part counts as a mesh by its file name, as `on_step_part` already judges it, without reading the file. `strategy_choices()` (no setup) marks none.
   - **The dialog.** The Add operation dialog lists such a strategy greyed out, with "· needs a STEP model" after its name and the full reason on hover. It is never chosen by default. If the setup is changed after the strategy, the reason shows under the fields with the warning mark, and Add is disabled.
   - **The two-step New operation (#195)** is a later lane. It reads the same `unavailable` field.
4. **Adding one is refused.** `add_operation` and `add_operation_on` refuse such a strategy on such a setup, with the same reason, so no route adds one, including the command line and scripts.
5. **An existing operation shows it at once; the late error stays.** An operation can still have one: it was made before this rule, or its setup's part was replaced by a mesh since. For such an operation:
   - the overview puts it in the error state with the reason, before anything is generated, and lists it in Messages with a Show link;
   - the parameter form's first note says the same;
   - generating fails with the same reason, naming the operation and the part.

   These come from the registry for every such strategy, not only swarf. The message is "*Strategy label* is B-rep only (*reason*): the setup's part '*file*' is a mesh; add the part as a STEP file". Decision 0111 item 3's test `req_op_07_req_nfr_07_swarf_on_a_mesh_part_says_it_is_b_rep_only` keeps the late error as the fallback (NFR-07). The strategies' own refusal of a mesh stays for other hosts of the library.

## Alternatives considered

- **Hide the strategies on a mesh part:** the owner chose disabled with the reason, so the user learns that the strategy exists and what it needs.
- **A fixed list of B-rep strategies in the application:** a strategy added later would be missed. The owner asked for the rule to come from each strategy's registration.
- **A boolean `needs_brep_part`:** the host would have to word each strategy's reason itself. With the reason in the registration, the strategy states what only it knows, in one place.
- **Block edge following too:** on a mesh it follows the mesh's feature edges, which is a supported use (decision 0047). Only its B-rep edge sets need a STEP part, and those cannot be made on a mesh.

## Verification

| What | Test |
| --- | --- |
| Swarf and radial drilling, and no other built-in strategy, declare a B-rep need in their registration, with a reason naming STEP (OP-16) | `req_op_16_req_op_07_strategies_say_in_their_registration_whether_they_need_a_step_part` (`openchuck-path` `tests/swarf.rs`) |
| On a setup with an STL part they are listed unavailable with the reason and cannot be added, edge following can, and an existing operation that uses one is in error before generation (OP-07, OP-10, UI-06) | `req_op_16_req_op_07_req_op_10_strategies_that_need_a_step_part_are_disabled_with_the_reason_on_an_stl_part` (`openchuck-app` `tests/operation.rs`) |
| The late error and the form note stay as the fallback (NFR-07) | `req_op_07_req_nfr_07_swarf_on_a_mesh_part_says_it_is_b_rep_only` (unchanged) |

## Consequences

- **Code:**
  - `openchuck-path`: `Strategy::needs_brep_part`, given by swarf and radial drilling.
  - `openchuck-app`: `StrategyChoice::unavailable`, `strategy_choices_for`, the refusal in `add_operation_on`, `brep_only` read from the registry, and the error state in the overview.
  - `openchuck-ui`: the Add operation dialog.
- **Library API:** the provided trait method is not breaking on its own. It rides the 0.13.0 bump of [decision 0175](0175-swarf-overcut-is-a-warning.md).
- **Documents:** the work breakdown's lane row, the decision index and the changelogs. The interface specification's Add operation dialog (11.4) gains the disabled entries; its wording is proposed here and edited when this record is accepted.
