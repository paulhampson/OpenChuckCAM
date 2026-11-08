# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #548 (decision 0523):

    python3 testdata/owner-checks/548-tab-length-limited-op/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial, 78 mm of
24 mm stock from X -13 with the chuck holding 8 mm (X -13 to -5), a 5 mm cylinder tab at the
head (X -5 to 0, 14 mm across, the size of the tenon), a 6 mm ball end mill (wider than the
tab is long), and two axial finishing operations with that tool:

- "Vase only": limited to a selection set of the vase's triangles (X 25 to 48), far from the tab;
- "Whole part": no selection, so it may cut down beside the tab.

Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
TOOL = "0c0c0548-0000-4000-8000-000000000101"
VASE = "0c0c0548-0000-4000-8000-000000000201"
OP_VASE = "0c0c0548-0000-4000-8000-000000000301"
OP_WHOLE = "0c0c0548-0000-4000-8000-000000000302"

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
"""

AXIAL = {"stepover": "1.0", "tolerance": "0.01", "adaptive_spacing": "false", "face_free_ends": "false"}


def main():
    vase = oc.triangles_in_x(oc.BALUSTER, 25.0, 48.0)
    oc.baluster_project(
        HERE / "project",
        "Owner check 548",
        SETUP,
        tools=[(TOOL, oc.tool_toml(TOOL, "V", "6 mm ball (check 548)", 6.0, 20.0, "ball_end", 6))],
        operations=[
            (OP_VASE, oc.operation_toml(OP_VASE, "V", "Vase only", oc.BALUSTER_SETUP,
                                        "axial_finishing", TOOL, AXIAL, selections=[VASE])),
            (OP_WHOLE, oc.operation_toml(OP_WHOLE, "W", "Whole part", oc.BALUSTER_SETUP,
                                         "axial_finishing", TOOL, AXIAL)),
        ],
        selections=[(VASE, oc.selection_toml(VASE, "V", "Vase", oc.BALUSTER_MODEL,
                                             oc.BALUSTER_MODEL_HASH, "triangles", vase))],
    )
    print(f"wrote {HERE / 'project'} ({len(vase)} triangles in the Vase set)")


if __name__ == "__main__":
    main()
