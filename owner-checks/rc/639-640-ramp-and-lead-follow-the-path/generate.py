# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issues #639, #640 and #642 (proposed decisions 0543 and
0544: a ramp follows the pass's own toolpath; a lead on a pass that turns A stays in the plane of
the cut):

    python3 owner-checks/rc/639-640-ramp-and-lead-follow-the-path/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 74 mm
of 24 mm stock from X -13 to 61, the chuck holding 8 mm (X -13 to -5), and the tutorial's two
tools (a 3.175 mm flat end mill and a 3.175 mm ball). Three operations, each on its own, all
entering the cut in a way the records change:

- "Rough, ramp": rotary roughing with Entry Ramp (3 degrees);
- "Rings, ramp": helical finishing in Rings with Entry Ramp (3 degrees);
- "Rings, lead": helical finishing in Rings with Entry Lead.

The held region at the start of the stock is where today's ramp and lead, which run along X,
reach. Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
FLAT = "0c0c0639-0000-4000-8000-000000000101"
BALL = "0c0c0639-0000-4000-8000-000000000102"
OP_ROUGH = "0c0c0639-0000-4000-8000-000000000301"
OP_RAMP = "0c0c0639-0000-4000-8000-000000000302"
OP_LEAD = "0c0c0639-0000-4000-8000-000000000303"

SETUP = """[stock]
type = "cylinder"
diameter = 24.0
length = 74.0
x_start = -13.0

[workholding.start]
length = 8.0
"""

ROUGH = {"stepdown": "1.0", "stepover": "2.2225", "stock_to_leave": "0.3",
         "entry": '"ramp"', "ramp_angle": "3.0"}
RINGS = {"adaptive_spacing": "false", "stepover": "1.0", "face_free_ends": "false",
         "pass_shape": '"rings"'}


def main():
    oc.baluster_project(
        HERE / "project",
        "Owner check 639 and 640",
        SETUP,
        tools=[
            (FLAT, oc.tool_toml(FLAT, "V", "3.175 mm flat (check 639)", 3.175, 25.0, "flat_end",
                                104, overall=42.0)),
            (BALL, oc.tool_toml(BALL, "W", "3.175 mm ball (check 639)", 3.175, 22.0,
                                "ball_end", 204, shank=3.175)),
        ],
        operations=[
            (OP_ROUGH, oc.operation_toml(OP_ROUGH, "V", "Rough, ramp", oc.BALUSTER_SETUP,
                                         "rotary_roughing", FLAT, ROUGH)),
            (OP_RAMP, oc.operation_toml(OP_RAMP, "W", "Rings, ramp", oc.BALUSTER_SETUP,
                                        "helical_finishing", BALL,
                                        {**RINGS, "entry": '"ramp"', "ramp_angle": "3.0"})),
            (OP_LEAD, oc.operation_toml(OP_LEAD, "X", "Rings, lead", oc.BALUSTER_SETUP,
                                        "helical_finishing", BALL, {**RINGS, "entry": '"lead"'})),
        ],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()
