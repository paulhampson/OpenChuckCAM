# openchuck-cli

The headless OpenChuckCAM front end over `openchuck-app` (GC-11, NFR-08). Wiring only.

```
openchuck-cli new <PROJECT> [--name <NAME>]
openchuck-cli validate <PROJECT>
openchuck-cli resave <PROJECT>
openchuck-cli regenerate <PROJECT> [--out <DIR>]
openchuck-cli report <PROJECT>
openchuck-cli inspect <FILE or PROJECT> [--units <UNIT>]
openchuck-cli import <PROJECT> <FILE> [--units <UNIT>]
openchuck-cli units <PROJECT> <MODEL> <UNIT | default>
openchuck-cli transform <PROJECT> <MODEL> [STEP]... [--clear]
```

A project is a folder of TOML files (decision records 0006 and 0008). `validate` also checks every model file against its recorded hash and warns about long order keys; `resave` loads and saves, writing only files whose text changes. Models (decision record 0010): `inspect` prints a model file's summary and check report (holes, non-manifold edges, flipped triangles, degenerate triangles) and the STL tessellation statement, or does so for every model in a project. `import` adds a model file to a project, referenced relative to the project folder. Files are read in millimetres unless a unit is set. If the file names another unit (`units=in` in an STL header) or the part's size is implausible in millimetres (under 2 mm or over 2000 mm), `import` asks on a terminal and stores the answer, or otherwise warns and leaves the question open; `inspect` and `validate` show open questions. `units` sets the unit a model is in (`um`, `mm`, `cm`, `m`, `in`), or `default` to unset it. `transform` adds placement steps in order: `translate=X,Y,Z`, `rotate=AXIS:DEGREES`, `scale=FACTOR`, `mirror=AXIS`, `centre` and `orient=DIRECTION` (such as `orient=+z`, which turns that direction onto the rotary axis, X, and centres the part on it). `validate` also reads every model and warns about mesh problems. A model is named by its ID or by its path as the project lists it.

A folder with no `project.toml` gives a warning and an offer to create the project: pass `--create` to accept, answer the prompt on a terminal, or the command stops with an error.

Licence: AGPL-3.0-or-later.
