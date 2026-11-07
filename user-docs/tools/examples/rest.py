# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Rest machining" (user-docs/src/examples/rest-machining.md).

Run with `python3 user-docs/tools/capture_example.py rest`. The click positions belong to the
layout of the build they were taken with, at 1920 x 1080; change them and the page together.
"""

GENERATE = 100  # seconds; the rest operation alone, from the setup's stock, takes about 45 s here


def run(cap, work):
    cap.click(1630, 54, 2)                     # + New operation
    cap.click(958, 379, 1.5)                   # Strategy
    cap.click(796, 652, 1.5)                   # Rest machining
    cap.click(913, 515, 2)                     # From the library...
    cap.click(430, 573, 1.5)                   # MAKERA Z1 > BALL END MILLS > Z1 ball 2x12
    cap.click(1496, 883, 2)                    # Add to project
    cap.fill(868, 327, "Rest")                 # Name
    cap.shot("01-add-rest")
    cap.click(707, 785, 3)                     # Add
    cap.click(1855, 1057, GENERATE)            # Generate: Rough, Finish and Rest
    cap.click(452, 789, 2)                     # Results
    cap.shot("02-rest-after-finish")
    cap.scroll(900, 950, 6)                    # the Results list down to the volumes
    cap.shot("03-rest-results")
    cap.click(57, 550, 1)                      # untick 1 Rough
    cap.click(57, 578, 1)                      # untick 2 Finish
    cap.click(1855, 1057, GENERATE)            # Generate: Rest alone
    cap.scroll(900, 950, 10, down=False)
    cap.shot("04-no-earlier-operation")
