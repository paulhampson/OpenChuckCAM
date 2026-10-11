# openchuck-gcode

G-code reader and interpreter: parses G-code with A words back into motion for replay and checks.

Part of [OpenChuckCAM](../../README.md). Architecture section 5.3 and 9; requirements GC-08, SIM-09.

**Status:** shell. The reader and replay arrive in slice 6, after spike SP-5 (see decision record 0001, open item 5).

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.
