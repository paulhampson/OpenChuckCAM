# Trial feedback: work packages for the owner to inspect

Prepared 20 Oct 2026 from the 72 issues in `paulhampson/OpenChuckCAM` (trial of nightly `50fd04ed0`, Windows). Source log: [trial-feedback.md](trial-feedback.md). **Nothing here has been sent to the coordinator or started.** The hold on lanes that have not started stays until the owner agrees a package.

Names with `BUG-` follow the owner's convention for bug fix lanes. All lane names are proposals; the coordinator takes decision, `OI-` and schema numbers. A feature that changes the project file needs a schema claim in the work breakdown first. Requirement and specification changes are proposed in the lane and the owner decides; none is edited to fit.

## A. Reported merged, still open on GitHub: verify on the next nightly

GitHub shows only #142, #146, #151 and #155 closed. The coordinator reports these lanes merged, so their issues are probably waiting for the owner's check on a nightly build (the closing keyword may be missing from the pull requests).

| Lane (PR) | Issues to verify |
| --- | --- |
| BI (150) | #139 About window and CLI build information |
| IW (161) | #144, #145 (Windows only: cannot be tested in the cloud), #147 |
| MV (166) | #140, #141, #143, #157, #158 |
| BUG-SEL (164) | #152, #153, #159 |
| UAL (163) | #156 |
| X9 (169) | #148 |
| BUG-INSP (draft 180, not released) | #174 only; the owner said it stays a draft |

If an issue is not fixed after the check, reopen it or say so, and it joins the package below that fits it.

## B. Proposed work packages (not started, held)

Order is a suggestion: safety first (NFR-12), then correctness, then what blocks daily use, then polish and features.

### WP1. Held region and workholding (safety, NFR-12, SET-09) — `BUG-HELD`
- #202 Toolpath generated inside the held region (roughing, at both ends, all the way through).
- #187 Held region modelled from the part, not the stock.
- #188 Workholding assumed cylindrical; box stock has no diameter to hold at.
- Owner decision first: box stock is held at its own outline over the gripped length (SET-09 wording is the owner's; schema claim if the file changes).
- Needs: the example project (#200 or #202). Test named after SET-09 that fails before the fix. Check every strategy, and whether the simulation and clearance use the wrong shape.

### WP2. Simulation results you rely on (safety, NFR-12) — `BUG-RESULTS`
- #217 Largest overcut (1.020 mm) does not match the histogram (-0.1 to +0.5 mm); label the bins. **First, high priority.**
- #208 Tooltip with the value on hover; click keeps it, click off the model hides it.
- #216 Band visibility of undercut and overcut (proposed P2 requirement; bands may be both bins and set ranges, TBD).
- Needs: the example project. Decisions 0113 and 0115 (deviation along the contact normal).

### WP3. Simulate controls and playback — `BUG-SIM`
- #203 "Turn part with A" on by default (existing projects too, remembered).
- #204 Check boxes for Model, Cut part, Toolpath (default Cut part only, last cannot be unticked).
- #205 Playback window (draggable, resizable, numeric entry, limits playback, outside shown faintly with a tick box to hide it, resets).
- #207 A readout 0 to 360, tooltip with the continuous value.
- #213 Toolpath visibility toggle per operation in all views, with show-all and hide-all (saved with the project: schema claim).
- #214 Stock simulation on by default, skip per project with a user default (schema claim).
- #215 Playback box moves down when the simulation runs (shares the layout cause of WP6).

### WP4. Performance and progress — `BUG-PERF`, `BUG-PROG`
- #200 Long toolpath rendering; UHD 620 is below the plan's 4.2 baseline, so report relative to the baseline; measure first; orbit and playback are slow. Example project needed.
- #201 One progress percentage across all stages with the operation named in the status bar; Jobs tab versus status bar options to be expanded in the lane.

### WP5. Positioning, tabs and the Setup diagram
- **Decision first, #175:** the Setup "Offset along X" has no effect and overlaps the Model step's translation options. Which control positions the part (SET-05)? Check how the translation reaches the setup, simulation and toolpaths (NFR-12).
- #181 Positioning row under the model in the tree (waits on the #175 decision).
- #182 Head and tail tab quick-create buttons (existing defaults; disabled with a tooltip when no room; a second press adds another).
- #186 Per tab auto-recalculation tick box (position and length; warning when no room; a manual edit unticks it; schema claim).
- #184 New tab diameter not rounded (the field precision).
- #185 Setup diagram pan and zoom, both panes, remembered, with limits.

### WP6. Layout, icons and drawing glitches — `BUG-INSP` follow-up (second collecting lane)
- One shared cause is suspected for the size and position faults, so fix it once and add a layout test.
- Inspector: #174 (draft PR 180), #178, #179, #198.
- Other panes: #190, #191 (tool library), #212 (legend wraps).
- Icons: #177 placeholder squares, with a whole-UI sweep.
- Drawing: #176 view cube artifact in axis-aligned views.

### WP7. Operations: creation and roughing — ordinary and `BUG-` names
- #195 Two-step New operation, strategy explanations in plain text only, with a skip tick box and a software setting.
- #197 Material preset from the tool (stock material default, warning triangle on mismatch, editing drops the preset).
- #223 Roughing default stock to leave (value decided in the lane).
- #199 Roughing ramp error that cannot be acted on (3 degree ramp, whole part, plunge works; reproduce on the project).
- #211 Extend the tool beyond the stock (option on existing strategies, user chooses ends or sides, up to the full tool diameter; a requirement is proposed first).

### WP8. Swarf — `BUG-SWARF` and a feature lane
- #206 Strategies that need a STEP part are disabled with the reason on an STL project (amends decision 0111).
- #226 Ruled but not developable face: a warning, produced with it, tolerance setting highlighted; the overcut stays visible (NFR-12); a decision record.
- #227 Check, avoid and boundary sets for swarf (P1; OP-02; amends decisions 0061 and 0111; how a boundary works and mesh sets are discussed in the lane).

### WP9. Status, validation and wording — `BUG-STALE`
- #196 Removing an operation leaves its toolpath drawn (check the simulation and G-code too).
- #209 Renaming marks an operation stale (check the G-code comment case).
- #210 Step bar colour: overall and per step, with shapes; an incomplete setup is red.
- #224 Highlight missing required values as soon as an operation is added.
- #189 "Machine setting" wording in the Setup inspector.
- One required-values list from each strategy's registration should drive #224, #210 and the Post and Export enabling (#140 to verify).

### WP10. Import and project files — `BUG-IMP`
- #220 STEP cannot be selected (the filter only offers STL; the label says "choose an STL").
- #221 Command window flashes with every action while a STEP is loaded.
- #222 Import step 2 squashed; the import, tool library and export dialogs go back inside the main window (amends decision 0029 item 17; the owner decides).
- #218 Project name and path in the window title.
- #219 New project: parent folder, then a sanitised folder named after the project.

### WP11. Tool library features
- #192 Machine section, type groups, foldable groups, "Standard tools" for tools with no machine, state remembered.
- #193 Filter by diameter and flute length (value and range, no reset).
- #194 Default tool profile shown read-only with dimensions, and a copy-to-edit action.

### WP12. Selection follow-up — `BUG-SEL` follow-up
- #225 All the lower dock's selection sets content moves to the inspector; the Selection sets dock tab is removed (amends the specification).

### WP13. Requirement proposals (the owner agrees before any work)
- #183 Simple math in number boxes (P2): + - * / and brackets, shows the result, rounds to the field precision. Open: invalid-expression error, units, crate justification.
- #216 (in WP2) and #156 (verify, user action logging P2) are the other P2 proposals.

## C. Decisions only the owner can make

| Decision | Issue | Package |
| --- | --- | --- |
| Which control positions the part | #175 | WP5 |
| Box stock held at its own outline (SET-09 wording) | #188 | WP1 |
| Dialogs back inside the main window (decision 0029 item 17) | #222 | WP10 |
| Strategies blocked on STL (decision 0111 amended) | #206 | WP8 |
| Swarf error becomes a warning (decision record) | #226 | WP8 |
| Swarf check, avoid and boundary sets | #227 | WP8 |
| Selection sets tab removed from the dock (specification) | #225 | WP12 |
| Math in number boxes (new requirement) | #183 | WP13 |
| Facing option (new requirement) | #211 | WP7 |
| Whether to lift the hold for the safety packages WP1 and WP2 | #202, #217 | WP1, WP2 |

## D. Needs from the owner

- Attach the example project to #200 or #202 (used by WP1, WP2, WP4, WP7), and the screenshot for #174 if missing. Check the attachment rules on those issues first.
- Verify the Group A lanes on the next nightly (and #145 on Windows).
- Open answers: Output warning when the stock simulation is skipped (#214), the Jobs tab versus status bar (#201), the window title format (#218), the new project folder rules (#219), the tick box memory (#204, #205).

## E. Notes

- #170 (nightly coverage) is not trial feedback and is left out.
- Schema claims needed: #213, #214, #186, #185 (if saved), #219 (if the layout changes), #227.
- Library API changes (decision 0048: bump the workspace minor version with a changelog entry): possibly #226 (error to warning).
