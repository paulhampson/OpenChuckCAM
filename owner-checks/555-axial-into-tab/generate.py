# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #555 (decision 0527):

    python3 owner-checks/555-axial-into-tab/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 78 mm
of 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), and a 5 mm cylinder tab at the
head (X -5 to 0) 11 mm across, narrower than the tenon (14 mm), so the tenon's end face at
X 0 is a rim between the tenon's side and the tab. One axial finishing operation with a 60 degree
V-bit is limited to a selection set of the tenon's side (X 0 to 10, its end face and the tab
left out). Before the fix the passes rolled over the rim and ran down the end face towards the
tab; now they stop on the side.

Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
VBIT = "0c0c0555-0000-4000-8000-000000000101"
SIDE = "0c0c0555-0000-4000-8000-000000000201"
OP = "0c0c0555-0000-4000-8000-000000000301"

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

[workholding.tabs.shape]
type = "cylinder"
diameter = 11.0

[workholding.tabs.layout]
type = "on_axis"
"""

AXIAL = {"adaptive_spacing": "false", "stepover": "0.5", "face_free_ends": "false"}
VBIT_SHAPE = "included_angle = 60.0\ntip_diameter = 0.1\n"


def main():
    side = oc.side_triangles_in_x(oc.BALUSTER, 0.0, 10.0)
    oc.baluster_project(
        HERE / "project",
        "Owner check 555",
        SETUP,
        tools=[(VBIT, oc.tool_toml(VBIT, "V", "60 degree V-bit 4 mm (check 555)", 4.0, 8.0,
                                   "v_bit", 60, extra_shape=VBIT_SHAPE))],
        operations=[
            (OP, oc.operation_toml(OP, "V", "Tenon side", oc.BALUSTER_SETUP, "axial_finishing",
                                   VBIT, AXIAL, selections=[SIDE])),
        ],
        selections=[(SIDE, oc.selection_toml(SIDE, "V", "Tenon side", oc.BALUSTER_MODEL,
                                             oc.BALUSTER_MODEL_HASH, "triangles", side))],
    )
    print(f"wrote {HERE / 'project'} ({len(side)} triangles in the Tenon side set)")


if __name__ == "__main__":
    main()
