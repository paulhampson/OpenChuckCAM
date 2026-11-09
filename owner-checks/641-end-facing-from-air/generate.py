# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #641 (decision 0547: End facing enters from air
beside the stock and checks its engagement):

    python3 owner-checks/641-end-facing-from-air/generate.py

It writes project/ beside this script: a copy of the user guide's tutorial project
(testdata/examples/baluster/baluster-project) whose stock is 3 mm longer (65 mm from X -1, so
3 mm of stock past the part's far end at X 61), with a Facing operation added before the
roughing, "Face end": End facing of the unheld (far) end with the tutorial's 3.175 mm flat end
mill, its Stepover and Stepdown taken from the tool's cutting data (decision 0547 items 3 and 4),
and "Face end, part only": the same with Coverage extent Part only, where the stock rises above
the area to face (decision 0547 question 4).

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
START = oc.REPO / "testdata/examples/baluster/baluster-project"
SETUP_ID = "57ad6798-2544-4dfb-90a0-caf67ec69484"
SETUP = f"setups/{SETUP_ID}.toml"
FLAT = "c0de0001-0000-4000-8000-000000000104"
OP = "0c0c0641-0000-4000-8000-000000000305"
OP_PART = "0c0c0641-0000-4000-8000-000000000306"


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    text = (out / SETUP).read_text(encoding="utf-8")
    if text.count("length = 63.0\n") != 1:
        raise SystemExit("the tutorial's setup is not as expected")
    oc.write(out / SETUP, text.replace("length = 63.0\n", "length = 65.0\n"))
    oc.write(out / f"operations/{OP}.toml",
             oc.operation_toml(OP, "U", "Face end", SETUP_ID, "facing", FLAT,
                               {"shape": '"end"'}))
    oc.write(out / f"operations/{OP_PART}.toml",
             oc.operation_toml(OP_PART, "UA", "Face end, part only", SETUP_ID, "facing", FLAT,
                               {"shape": '"end"', "extent": '"part"'}))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
