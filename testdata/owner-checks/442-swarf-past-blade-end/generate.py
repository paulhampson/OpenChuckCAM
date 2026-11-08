# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #442 (decision 0532) into a folder you name:

    python3 testdata/owner-checks/442-swarf-past-blade-end/generate.py OUT_FOLDER

It copies the starting project of the user guide's blisk examples
(testdata/examples/blisk-start/blisk-project: the public blisk, 42 mm oak stock, a 3 mm flat end
mill and a 3 mm ball) into OUT_FOLDER, which must not exist yet or must be empty, and adds:

- the face set "Blade wall": face 19, the side of the blade above the hub floor in the middle of
  the front view (the set of the user guide's swarf example);
- the operation "Blade swarf": swarf with the 3 mm flat end mill, cutting on Blade wall, every
  other setting at its default (so Keep off the end faces is on, on a build that has it).

The project is not stored in the repository so as not to keep a second copy of the blisk.
Written for this repository (lane OWNERCHECKS); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ownercheck as oc  # noqa: E402

START = oc.REPO / "testdata/examples/blisk-start/blisk-project"
MODEL = "b1150000-0000-4000-8000-000000000001"
MODEL_HASH = "blake3:883a66d9e35b50c8b8ff2e5380d698a14566c0e7d14eb92efc8dfb225de1c616"
SETUP_ID = "b1150000-0000-4000-8000-000000000002"
FLAT = "b1150000-0000-4000-8000-000000000101"
WALL = "0c0c0442-0000-4000-8000-000000000201"
OP = "0c0c0442-0000-4000-8000-000000000301"
SCHEMA = 27  # the starting project's schema


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    out = Path(sys.argv[1]).resolve()
    if out.exists() and any(out.iterdir()):
        raise SystemExit(f"{out} is not empty")
    shutil.copytree(START, out, dirs_exist_ok=True)
    (out / "selections").mkdir()
    (out / "operations").mkdir()
    oc.write(out / f"selections/{WALL}.toml",
             oc.selection_toml(WALL, "V", "Blade wall", MODEL, MODEL_HASH, "faces", [19])
             .replace(f"schema = {oc.SCHEMA}", f"schema = {SCHEMA}", 1))
    oc.write(out / f"operations/{OP}.toml",
             oc.operation_toml(OP, "V", "Blade swarf", SETUP_ID, "swarf", FLAT, {},
                               selections=[WALL])
             .replace(f"schema = {oc.SCHEMA}", f"schema = {SCHEMA}", 1))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
