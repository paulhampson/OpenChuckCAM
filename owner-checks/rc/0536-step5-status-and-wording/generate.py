# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for decision 0536 build step 5 (the lost status lines, error
dialogs, recovery messages and the NFR-12 wording):

    python3 owner-checks/rc/0536-step5-status-and-wording/generate.py

It writes project/ beside this script: a copy of the user guide's tutorial project
(testdata/examples/baluster/baluster-project) with a Facing operation added before the roughing,
"Face round": the stock turned down to 23 mm across with the tutorial's 3.175 mm flat end mill,
in 0.5 mm layers. Its Results line is one of the texts the step rewords ("Facing coverage
complete (verified path geometry)").

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
SETUP_ID = "57ad6798-2544-4dfb-90a0-caf67ec69484"
FLAT = "c0de0001-0000-4000-8000-000000000104"
OP = "0c0c0536-0000-4000-8000-000000000305"


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    oc.write(out / f"operations/{OP}.toml",
             oc.operation_toml(OP, "U", "Face round", SETUP_ID, "facing", FLAT,
                               {"shape": '"round"', "size": "23.0", "stepdown": "0.5"}))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
