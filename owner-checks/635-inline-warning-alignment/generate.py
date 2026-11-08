# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the owner-check project for issue #635:

    python3 owner-checks/635-inline-warning-alignment/generate.py

It writes project/ beside this script with the generator of the #442 check: the starting project
of the user guide's blisk examples (testdata/examples/blisk-start/blisk-project), the face set
"Blade wall" (face 19) and one swarf operation, "Blade swarf", on it with every setting at its
default. That is the issue's own recipe (a single blade wall set, swarf at its defaults), and the
operation is generated with a warning shown under its settings in the Inspector.

Written for this repository (lane OWNERCHECKS2); AGPL-3.0-or-later, as the rest of the
repository. Only public material is used.
"""
import shutil
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True  # keep the repository free of __pycache__ folders

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent / "442-swarf-past-blade-end" / "generate.py"


def main():
    out = HERE / "project"
    if out.exists():
        shutil.rmtree(out)
    subprocess.run([sys.executable, "-B", str(SOURCE), str(out)], check=True)
    text = (out / "project.toml").read_text(encoding="utf-8")
    print(f"wrote {out} ({text.splitlines()[1]})")


if __name__ == "__main__":
    main()
