# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Swarf on a blisk blade" (user-docs/src/examples/swarf.md).

Run with `python3 user-docs/tools/capture_example.py blisk_swarf` after building the release
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
    b.new_set(cap, "faces", [b.BLADE_WALL])
    b.save_set(cap, "Blade wall")
    b.leave_picking(cap)
    b.new_set(cap, "faces", [b.HUB_FLOOR])
    b.save_set(cap, "Hub floor")
    b.leave_picking(cap)
    b.add_operation(cap, "swarf", "flat", "Blade swarf")
    cap.click(1810, 429, 1)                    # Cut on: add...
    cap.click(1800, 462, 1.5)                  # Blade wall
    cap.shot("01-geometry")
    b.generate(cap)
    cap.click(455, 790, 1)                     # Results
    cap.shot("02-flat")
    b.dump_results(cap, work, "flat")
    cap.scroll(1740, 700, 6)
    cap.click(1675, 465, 1)                    # the tool list (T1 3 mm flat end mill), below Save to library...
    cap.click(1650, 529, 1.5)                  # T2 3 mm ball
    b.generate(cap)
    cap.click(455, 790, 1)                     # Results
    cap.shot("03-ball")
    b.dump_results(cap, work, "ball")
    cap.scroll(1740, 500, 10, down=False)
    cap.click(1810, 525, 1)                    # Avoid: add...
    cap.click(1800, 590, 1.5)                  # Hub floor
    cap.shot("04-avoid-set")
    b.generate(cap)
    cap.click(455, 790, 1)                     # Results
    cap.shot("05-avoid")
    b.dump_results(cap, work, "avoid")
    cap.scroll(1740, 700, 6)
    cap.click(1777, 780, 1.5)                  # Keep off the end faces: off
    cap.shot("06-keep-off-setting")
    b.generate(cap)
    cap.click(455, 790, 1)                     # Results
    cap.shot("07-keep-off-off")
    b.dump_results(cap, work, "keep-off-off")
