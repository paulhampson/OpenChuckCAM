#!/bin/bash
# SP-12 spike (throwaway): run the LinuxCNC twin of the Z1 test program.
# Needs a LinuxCNC 2.9 run-in-place build (2.9.10 was built from source for the
# spike: ./configure --with-realtime=uspace --disable-gtk2
# --disable-build-documentation, with PYTHON pointing at the Python that has
# tkinter). LINUXCNC_RIP is the build's top directory.
#
# 1. Interpreter pass: canonical calls to out/rs274_canon.txt.
# 2. Simulator pass: headless; sim_driver.py runs the file, resumes at each M0
#    and writes per-line times to out/sim_report.json.
#
# Running as root: set RTAPI_UID to an unprivileged UID and RTAPI_FIFO_PATH to
# a socket *file* in a directory that UID can write, and make sure the build
# directory is traversable by that UID and by group root (rtapi_app keeps gid 0).
set -e
cd "$(dirname "$0")"
. "$LINUXCNC_RIP/scripts/rip-environment"
rs274 -i z1sim.ini -t sim.tbl -v "${TMPDIR:-/tmp}/sp12-rs274.var" \
  z1_conventions_test.ngc out/rs274_canon.txt </dev/null
SP12_PROGRAM="$PWD/z1_conventions_test.ngc" SP12_REPORT="$PWD/out/sim_report.json" \
  linuxcnc z1sim.ini
