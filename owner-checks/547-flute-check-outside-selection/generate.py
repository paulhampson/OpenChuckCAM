# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #547 (decisions 0522 and 0524):

    python3 owner-checks/547-flute-check-outside-selection/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 78 mm
of 24 mm stock from X -13, the chuck holding 8 mm (X -13 to -5), a 5 mm tab at the head
(X -5 to 0, 14 mm across) and a 4 mm tab at the tail (X 61 to 65, 6 mm across). Two operations:

- "Rough": rotary roughing of the whole part with the tutorial's 3.175 mm flat end mill, leaving
  0.3 mm;
- "Finish vase": helical finishing with a 3.175 mm ball whose flutes are only 5 mm long, limited
  to a selection set of the vase's triangles (X 29 to 44). Near the vase the roughing leaves
  little stock; the deep stock is in the tabs and the held region, which the selection never
  reaches.

Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
FLAT = "0c0c0547-0000-4000-8000-000000000101"
BALL = "0c0c0547-0000-4000-8000-000000000102"
VASE = "0c0c0547-0000-4000-8000-000000000201"
OP_ROUGH = "0c0c0547-0000-4000-8000-000000000301"
OP_FINISH = "0c0c0547-0000-4000-8000-000000000302"

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
diameter = 14.0

[workholding.tabs.layout]
type = "on_axis"

[[workholding.tabs]]
x_start = 61.0
length = 4.0

[workholding.tabs.shape]
type = "cylinder"
diameter = 6.0

[workholding.tabs.layout]
type = "on_axis"
"""

ROUGH = {"stepdown": "1.0", "stepover": "2.2225", "stock_to_leave": "0.3"}
FINISH = {"adaptive_spacing": "false", "stepover": "0.5", "face_free_ends": "false"}


def main():
    vase = oc.triangles_in_x(oc.BALUSTER, 29.0, 44.0)
    oc.baluster_project(
        HERE / "project",
        "Owner check 547",
        SETUP,
        tools=[
            (FLAT, oc.tool_toml(FLAT, "V", "3.175 mm flat (check 547)", 3.175, 25.0, "flat_end",
                                104, overall=42.0)),
            (BALL, oc.tool_toml(BALL, "W", "Short-flute ball 3.175x5 (check 547)", 3.175, 5.0,
                                "ball_end", 204, overall=38.0)),
        ],
        operations=[
            (OP_ROUGH, oc.operation_toml(OP_ROUGH, "V", "Rough", oc.BALUSTER_SETUP,
                                         "rotary_roughing", FLAT, ROUGH)),
            (OP_FINISH, oc.operation_toml(OP_FINISH, "W", "Finish vase", oc.BALUSTER_SETUP,
                                          "helical_finishing", BALL, FINISH, selections=[VASE])),
        ],
        selections=[(VASE, oc.selection_toml(VASE, "V", "Vase", oc.BALUSTER_MODEL,
                                             oc.BALUSTER_MODEL_HASH, "triangles", vase))],
    )
    print(f"wrote {HERE / 'project'} ({len(vase)} triangles in the Vase set)")


if __name__ == "__main__":
    main()
