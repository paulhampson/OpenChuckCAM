# openchuck-gcode

G-code reader and interpreter: parses G-code with A words back into motion for replay and checks.

Part of [OpenChuckCAM](https://github.com/paulhampson/OpenChuckCAM). Architecture section 5.3 and 9; requirements GC-08, SIM-09.

It has the in-house reader for the codes the OpenChuckCAM posts emit (`read`, decision record 0005), the modal replay (`replay`), the static checks (`static_check`), the comparison of a replayed program with the toolpath IR (`compare`) and the comparison with a simulator's report (`compare_simulator`).

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.
