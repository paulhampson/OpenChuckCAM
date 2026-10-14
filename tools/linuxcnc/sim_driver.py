#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Headless LinuxCNC display program.

LinuxCNC starts this as its DISPLAY. It homes the simulated machine, runs the
file named in CHECK_PROGRAM, resumes at every M0, samples position at about
1 kHz, and writes a per-line report to CHECK_REPORT: for each program line
that moved, the time it moved for and the position it ended at. Then it
shuts LinuxCNC down by exiting.
"""

import json
import os
import sys
import time

import linuxcnc

PROGRAM = os.environ["CHECK_PROGRAM"]
REPORT = os.environ["CHECK_REPORT"]


def wait(stat, pred, timeout=30.0):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        stat.poll()
        if pred(stat):
            return True
        time.sleep(0.002)
    return False


def main():
    stat, cmd, err = linuxcnc.stat(), linuxcnc.command(), linuxcnc.error_channel()
    wait(stat, lambda s: True)
    cmd.state(linuxcnc.STATE_ESTOP_RESET)
    cmd.state(linuxcnc.STATE_ON)
    wait(stat, lambda s: s.task_state == linuxcnc.STATE_ON)
    cmd.mode(linuxcnc.MODE_MANUAL)
    cmd.teleop_enable(0)
    cmd.home(-1)
    if not wait(stat, lambda s: all(s.homed[:4])):
        sys.exit("homing failed")
    cmd.mode(linuxcnc.MODE_AUTO)
    wait(stat, lambda s: s.task_mode == linuxcnc.MODE_AUTO)
    cmd.program_open(PROGRAM)
    cmd.auto(linuxcnc.AUTO_RUN, 0)
    wait(stat, lambda s: s.interp_state != linuxcnc.INTERP_IDLE, 5.0)

    samples, errors = [], []
    t0 = time.monotonic()
    while True:
        stat.poll()
        now = time.monotonic() - t0
        pos = list(stat.position[:4])
        samples.append((now, stat.motion_line, pos))
        e = err.poll()
        if e:
            errors.append(e[1])
        if stat.task_paused or stat.paused:
            time.sleep(0.05)
            cmd.auto(linuxcnc.AUTO_RESUME)
        if stat.interp_state == linuxcnc.INTERP_IDLE and stat.queue == 0 and now > 1:
            break
        if now > 900:
            errors.append("timeout")
            break
        time.sleep(0.001)

    # Per line: from the first sample that differs from the line's first
    # position to the first sample that reaches the line's last position.
    by_line = {}
    for t, line, pos in samples:
        by_line.setdefault(line, []).append((t, pos))
    rows = []
    for line in sorted(by_line):
        run = by_line[line]
        first, last = run[0][1], run[-1][1]
        moved = [t for t, p in run if p != first]
        if not moved:
            continue
        start = moved[0]
        end = next(t for t, p in run if p == last)
        rows.append({"line": line, "seconds": round(end - start, 3), "t0": round(start, 4), "t1": round(end, 4), "first": [round(v, 4) for v in first], "end": [round(v, 4) for v in last]})
    # Every distinct position sampled, in order: LinuxCNC's reported line number
    # lags the motion, so the comparison follows the path instead of the rows.
    path, previous = [], None
    for _, _, pos in samples:
        rounded = [round(v, 4) for v in pos]
        if rounded != previous:
            path.append(rounded)
            previous = rounded
    # The longest pause between samples: a busy machine leaves gaps, and a corner falling in one
    # cannot be seen. The runner retries once when the gap was large.
    max_gap = max((b[0] - a[0] for a, b in zip(samples, samples[1:])), default=0.0)
    stat.poll()
    with open(REPORT, "w", encoding="utf-8") as fh:
        json.dump({"program": PROGRAM, "rows": rows, "errors": errors, "path": path, "max_gap": round(max_gap, 4),
                   "final_position": list(stat.actual_position[:4]),
                   "g92_offset": list(stat.g92_offset[:4]),
                   "samples": len(samples)}, fh, indent=1)
    cmd.state(linuxcnc.STATE_ESTOP)


if __name__ == "__main__":
    main()
