# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests of the post-build steps of user-docs/tools/build.py (decision 0400 sections S and V, NFR-19, NFR-12).

Run with `python3 -m unittest discover -s user-docs/tools/tests`.
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import build  # noqa: E402

NAV = ('<nav class="sidebar" aria-label="Contents"><ol class="chapter"><li class="chapter-item">'
       '<span class="chapter-link-wrapper"><a href="index.html">Home</a></span></li>'
       '<li class="chapter-item"><span class="chapter-link-wrapper"><a href="tutorial/index.html">Tutorial</a></span></li>'
       '<li><a href="https://example.org/x.html">elsewhere</a></li></ol></nav>')


class Navigation(unittest.TestCase):
    """NFR-19: the contents work without JavaScript, and the current page is marked."""

    def test_req_nfr_19_links_are_made_relative_to_the_page(self):
        html = build.fix_nav(f"<body>{NAV}<a href=\"index.html\">x</a></body>", "tutorial/05-hold.html")
        self.assertIn('href="../index.html"', html)
        self.assertIn('href="../tutorial/index.html"', html)
        self.assertIn('<a href="index.html">x</a>', html, "links outside the contents are not touched")
        self.assertIn('href="https://example.org/x.html"', html)

    def test_req_nfr_19_the_current_page_is_marked(self):
        html = build.fix_nav(NAV, "tutorial/index.html")
        self.assertIn('<a href="../tutorial/index.html" aria-current="page">', html)
        self.assertEqual(html.count("aria-current"), 1)

    def test_req_nfr_19_a_page_at_the_root_keeps_root_links(self):
        html = build.fix_nav(NAV, "index.html")
        self.assertIn('href="index.html" aria-current="page"', html)
        self.assertIn('href="tutorial/index.html"', html)


TEMPLATE = ('<meta name="docs-version" content="@@DOCS_VERSION@@">'
            '<a class="version">@@VERSION_LABEL@@</a>'
            '<!--latest-banner--><div class="banner">development <span data-newest-release></span></div><!--/latest-banner-->'
            '<footer class="site-footer"><p class="notice">@@GCODE_NOTICE@@</p><p class="stamp">@@STAMP@@</p></footer>')
NOTICE = "Generated G-code must be verified by the user before it is run on a machine."


class Stamp(unittest.TestCase):
    """Section V: every page names its version; NFR-12: the footer carries the program's own notice."""

    def test_req_nfr_19_development_build(self):
        html = build.fill(TEMPLATE, version=None, tag=None, commit="abc1234", date="2026-11-02", notice=NOTICE)
        self.assertIn('content="latest"', html)
        self.assertIn("Version: latest", html)
        self.assertIn("Describes the development version at commit abc1234 of 2026-11-02 (a preview, not a published build).", html)
        self.assertIn('<span data-newest-release></span>', html, "the assembler fills in the newest release")
        self.assertNotIn("@@", html)

    def test_req_nfr_19_nightly_build(self):
        html = build.fill(TEMPLATE, version=None, tag=None, commit="abc1234", date="2026-11-02", notice=NOTICE,
                          nightly="2026-11-03")
        self.assertIn('content="latest"', html)
        self.assertIn("Describes the nightly build of 2026-11-03, commit abc1234.", html)
        self.assertNotIn("@@", html)

    def test_req_nfr_19_a_release_is_not_a_nightly_build(self):
        with self.assertRaises(ValueError):
            build.fill(TEMPLATE, version="0.31.0", tag="app-v0.31.0", commit="abc1234", date="2026-11-02",
                       notice=NOTICE, nightly="2026-11-03")

    def test_req_nfr_19_release_build(self):
        html = build.fill(TEMPLATE, version="0.31.0", tag="app-v0.31.0", commit="abc1234", date="2026-11-02", notice=NOTICE)
        self.assertIn('content="0.31.0"', html)
        self.assertIn("Version: 0.31.0", html)
        self.assertIn("Describes OpenChuckCAM 0.31.0 (app-v0.31.0, commit abc1234), built 2026-11-02.", html)
        self.assertNotIn("development", html, "a release has no development-version banner")
        self.assertNotIn("@@", html)

    def test_req_nfr_12_the_notice_is_the_programs(self):
        html = build.fill(TEMPLATE, version=None, tag=None, commit="abc1234", date="2026-11-02", notice=NOTICE)
        self.assertIn(f'<p class="notice">{NOTICE}</p>', html)

    def test_req_nfr_19_a_release_needs_its_tag(self):
        with self.assertRaises(ValueError):
            build.fill(TEMPLATE, version="0.31.0", tag=None, commit="abc1234", date="2026-11-02", notice=NOTICE)
        with self.assertRaises(ValueError):
            build.fill(TEMPLATE, version="0.31.0", tag="app-v0.32.0", commit="abc1234", date="2026-11-02", notice=NOTICE)


if __name__ == "__main__":
    unittest.main()
