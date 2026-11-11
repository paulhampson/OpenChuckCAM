# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #778 (decision 0645: a swarf ramp follows the pass
along the face, never round the part) into project/ beside it:

    python3 owner-checks/778-swarf-ramp-along-the-blade/generate.py

It copies the starting project of the user guide's blisk examples
(testdata/examples/blisk-start/blisk-project: the public testdata/step/valid/blisk.step, 42 mm
oak stock from X 9 to 61, a 3 mm flat end mill and a 3 mm ball) and adds, as the user guide's
swarf example and lane RAMPENTRY's public test set it up:

- the face set "Blade wall": face 19, the side of a blade above the hub floor;
- the operation "Blade swarf, ramp": swarf with the 3 mm flat end mill on Blade wall, Entry Ramp,
  every other setting at its default;
- the operation "Blade swarf, lead": the same with Entry Lead (refused by name; unchanged, #572).

Not the owner's own project attached to the issue: that is re-run by the owner, numbers only.
Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

START = oc.REPO / "testdata/examples/blisk-start/blisk-project"
MODEL = "b1150000-0000-4000-8000-000000000001"
MODEL_HASH = "blake3:883a66d9e35b50c8b8ff2e5380d698a14566c0e7d14eb92efc8dfb225de1c616"
SETUP_ID = "b1150000-0000-4000-8000-000000000002"
FLAT = "b1150000-0000-4000-8000-000000000101"
WALL = "0c0c0778-0000-4000-8000-000000000201"
OP_RAMP = "0c0c0778-0000-4000-8000-000000000301"
OP_LEAD = "0c0c0778-0000-4000-8000-000000000302"
SCHEMA = 27  # the starting project's schema


def main():
    out = Path(__file__).resolve().parent / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    (out / "selections").mkdir()
    (out / "operations").mkdir()
    oc.write(out / f"selections/{WALL}.toml",
             oc.selection_toml(WALL, "V", "Blade wall", MODEL, MODEL_HASH, "faces", [19])
             .replace(f"schema = {oc.SCHEMA}", f"schema = {SCHEMA}", 1))
    for op, order, name, entry in [(OP_RAMP, "V", "Blade swarf, ramp", "ramp"),
                                   (OP_LEAD, "W", "Blade swarf, lead", "lead")]:
        oc.write(out / f"operations/{op}.toml",
                 oc.operation_toml(op, order, name, SETUP_ID, "swarf", FLAT,
                                   {"entry": f'"{entry}"'}, selections=[WALL])
                 .replace(f"schema = {oc.SCHEMA}", f"schema = {SCHEMA}", 1))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
