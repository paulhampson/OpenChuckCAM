# SPDX-License-Identifier: AGPL-3.0-or-later
"""Takes the screenshots of the user guide's tutorial from the real application (decision 0400, section G).

It starts a virtual X display (Xvfb, 1920 x 1080), runs the release build of
`openchuck-ui` on it with Mesa's lavapipe software Vulkan driver (the same way
decision 0131 took the reference screens), and drives the program with
`xdotool` clicks and key presses at fixed positions, as a user would. Each
`shot()` is a real capture of the window, not a drawing.

    cargo build --release -p openchuck-ui
    python3 user-docs/tools/capture.py [--work DIR] [--out DIR] [--hero-out DIR] [--keep-project DIR]
    python3 user-docs/tools/capture.py --heroes

The tutorial's screenshots go to user-docs/src/tutorial/images/ and the home page's hero images
to user-docs/src/images/, with their small copies (thumbnails.py). Beside the tutorial's images it writes CAPTURED.toml: the commit the
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
# The tutorial, the hero images and the worked examples (capture_example.py) are all taken at
# 1920 x 1080, the owner's choice of 7 Nov 2026 for the guide's screenshots; the tutorial's click
# positions were recorded again at that size for the 0.55 release recapture (lane SHOTS). (The
# product's own layout tests at 1366 x 768, UI-16, are a different thing.)
SCREEN = (1920, 1080)
WINDOW = "1920x1080"
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
    run("xdotool", "type", "--delay", "60", "--", text)
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


def shot(name, quiet=(1000, 1058)):
    """The mouse goes to a quiet spot of the status bar so no hover is drawn."""
    pause(1.0)
    move(*quiet)
    pause(0.7)
    path = (HERO if name.startswith("hero-") else OUT) / f"{name}.png"
    grab(path)
    print("captured", path.name, flush=True)


def status():
    """The left end of the status bar ("Ready", or the progress of a job), as raw pixels."""
    out = subprocess.run(["import", "-window", "root", "-crop", "120x24+0+1046", "rgb:-"],
                         env=ENV, capture_output=True, check=False)
    return out.stdout


def generate(timeout=900, at=(1854, 1058)):
    """Click Generate stale at `at` and wait, bounded, until the status bar reads as it did before
    (Ready). Shared by the tutorial, the heroes and every worked example (issue 629).

    Unlike a wait for green dots, this also ends when a run finishes with warnings. It is a
    heuristic on the status bar's pixels, sampled every 5 s: it cannot tell a run that never started
    (for example an operation in error) from one that ended between two samples, so after 30 s with
    no change it prints "nothing ran" and goes on. Check the screens before committing them. A run
    still going after `timeout` seconds raises TimeoutError.
    """
    ref = status()
    click(*at, 3)
    waited, busy = 0, False
    while waited < timeout:
        pause(5)
        waited += 5
        now = status()
        busy = busy or now != ref
        if busy and now == ref:
            break
        if not busy and waited >= 30:
            print("nothing ran: the operation may be in error", flush=True)
            break
    else:
        raise TimeoutError(f"generation did not end within {timeout} s")
    pause(2)
    print("generated in about", waited, "s", flush=True)


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
    ap.add_argument("--heroes", action="store_true",
                    help="take only the two hero images, at 1920 x 1080, from the tutorial's finished project")
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
    width, height = SCREEN
    project = []
    if args.heroes:
        shutil.copytree(REPO / "testdata/examples/baluster/baluster-project", WORK / "project")
        project = [str(WORK / "project")]
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

    xvfb = subprocess.Popen(["Xvfb", ":99", "-screen", "0", f"{width}x{height}x24"], env=ENV,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    pause(2)
    ui = subprocess.Popen([args.ui, *project], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        pause(10)
        win = run("xdotool", "search", "--onlyvisible", "--name", ".", stdout=subprocess.PIPE,
                  text=True).stdout.split()[0]
        run("xdotool", "windowmove", win, "0", "0", "windowsize", win, str(width), str(height))
        run("xdotool", "windowfocus", win)
        pause(3)
        if args.heroes:
            heroes()
        else:
            tutorial(args)
    finally:
        ui.terminate()
        xvfb.terminate()
    if shutil.which("optipng"):
        for png in sorted([*OUT.glob("*.png"), *HERO.glob("hero-*.png")]):
            subprocess.run(["optipng", "-quiet", "-o2", str(png)], check=False)
    import thumbnails  # the small copies of the hero images for the README and the front page

    for path in thumbnails.make(HERO):
        print("wrote", path)
    print("wrote", record(HERO if args.heroes else OUT, commit, time.strftime("%Y-%m-%d")))


def heroes():
    """The README's and the front page's two images, from the tutorial's finished project, at 1920 x 1080."""
    click(300, 54, 2)                                # 3 Operations
    generate(900)                                    # Generate stale (2)
    # Hero 1: the finishing path in a large 3D view.
    click(90, 551, 1)                                # hide the Rough path
    click(1558, 789, 1.5)                            # fold the dock
    click(668, 98, 1)                                # Iso view
    click(722, 98, 2)                                # Fit
    scroll(920, 560, 3, down=False)                  # zoom in
    click(148, 578, 2)                               # select Finish: its settings in the Inspector
    shot("hero-1-toolpaths", quiet=(1000, 1058))
    click(1558, 1020, 1.5)                           # unfold the dock
    click(90, 551, 1)                                # show the Rough path again
    # Hero 2: the cut part at the end of the program, coloured by deviation from the model.
    click(1847, 1058, 5)                             # Continue to simulate
    pause(70)                                        # the cut part of the whole program
    click(300, 98, 1)                                # Shaded
    click(1596, 134, 4)                              # Deviation colours
    click(541, 946, 1.5)                             # speed 10 000x
    click(355, 832, 3)                               # play to the end
    pause(35)
    click(1558, 789, 2)                              # fold the dock
    click(668, 98, 1)                                # Iso view
    click(722, 98, 2)                                # Fit
    scroll(920, 560, 3, down=False)                  # zoom in
    shot("hero-2-deviation", quiet=(1000, 1058))


def tutorial(args):
    # 1. A new project.
    click(773, 340, 2)                               # New project...
    click(922, 519, 0.5)                             # Parent folder
    type_text(str(WORK))
    click(922, 570, 0.5)                             # Project name
    key("ctrl+a")
    type_text("Baluster")
    shot("01-new-project")
    click(798, 603, 3)                               # OK
    shot("02-project-created")

    # 2. Import the model.
    click(774, 341, 2)                               # Import model...
    click(839, 350, 0.5)                             # path field
    type_text(str(WORK / "baluster.stl"))
    key("Return")
    click(1073, 350, 5)                              # Check and preview
    shot("03-import-model")
    scroll(957, 556, 3)                              # down to the mesh check and the copy option
    shot("03b-import-check")
    click(1126, 781, 4)                              # Add to project

    # 3. Setup: machine, stock, holding.
    click(1844, 1058, 3)                             # Continue to setup
    click(1735, 54, 2.5)                             # Add setup
    click(1640, 482, 2.5)                            # Use this machine (Makera Z1)
    shot("04-machine")
    click(96, 343, 2)                                # Stock in the tree
    click(1814, 210, 1.5)                            # Shape list
    click(1810, 244, 2.5)                            # Cylinder
    shot("05-stock")
    click(1820, 297, 1.5)                            # Material
    click(1815, 459, 1.5)                            # Hardwood
    click(96, 427, 2)                                # Workholding in the tree
    click(1787, 674, 2)                              # Held at the start
    fill(1830, 706, "8")                             # Grip length
    scroll(1744, 700, 12)
    click(1877, 1016, 2.5)                           # Apply
    shot("06-workholding")

    # 4. Tools.
    click(1842, 1058, 3)                             # Continue to operations
    click(1735, 54, 2.5)                             # Tool library...
    click(502, 225, 0.5)                             # Search tools
    type_text("3.175")
    click(452, 598, 1.5)                             # Z1 O-flute 3.175x25
    shot("07-tool-library")
    click(1496, 882, 2)                              # Add to project
    click(442, 508, 1.5)                             # Z1 ball 3.175x22
    click(1496, 882, 2)
    shot("08-tools-in-project")
    click(1419, 882, 2)                              # Close

    # 5. Operations.
    click(1608, 54, 2)                               # New operation
    fill(867, 327, "Rough")
    shot("09-add-roughing")
    click(706, 785, 3)                               # Add
    fill(1827, 728, "1")                             # Stepdown
    click(720, 98, 1.5)                              # Fit
    shot("10-roughing-settings")
    click(1642, 54, 2)                               # New operation (Duplicate now stands left of it)
    click(958, 378, 1.5)                             # Strategy list
    shot("11-strategy-list")
    click(744, 525, 1.5)                             # Helical finishing
    click(793, 514, 1.5)                             # Tool
    click(780, 612, 1)                               # Z1 ball 3.175x22
    fill(867, 327, "Finish")
    click(706, 785, 3)                               # Add
    shot("12-finishing-settings")
    # Adaptive spacing is on by default. On this piece it cannot confirm the spacing between
    # some passes and leaves a partial path that has to be accepted before export, so the
    # tutorial turns it off. Scrolled to the end of the Inspector it is at a fixed place.
    scroll(1744, 600, 40)
    click(1775, 792, 1.5)                            # Adaptive spacing off
    shot("12b-adaptive-spacing-off")

    # 6. Generate.
    generate(900)                                    # Generate stale (2)
    click(577, 98)                                   # Front view
    click(720, 98, 1.5)                              # Fit
    click(90, 578, 1.5)                              # hide the Finish path
    shot("13-generated")
    click(90, 578, 1)                                # show it again

    # 7. Simulate.
    click(1846, 1058, 5)                             # Continue to simulate
    pause(90)                                        # the cut part of the whole program
    click(299, 98, 1)                                # Shaded
    click(720, 98, 1.5)                              # Fit
    scroll(920, 440, 6, down=False)                  # zoom in
    click(428, 830, 15)                              # next operation: after roughing
    shot("15-after-roughing")
    click(540, 945, 1)                               # speed 10 000x
    click(355, 830, 1)                               # play
    pause(60)                                        # until "Updating cut part" has gone
    shot("14-simulate-final")

    # 8. Post and export.
    click(1854, 1058, 3)                             # Post and export...
    click(697, 620, 0.5)                             # Output folder
    type_text(str(WORK / "output"))
    key("Return")
    shot("16-export-dialog")
    scroll(677, 716, 8)
    shot("17-export-checks")
    click(498, 771, 1.5)                             # I will verify this G-code...
    click(1389, 858, 5)                              # Write G-code
    shot("18-export-result")
    click(1301, 744, 1)                              # Dismiss
    click(1310, 858, 1)                              # Cancel
    click(19, 17, 1)                                 # File
    click(28, 178, 3)                                # Save
    if args.keep_project:
        # copytree refuses an existing folder, so nothing of the user's is replaced.
        shutil.copytree(WORK / "Baluster", Path(args.keep_project), ignore=shutil.ignore_patterns(".openchuck-recovery"))


if __name__ == "__main__":
    main()
