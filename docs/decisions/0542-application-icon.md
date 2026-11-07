# 0542: The application icon (lane ICON)

- **Status:** proposed by lane ICON on 7 Nov 2026. It records the owner's choices of 7 Nov 2026, relayed by the coordinator: the design (concept A, @@CHOICE@@), embedding the Windows program icon with no new dependency (option 1), and rendering the PNGs with `resvg` in `xtask` only (option 1). Only the owner accepts this record (decision 0049).
- **What it amends, if accepted:** nothing in the requirements or the architecture. It adds files to `openchuck-ui` (its assets, a build script and one module), a command to `xtask`, a dependency to `xtask` only, and the user guide's favicon. No library API change and no project schema change.
- **Date:** 7 Nov 2026.
- **Open items:** closes OI-2106 (OpenChuckCAM has no icon of its own on any platform, raised with [decision 0541](0541-macos-app-bundle-linux-glibc-floor-and-release-tags.md)) when merged. Raises OI-2109 (the native check on Windows, an owner item).

## Context

Until now the program had no icon of its own:

- the window and taskbar showed eframe's default icon, the egui logo, which the owner will not ship;
- the Windows `.exe` embedded no icon, so Explorer showed the generic one;
- the macOS bundle (lane PACKAGING, decision 0541) had none of its own (OI-2106).

No requirement names an icon; the work is release work for milestone 2026.1-alpha, asked for by the owner through OI-2106.

## Decision (proposed)

1. **The design.** The owner chose @@CHOICE@@ from three concepts and one refinement round (sheets on the throwaway branch `claude/icon-concepts`, never merged: `icon-concepts/contact-sheet.png` and `icon-concepts/refine/contact-sheet-refine.png`, with the scores against the logo tests in `icon-concepts/refine/NOTES.md`). It shows a chuck seen end-on, three jaws gripping round stock, inside an orange arrow for the A-axis rotation, on a slate tile. It is drawn on a 512-unit canvas with a 32-unit grid, so its main shapes land on whole pixels at 16 px. It does not resemble the egui logo (a white hexagon with a four-part cross); four jaws were not offered because they would.
2. **Licence.** The icon is original work, drawn for the project with no third-party artwork or fonts, and licensed under the project's licence, AGPL-3.0-or-later (NFR-10). The SVG carries the SPDX identifier; `apps/openchuck-ui/assets/icon/README.md` gives the licence of the binary files.
3. **Files.** `apps/openchuck-ui/assets/icon/` holds `icon.svg` (the master), `icon-512.png`, `icon-256.png`, `icon-128.png`, `icon-64.png`, `icon-48.png`, `icon-32.png`, `icon-16.png`, `icon.ico` (256, 64, 48, 32 and 16), `icon.res` and the README. The folder is in the UI crate, beside its fonts, because the window icon is embedded with `include_bytes!` and architecture section 12 has no top-level assets folder (agreed with lane PACKAGING through the coordinator).
4. **Generation.** `cargo xtask icon` writes every generated file from `icon.svg`; `cargo xtask icon --check` writes nothing and lists the files that differ: each PNG may differ from a fresh rendering by at most 2 in 255 in any channel, because the renderer's anti-aliasing can differ by a step between x86-64 and ARM processors (the workspace tests run on both); the favicon SVG, `icon.ico` and `icon.res` must match byte for byte, the last two against the committed PNGs they are made of. The rendering uses `resvg` 0.49.0 (Apache-2.0 OR MIT), a dependency of `xtask` only, never of a product crate (owner's choice, option 1; NFR-14 row in `docs/dependencies.md`). The generated files are committed, so building the application needs no renderer and no network.
5. **Window and taskbar icon (Windows and Linux).** `openchuck-ui` embeds `icon-256.png` and passes it to `ViewportBuilder::with_icon` for the application and the benchmark windows (`src/app_icon.rs`). It decodes with `eframe::icon_data::from_png_bytes`, which uses the `image` crate eframe already brings, so no dependency is added. If the PNG ever failed to decode, the window would keep the default icon rather than fail to open (NFR-07).
6. **Windows program icon.** `cargo xtask icon` also writes `icon.res`, a compiled Windows resource (one `RT_GROUP_ICON` of the five PNG-compressed `RT_ICON` images), and `apps/openchuck-ui/build.rs` passes it to the linker when the target is `windows-msvc`. The MSVC linker accepts a `.res` file as an input, so no resource compiler and no new crate are needed (owner's choice, option 1, over the `winresource` and `embed-resource` crates). On a `windows-gnu` target the build script warns that the program gets no embedded icon; CI and the release archives use `x86_64-pc-windows-msvc`.
7. **macOS bundle.** Lane PACKAGING's `cargo xtask macos-app` reads `apps/openchuck-ui/assets/icon/icon-512.png` when it exists (decision 0541); this lane does not edit PACKAGING's files.
8. **User guide favicon.** `cargo xtask icon` copies the SVG to `user-docs/theme/favicon.svg` and writes a 32-pixel `favicon.png` beside it; the guide's page template links both.

## Verification

Done in the lane's container (Linux, x86_64) on branch `claude/lane-icon`; commit @@COMMIT@@.

- `cargo test -p xtask icon`: the renderer gives the asked size, scales the drawing and is deterministic; invalid input is an error, not a panic (NFR-07); the ICO directory has one entry per PNG with the right size bytes, length and offset; the RES file holds the empty first resource, one `RT_ICON` per PNG and one group whose entries match; `generate` makes every committed size; `icon_same_image_allows_an_anti_aliasing_step_but_not_a_changed_drawing` checks the tolerance; and `icon_committed_files_match_the_svg` checks that every committed file is what the SVG gives.
- `cargo test -p openchuck-ui icon`: `icon_window_icon_is_the_embedded_256_pixel_image`, `icon_window_viewport_carries_the_icon` and `icon_native_options_carry_the_application_icon` (the window the application and the benchmark open carries the 256 by 256 icon).
- `icon.res` was linked with `rust-lld -flavor link /dll /noentry /machine:x64` into a test DLL, and `pefile` read back five `RT_ICON` resources of the PNG sizes and one `RT_GROUP_ICON` (language 1033), so the linker accepts the file. ImageMagick's `identify` reads `icon.ico` as five PNG images of 256, 64, 48, 32 and 16 pixels.
- Not verified here: the icon on a real Windows title bar, taskbar and Explorer, which cannot be seen headless. That is OI-2109, an owner item. The Windows CI build links the resource on every pull request.

## Open items

- OI-2109: the owner checks on Windows that the title bar, the taskbar and Explorer show the icon, at 100% and 150% display scale.
