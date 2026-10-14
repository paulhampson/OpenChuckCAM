# openchuck-post

Reusable post-processing library: neutral toolpath IR to G-code through user-editable post definitions.

Part of [OpenChuckCAM](../../README.md). Architecture section 5.3; requirements GC-02 to GC-10, GC-13, INT-03.

**Status:** slice 6 under way. Built: the neutral machine program stage (`to_machine_program`) and the four named feed rules (`FeedRule`: `linuxcnc`, `makera`, `inverse_time`, `contact`; decision 0019 item 5). To come: the template engine, post definitions and options, and the `PostProcessor` trait.

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on other members of that family and on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

Licence: AGPL-3.0-or-later.
