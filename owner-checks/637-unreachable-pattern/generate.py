# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #637:

    python3 owner-checks/637-unreachable-pattern/generate.py

It writes project/ beside this script with the generator of the #512 check: the starting project
of the user guide's blisk examples with face-flow finishing ("Floor finish") on the hub floor
between two blades. The owner attached that check's zip to the issue: generated, the operation
leaves areas it could not reach, which the view marks with the pattern the issue is about.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent / "512-face-flow-finishing" / "generate.py"


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    subprocess.run([sys.executable, "-B", str(SOURCE), str(out)], check=True)
    text = (out / "project.toml").read_text(encoding="utf-8")
    print(f"wrote {out} ({text.splitlines()[1]})")


if __name__ == "__main__":
    main()
