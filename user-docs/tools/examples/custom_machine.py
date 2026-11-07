# SPDX-License-Identifier: AGPL-3.0-or-later
"""Screens of the worked example "Create a custom machine" (user-docs/src/examples/custom-machine.md).

Run with `python3 user-docs/tools/capture_example.py custom_machine`. The click positions belong
to the layout of the build they were taken with, at 1920 x 1080; change them and the page together.
"""

# The machine library window opens centred; its top-left corner is at (461, 181).
DX, DY = 277, 156


def at(x, y):
    """A point of the machine library window, given as it was measured with the window at (184, 25)."""
    return x + DX, y + DY


def win_click(cap, x, y, wait=1.0):
    cap.click(*at(x, y), wait)


def win_fill(cap, x, y, text):
    cap.fill(*at(x, y), text)


def scroll_form(cap, ticks, down=True):
    cap.scroll(*at(900, 500), ticks, down)


def run(cap, work):
    cap.click(175, 54, 2)                      # 2 Setup
    cap.click(105, 315, 2)                     # Setup 1 > Machine profile
    cap.shot("01-machine-section")
    cap.click(1813, 951, 2)                    # Manage machines...
    win_click(cap, 236, 194, 2)                # BUILT IN: Makera Z1
    cap.shot("02-built-in")
    win_click(cap, 702, 90, 2)                 # Copy to edit
    win_fill(cap, 908, 146, "Example router")  # Name
    win_fill(cap, 746, 394, "-300")            # Rotary reference X
    win_fill(cap, 746, 426, "-150")            # Rotary reference Y
    win_fill(cap, 746, 458, "-80")             # Axis height (machine Z)
    win_fill(cap, 746, 546, "-600")            # Lowest X
    win_fill(cap, 746, 610, "-300")            # Lowest Y
    win_fill(cap, 746, 674, "-120")            # Lowest Z
    cap.shot("03-copy-edited")
    scroll_form(cap, 8)
    win_fill(cap, 720, 154, "3000")            # X and Y feed
    win_fill(cap, 720, 186, "1500")            # Z feed
    win_fill(cap, 720, 218, "7200")            # A feed
    win_fill(cap, 720, 250, "5000")            # X and Y rapid
    win_fill(cap, 720, 282, "3000")            # Z rapid
    win_fill(cap, 720, 314, "7200")            # A rapid
    win_click(cap, 694, 346, 1)                # A in the feed word: Separate
    win_fill(cap, 720, 466, "60")              # Safe Z first: it may not be below the safe cylinder
    win_fill(cap, 720, 434, "50")              # Safe cylinder radius
    win_fill(cap, 720, 618, "99")              # Highest tool number
    cap.shot("04-feeds-and-clearance")
    scroll_form(cap, 4)
    win_fill(cap, 720, 361, "24000")           # Highest spindle speed
    win_fill(cap, 720, 329, "6000")            # Lowest spindle speed
    cap.shot("05-spindle-and-short-moves")
    scroll_form(cap, 30, down=False)
    win_click(cap, 554, 90, 2)                 # Save
    cap.shot("06-saved")
    win_click(cap, 697, 490, 1.5)              # Mark confirmed... (axis height)
    cap.shot("07-mark-confirmed")
    win_click(cap, 607, 433, 1)                # Cancel: an example's values are never confirmed
    win_click(cap, 1142, 720, 1.5)             # Close
    cap.click(1720, 947, 1.5)                  # the profile list
    cap.click(1720, 980, 1.5)                  # Example router
    cap.click(1655, 979, 3)                    # Use this machine
    cap.shot("08-in-use")
