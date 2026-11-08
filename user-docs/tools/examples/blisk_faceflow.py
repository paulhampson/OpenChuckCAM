# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Face-flow finishing on a blisk floor" (user-docs/src/examples/faceflow.md).

Run with `python3 user-docs/tools/capture_example.py blisk_faceflow` after building the release
program and the STEP reader (`cargo build --release -p openchuck-ui` and
`cargo build --release -p openchuck-import --features kernel --bins`), with `xclip` installed.
The click positions belong to the layout of the build they were taken with, at 1920 x 1080;
change them and the page together.
"""
import _blisk as b

PROJECT = b.PROJECT


def run(cap, work):
    b.require_xclip(cap)
    b.front_view(cap)
    b.new_set(cap, "faces", [b.HUB_FLOOR])
    b.save_set(cap, "Hub floor")
    b.leave_picking(cap)
    b.new_set(cap, "edges", [b.GUIDE_EDGE])
    b.save_set(cap, "Floor guide")
    b.leave_picking(cap)
    b.add_operation(cap, "face_flow", "ball", "Floor finish")
    cap.click(1780, 524, 1)                    # Cut on: add...
    cap.click(1770, 557, 1.5)                  # Hub floor
    cap.shot("01-geometry")
    b.generate(cap)
    cap.click(450, 790, 1)                     # Results
    cap.shot("02-stepover")
    b.dump_results(cap, work, "stepover")
    cap.scroll(1740, 700, 6)                   # down to Pattern and tolerance
    cap.fill(1815, 856, "0.02")                # Scallop height
    cap.shot("03-scallop-setting")
    b.generate(cap)
    cap.click(450, 790, 1)                     # Results
    cap.shot("04-scallop")
    b.dump_results(cap, work, "scallop")
    cap.scroll(1740, 500, 10, down=False)
    cap.click(1780, 556, 1)                    # Guide edge: add...
    cap.click(1770, 589, 1.5)                  # Floor guide
    b.generate(cap)
    cap.click(450, 790, 1)                     # Results
    cap.shot("05-guide-result")
    b.dump_results(cap, work, "guide")
