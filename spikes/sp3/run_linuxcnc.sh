#!/bin/sh
# NOT RUN in the SP-3 session: LinuxCNC could not be installed there (no package source reachable).
# Run on a machine with LinuxCNC 2.9+ installed. Needs no hardware; it only interprets the files.
# 1. Interpreter pass: prints the canonical calls (STRAIGHT_FEED / STRAIGHT_TRAVERSE with A words)
#    and errors on any unsupported word. Compare the printed X Y Z A values with the file.
# 2. Simulator pass (manual): open sim/axis/axis_mm.ini (or a 4-axis XYZA sim config), load the file,
#    check the A DRO and the backplot, and record the run time shown for sp3_g94.ngc vs sp3_g93.ngc.
#    The two should agree with "cut_time_ir" printed by `cargo run`.
set -eu
for f in out/sp3_g94.ngc out/sp3_g93.ngc; do
  echo "== $f"
  rs274 -g "$f" 2>&1 | head -40
done
