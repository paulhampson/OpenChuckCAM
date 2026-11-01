# Baluster: the model of the tutorial

`baluster.stl` is the part that [tutorial](../../../user-docs/src/tutorial/index.md) cuts: a small turned and fluted piece 61 mm long and 22 mm across at its widest, lying along X (the rotary axis). From X = 0: a plain 14 mm end that the chuck holds, a flat shoulder up to a collar, a bead, a cove, a vase with six flutes, a neck and a ball finial that ends in a point. It has 5,376 triangles (269 KB) and is a closed mesh that `openchuck-cli inspect` reports no problems for.

- `generate.py` writes it: `python3 testdata/examples/baluster/generate.py testdata/examples/baluster/baluster.stl`. The script is deterministic, so the same bytes come out every time.
- `baluster-project/` is the finished tutorial project (**File > Open project**), made by `user-docs/tools/capture.py` while it took the tutorial's screenshots. It holds its own copy of the model in `models/`, so it does not depend on `baluster.stl` beside it. The G-code and the setup sheet it writes are not stored here.

Written for this repository by lane DOCSUSER, under AGPL-3.0-or-later like the rest of it. It is an example for learning the program, not a part that has been cut on a machine.
