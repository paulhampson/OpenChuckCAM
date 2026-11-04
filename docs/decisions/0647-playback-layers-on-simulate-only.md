# 0647: The cut part, the tool and the trace draw on Simulate only (SIM-01, SIM-02, UI-03; issue #535; lane VIEW-ORIENT)

- **Status:** proposed by lane VIEW-ORIENT on 4 Nov 2026. The choices (the tool hidden on every step but Simulate; the faint trace hidden outside Simulate; the toolpath lines kept) were made by the coordinator as small UI calls the owner can overrule. Not accepted until the owner says so (decision 0049).
- **Date:** 4 Nov 2026.
- **Number:** provisional 0647; assigned by the Merge Manager at release.
- **Issue:** [#535](https://github.com/paulhampson/OpenChuckCAM/issues/535). Stacked on [0646](0646-view-orientation-a0-mark-and-cube-frame.md) (same code: `orient.rs`).
- **Open items:** none. No project schema change, no library change, no toolpath or G-code change.

## Context

After a simulation the cut part, the tool and the faint trace of the path stayed drawn on Model, Setup and Operations (seen on the Model step in the screenshots of issue #531). The viewport keeps these layers after the playback fills them, and its `layer_shown` rule treated the tool as always shown and the cut part and trace as shown whenever the Show state said so, which on the other steps defaults to on.

## Decision

1. **The cut part (the Stock layer), the tool and the trace draw on Simulate only** (`orient::layer_drawn_on`). On every other step they are not drawn. The toolpath lines still draw on every step, as each operation's eye says (section 6), and the stock outline and the guides, which belong to the scene, are unchanged.
2. **The layers are kept, not cleared**, so returning to Simulate shows them again at the kept cursor (decision 0238 item 5); nothing is rebuilt.
3. **The cut part is not picked outside Simulate**: hovering a surface on Operations reads the part, not a stale cut part. The readout box was already Simulate's for the tool's position (Operations reads the pointer).

## Alternatives considered

- **Clear the layers on leaving Simulate.** Rebuilding the cut part costs a preparation and would lose the kept Simulate state (0238 item 5).
- **Keep the faint trace on Operations.** It is the playback window's trace, with no meaning outside Simulate.

## Consequences

- Code: `orient.rs` (`layer_drawn_on`), `viewport.rs` (`layer_shown`, the cut-part pick). One existing assertion changed: `req_ui_03_req_sim_03_the_step_and_show_or_hide_choose_what_is_drawn` expected the cut part to show on Operations; it now expects it hidden (the toolpath lines still show). That assertion encoded the defect.
- Specification 7.1 (the Show row) says so in the same change.

## Verification

Tests, uninstrumented, written first: `req_sim_01_the_cut_part_tool_and_trace_draw_on_simulate_only` (pure); `req_sim_01_a_run_leaves_no_cut_part_tool_or_trace_on_the_other_steps` (a real run: the cut part showed on Operations before the fix; not drawn on Operations, Setup or Model after, toolpath lines still drawn, drawn again on Simulate at the same cursor); `req_ui_02_req_sim_01_operations_reads_the_pointer_not_the_old_tool_move`. Not seen on screen by a person.
