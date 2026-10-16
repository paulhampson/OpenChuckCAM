# 0038: User interface specification, and the requirements it needs

- **Status:** accepted by the project owner (15 Oct 2026): the specification, the requirements and their priorities, the verification rows, the resolutions of the specification's section 17 (D1 to D11), and the recommendations on OI-163 and OI-165. OI-164 and OI-166 had no recommendation and stay open.
- **Date:** 15 Oct 2026.
- **Branch:** `claude/compassionate-pascal-se4cct`. This is a documentation change, not a lane of the work breakdown's parallel plan; it informs lanes A, M, N, O, S and T.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-163 and OI-165 are closed by this record's acceptance, and OI-166 and OI-164 on 15 Oct 2026 (amendments below).
- **Amendment (15 Oct 2026, owner):** the default scale stays 120% (OI-166). UI-16 therefore states its 1366 by 768 at 100% scale, in `requirements.md`, the verification plan, the work breakdown's slice 8 and the specification's section 3; on a 1366 by 768 screen the user reduces the scale once, and it is remembered (UI-15).
- **Amendment (16 Oct 2026, merge with `main`):** decision 0037 was accepted on 15 Oct 2026 with the orange, green and yellow highlight colours. With OI-163 closed in favour of this specification, this record amends 0037 item 5: working selection blue, a swatch per set, hover a yellow outline (specification section 7.4, D7). Decision 0040 (accepted) colours the simulated part in six bands by triangle; the specification's continuous scale with tolerance-based thresholds differs. That is not decided and is OI-194 (specification D12); until then 0040's bands stand.
- **Amendment (15 Oct 2026, owner):** the typeface is IBM Plex Sans and Mono (OI-164). The licence is the SIL Open Font License 1.1 with the Reserved Font Name "Plex" (Copyright 2017 IBM Corp.), which allows the fonts to be bundled or embedded with the AGPL application as a separate work. Conditions: ship the unmodified font files with IBM's copyright line and the OFL text (`LICENSES/OFL-1.1.txt`, and a line in the release and nightly notices, which `cargo about` does not produce for files that are not crates); do not subset or otherwise modify them, since a modified font may not be called "Plex"; never sell them on their own; load them from the application, never from Google Fonts at run time (NFR-09). Recorded in `docs/dependencies.md` (bundled assets) and specification section 14.3; the bundling and notices are slice 8 item 10.

Numbering: `main` holds records up to 0033 and OI-153; unmerged branches hold records 0034 to 0037 and OI-154 to OI-162. This record therefore takes 0038 and OI-163 to OI-166. Unmerged branches also hold requirement IDs OP-20, OP-21 and SET-09, so the IDs proposed here avoid the OP and SET families. It takes no project schema number: nothing proposed here is stored in the project.

## Context

A user interface design was made for the desktop application: a design canvas of five workflow steps, an empty state, an import dialog, an operation error state, a tool library, a selection set panel, an undo history and a small-window check. [docs/ui-design/ui-specification.md](../ui-design/ui-specification.md) specifies it. The specification follows these records:

- the accepted records that touch the interface:
  - [0010](0010-slice-2-stl-import.md) (units, mesh checks, placement);
  - [0011](0011-slice-3-machine-stock-setup.md) (machine provenance);
  - [0014](0014-slice-4-tool-library.md) (tool library);
  - [0025](0025-slice-7-simulation.md) (simulation resolution);
  - [0026](0026-entry-moves.md) (entry moves);
  - [0028](0028-slice-19-output-options-and-setup-sheet.md) (output grouping, programmed time);
  - [0030](0030-slice-9-pipeline-cache.md) (stale and reused results);
  - [0032](0032-slice-10-mesh-selection.md) (selection roles, unresolved references);
- the proposed records [0029](0029-slice-8-viewport-and-shell.md) (the slice 8 shell as built) and 0037 (lane M, unmerged: picking and the selection panel).

The design was reviewed against the requirements. The existing UI, SIM, GC and NFR requirements cover:

- the window and platforms (UI-01, NFR-02);
- the viewport (UI-02, UI-03, NFR-03);
- selection (UI-04, UI-05);
- the tree (UI-06);
- the forms (UI-07);
- undo (UI-08);
- jobs (UI-09);
- toolpath display and simulation (SIM-01 to SIM-07);
- export (GC-02 to GC-06, GC-12, GC-14);
- the G-code notice (NFR-12).

They do not cover several things that the design needs, and that a desktop CAM tool needs to be usable and safe. Some are already half-decided in records without a requirement behind them:

- the status bar G-code notice and the remembered scale of 0029;
- the visible warning for an unresolved selection (0032 item 7, 0037 item 8);
- the machine provenance that 0011 stores but no requirement says to show.

Requirements section 7 also assumes "inch display and input are a user setting" with no requirement ID.

## Decision (accepted)

1. **Adopt the user interface specification** as the guidance for the interface lanes. On acceptance its status becomes "accepted", and it ranks with the decision records below the architecture (`docs/README.md` precedence 5). Where it disagrees with an accepted record, the record wins until one of them is changed, and section 17 of the specification lists the known differences with their proposed resolutions (D1 to D11).
2. **Add these requirements to `requirements.md`** (IDs reserved now, never reused). P1 is proposed only where the first release needs it to be usable or safe.

   | ID | Requirement | Priority | Section | Why |
   | --- | --- | --- | --- | --- |
   | UI-10 | Organise the work as ordered steps (model, setup, operations, simulate, post and export) that the user can move between at any time. A step whose inputs do not exist yet is unavailable and says why. | P1 | 3.2 | Makes the workflow of requirements section 2 visible and stops the user reaching a step that cannot work |
   | UI-11 | Show the state of each model, setup, selection set and operation (up to date, stale, running, error, warning, off) in the project tree, by shape as well as colour, with a legend that stays visible. An item in error says why and how to fix it. | P1 | 3.2 | Stale results and chain invalidation (architecture 6, decision 0030) and unresolved references (decision 0032 item 7) are invisible without it |
   | UI-12 | Collect errors, warnings and information from import, setup checks, file checks, generation and jobs in one messages panel, by severity, each naming its source and offering the action that fixes it. | P1 | 3.2 | Gives one home to the reports that records 0029, 0032 and 0037 each show in their own place |
   | UI-13 | Show, for each machine profile value, whether it is confirmed on the machine, taken from a vendor profile, or an unconfirmed placeholder, with how to confirm it. | P1 | 3.3 | The profile stores provenance (decision 0011 item 8), and an unconfirmed axis height moves every Z (OI-60); the user must see it where they set up |
   | UI-14 | Every command is available from the menus and the keyboard, every control can be used with the keyboard, and anything shown on hover is also shown on keyboard focus. | P1 | 3.2 | Basic operability and accessibility |
   | UI-15 | Keep per-user settings on the computer (window layout, scale, theme, display units, recent projects, export acknowledgements), never in the project. | P1 | 3.2 | Scale is not remembered today (decision 0029, owner's run), and GC-16 needs a place to store acknowledgements that does not travel with the project |
   | UI-16 | The window is usable at 1366 by 768 logical pixels, and its panels can be resized and collapsed. | P1 | 3.2 | Laptops are common in small shops; the design checks this size |
   | UI-17 | A command search that finds any command by name and runs it. | P2 | 3.2 | Speed for experienced users; not needed to be usable |
   | UI-18 | Display and enter lengths in inches as a user setting, and accept a unit typed with a value; values are stored in millimetres. | P2 | 3.2 | Formalises the requirements section 7 assumption; decision 0029 item 11 keeps it out of P1 |
   | UI-19 | Parameter forms group settings as basic and advanced, mark values that differ from their default with a way to reset them, and show derived values with how they were worked out. | P2 | 3.2 | Extends UI-07; the value sources of decision 0029 item 11 are the data |
   | UI-20 | A light theme beside the dark one. | P2 | 3.2 | Preference and glare; the viewport stays dark in both |
   | UI-21 | Measure distances, angles and radii in the viewport, cut the view with a section plane, and turn the part to a chosen A angle in the view without changing the program. | P2 | 3.2 | Inspection tools common to CAD and CAM |
   | UI-22 | Show the undo history and let the user return to any step in it. | P2 | 3.2 | Extends UI-08 |
   | UI-23 | A start page with recent projects, and import by dropping a file on the window. | P2 | 3.2 | First-run and return-to-work convenience |
   | UI-24 | When a selection set no longer matches its model, offer to select again, to show what is missing, and to keep the part that still matches. | P2 | 3.2 | Closes the loop of decision 0032 item 7 and OI-172 (lane M's "select again" item, numbered OI-161 when this record was written); full re-matching after re-export stays PRJ-07 |
   | SIM-11 | Read the deviation at any point of the simulated stock by pointing at it, with the operation and move that left it, and go to the worst overcut and the most material left. | P2 | 3.6 | Makes SIM-05's worst values findable on the part |
   | SIM-12 | Show the posted G-code beside playback, following the current move. | P2 | 3.6 | Supports the slice 9 check that move N matches its G-code line; SIM-09 (P3) runs G-code in the simulator, which this does not |
   | SIM-13 | Summarise each generated operation: moves, cut length, A travel, A unwinds and programmed time. | P2 | 3.6 | The data exists (the IR's contact length and durations, decision 0028 item 3) |
   | GC-15 | Refuse to write G-code while an enabled operation is stale or in error, and warn when an operation has not been simulated, listing the checks before writing. | P1 | 3.7 | A stale or failed toolpath must never be posted as current; complements GC-08 (P2), which checks the G-code itself |
   | GC-16 | Before the first export of a project on a computer, the user confirms that they will verify the G-code before running it. The confirmation is kept on that computer, not in the project, so another computer asks again; the notice itself is shown at every export. | P1 | 3.7 | Strengthens how NFR-12 is met in the UI without changing NFR-12; the owner asked for once per project, reset on a new machine |
   | NFR-15 | The interface is legible and accessible: text at least 12 pixels at 100% scale, scaling from 80% to 200%, contrast of 4.5:1 for text and 3:1 for meaningful graphics, no state shown by colour alone, and colour maps that differ in lightness. | P1 | 5 | Long sessions and colour vision deficiency; the deviation map is where an overcut is found |

3. **Verification rows** to add to `verification-plan.md` on acceptance:

   | ID | Method | Level | How |
   | --- | --- | --- | --- |
   | UI-10 | T, D | Integration | View-model test: each step's availability and reason for an empty project, a model only, a setup, a generated operation; demo script walks the steps |
   | UI-11 | T | Integration | View-model test: an edit to operation N marks N and later stale; a running job, an error from an unresolved set, a disabled operation each give their state |
   | UI-12 | T | Integration | Seeded file change, mesh finding, setup check and generation error each give one message with source and action, sorted by severity; fixing the cause removes it |
   | UI-13 | T | Unit | The machine panel view model gives confirmed, vendor or unconfirmed for each field from `provenance`, unlisted fields as unconfirmed |
   | UI-14 | I, D | System | Inspection that every menu command has a shortcut or menu path; keyboard-only demo script through the five steps |
   | UI-15 | T | Unit | Settings round-trip in the configuration folder; a missing or damaged file gives defaults and a message; nothing is written to the project folder |
   | UI-16 | D | System | Screenshot at 1366 by 768: no overlay overlaps another, no clipped text in the reference screens |
   | GC-15 | T | Integration | Export is refused with a stale or failed enabled operation, allowed with a warning when unsimulated, and lists disabled operations as not posted |
   | GC-16 | T | Integration | The acknowledgement is asked once per project per settings folder, not asked again in the same folder, asked again with a fresh folder, and never written to the project |
   | NFR-15 | A, D | System | Contrast analysis of the theme tokens and colour roles; scale demo at 80% and 200% |
   | P2 rows (UI-17 to UI-24, SIM-11 to SIM-13) | T or D | | Added when each is scheduled |

4. **Where the work goes.** As placed in the work breakdown on acceptance: UI-10 to UI-16 and NFR-15 in slice 8 (items 8 to 12), with UI-14 and NFR-15 closing at slice 20 (UI-11 and UI-12 were first placed in slice 9, and moved to slice 8 on 16 Oct 2026 because slice 9 closed before they were built); GC-15 and GC-16 in slice 19 (item 4). The proposal was:
   - UI-10, UI-11, UI-12, UI-15 and UI-16, and the footer and primary-action rules: lane A (the rest of slice 8) or a new interface lane after it.
   - UI-13: lane A (the machine panel).
   - UI-14 and NFR-15: slice 8 for the shell, checked again in slice 20.
   - GC-15 and GC-16: lane S (export dialogs, OI-144).
   - SIM-11 to SIM-13: lane N, as P2.
   - UI-24: lane M or O, as P2.
   - The other P2 items: after the first release.

## Alternatives considered

- **Keep the specification only on the design canvas.** Rejected: the canvas is private and is not reviewed with the code; `docs/` is the source of truth (`docs/README.md`).
- **Fold the new behaviour into UI-06, UI-07 and NFR-12 by rewording them.** Rejected: requirement IDs are stable and their verification rows are written; new IDs keep the history and let each be prioritised.
- **Make the safety acknowledgement once per export, or once per installation.** Once per export trains users to click through; once per installation is too weak for a new project. The owner chose once per project, reset on a new computer.
- **Store the acknowledgement in the project.** Rejected: the project travels with Git (PRJ-01, PRJ-05) and would carry one user's confirmation to another computer.
- **Make inch display P1.** Rejected for now, as decision 0029 item 11 did: everything is millimetres internally (NFR-05), and the Z1 post writes millimetres only (OI-89).

## Consequences

- **Made with the acceptance (15 Oct 2026):**
  - `requirements.md` gains the rows above. Sections 3.2, 3.3, 3.6, 3.7 and 5 are touched, and the requirement counts in the work breakdown change.
  - `verification-plan.md` gains the rows above.
  - `docs/ui-design/ui-specification.md` is accepted, with rendered mockups in `docs/ui-design/mockups/`.
  - The work breakdown's slices 8 and 19 (first 8, 9 and 19; see item 4), its traceability table and counts (91 of 144 requirements P1), and `xtask/traceability.toml` (UI-14 and NFR-15 close at slice 20).
  - Requirements section 7's inch assumption now cites UI-18.
- **The specification's differences from the build (section 17)** are work for the lanes named in item 4. D1 changes a colour in the slice 8 viewport. D7 means lane M changes the highlight colours of its proposed decision 0037.
- **No new dependency.** Settings use `toml`, already in the workspace, in the configuration folder of decision 0029 item 15. Bundling the mockups' typeface would need one (OI-164).
- **Nothing added is stored in the project,** so no schema number is needed: the acknowledgement is keyed by the project folder path (OI-165).

## Open items

| ID | Item | Decide in | Needs |
| --- | --- | --- | --- |
| OI-163 | **Closed 15 Oct 2026: the specification's colours.** Selection highlight colours: decision 0037 (proposed) uses orange for the working selection, green for the chosen set and yellow for hover. The specification proposes blue, per-set swatches and a yellow outline, because amber is cut moves and green is the Y axis, link moves and the up-to-date state | With lane M's review | Owner's choice |
| OI-164 | **Closed 15 Oct 2026: IBM Plex, bundled under the OFL.** Typeface: the mockups use IBM Plex Sans and Mono (OFL-1.1); egui ships its own fonts, which `deny.toml` allows. Bundling Plex adds font files and a licence record (NFR-10, NFR-14) | Lane A or T | Owner's choice |
| OI-165 | **Closed 15 Oct 2026: the folder path.** How the export acknowledgement (GC-16) identifies a project on a computer: the canonical project folder path (no schema change; moving or renaming the folder asks again) or a project ID in `project.toml` (a schema change). Recommended: the folder path | Lane S | Owner's choice |
| OI-166 | Slice 8 starts at 120% scale; at that scale a 1366 by 768 screen gives about 1138 by 640 logical pixels, below the UI-16 size. Choose the default scale from the screen size, or set UI-16 at 100% | Lane A | Owner's choice |
