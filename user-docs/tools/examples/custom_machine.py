# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Create a custom machine" (user-docs/src/examples/custom-machine.md).

Run with `python3 user-docs/tools/capture_example.py custom_machine`. The click positions belong
to the layout of the build they were taken with; change them and the page together.
"""


def scroll_form(cap, ticks, down=True):
    cap.scroll(900, 500, ticks, down)


def run(cap, work):
    cap.click(175, 54, 2)                      # 2 Setup
    cap.click(105, 315, 2)                     # Setup 1 > Machine profile
    cap.scroll(1190, 500, 7)                   # the Inspector down to Machine profiles
    cap.shot("01-machine-section")
    cap.click(1263, 391, 2)                    # Manage machines...
    cap.click(236, 194, 2)                     # BUILT IN: Makera Z1
    cap.shot("02-built-in")
    cap.click(702, 90, 2)                      # Copy to edit
    cap.fill(908, 146, "Example router")       # Name
    cap.fill(746, 394, "-300")                 # Rotary reference X
    cap.fill(746, 426, "-150")                 # Rotary reference Y
    cap.fill(746, 458, "-80")                  # Axis height (machine Z)
    cap.fill(746, 546, "-600")                 # Lowest X
    cap.fill(746, 610, "-300")                 # Lowest Y
    cap.fill(746, 674, "-120")                 # Lowest Z
    cap.shot("03-copy-edited")
    scroll_form(cap, 8)
    cap.fill(720, 154, "3000")                 # X and Y feed
    cap.fill(720, 186, "1500")                 # Z feed
    cap.fill(720, 218, "7200")                 # A feed
    cap.fill(720, 250, "5000")                 # X and Y rapid
    cap.fill(720, 282, "3000")                 # Z rapid
    cap.fill(720, 314, "7200")                 # A rapid
    cap.click(694, 346, 1)                     # A in the feed word: Separate
    cap.fill(720, 466, "60")                   # Safe Z first: it may not be below the safe cylinder
    cap.fill(720, 434, "50")                   # Safe cylinder radius
    cap.fill(720, 618, "99")                   # Highest tool number
    cap.shot("04-feeds-and-clearance")
    scroll_form(cap, 4)
    cap.fill(720, 361, "24000")                # Highest spindle speed
    cap.fill(720, 329, "6000")                 # Lowest spindle speed
    cap.shot("05-spindle-and-short-moves")
    scroll_form(cap, 30, down=False)
    cap.click(554, 90, 2)                      # Save
    cap.shot("06-saved")
    cap.click(697, 490, 1.5)                   # Mark confirmed... (axis height)
    cap.shot("07-mark-confirmed")
    cap.click(607, 433, 1)                     # Cancel: an example's values are never confirmed
    cap.click(1142, 720, 1.5)                  # Close
    cap.click(1225, 387, 1.5)                  # the profile list
    cap.click(700, 420, 1.5)                   # Example router
    cap.click(1100, 419, 3)                    # Use this machine
    cap.shot("08-in-use")
