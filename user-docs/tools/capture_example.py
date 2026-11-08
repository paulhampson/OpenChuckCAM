# SPDX-License-Identifier: AGPL-3.0-or-later
"""Takes the screenshots of one worked example of the user guide from the real application.

    cargo build --release -p openchuck-ui
    python3 user-docs/tools/capture_example.py NAME [--work DIR] [--ui PATH]

NAME is a script in user-docs/tools/examples/ (for example `custom_machine`). Each example's
script holds its own steps and click positions, so examples are added as new files, never by
editing a shared list. It runs the program the way capture.py does (Xvfb at 1920 x 1080, Mesa's
lavapipe software Vulkan driver, `xdotool`), with a scratch HOME so it never reads or changes your
settings, libraries or projects, on a fresh copy of the tutorial's finished project
(testdata/examples/baluster/baluster-project) unless the example says otherwise.

The images go to user-docs/src/examples/images/NAME/, with a CAPTURED.toml naming the commit,
date, renderer and window size, as for the tutorial. Commit any change to the screens first.
Recapture every example at the release commit (docs/releasing.md, "Each application release").
"""
import argparse
import importlib
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "examples"))

import capture  # noqa: E402  (the shared helpers: click, fill, key, scroll, shot, record)

REPO = capture.REPO
# The guide's screenshots are 1920 x 1080 at 100 % scale (the owner's choice of 7 Nov 2026).
W, H = 1920, 1080


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("name", help="the example's script in user-docs/tools/examples/, without .py")
    ap.add_argument("--work", help="scratch folder, which must not exist yet (default: a new temporary folder)")
    ap.add_argument("--ui", default=str(REPO / "target/release/openchuck-ui"))
    args = ap.parse_args()
    example = importlib.import_module(args.name)

    import check

    if capture._git("status", "--porcelain", "--", *check.SCREEN_PATHS):
        sys.exit("error: uncommitted changes to the screens; commit them first so CAPTURED.toml names what was captured")
    commit = capture._git("rev-parse", "HEAD")
    work = capture.new_folder(args.work, "--work") if args.work else Path(tempfile.mkdtemp(prefix="ocam-"))
    out = REPO / "user-docs/src/examples/images" / args.name
    out.mkdir(parents=True, exist_ok=True)
    print("working in", work, flush=True)
    (work / "home").mkdir()
    project = work / "project"
    shutil.copytree(REPO / getattr(example, "PROJECT", "testdata/examples/baluster/baluster-project"), project)

    capture.WORK, capture.OUT, capture.HERO = work, out, out
    capture.WINDOW = f"{W}x{H}"

    def shot(name):
        """As capture.shot, with the mouse parked on the status bar of the larger window."""
        capture.pause(1.0)
        capture.move(1000, H - 22)
        capture.pause(0.7)
        capture.grab(out / f"{name}.png")
        print("captured", f"{name}.png", flush=True)

    capture.shot = shot
    capture.ENV = dict(
        os.environ,
        DISPLAY=":99",
        HOME=str(work / "home"),
        WGPU_BACKEND="vulkan",
        XDG_CONFIG_HOME=str(work / "home/.config"),
        XDG_STATE_HOME=str(work / "home/.local/state"),
    )
    icd = next(iter(sorted(Path("/usr/share/vulkan/icd.d").glob("lvp_icd*.json"))), None)
    if icd:
        capture.ENV["VK_ICD_FILENAMES"] = str(icd)

    xvfb = subprocess.Popen(["Xvfb", ":99", "-screen", "0", f"{W}x{H}x24"], env=capture.ENV,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    capture.pause(2)
    ui = subprocess.Popen([args.ui, str(project)], env=capture.ENV,
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        capture.pause(12)
        win = capture.run("xdotool", "search", "--onlyvisible", "--name", ".", stdout=subprocess.PIPE,
                          text=True).stdout.split()[0]
        capture.run("xdotool", "windowmove", win, "0", "0", "windowsize", win, str(W), str(H))
        capture.run("xdotool", "windowfocus", win)
        capture.pause(3)
        example.run(capture, work)
    finally:
        ui.terminate()
        xvfb.terminate()
    if shutil.which("optipng"):
        for png in sorted(out.glob("*.png")):
            subprocess.run(["optipng", "-quiet", "-o2", str(png)], check=False)
    print("wrote", capture.record(out, commit, time.strftime("%Y-%m-%d")))


if __name__ == "__main__":
    main()
