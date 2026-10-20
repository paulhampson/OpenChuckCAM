# Issues for the owner to verify and close on a new build

Updated 21 Oct 2026. "Merged" = on main, so it is in the next nightly. "In PR" = fixed in an open pull request, not yet in a build. Close the issue yourself once you have checked it; if it is not fixed, reopen it or tell me and it joins the next lane.

## A. Merged, verify on the next nightly build

| Issue | What to check | Lane (PR) |
| --- | --- | --- |
| #139 | About window and CLI show the build information | BI (150) |
| #140 | Post and Export greyed out until the project is complete | MV (166) |
| #141 | Menus and 3D view controls work | MV (166) |
| #143 | (menus / view controls, see issue) | MV (166) |
| #144 | (see issue) | IW (161) |
| #145 | Windows only: import dialog behaviour | IW (161); note #222 moves the dialogs again (see C) |
| #147 | Layout shift | IW (161) |
| #148 | Tool library issue | X9 (169) |
| #152, #153, #159 | Selection usability (confirm before clearing, options in the lower window, selection mode) | BUG-SEL (164) |
| #156 | User action logging | UAL (163) |
| #157, #158 | View cube position and face labels | MV (166) |

## B. In a pull request, verify after it merges (nightly after merge)

| Issue | What to check | PR / status |
| --- | --- | --- |
| #196 | Removed or turned-off operation: its path is gone from viewport and is NOT in the exported G-code | #229 BUG-STALE, 0172 accepted, queued |
| #209 | Renaming an operation does not mark it stale | #229 |
| #189 | "Machine setting" wording in the Setup inspector | #229 |
| #218 | Window title "OpenChuckCAM - Name* - path" | #230 BUG-IMP, queued after #229 |
| #219 | New project: folder named after the project; warning when the folder exists | #230 |
| #220 | STEP files can be chosen in the import dialog; label says "choose a model" | #230 |
| #217 | Overcut histogram shows the 1.020 mm and has labelled bins | #231 BUG-RESULTS, records 0162/0163 await you |
| #208 | Hover tooltip with value; click keeps it | #231 |
| #174, #176, #177, #178, #179, #198, #212 | Inspector widths and scroll bar, view cube artifact, missing icons, legend wrap | #180 BUG-INSP, 0187 pending |
| #190, #191 | Tool library pane growth and scroll bar (did not reproduce in the rig; lane fixed the shared cause, OI-572) | #180 (Refs only) |
| #215 | Playback tab fully visible (dock height about 260) and does not move when the simulation runs | #180 (Refs; OI-570/571) |
| #200 | Toolpath rendering speed on your UHD 620 laptop (OI-590): run the nightly or `cargo run --release -p openchuck-viewport --example path_frames` | BUG-PERF (building); Refs only, you close it |

## C. Windows-only checks that keep an issue open

| Item | What to do |
| --- | --- |
| #221 (OI-582) | Command window flash with STEP loaded: confirm none on any action |
| #222 (OI-580) | Import step 2 not squashed; import, tool library and export open inside the main window |
| #145, #146, #147 | Re-check after #222 |
| OI-523 / OI-516 | Simulate step at 1366x768: Speed, Whole program and Go to move visible without scrolling |
| OI-359 | (same dock height, see above) |
| OI-430, 431, 440, 442, 455 | Earlier Windows/display checks from the conformance pass |
| OI-526 | Esc closes the tool library window; floating windows placed inside the window |
| OI-535 | Before/after look of the inspector label/scroll bar fix (no screenshots from the lane) |

## D. Not built yet (no action until a lane delivers)

#175 #181 #182 #184 #185 #186 #192 #193 #194 #195 #197 #199 #201 #202 #187 #188 #203 #204 #205 #206 #207 #210 #211 #213 #214 #216 (SIM-15 build) #223 #224 #225 #226 #227 #183.

BUG-HELD (#202, #187, #188) is in progress; BUG-RESULTS and BUG-PERF are in PR/building.

## E. Decisions and actions that are only yours

- OI-584: Unicode normalisation of folder names needs a crate that is not in the crate selection (NFR-14 justification).
- OI-303: crates.io token and the first publish.
- Reference-hardware benchmarks: OI-129, OI-173, OI-176, OI-264, OI-266.
- Z1 air cut and first cut (gates 4 and 5).
- Run `openchuck-cli simulate` on blisk_1_overcut_mismatch to see where the 1.020 mm overcut places are (OI-562 may link them to #202).
- Open answers: Output warning when stock simulation skipped (#214); Jobs tab vs status bar for progress (#201).
