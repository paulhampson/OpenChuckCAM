# openchuck-types

Plain data shared by the OpenChuckCAM libraries (architecture section 5.1): unit newtypes, geometry, mesh and B-rep data, stock, tools, the machine profile, the one shared forward-kinematics implementation, the neutral toolpath IR (GC-01), the job context and typed errors.

Everything is serde-serialisable, double precision, in millimetres and degrees (NFR-05). The crate depends on no other OpenChuckCAM crate and names no controller, axis letter or G-code dialect.

This crate belongs to the reusable library family (`openchuck-types`, `openchuck-path`, `openchuck-post`, `openchuck-gcode`). It depends only on third-party crates, never on the application, UI, GPU, Git or the C++ kernel (architecture sections 4 and 12).

```
cargo run -p openchuck-types --example kinematics
```

Licence: AGPL-3.0-or-later.
