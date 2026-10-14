# Post definitions

Built-in post definitions as data files (architecture section 12, GC-03): the Makera Z1 post (GC-09) and the LinuxCNC verification post (GC-10), each with golden tests. They were built in work breakdown slice 6 ([decision 0020](../docs/decisions/0020-slice-6-post-and-round-trip.md)) after spikes SP-10 (template engine) and SP-12 (Z1 conventions).

A post is a folder: `post.toml` (schema, name, template file, feed rule, every option) and one template. The two shipped posts share a template (a test keeps them identical) and differ by options and feed rule. To make your own, copy a folder, edit it and point `openchuck-cli regenerate --post` at it; no rebuild is needed. A new post should come with golden G-code files that the reader and replay checks cover. The Z1 conventions in `makera-z1/post.toml` are derived and unconfirmed until the air cut (OI-37); generated G-code is yours to verify before running (NFR-12).

Machine conventions (A-axis letter, direction, wrap, feed mode) live here and in machine profiles, never in the engine (GC-04).
