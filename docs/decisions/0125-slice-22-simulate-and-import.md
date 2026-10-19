# 0125: Slice 22 Simulate results, playback dock, one import window and selection-set rows (lane X6)

- **Status:** proposed. Lane X6, slice 22 ([decision 0114](0114-slice-22-conformance-report.md) section E, accepted). It records how the lane built differences C14 (the import window), C17 (Simulate) and C20 (selection sets), and the choices the specification leaves open. It is guidance until the owner accepts it.
- **Date:** 19 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-370 to OI-374 are new; OI-372 and OI-374 are closed. OI-342 stays open (this lane's boards are re-rendered).

Numbering: the coordinator assigned this lane records 0125 and 0126 and OI-370 to OI-374. 0119 was the highest record on `main`, and OI-365 the highest open item, when the lane started. Record 0126 is not used.

## Context

Decision 0114 found: the import path asked for in a separate modal and the import window drawing its own text for findings, units and the mesh check (C14); the results as text lines in a floating window, no resolution control for SIM-07 (a P1 requirement) and a playback panel without segments, the extra speeds in a list and no keys (C17); and selection-set rows that hid their reason in a tooltip and kept rename and delete in a details block (C20). Requirements: IMP-01, IMP-03, IMP-04, IMP-05, SIM-04, SIM-05, SIM-07, UI-05. No requirement, architecture or project format change; no engine change (slice 22 item 4).

## Decision

1. **One import window (C14; IMP-01).**
   - The path field, Browse and Check are in the window. The modal path dialog is removed (`Dialog::ModelFile`); File > Import model, the Model "+" and a dropped file open the window.
   - Blocks as in the board: Units (the unit question in the warning box, a segmented control of the five units, the largest side after conversion), Placement, Mesh check, the STL statement, the footer with the sentence that the file is unchanged and its path and hash stored, Cancel, and "Add to project" as the filled primary at the right.
   - The mesh check has one row per kind of finding with its count and a tick when there are none (`mesh_rows`), plus a row for closed pieces that are inside out, which the check reports (`MeshReport::inside_out_shells`) and the board did not list.
   - The legend (rotary axis, hole, flipped) sits bottom left of the main view and the size, largest radius and volume bottom right (`ImportSession::overlays`, placed from the view's last rectangle).
   - Placement keeps its combo of eight choices, now worded in plain words and without the orange axis (D1).
2. **Results inspector (C17; SIM-05, SIM-07).** On the Simulate step the inspector shows `results_panel.rs`: colour by Deviation, Remaining or Overcut (it leaves the other bands out of the view's bands, so those triangles keep the part's colour), the worst values, the share within tolerance, a histogram in the view's band colours, a table by operation and the resolution control.
   - **Resolution (SIM-07).** A cell size field, a "Default" button, the range 0.025 to 1 mm, and the cost against the default stated (64 times the time and 16 times the memory at the finest, from `SimulationSettings::relative_time` and `relative_memory`). It sets the project's simulation settings with `Command::SetSimulation`, which is undoable and makes the simulation stale; a value out of range is refused with the application's own sentence.
   - **Within tolerance** counts the deviation samples with an absolute value of at most the cell size, the simulation's own error (the same tolerance the colour bands use).
   - **The floating Results window** no longer opens by itself after a successful run, because the inspector carries the simulation results. It still opens after a failed run and from Project > Results…, for the generation notes and G-code checks (OI-373).
3. **Playback dock (C17; SIM-04).** Previous and next operation replace to-start and to-end; step back, play or pause and step forward stay. Speeds are 1×, 10×, 100×, 1000× and 10 000× as one segmented row, with "Whole program" 10 s or 60 s. There is no Max speed: the owner decided it makes no sense (OI-372, closed; relayed by the coordinator, 19 Oct 2026), so the build and the board omit it. The timeline is one segment per operation sized by its moves, the playhead at the cursor's move, click or drag to seek, and each enabled operation the run did not make is a dashed amber segment labelled "not generated". Space, the arrows and Shift with an arrow are bound on the Simulate step when no text field has the keyboard. The tree gains an eye button per operation on Simulate and the line "Hiding an operation changes the view only. The simulation results do not change." The "Operations shown" check boxes of the playback panel are removed.
4. **Selection-set rows (C20).** A set with a problem shows the reason as a red sentence in the row, with "Reselect" in place of "Select in view" (OI-374, closed: agreed by the owner as built, relayed by the coordinator, 19 Oct 2026; the specification wording is left for the owner's later edit). Rename and Delete are in a row menu ("⋯"). A refused delete names the operations that use the set (`delete_refusal`). The details block keeps the rename field, the colour swatches and the tangent-edge choice.
5. **Boards.** `ImportCheck.dc.html` (image 01b, now 1280 × 760: the window over the main view), `Simulate.dc.html` (images 04 and 15: the speeds in a row of their own) and `SelectionSets.dc.html` (image 11: Replace set, Clear, the Same direction angle, the checking line and the row menu) are re-rendered from their sources with the bundled IBM Plex files, as step 1.

## Not built, and why

- **Selecting a mesh-check row highlights only its triangles (11.2).** The row selection is kept (`ImportSession::selected`), but the view draws all findings, as the engine's scene does today. Highlighting one finding needs a per-finding layer in the scene (OI-370).
- **A table of overcut and material left by operation (11.5).** The application's simulation reports the deviation per setup, not per operation. The table lists moves, volume removed and rapid-cut faults instead; per-operation deviation needs an engine change, which slice 22 excludes (OI-371).
- **"Go to overcut" and "Go to largest left", the hover probe and the G-code panel** are SIM-11 and SIM-12 (P2) and stay unbuilt, as the report says.

## Alternatives considered

- **Draw the import preview inside the window** (as the board did): rejected; section 3 and decision 0029 item 17 put the preview in the main view.
- **Keep the floating Results window opening after a run:** rejected; it would cover the inspector that replaces it.
- **A Max speed** (first built as the whole program in 1 second): removed on the owner's decision.

## Consequences

- `openchuck-ui`: new `results_panel.rs`; `import_panel.rs`, `playback_panel.rs`, `selection_panel.rs`, `run_panel.rs`, `shell.rs`, `shell/tree.rs` and the viewport's `last_rect` change. No other crate changes, so no changelog entry applies.
- `docs/ui-design/mockups`: three boards and four images re-rendered.
- Specification edit still to agree, one line: section 11.5's Speed line reads "1×, 10×, 100×, 1000×, 10 000× or Max, or the whole program in 10 s or 60 s" and should drop "or Max". It is not made here (OI-372 notes it).
