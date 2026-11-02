# openchuck-sim

Material removal simulation, deviation measurement and the implementations of the pathing ports.

Part of [OpenChuckCAM](../../README.md). Architecture section 5.5; requirements SIM-01 to SIM-10, COL-01 to COL-08.

**Status:** headless material removal simulation (work breakdown slice 7, decision 0025): dexel stock in the part frame behind the `StockModel` port, a sweep for every revolved tool through the shared kinematics, checkpoints, deviation against the target, and a display mesh as plain data. Collision detection (COL) and time estimation (SIM-08) come later.



Licence: AGPL-3.0-or-later.
