# Crankshaft: the simultaneous finishing example

`crankshaft-project/` is the starting project of the user guide's worked example [Simultaneous finishing](../../../user-docs/src/examples/simultaneous-finishing.md). Open it with **File > Open project**, after copying it to a folder of your own.

The part is the public model [`testdata/step/valid/crankshaft.step`](../../step/valid/crankshaft.step): a small crankshaft 40 mm long whose three pins sit 10 mm off the rotary axis, so that over most of its length the part's section does not surround the axis. The project holds its own copy of the file in `models/`, byte for byte the same.

What the project holds:

- the model, centred on the rotary axis (one placement step);
- the built-in Makera Z1 machine profile;
- one setup: a 38 mm hardwood cylinder from X = -4 to 42, held for 6 mm at the start;
- two tools from the Makera Z1 library: `Z1 O-flute 3.175x25` (T104) and `Z1 ball 3.175x22` (T204).

It holds no operations: the worked example adds them in the program, as the reader does. No G-code or setup sheet is stored.

`generate.py` writes the project with `openchuck-cli` alone. It needs the release command line and the STEP reader program (see the script's header):

```text
python3 testdata/examples/crankshaft-simultaneous/generate.py
```

The setup and model ids are new on each run, so a new project has the same content but not the same bytes.

The model is AGPL-3.0-or-later, as part of this repository, contributed by the project owner (issue #411; see `testdata/README.md`). The project was written for this repository by lane EXAMPLES-C, under the same licence. It is an example for learning the program, not a part that has been cut on a machine.
