# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #567 (decisions 0538 and 0540):

    python3 owner-checks/567-collision-acknowledge/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 78 mm
of 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), and tab 1 at the head (a cylinder
14 mm across, X -5 to 0, 5 mm long). The tool is a 6 mm ball (T6), wider than the tab is long, used
by one axial finishing operation on the whole part, so generation gives the "Tab length" warning,
a possible collision that must be acknowledged before the G-code is written (decision 0538).

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
TOOL = "0c0c0567-0000-4000-8000-000000000101"
OP = "0c0c0567-0000-4000-8000-000000000301"

SETUP = """[stock]
type = "cylinder"
diameter = 24.0
length = 78.0
x_start = -13.0

[workholding.start]
length = 8.0

[[workholding.tabs]]
x_start = -5.0
length = 5.0
auto = "head"

[workholding.tabs.shape]
type = "cylinder"
diameter = 14.0

[workholding.tabs.layout]
type = "on_axis"
"""

AXIAL = {"stepover": "1.0", "tolerance": "0.01", "adaptive_spacing": "false", "face_free_ends": "false"}


def main():
    oc.baluster_project(
        HERE / "project",
        "Owner check 567",
        SETUP,
        tools=[(TOOL, oc.tool_toml(TOOL, "V", "6 mm ball (check 567)", 6.0, 20.0, "ball_end", 6))],
        operations=[(OP, oc.operation_toml(OP, "V", "Whole part", oc.BALUSTER_SETUP,
                                           "axial_finishing", TOOL, AXIAL))],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()
