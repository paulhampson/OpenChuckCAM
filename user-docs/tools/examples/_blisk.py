# SPDX-License-Identifier: AGPL-3.0-or-later
"""Steps shared by the worked examples on the public blisk (blisk_selection_sets, blisk_faceflow,
blisk_swarf). Not an example itself: capture_example.py runs the scripts that import it.

The blisk captures also need `xclip` (Debian or Ubuntu: `apt-get install xclip`), which reads the
Results text after Copy all; `require_xclip` stops a capture before its first step without it.

The positions belong to the layout of the build they were measured with, at 1920 x 1080, with the
view set by `front_view` (Front, Fit, then zoomed in seven steps). Change them and the pages together.
"""
import shutil
import subprocess

PROJECT = "testdata/examples/blisk-start/blisk-project"

# Points on the part in the view `front_view` leaves.
HUB_FLOOR = (1150, 330)     # face 20, the hub floor between two blades
BLADE_WALL = (1000, 280)    # face 19, the side of the blade above that floor
RIM = (700, 450)            # face 1, the rim of the disc at the chuck end
GUIDE_EDGE = (1010, 327)    # edge 65, where the blade wall meets the floor

# The add operation dialog.
STRATEGY = (958, 380)
NAME = (868, 328)
ADD = (707, 786)
# The strategy list, scrolled to its top (Helical, Face flow) or its bottom (Swarf).
HELICAL_TOP = (745, 525)
FACE_FLOW_TOP = (751, 717)
SWARF_BOTTOM = (765, 743)
# The tool list opens under the Tool field, whose height depends on the strategy's description.
TOOL_Y = {"face_flow": 580, "swarf": 548, "helical": 516}


def require_xclip(cap):
    """Stop before any step when xclip, which `dump_results` needs, is not on the capture's PATH."""
    if not shutil.which("xclip", path=cap.ENV.get("PATH")):
        raise SystemExit("error: the blisk captures need xclip (apt-get install xclip)")


def front_view(cap):
    cap.click(298, 54, 3)                      # 3 Operations
    cap.click(576, 98, 1)                      # Front
    cap.click(721, 98, 2)                      # Fit
    cap.scroll(950, 450, 4, down=False)
    cap.scroll(1000, 450, 3, down=False)


def new_set(cap, kind, points):
    """Start a new selection, pick `points` (the first a click, the rest Shift-clicks) and save it."""
    cap.click(243, 460, 1)                     # Selection sets +
    cap.click(373 if kind == "faces" else 421, 149, 0.5)   # Pick: Faces or Edges
    cap.click(321, 233, 0.5)                   # Select one (Q)
    first, *rest = points
    cap.click(*first, 1.5)
    for p in rest:
        cap.move(*p)
        cap.run("xdotool", "keydown", "shift", "click", "1", "keyup", "shift")
        cap.pause(1.5)


def save_set(cap, name):
    cap.fill(1655, 278, name)                  # the name field of New selection
    cap.click(1786, 278, 3)                    # Save as new set


def leave_picking(cap):
    cap.key("Escape")
    cap.pause(0.5)


def add_operation(cap, strategy, tool, name):
    """strategy: 'helical', 'face_flow' or 'swarf'; tool: 'flat' or 'ball'."""
    cap.click(1630, 54, 2)                     # + New operation
    cap.click(*STRATEGY, 1)
    if strategy == "swarf":
        cap.scroll(958, 650, 10)
        cap.click(*SWARF_BOTTOM, 1)
    else:
        cap.scroll(958, 650, 10, down=False)
        cap.click(*(HELICAL_TOP if strategy == "helical" else FACE_FLOW_TOP), 1)
    y = TOOL_Y[strategy]
    cap.click(785, y, 1)
    cap.click(760, y + (97 if tool == "ball" else 65), 1)
    cap.fill(*NAME, name)
    cap.click(*ADD, 2)


def generate(cap, timeout=900):
    """Generate the stale operations and wait, bounded: the shared `capture.generate` (issue 629)."""
    cap.generate(timeout)


def dump_results(cap, work, label, copy_all=(561, 832)):
    """Copy the Results text (Copy all) into `work`, for checking the page's quotations. Nothing
    here is published."""
    cap.click(*copy_all, 0.8)
    out = subprocess.run(["xclip", "-o", "-selection", "clipboard"], env=cap.ENV,
                         capture_output=True, text=True, check=True).stdout
    (work / f"{label}.txt").write_text(out)
    print(f"--- {label}\n{out}", flush=True)
