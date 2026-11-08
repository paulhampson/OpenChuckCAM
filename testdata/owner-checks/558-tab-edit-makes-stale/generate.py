# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #558 (decision 0528):

    python3 testdata/owner-checks/558-tab-edit-makes-stale/generate.py

It writes project/ beside this script: the public CrankShaft (testdata/step/valid/
crankshaft.step) centred on the rotary axis, X 0 to 40, in 40 mm stock from X -13 to X 47 with
the chuck holding 8 mm (X -13 to -5) and nothing holding the other end, a 5 mm tab at the head
(X -5 to 0) and a 7 mm tab at the free end (X 40 to 47), both 10 mm across, and one rotary
roughing operation with a 3.175 mm flat end mill. The check shortens the free-end tab.

Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
MODEL = "0c0c0558-0000-4000-8000-000000000001"
SETUP_ID = "0c0c0558-0000-4000-8000-000000000002"
FLAT = "0c0c0558-0000-4000-8000-000000000101"
OP = "0c0c0558-0000-4000-8000-000000000301"

TRANSFORMS = """
[[models.transform]]
kind = "centre_on_rotary_axis"
"""

SETUP = """[stock]
type = "cylinder"
diameter = 40.0
length = 60.0
x_start = -13.0

[workholding.start]
length = 8.0

[[workholding.tabs]]
x_start = -5.0
length = 5.0

[workholding.tabs.shape]
type = "cylinder"
diameter = 10.0

[workholding.tabs.layout]
type = "on_axis"

[[workholding.tabs]]
x_start = 40.0
length = 7.0

[workholding.tabs.shape]
type = "cylinder"
diameter = 10.0

[workholding.tabs.layout]
type = "on_axis"
"""


def main():
    oc.step_project(
        HERE / "project", "Owner check 558", "testdata/step/valid/crankshaft.step",
        "ba8bf9bca89758b678de9d18a76fb22ed12889fa246846bef2be08dbd4154fc0",
        "4fbc3c01f54cf2efe2fa97e8e6720d55d197c7de3c3f78b6f7dbcd83b38c6251",
        MODEL, SETUP_ID, TRANSFORMS, SETUP,
        tools=[(FLAT, oc.tool_toml(FLAT, "V", "3.175 mm flat (check 558)", 3.175, 25.0,
                                   "flat_end", 104, overall=42.0))],
        operations=[(OP, oc.operation_toml(OP, "V", "Rough", SETUP_ID, "rotary_roughing", FLAT,
                                           {"stepdown": "2.0", "stepover": "2.0",
                                            "stock_to_leave": "0.3"}))],
    )
    print(f"wrote {HERE / 'project'}")


if __name__ == "__main__":
    main()
