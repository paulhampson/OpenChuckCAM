# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Stepover, stock to leave and scallop" (user-docs/src/examples/stepover-and-scallop.md).

Run with `python3 user-docs/tools/capture_example.py scallop`. The click positions belong to the
layout of the build they were taken with, at 1920 x 1080; change them and the page together.
"""

GENERATE = 75  # seconds; the finish at a 0.01 mm scallop height takes the longest, under 30 s here


def export_times(cap, name):
    cap.click(1855, 1057, GENERATE)            # Generate
    cap.click(1847, 1057, 5)                   # Continue to simulate
    cap.click(1855, 1057, 4)                   # Post and export...
    cap.click(486, 556, 2)                     # One file per operation
    cap.shot(name)
    cap.click(1433, 207, 3)                    # close the dialog (nothing is written)


def run(cap, work):
    cap.click(148, 578, 2)                     # 2 Finish
    cap.fill(1810, 856, "0.9")                 # Stepover
    cap.shot("01-stepover")
    export_times(cap, "02-time-stepover-0.9")
    cap.click(300, 54, 3)                      # 3 Operations
    cap.click(300, 54, 3)
    cap.click(148, 578, 2)                     # 2 Finish
    cap.fill(1810, 988, "0.01")                # Scallop height; the Inspector now has the Last run note
    cap.shot("03-scallop-height")
    export_times(cap, "04-time-scallop-0.01")
