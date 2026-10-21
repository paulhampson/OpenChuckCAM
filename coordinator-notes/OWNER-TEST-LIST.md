# Issues for the owner to verify and close on a new build

Updated 21 Oct 2026. Nightly run 13 was triggered on main at eced9a1 (https://github.com/paulhampson/OpenChuckCAM/actions/runs/39621031609); it contains everything marked "in nightly 13" below. Close each issue yourself once you have checked it; if it is not fixed, reopen it or tell me and it joins the next lane.

## A. Earlier lanes, verify on the nightly

| Issue | What to check | Lane (PR) |
| --- | --- | --- |
| #139 | About window and CLI show the build information | BI (150) |
| #140 | Post and Export greyed out until the project is complete | MV (166); also #234 |
| #141, #143 | Menus and 3D view controls | MV (166) |
| #144, #147 | Layout issues (see issues) | IW (161) |
| #145 | Windows only: import dialog; #222 moved the dialogs inside the main window | IW (161), #230 |
| #148 | Tool library issue | X9 (169) |
| #152, #153, #159 | Selection usability | BUG-SEL (164) |
| #156 | User action logging | UAL (163) |
| #157, #158 | View cube position and face labels | MV (166) |

## B. In nightly 13: verify and close

| Issue | What to check | PR |
| --- | --- | --- |
| #196 | A removed or turned-off operation: its path is gone from the viewport AND is not in the exported G-code | #229 |
| #209 | Renaming an operation does not mark it stale | #229 |
| #189 | "Machine setting" wording in the Setup inspector | #229 |
| #218 | Window title "OpenChuckCAM - Name* - path" | #230 |
| #219 | New project: folder named after the project; warning/choice if it exists | #230 |
| #220 | STEP files can be chosen in the import dialog; label says "choose a model" | #230 |
| #217 | Histogram has labelled bins and the extremes; regenerate and re-simulate your blisk project and compare (the 1.020 mm overcut was pencil finishing through blade tips, fixed) | #231 |
| #208 | Hover tooltip with the value; click keeps it; click off hides it | #231 |
| #202, #187, #188 | Held region = stock section (or the holding diameter as a cylinder); no cutting move over the gripped length; ramp entries beside a held region are refused with a message naming the held region. NOTE: your project has no END held region (the tail is a tab): lengthen the stock, delete the tail tab, turn on "Held at the end" | #233 |
| #210, #224 | Step bar health colours and per-step status; missing required values marked as soon as an operation is added | #234 |
| #174, #176, #177, #178, #179, #198, #212 | Inspector widths and scroll bar; view cube artifact; missing icons; legend wrap; typing in a box does not widen it | #180 |
| #190, #191, #215 | Tool library pane growth and scroll bar (could not be reproduced in the rig); playback tab fully visible at the new dock height 260, nothing moving when the simulation runs | #180 (Refs only: you close them) |
| #206, #226 | STEP-only strategies are disabled with the reason on an STL project; swarf overcut above tolerance is a warning (tolerance field highlighted); Write G-code also writes the setup sheet (HTML + PDF) | #235 |
| #200 | Toolpath rendering speed on your UHD 620 laptop (OI-590): `cargo run --release -p openchuck-viewport --example path_frames` and orbit/playback in the nightly | #232 (Refs only: you close it) |

## B3. Second nightly (run 14, main f213d19): additionally in this build

https://github.com/paulhampson/OpenChuckCAM/actions/runs/39640543774 (in progress when written)

| Issue | What to check | PR |
| --- | --- | --- |
| #203 | "Turn part with A" on by default (existing projects too), remembered | #238 |
| #204 | Simulate Show ticks Model / Cut part / Toolpath; default Cut part only; the last tick cannot be unticked; ticks remembered; the deviation colours count as Cut part | #238 |
| #205 | Playback window: draggable and resizable, numeric start/end, limits playback, outside the window faint or hidden (remembered tick), window resets. KNOWN BUG OI-605: dragging out a NEW window leftwards takes one move too many at each end, and dragging one end past the other collapses the window; the number fields are fine; fix queued | #238 |
| #207 | A readout 0..360 with a tooltip of the continuous value | #238 |
| #213 | Eye toggle per operation in all views, show-all/hide-all, saved with the project (schema 15) | #238 |
| #214 | Stock simulation on by default; skip is per project; skipping needs an acknowledgement before writing | #238 |

## B4. Third nightly (run 15, main 452724e): additionally in this build

https://github.com/paulhampson/OpenChuckCAM/actions/runs/39663424881

| Issue | What to check | PR |
| --- | --- | --- |
| #227 | Swarf honours boundary / avoid / check FACE sets with per-set options (boundary: limits the pass, limits the cut length at the tip; avoid/check: Cut short / Warn only / Ignore); project schema 14; library 0.14.0 | #240 |
| #239 | Changing a later operation's parameter no longer replays the roughing's simulation; the status bar says "Re-running the simulation for <operation>" when a replay is needed | #247 |
| #243 | In the operation inspector the revert button appearing does not move the label | #247 |
| #241 | The About window of THIS nightly should say "Uncommitted changes: no" (the dirty flag now counts tracked changes only; occt-install is ignored). The lane keeps #241 open until the nightly confirms: you close it (OI-710) | #247 (Refs) |

## C. Windows-only checks that keep an issue open

| Item | What to do |
| --- | --- |
| #221 (OI-582) | Command window flash with STEP loaded: confirm none on any action |
| #222 (OI-580) | Import step 2 not squashed; import, tool library and export open inside the main window |
| #145, #146, #147 | Re-check after #222 |
| OI-523 / OI-516 | Simulate step at 1366x768: Speed, Whole program and Go to move visible without scrolling |
| OI-430, 431, 440, 442, 455 | Earlier Windows/display checks from the conformance pass |
| OI-526 | Esc closes the tool library window; floating windows placed inside the window |
| OI-535 | Before/after look of the inspector label/scroll bar fix (the lane had no screenshots) |
| OI-571, 572, 573 | Remaining inspector items from the nightly check |

## C2. Known issues in the second nightly (#238), fix pending

- OI-605 (playback window): dragging out a NEW playback window leftwards takes one move too many at each end (moves 19 to 40 where the band reads 20 to 39); dragging either end past the other end collapses the window on the next pointer move. The number fields ("Window: moves [from] to [to]") are fine. Fix ready (BUG-SIM follow-up PR, after #238 merges).

## D. Open draft PRs

| PR | Waiting on |
| --- | --- |
| #238 BUG-SIM (#203, #204, #205, #207, #213, #214) | You accept records 0168 and 0169 in the BUG-SIM session (session_01Mac8EW13nNLUpRmoNU7uTN). Schema 15. Then release; second nightly after it merges |
| #227 PR (BUG-SWARF second PR, branch claude/lane-bug-swarf-227, schema 14) | You say "go" and accept 0176 in the BUG-SWARF session (session_017VjsYJeoNcEsg2BMa1hp3z) |
| #237 REQ-VOL (docs-only, P2 requirement OP-22 for user-defined volume sets, Refs #236) | You accept 0188 in the REQ-VOL session (session_01D5EbVtTtiTeyn8JeCUsWqQ) |

## E. Not built yet

#175 #181 #182 #184 #185 #186 #192 #193 #194 #195 #197 #199 #201 #211 #223 #225 #227 #183 #216 (SIM-15 build deferred). #236 is only a P2 requirement (OP-22). Lane TB (setup sheet traceability block, GC-12 item 6), the OI-554 warning (grip reaching into the part) and the tool-library lane are queued; UIR and REL are held.

## F. Decisions and actions that are only yours

- OI-584: Unicode normalisation of folder names needs a crate that is not in the crate selection (NFR-14 justification).
- OI-303: crates.io token and the first publish.
- Reference-hardware benchmarks: OI-129, OI-173, OI-176, OI-264, OI-266, OI-590, OI-604.
- Z1 air cut and first cut (gates 4 and 5).
- Run `openchuck-cli simulate` on blisk_1_overcut_mismatch if you want to see the overcut places before and after (OI-562).
- Open answer: Jobs tab vs status bar for progress (#201).
