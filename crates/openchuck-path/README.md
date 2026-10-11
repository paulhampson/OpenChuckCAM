# openchuck-path

Reusable 4-axis pathing library: strategies, tool-axis control, A-axis management, linking, gouge protection and feed intent.

Part of [OpenChuckCAM](../../README.md). Architecture section 5.2; requirements OP-01 to OP-17, INT-01 to INT-04.

**Status:** shell. Strategies, the `SurfaceSource`, `StockModel` and `CollisionOracle` ports and the shared services arrive from slice 5.

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.
