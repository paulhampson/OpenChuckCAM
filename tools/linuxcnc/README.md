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

Run it on an otherwise idle machine: the simulator runs in real time (the reference jobs take 1.5 to 2.5 minutes each) and a busy machine can leave gaps in the samples. trivkins treats A as a plain number, so a pass proves the dialect, the reached positions and the absence of faults, not the A direction (decision record 0001).
