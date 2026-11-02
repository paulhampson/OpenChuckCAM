# openchuck-gcode

G-code reader and interpreter: parses G-code with A words back into motion for replay and checks.

Part of [OpenChuckCAM](https://github.com/paulhampson/OpenChuckCAM). Architecture section 5.3 and 9; requirements GC-08, SIM-09.

It has the in-house reader for the codes the OpenChuckCAM posts emit (`read`, decision record 0005), the modal replay (`replay`), the static checks (`static_check`), the comparison of a replayed program with the toolpath IR (`compare`) and the comparison with a simulator's report (`compare_simulator`).

`compare_simulator_range` checks a bounded LinuxCNC fixture against inclusive lines of the original unsplit program. It independently replays the original prefix to verify both actual interpreter states (`SimReport::boundaries`) before comparing the original moves. Ordinary whole-program reports default to an empty boundary list. The bounded API supports the absolute metric G0/G1 first-cut subset with zero coordinate offsets (GC-10, OI-991).

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.
