# openchuck-ui

The OpenChuckCAM desktop application, built on egui with the wgpu backend (architecture section 5.8). It talks only to `openchuck-app` and `openchuck-viewport`, and contains no CAM logic.

**Status:** work breakdown slice 8 is in progress (decision record 0026). It has:

- the window and menus: new, open and save, with unsaved-change prompts;
- the 3D viewport: part, stock, rotary axis, A+ arrow and work coordinate system;
- the project tree, with operations that can be enabled, disabled, moved and duplicated;
- parameter and tool forms, and the custom-profile editor;
- the import dialog, and the model and machine panels.

## Running

    cargo run --release -p openchuck-ui -- [PROJECT_FOLDER]

## Frame-time benchmark (NFR-03, verification plan 4.2)

    cargo run --release -p openchuck-benches --example bench_model -- bench1m.stl
    cargo run --release -p openchuck-ui -- --bench bench1m.stl [--seconds 60] [--mode edges|shaded|wireframe] [--out report.txt]

The benchmark plays a scripted 60-second camera path with vsync off and prints the frame-time percentiles. It exits with 0 on a pass and 2 when p95 is over 33 ms.

Licence: AGPL-3.0-or-later.
