# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for decision 0536 build step 4 (setup, model, post and export
messages in one form, with field labels; the command-line help text):

    python3 owner-checks/rc/0536-step4-setup-export-cli-messages/generate.py

It writes project/ beside this script: a copy of the user guide's tutorial project
(testdata/examples/baluster/baluster-project) whose setup has a safe cylinder of radius 30 mm
inside a minimum clearance of 35 mm set for the setup. That gives a setup message that is a
possible collision (decision 0538), shown on the Setup step, in Messages and in the export.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import ownercheck as oc  # noqa: E402

HERE = Path(__file__).resolve().parent
START = oc.REPO / "testdata/examples/baluster/baluster-project"
SETUP = "setups/57ad6798-2544-4dfb-90a0-caf67ec69484.toml"
STOCK_END = "x_start = -1.0\n"
CLEARANCE = """
[clearance]
radius = 30.0
safe_z = 40.0

[clearance_rules]
min_radius = 35.0
"""


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    text = (out / SETUP).read_text(encoding="utf-8")
    if text.count(STOCK_END) != 1:
        raise SystemExit("the tutorial's setup is not as expected")
    # clearance_rules came with schema 26, so this one file is written at 26.
    text = text.replace("schema = 25\n", "schema = 26\n", 1)
    oc.write(out / SETUP, text.replace(STOCK_END, STOCK_END + CLEARANCE))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
