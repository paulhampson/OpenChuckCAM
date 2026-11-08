# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #600 (decision 0536 step 2):

    python3 owner-checks/600-feed-limit-summary/generate.py

It writes project/ beside this script: an unchanged copy of the user guide's tutorial project,
testdata/examples/baluster/baluster-project (the public baluster, a roughing and a finishing
operation), so the check reads the messages of the job the tutorial makes.

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


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(START, out)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
