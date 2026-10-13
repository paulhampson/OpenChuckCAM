#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Gate 3 (verification plan section 6): run G-code programs in LinuxCNC's headless
# simulator and compare what it reached with our own replay of the same file.
#
#   tools/linuxcnc/run_replay.sh program.ngc [more.ngc ...]
#
# Needs LinuxCNC 2.9 or later with the Python `linuxcnc` module, and a built
# openchuck-cli. Set LINUXCNC_RIP to the top of a run-in-place build (its
# scripts/rip-environment is sourced); with a packaged install leave it unset and
# have `linuxcnc` on the PATH. As root, also set RTAPI_UID and RTAPI_FIFO_PATH (a
# socket file, not a directory) as LinuxCNC's run-in-place build requires.
#
# For each program the driver (sim_driver.py) homes the simulated machine, runs the
# file, samples the commanded position about every millisecond and writes a JSON
# report of where every line ended and any error LinuxCNC raised. `openchuck-cli
# gcode-sim` then fails the run if a move did not reach its position, if LinuxCNC
# reported a fault (a limit included), or if the program ended elsewhere.
#
# The simulator uses trivkins, which treats A as a plain number: this proves the
# dialect, the reached values and the absence of faults, not the A direction.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
[ "$#" -ge 1 ] || { echo "usage: $0 program.ngc [more.ngc ...]" >&2; exit 2; }
if [ -n "${LINUXCNC_RIP:-}" ]; then
  # shellcheck disable=SC1091
  . "$LINUXCNC_RIP/scripts/rip-environment"
fi
command -v linuxcnc >/dev/null || { echo "linuxcnc is not available (set LINUXCNC_RIP or install it)" >&2; exit 3; }
cli="${OPENCHUCK_CLI:-$root/target/debug/openchuck-cli}"
[ -x "$cli" ] || { echo "build the CLI first: cargo build -p openchuck-cli (or set OPENCHUCK_CLI)" >&2; exit 3; }
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$here"/checks.ini "$here"/sim.tbl "$here"/sim_driver.py "$work"/
status=0
for program in "$@"; do
  abs="$(cd "$(dirname "$program")" && pwd)/$(basename "$program")"
  name="$(basename "$program")"
  report="$work/$name.json"
  ( cd "$work" && CHECK_PROGRAM="$abs" CHECK_REPORT="$report" linuxcnc checks.ini > "$work/$name.log" 2>&1 ) || true
  if [ ! -s "$report" ]; then
    echo "$name: the simulator wrote no report; see below" >&2
    tail -20 "$work/$name.log" >&2
    status=1
    continue
  fi
  "$cli" gcode-sim "$abs" "$report" || status=1
done
exit $status
