# 0129: Slice 22 Setup panes: the front and end views, drawn in 2D from a view model (lane X5)

- **Status:** accepted by the project owner on 19 Oct 2026, given directly in the lane X5 session, as built at the head of pull request 130. Lane X5, slice 22 ([decision 0114](0114-slice-22-conformance-report.md) section E, accepted; [decision 0105](0105-slice-22-conformance-pass.md), accepted). The owner decided items 4, 5 and 6 (OI-380, OI-381, OI-382) on 19 Oct 2026, relayed by the coordinator (decision 0049).
- **Date:** 19 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-380 to OI-383 are new. OI-334 stays open with its item (o), the Setup step's panes, built; OI-342 stays open with this lane's board re-rendered.

Numbering: the coordinator assigned this lane records 0129 and 0130 and OI-380 to OI-384. This lane uses 0129 and OI-380 to OI-383. Record 0130 and OI-384 are not used.

## Context

Decision 0114 C15 found the Setup step's two orthographic panes not built: the front view with the chuck, clearance, safe Z, travel limit and dimensions, the end view from +X with the A+ arrow, the held regions and tabs in section (7.5), and the legend strip. Only the workholding panel showed on Setup. Lane X3 built the 3D view's line styles, roles and widths (decision 0119), and lane X2 the Setup step and its legend strip (decision 0117).

Requirements: UI-03, UI-25, SET-01, SET-03, SET-05, SET-06, SET-09, NFR-07 and NFR-15. No requirement, architecture or project format change, and no engine change (slice 22 item 4).

## Decision

1. **The boards first (OI-342).** `Workholding.dc.html` (image 16) loses "Refused in the draft" from its legend, and both it and `Setup.dc.html` (image 02) lose the chuck (item 4): specification 7.5 lists only the held region and the tab. The solid that fails a check keeps its red outline in the panes, as 7.5 says. The image was rendered again with headless Chromium at 1440 by 900, with the bundled IBM Plex files in place of Google Fonts. (Plex Medium is not bundled, so weight 500 text uses Regular.)
2. **A view model in `openchuck-app`.** `openchuck_app::setup_panes` is new. `setup_panes(project, view, solids, failing)` returns plain 2D data in millimetres, in the model frame, for the setup a scene shows:
   - **Front view (from −Y; right is +X, up is +Z):** the stock's bounds, the part's side silhouette, the rotary axis, the work zero, the clearance cylinder (the setup's or the machine's, SET-06) with its radius, the safe Z, the Z+ travel limit, the held regions and tabs as outlines (a rectangle or a trapezoid), and the dimensions (stock length, stock diameter, axis to safe Z, "Held 6.000").
   - **End view (from +X; right is +Y, up is +Z):** the stock as a circle or a rectangle, the part's radius, the clearance circle, each tab as a circle at its largest diameter and its place, the A+ arc from the shared kinematics (the convention the posts use), and the first tab around the axis with "Start 90.00°", measured from +Y towards +Z (7.5).
   - **The legend:** the roles drawn, then "Held region" and "Tab" when the setup has them.
   - **The Z+ travel limit** is the machine's highest Z minus the rotary axis's Z for the setup's part offset (the same arithmetic as the setup check, SET-05).
   Two additions feed it: `SceneSolid` gains a `SolidSection` (its X range, radii and centre), set where `workholding_view::display_items` builds the solid, so the draft and the stored workholding draw the same way; and `SceneView` gains `part_extent` and `part_silhouette`, built in the scene job beside the part. The silhouette is the lowest and highest Z of the placed mesh at 160 strips along X, found where triangle edges cross the strip boundaries, so it costs nothing on the interface thread.
3. **Drawing in `openchuck-ui`.** `setup_panes.rs` draws the view model with egui's painter, in the front pane and the 270-pixel end pane side by side (11.3), over the viewport's dark gradient.
   - **Styles** are the renderer's own `LineStyle` patterns (stock and clearance dashed, rotary axis dash-dot, travel limit a one-direction hatched band) and the role colours and 1, 1.5, 2, 3 pixel widths of `roles.rs`; the clearance green `#4cc38a` is added there (7.4).
   - **A held region** is a translucent violet fill with the 6-pixel cross-hatch and a 1.5-pixel violet outline; **a tab** is opaque sand with a 1.5-pixel dark outline (7.5). A solid of the draft that fails a check is its red outline only.
   - **Hover and selection.** Pointing at a solid outlines it 2 pixels in hover yellow and opens its tooltip (7.5); a solid with keyboard focus in the inspector is outlined as a hover; the selected solid has the 3-pixel accent outline. A click selects it and is reported as the view's click is (`PickInput::solid_click`), so lane X4's wiring (OI-355) serves both.
   - **The draft.** The panes draw the draft's solids while the viewport holds one, with the chip "Showing unapplied workholding" at the top left (11.7).
   - **The shell** draws the panes in place of the 3D view on Setup, with the legend strip's entries from the view model (a swatch drawn the way each role is drawn). Other steps are unchanged.
4. **No chuck is drawn (OI-380, decided by the owner, relayed by the coordinator on 19 Oct 2026).** The project holds no chuck geometry, and the owner wants no extra chuck marking at this stage, since it would add visual clutter: the held region stays the only indication of what the chuck grips. This replaces the owner's first answer, an exclusion zone (built, then removed in this change), and is the original option B. Boards 2 and 2c (`Setup.dc.html`, `Workholding.dc.html`; images 02 and 16) are re-rendered without the chuck, so they match. Specification 11.3 still lists the chuck in the front view's contents: that text is left for the owner's later edit, and until then the code differs from it deliberately. A chuck needs a later data change (where it is and how far it reaches, a lane of its own with a schema claim).
5. **No viewport toolbar on the panes (OI-382, decided by the owner, relayed by the coordinator on 19 Oct 2026: option A, none for the first release).** Specification 11.3 says "two panes side by side" and board 2 shows the toolbar strip over the front pane. The toolbar's controls (shaded, wire, projection, standard views, fit, show or hide, turn part to A) act on the 3D view, so the panes have none. Options: (A) as built; (B) keep the strip and make "Fit" and "Show or hide" act on the panes. Chosen: A.
6. **The panes do not turn with A (OI-381, decided by the owner, relayed by the coordinator on 19 Oct 2026: option A).** Specification 7.5 says the solids turn with the part "in the machine view and with Turn part to A (UI-21, P2)". The panes are orthographic views at A = 0, the stored placement, which is what the setup edits (the first tab's "Start 90.00°" is an angle of the stored placement). Chosen: leave UI-21 (P2) to the 3D view.
7. **Default tabs offered are not in the panes yet (OI-383).** 7.5 draws them as dashed sand outlines in the view. The draft carries them as meshes without a section, so the panes need `SolidSection` on them too, which is lane X4's wiring of the draft. Until then the panes show the stored or draft tabs only.

## Alternatives considered

- **The front pane as the 3D view, set to ortho Front, with 2D lines over it.** Reuses the renderer, but a tab seen in 3D is a shaded solid, not the section outline 7.5 asks for, and the clearance, safe Z and dimensions would need the camera's matrices on the interface thread. Rejected.
- **Computing the panes in the scene job.** The draft changes with each edit, and the job's cost (a mesh read) would repeat for no reason. The job builds only what needs the mesh (the extent and the silhouette). The panes are worked out from those on each frame, in microseconds.

## Consequences

- `docs/open-items.md`: OI-380 to OI-383 added; OI-334's item (o) and OI-342's image 16 noted.
- `docs/work-breakdown.md`: lane X5's row and parallel-work row.
- `docs/ui-design/mockups/source/Workholding.dc.html` and `Setup.dc.html`, with `mockups/16-workholding.png` and `mockups/02-setup.png`, re-rendered.
- Tests: `crates/openchuck-app/tests/setup_panes.rs` and the unit tests of `setup_panes.rs` in both crates, and `req_ui_03_the_setup_step_shows_the_front_and_end_panes_with_their_legend` in the shell's frame tests. Pixel widths, contrast and the look of the hatch are *(display)* checks for lane X8.
- Not verified: no display was available to see the panes drawn; the geometry and the shapes are tested, the picture is not.
