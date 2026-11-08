# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #477 (decision 0500):

    python3 testdata/owner-checks/477-free-end-ring/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 74 mm
of 24 mm stock from X -13 to X 61, the chuck holding 8 mm (X -13 to -5). Between the chuck and
the part's start (X 0) there are 5 mm of free stock and no tab, so the end face at X 0 is a free
end; the stock ends at the part's other end (X 61), where nothing holds it (no held region, no
tailstock). One helical finishing operation with a 3.175 mm ball has Face free ends on and a
Stepdown of 2 mm, so it cuts a ring beside X 0 in layers down to the rotary axis: the last layer
parts the part from the stock the chuck holds.

Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
TOOL = "0c0c0477-0000-4000-8000-000000000101"
OP = "0c0c0477-0000-4000-8000-000000000301"

SETUP = """[stock]
type = "cylinder"
diameter = 24.0
length = 74.0
x_start = -13.0

[workholding.start]
length = 8.0
"""

HELICAL = {
    "face_free_ends": "true",
    "stepdown": "2.0",
    "adaptive_spacing": "false",
    "stepover": "1.0",
}


def main():
    oc.baluster_project(
        HERE / "project",
        "Owner check 477",
        SETUP,
        tools=[(TOOL, oc.tool_toml(TOOL, "V", "3.175 mm ball (check 477)", 3.175, 22.0,
                                   "ball_end", 204, shank=3.175))],
        operations=[
            (OP, oc.operation_toml(OP, "V", "Finish with free ends", oc.BALUSTER_SETUP,
                                   "helical_finishing", TOOL, HELICAL)),
        ],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()
