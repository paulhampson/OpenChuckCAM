# SPDX-License-Identifier: AGPL-3.0-or-later
"""Tests of user-docs/tools/thumbnails.py: which files it reads and the names it writes.

Run with `python3 -m unittest discover -s user-docs/tools/tests`.
"""
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import thumbnails  # noqa: E402


class Names(unittest.TestCase):
    def test_a_thumbnail_is_named_after_its_hero_image(self):
        self.assertEqual(thumbnails.thumb_name(Path("x/hero-1-toolpaths.png")), Path("x/hero-1-toolpaths-thumb.png"))

    def test_only_full_size_hero_images_are_read(self):
        with tempfile.TemporaryDirectory() as d:
            for name in ("hero-2-b.png", "hero-1-a.png", "hero-1-a-thumb.png", "other.png"):
                Path(d, name).write_bytes(b"")
            self.assertEqual([p.name for p in thumbnails.heroes(d)], ["hero-1-a.png", "hero-2-b.png"])


if __name__ == "__main__":
    unittest.main()
