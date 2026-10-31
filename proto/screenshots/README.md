# Prototype screenshots (lane DOCSITE, throwaway)

Taken with Chromium (Playwright) from the mdBook prototype built by `../build.sh`, from local files, with no network request. Named `<generator>-<page>-<width>-<theme>.png`: pages `home`, `step` (tutorial step 5) and `glossary`; widths 1366 (1366 x 768 window, whole page) and 390 (a phone, at 2x pixel density, whole page); themes `light` and `dark` (the system setting, which the site follows by default).

- `mdbook-print.pdf`: the whole-guide print page (`print.html`) printed to A4 by Chromium, 10 pages.
- `mdbook-print-view.png`: the same page with the print styles applied, as one image.

Zola: not built. The clone of getzola/zola v0.23.6 succeeded, but this session's permission check refused the build (see `../COMPARISON.md`).
