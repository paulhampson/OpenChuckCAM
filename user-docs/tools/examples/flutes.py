# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "The flute-length warning" (user-docs/src/examples/flute-length-warning.md).

Run with `python3 user-docs/tools/capture_example.py flutes`. The click positions belong to the
layout of the build they were taken with, at 1920 x 1080; change them and the page together.
"""

GENERATE = 75  # seconds; the finish in layers takes the longest, under 30 s here


def hover_shot(cap, x, y, name):
    """A shot with the mouse left on (x, y), so its tooltip shows (shot() parks the mouse)."""
    cap.move(x, y)
    cap.pause(2.0)
    cap.grab(cap.OUT / f"{name}.png")
    print("captured", f"{name}.png", flush=True)


def run(cap, work):
    cap.click(148, 578, 2)                     # 2 Finish
    cap.click(1764, 600, 2)                    # Edit tool...: the project's copy of T204
    cap.fill(1000, 257, "Short-flute ball 3.175x3")   # Name: not the library's tool any more
    cap.fill(850, 353, "3")                    # Flute length
    cap.shot("01-short-flutes")
    cap.click(1419, 883, 2)                    # Close
    cap.click(1855, 1057, GENERATE)            # Generate
    cap.shot("02-flute-warnings")
    hover_shot(cap, 114, 578, "03-needs-acknowledging")   # the warning mark on 2 Finish
    cap.click(1847, 1057, 6)                   # Continue to simulate
    cap.click(1855, 1057, 5)                   # Post and export...
    cap.scroll(700, 650, 15)                   # the dialog down to its checks and ticks
    cap.shot("04-export-acknowledge")
    cap.click(1433, 207, 3)                    # close the dialog: nothing is written
    cap.click(300, 54, 3)                      # 3 Operations
    cap.click(300, 54, 3)
    cap.click(148, 578, 2)                     # 2 Finish
    cap.scroll(1740, 600, 25)                  # the Inspector down to Face free ends
    cap.click(1763, 761, 1.5)                  # Finishing layers
    cap.fill(1810, 792, "2")                   # Stepdown, required with layers
    cap.click(1855, 1057, GENERATE)
    cap.click(1622, 344, 1)                    # fold TOOL AND FEEDS: its feed-limit note is under review (#600)
    cap.shot("05-finishing-layers")
    cap.click(1763, 668, 1.5)                  # untick Face free ends (where it is with TOOL AND FEEDS folded)
    cap.click(1855, 1057, GENERATE)
    cap.click(452, 789, 2)                     # Results
    cap.click(813, 831, 1)                     # Information off: warnings only (the feed-limit lines, #600)
    cap.shot("06-face-free-ends-off")
