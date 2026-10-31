# Prototype screenshots (lane DOCSITE, throwaway)

Taken with Chromium (Playwright) from the prototypes built by `../build.sh`: mdBook from local files, Zola from a local web server (its links are absolute). No request went to any other site. Named `<generator>-<page>-<width>-<theme>.png`: pages `home`, `step` (tutorial step 5) and `glossary`; widths 1366 (1366 x 768 window, whole page) and 390 (a phone, at 2x pixel density, whole page); themes `light` and `dark` (the system setting, which the site follows by default).

- `mdbook-print.pdf`: the whole-guide print page (`print.html`) printed to A4 by Chromium, 10 pages.
- `mdbook-print-view.png`: the same page with the print styles applied, as one image.

Zola: built from v0.23.6 with the owner's approval. It has no whole-guide print page, so there is no Zola print file.
