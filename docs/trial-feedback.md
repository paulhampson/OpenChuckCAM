# Trial feedback log

Source: trial of a nightly build from commit `50fd04ed0`. Raw feedback, not yet triaged against requirements. Task list and work packages are not yet created.

Convention (owner, 19 Oct 2026): from now on, bug fix lanes generated from the trial feedback are prefixed `BUG` (for example `BUG-SEL`). Lanes already named by the coordinator keep their names.

| ID | Area | Issue | Details and expectations | Open questions |
| --- | --- | --- | --- | --- |
| F-01 | About window, CLI | No build traceability. | Show commit hash, dirty flag, build date, Rust version and target triple, library versions. Scrollable licence text box. Copy button. CLI `--version` has the same gap. Candidate new requirement; relates to decision 0110 (traceability data in output). | Confirm where the build info is generated. |
| F-02 | File menu | Post and Export enabled while project is incomplete. | Reported with only model and setup steps done. Clicking opens the dialog; the Write G-code button is greyed out, so no G-code is produced (not an NFR-12 safety issue). Expected: menu item greyed out until all required information is present. | Define the "required information" list. Tooltip naming what is missing (proposed). |
| F-03 | View menu | Menu too long at 1920x1080 (100% and 120% UI scale). | Group multi-option items, such as view directions, into submenus. Disable view directions that do not apply in the current view (for example Setup). Scale: whole-UI scaling as a submenu in the View menu with fixed options 80, 90, 100, 110, 120 % (revised; an Excel-style control on the main UI is no longer wanted). | Default and persistence of the chosen scale (proposed: 100 %, remembered between sessions). |
| F-04 | Setup view | Cramped; tab and held-region labels overlap. | Seen with the blisk STL: "Tab 1" and "Held 5.000" overlap near the chuck end; "40.000" and "A axis" are close. Suggested fix: callouts (offset labels with leader lines). Check UI-25 and decision 0104. | Does it happen with other parts? Window size and scale of the screenshot. Which labels must stay visible. |
| F-05 | 3D viewports | View selector inconsistent between views. | Model has front / top / iso; Operations (and others) do not. The view selector should be the same across all 3D viewports. Related to F-03 (view directions disabled where unsupported). | Which views should get it (Setup, Operations, Simulation, others)? |
| F-06 | Import | "Check" button label does not convey what happens. | The button says "Check", but the import flow then moves to the positioning step, so it looks as if the model has already been imported. Wording left open ("Check and Import" was one suggestion); the label must make clear what the step does. | Final wording; which controls carry the label; whether the check can stop the import. |
| F-07 | Import dialog | Dialog is lost when the app is backgrounded. | During the next step of the import, if the app goes to the background and is then returned to, the import model dialog is no longer in front and the app feels stuck. Expected: the dialog is brought to the front again when the window regains focus in that state. | Trial is Windows only; other platforms not tested. Dialog is hidden (backgrounded) behind the main window, not gone. |
| F-08 | Import dialog | Dialog is resizable and opens too tall. | Expected: fixed size, with a sensible default height that is not too tall. | Content may need to scroll (for example long check results); the fixing task decides the best approach. Check at 1920x1080 and at 120% UI scale. |
| F-09 | Import dialog | Loading indicator shifts the layout. | A "refreshing" indicator appears in the import model dialog and pushes the other elements around. When it shows for only a split second, the dialog looks like it is glitching. Expected: no layout shift (reserve space for it, or show it without moving other elements). | Triggered by changing one of the drop-down settings for positioning the part (positioning step). Related to F-08 (fixed-size dialog). |
| F-10 | Project tree, tool library | Tools should not be in the project tree. | Unexpected to find tools in the project tree. Expected: a separate tool library dialog that lists tools and supports managing them. Tools should be associated with operations. **Conflicts with UI-06** (requirements: project tree lists "the model, stock, setups, operations and tools"), and touches UI specification 12.1 (tool library) and TL-01, TL-07, TL-11. The UI-06 conflict is to be resolved as part of the work on this issue (owner direction), by proposing the requirement change there; do not edit requirements.md to fit. | Answered: the tool library dialog cannot be reached at all in the trial build (a separate defect, or a gap in the UI wiring; check how UI-spec 12.1 is meant to open it). The snapshot state (TL-07) need not be in the tree, only indicated in the operation inspector. Management actions expected: add, edit, delete, duplicate, import/export. |
| F-11 | Selection | Edge and triangle modes unclear; hover targets hard to tell apart. | The distinction between edge selection and triangle selection is unintuitive. Expected: clearer indication of the active mode and of what is being hovered over. Issue [#151](https://github.com/paulhampson/OpenChuckCAM/issues/151). | Answered: triangles and edges are highlighted in the same colour today; use a different colour for each. Other cues (mode label, cursor) left to the fixing task. |
| F-12 | Selection | "Start new selection" triggers only on mode switch. | Expected: it triggers whenever a selection would be cleared. Test: the user is not adding to or removing from a selection (no shift or control held) while elements are already selected, so the modifier key was probably forgotten. Issue [#152](https://github.com/paulhampson/OpenChuckCAM/issues/152). | Answered: warn and ask for confirmation. Applies to any click that might clear or reset the selection, except explicit clear buttons. Reason: avoid losing a high-effort manual selection of many triangles to a misclick. |
| F-13 | Selection | Selection parameters and options are in the lower window. | Expected: an inspector on the right-hand side. Issue [#153](https://github.com/paulhampson/OpenChuckCAM/issues/153). | Check the UI specification and the X4 inspector layout for the intended place. |
| F-14 | UI, Windows | The UI opens a CMD window. | Expected: no console window; the information it prints goes into a log file. Issue [#155](https://github.com/paulhampson/OpenChuckCAM/issues/155). | Answered: the log file goes in the same location as the binary; verbosity left as is; the About window gets a button to open the log; the CLI keeps printing to the console. Not yet answered: rotation or size limit. |
| F-15 | Logging | User action logging (proposed P2 requirement). | So bug fixes can use accurate step information. Builds on F-14's log file. Issue [#156](https://github.com/paulhampson/OpenChuckCAM/issues/156). | Needs a requirement ID and an owner decision; what may be recorded (paths, model names). |
| F-16 | 3D viewport | XYZ axes gadget placement. | Move it to the bottom-left corner of the 3D viewport and do not draw it in a box. Issue [#157](https://github.com/paulhampson/OpenChuckCAM/issues/157). | Answered: applies to every 3D viewport. Relates to #143 and lane X3. |
| F-17 | View cube | Face labels always face the user. | Top/Back/Front/Left/Right/Bottom text should lie flat on the cube faces; today it faces the user and snaps in and out of view. Issue [#158](https://github.com/paulhampson/OpenChuckCAM/issues/158). | |
| F-18 | Selection, viewport | Pick mode control overlaps the toolbar; not clearly selectable. | When selecting faces, the mode selection overlaps the top menu bar of the 3D viewport. It is not obvious that "Pick" is a label and Both / Faces / Edges are selectable options. Issue [#159](https://github.com/paulhampson/OpenChuckCAM/issues/159). Same control as F-11. | |
| F-19 | Inspector | Wasted space; entry boxes cramped right; scroll bar overlaps them. | Seen in the Stock section of the Setup step: the label column is wide, the entry boxes are squeezed to the right, so the scroll bar overlaps them. Expected: entries use the available width and the scroll bar does not overlap. Issue [#160](https://github.com/paulhampson/OpenChuckCAM/issues/174). Lane X4 (inspectors, PR 154) may already cover it. Screenshot to be attached by the owner. | |

## Task list (work packages)

Issues are in `paulhampson/OpenChuckCAM`. Lane names are proposals for the coordinator: the `X` lanes are slice 22 lanes in the work breakdown (Parallel work), and the coordinator confirms or changes the mapping and takes any decision, `OI-` and schema numbers (none are claimed here).

| ID | Issue | Log ID | Type |
| --- | --- | --- | --- |
| WP1 | Import dialog (proposed lane: X6, "one import window") | F-06, F-07, F-08, F-09 | |
| WP1.1 | [#146](https://github.com/paulhampson/OpenChuckCAM/issues/146) Import dialog fixed size, sensible default height (do first: sets the layout for the rest) | F-08 | Bug |
| WP1.2 | [#147](https://github.com/paulhampson/OpenChuckCAM/issues/147) Loading indicator must not shift the layout | F-09 | Bug |
| WP1.3 | [#144](https://github.com/paulhampson/OpenChuckCAM/issues/144) "Check" label does not convey what happens (wording open) | F-06 | Bug |
| WP1.4 | [#145](https://github.com/paulhampson/OpenChuckCAM/issues/145) Dialog hidden when the app is backgrounded (Windows only tested) | F-07 | Bug |
| WP2 | Menu bar (proposed lane: X2, shell frame) | F-02, F-03 | |
| WP2.1 | [#140](https://github.com/paulhampson/OpenChuckCAM/issues/140) Grey out Post and Export until the project is complete (define "required information") | F-02 | Bug |
| WP2.2 | [#141](https://github.com/paulhampson/OpenChuckCAM/issues/141) View menu: submenus, disable unsupported view directions, UI scale submenu 80/90/100/110/120 % | F-03 | Feature |
| WP3 | Viewport (proposed lane: X3, viewport drawing; F-05 shares the view directions with WP2.2) | F-04, F-05 | |
| WP3.1 | [#143](https://github.com/paulhampson/OpenChuckCAM/issues/143) Same front / top / iso selector in every 3D viewport | F-05 | Bug |
| WP3.2 | [#142](https://github.com/paulhampson/OpenChuckCAM/issues/142) Setup view labels overlap: use callouts (check UI-25, decision 0104; screenshot to be attached by the owner) | F-04 | Bug |
| WP4 | Tool library (proposed lane: X9, tool library window 12.1) | F-10 | |
| WP4.1 | [#148](https://github.com/paulhampson/OpenChuckCAM/issues/148) Tools out of the project tree; tool library dialog (currently unreachable); tools associated with operations; snapshot state (TL-07) in the operation inspector. Resolves the UI-06 conflict as part of the work (owner direction). May split out the unreachable dialog as its own defect | F-10 | Feature |
| WP5 | Build information (new lane; no requirement yet, propose one; relates to decision 0110) | F-01 | |
| WP5.1 | [#139](https://github.com/paulhampson/OpenChuckCAM/issues/139) About window and CLI `--version`: commit hash, dirty flag, build date, Rust and target triple, library versions, scrollable licence text, copy button | F-01 | Feature |

Dependencies and notes
- WP1.1 before WP1.2 (a fixed layout decides how the indicator space is reserved).
- WP2.2 and WP3.1 both decide which views support which view directions; do them in one decision or sequence them so the view-direction rules are defined once.
- WP4.1 and WP2.1 both touch the project tree and tool list, and WP4.1 changes what "required information" means for export (a tool per operation).
- WP5.1 touches `openchuck-ui` and `openchuck-cli` only through a shared build-info source.

Later feedback (sent to the coordinator for mapping)
| ID | Issue | Log ID | Type |
| --- | --- | --- | --- |
| WP6 | Selection (lane to be assigned by the coordinator; relates to slice 10 lane M: picking, hover, selection panel) | F-11, F-12, F-13 | |
| WP6.1 | [#151](https://github.com/paulhampson/OpenChuckCAM/issues/151) Mode indication and hover distinction | F-11 | Bug |
| WP6.2 | [#152](https://github.com/paulhampson/OpenChuckCAM/issues/152) New-selection notice whenever a selection would be cleared | F-12 | Bug |
| WP6.3 | [#153](https://github.com/paulhampson/OpenChuckCAM/issues/153) Selection options in a right-hand inspector | F-13 | Bug |

| WP7 | UI logging (lane to be named `BUG-...` by the coordinator) | F-14 | |
| WP7.1 | [#155](https://github.com/paulhampson/OpenChuckCAM/issues/155) No console window on Windows; output to a log file | F-14 | Bug |

Lane names confirmed by the coordinator (19 Oct 2026): BI = WP5 (#139, started); IW = WP1 (#146 first, #147, #144, #145; after X6 #129 and X4); MV = #140, #141, #143; SL = #142 (needs the owner's screenshot); X9 widened to #148; BUG-SEL = WP6 (#151, #152, #153; after X4 and SU; records 0153-0154, OI-448..452); BUG-LOG = WP7 (#155; records 0155-0156, OI-453..457; does not depend on X4).

| WP8 | User action logging (feature, P2 proposal; ordinary lane name) | F-15 | |
| WP8.1 | [#156](https://github.com/paulhampson/OpenChuckCAM/issues/156) User action logging | F-15 | Feature |

The owner attached the Setup view screenshot to #142 (19 Oct 2026); lane SL is unblocked on that point.

| WP9 | View gadget and cube (lane to be named `BUG-...` by the coordinator; relates to MV #143 and X3) | F-16, F-17 | |
| WP9.1 | [#157](https://github.com/paulhampson/OpenChuckCAM/issues/157) Axes gadget bottom-left, no box | F-16 | Bug |
| WP9.2 | [#158](https://github.com/paulhampson/OpenChuckCAM/issues/158) Face labels flat on the cube | F-17 | Bug |

| WP6.4 | [#159](https://github.com/paulhampson/OpenChuckCAM/issues/159) Pick mode control overlaps the toolbar; label versus options (suggested for BUG-SEL with #151) | F-18 | Bug |

Coordinator decision (19 Oct 2026): WP9 (#157, #158) is folded into lane MV, which starts after X4 merges.

| WP10 | Inspector layout (probably X4, PR 154; else a BUG- lane) | F-19 | |
| WP10.1 | [#174](https://github.com/paulhampson/OpenChuckCAM/issues/174) Inspector entry boxes use the width; scroll bar must not overlap | F-19 | Bug |

Lane BUG-INSP (Sonnet, branch `claude/lane-bug-insp-field-width`, PR titled "Lane BUG-INSP:") was started by the coordinator for #174 (WP10) on 20 Oct 2026 after X4 (PR 154) merged and did not fix it. The owner had asked that no work package start until the owner gives the go-ahead; whether BUG-INSP may continue is an open question for the owner. The owner attached the screenshot to #174.

Owner decision (20 Oct 2026): BUG-INSP continues local work with no push or PR yet; further bug fixes will be added to it before it is pushed. The hold on lanes that have not started stays until the owner gives the go-ahead.

| ID | Area | Issue | Details and expectations | Open questions |
| --- | --- | --- | --- | --- |
| F-20 | Setup inspector | "Offset along X" does not move the part in the diagram. | In the Part position section of the setup inspector, changing the offset along X leaves the part where it was in the Setup diagram. Issue [#175](https://github.com/paulhampson/OpenChuckCAM/issues/175). Planned lane: BUG-INSP (WP10). | Answered: no effect at all, even in the 3D view; it is the only Part position field; the diagram does not catch up later. Raised in priority: a setup value appears to be ignored, so check the simulation, toolpaths and G-code (NFR-12). Later note (owner): the offset seems to overlap with the part translation options, so a decision is probably needed; the translation options are in the model inspector (SET-05; UI specification section 11 item 3 names rotation about X and offset along X): which control is the single place that positions the part. |
| F-21 | View cube | Rendering artifact in some orientations. | In the top-right view cube, a horizontal line crosses the cube box and the faces (FRONT, RIGHT) are drawn as flat panels with a gap above, as if clipped or drawn partly outside the box. Issue [#176](https://github.com/paulhampson/OpenChuckCAM/issues/176). Related to #158 and #157 (lane MV). Screenshot to be attached by the owner. | Answered: vertical and side-on orientations trigger it, and it also happens looking vertically (the axis-aligned views). Not yet answered: every 3D viewport? Screenshot is in `docs/trial-feedback-images/f-21-view-cube-artifact.png`. |
| F-22 | UI, icons | Missing icons shown as placeholder squares. | Seen in the Retaining tabs row (Setup inspector), at the right end of the bottom dock tab row, and in the viewport toolbar ("Show" drop-down). The owner asks for the whole UI to be audited for missing icons. Probably glyphs missing from the bundled font (lane X7a). Issue [#177](https://github.com/paulhampson/OpenChuckCAM/issues/177); screenshots in `docs/trial-feedback-images/f-22-*.png`. | Cause to confirm (font coverage and Windows fallback). |
| F-23 | Inspector | Misaligned when dragged fully to its leftmost position. | In the Setup step (Machine and Stock sections), section headings, field labels, buttons and the warning row start at different left edges and text is cut off. Expected: aligned, unclipped content at every inspector width. Issue [#178](https://github.com/paulhampson/OpenChuckCAM/issues/178); screenshot `docs/trial-feedback-images/f-23-inspector-misaligned-wide.png`. Planned lane: BUG-INSP. Related to F-19. | Answered: window maximised on a 1920x1080 screen; exact panel width not known. |
| F-24 | Inspector | Width resize handle shown, but the panel springs back. | The inspector offers width adjustment, but it returns to its original size. Owner direction: do not allow resizing if it is not supported. Issue [#179](https://github.com/paulhampson/OpenChuckCAM/issues/179). May share a cause with F-23. Planned lane: BUG-INSP. | Whether any width range is meant to be supported (check the UI specification). |
