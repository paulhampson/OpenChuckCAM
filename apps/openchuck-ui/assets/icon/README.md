<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# The OpenChuckCAM application icon

The project's own icon ([decision 0542](../../../../docs/decisions/0542-application-icon.md), the owner's choice of 7 Nov 2026). The mark shows a chuck seen end-on: three jaws around round stock, inside an arrow for the A-axis rotation. It is one colour. Application icons show it white on a near-black rounded tile, so it reads on light and dark taskbars, in Explorer and in the macOS Dock. Without a tile it is ink on light backgrounds and white on dark ones.

## Files

| File | What it is | Used by |
| --- | --- | --- |
| `icon.svg` | Master of the application icon: the white mark on the near-black tile, on a 512 by 512 canvas | Every PNG, `icon.ico`, `icon.res` and the favicon PNG are made from it |
| `mark-ink.svg`, `mark-white.svg` | The mark without a tile, in ink (for light backgrounds) and white (for dark ones) | The repository `README.md` (a `<picture>` by colour scheme); one-colour uses such as documents, print or a future menu-bar icon |
| `favicon-adaptive.svg` | The mark without a tile, ink, or white when the browser uses a dark colour scheme | Copied to `user-docs/favicon/favicon.svg` |
| `icon-512.png`, `icon-256.png`, `icon-128.png`, `icon-64.png`, `icon-48.png`, `icon-32.png`, `icon-16.png` | `icon.svg` rendered at each size | `icon-256.png`: the window and taskbar icon (`src/app_icon.rs`, embedded at compile time). `icon-512.png`: the macOS bundle (`cargo xtask macos-app`, decision 0541) |
| `icon.ico` | Windows icon file, PNG entries at 256, 64, 48, 32 and 16 | For anyone who needs a `.ico` (installers, shortcuts) |
| `icon.res` | Compiled Windows resource holding the same images as one icon group | `build.rs` links it into `openchuck-ui.exe` on the MSVC target, so Explorer and the taskbar show the icon |

The user guide's favicon is `user-docs/favicon/favicon.svg` (a copy of `favicon-adaptive.svg`), with `favicon.png` (`icon.svg` at 32 pixels) for browsers that do not take SVG favicons; the guide's build copies both to the site. The guide's header draws the mark as an inline SVG in `user-docs/theme/index.hbs` (in the text colour, so it follows the theme): if the mark's shapes change, change them there too.

## Colours

| Colour | Hex | Where |
| --- | --- | --- |
| Ink | `#1B1F24` | The tile, and the mark on light backgrounds |
| Tile rim | `#3A4048` | A 12-unit edge on the tile, so it holds on a dark taskbar or Dock |
| White | `#F2F4F7` | The mark on the tile, and on dark backgrounds |

Contrast, as the WCAG ratio (3:1 is the minimum for graphics):

| Mark | On | Ratio |
| --- | --- | --- |
| White | the ink tile | 15.0:1 |
| Ink, no tile | white (`#FFFFFF`) | 16.6:1 |
| Ink, no tile | a light Windows taskbar (`#F3F3F3`) | 14.9:1 |
| White, no tile | a dark Windows taskbar (`#202020`) | 14.8:1 |
| White, no tile | a dark browser tab (`#35363A`) | 11.0:1 |

A tile-less mark in one fixed colour cannot work everywhere: ink on a dark taskbar is 1.0:1, white on a light one is 1.0:1. So application icons, which Windows and Linux draw as they are on a background of the user's theme, always carry the tile. macOS 26 also expects an application icon to fill a rounded tile.

## Making the files

Edit the SVG files only, then run from the workspace root:

```sh
cargo xtask icon          # writes the PNGs, icon.ico, icon.res and the user guide's favicon
cargo xtask icon --check  # writes nothing; lists any committed file that differs
```

The rendering uses the `resvg` crate. It is a dependency of `xtask` only, so it needs no browser, system tool or network, and it gives the same image every time.

The check allows a PNG to differ from a fresh rendering by at most 2 in 255 in any channel, because anti-aliasing can differ by a step between x86-64 and ARM processors. `icon.ico` and `icon.res` must match the committed PNGs byte for byte.

The generated files are committed, so building the application never renders the icon. The xtask test `icon::tests::icon_committed_files_match_the_svg` fails if a committed file does not match its SVG.

## Licence

Original work for OpenChuckCAM, drawn for the project with no third-party artwork or fonts. It is licensed under the project's licence, AGPL-3.0-or-later (NFR-10). Each SVG carries the SPDX identifier in a comment. The PNG, ICO and RES files cannot hold a comment, so this README gives their licence.
