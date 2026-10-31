# SPDX-License-Identifier: AGPL-3.0-or-later
"""Puts the published documentation site together, and adds a release to the publish branch
(decision 0400 section V, NFR-19).

    python3 user-docs/tools/assemble.py site --latest BUILT [--releases CHECKOUT] --out DIR
    python3 user-docs/tools/assemble.py add-release --built BUILT --branch CHECKOUT --version X.Y.Z

`site` copies the development build to DIR/latest/, every released folder of the docs-releases
checkout to DIR/v<version>/ after checking it against its manifest, and writes versions.json,
versions.html, a root index.html that redirects to the newest release (or to latest/ before the
first release, decision D6) and a 404 page. `add-release` adds one release's build to the
checkout with its manifest; it refuses to replace a version that is already there.
"""
import argparse
import html
import json
import os
import re
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import manifest  # noqa: E402

VERSION = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
NEWEST_PLACEHOLDER = "<span data-newest-release></span>"


class AssemblyError(Exception):
    pass


def sort_versions(versions):
    """Newest first, by number."""
    return sorted(versions, key=lambda v: tuple(int(x) for x in VERSION.match(v).groups()), reverse=True)


def release_versions(names):
    """The versions of the folders named v<major>.<minor>.<patch>, newest first."""
    return sort_versions([n[1:] for n in names if n.startswith("v") and VERSION.match(n[1:])])


def _newest_text(newest):
    if newest is None:
        return "There is no release yet."
    return (f'See the documentation for the newest release, '
            f'<a href="../v{html.escape(newest)}/">{html.escape(newest)}</a>.')


def _fill_latest(folder, newest):
    for root, _, files in os.walk(folder):
        for name in files:
            if name.endswith(".html"):
                path = os.path.join(root, name)
                with open(path, encoding="utf-8") as f:
                    text = f.read()
                if NEWEST_PLACEHOLDER in text:
                    with open(path, "w", encoding="utf-8") as f:
                        f.write(text.replace(NEWEST_PLACEHOLDER, _newest_text(newest)))


PAGE = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>{title}</title>
{head}<style>
body {{ font: 17px/1.65 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; margin: 0; padding: 2.5rem 1rem;
  background: #fbfaf7; color: #1b1d1f; }}
main {{ max-width: 40rem; margin: 0 auto; }}
a {{ color: #365569; }}
table {{ border-collapse: collapse; width: 100%; }}
th {{ text-align: left; border-bottom: 2px solid currentColor; padding: 0.3rem 0.8rem 0.3rem 0; }}
td {{ border-bottom: 1px solid #d6d2ca; padding: 0.45rem 0.8rem 0.45rem 0; vertical-align: top; }}
@media (prefers-color-scheme: dark) {{
  body {{ background: #151719; color: #e4e2dd; }} a {{ color: #9cb6c8; }} td {{ border-color: #34383c; }}
}}
</style>
</head>
<body>
<main>
{body}
</main>
</body>
</html>
"""


def _versions_page(newest, releases):
    rows = ['<tr><td><a href="latest/">latest</a></td><td>The nightly build: the development version, built from the main branch. '
            'It may describe features that are not in a release yet.</td></tr>']
    for v in releases:
        what = "The newest release." if v == newest else "An earlier release."
        rows.append(f'<tr><td><a href="v{v}/">{v}</a></td><td>{what}</td></tr>')
    note = "" if releases else "<p>There is no release yet. When there is, it will be listed here.</p>\n"
    body = ("<h1>Versions of the OpenChuckCAM user guide</h1>\n"
            "<p>Each release of OpenChuckCAM has its own copy of this guide. Use the copy that matches the "
            "program you run.</p>\n"
            "<table>\n<thead><tr><th>Version</th><th>Describes</th></tr></thead>\n<tbody>\n"
            + "\n".join(rows) + "\n</tbody>\n</table>\n" + note)
    return PAGE.format(title="Versions of the OpenChuckCAM user guide", head="", body=body)


def assemble(latest, releases, out, base="/"):
    """Build the published site in `out` (which should be empty or absent)."""
    names = sorted(os.listdir(releases)) if releases else []
    versions = release_versions(names)
    problems = []
    for v in versions:
        problems += [f"v{v}/{p}" for p in manifest.verify(os.path.join(releases, f"v{v}"))]
    if problems:
        raise AssemblyError("released documentation does not match its manifest:\n  " + "\n  ".join(problems))
    newest = versions[0] if versions else None

    if os.path.exists(os.path.join(out, "latest")):
        raise AssemblyError(f"{out} already holds an assembled site; give an empty or new folder")
    os.makedirs(out, exist_ok=True)
    shutil.copytree(latest, os.path.join(out, "latest"), dirs_exist_ok=False)
    _fill_latest(os.path.join(out, "latest"), newest)
    for v in versions:
        shutil.copytree(os.path.join(releases, f"v{v}"), os.path.join(out, f"v{v}"))

    data = {"newest_release": newest,
            "versions": [{"version": "latest", "path": "latest/"}] + [{"version": v, "path": f"v{v}/"} for v in versions]}
    with open(os.path.join(out, "versions.json"), "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
        f.write("\n")
    with open(os.path.join(out, "versions.html"), "w", encoding="utf-8") as f:
        f.write(_versions_page(newest, versions))

    target = f"v{newest}/" if newest else "latest/"
    with open(os.path.join(out, "index.html"), "w", encoding="utf-8") as f:
        f.write(PAGE.format(
            title="OpenChuckCAM user guide",
            head=f'<meta http-equiv="refresh" content="0; url={target}">\n',
            body=f'<p>The OpenChuckCAM user guide is at <a href="{target}">{target}</a>. '
                 f'<a href="versions.html">All versions</a>.</p>'))
    with open(os.path.join(out, "404.html"), "w", encoding="utf-8") as f:
        f.write(PAGE.format(
            title="Page not found - OpenChuckCAM user guide", head="",
            body="<h1>Page not found</h1>\n<p>There is no page at this address. It may have moved, or it may "
                 "belong to another version of the guide.</p>\n<p>Go to <a href=\"{b}\">the start of the "
                 "guide</a> or <a href=\"{b}versions.html\">the list of versions</a>.</p>".format(b=html.escape(base))))
    return data


def add_release(built, branch, version):
    """Copy one release's build into the publish-branch checkout, with its manifest. Never replaces."""
    if not VERSION.match(version):
        raise AssemblyError(f"'{version}' is not a release version (major.minor.patch)")
    target = os.path.join(branch, f"v{version}")
    if os.path.exists(target):
        raise AssemblyError(f"v{version} is already on the publish branch; a released version is never replaced "
                            "(docs/releasing.md, \"Withdrawing a released version\")")
    shutil.copytree(built, target)
    return manifest.write(target)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("site")
    s.add_argument("--latest", required=True)
    s.add_argument("--releases")
    s.add_argument("--out", required=True)
    s.add_argument("--base", default="/", help="the site's path on the server, for the 404 page (for example /OpenChuckCAM/)")
    r = sub.add_parser("add-release")
    r.add_argument("--built", required=True)
    r.add_argument("--branch", required=True)
    r.add_argument("--version", required=True)
    args = ap.parse_args(argv)
    try:
        if args.cmd == "site":
            data = assemble(args.latest, args.releases if args.releases and os.path.isdir(args.releases) else None, args.out, args.base)
            print(f"Assembled latest and {len(data['versions']) - 1} released versions in {args.out}.")
        else:
            digest = add_release(args.built, args.branch, args.version)
            print(f"Added v{args.version}; the SHA-256 of its MANIFEST.sha256 is {digest}")
            gh = os.environ.get("GITHUB_OUTPUT")
            if gh:
                with open(gh, "a", encoding="utf-8") as f:
                    f.write(f"manifest_sha256={digest}\n")
    except AssemblyError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
