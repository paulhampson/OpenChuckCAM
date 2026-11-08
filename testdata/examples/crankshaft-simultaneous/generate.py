# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes the example project of the worked example "Simultaneous finishing"
(user-docs/src/examples/simultaneous-finishing.md) from the public model
testdata/step/valid/crankshaft.step, using only `openchuck-cli`.

    cargo build --release -p openchuck-cli
    cargo build --release -p openchuck-import --features kernel --bin openchuck-step-worker
    python3 testdata/examples/crankshaft-simultaneous/generate.py

It writes testdata/examples/crankshaft-simultaneous/crankshaft-project/, which must not exist
yet. The project holds the model, the machine, one setup and two tools; the worked example adds
the operations in the program, as the reader does. The setup's id is new on each run, so the
files are the same in content but not byte for byte.
"""
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent / "crankshaft-project"
CLI = REPO / "target/release/openchuck-cli"
MODEL = REPO / "testdata/step/valid/crankshaft.step"
LIBRARY = REPO / "crates/openchuck-project/defaults/z1"


def cli(*args):
    print("openchuck-cli", *args, flush=True)
    subprocess.run([str(CLI), *map(str, args)], check=True, stdout=subprocess.DEVNULL)


def main():
    if OUT.exists():
        sys.exit(f"error: {OUT} already exists; remove it first")
    cli("new", OUT, "--name", "Crankshaft simultaneous")
    cli("import", OUT, MODEL)
    model = "models/" + next((OUT / "models").iterdir()).name
    cli("transform", OUT, model, "centre")                    # the rotary axis through the part's centre
    cli("machine", OUT, "makera-z1")                          # before the tools, so they take the Z1's numbers
    cli("setup", OUT, "--new", "Setup 1", "--part", model,
        "--stock", "cylinder=38,46,-4",                       # 38 mm bar from X -4 to 42; the part is X 0 to 40
        "--held-start", "6",                                  # the chuck grips X -4 to 2
        "--material", "Hardwood")
    cli("tool", OUT, "Z1 O-flute 3.175x25", "--add", "--library", LIBRARY, "--number", "104")
    cli("tool", OUT, "Z1 ball 3.175x22", "--add", "--library", LIBRARY, "--number", "204")  # the Z1 tool set's numbers
    print("wrote", OUT)


if __name__ == "__main__":
    main()
