# 0057: Keeping tangent edges on a STEP part is chosen per edge selection, with a project default (lane Z; OI-232)

- **Status:** accepted by the project owner (17 Oct 2026, directly in the lane Z session) as option C: a separate keep-tangent-edges setting, which the owner refined the same day (relayed by the orchestrator session) from a per-operation switch to a per-selection setting with a project default. Item 1 is that refined form; alternative C below is the first, per-operation form. Lane Z built the seam on `claude/lane-z-brep-pipeline`; the stored setting and its interface are lane O's (project schema 12). It changes no requirement, no architecture rule and no library-family API, and lane Z takes no schema number.
- **Date:** 17 Oct 2026.
- **Open items:** OI-232 stays open (state: seam built; storage and interface wait for lane O). No new open item.

Numbering: from the records 0056 to 0060 the orchestrator assigned to lane Z; this takes 0057. 0058 to 0060 are unused.

## Context

`BrepEdges::with_crease(0.0)` keeps the edges between tangent faces, such as the boundaries of a fillet, for a user who wants to engrave the line where a fillet starts (decision 0053 item 2). Decision 0056 item 4 passes the operation's `crease_angle` to the B-rep source, 1 degree when it is not set, but the parameter's range excludes 0, because on a mesh a crease angle of 0 makes every triangle edge a crease. So a STEP part could not keep its tangent edges from the application (OI-232).

## Decision

1. **The owner's choice: per edge selection, with a project default that is off.** Whether tangent edges are kept is a property of the `Edges` selection set (it may change from one selection to the next), with a project-level default for selections that do not set it; the default is expected to be off. The operation resolves it as: the chosen `Edges` set's own setting if it has one, otherwise the project's default, otherwise off. A mesh part has no such choice: its edges are chosen by the crease angle.
2. **Storage and interface are lane O's.** The per-selection field and the project default are a project format change, so they belong with lane O's stored edge-chain selection kind and its schema 12 (OI-204); lane Z takes no schema number and adds no form control or operation parameter.
3. **The seam is built now (lane Z).** `openchuck-app` gains `operation::EdgeOptions { keep_tangent }`, `operation::edge_options(project, operation)`, which resolves the setting and answers off until lane O stores it (the one private function `keep_tangent_edges` is the place lane O wires), and `operation::generate_operation_with`, which takes the options as given. With `keep_tangent` the B-rep source is built at a crease angle of 0, whatever the operation's `crease_angle`, whose meaning and range are unchanged. Asked to keep tangent edges on a mesh part, the operation fails with a message naming it. The pipeline calls `edge_options`, so nothing changes for any project until lane O lands.

## Alternatives considered

- **A. Leave it (no way to keep tangent edges):** rejected by the owner; the fillet boundary can be exactly the line to engrave (decision 0053, alternatives).
- **B. Allow 0 in the crease angle's range:** a crease angle of 0 on a mesh selects every triangle edge, which is never meant.
- **C as first chosen: a per-operation on/off parameter, or a project setting only:** the owner first chose an operation switch (shown for STEP parts only; a switch, not the second crease parameter decision 0056 rejected), then changed to per selection: the choice may differ between selections of one part, and going to an operation's or the project's settings each time is a poor experience.
- **D. A range for the crease angle that depends on the model's kind:** one parameter whose limits change with the part is confusing, and the schema is the same for every part (OP-16).

## Verification

| What | Test |
| --- | --- |
| Off (the default resolution): the filleted box has no edge to follow, through `regenerate` and through `generate_operation_with`; on: its 48 fillet boundaries are followed, each its own chain, as at crease angle 0 in `req_op_08_step_the_boundaries_of_fillets_are_edges_only_at_crease_angle_zero`; a saved project reads back equal and holds no new field | `req_op_08_keeping_tangent_edges_follows_the_fillet_boundaries_of_a_step_part` (`crates/openchuck-app/tests/step_strategies.rs`, kernel, 2.2 s) |
| Keeping tangent edges on a mesh part is refused by name; off, the mesh is followed as before; the project resolves off | `req_op_08_req_nfr_07_keeping_tangent_edges_is_refused_on_a_mesh_part` (`operation.rs`, no kernel) |

Every existing test passes unchanged, so old projects and their output are unchanged.

## Consequences

- **Code:** `openchuck-app` (`operation.rs`: `EdgeOptions`, `edge_options`, `generate_operation_with`, the B-rep edge source's crease). No library, format or interface change.
- **Documents:** the open-items register (OI-232), the decision index, the `openchuck-app` changelog, the work breakdown's next free numbers.
- **For lane O:** store the per-selection setting on the `Edges` set and the project default (schema 12), make `keep_tangent_edges` read them, and offer the choice where an edge selection is made, for STEP parts only.
