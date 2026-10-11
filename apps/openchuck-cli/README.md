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
openchuck-cli machines [--library <DIR>]
openchuck-cli machine <PROJECT> [<KEY | NAME | FILE>] [--library <DIR>] [--clear]
openchuck-cli setup <PROJECT> [<SETUP> | --new <NAME>] [--name <NAME>] [--part <MODEL | none>]
    [--stock <cylinder=D,L,X0 | box=X0,Y0,Z0,X1,Y1,Z1 | bbox=OFFSET | none>] [--material <NAME | none>]
    [--part-offset <MM>] [--work-offset <G54..G59>] [--work-x <MM>] [--clearance <RADIUS,SAFE_Z | machine>]
```

A project is a folder of TOML files (decision records 0006 and 0008). `validate` also checks every model file against its recorded hash and warns about long order keys; `resave` loads and saves, writing only files whose text changes. Models (decision record 0010): `inspect` prints a model file's summary and check report (holes, non-manifold edges, flipped triangles, degenerate triangles) and the STL tessellation statement, or does so for every model in a project. `import` adds a model file to a project, referenced relative to the project folder. Files are read in millimetres unless a unit is set. If the file names another unit (`units=in` in an STL header) or the part's size is implausible in millimetres (under 2 mm or over 2000 mm), `import` asks on a terminal and stores the answer, or otherwise warns and leaves the question open; `inspect` and `validate` show open questions. `units` sets the unit a model is in (`um`, `mm`, `cm`, `m`, `in`), or `default` to unset it. `transform` adds placement steps in order: `translate=X,Y,Z`, `rotate=AXIS:DEGREES`, `scale=FACTOR`, `mirror=AXIS`, `centre` and `orient=DIRECTION` (such as `orient=+z`, which turns that direction onto the rotary axis, X, and centres the part on it). `validate` also reads every model and warns about mesh problems. A model is named by its ID or by its path as the project lists it.

Machine and setups (decision record 0011): `machines` lists the built-in profiles (the Makera Z1: feed limits confirmed, six values unconfirmed until checked on a Z1) and the profile files in a library folder. `machine` copies a profile into the project's `machine.toml` and prints it with the source of each value and the direction of positive A in words; without a profile it shows the current one. `setup` adds or edits a setup and prints it with its checks: the part model, the stock (a cylinder on the rotary axis, a box, or the part's bounding box plus an offset; model-frame millimetres), the material, the part offset along the rotary axis from the machine's rotary reference point, the work offset register and the model X of its zero (always on the rotary axis, so the program's Y0 Z0 is the axis), what the register should hold, and the clearance (a safe cylinder radius and a safe Z above the axis, or the machine's). Setup problems (a safe cylinder that does not enclose stock or part, a part outside its stock, positions outside the machine's travel) are warnings in `setup`, `validate` and `report`, never errors, and name any machine value they used that is not confirmed.

A folder with no `project.toml` gives a warning and an offer to create the project: pass `--create` to accept, answer the prompt on a terminal, or the command stops with an error.

Licence: AGPL-3.0-or-later.
