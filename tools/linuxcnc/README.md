# LinuxCNC simulator configuration (gate 3)

Used by `cargo xtask replay` (verification plan section 6, gate 3; work breakdown slice 6 item 9). Not a product component: test tooling.

- `checks.ini`: headless LinuxCNC 2.9 with trivkins XYZA, millimetres, exact stop. Axis limits are a verification envelope around the rotary axis in work coordinates, wide enough for the safe Z; the machine's real travel is checked by gate 1.
- `sim.tbl`: a dummy tool table (tools 1 to 9).
- `sim_driver.py`: LinuxCNC's display program. It homes the machine, runs `CHECK_PROGRAM`, resumes at every `M0`, samples the commanded position about every millisecond and writes `CHECK_REPORT`: the lines that moved, the errors LinuxCNC raised, the final position and every distinct position sampled.

```
cargo xtask replay                         # every LinuxCNC golden program
cargo xtask replay --docker IMAGE prog.ngc # LinuxCNC from the image in docker/linuxcnc
LINUXCNC_RIP=/path/to/linuxcnc cargo xtask replay prog.ngc   # a run-in-place build
```

`openchuck-cli gcode-sim program.ngc report.json` does the comparison on its own.

A failing program is run once more and the second result counts, because a loaded machine can leave a gap in the samples that hides a corner; the driver writes the longest gap (`max_gap`, seconds) into its report. Run it on an otherwise idle machine: the simulator runs in real time (the reference jobs take 1.5 to 2.5 minutes each) and a busy machine can leave gaps in the samples. trivkins treats A as a plain number, so a pass proves the dialect, the reached positions and the absence of faults, not the A direction (decision record 0001).

The first-cut golden is expanded automatically into temporary bounded fixtures
before CI shares are assigned (GC-10, OI-991; decision 0244, accepted by the
owner on 26 Oct 2026). Every original line is
retained exactly once across contiguous source ranges. The planner supports only
this fixture's absolute metric G0/G1 subset and refuses unfamiliar modes.
Each unit targets at most 300 s estimated simulator time including startup;
both watchdogs cap it at 600 s, or a smaller explicit `--timeout`. A timeout
stops the lane for a long-test disposition rather than automatically retrying.

A fixture homes simulated joints at the original boundary coordinates, restores
motion/feed/tool/spindle state, and pauses before and after its original range. Each pause is followed by a
LinuxCNC-only `M66 E0 L0` queue barrier so read-ahead cannot change the
reported modal state before it is checked.
`boundary_checks.py` verifies actual and commanded coordinates, G modes, F,
tool, spindle enable/direction/S, offsets and rotation. Missing or mismatched
checks fail. The independent `gcode-sim --source-lines FIRST:LAST` comparison checks each
fixture against the original unsplit source, including the full prefix state,
at the existing tolerance. These fixtures introduce stops and fresh interpreter
initialization; they do not test uninterrupted full-program dynamics. Production
G-code is unchanged and remains the user's responsibility to verify.

With `--keep-reports DIR`, each chunk retains its JSON report, fixture program,
expected boundary/source-range manifest and simulator INI. The three CI shares
include every chunk exactly once. Host-side helper tests require Python 3:

```
python3 -m unittest discover -s tools/linuxcnc/tests
cargo test -p xtask req_gc_10 --locked
cargo xtask replay --docker IMAGE --keep-reports target/replay testdata/golden/first_cut/first_cut_linuxcnc.ngc
```
