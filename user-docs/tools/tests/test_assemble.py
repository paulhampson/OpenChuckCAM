# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests of the release manifests and the assembly of the published site (decision 0400 section V, NFR-19).

Run with `python3 -m unittest discover -s user-docs/tools/tests`.
"""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import assemble  # noqa: E402
import manifest  # noqa: E402

BANNER = '<span data-newest-release></span>'


def write(root, rel, text):
    path = os.path.join(root, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    return path


def read(root, rel):
    with open(os.path.join(root, rel), encoding="utf-8") as f:
        return f.read()


def built(version):
    d = tempfile.mkdtemp()
    write(d, "index.html", f'<html><meta name="docs-version" content="{version}"><p>{BANNER}</p></html>')
    write(d, "tutorial/index.html", "<html>tutorial</html>")
    return d


class Manifest(unittest.TestCase):
    """Section V: each released folder carries a SHA-256 of every file, checked on every deploy."""

    def test_req_nfr_19_written_manifest_verifies(self):
        d = built("0.31.0")
        manifest.write(d)
        self.assertEqual(manifest.verify(d), [])

    def test_req_nfr_19_manifest_is_sorted_and_deterministic(self):
        a, b = built("0.31.0"), built("0.31.0")
        manifest.write(a)
        manifest.write(b)
        self.assertEqual(read(a, manifest.NAME), read(b, manifest.NAME))
        names = [line.split("  ", 1)[1] for line in read(a, manifest.NAME).splitlines()]
        self.assertEqual(names, sorted(names))
        self.assertNotIn(manifest.NAME, names)

    def test_req_nfr_19_changed_file_fails(self):
        d = built("0.31.0")
        manifest.write(d)
        write(d, "tutorial/index.html", "<html>changed</html>")
        self.assertTrue(any("tutorial/index.html" in p for p in manifest.verify(d)))

    def test_req_nfr_19_added_and_removed_files_fail(self):
        d = built("0.31.0")
        manifest.write(d)
        write(d, "extra.html", "x")
        os.remove(os.path.join(d, "index.html"))
        problems = manifest.verify(d)
        self.assertTrue(any("extra.html" in p for p in problems))
        self.assertTrue(any("index.html" in p for p in problems))

    def test_req_nfr_19_missing_manifest_fails(self):
        self.assertTrue(manifest.verify(built("0.31.0")))


class Versions(unittest.TestCase):
    """Section V and D6: version order, the newest release, and the list readers choose from."""

    def test_req_nfr_19_versions_sort_by_number_not_text(self):
        self.assertEqual(assemble.sort_versions(["0.9.0", "0.31.0", "0.10.2", "1.0.0"]),
                         ["1.0.0", "0.31.0", "0.10.2", "0.9.0"])

    def test_req_nfr_19_folder_names_that_are_not_versions_are_ignored(self):
        self.assertEqual(assemble.release_versions(["v0.31.0", "README.md", "vfoo", "v1.0.0", ".git"]),
                         ["1.0.0", "0.31.0"])


class Assemble(unittest.TestCase):
    """Section V: latest beside every verified release, versions.json, versions.html and the root redirect."""

    def releases(self, *versions):
        d = tempfile.mkdtemp()
        for v in versions:
            folder = os.path.join(d, f"v{v}")
            os.makedirs(folder)
            src = built(v)
            for root, _, files in os.walk(src):
                for name in files:
                    rel = os.path.relpath(os.path.join(root, name), src)
                    write(folder, rel, read(src, rel))
            manifest.write(folder)
        write(d, "README.md", "Only the release workflow writes this branch.\n")
        return d

    def test_req_nfr_19_no_release_yet(self):
        out = tempfile.mkdtemp()
        assemble.assemble(built("latest"), None, out)
        data = json.loads(read(out, "versions.json"))
        self.assertEqual(data, {"newest_release": None, "versions": [{"version": "latest", "path": "latest/"}]})
        self.assertIn('url=latest/', read(out, "index.html"))
        self.assertIn("no release yet", read(out, "latest/index.html"))
        self.assertIn("latest", read(out, "versions.html"))

    def test_req_nfr_19_root_goes_to_the_newest_release(self):
        out = tempfile.mkdtemp()
        assemble.assemble(built("latest"), self.releases("0.30.0", "0.31.0"), out)
        data = json.loads(read(out, "versions.json"))
        self.assertEqual(data["newest_release"], "0.31.0")
        self.assertEqual([v["version"] for v in data["versions"]], ["latest", "0.31.0", "0.30.0"])
        self.assertIn('url=v0.31.0/', read(out, "index.html"))
        self.assertTrue(os.path.exists(os.path.join(out, "v0.30.0/tutorial/index.html")))
        self.assertIn("0.31.0", read(out, "latest/index.html"))
        self.assertNotIn(BANNER, read(out, "latest/index.html"))

    def test_req_nfr_19_released_folders_are_served_unchanged(self):
        releases = self.releases("0.31.0")
        out = tempfile.mkdtemp()
        assemble.assemble(built("latest"), releases, out)
        self.assertEqual(read(out, "v0.31.0/index.html"), read(releases, "v0.31.0/index.html"))
        self.assertFalse(os.path.exists(os.path.join(out, "README.md")))

    def test_req_nfr_19_a_changed_release_fails_the_assembly(self):
        releases = self.releases("0.31.0")
        write(releases, "v0.31.0/index.html", "tampered")
        with self.assertRaises(assemble.AssemblyError) as e:
            assemble.assemble(built("latest"), releases, tempfile.mkdtemp())
        self.assertIn("v0.31.0", str(e.exception))

    def test_req_nfr_19_an_output_folder_already_holding_a_site_is_refused(self):
        out = tempfile.mkdtemp()
        assemble.assemble(built("latest"), None, out)
        with self.assertRaises(assemble.AssemblyError):
            assemble.assemble(built("latest"), None, out)

    def test_req_nfr_19_versions_page_lists_every_version_with_links(self):
        out = tempfile.mkdtemp()
        assemble.assemble(built("latest"), self.releases("0.31.0"), out)
        page = read(out, "versions.html")
        self.assertIn('href="latest/"', page)
        self.assertIn('href="v0.31.0/"', page)
        self.assertNotIn("<script", page)


class Publish(unittest.TestCase):
    """Section V: a release adds its folder once and never replaces one."""

    def test_req_nfr_19_adding_a_new_version(self):
        branch = tempfile.mkdtemp()
        assemble.add_release(built("0.31.0"), branch, "0.31.0")
        self.assertEqual(manifest.verify(os.path.join(branch, "v0.31.0")), [])

    def test_req_nfr_19_an_existing_version_is_never_replaced(self):
        branch = tempfile.mkdtemp()
        assemble.add_release(built("0.31.0"), branch, "0.31.0")
        before = read(branch, "v0.31.0/index.html")
        with self.assertRaises(assemble.AssemblyError):
            assemble.add_release(built("0.31.0"), branch, "0.31.0")
        self.assertEqual(read(branch, "v0.31.0/index.html"), before)

    def test_req_nfr_19_a_bad_version_is_refused(self):
        with self.assertRaises(assemble.AssemblyError):
            assemble.add_release(built("x"), tempfile.mkdtemp(), "../escape")


if __name__ == "__main__":
    unittest.main()
