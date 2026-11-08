# Blisk: the starting project of the worked examples

`blisk-project/` is the project the user guide's worked examples on selection sets, face-flow finishing and swarf start from (`user-docs/src/examples/`). Open a copy of it with **File > Open project**, so the example in the repository stays as it is.

- **The part** is the project owner's bladed disk, `testdata/step/valid/blisk.step` (25 faces, 69 edges; see `testdata/README.md` for its source and licence, OI-832 and decision record 0209). The project holds its own copy in `models/`, centred on the rotary axis and moved 10 mm along X, so the part runs from X = 10 to X = 60: the disc at the chuck end from X = 10 to X = 20, then five twisted blades on a coned hub to X = 60.
- **The machine** is the built-in Makera Z1 profile.
- **The setup** has a cylinder of oak stock 42 mm across and 52 mm long, from X = 9 to X = 61.
- **The tools** are a 3 mm flat end mill (tool 1) and a 3 mm ball end mill (tool 2), each with 16 mm of flute (enough to reach the hub through the uncut stock) and cutting data for oak.
- It has no selection sets and no operations: each example makes its own.

`generate.py` writes it: `python3 testdata/examples/blisk-start/generate.py`. The ids are fixed, so the same bytes come out every time. The G-code the examples write is not stored here.

Written for this repository by lane EXAMPLES-B, under AGPL-3.0-or-later like the rest of it. It is an example for learning the program, not a part that has been cut on a machine.
