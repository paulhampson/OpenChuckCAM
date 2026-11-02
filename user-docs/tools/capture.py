# SPDX-License-Identifier: AGPL-3.0-or-later
"""Takes the screenshots of the user guide's tutorial from the real application (decision 0400, section G).

It starts a virtual X display of 1366 x 768 (Xvfb), runs the release build of
`openchuck-ui` on it with Mesa's lavapipe software Vulkan driver (the same way
decision 0131 took the reference screens), and drives the program with
`xdotool` clicks and key presses at fixed positions, as a user would. Each
`shot()` is a real capture of the window, not a drawing.

    cargo build --release -p openchuck-ui
    python3 user-docs/tools/capture.py [--work DIR] [--out DIR] [--hero-out DIR] [--keep-project DIR]

The tutorial's screenshots go to user-docs/src/tutorial/images/ and the home page's hero images
to user-docs/src/images/. Beside the tutorial's images it writes CAPTURED.toml: the commit the
screens were taken at, the date, the renderer and the window size. The docs check compares that
commit with the last change to the screens (user-docs/tools/check.py); after checking the screens
against a later build without re-capturing, add `reviewed_at = "<commit>"` to the file by hand.
Commit any change to the screens before capturing: the script refuses uncommitted changes there,
because the record would name a commit that is not what was captured.

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
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
ENV = {}
# The window size and renderer of every capture (decision 0131 took the reference screens the same way).
WINDOW = "1366x768"
RENDERER = "Mesa lavapipe (software Vulkan) on Xvfb"


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
    path = (HERO if name.startswith("hero-") else OUT) / f"{name}.png"
    grab(path)
    print("captured", path.name, flush=True)


def pixel(x, y):
    from PIL import Image  # only the capture needs Pillow, not the record or its tests

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


def record(images, commit, date):
    """Write CAPTURED.toml beside the images: what user-docs/tools/check.py reads (section G)."""
    text = (
        "# SPDX-License-Identifier: AGPL-3.0-or-later\n"
        "# Written by user-docs/tools/capture.py. Add reviewed_at = \"<commit>\" after checking the\n"
        "# screens against a later build without re-capturing (user-docs/STYLE.md, Figures).\n"
        f'captured_at = "{commit}"\n'
        f'captured_on = "{date}"\n'
        f'renderer = "{RENDERER}"\n'
        f'window = "{WINDOW}"\n'
    )
    path = Path(images) / "CAPTURED.toml"
    path.write_text(text, encoding="utf-8")
    return path


def new_folder(path, option):
    """Make the folder at `path`, which must not exist: the script never deletes a folder it did not make."""
    path = Path(path)
    if path.exists():
        sys.exit(f"error: {option} {path} already exists; remove it yourself or choose a new folder")
    path.mkdir(parents=True)
    return path


def _git(*args):
    return subprocess.run(["git", *args], cwd=REPO, capture_output=True, text=True, check=True).stdout.strip()


def main():
    global WORK, OUT, HERO, ENV
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", help="scratch folder, which must not exist yet (default: a new temporary folder)")
    ap.add_argument("--out", default=str(REPO / "user-docs/src/tutorial/images"))
    ap.add_argument("--hero-out", default=str(REPO / "user-docs/src/images"))
    ap.add_argument("--ui", default=str(REPO / "target/release/openchuck-ui"))
    ap.add_argument("--keep-project", help="copy the finished project to this folder, which must not exist yet (without recovery files)")
    args = ap.parse_args()
    OUT, HERO = Path(args.out), Path(args.hero_out)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import check

    if _git("status", "--porcelain", "--", *check.SCREEN_PATHS):
        sys.exit("error: uncommitted changes to the screens; commit them first so CAPTURED.toml names what was captured")
    commit = _git("rev-parse", "HEAD")
    if args.keep_project and Path(args.keep_project).exists():
        sys.exit(f"error: --keep-project {args.keep_project} already exists; remove it yourself or choose a new folder")
    WORK = new_folder(args.work, "--work") if args.work else Path(tempfile.mkdtemp(prefix="openchuck-tutorial-"))
    print("working in", WORK, flush=True)
    (WORK / "home").mkdir()
    OUT.mkdir(parents=True, exist_ok=True)
    HERO.mkdir(parents=True, exist_ok=True)
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
        for png in sorted([*OUT.glob("*.png"), *HERO.glob("hero-*.png")]):
            subprocess.run(["optipng", "-quiet", "-o2", str(png)], check=False)
    print("wrote", record(OUT, commit, time.strftime("%Y-%m-%d")))


def tutorial(args):
    # 1. A new project.
    click(497, 185, 2)                               # New project...
    click(645, 363, 0.5)                             # Parent folder
    type_text(str(WORK))
    click(645, 405, 0.5)                             # Project name
    key("ctrl+a")
    type_text("Baluster")
    shot("01-new-project")
    click(521, 458, 3)                               # OK
    shot("02-project-created")

    # 2. Import the model.
    click(501, 185, 2)                               # Import model...
    click(562, 194, 0.5)                             # path field
    type_text(str(WORK / "baluster.stl"))
    key("Return")
    click(796, 194, 5)                               # Check and preview
    shot("03-import-model")
    scroll(680, 400, 3)                              # down to the mesh check and the copy option
    shot("03b-import-check")
    click(849, 626, 4)                               # Add to project

    # 3. Setup: machine, stock, holding.
    click(1290, 746, 3)                              # Continue to setup
    click(1181, 54, 2.5)                             # Add setup
    click(1100, 495, 2.5)                            # Use this machine (Makera Z1)
    shot("04-machine")
    click(96, 343, 2)                                # Stock in the tree
    click(1260, 223, 1.5)                            # Shape list
    click(1255, 256, 2.5)                            # Cylinder
    shot("05-stock")
    click(1270, 299, 1.5)                            # Material
    click(1262, 460, 1.5)                            # Hardwood
    click(96, 427, 2)                                # Workholding in the tree
    click(1234, 451, 2)                              # Held at the start
    fill(1282, 483, "8")                             # Grip length
    scroll(1190, 500, 12)
    click(1323, 705, 2.5)                            # Apply
    shot("06-workholding")

    # 4. Tools.
    click(1288, 746, 3)                              # Continue to operations
    click(1181, 54, 2.5)                             # Tool library...
    click(226, 90, 0.5)
    type_text("3.175")
    click(177, 464, 1.5)                             # Z1 O-flute 3.175x25
    shot("07-tool-library")
    click(1219, 708, 2)                              # Add to project
    click(165, 374, 1.5)                             # Z1 ball 3.175x22
    click(1219, 708, 2)
    shot("08-tools-in-project")
    click(1141, 708, 2)                              # Close

    # 5. Operations.
    click(1066, 54, 2)                               # New operation
    fill(590, 172, "Rough")
    shot("09-add-roughing")
    click(430, 630, 3)                               # Add
    fill(1275, 449, "1")                             # Stepdown
    click(722, 99, 1.5)                              # Fit
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
    # Adaptive spacing is on by default. On this piece it cannot confirm the spacing between
    # some passes and leaves a partial path that has to be accepted before export, so the
    # tutorial turns it off. Scrolled to the end of the Inspector it is at a fixed place.
    scroll(1190, 400, 40)
    click(1222, 513, 1.5)                            # Adaptive spacing off
    shot("12b-adaptive-spacing-off")

    # 6. Generate.
    click(1301, 746, 5)                              # Generate stale (2)
    wait_green([(113, 551), (113, 579)], 600, "the toolpaths")
    click(577, 99)                                   # Front view
    click(722, 99, 1.5)                              # Fit
    click(90, 579, 1.5)                              # hide the Finish path
    shot("13-generated")
    click(90, 579, 1)                                # show it again

    # Hero 1 (README and home page): the finishing path in a large 3D view.
    click(90, 551, 1)                                # hide the Rough path
    click(1004, 477, 1.5)                            # fold the dock
    click(668, 99, 1)                                # Iso view
    click(722, 99, 2)                                # Fit
    scroll(690, 420, 3, down=False)                  # zoom in
    shot("hero-1-toolpaths")
    click(285, 707, 1.5)                             # unfold the dock
    click(90, 551, 1)                                # show the Rough path again

    # 7. Simulate.
    click(1297, 746, 5)                              # Continue to simulate
    pause(60)                                        # the cut part of the whole program
    click(299, 99, 1)                                # Shaded
    click(722, 99, 1.5)                              # Fit
    scroll(640, 290, 6, down=False)                  # zoom in
    click(429, 520, 10)                              # next operation: after roughing
    shot("15-after-roughing")
    click(540, 634, 1)                               # speed 10 000x
    click(355, 520, 1)                               # play
    pause(30)
    shot("14-simulate-final")

    # Hero 2 (README and home page): the cut part coloured by deviation from the model.
    scroll(1190, 300, 6, down=False)                 # Inspector back to its top
    click(668, 99, 1)                                # Iso view
    click(722, 99, 1.5)                              # Fit
    scroll(640, 290, 6, down=False)                  # zoom in
    click(1036, 135, 4)                              # Deviation colours
    click(1004, 477, 2)                              # fold the dock
    scroll(640, 330, 2, down=False)
    scroll(640, 330, 3)
    shot("hero-2-deviation")
    click(285, 707, 1.5)                             # unfold the dock
    click(1036, 135, 1)                              # Deviation colours off

    # 8. Post and export.
    click(1303, 746, 3)                              # Post and export...
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
        # copytree refuses an existing folder, so nothing of the user's is replaced.
        shutil.copytree(WORK / "Baluster", Path(args.keep_project), ignore=shutil.ignore_patterns(".openchuck-recovery"))


if __name__ == "__main__":
    main()
