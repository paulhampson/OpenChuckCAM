# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #687 (decision 0543 as amended by pull request
#761: a ramp is checked against the pass beneath it; a ramp's legs may be as short as ten path
tolerances):

    python3 owner-checks/687-blisk-ramp-in-rings/generate.py

It writes project/ beside this script: a copy of the public blisk example
(testdata/examples/blisk-start/blisk-project: testdata's blisk.step in 42 mm stock, the 3 mm
flat end mill and the 3 mm ball) with one operation, as lane RAMPENTRY's public-blisk test
(crates/openchuck-path/tests/ramp_follows.rs) sets it:

- "Rough rings, ramp": continuous rotary roughing in Rings with the 3 mm flat end mill,
  Stepover and Stepdown 1 mm, Stock to leave 0, Entry Ramp at 3 degrees, Allow single-point
  plunge on, Steeper ramp near the axis on with Maximum ramp angle 20 degrees.

Not the owner's own project from the issue: that is re-run by the owner, numbers only.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
START = oc.REPO / "testdata/examples/blisk-start/blisk-project"
SETUP = "b1150000-0000-4000-8000-000000000002"
FLAT = "b1150000-0000-4000-8000-000000000101"
OP = "0c0c0687-0000-4000-8000-000000000301"
PARAMS = {"form": '"continuous"', "pass_shape": '"rings"', "stepover": "1.0", "stepdown": "1.0",
          "stock_to_leave": "0.0", "feed": "600.0", "plunge_feed": "200.0",
          "entry": '"ramp"', "ramp_angle": "3.0", "allow_single_point_plunge": "true",
          "ramp_steeper_near_axis": "true", "ramp_angle_max": "20.0"}


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    if "3 mm flat end mill" not in (out / f"tools/{FLAT}.toml").read_text(encoding="utf-8"):
        raise SystemExit("the blisk example's tools are not as expected")
    (out / "operations").mkdir(exist_ok=True)
    text = oc.operation_toml(OP, "V", "Rough rings, ramp", SETUP, "rotary_roughing", FLAT, PARAMS)
    oc.write(out / f"operations/{OP}.toml", text.replace("schema = 25", "schema = 27", 1))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
