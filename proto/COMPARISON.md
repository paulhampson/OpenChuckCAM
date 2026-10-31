# mdBook or Zola for the user guide: prototype comparison (lane DOCSITE)

Throwaway: this branch (`claude/lane-docsite-proto`) is never merged. It answers the owner's request for a small prototype of each generator before decision D1 (decision 0400, proposed).

Both prototypes have the same three pages (Home, tutorial step 5 "Hold the work", Glossary), the same text and one stylesheet (`shared/style.css`, the palette, type and components of the design spec). Build both with `./build.sh` (`MDBOOK=` and `ZOLA=` name the binaries).

## What was built

| | mdBook 0.5.4 | Zola 0.23.6 |
| --- | --- | --- |
| Built and screenshotted here | Yes: light and dark, 1366 and 390 px wide, and the print view (a 10-page A4 PDF) | **No.** Zola is not on crates.io, GitHub release downloads are blocked in this container, and building it from a clone was not permitted. The sources are written but have not been run, so they may need small fixes. |
| How it was installed | `cargo install mdbook --version 0.5.4 --locked` from crates.io (MPL-2.0) | Would be the release binary, or `cargo build` of the tagged source (MIT) |
| Build time for these pages | 0.08 s | not measured (Zola is also fast: well under a second for a site this size, by its own figures) |
| Output | 1.4 MB, of which 0.8 MB is the four Plex font files and 0.2 MB is mdBook's unused default scripts and styles | not built |

## How things are written

| | mdBook | Zola |
| --- | --- | --- |
| Navigation order | One file, `src/SUMMARY.md`: a nested list with part headings ("Start here", "Tutorial"). Pages not yet written are listed as drafts (`- [Title]()`) and shown as plain text. Reviewers see the whole structure in one diff. | No contents file. Each page has a `weight` in its front matter; part headings come from `[extra] part = "..."`. The contents list is built by our template (`macros/nav.html`, 31 lines). Pages not written cannot be shown. |
| Callouts | GitHub's alert syntax: `> [!NOTE]` and `> [!WARNING]`. Also renders on GitHub. | Shortcodes: `{% note() %} ... {% end %}`. Shows as literal text on GitHub. |
| Figures with captions | A few lines of HTML in the Markdown (`<figure>`, `<img>`, `<figcaption>`). Renders on GitHub too. Image width and height are written by hand. | A shortcode: `{{ figure(src=..., number=5, alt=..., caption=...) }}`. Shorter, and it reads the image size itself. Does not render on GitHub. |
| Glossary links | Ordinary links: `[held region](../glossary.md#held-region)`. mdBook rewrites `.md` to `.html`. They also work on GitHub. Not checked by mdBook; lychee checks them in CI. | `[held region](@/glossary.md#held-region)`. Zola checks the page and the anchor at build time, which is a real advantage. Broken on GitHub. |
| Glossary entries | Headings with fixed ids: `## Held region {#held-region}` (same in both) | Same |
| Reading the source on GitHub | Reads correctly: links, alerts and figures work | Shortcodes and `@/` links show as raw text |

## How much of the look is ours

| | mdBook | Zola |
| --- | --- | --- |
| Stylesheet (shared) | 229 lines | the same file |
| Page template | `theme/index.hbs`, 101 lines (replaces mdBook's 367-line default) | 8 templates, 125 lines in all |
| Extra code we keep | `fixnav.py`, 38 lines: mdBook writes the contents list with links relative to the site root and marks no current page (its own theme fixes this in JavaScript), so this post-build step makes it work without JavaScript | `static-search.js`, 18 lines: Zola builds the search index but gives no search box |
| Default files we cannot switch off | mdBook still writes its own `book.js`, highlight and theme files into the output, unused (about 0.2 MB). Harmless; can be blanked with empty files in `theme/`. | none |

## Search

- **mdBook:** built in. Our page uses mdBook's own search script (searcher.js), with its 72 KB index, so results, highlighting and keyboard use come with it. That script expects mdBook's toggle button, so the template adds a hidden one and presses it on first focus (6 lines). Works offline.
- **Zola:** builds the same kind of index (elasticlunr). The search box and results list are ours (18 lines here; a full version with highlighting would be about 80). The elasticlunr script itself has to be supplied; here it is copied from mdBook's output.

## Print

- **mdBook:** `print.html` holds the whole guide in one page in contents order. With our print styles: black on white, no navigation, each chapter starting on a new page, figures and callouts never split, link URLs printed after external links. This is how the tutorial and safety pages can be printed as one document.
- **Zola:** each page prints well with the same print styles. There is no whole-guide page. One could be added as a template that walks every section and page (about 30 lines), with internal links to fix up as for mdBook's.

## Awkward points

**mdBook**
- Its own theme is easy to recognise. We replace it completely, so nothing of it shows, but a major version can change the template data (0.5 already moved the contents list into JavaScript). Upgrades must be deliberate.
- The contents list and the search script assume mdBook's own page layout. Both are adapted here in about 45 lines.
- No layout per page type: the home page uses the same layout as every other page. Fine for a manual, and the design spec asks for no hero section anyway.

**Zola**
- The contents order is spread over the front matter of every page.
- Shortcodes and `@/` links make the Markdown unreadable on GitHub, and lock the content to Zola.
- More template code to write and keep (all of the navigation, the search box, a print page).
- A real advantage: internal links and anchors are checked at build time without another tool.

## Recommendation (unchanged)

mdBook. It gives the manual its order in one reviewable file, a whole-guide print page and working search with the least of our own code, and its Markdown stays readable on GitHub. Zola's built-in link checking is good, but lychee in CI covers the same ground.
