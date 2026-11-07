# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Simultaneous finishing" (user-docs/src/examples/simultaneous-finishing.md).

Run with `python3 user-docs/tools/capture_example.py simultaneous`, after building the release
program and the STEP reader (`cargo build --release -p openchuck-import --features kernel --bin
openchuck-step-worker`). It starts from the project written by
testdata/examples/crankshaft-simultaneous/generate.py. The click positions belong to the layout
of the build they were taken with, at 1920 x 1080; change them and the page together.
"""

PROJECT = "testdata/examples/crankshaft-simultaneous/crankshaft-project"
GENERATE = 160  # seconds; both operations take about 90 s here


def add_operation(cap, name, tool_row, strategy_row=None):
    cap.click(1630, 54, 2)                     # + New operation
    if strategy_row:
        cap.click(958, 379, 1.5)               # Strategy
        cap.click(780, strategy_row, 1.5)
    cap.click(785, 515, 1.5)                   # Tool
    cap.click(793, tool_row, 1)
    cap.fill(868, 327, name)                   # Name


def run(cap, work):
    cap.shot("01-setup")                       # the project opens on the Setup step
    cap.click(300, 54, 2)                      # 3 Operations
    add_operation(cap, "Rough", 580)           # Rotary roughing, Z1 O-flute 3.175x25
    cap.click(707, 785, 3)                     # Add
    cap.fill(1810, 983, "1")                   # Stepdown
    add_operation(cap, "Finish", 612, strategy_row=620)   # Simultaneous 4-axis finishing, Z1 ball
    cap.shot("02-add-simultaneous")
    cap.click(707, 785, 3)                     # Add
    cap.click(90, 550, 1)                      # hide 1 Rough's path, before generating (#607)
    cap.click(1855, 1057, GENERATE)            # Generate
    cap.click(1640, 695, 1)                    # fold TOOL AND FEEDS: its feed-limit note is under review (#600)
    cap.click(668, 98, 2)                      # Iso
    cap.click(452, 789, 2)                     # Results
    cap.click(813, 831, 1)                     # Information off: the notes and warnings only
    cap.shot("03-finish-path")
    cap.scroll(1740, 600, 15)                  # the Inspector down to TOOL AXIS AND A
    cap.shot("04-tool-axis")
    cap.fill(1810, 952, "10")                  # Lead angle
    cap.click(1855, 1057, GENERATE)
    cap.shot("05-lead-10")
    cap.click(1840, 697, 1)                    # Pattern: axial, with the lead still at 10
    cap.click(1855, 1057, 30)
    cap.shot("06-axial-with-lead")
