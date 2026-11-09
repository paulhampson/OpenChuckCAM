# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #644 (decision 0546: run-outs on ring and spiral
passes follow the pass):

    python3 owner-checks/644-run-outs-follow-the-pass/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 78 mm
of 24 mm stock from X -13 to 65 (4 mm of stock past the part's far end at X 61), the chuck holding
8 mm (X -13 to -5), and a 3.175 mm ball. Two helical finishing operations with Extend past the
stock at the far end (the end the chuck does not hold):

- "Spiral, extend": the helix (Passes Helix), 1 mm pitch;
- "Rings, extend": Passes Rings, 1 mm apart.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
BALL = "0c0c0644-0000-4000-8000-000000000102"
OP_SPIRAL = "0c0c0644-0000-4000-8000-000000000301"
OP_RINGS = "0c0c0644-0000-4000-8000-000000000302"

SETUP = """[stock]
type = "cylinder"
diameter = 24.0
length = 78.0
x_start = -13.0

[workholding.start]
length = 8.0
"""

BASE = {"adaptive_spacing": "false", "stepover": "1.0", "face_free_ends": "false",
        "extend": '"far end"'}


def main():
    oc.baluster_project(
        HERE / "project",
        "Owner check 644",
        SETUP,
        tools=[(BALL, oc.tool_toml(BALL, "V", "3.175 mm ball (check 644)", 3.175, 22.0,
                                   "ball_end", 204, shank=3.175))],
        operations=[
            (OP_SPIRAL, oc.operation_toml(OP_SPIRAL, "V", "Spiral, extend", oc.BALUSTER_SETUP,
                                          "helical_finishing", BALL, BASE)),
            (OP_RINGS, oc.operation_toml(OP_RINGS, "W", "Rings, extend", oc.BALUSTER_SETUP,
                                         "helical_finishing", BALL,
                                         {**BASE, "pass_shape": '"rings"'})),
        ],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()
