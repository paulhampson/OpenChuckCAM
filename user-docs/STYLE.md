# Style guide for the user guide

How to write and change the pages in `user-docs/src/` ([decision 0400](../docs/decisions/0400-user-documentation-site.md)). The guide is for people who want to make parts with OpenChuckCAM, including people who have never used CAM software. Write so that a newcomer can follow every step and an experienced machinist is never talked down to.

`user-docs/tools/check.py` enforces the rules marked *(checked)*. The review checklist at the end covers the rest.

## Wording

- Use plain words ([decision 0114](../docs/decisions/0114-slice-22-conformance-report.md) and the project's plain-words rule). Prefer the short, common word: *use*, not *utilise*; *about*, not *approximately*.
- Write to the reader as *you*, in the present tense. Give steps as instructions: "Press **Apply**", not "The user should press Apply".
- Define each CAM term the first time a page uses it, in a few words, and link it to the [glossary](src/glossary.md): `[stepover](../glossary.md#stepover)`. Add the term to the glossary if it is not there. Do not explain a term twice on one page.
- Write UI labels exactly as the program shows them, in bold: **Held at the start**, **File > Save**. Write keys as `<kbd>Ctrl</kbd>+<kbd>S</kbd>`. Write file names, values the reader types, and G-code in `code`.
- Give numbers with their units: "8 mm", "0.01 mm", "10 %".
- Say what something does and when it matters. Leave out how impressive it is.

Avoid *(checked)*:

- marketing words: seamless, powerful, robust, effortless, leverage, unlock, elevate, supercharge, cutting-edge, game-changer, best-in-class, revolutionary, blazing;
- "Let's", "dive in";
- placeholder text: lorem ipsum, TODO, TBD, FIXME, XXX;
- more than two em dashes on a page (a warning): use a full stop, a comma or a colon.

Also avoid: asides that comment on how easy something is ("simply", "just", "of course"), a bold lead-in on every bullet, and a closing paragraph that repeats the page.

## Safety (NFR-12)

- Never say or suggest that generated G-code, or anything the program checks, is safe, correct or ready for the machine *(checked: phrases such as "safe to run", "ready to run", "guaranteed correct")*.
- Every page carries the program's own notice in its footer, read from `crates/openchuck-types/src/notice.rs` at build time *(checked)*. A page about generating, posting or running a program also says it in the text, where the reader needs it.
- Use a **Warning** callout only for something that can damage the machine, the work or a person. Use a **Note** for anything else worth setting apart. There are no other callouts:

  ```markdown
  > [!WARNING]
  > A held region is a length along the axis, not a 3D body. ...
  ```

- The pages adapted from `docs/z1-air-cut-procedure.md` and `docs/z1-safe-practice-checklist.md` name their source and say it is the authority. When the source changes, change the page in the same pull request.

## Structure

- One topic per page, with one `#` heading. Use `##` for sections and `###` sparingly.
- A procedure is a numbered list of steps, one action each. Put what the reader sees after a step in the paragraph that follows the list, or in a figure.
- Use a table when the reader compares items along the same properties. Use a definition list (`Term` on one line, `: description` on the next) for a list of terms with descriptions.
- Add a page to `src/SUMMARY.md`; the order there is the order of the guide.
- A page that is not written yet says so in its first sentence, exactly: "This page is not written yet." Then say what it will cover and link to what exists. Never fill a page with placeholder text.

## Figures

- Every image is in a figure with a caption *(checked)* and has alt text that says what the image shows, for a reader who cannot see it *(checked)*:

  ```html
  <figure>
    <img src="images/06-workholding.png" width="1366" height="768"
         alt="The Workholding and tabs section of the Inspector with Held at the start ticked ...">
    <figcaption><b>Figure 5.</b> An 8 mm held region at the start of the stock, drawn hatched in the view.</figcaption>
  </figure>
  ```

- Number figures within a page. Say in the caption what to look at.
- Screenshots show the real program, taken with `user-docs/tools/capture.py` at 1366 x 768 (decision 0400 section G), and only the repository's example models or generated ones. Never show a private project. The script records the commit in `src/tutorial/images/CAPTURED.toml`; see `user-docs/README.md`, Taking the screenshots.
- A diagram is inline SVG drawn with `currentColor`, so it works in the light and the dark theme, with a `<title>` and `<desc>`.

## Versions

The guide published as *latest* is the nightly build: it is built from the commit of the `nightly` pre-release and its footer says "Describes the nightly build of <date>, commit <commit>". Write each page for the program as it is on `main`, and change it in the same pull request as the behaviour it describes; it reaches readers with the next nightly build. A release's copy is fixed when the release is made.

## Design

The look is set by `theme/style.css` and `theme/index.hbs`; pages do not set colours or fonts.

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--bg` | `#fbfaf7` | `#151719` | Page background |
| `--text` | `#1b1d1f` (16.2:1) | `#e4e2dd` (13.9:1) | Body text |
| `--muted` | `#545860` (6.8:1) | `#a2a6ab` (7.3:1) | Captions, labels |
| `--accent` | `#365569` (7.6:1) | `#9cb6c8` (8.5:1) | Links, focus ring, Note |
| `--warn` | `#7a4e0e` (6.5:1 on `--warn-bg`) | `#d8a75e` (7.6:1 on `--warn-bg`) | Warning |
| `--rule` | `#d6d2ca` | `#34383c` | Lines (decorative only) |

Contrast ratios are against the background named (WCAG 2.x); every text colour meets AA, most meet AAA. Text is IBM Plex Sans at 17 px with a line height of 1.65 and a measure of about 68 characters; code is IBM Plex Mono. No gradients, shadows, rounded cards, emoji or icon fonts. Colour never carries meaning alone: callouts have a text label.

The guide reads without JavaScript; scripts only add search, the theme switch and the version list. It makes no request to any other site and sets no cookies *(checked)*.

## Review checklist

Before asking for review of a change to the guide:

1. The guide builds, and `check.py`, the link check and the HTML check pass (see `user-docs/README.md`).
2. Every step was followed in the current program, and every UI label matches it.
3. Every CAM term is defined on first use and linked to the glossary.
4. No sentence suggests that generated output is safe or checked; safety material defers to its `docs/` source.
5. Every image has alt text, a numbered caption, and shows only repository or generated models.
6. Screenshots were re-taken, or `CAPTURED.toml` was marked reviewed, if the screens changed.
7. A new page is in `SUMMARY.md`; a stub says "This page is not written yet."
8. Procedures are numbered lists with one action per step.
9. No marketing words, no "simply" or "just", no restating summary.
10. The page reads at phone width and in the dark theme.
11. Links to `docs/` and to the repository use full GitHub addresses; links between pages use relative `.md` paths.
12. A change to the behaviour of the program changes its page in the same pull request.
