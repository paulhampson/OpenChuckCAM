# 0344: One finishing Stepdown, and free ends faced in layers (OP-04, OP-09, OP-21, SET-09, TL-05, NFR-12; issues #404 and #314; lane ENDFACE)

- **Status:** accepted (30 Oct 2026) by the project owner, relayed acceptance (decision 0049) by the coordinator session; proposed the same day by lane ENDFACE. The owner's answers on each choice below were relayed by the coordinator session (decision 0049): the shape, C1 = A, C2 = B, C3 = A without the width refusal, C4 = B, OI-1390 option 3 with a warning, OI-1394 (the setup's own stock is known), OI-1396 (the flute refusal kept) and project schema 23 assigned to the lane. The record awaits the owner's acceptance.
- **Date:** 30 Oct 2026.
- **Builds on:** [0343](0343-end-faces-beside-a-short-tab.md) (end faces beside a short tab), [0297](0297-helix-strategies-turn-at-faces-across-the-axis.md) (one turn at each face), [0230](0230-material-preset-per-operation.md) (the preset fills cutting values), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (keep-out).
- **Open items:** OI-1370 closed by this record. New: OI-1393 (a check that a free-end ring would cut stock free, or a limit on the turn's depth). OI-1394 and OI-1396 raised and closed by the owner's answers. Finishing layers (OI-1391 and OI-1392) are [decision 0345](0345-finishing-layers.md).

## Context

A part's end face with nothing of the target past it (no tab or held region within the tool's reach: a "free end") got no turn (decision 0297 needs a drop on both sides of a face). Finishing never cut past the part's end, so such a face kept what roughing left.

Measured on the blisk STL in a synthetic setup: X 15 to 65, cylinder stock 42 mm from X 0, chuck holding 10 mm, raw stock between the chuck and the part. Roughing with a flat 3.175 mm tool at 0.3 mm stock to leave left a helical step on the free end face, mean 1.05 mm and worst 1.30 mm, and finishing left it as it was (mean 0.98 mm). Facing (decision 0252) was the only operation that cut such a face.

## Decision

1. **One Stepdown (the shape the owner agreed).**
   - Helical finishing, axial finishing and indexed finishing have a Stepdown, the depth of each layer along the tool axis; rest machining has one for its free ends.
   - It uses roughing's name, `stepdown`, so the tool's cutting data for the operation's preset material fills it when the operation sets none, and editing it drops the preset as any cutting value does (TL-05, decision 0230).
   - With Finishing layers on (decision 0345) it is required. With them off it serves only the free ends, and zero means none was given.
2. **No guessed depth of cut (C1 = A; NFR-12).** The Stepdown comes from the operation, else the preset, else nothing.
   - With layers on, a missing Stepdown is required, as roughing's is.
   - With layers off it is asked for only where a free end needs it (item 5).
   - The wording is FIRSTRUN's (decision 0302): "Required. Enter the depth of each layer, or add cutting data for this tool and material in the tool library."
3. **Face free ends (C4 = B).** A per-operation switch on helical finishing, axial finishing and rest machining.
   - It is on for a new operation.
   - Project schema 23 migrates each existing operation of those strategies to an explicit off, so saved projects machine as before and every golden program is unchanged. An older file that already holds the switch is refused, naming it.
   - Indexed finishing and the simultaneous spiral have no free-end turn.
4. **Finding a free end** (`openchuck-path::faces`).
   - It is where, on every line of A of the finishing face search, the drop finds the target on one side of an X and nothing on the other, within one sample.
   - On each line the edge is narrowed by bisection to 1e-6 mm; the face is one tool radius (grown by the stock to leave) inside the outermost edge.
   - It must be the part's own end: the part alone, without the held regions and tabs, is under the tool just inside the edge (`SurfaceSource::drop_on_part`). A tab's outer end is not a free end: facing beside it to the axis would cut the tab free of the stock it holds the part to (OP-21).
   - An edge inside a keep-out interval is the held region's own end, or within its reach, and is left out (SET-09).
   - An end the stock does not carry on past (the stock's own end) has nothing to face.
5. **The turn.** With a Stepdown, on the stock an earlier operation of the setup left (SIM-06; the host marks it, `StockModel::left_by_earlier_operations`), or, for the first operation of a setup, the setup's own stock definition (OI-1394, the owner's acceptance):
   - The tool stands beside the face, its side (grown by the stock to leave) a tenth of the tolerance clear of it.
   - It cuts one whole turn of A at constant X in each layer, at whole Stepdowns above the rotary axis: the first is the highest below the top of the stock it meets (read from the stock across the ring at 72 angles), the last on the axis (owner's option 3 on OI-1390).
   - Each layer is its own pass, so the shared services make its entry and link.
   - The face keeps only the stock to leave.
6. **When the turn is not made (C2 = B).** The rest of the path is made, and a warning names the face:
   - With no Stepdown, the warning ends with the Required wording and points at the Stepdown field.
   - As a backstop, on a stock of no known size: "the setup's stock has no known size, so the tool would cut there blind. Define the stock, or turn off Face free ends". In practice a setup with no stock is refused before this.
   - **On the setup's own stock (OI-1394):** the turn is made, and the note adds the starting depth and that the stock past the face was read from the setup's own stock with no earlier operation in this setup, so the real blank must match it. The owner asked why the starting stock could not be used: finish-only setups will be legitimate in multi-setup jobs. It can: the definition is a full stock model, and treating it as unknown was a conservative assumption, not a technical limit.
     - **A real blank smaller than the definition:** more layers in air, which is slow but not dangerous.
     - **A real blank larger than the definition:** a setup error that affects every operation's entries and links already.
     - **"Bounding box plus offset" stock much larger than the part:** the layers start from the box's top, a deep full-width cut where the blank really is that size, with the cut-free warning and the starting depth in the note so a mismatch shows before the program runs.
7. **A face deeper than the flutes refuses the operation (C3; the owner's option 2 on OI-1396).**
   - When the stock beside a free end is deeper than the tool's flute length, the operation is refused at the Face free ends switch, where the user acts. The message names the face's X, the depth, the flute length and each remedy: "the end face at X 0.000 has nothing beyond it and stock 10.300 mm deep beside it, more than the tool's 8.000 mm of flute, so the tool cannot face it. Use a tool with longer flutes, or turn off Face free ends for this operation, or face it with Facing".
   - With Face free ends on for a new operation, a new finishing operation on a part thicker than its tool's flutes fails until the user does one of these. The owner chose this knowing it. The alternatives put to the owner were skipping that end with a warning (recommended by the lane) and making Face free ends opt-in.
   - The app's own test projects, which build new operations on such parts, set Face free ends off where they are not about free ends. That is a fixture change, not a product one, and no golden program changes.
   - A wide cut is not refused (the owner's change to C3).
8. **The cut-free warning (the owner's choice on OI-1390).**
   - When stock lies past the ring the turn cuts, and no held region past it grips that stock, a warning names the end face's X, says it is faced down to the rotary axis, gives the ring's width and X span, and says the stock past the ring is not held by anything in this setup, so it can be cut free there and thrown while the rotary axis turns. It ends: "Before running this program, check the stock and the workholding on the machine".
   - It is raised from a presence test of the stock past the ring (5 positions along X, 16 angles, 3 radii), not a test of whether that stock is joined to anything else (OI-1393). It shows in Messages and on the operation's notes.
   - The setup sheet carries no operation's cut warnings, so it is not there. It does not block export.
9. **Notes.** An operation that faced a free end says so: "the end face at X 15.000 has nothing beyond it: the tool goes down beside it in 21 layers of 1 mm to the rotary axis, turning once round the axis in each".

## Risks, and what limits them

The owner chose to go down to the axis with wide cuts allowed and no check that a piece is cut free, knowing the hazard; this is recorded as asked.

- **A loose piece.**
  - **What can happen:** a ring cut to the axis parts the stock past it from the part. With nothing holding it (no tailstock: that is what makes the end free), it is loose while A turns: it can be thrown, break the tool or damage the machine.
  - **What limits it:** the warning (item 8), named and specific, raised only when stock lies past the ring and no held region past it grips it. The turn needs stock an earlier operation left. A held region past the end means no free end there, or no warning.
  - **What would do better:** a test that the stock past the ring is joined to a held region or a tab (OI-1393), or turning only as deep as the stock past the face is already clear (option 1 of OI-1390, which never cuts anything free).
- **Full-width engagement.**
  - **What can happen:** where roughing left stock past the end, the turn cuts with up to the tool's whole diameter: force, heat, chip packing, deflection or a broken finishing tool, worse on a long or small tool or a ball nose. The finishing feed and speed are not slotting values.
  - **What limits it:** layers of the Stepdown, and the flute check. The note gives the layer count, and the warning gives the ring.
  - **What would do better:** an engagement figure in the warning (the width of stock met in each layer), or a setting for the widest cut allowed (OI-1393).
- **A forgotten earlier operation.**
  - **What can happen:** the check tells a carried stock from the setup's own, not a thin skin from a deep block.
  - **What limits it:** the stock's real top is read (item 5), so the layer count shows the depth met.
  - **What would do better:** as above.
- **Generated G-code is the user's to verify** before it is run (NFR-12). Nothing here says a path is safe.

## Consequences

- **Saved projects and goldens.** Unchanged: existing operations are migrated to Face free ends off. A new operation faces free ends where the stock is known.
- **Library.** Additive: the module `finishing`, `StockModel::left_by_earlier_operations` and `SurfaceSource::drop_on_part` (provided methods, default as before).
- **Project.** Schema 23, claimed in the work breakdown. The schema 22 step is a stand-in for lane Z1AXIS's until that lane merges.
- **Determinism.** The free ends, layers and warnings depend only on the part, the stock, the tool and the parameters, not on the thread count (GC-07, NFR-06). Tested at 1, 2 and N threads.

**Tests** (`crates/openchuck-path/tests/helical_faces.rs` unless noted). Each measures independently, on a radius 10 cylinder whose start end has stock 5 mm past it.
- `req_op_04_req_op_09_a_free_end_is_faced_in_layers_down_to_the_axis` (helical, axial and rest: within the tolerance, never cut in, 6 layers, the warning)
- `req_nfr_12_req_tl_05_a_free_end_with_no_stepdown_is_left_and_said`
- `req_nfr_12_a_free_end_with_unknown_stock_is_left_and_said`
- `req_nfr_07_req_nfr_12_a_free_end_deeper_than_the_flutes_is_refused`
- `req_op_04_face_free_ends_off_leaves_the_path_as_before`
- `req_set_09_stock_past_a_free_end_that_a_held_region_grips_gets_no_cut_free_warning`
- `req_op_21_the_outer_end_of_a_tab_is_not_a_free_end` (failed without the part-only drop)
- `req_nfr_06_req_gc_07_free_end_turns_are_identical_at_1_2_and_n_threads`
- `crates/openchuck-path/tests/finishing.rs`:
  - `req_op_04_face_free_ends_is_on_for_helical_axial_and_rest_and_nowhere_else`
  - `req_op_04_finishing_layers_are_off_by_default_on_helical_axial_and_indexed`
  - `req_tl_05_req_nfr_12_stepdown_is_required_only_with_layers_on`
- `crates/openchuck-project/tests/persistence.rs`:
  - `req_prj_04_req_op_04_older_finishing_operations_keep_free_ends_off`
