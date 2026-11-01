# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests of the documentation checks (decision 0400, NFR-19, NFR-12).

Run with `python3 -m unittest discover -s user-docs/tools/tests`.
Each test is named after the requirement or decision section it covers.
"""
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import check  # noqa: E402

NOTICE = (
    "Generated G-code must be verified by the user before it is run on a machine. "
    "OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup."
)


def page(body="", footer=NOTICE, head="", version="latest", stamp="Describes the development version"):
    return textwrap.dedent(f"""\
        <!DOCTYPE html><html lang="en"><head><meta charset="utf-8">
        <meta name="docs-version" content="{version}">{head}<title>t</title></head>
        <body><main>{body}</main>
        <footer class="site-footer"><p class="notice">{footer}</p><p class="stamp">{stamp}</p></footer></body></html>
        """)


def write(root, rel, text):
    path = os.path.join(root, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    return path


class ThirdPartyRequests(unittest.TestCase):
    """NFR-19 (and NFR-09): the site makes no request to any other site."""

    def test_req_nfr_19_local_resources_pass(self):
        html = page('<img src="images/a.png" alt="x">', head='<link rel="stylesheet" href="theme/style.css"><script src="a.js"></script>')
        self.assertEqual(check.third_party_requests(html), [])

    def test_req_nfr_19_remote_script_stylesheet_image_font_and_frame_are_found(self):
        html = page(
            '<img src="https://cdn.example/a.png" alt="x"><iframe src="//evil.example/f"></iframe>'
            '<img srcset="b.png 1x, https://cdn.example/b2.png 2x" alt="y">',
            head='<link rel="stylesheet" href="https://fonts.example/css">'
                 '<script src="http://cdn.example/x.js"></script>'
                 '<style>@font-face { src: url("https://fonts.example/f.woff2"); }</style>',
        )
        found = check.third_party_requests(html)
        for url in ["https://cdn.example/a.png", "//evil.example/f", "https://cdn.example/b2.png",
                    "https://fonts.example/css", "http://cdn.example/x.js", "https://fonts.example/f.woff2"]:
            self.assertIn(url, found)

    def test_req_nfr_19_a_link_to_another_site_is_not_a_request(self):
        html = page('<a href="https://github.com/paulhampson/OpenChuckCAM">repository</a>',
                    head='<link rel="canonical" href="https://example.org/x">')
        self.assertEqual(check.third_party_requests(html), [])

    def test_req_nfr_19_stylesheet_url_is_found(self):
        self.assertEqual(check.css_requests('a { background: url(https://x.example/i.png) } b { background: url(local.png) }'),
                         ["https://x.example/i.png"])


class FiguresAndAltText(unittest.TestCase):
    """Decision 0400 section S: every image has alt text and sits in a figure with a caption."""

    def test_req_nfr_19_image_in_captioned_figure_passes(self):
        html = page('<figure><img src="a.png" alt="The part"><figcaption>Figure 1. The part.</figcaption></figure>')
        self.assertEqual(check.image_problems(html), [])

    def test_req_nfr_19_missing_or_empty_alt_fails(self):
        html = page('<figure><img src="a.png"><figcaption>c</figcaption></figure>'
                    '<figure><img src="b.png" alt=" "><figcaption>c</figcaption></figure>')
        problems = check.image_problems(html)
        self.assertEqual(len(problems), 2)
        self.assertTrue(all("alt" in p for p in problems))

    def test_req_nfr_19_image_outside_a_captioned_figure_fails(self):
        html = page('<p><img src="a.png" alt="x"></p><figure><img src="b.png" alt="y"></figure>')
        problems = check.image_problems(html)
        self.assertEqual(len(problems), 2)
        self.assertTrue(all("caption" in p for p in problems))


class Notice(unittest.TestCase):
    """NFR-12: every page states that generated G-code must be verified before it is run."""

    def test_req_nfr_12_the_notice_is_read_from_the_program(self):
        repo = tempfile.mkdtemp()
        write(repo, "crates/openchuck-types/src/notice.rs",
              'pub const VERIFY_GCODE_NOTICE: &str = "Generated G-code must be verified by the user before it is run on a machine. \\\n'
              'OpenChuckCAM does not guarantee that a program is safe or correct for any machine, tool or setup.";\n')
        self.assertEqual(check.read_notice(repo), NOTICE)

    def test_req_nfr_12_page_with_the_notice_passes(self):
        self.assertTrue(check.has_notice(page(), NOTICE))

    def test_req_nfr_12_page_without_the_notice_fails(self):
        self.assertFalse(check.has_notice(page(footer="Licensed AGPL-3.0-or-later."), NOTICE))

    def test_req_nfr_12_the_notice_must_be_in_the_footer_not_only_the_text(self):
        html = page(body=f"<p>{NOTICE}</p>", footer="")
        self.assertFalse(check.has_notice(html, NOTICE))


class SourceText(unittest.TestCase):
    """Decision 0400 section S and user-docs/STYLE.md: wording rules checked in the Markdown source."""

    def test_req_nfr_19_plain_text_passes(self):
        errors, warnings = check.text_problems("a.md", "# Hold the work\n\nTick **Held at the start**.\n")
        self.assertEqual((errors, warnings), ([], []))

    def test_req_nfr_19_placeholder_text_fails(self):
        for text in ["Lorem ipsum dolor", "TODO: write this", "FIXME later", "TBD"]:
            errors, _ = check.text_problems("a.md", text)
            self.assertTrue(errors, text)

    def test_req_nfr_19_marketing_words_fail(self):
        for word in ["seamless", "powerful", "robust", "effortless", "leverage", "unlock", "elevate"]:
            errors, _ = check.text_problems("a.md", f"A {word} way to cut.")
            self.assertTrue(errors, word)

    def test_req_nfr_19_a_word_inside_another_is_not_matched(self):
        errors, _ = check.text_problems("a.md", "Robustness tests and the unlocked door.")
        self.assertEqual(errors, [])

    def test_req_nfr_12_wording_that_calls_output_safe_fails(self):
        for text in ["The program is safe to run.", "The G-code is ready to run.", "Output is guaranteed correct."]:
            errors, _ = check.text_problems("a.md", text)
            self.assertTrue(errors, text)

    def test_req_nfr_19_lets_dive_in_fails(self):
        errors, _ = check.text_problems("a.md", "Let's dive in.")
        self.assertTrue(errors)

    def test_req_nfr_19_more_than_two_em_dashes_warn(self):
        errors, warnings = check.text_problems("a.md", "a — b — c — d")
        self.assertEqual(errors, [])
        self.assertTrue(warnings)

    def test_req_nfr_19_code_is_not_checked_for_words(self):
        errors, _ = check.text_problems("a.md", "Use `--robust` here.\n\n```\nTODO powerful\n```\n")
        self.assertEqual(errors, [])

    def test_req_nfr_19_stub_page_must_say_it_is_not_written(self):
        errors, _ = check.text_problems("a.md", "# Tabs\n\nThis page is not written yet. It will cover tabs.\n")
        self.assertEqual(errors, [])
        self.assertTrue(check.is_stub("# Tabs\n\nThis page is not written yet.\n"))
        self.assertFalse(check.is_stub("# Tabs\n\nA tab is a bridge.\n"))


class Pins(unittest.TestCase):
    """Decision 0400 section P: a tool pin that could not be verified fails the check."""

    def test_req_nfr_19_unverified_pin_fails(self):
        d = tempfile.mkdtemp()
        p = write(d, "w.yml", "env:\n  VNU_SHA256: UNVERIFIED\n")
        self.assertEqual(len(check.unverified_pins([p])), 1)

    def test_req_nfr_19_the_shared_action_holding_the_pins_is_scanned(self):
        self.assertIn(os.path.join(check.REPO, ".github", "actions", "user-docs", "action.yml"), check.WORKFLOWS)

    def test_req_nfr_19_verified_pins_pass(self):
        d = tempfile.mkdtemp()
        p = write(d, "w.yml", "env:\n  VNU_SHA256: 3925f07364f5e5a111cef54a7baefa575b22d05c70f8ced048f0616d4579b7ab\n")
        self.assertEqual(check.unverified_pins([p]), [])


class ReleaseChecks(unittest.TestCase):
    """Decision 0400 section R: the checks the release workflow adds for an app-v tag."""

    def site(self, version, notes):
        d = tempfile.mkdtemp()
        write(d, "index.html", page(version=version, stamp=f"Describes OpenChuckCAM {version} (app-v{version}, commit abc1234)"))
        write(d, "release-notes.html", page(body=notes, version=version,
                                            stamp=f"Describes OpenChuckCAM {version} (app-v{version}, commit abc1234)"))
        return d

    def test_req_nfr_19_release_with_notes_and_stamp_passes(self):
        d = self.site("0.31.0", '<h2 id="0310">0.31.0</h2><p>Changes.</p>')
        self.assertEqual(check.release_problems(d, "0.31.0"), [])

    def test_req_nfr_19_release_notes_without_a_section_for_the_version_fail(self):
        d = self.site("0.31.0", '<h2 id="0300">0.30.0</h2>')
        problems = check.release_problems(d, "0.31.0")
        self.assertTrue(any("release notes" in p for p in problems))

    def test_req_nfr_19_stamp_that_does_not_match_the_tag_fails(self):
        d = self.site("0.31.0", '<h2>0.31.0</h2>')
        problems = check.release_problems(d, "0.32.0")
        self.assertTrue(any("stamp" in p or "version" in p for p in problems))


class Screens(unittest.TestCase):
    """Decision 0400 sections G and R: tutorial screenshots older than the last change to the screens."""

    def repo(self):
        d = tempfile.mkdtemp()
        env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@e", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@e")

        def git(*args):
            return subprocess.run(["git", *args], cwd=d, env=env, check=True, capture_output=True, text=True).stdout.strip()

        git("init", "-q")
        write(d, "apps/openchuck-ui/src/main.rs", "fn main() {}\n")
        git("add", ".")
        git("commit", "-q", "-m", "one")
        first = git("rev-parse", "HEAD")
        write(d, "README.md", "unrelated\n")
        git("add", ".")
        git("commit", "-q", "-m", "two")
        return d, git, first

    def test_req_nfr_19_screens_captured_after_the_last_ui_change_pass(self):
        d, git, first = self.repo()
        images = write(d, "user-docs/src/tutorial/images/CAPTURED.toml", f'captured_at = "{first}"\n')
        self.assertEqual(check.stale_screens(d, os.path.dirname(images)), None)

    def test_req_nfr_19_screens_older_than_a_ui_change_are_stale(self):
        d, git, first = self.repo()
        images = write(d, "user-docs/src/tutorial/images/CAPTURED.toml", f'captured_at = "{first}"\n')
        write(d, "crates/openchuck-viewport/src/lib.rs", "// changed\n")
        git("add", ".")
        git("commit", "-q", "-m", "ui change")
        self.assertIn("crates/openchuck-viewport", check.stale_screens(d, os.path.dirname(images)))

    def test_req_nfr_19_a_later_review_makes_them_current(self):
        d, git, first = self.repo()
        write(d, "crates/openchuck-app/src/lib.rs", "// changed\n")
        git("add", ".")
        git("commit", "-q", "-m", "ui change")
        reviewed = git("rev-parse", "HEAD")
        images = write(d, "user-docs/src/tutorial/images/CAPTURED.toml",
                       f'captured_at = "{first}"\nreviewed_at = "{reviewed}"\n')
        self.assertEqual(check.stale_screens(d, os.path.dirname(images)), None)

    def test_req_nfr_19_images_without_a_capture_record_are_stale(self):
        d, git, first = self.repo()
        images = os.path.join(d, "user-docs/src/tutorial/images")
        os.makedirs(images)
        open(os.path.join(images, "01.png"), "wb").close()
        self.assertIn("CAPTURED.toml", check.stale_screens(d, images))

    def test_req_nfr_19_no_tutorial_images_is_not_stale(self):
        d, git, first = self.repo()
        self.assertEqual(check.stale_screens(d, os.path.join(d, "user-docs/src/tutorial/images")), None)


if __name__ == "__main__":
    unittest.main()
