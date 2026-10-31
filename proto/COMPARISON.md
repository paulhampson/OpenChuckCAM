# mdBook or Zola for the user guide: prototype comparison (lane DOCSITE)

Throwaway: this branch (`claude/lane-docsite-proto`) is never merged. It answers the owner's request for a small prototype of each generator before decision D1 (decision 0400, proposed).

Both prototypes have the same three pages (Home, tutorial step 5 "Hold the work", Glossary), the same text and one stylesheet (`shared/style.css`, the palette, type and components of the design spec). Build both with `./build.sh` (`MDBOOK=` and `ZOLA=` name the binaries). Screenshots of both are in `screenshots/`.

## What was built

| | mdBook 0.5.4 | Zola 0.23.6 |
| --- | --- | --- |
| Built and screenshotted | Yes: light and dark, 1366 and 390 px wide, and the print view (a 10-page A4 PDF) | Yes: light and dark, 1366 and 390 px wide (no whole-guide print page exists) |
| How it was installed here | `cargo install mdbook --version 0.5.4 --locked` from crates.io (MPL-2.0) | Not on crates.io. Built from the tag v0.23.6 (commit 1969d846) with `cargo build --release --locked`, 10.5 minutes, with the owner's approval. CI would download the release binary instead (MIT). |
| Build time for these pages | 0.07 s | 0.04 s |
| Output | 1.4 MB: 0.8 MB of Plex fonts, 0.2 MB of mdBook default files we do not use, a 72 KB search index | 1.1 MB: the same fonts, a 40 KB search index |
| Search | Works (tested: "chuck" finds 5 places) | Works (tested: "chuck" finds 4 pages) |

## An important finding: Zola 0.23 was a breaking release

Zola 0.23.0 (5 August 2026) moved to a new major version of its template language (Tera 2) and **removed shortcodes**; its changelog calls it "probably the most breaking version of Zola that will happen", and existing themes need rewriting. The first Zola sources here were written in the old style and did not build; they were rewritten to the new one (components, GitHub alerts). This matters for NFR-14: a generator three months past a rewrite of its template language is a higher maintenance risk than one whose main change was a year ago. mdBook 0.5 also changed its theme template (the contents list moved into JavaScript), which is why our mdBook template is our own.

## How things are written

| | mdBook | Zola 0.23 |
| --- | --- | --- |
| Navigation order | One file, `src/SUMMARY.md`: a nested list with part headings ("Start here", "Tutorial"). Pages not yet written are listed as drafts (`- [Title]()`) and shown as plain text. Reviewers see the whole structure in one diff. | No contents file. Each page and section has a `weight` in its front matter; part headings come from `[extra] part = "..."`. Our template builds the list (about 25 lines in `base.html`), and it cannot interleave sections and loose pages by weight without more template code. Pages not yet written cannot be shown. |
| Callouts | GitHub's alert syntax, `> [!NOTE]` and `> [!WARNING]`. mdBook writes a "Note" or "Warning" title into the HTML. | The same syntax (`github_alerts = true`). Zola writes no title, so "Note" and "Warning" come from CSS (`::before`); most screen readers read it, but it is not in the HTML. |
| Figures with captions | A few lines of HTML in the Markdown (`<figure>`, `<img>`, `<figcaption>`). Renders on GitHub too. Image width and height are written by hand. | A component: `{{ <figure src="..." number={5} width={1366} height={768} alt="..." caption="..." /> }}`. Shorter, and the caption can hold Markdown links. Shows as raw text on GitHub. |
| Glossary links | Ordinary links: `[held region](../glossary.md#held-region)`, rewritten to `.html`. Work on GitHub. Checked in CI by lychee, not by mdBook. | `[held region](@/glossary.md#held-region)`. Zola checks the page and anchor at build time, a real advantage. Broken on GitHub. |
| Links in the output | Relative (`../glossary.html`), so one build works at any address: `/latest/`, `/v0.31.0/`, or a file on disk. | Absolute (`/glossary/`), from `base_url`. Each version must be built with its own `--base-url`, and the pages cannot be opened from disk. Workable, but one more thing to get right in the release job. |
| Reading the source on GitHub | Reads correctly: links, alerts and figures work | Alerts work; components and `@/` links show as raw text |

## How much of the look is ours

| | mdBook | Zola |
| --- | --- | --- |
| Stylesheet (shared) | 289 lines, of which 46 are the home page hero | the same file |
| Page templates | `theme/index.hbs`, 101 lines (replaces mdBook's 367-line default) | 6 templates, 161 lines in all (115 before the landing page) |
| Extra code we keep | `fixnav.py`, 38 lines: mdBook writes the contents list with links relative to the site root and marks no current page (its own theme fixes this in JavaScript), so this post-build step makes it work without JavaScript. Search reuses mdBook's own script, adapted in 6 lines. | `static-search.js`, 18 lines: Zola builds the search index but gives no search box (a full one, with highlighting, would be about 80 lines). The elasticlunr script must be supplied; here it is copied from mdBook's output. |
| Default files we cannot switch off | mdBook still writes its own `book.js`, highlight and theme files, unused (about 0.2 MB). Harmless. | Zola writes `robots.txt` and `sitemap.xml` (wanted anyway). |

## The home page hero (added at the owner's request)

The owner asked for a proposed hero section on the home page in each tool, with the same design intent. Both are built and in `screenshots/` (`*-home-*`).

**Design.** A two-column band at desktop width, stacked on a phone:
- left: a small line naming the guide's version, with a link to the other versions (the version switch); one plain headline; one paragraph saying what the program is and who the guide is for; two routes, *Start here* and *Your first project*, as a ruled, numbered list of links (not buttons); the G-code notice (NFR-12), set quietly with an ochre rule and the label "Before you cut";
- right: a real screenshot of the application, the finishing toolpath of the tutorial's baluster (lane DOCSUSER's `hero-1-toolpaths.png`), with a caption saying it is from a development build.

No gradient, no buttons, no icons, no feature cards, no marketing words. The same CSS serves both tools (46 lines added to the shared stylesheet).

| | mdBook | Zola |
| --- | --- | --- |
| How it is done | The hero is a block of HTML at the top of `src/index.md` (16 lines). mdBook does not parse Markdown inside it, so the hero's words are HTML in the Markdown file. Template change: 3 lines in `index.hbs` (a `home` class on the body when `is_index`, and no development banner on the home page, since the hero names the version). | A dedicated landing template, `templates/index.html` (47 lines). The hero's words live in the front matter of `content/_index.md` (`[extra]`: headline, lead, routes, image, alt text, caption), so the template holds layout only. |
| Page around it | The usual page: the contents sidebar stays on the left, so the hero shares the width with it. | A page of its own: no sidebar, the hero spans the page, and a "Contents of this guide" overview built from the sections replaces the sidebar. |
| What was hacked | Nothing that fights mdBook: `is_index` is a variable mdBook gives every template. The cost is that the hero is HTML inside a Markdown file, and the home page cannot drop the sidebar without more template branching. | Nothing: this is what Zola's section templates are for. |
| On GitHub | The HTML block renders on GitHub as plain text and a picture: readable. | The front matter shows as a table of values; the hero itself is only in the built site. |
| Survives a major upgrade? | The HTML block in the Markdown survives anything. The 3 template lines depend on `is_index` and on our own `index.hbs`; if mdBook renames the variable, the hero still shows, only the wider column and the hidden banner are lost until the line is fixed. | The landing template uses Zola's template language, which changed completely in 0.23 (shortcodes removed, Tera 2). Another such change would mean rewriting it. |
| Extra upkeep | Low: one HTML block and three template lines. | Moderate: a 47-line template, plus the contents overview's logic. |

**Honest summary.** Zola gives the better home page: a true landing layout without the sidebar, with the hero's words kept as data. mdBook gets close with very little code, at the cost of the sidebar beside the hero and the hero being HTML inside the Markdown. Neither hero is a reason on its own to choose the tool; the maintenance points above still favour mdBook. If the owner prefers the full-width landing, mdBook can get it too: a separate static `index.html` built from a small template outside mdBook, about 60 lines, at the cost of a second build step.

## Print

- **mdBook:** `print.html` holds the whole guide in one page in contents order. With our print styles: black on white, no navigation, each chapter on a new page, figures and callouts never split, link URLs printed after external links. This is how the tutorial and the safety pages can be printed as one document (`screenshots/mdbook-print.pdf`).
- **Zola:** each page prints well with the same print styles. There is no whole-guide page; one could be added as a template that walks every section and page (about 30 lines).

## Awkward points

**mdBook**
- Its own theme is easy to recognise. We replace it completely, so nothing of it shows, but its template data can change between versions (0.5 moved the contents list into JavaScript). Upgrades must be deliberate.
- The contents list and the search script assume mdBook's own layout; adapted here in about 45 lines.
- One layout for every page, the home page included. Fine for a manual; the design spec asks for no hero section.

**Zola**
- Just past a breaking rewrite of its template language (above).
- The contents order is spread over the front matter of every page.
- Components and `@/` links make the Markdown unreadable on GitHub, and lock the content to Zola.
- Absolute links: each version needs its own base URL, and pages cannot be checked from disk.
- More of our own code to keep: navigation, the search box, a print page.
- Real advantages: internal links and anchors checked at build time, slightly faster and smaller output, and a figure component.

## Recommendation (unchanged)

mdBook. It gives the manual its order in one reviewable file, a whole-guide print page and working search with the least of our own code, its output works at any address, and its Markdown stays readable on GitHub. Zola is a capable second choice, but it has just broken its template language, and its link checking is covered by lychee in CI.
