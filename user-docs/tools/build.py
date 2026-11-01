# SPDX-License-Identifier: AGPL-3.0-or-later
"""Builds the user guide with mdBook and finishes the pages (decision 0400, NFR-19).

    python3 user-docs/tools/build.py --out DIR [--nightly YYYY-MM-DD | --version X.Y.Z --tag app-vX.Y.Z]

Without --version it builds the development version: with --nightly DATE the published "latest"
(the nightly build of that date), without it a preview. MDBOOK names the mdBook binary
(default `mdbook`, pinned in .github/actions/user-docs/action.yml). After mdBook it:
- makes the contents list work without JavaScript (links relative to the page, the current page
  marked), because mdBook's {{#toc}} writes links relative to the site root;
- fills in the version, the footer stamp and the program's own G-code notice (NFR-12);
- adds the stylesheet, the version script and the interface's IBM Plex fonts, copied unmodified
  from apps/openchuck-ui/assets/fonts (OFL-1.1) so the repository holds one copy of them.
The date in the stamp is the commit's date, so the same commit always builds the same pages.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BOOK = os.path.abspath(os.path.join(HERE, ".."))
REPO = os.path.abspath(os.path.join(BOOK, ".."))
FONTS = os.path.join(REPO, "apps", "openchuck-ui", "assets", "fonts")
FONT_FILES = ["IBMPlexSans-Regular.ttf", "IBMPlexSans-SemiBold.ttf", "IBMPlexMono-Regular.ttf", "IBMPlexMono-SemiBold.ttf"]

sys.path.insert(0, HERE)

import check  # noqa: E402

_NAV = re.compile(r'(<nav class="sidebar".*?</nav>)', re.S)
_HREF = re.compile(r'<a href="([^"#:]+\.html)">')
_BANNER = re.compile(r"<!--latest-banner-->.*?<!--/latest-banner-->", re.S)
# mdBook 0.5.4 writes a part title as an li inside another li, which is not valid HTML.
_PART = re.compile(r'<li class="chapter-item[^"]*"><li class="part-title">(.*?)</li></li>', re.S)


def fix_nav(text, rel):
    """Make the contents' links relative to the page at `rel`, mark the current page, and un-nest part titles."""
    prefix = "../" * rel.count("/")

    def nav(m):
        def link(a):
            target = a.group(1)
            current = ' aria-current="page"' if target == rel else ""
            return f'<a href="{prefix}{target}"{current}>'
        return _PART.sub(r'<li class="part-title">\1</li>', _HREF.sub(link, m.group(1)))

    return _NAV.sub(nav, text, count=1)


def fill(text, version, tag, commit, date, notice, nightly=None):
    """Fill the template's placeholders.

    version None: the development version. With `nightly` (the date of the nightly build it was
    built for) it is the published "latest"; without, a preview (a pull request or a local build).
    """
    if version is None:
        if nightly:
            stamp = f"Describes the nightly build of {nightly}, commit {commit}."
        else:
            stamp = f"Describes the development version at commit {commit} of {date} (a preview, not a published build)."
        text = text.replace("<!--latest-banner-->", "").replace("<!--/latest-banner-->", "")
        meta, label = "latest", "latest"
    else:
        if nightly:
            raise ValueError("a release build is not a nightly build")
        if tag != f"app-v{version}":
            raise ValueError(f"a release build needs its tag app-v{version}, not {tag!r}")
        stamp = f"Describes OpenChuckCAM {version} ({tag}, commit {commit}), built {date}."
        text = _BANNER.sub("", text)
        meta, label = version, version
    return (text.replace("@@DOCS_VERSION@@", meta)
                .replace("@@VERSION_LABEL@@", f"Version: {label}")
                .replace("@@STAMP@@", stamp)
                .replace("@@GCODE_NOTICE@@", notice))


def _git(*args):
    return subprocess.run(["git", *args], cwd=REPO, capture_output=True, text=True, check=True).stdout.strip()


def build(out, version=None, tag=None, nightly=None):
    mdbook = os.environ.get("MDBOOK", "mdbook")
    if os.path.exists(out):
        shutil.rmtree(out)
    subprocess.run([mdbook, "build", BOOK, "--dest-dir", out], check=True)
    commit = _git("rev-parse", "--short=7", "HEAD")
    date = _git("log", "-1", "--format=%cs")
    notice = check.read_notice(REPO)
    for root, _, files in os.walk(out):
        for name in files:
            path = os.path.join(root, name)
            if name.startswith("searchindex") and name.endswith(".js"):
                # The search index holds the page text too, placeholders included.
                with open(path, encoding="utf-8") as f:
                    text = f.read()
                with open(path, "w", encoding="utf-8") as f:
                    f.write(text.replace("@@VERSION_LABEL@@", "Version: " + (version or "latest")))
                continue
            if not name.endswith(".html"):
                continue
            rel = os.path.relpath(path, out).replace(os.sep, "/")
            with open(path, encoding="utf-8") as f:
                text = f.read()
            text = fill(fix_nav(text, rel), version, tag, commit, date, notice, nightly)
            with open(path, "w", encoding="utf-8") as f:
                f.write(text)
    # mdBook's contents frame for readers without JavaScript; this theme puts the contents in each
    # page instead, so nothing links to it, and mdBook writes it without a <title>.
    toc = os.path.join(out, "toc.html")
    if os.path.exists(toc):
        os.remove(toc)
    os.makedirs(os.path.join(out, "fonts"), exist_ok=True)
    for name in FONT_FILES:
        shutil.copyfile(os.path.join(FONTS, name), os.path.join(out, "fonts", name))
    # The OFL asks for the licence and the copyright notice to travel with the fonts.
    for name in ("OFL-1.1.txt", "IBM-Plex-NOTICE.txt"):
        shutil.copyfile(os.path.join(REPO, "LICENSES", name), os.path.join(out, "fonts", name))
    for name in ("style.css", "versions.js"):
        shutil.copyfile(os.path.join(BOOK, "theme", name), os.path.join(out, name))


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", required=True)
    ap.add_argument("--version", help="a release version (X.Y.Z); without it, the development version")
    ap.add_argument("--tag", help="the release tag, app-v<version>")
    ap.add_argument("--nightly", metavar="DATE", help="the date of the nightly build this development version is published for")
    args = ap.parse_args(argv)
    try:
        build(os.path.abspath(args.out), args.version, args.tag, args.nightly)
    except (ValueError, subprocess.CalledProcessError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    print(f"Built the {'development version' if not args.version else args.version} in {args.out}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
