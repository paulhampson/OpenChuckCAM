# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #638 (decision 0545: blended steps between rings):

    python3 owner-checks/638-blended-ring-steps/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 74 mm
of 24 mm stock from X -13 to 61, the chuck holding 8 mm (X -13 to -5), and a 3.175 mm ball.
Two helical finishing operations in Rings, 1 mm apart:

- "Rings, as saved": no Steps between rings setting, as a project saved before it existed
  (it reads as Sharp: today's step, unchanged);
- "Rings, blended": Steps between rings Blended, Blend over 45 degrees.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
BALL = "0c0c0638-0000-4000-8000-000000000102"
OP_SAVED = "0c0c0638-0000-4000-8000-000000000301"
OP_BLEND = "0c0c0638-0000-4000-8000-000000000302"

SETUP = """[stock]
type = "cylinder"
diameter = 24.0
length = 74.0
x_start = -13.0

[workholding.start]
length = 8.0
"""

RINGS = {"adaptive_spacing": "false", "stepover": "1.0", "face_free_ends": "false",
         "pass_shape": '"rings"'}


def main():
    oc.baluster_project(
        HERE / "project",
        "Owner check 638",
        SETUP,
        tools=[(BALL, oc.tool_toml(BALL, "V", "3.175 mm ball (check 638)", 3.175, 22.0,
                                   "ball_end", 204, shank=3.175))],
        operations=[
            (OP_SAVED, oc.operation_toml(OP_SAVED, "V", "Rings, as saved", oc.BALUSTER_SETUP,
                                         "helical_finishing", BALL, RINGS)),
            (OP_BLEND, oc.operation_toml(OP_BLEND, "W", "Rings, blended", oc.BALUSTER_SETUP,
                                         "helical_finishing", BALL,
                                         {**RINGS, "ring_step": '"blended"',
                                          "ring_blend_angle": "45.0"})),
        ],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()
