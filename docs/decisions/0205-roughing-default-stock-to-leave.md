# 0205: A default stock to leave for roughing: the options (lane OPCREATE; issue 223, F-67)

- **Status:** accepted by the project owner (21 Oct 2026, directly in the lane OPCREATE session), option B, which the owner had chosen earlier (relayed by the coordinator session) and the lane built.
- **What it amends:** nothing until chosen. If chosen it adds an entry to OP-11 (stock to leave) that the owner agrees; `docs/requirements.md` is not edited here.
- **Date:** 21 Oct 2026.
- **Open items:** OI-794 is new.

## Context

Issue 223 asks that a new roughing operation start with a sensible stock to leave, shown in the inspector with its source and changeable. Today the default is **0 mm for every strategy** (`stock_to_leave` in the common parameters, `openchuck-path/src/params.rs`), so roughing cuts to the final surface, leaves nothing for finishing, and a finishing pass that follows finds nothing to cut. The owner's own projects set 0.3 to 0.5 mm by hand. Decision 0058 sets how a per-selection amount raises roughing's amount; it does not set a default.

The default must be deterministic (GC-07, NFR-06), shown and not hidden, and must not change the output of any existing project: an operation saved without the value has meant 0 so far, and a stored value always wins. A default that applied to old operations would change their G-code, so any option below applies to **operations added after the change** by writing the value into the new operation (visible in the inspector, as any value the user set), or applies only where the file has no value and says so in the inspector. The lane recommends the first: the value is stored, so the project says what it was made with.

## Options

- **A. A fixed amount, 0.5 mm, for rotary roughing only.** Simple and predictable; matches what the owner uses on the blisk (0.5 and 0.3). Too much on a small tool (a 1 mm tool), too little for a large one.
- **B. A fraction of the tool diameter, 10 percent, rounded to 0.05 mm, with a floor of 0.1 mm and a ceiling of 1 mm, for rotary roughing only (recommended).** 0.3 mm for a 3.175 mm tool, 0.6 mm for 6 mm. It follows the tool's size, is easy to explain in one sentence, is deterministic, and shows as an ordinary value once the operation exists. It needs the tool at the moment the operation is added; with no tool yet the field stays empty and the missing-value mark asks for it, as for the feeds.
- **C. From the tool's cutting data per material (TL-05)**, adding a stock to leave field to the cutting data. The most accurate, but it changes the tool library and project formats (schema claim) and needs data that no tool has today. Left for after issue 197.
- **D. Keep 0 as the default** and only mark roughing with a warning when it leaves nothing for finishing. Changes nothing stored, but the user still has to set the value.

Finishing operations keep 0: they are meant to cut to the surface. Whether a finishing operation should warn when the roughing before it left less than it expects (the issue's second question) is OI-794, not part of this choice.

## Decision

1. **Option B, the owner's choice.** A new rotary roughing operation added with a tool starts with a stock to leave of 10 percent of the tool's diameter, rounded to 0.05 mm, at least 0.1 mm and at most 1.0 mm (0.3 mm for a 3.175 mm tool, 0.4 mm for 4 mm, 0.6 mm for 6 mm). Rotary roughing only; no other strategy changes.
2. **Where.** `Application::add_operation_on` writes the value into the new operation's parameters in the same undoable step, so the inspector shows it as a value set on the operation and the user can change or reset it. Operations already in a project are not touched, and an operation saved without the value still means 0, so existing projects' G-code is byte for byte unchanged. No project format change, no schema number.
3. **No tool yet.** An operation added with no tool gets no value (the diameter is not known); the stock to leave stays at the schema default of 0 until the user sets it. A tool chosen afterwards does not write it. This is a gap the owner may want closed (OI-794).
4. **No finishing warning for now** (the owner's answer to the issue's second question).

Tests: `req_op_11_the_default_roughing_stock_to_leave_follows_the_tool_diameter` (opflow.rs) and `req_op_11_req_op_03_a_new_roughing_operation_starts_with_a_stock_to_leave` (`crates/openchuck-app/tests/operation.rs`).
