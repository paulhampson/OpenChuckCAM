# 0205: A default stock to leave for roughing: the options (lane OPCREATE; issue 223, F-67)

- **Status:** proposed. A choice for the owner; nothing is built. The lane recommends option B below.
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

None yet; the owner chooses A, B, C or D. If B or A is chosen, the lane builds it in `openchuck-app` `add_operation` (writes the value into the new operation, one undoable step), shows it in the inspector with the usual provenance ("Set for this operation"), and tests that an existing project's G-code is byte for byte the same.
