# Credits and licences

OpenChuckCAM is licensed under the GNU Affero General Public License, version 3 or later (AGPL-3.0-or-later). It is built on work that others published, and this page credits it. Each download holds the full texts named below, in the same folder as the programs.

## In each download

| File | What it is |
| --- | --- |
| `LICENSE.txt` | The licence of OpenChuckCAM itself, AGPL-3.0-or-later. |
| `THIRD-PARTY-NOTICES.html` | The licences of the open-source Rust libraries built into the programs. |
| `OPENCASCADE-NOTICE.txt`, `LICENSE_LGPL_21.txt`, `OCCT_LGPL_EXCEPTION.txt` | The notice and source offer for the STEP reader's geometry kernel, and its licence texts. |
| `FONT-NOTICE.txt`, `OFL-1.1.txt` | The notice for the fonts the program shows, and their licence. |
| `TOOL-DATA-CREDITS.txt` | The sources of the ready-made Makera Z1 tool data. |

## The STEP reader

STEP files are read with the OpenCASCADE Technology (OCCT) geometry kernel, licensed under the GNU Lesser General Public License, version 2.1, with the Open CASCADE exception. The kernel is built into one program only, `openchuck-step-worker`, which ships beside the others. `OPENCASCADE-NOTICE.txt` says where its source is and how to rebuild that program against your own copy of OCCT.

## Fonts

The program and this guide use the IBM Plex Sans and IBM Plex Mono fonts, copyright IBM Corp., under the SIL Open Font License, version 1.1. The program carries the font files itself and never loads them from a network.

## Makera Z1 tool data

The ready-made Makera Z1 tools and the Z1 default tool set are written from facts published by Makera and by the Carvera community. [Tool library](reference/tool-library.md#where-the-makera-z1-tool-data-comes-from) names the sources. OpenChuckCAM is an independent project, not made or endorsed by Makera.

## The source code

The source of OpenChuckCAM, and of this guide, is in the project's [GitHub repository](https://github.com/paulhampson/OpenChuckCAM). The commit a download was built from is named in its `README.txt` and in **Help > About**.
