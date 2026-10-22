# 0203: The Setup panes zoom and pan (lane SETUPPOS; issue #185)

- **Status:** accepted (22 Oct 2026, by the owner directly in the lane SETUPPOS session: "I accept records 0201,0202,0203"). Items 1 and 3 are the owner's answers on issue #185, relayed in its comment of 20 Oct 2026.
- **What it amends:** the interface specification's section 11.3 (edited with this record) and [decision 0129](0129-slice-22-setup-panes.md), whose panes were fitted only. No requirement changes (UI-03).
- **Date:** 21 Oct 2026.
- **Open items:** none.

## Context

Trial feedback F-29 (issue #185): the Setup diagram, the front and end panes of the Setup step (decision 0129, lane X5), had no zoom or pan, which matters most where labels crowd (issue #142). The owner answered: both panes; zoom and pan are remembered, not reset on leaving the step; zoom has a minimum and a maximum. Whether "remembered" means per setup and whether it is saved in the project was left to the lane. Requirements: UI-03, UI-15 (per-user layout), PRJ-01 (the project holds the job, not its view state). Records read: 0129, 0119 (the viewport's controls), 0169 (what is saved in the project).

## Decision

1. **Both panes zoom and pan.** The wheel over a pane zooms it about the pointer, so the point under the pointer stays still. A drag that starts in a pane moves its drawing. A double-click in a pane, or its "Fit" button (shown at its top right while it is zoomed or moved, with the tooltip "Fit the front view to its pane again (double-click does the same). Scroll to zoom, drag to move."), returns it to the fitted view. A click without a drag still selects a held region or tab (7.5). The view model is unchanged: the zoom and pan are applied to each pane's fitted map in the interface (`PaneView`).
2. **Remembered per setup for the session, not saved (the lane's proposal).** Each pane keeps its zoom and pan for each setup while the program runs, across leaving and coming back to the Setup step; it is not written to the project, so no schema number is needed, and a project opens with its panes fitted. Saving it would put view state in the project file, which the project format keeps for the job (decision 0169 saved the path eyes only because they choose what is shown in every step). A per-user setting does not fit either, because the view belongs to one setup's drawing.
3. **Limits.** The zoom runs from the fitted view (1x, `MIN_ZOOM`; zooming out further only adds empty space) to 20 times it (`MAX_ZOOM`), about 0.1 mm a pixel on a 100 mm stock in the 270-pixel end pane. The pan keeps the pane's centre on the drawing (at most half the zoomed pane's size each way), so the drawing never leaves the pane.

## Alternatives considered

- **Saved in the project (a schema number):** view state in the job's files, and a change to the project for something that changes no output. Not chosen; the owner can ask for it later.
- **Remembered per pane only, shared by every setup:** a second setup (SET-07) draws a different stock; its fitted view is the better start.
- **The 3D viewport's camera controls:** the panes are 2D orthographic drawings with their own labels and dimensions; a wheel zoom and a drag pan are what a 2D drawing needs.

## Consequences

- `openchuck-ui`: `setup_panes::PaneView`, `Pane`, `MIN_ZOOM`, `MAX_ZOOM`, `pane_view`; `show` takes the key of the setup it draws; the panes sense drags.
- No change to the view model, the project or any output.

## Tests

In `apps/openchuck-ui/src/setup_panes.rs`: `req_ui_03_a_pane_zooms_about_the_pointer_within_limits`, `req_ui_03_a_pane_pans_within_limits`, `req_ui_03_each_pane_keeps_its_view_per_setup_and_a_double_click_fits` (the wheel over the front pane zooms it only; another setup's panes stay fitted; a double-click fits it again).
