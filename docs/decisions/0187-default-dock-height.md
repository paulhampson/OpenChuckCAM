# 0187: The default dock height fits the playback tab (lane BUG-INSP; issue 215; OI-570)

- **Status:** proposed. The owner chose option A in answer to OI-570 (relayed by the coordinator, 20 Oct 2026); the owner accepts this record in the lane BUG-INSP session. It amends the dock row of the interface specification section 3 once accepted; the specification is not edited before then.
- **Date:** 20 Oct 2026.
- **Open items:** OI-570, closed by this change; OI-571 stays open.

## Context

Trial feedback F-59 (issue 215): the playback box hides its own controls. At 1366 × 768 and 100% the playback tab (transport, speed, timeline, time and Go to move, the readout, the row of Turn the part with A, Tool, Toolpaths, Stock and Colour by deviation, the drawing note) needs about 215 pixels. The default dock height of 196 pixels (specification section 3, `settings::region::DOCK`) leaves about 155 pixels for it, so the last rows were never visible without scrolling.

## Decision

1. **The dock's default height is 260 pixels** (`settings::region::DOCK`; the specification section 3 row becomes "260 high (120 to half the window)"). The smallest (120) and the largest (half the window, which the front end applies) do not change. A size the user has saved still wins, and a size the window clamps is still not saved as the user's choice (OI-516).
2. **The playback tab's lines that come and go keep their space.** The readout line and the drawing note take their line before there is anything to say in them, so no row moves when they fill.

## Consequences

- Test `req_ui_16_req_sim_04_the_whole_playback_tab_shows_in_the_default_dock_and_stays_put`: every row down to "Colour the part by deviation" is above the footer at 1366 × 768 and 1920 × 1080, and no row moves while the simulation plays or steps. The default-size tests expect 260.
- At 1366 × 768 the dock now takes 260 of the 768 pixels by default; the viewport has that much less until the dock is collapsed (Ctrl 3) or resized.
- OI-571 (the box moving down when the simulation runs) did not reproduce in the rig and stays open for the owner's check.
