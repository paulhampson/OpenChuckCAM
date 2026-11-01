# SPDX-License-Identifier: AGPL-3.0-or-later
"""Style, privacy and safety checks of the user documentation (decision 0400, NFR-19, NFR-12).

    python3 user-docs/tools/check.py --site DIR [--release VERSION] [--screens strict|warn]

DIR is a built site (one version, or the assembled whole). The Markdown source in
user-docs/src/ and the documentation workflows are read from the repository this file is in.
Links, anchors and HTML validity are checked by lychee and the Nu HTML Checker, not here.
Exit status 1 when any error is found; warnings are printed and do not fail.

Python standard library only, so CI needs no package.
"""
import argparse
import html.parser
import os
import re
import subprocess
import sys
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, "user-docs", "src")
IMAGES = os.path.join(SRC, "tutorial", "images")
# Where the documentation tools are pinned and used: an UNVERIFIED pin in any of them fails the check.
WORKFLOWS = [os.path.join(REPO, ".github", "workflows", n) for n in ("docs-site.yml", "release.yml")]
WORKFLOWS.append(os.path.join(REPO, ".github", "actions", "user-docs", "action.yml"))

# The folders whose changes can change what the tutorial screenshots show (decision 0400 section G).
SCREEN_PATHS = ["apps/openchuck-ui", "crates/openchuck-viewport", "crates/openchuck-app/src"]

STUB_SENTENCE = "This page is not written yet."
EM_DASH_LIMIT = 2

# Errors in the Markdown source (user-docs/STYLE.md). Whole words, any case.
PLACEHOLDERS = ["lorem ipsum", "TODO", "TBD", "FIXME", "XXX"]
MARKETING = ["seamless", "seamlessly", "powerful", "robust", "effortless", "effortlessly", "leverage",
             "unlock", "elevate", "supercharge", "cutting-edge", "game-changer", "best-in-class",
             "revolutionary", "blazing"]
PHRASES = ["let's", "let us dive", "dive in", "dive into"]
# NFR-12: nothing may say or suggest that generated output is safe or checked.
UNSAFE = ["safe to run", "ready to run", "guaranteed correct", "guaranteed safe", "verified safe",
          "safe to use on your machine", "no need to check"]


def _words(term):
    return re.compile(r"(?<![\w-])" + re.escape(term) + r"(?![\w-])", re.IGNORECASE)


def _prose(text):
    """The text with code removed: fenced blocks, then inline code."""
    text = re.sub(r"^(```|~~~).*?^\1\s*$", "", text, flags=re.S | re.M)
    return re.sub(r"`[^`\n]*`", "", text)


def text_problems(name, text):
    """Errors and warnings for one Markdown source file."""
    errors, warnings = [], []
    prose = _prose(text)
    for term in PLACEHOLDERS:
        pattern = _words(term) if term != "lorem ipsum" else re.compile("lorem ipsum", re.I)
        if term in ("TODO", "TBD", "FIXME", "XXX"):
            pattern = re.compile(r"(?<![\w-])" + term + r"(?![\w-])")
        if pattern.search(prose):
            errors.append(f"{name}: placeholder text '{term}'; a page that is not written says '{STUB_SENTENCE}'")
    for term in MARKETING:
        if _words(term).search(prose):
            errors.append(f"{name}: marketing word '{term}' (user-docs/STYLE.md)")
    for term in PHRASES:
        if _words(term).search(prose):
            errors.append(f"{name}: '{term}' (user-docs/STYLE.md)")
    for term in UNSAFE:
        if _words(term).search(prose):
            errors.append(f"{name}: '{term}' suggests generated output is safe or checked (NFR-12)")
    dashes = prose.count("—")
    if dashes > EM_DASH_LIMIT:
        warnings.append(f"{name}: {dashes} em dashes (more than {EM_DASH_LIMIT}); prefer a full stop, comma or colon")
    return errors, warnings


def is_stub(text):
    return STUB_SENTENCE in text


class _Collector(html.parser.HTMLParser):
    """Collects the elements the checks need, with the figure each image is in."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.requests = []
        self.images = []  # (src, alt or None, figure index or None)
        self.figures = []  # True when the figure has a figcaption
        self.stack = []
        self.footer_depth = 0
        self.footer_text = []
        self.styles = []
        self._in_style = False

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag == "figure":
            self.figures.append(False)
            self.stack.append(len(self.figures) - 1)
        elif tag == "figcaption" and self.stack:
            self.figures[self.stack[-1]] = True
        elif tag == "footer":
            self.footer_depth += 1
        elif tag == "style":
            self._in_style = True
        if tag == "img":
            self.images.append((a.get("src"), a.get("alt"), self.stack[-1] if self.stack else None))
        for key in ("src", "data", "poster"):
            if a.get(key):
                self.requests.append(a[key])
        if a.get("srcset"):
            self.requests.extend(part.strip().split()[0] for part in a["srcset"].split(",") if part.strip())
        if tag == "link" and a.get("href"):
            rel = (a.get("rel") or "").lower().split()
            if set(rel) & {"stylesheet", "icon", "preload", "prefetch", "modulepreload", "manifest", "apple-touch-icon"}:
                self.requests.append(a["href"])
        if a.get("style"):
            self.styles.append(a["style"])

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag == "figure":
            self.handle_endtag(tag)

    def handle_endtag(self, tag):
        if tag == "figure" and self.stack:
            self.stack.pop()
        elif tag == "footer" and self.footer_depth:
            self.footer_depth -= 1
        elif tag == "style":
            self._in_style = False

    def handle_data(self, data):
        if self.footer_depth:
            self.footer_text.append(data)
        if self._in_style:
            self.styles.append(data)


def _parse(text):
    c = _Collector()
    c.feed(text)
    c.close()
    return c


def _remote(url):
    return bool(re.match(r"^(?:[a-z][a-z0-9+.-]*:)?//", url.strip(), re.I)) and not url.strip().lower().startswith("file:")


def css_requests(css):
    """URLs another site would be asked for by a stylesheet (url() and @import)."""
    found = re.findall(r"""url\(\s*['"]?([^'")\s]+)['"]?\s*\)""", css)
    found += re.findall(r"""@import\s+['"]([^'"]+)['"]""", css)
    return [u for u in found if _remote(u)]


def third_party_requests(text):
    """URLs in an HTML page that a browser would fetch from another site (NFR-19)."""
    c = _parse(text)
    found = [u for u in c.requests if _remote(u)]
    for css in c.styles:
        found += css_requests(css)
    return found


def image_problems(text):
    c = _parse(text)
    problems = []
    for src, alt, figure in c.images:
        if alt is None or not alt.strip():
            problems.append(f"image {src} has no alt text")
        if figure is None or not c.figures[figure]:
            problems.append(f"image {src} is not in a figure with a caption")
    return problems


def read_notice(repo=REPO):
    """The program's own G-code notice (crates/openchuck-types/src/notice.rs), so the site says the same."""
    with open(os.path.join(repo, "crates", "openchuck-types", "src", "notice.rs"), encoding="utf-8") as f:
        rust = f.read()
    m = re.search(r'VERIFY_GCODE_NOTICE: &str = "(.*?)";', rust, re.S)
    if not m:
        raise SystemExit("could not read VERIFY_GCODE_NOTICE from notice.rs")
    return re.sub(r"\\\n\s*", "", m.group(1))


def has_notice(text, notice):
    footer = " ".join("".join(_parse(text).footer_text).split())
    return notice in footer


def unverified_pins(paths):
    problems = []
    for p in paths:
        if not os.path.exists(p):
            continue
        with open(p, encoding="utf-8") as f:
            for n, line in enumerate(f, 1):
                if "UNVERIFIED" in line and not line.lstrip().startswith("#"):
                    problems.append(f"{os.path.relpath(p, REPO)}:{n}: a tool pin is marked UNVERIFIED")
    return problems


def _meta_version(text):
    m = re.search(r'<meta name="docs-version" content="([^"]*)"', text)
    return m.group(1) if m else None


def release_problems(site, version):
    """Section R: the copy for a release names its version, and the release notes have a section for it."""
    problems = []
    for root, _, files in os.walk(site):
        for name in files:
            if not name.endswith(".html"):
                continue
            path = os.path.join(root, name)
            with open(path, encoding="utf-8") as f:
                text = f.read()
            if "docs-version" not in text:
                continue
            rel = os.path.relpath(path, site)
            if _meta_version(text) != version:
                problems.append(f"{rel}: page version is {_meta_version(text)}, not {version}")
            if f"app-v{version}" not in text:
                problems.append(f"{rel}: the footer stamp does not name app-v{version}")
    notes = os.path.join(site, "release-notes.html")
    if not os.path.exists(notes):
        problems.append("release-notes.html is missing")
    else:
        with open(notes, encoding="utf-8") as f:
            text = f.read()
        heads = [re.sub(r"<[^>]+>", "", h).strip() for h in re.findall(r"<h2[^>]*>(.*?)</h2>", text, re.S)]
        if not any(re.match(re.escape(version) + r"(\s|$|\b)", h) for h in heads):
            problems.append(f"the release notes have no section headed {version}")
    return problems


def _git(repo, *args):
    return subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True, check=True).stdout.strip()


def stale_screens(repo, images):
    """None when the tutorial screenshots are current, otherwise the reason (section G)."""
    if not os.path.isdir(images):
        return None
    pictures = [n for n in os.listdir(images) if n.lower().endswith((".png", ".jpg", ".jpeg", ".webp"))]
    record = os.path.join(images, "CAPTURED.toml")
    if not os.path.exists(record):
        return f"{len(pictures)} tutorial images have no CAPTURED.toml" if pictures else None
    with open(record, "rb") as f:
        data = tomllib.load(f)
    marks = [data[k] for k in ("captured_at", "reviewed_at") if data.get(k)]
    if not marks:
        return "CAPTURED.toml names no captured_at or reviewed_at commit"
    latest_change = _git(repo, "log", "-1", "--format=%H", "--", *SCREEN_PATHS)
    if not latest_change:
        return None
    for mark in marks:
        try:
            if subprocess.run(["git", "merge-base", "--is-ancestor", latest_change, mark], cwd=repo).returncode == 0:
                return None
        except OSError:
            break
    changed = _git(repo, "diff", "--name-only", f"{marks[-1]}..HEAD", "--", *SCREEN_PATHS).splitlines()
    where = sorted({"/".join(p.split("/")[:2]) for p in changed}) or SCREEN_PATHS
    return f"the screens changed after the tutorial screenshots were taken or reviewed ({', '.join(where)})"


def _html_files(site):
    for root, _, files in os.walk(site):
        for name in sorted(files):
            if name.endswith(".html"):
                yield os.path.join(root, name)


def _source_files():
    for root, _, files in os.walk(SRC):
        for name in sorted(files):
            if name.endswith(".md"):
                yield os.path.join(root, name)


def _is_content_page(text):
    """A page built from the guide's template (not a redirect or the versions list)."""
    return 'name="docs-version"' in text


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--site", required=True, help="a built site")
    ap.add_argument("--release", metavar="VERSION", help="also run the release checks for this version")
    ap.add_argument("--screens", choices=["strict", "warn"], default="warn",
                    help="stale tutorial screenshots fail (strict, the release) or warn (pull requests)")
    args = ap.parse_args(argv)
    errors, warnings = [], []

    for path in _source_files():
        rel = os.path.relpath(path, REPO)
        with open(path, encoding="utf-8") as f:
            e, w = text_problems(rel, f.read())
        errors += e
        warnings += w

    notice = read_notice()
    for path in _html_files(args.site):
        rel = os.path.relpath(path, args.site)
        with open(path, encoding="utf-8") as f:
            text = f.read()
        errors += [f"{rel}: requests {u} from another site (NFR-19)" for u in third_party_requests(text)]
        if _is_content_page(text):
            errors += [f"{rel}: {p}" for p in image_problems(text)]
            if not has_notice(text, notice) and not rel.endswith("print.html") and not rel.endswith("toc.html"):
                errors.append(f"{rel}: the footer does not carry the G-code notice (NFR-12)")
    for root, _, files in os.walk(args.site):
        for name in files:
            if name.endswith(".css"):
                with open(os.path.join(root, name), encoding="utf-8") as f:
                    errors += [f"{os.path.relpath(os.path.join(root, name), args.site)}: requests {u} from another site (NFR-19)"
                               for u in css_requests(f.read())]

    errors += unverified_pins(WORKFLOWS)
    if args.release:
        errors += release_problems(args.site, args.release)
    stale = stale_screens(REPO, IMAGES)
    if stale:
        (errors if args.screens == "strict" else warnings).append(f"tutorial screenshots: {stale}")

    for w in warnings:
        print(f"warning: {w}")
    for e in errors:
        print(f"error: {e}")
    print(f"{len(errors)} errors, {len(warnings)} warnings")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
