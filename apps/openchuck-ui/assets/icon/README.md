<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# The OpenChuckCAM application icon

The project's own icon ([decision 0542](../../../../docs/decisions/0542-application-icon.md)): a chuck seen end-on, three jaws gripping round stock, inside an orange arrow for the A-axis rotation.

## Files

| File | What it is | Used by |
| --- | --- | --- |
| `icon.svg` | The master drawing, hand-written SVG on a 512 by 512 canvas | Everything below is made from it |
| `icon-512.png`, `icon-256.png`, `icon-128.png`, `icon-64.png`, `icon-48.png`, `icon-32.png`, `icon-16.png` | The rendered sizes | `icon-256.png`: the window and taskbar icon (`src/app_icon.rs`, embedded at compile time); `icon-512.png`: the macOS bundle (`cargo xtask macos-app`, decision 0541) |
| `icon.ico` | Windows icon file, PNG entries at 256, 64, 48, 32 and 16 | For anyone who needs a `.ico` (installers, shortcuts) |
| `icon.res` | Compiled Windows resource with the same images (one icon group) | `build.rs` links it into `openchuck-ui.exe` on the MSVC target, so Explorer shows the icon |

The user guide's favicon (`user-docs/theme/favicon.svg` and `favicon.png`) is made from the same SVG.

## Making the files

Edit `icon.svg` only, then run from the workspace root:

```sh
cargo xtask icon          # writes the PNGs, icon.ico, icon.res and the user guide's favicon
cargo xtask icon --check  # writes nothing; lists any committed file that differs from the SVG
```

The rendering uses the `resvg` crate in `xtask` only, so it needs no browser, system tool or network, and it gives the same image every time. The check allows a PNG to differ from a fresh rendering by at most 2 in 255 in any channel, because anti-aliasing can differ by a step between x86-64 and ARM processors; `icon.ico` and `icon.res` must match the committed PNGs byte for byte. The generated files are committed: building the application never renders the icon. The xtask test `icon::tests::icon_committed_files_match_the_svg` fails if a committed file does not match the SVG.

## Licence

Original work for OpenChuckCAM, drawn for the project with no third-party artwork or fonts. Licensed under the project's licence, AGPL-3.0-or-later (NFR-10). `icon.svg` carries the SPDX identifier in a comment; the PNG, ICO and RES files cannot hold a comment, and this README gives their licence.
