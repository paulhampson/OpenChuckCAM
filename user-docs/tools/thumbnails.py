# SPDX-License-Identifier: AGPL-3.0-or-later
"""Makes the small copies of the home page's hero images that the README and the guide's front page show.

    python3 user-docs/tools/thumbnails.py [--dir user-docs/src/images] [--width 400]

For each `hero-<n>-<name>.png` in the folder it writes `hero-<n>-<name>-thumb.png`, scaled to the
width given (400 px by default), keeping the aspect ratio, with Pillow's Lanczos filter. The same
images always give the same thumbnails. `capture.py` runs it after it takes the hero images, so
the thumbnails never fall behind them; run it by hand after replacing a hero image some other way.
"""
import argparse
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
WIDTH = 400


def thumb_name(path):
    """The thumbnail's file name for a hero image's path."""
    return path.with_name(f"{path.stem}-thumb.png")


def heroes(folder):
    """The full-size hero images in the folder, in name order, never the thumbnails."""
    return sorted(p for p in Path(folder).glob("hero-*.png") if not p.stem.endswith("-thumb"))


def make(folder, width=WIDTH):
    """Writes one thumbnail per hero image and returns their paths."""
    from PIL import Image  # only making thumbnails needs Pillow, not the tests of the names

    written = []
    for src in heroes(folder):
        with Image.open(src) as im:
            height = round(im.height * width / im.width)
            small = im.convert("RGB").resize((width, height), Image.LANCZOS)
            out = thumb_name(src)
            small.save(out, optimize=True)
            written.append(out)
    return written


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dir", default=str(REPO / "user-docs/src/images"))
    ap.add_argument("--width", type=int, default=WIDTH)
    args = ap.parse_args()
    for path in make(args.dir, args.width):
        print("wrote", path)


if __name__ == "__main__":
    main()
