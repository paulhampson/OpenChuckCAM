# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Selection sets on a blisk" (user-docs/src/examples/selection-sets.md).

Run with `python3 user-docs/tools/capture_example.py blisk_selection_sets` after building the release
program and the STEP reader (`cargo build --release -p openchuck-ui` and
`cargo build --release -p openchuck-import --features kernel --bins`). The click positions belong to
the layout of the build they were taken with, at 1920 x 1080; change them and the page together.
"""
import _blisk as b

PROJECT = b.PROJECT


def run(cap, work):
    b.front_view(cap)
    b.new_set(cap, "faces", [b.HUB_FLOOR])
    cap.shot("01-new-selection")
    b.save_set(cap, "Hub floor")
    cap.shot("02-face-set")
    b.leave_picking(cap)
    b.new_set(cap, "edges", [b.GUIDE_EDGE])
    b.save_set(cap, "Floor guide")
    cap.shot("03-edge-set")
    b.leave_picking(cap)
    b.new_set(cap, "faces", [b.BLADE_WALL])
    b.save_set(cap, "Blade wall")
    b.leave_picking(cap)
    b.new_set(cap, "faces", [b.RIM])
    b.save_set(cap, "Rim")
    cap.shot("04-four-sets")
    b.leave_picking(cap)
    b.add_operation(cap, "face_flow", "ball", "Floor finish")
    cap.click(1780, 524, 1)                    # Cut on: add...
    cap.click(1770, 557, 1.5)                  # Hub floor
    cap.click(1780, 588, 1)                    # Guide edge: add... (a row lower now)
    cap.click(1770, 621, 1.5)                  # Floor guide
    cap.click(1780, 685, 1)                    # Check: add...
    cap.click(1770, 782, 1.5)                  # Blade wall
    cap.scroll(1740, 700, 6)                   # down to Pattern and tolerance
    cap.fill(1815, 825, "0.2")                 # Stock to leave
    cap.scroll(1740, 500, 10, down=False)
    cap.shot("05-faceflow-sets")
    b.generate(cap)
    cap.click(450, 790, 1)                     # Results
    cap.shot("06-faceflow-result")
    b.dump_results(cap, work, "faceflow")
    b.add_operation(cap, "swarf", "flat", "Blade wall swarf")
    cap.click(1810, 429, 1)                    # Cut on: add...
    cap.click(1800, 494, 1.5)                  # Floor guide: the wrong kind of set, on purpose
    cap.shot("07-wrong-kind")
    cap.click(1843, 604, 1.5)                  # the x of Floor guide in Cut on
    cap.click(1810, 429, 1)                    # Cut on: add...
    cap.click(1800, 526, 1.5)                  # Blade wall
    b.generate(cap)
    cap.click(455, 790, 1)                     # Results
    cap.shot("08-swarf-before")
    b.dump_results(cap, work, "before")
    cap.click(1810, 525, 1)                    # Avoid: add...
    cap.click(1800, 558, 1.5)                  # Hub floor
    cap.click(1636, 556, 1)                    # Avoid: add... (the row under the chip)
    cap.click(1630, 653, 1.5)                  # Rim
    cap.click(1635, 684, 1.5)                  # Rim: Warn only
    cap.shot("09-swarf-sets")
    b.generate(cap)
    cap.click(455, 790, 1)                     # Results
    cap.shot("10-swarf-after")
    b.dump_results(cap, work, "after")
