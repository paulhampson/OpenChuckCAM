#!/bin/bash
# Spike (throwaway): run the SP-3 and SP-5 fixtures through LinuxCNC 2.9.
# Set LINUXCNC_RIP to the top of a run-in-place build of LinuxCNC 2.9 (2.9.10
# was built from source for this spike). Add PATH entries so `python3` is the
# interpreter with tkinter that the build was configured with.
# As root, also set RTAPI_UID and RTAPI_FIFO_PATH (a socket file, not a
# directory), and make the build directory traversable for that UID and for
# group root (see ../sp12/linuxcnc/run_sim.sh on the SP-12 branch).
#
# 1. rs274 interpreter pass on every fixture, then the dialect file one
#    statement at a time (so one error does not hide the rest).
# 2. Headless simulator run of the three SP-3 files (about 90 s each).
set -e
cd "$(dirname "$0")"
. "$LINUXCNC_RIP/scripts/rip-environment"
mkdir -p out
TMP="${TMPDIR:-/tmp}"
interp() {  # file.ngc -> prints "exit=N canon_lines=M" and the last message
  n=$(basename "$1" .ngc)
  set +e
  printf '1\n' | rs274 -i checks.ini -t sim.tbl -v "$TMP/chk-$n.var" "$1" "out/canon_$n.txt" > "$TMP/chk-$n.out" 2>&1
  rc=$?
  set -e
  echo "exit=$rc canon_lines=$(wc -l < "out/canon_$n.txt") $(grep -v -E '^(enter|[1-5] = |$|link )' "$TMP/chk-$n.out" | tail -2 | tr '\n' ' ')"
}
{
  echo "== rs274 on each fixture"
  for f in fixtures/*.ngc; do printf '%-45s ' "$f"; interp "$f"; done
  echo
  echo "== linuxcnc_dialect_extra.ngc, one statement at a time"
  echo "(each statement runs after: G21 G90 G17, #1=5, #<depth>=-1.5, G0 X0 Y0 Z0 A0, F100, S500 M3)"
  i=0
  while IFS= read -r line; do
    i=$((i + 1))
    case "$line" in "("*) continue ;; esac
    f="$TMP/dialect_$(printf %02d "$i").ngc"
    printf '%%\nG21 G90 G17\n#1 = 5.0\n#<depth> = -1.5\nG0 X0 Y0 Z0 A0\nF100\nS500 M3\n%s\nM2\n%%\n' "$line" > "$f"
    printf '%-40s ' "$line"; interp "$f"
  done < fixtures/linuxcnc_dialect_extra.ngc
} | tee out/interpreter_results.txt
echo
echo "== simulator, SP-3 files (exact stop, commanded position sampled at about 1 kHz)"
IR=38.002   # cut_time_ir printed by `cargo run` in the SP-3 spike
for n in sp3_g94 sp3_g93 sp3_naive; do
  CHECK_PROGRAM="$PWD/fixtures/$n.ngc" CHECK_REPORT="$PWD/out/sim_$n.json" linuxcnc checks.ini > "$TMP/chk-sim-$n.log" 2>&1
  python3 -I analyse.py "out/sim_$n.json" "fixtures/$n.ngc" $IR
done | tee out/simulator_results.txt
