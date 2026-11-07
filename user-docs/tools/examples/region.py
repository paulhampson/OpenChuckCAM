# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Limit an operation to a region" (user-docs/src/examples/limit-to-a-region.md).

Run with `python3 user-docs/tools/capture_example.py region`. The click positions belong to the
layout of the build they were taken with, at 1920 x 1080; change them and the page together.
"""

GENERATE = 75  # seconds; the Finish limited by an avoid set takes the longest, about 40 s here


def run(cap, work):
    cap.click(243, 460, 2)                     # SELECTION SETS +: starts picking
    cap.click(577, 98, 2)                      # Front view
    cap.click(1085, 445, 2)                    # the ball of the finial, By angle 30 degrees
    cap.fill(1658, 277, "Finial ball")         # the set's name
    cap.shot("01-pick-the-ball")
    cap.click(1787, 277, 2)                    # Save as new set
    cap.key("Escape")                          # stop picking
    cap.click(148, 578, 2)                     # 2 Finish
    cap.click(1855, 1057, GENERATE)            # Generate: the whole part, as in the tutorial
    cap.click(90, 550, 2)                      # hide 1 Rough's path
    cap.click(452, 789, 2)                     # Results
    cap.shot("02-before")
    cap.click(1817, 396, 1.5)                  # Cut on: add...
    cap.click(1807, 429, 2)                    # Finial ball
    cap.click(1855, 1057, GENERATE)
    cap.shot("03-cut-on")
    cap.click(1834, 397, 2)                    # the x on Finial ball: Cut on is empty again
    cap.click(1817, 460, 1.5)                  # Avoid: add...
    cap.click(1807, 493, 2)                    # Finial ball
    cap.click(1855, 1057, GENERATE)
    cap.shot("04-avoid")
