# openchuck-post

Reusable post-processing library: neutral toolpath IR to G-code through user-editable post definitions.

Part of [OpenChuckCAM](https://github.com/paulhampson/OpenChuckCAM). Architecture section 5.3; requirements GC-02 to GC-10, GC-13, INT-03.

It has the neutral machine program stage (`to_machine_program`), the named feed rules (`FeedRule`: `linuxcnc`, `makera`, `inverse_time`, `contact`; decision 0019 item 5), the template engine, post definitions and their options (`PostDefinition`, `PostOptions`), and the two built-in posts (`builtin`): the Makera Z1 post and the LinuxCNC verification post. The built-in posts are data files in this crate's `posts/` folder (decision 0103), so the published crate carries them; a user can copy one, edit it and load it from disk without recompiling.

Generated G-code is yours to verify before you run it on a machine (NFR-12). The Makera Z1 conventions are derived and unconfirmed until they are checked on the machine.

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.
