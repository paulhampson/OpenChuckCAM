# Post definitions

Built-in post definitions as data files, inside the `openchuck-post` crate so the published crate carries them (architecture section 12, decision 0103, GC-03): the Makera Z1 post (GC-09) and the LinuxCNC verification post (GC-10), each with golden tests. They were built in work breakdown slice 6 ([decision 0020](../../../docs/decisions/0020-slice-6-post-and-round-trip.md)) after spikes SP-10 (template engine) and SP-12 (Z1 conventions).

A post is a folder: `post.toml` (schema, name, template file, feed rule, every option) and one template. The two shipped posts share a template (a test keeps them identical) and differ by options and feed rule. To make your own, copy a folder, edit it and point `openchuck-cli regenerate --post` at it; no rebuild is needed. A new post should come with golden G-code files that the reader and replay checks cover. The Z1 conventions in `makera-z1/post.toml` are derived and unconfirmed until the air cut (OI-37); generated G-code is yours to verify before running (NFR-12).

Machine conventions (A-axis letter, direction, wrap, feed mode) live here and in machine profiles, never in the engine (GC-04).

`combine_rotary_runs` (decision 0506, accepted) writes a run of consecutive fed moves that turn A only, at the same X, Y and Z, the same way round and with the same feed, as one block, so the controller plans one rotation instead of one per step. Both built-in posts set it to `true`; a post that leaves the key out writes every step. Set it to `false` in a copied post to compare the two on a machine.
