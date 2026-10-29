# SPDX-License-Identifier: AGPL-3.0-or-later
"""Takes the screenshots of TUTORIAL.md from the real application (lane DOCSUSER).

It starts a virtual X display of 1366 x 768 (Xvfb), runs the release build of
`openchuck-ui` on it with Mesa's lavapipe software Vulkan driver (the same way
decision 0131 took the reference screens), and drives the program with
`xdotool` clicks and key presses at fixed positions, as a user would. Each
`shot()` is a real capture of the window, not a drawing.

    cargo build --release -p openchuck-ui
    python3 docs/tutorial/capture.py [--work DIR] [--out DIR] [--keep-project DIR]

Needs: Xvfb, xdotool, ImageMagick (`import`), optipng (optional), Python 3 with
Pillow, and a Vulkan software driver (Debian or Ubuntu: `mesa-vulkan-drivers`,
`xvfb`, `xdotool`, `imagemagick`, `optipng`, `python3-pil`, `libxkbcommon-x11-0`).

The script uses its own HOME inside --work, so it never reads or changes your
settings or tool library. The click positions belong to the layout of this
build; when a screen changes, change the positions here and run it again.
Everything is deterministic except the time the program takes to generate and
simulate, which the script waits for.
"""
import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[2]
ENV = {}


def run(*args, **kw):
    return subprocess.run(args, env=ENV, check=kw.pop("check", True), **kw)


def pause(seconds):
    time.sleep(seconds)


def move(x, y):
    run("xdotool", "mousemove", str(x), str(y))
    pause(0.3)


def click(x, y, wait=1.0):
    move(x, y)
    run("xdotool", "click", "1")
    pause(wait)


def type_text(text):
    run("xdotool", "type", "--delay", "60", text)
    pause(0.5)


def key(*names):
    run("xdotool", "key", *names)
    pause(0.8)


def scroll(x, y, ticks, down=True):
    move(x, y)
    for _ in range(ticks):
        run("xdotool", "click", "5" if down else "4")
        pause(0.15)
    pause(1.0)


def fill(x, y, text):
    """Click a field, replace its text and confirm with Enter."""
    click(x, y, 0.5)
    key("ctrl+a")
    type_text(text)
    key("Return")


def grab(path):
    run("import", "-window", "root", str(path))


def shot(name):
    """The mouse goes to a quiet spot of the status bar so no hover is drawn."""
    pause(1.0)
    move(700, 745)
    pause(0.7)
    path = OUT / f"{name}.png"
    grab(path)
    print("captured", path.name, flush=True)


def pixel(x, y):
    path = WORK / "probe.png"
    grab(path)
    with Image.open(path) as im:
        return im.convert("RGB").getpixel((x, y))


def wait_green(points, timeout, what):
    """Waits for the 'up to date' dots of the operations in the project tree."""
    end = time.time() + timeout
    while time.time() < end:
        if all((lambda p: p[1] > 150 and p[0] < 110)(pixel(*pt)) for pt in points):
            return
        pause(3)
    sys.exit(f"timed out waiting for {what}")


def main():
    global WORK, OUT, ENV
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", default="/home/user/tutorial", help="scratch folder (made fresh)")
    ap.add_argument("--out", default=str(REPO / "docs/tutorial/images"))
    ap.add_argument("--ui", default=str(REPO / "target/release/openchuck-ui"))
    ap.add_argument("--keep-project", help="copy the finished project here (without recovery files)")
    args = ap.parse_args()
    WORK, OUT = Path(args.work), Path(args.out)
    if WORK.exists():
        shutil.rmtree(WORK)
    (WORK / "home").mkdir(parents=True)
    OUT.mkdir(parents=True, exist_ok=True)
    shutil.copy(REPO / "testdata/examples/baluster/baluster.stl", WORK / "baluster.stl")
    ENV = dict(
        os.environ,
        DISPLAY=":99",
        HOME=str(WORK / "home"),
        WGPU_BACKEND="vulkan",
        XDG_CONFIG_HOME=str(WORK / "home/.config"),
        XDG_STATE_HOME=str(WORK / "home/.local/state"),
    )
    icd = next(iter(sorted(Path("/usr/share/vulkan/icd.d").glob("lvp_icd*.json"))), None)
    if icd:
        ENV["VK_ICD_FILENAMES"] = str(icd)

    xvfb = subprocess.Popen(["Xvfb", ":99", "-screen", "0", "1366x768x24"], env=ENV,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    pause(2)
    ui = subprocess.Popen([args.ui], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        pause(10)
        win = run("xdotool", "search", "--onlyvisible", "--name", ".", stdout=subprocess.PIPE,
                  text=True).stdout.split()[0]
        run("xdotool", "windowmove", win, "0", "0", "windowsize", win, "1366", "768")
        run("xdotool", "windowfocus", win)
        pause(3)
        tutorial(args)
    finally:
        ui.terminate()
        xvfb.terminate()
    if shutil.which("optipng"):
        for png in sorted(OUT.glob("*.png")):
            subprocess.run(["optipng", "-quiet", "-o2", str(png)], check=False)


def tutorial(args):
    # 1. A new project.
    click(497, 185)                                  # New project...
    click(645, 367, 0.5)                             # Parent folder
    type_text(str(WORK))
    click(645, 405, 0.5)                             # Project name
    key("ctrl+a")
    type_text("Baluster")
    shot("01-new-project")
    click(521, 454, 3)                               # OK
    shot("02-project-created")

    # 2. Import the model.
    click(501, 185, 2)                               # Import model...
    click(562, 205, 0.5)                             # path field
    type_text(str(WORK / "baluster.stl"))
    key("Return")
    click(796, 205, 5)                               # Check and preview
    shot("03-import-model")
    click(849, 614, 4)                               # Add to project

    # 3. Setup: machine, stock, holding.
    click(1299, 746, 3)                              # Continue to setup
    click(1211, 54, 2)                               # Add setup
    click(1100, 504, 2)                              # Use this machine (Makera Z1)
    shot("04-machine")
    click(96, 343, 2)                                # Stock in the tree
    click(1205, 223, 2)                              # Cylinder
    shot("05-stock")
    click(1225, 299, 1.5)                            # Material
    click(1214, 460, 1.5)                            # Hardwood
    click(96, 427, 2)                                # Workholding in the tree
    click(1189, 451, 2)                              # Held at the start
    fill(1260, 483, "8")                             # Grip length
    scroll(1190, 500, 6)
    click(1323, 705, 2.5)                            # Apply
    shot("06-workholding")

    # 4. Tools.
    click(1288, 746, 3)                              # Continue to operations
    click(1181, 54, 2)                               # Tool library...
    click(226, 86, 0.5)
    type_text("3.175")
    click(177, 458, 1.5)                             # Z1 O-flute 3.175x25
    shot("07-tool-library")
    click(1219, 708, 2)                              # Add to project
    click(165, 356, 1.5)                             # Z1 ball 3.175x22
    click(1219, 708, 2)
    shot("08-tools-in-project")
    click(1141, 708, 2)                              # Close

    # 5. Operations.
    click(1066, 54, 2)                               # New operation
    fill(590, 172, "Rough")
    shot("09-add-roughing")
    click(430, 630, 3)                               # Add
    fill(1240, 449, "1")                             # Stepdown
    click(716, 99, 1.5)                              # Fit
    shot("10-roughing-settings")
    click(1081, 54, 2)                               # New operation
    click(680, 224, 1.5)                             # Strategy list
    shot("11-strategy-list")
    click(467, 369, 1.5)                             # Helical finishing
    click(517, 360, 1.5)                             # Tool
    click(500, 457, 1)                               # Z1 ball 3.175x22
    fill(590, 172, "Finish")
    click(430, 630, 3)                               # Add
    shot("12-finishing-settings")

    # 6. Generate.
    click(1301, 746, 5)                              # Generate stale (2)
    wait_green([(113, 551), (113, 579)], 300, "the toolpaths")
    click(571, 99)                                   # Front view
    click(716, 99, 1.5)                              # Fit
    click(90, 579, 1.5)                              # hide the Finish path
    shot("13-generated")
    click(90, 579, 1)                                # show it again

    # Hero 1 (README): the finishing path in a large 3D view. The side panels and
    # the dock are folded away to give the view the whole window, then restored.
    click(90, 551, 1)                                # hide the Rough path
    click(244, 92, 1)                                # fold the project panel
    click(1347, 92, 1)                               # fold the Inspector
    click(1321, 477, 1.5)                            # fold the dock
    click(662, 99, 1)                                # Iso view
    click(475, 99, 2)                                # Fit
    scroll(690, 420, 3, down=False)                  # zoom in
    shot("hero-1-toolpaths")
    click(13, 92, 1)                                 # unfold the project panel
    click(1352, 92, 1)                               # unfold the Inspector
    click(285, 707, 1.5)                             # unfold the dock
    click(90, 551, 1)                                # show the Rough path again

    # 7. Simulate.
    click(1297, 746, 5)                              # Continue to simulate
    pause(100)                                       # the cut part of the whole program
    click(279, 99, 1)                                # Shaded
    click(768, 99, 1)                                # Show
    click(846, 354, 1)                               # Cut part display: Solid
    key("Escape")
    scroll(640, 290, 6, down=False)                  # zoom in
    click(429, 520, 10)                              # next operation: after roughing
    shot("15-after-roughing")
    click(535, 618, 1)                               # speed 10 000x
    click(355, 520, 1)                               # play
    pause(25)
    shot("14-simulate-final")

    # Hero 2 (README): the cut part coloured by deviation from the model.
    scroll(1190, 300, 8, down=False)                 # Inspector back to its top
    click(1042, 135, 2)                              # Deviation colours
    click(662, 99, 1)                                # Iso view
    click(1004, 477, 2)                              # fold the dock
    scroll(640, 290, 2, down=False)                  # zoom in a little
    shot("hero-2-deviation")
    click(1042, 135, 1)                              # Deviation colours off
    click(285, 707, 1.5)                             # unfold the dock

    # 8. Post and export.
    click(1275, 746, 3)                              # Continue to post and export
    click(1301, 746, 3)                              # Post and export...
    click(420, 465, 0.5)                             # Output folder
    type_text(str(WORK / "output"))
    key("Return")
    shot("16-export-dialog")
    scroll(400, 560, 8)
    shot("17-export-checks")
    click(220, 616, 1.5)                             # I will verify this G-code...
    click(1113, 702, 5)                              # Write G-code
    shot("18-export-result")
    click(1025, 589, 1)                              # Dismiss
    click(1036, 702, 1)                              # Cancel
    click(19, 17, 1)                                 # File
    click(28, 146, 3)                                # Save
    if args.keep_project:
        dest = Path(args.keep_project)
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(WORK / "Baluster", dest, ignore=shutil.ignore_patterns(".openchuck-recovery"))


if __name__ == "__main__":
    main()
