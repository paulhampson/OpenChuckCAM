# openchuck-cli

The headless OpenChuckCAM front end over `openchuck-app` (GC-11, NFR-08). Wiring only.

```
openchuck-cli new <PROJECT> [--name <NAME>]
openchuck-cli validate <PROJECT>
openchuck-cli resave <PROJECT>
openchuck-cli regenerate <PROJECT> [--out <DIR>]
openchuck-cli report <PROJECT>
```

A project is a folder of TOML files (decision records 0006 and 0007). `validate` also checks every model file against its recorded hash and warns about long order keys; `resave` loads and saves, writing only files whose text changes.

Licence: AGPL-3.0-or-later.
