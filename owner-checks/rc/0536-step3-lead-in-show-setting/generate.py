# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for decision 0536 build step 3 (the lead-in of every message,
and "Show setting" for every kind of field):

    python3 owner-checks/rc/0536-step3-lead-in-show-setting/generate.py

It writes project/ beside this script: the public baluster of the user guide's tutorial in 74 mm
of 24 mm stock from X -13 to 61, the chuck holding 8 mm (X -13 to -5), nothing at the far end.
Three operations each give a warning that points at a different kind of setting:

- "Rough": rotary roughing with a 3.175 mm flat end mill (stock to leave 0.3 mm);
- "Free ends": helical finishing with a 3.175 mm ball, Face free ends on and a Stepdown of 2 mm
  (a tick box and a number: the free-end ring that can part the part, as in the #477 check);
- "Short flutes" (first, before any roughing): helical finishing of the whole part with a ball whose flutes are 5 mm long
  (a tool's field: the flute length, as in the #547 check).

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
FLAT = "0c0c0536-0000-4000-8000-000000000101"
BALL = "0c0c0536-0000-4000-8000-000000000102"
SHORT = "0c0c0536-0000-4000-8000-000000000103"
OP_ROUGH = "0c0c0536-0000-4000-8000-000000000301"
OP_FREE = "0c0c0536-0000-4000-8000-000000000302"
OP_SHORT = "0c0c0536-0000-4000-8000-000000000303"

SETUP = """[stock]
type = "cylinder"
diameter = 24.0
length = 74.0
x_start = -13.0

[workholding.start]
length = 8.0
"""

ROUGH = {"stepdown": "1.0", "stepover": "2.2225", "stock_to_leave": "0.3"}
FREE = {"face_free_ends": "true", "stepdown": "2.0", "adaptive_spacing": "false", "stepover": "1.0"}
SHORTP = {"adaptive_spacing": "false", "stepover": "1.0", "face_free_ends": "false"}


def main():
    oc.baluster_project(
        HERE / "project",
        "Owner check 0536 step 3",
        SETUP,
        tools=[
            (FLAT, oc.tool_toml(FLAT, "V", "3.175 mm flat (check 0536)", 3.175, 25.0, "flat_end",
                                104, overall=42.0)),
            (BALL, oc.tool_toml(BALL, "W", "3.175 mm ball (check 0536)", 3.175, 22.0,
                                "ball_end", 204, shank=3.175)),
            (SHORT, oc.tool_toml(SHORT, "X", "Short-flute ball 3.175x5 (check 0536)", 3.175, 5.0,
                                 "ball_end", 205, overall=38.0)),
        ],
        operations=[
            (OP_ROUGH, oc.operation_toml(OP_ROUGH, "V", "Rough", oc.BALUSTER_SETUP,
                                         "rotary_roughing", FLAT, ROUGH)),
            (OP_FREE, oc.operation_toml(OP_FREE, "W", "Free ends", oc.BALUSTER_SETUP,
                                        "helical_finishing", BALL, FREE)),
            (OP_SHORT, oc.operation_toml(OP_SHORT, "U", "Short flutes", oc.BALUSTER_SETUP,
                                         "helical_finishing", SHORT, SHORTP)),
        ],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()
