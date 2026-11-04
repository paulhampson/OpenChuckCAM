# 0512: The cut part, the tool and the trace draw on Simulate only (SIM-01, SIM-02, UI-03; issue #535; lane VIEW-ORIENT)

- **Status:** accepted by the project owner on 4 Nov 2026, relayed by the coordinator (decision 0049), including the two small calls: the tool hidden outside Simulate and the faint trace hidden outside Simulate. Proposed by lane VIEW-ORIENT the same day.
- **Date:** 4 Nov 2026.
- **Number:** proposed as 0647; renumbered 0512 on 4 Nov 2026 at release (assigned by the Merge Manager).
- **Issue:** [#535](https://github.com/paulhampson/OpenChuckCAM/issues/535). Stacked on [0511](0511-view-orientation-a0-mark-and-cube-frame.md) (same code: `orient.rs`).
- **Open items:** none. No project schema change, no library change, no toolpath or G-code change.

## Context

After a simulation the cut part, the tool and the faint trace of the path stayed drawn on Model, Setup and Operations (seen on the Model step in the screenshots of issue #531). The viewport keeps these layers after the playback fills them, and its `layer_shown` rule treated the tool as always shown and the cut part and trace as shown whenever the Show state said so, which on the other steps defaults to on.

## Decision

1. **The cut part (the Stock layer), the tool and the trace draw on Simulate only** (`orient::layer_drawn_on`). On every other step they are not drawn. The toolpath lines still draw on every step, as each operation's eye says (section 6), and the stock outline and the guides, which belong to the scene, are unchanged.
2. **The layers are kept, not cleared**, so returning to Simulate shows them again at the kept cursor (decision 0238 item 5); nothing is rebuilt.
3. **The cut part is not picked outside Simulate**: hovering a surface on Operations reads the part, not a stale cut part. The readout box was already Simulate's for the tool's position (Operations reads the pointer).

4. **The A0 mark of decision 0511 is at the far end of the part, not of the stock** (CodeRabbit on #533, carried here by the coordinator's ruling): with stock longer than the part the mark stayed on the stock end, off the part it identifies. It uses the part's own extent, and the scene's end only when no part is readable; the part, not the shorter of part and stock, so it is on the workpiece whichever is longer.

## Alternatives considered

- **Clear the layers on leaving Simulate.** Rebuilding the cut part costs a preparation and would lose the kept Simulate state (0238 item 5).
- **Keep the faint trace on Operations.** It is the playback window's trace, with no meaning outside Simulate.

## Consequences

- Code: `orient.rs` (`layer_drawn_on`), `viewport.rs` (`layer_shown`, the cut-part pick). One existing assertion changed: `req_ui_03_req_sim_03_the_step_and_show_or_hide_choose_what_is_drawn` expected the cut part to show on Operations; it now expects it hidden (the toolpath lines still show). That assertion encoded the defect.
- Specification 7.1 (the Show row) says so in the same change.

## Verification

Tests, uninstrumented, written first: `req_sim_01_the_cut_part_tool_and_trace_draw_on_simulate_only` (pure); `req_sim_01_a_run_leaves_no_cut_part_tool_or_trace_on_the_other_steps` (a real run: the cut part showed on Operations before the fix; not drawn on Operations, Setup or Model after, toolpath lines still drawn, drawn again on Simulate at the same cursor); `req_ui_02_req_sim_01_operations_reads_the_pointer_not_the_old_tool_move`. `req_ui_03_the_a0_mark_is_at_the_far_end_of_the_part_not_of_the_stock` (stock reaching X 12 past a part ending at X 10: the mark was at 12 before the change, 10 after). The tool is not drawn outside Simulate (the first tests above), which also settles CodeRabbit's second finding on #533 (the tool placed with the machine-view frame outside Simulate). Not seen on screen by a person.
