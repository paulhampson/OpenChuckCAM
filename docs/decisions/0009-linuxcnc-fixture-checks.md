# 0009: LinuxCNC checks of the SP-3 and SP-5 fixtures

- **Status:** accepted by the project owner on 11 Oct 2026, including the two fixture decisions (items 2 and 3 below). The owner also accepted decision 0001 on the strength of these results.
- **Date:** 11 Oct 2026.
- **Spike branch:** `spike/linuxcnc-fixture-checks` (throwaway, never merged; write-up in `spikes/linuxcnc-checks/RESULT.md`, raw output in `spikes/linuxcnc-checks/out/`, reproduced by `run_checks.sh`).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-01 and OI-31 close here; OI-46 and OI-47 are new.

## Context

Two checks were waiting for a LinuxCNC 2.9 run, which neither earlier session could install:

- **OI-01** (decision 0001 item 1): SP-3's pass criterion, that the wrapped path "replays in the LinuxCNC simulator", was not met. SP-3's `run_linuxcnc.sh` was written but never run.
- **OI-31** (decision 0005 item 2): the SP-5 hand-written file `linuxcnc_post_subset.ngc` was checked by hand only and must read in LinuxCNC's own interpreter before it becomes a `testdata/` fixture.

Requirements and sections: GC-07, GC-09, GC-10, OP-13, NFR-05, NFR-12; verification plan section 4.1 (limits), section 6 (G-code gates 2 and 3) and the SP-3 pass criteria in the work breakdown.

## Method

LinuxCNC 2.9.10 (branch `2.9`) was built from source in the session (run-in-place, uspace realtime, documentation off). Two tools were used:

1. **`rs274`**, the standalone interpreter, on each fixture; the dialect file also one statement at a time, so an error in one statement does not hide the rest.
2. **The simulator, headless**: a trivkins XYZA configuration (mm, exact stop `G61`, A limited to 360 deg/s as in Makera's Z1 machine file), a display program that runs the file and samples the *commanded* position at about 1 kHz, and a comparison with the file and with SP-3's `cut_time_ir` (38.002 s, from running the SP-3 spike).

## Results

### SP-3 files in the simulator (OI-01)

| File | Cut time, first to last cut line | IR cut time | Difference |
| --- | --- | --- | --- |
| `sp3_g94.ngc` (compensated) | 37.859 s | 38.002 s | -0.37 % |
| `sp3_g93.ngc` (inverse time) | 37.860 s | 38.002 s | -0.37 % |
| `sp3_naive.ngc` (raw contact feed) | 24.042 s | 38.002 s | -36.74 % (SP-3 predicted -36.85 %) |

- All three read in `rs274` without error and run to completion with no limit fault. Each ends at X30 Y2 Z-10 A-90, as the file says.
- Node positions agree with the file to within one sample step: 10 of 1,244 node checks fall outside the window between the last sample of one line and the first of the next, by at most 0.0026 (mm or degrees) for the two compensated files. The naive file has 44, by at most 0.34, because its A axis moves at up to 353 deg/s, where one 1 ms sample is 0.35 degrees.
- LinuxCNC accepts `G93` and takes 1/F minutes per move, and applies a compensated G94 F to the XYZ length on mixed moves even when F is as low as 2.4 mm/min near the sine peaks. Decision 0001 doubted that rule ("fragile"); in LinuxCNC it holds. The Z1 applies feed differently (decision 0007), so this says nothing about the Z1.
- The simulator cannot show the A *direction*: trivkins treats A as a plain number (decision 0001). OI-03 and the SP-12 air cut cover that.

### SP-5 fixtures in `rs274` (OI-31)

- **`linuxcnc_post_subset.ngc` is rejected as written.** At N160 LinuxCNC reports "Radius to end of arc differs from radius to start" (r1 = 5.0, r2 = 14.76), and the other three arcs (N170, N180, N190) have the same defect: their end points are not on the circle the centre and start point define. The SP-5 reader reads the file because it checks syntax, not geometry, so its tests passed. Only the four arc lines need changing; with them corrected (`spikes/linuxcnc-checks/fixtures/linuxcnc_post_subset_corrected.ngc`) the whole file reads without error, including N numbers, `%`, both comment forms, `(MSG,...)`, A alone, A down to -12345.6789, G93 and G94, G43 H, M0 to M9 and M30. `T2 M6` needs tool 2 in the tool table, which is a machine-configuration matter.
- **`linuxcnc_dialect_extra.ngc` fails as a whole file**, because its `o100 sub` has no `endsub`. Statement by statement, LinuxCNC evaluates `#` parameters and `[...]` expressions, reads lowercase and spaces inside words (`G 1 X 1 . 5 A 2 0` is `G1 X1.5 A20`), executes a `/` line when block delete is off, and accepts `G38.2`, `G33.1` (spindle running), `G10 L2` and `M30`. This matches the SP-5 reader's behaviour: it reads lowercase, spacing and block delete, and refuses `#`, `[...]` and O-words with an error that names the feature.

## Decision

1. **SP-3's simulator criterion is met** for the LinuxCNC rule: reached positions match the file, the run completes, and the compensated and inverse-time files run to the IR's cut time within 0.4 %. OI-01 closes. The record 0001 criteria that need the A direction stay with OI-03.
2. **The corrected arc lines are the ones that go into the fixture.** When slice 6 adds the post-subset file to `testdata/`, it takes `linuxcnc_post_subset_corrected.ngc`, not the SP-5 original (OI-46).
3. **Fixtures that stand for runnable programs must read in `rs274`**, not only in the reader. The dialect file is an exception by purpose (it holds features the reader refuses); it stays a reader fixture and is not expected to run as a whole. This is a fixture rule for slice 6 to apply; no document changes until then.

## Alternatives considered

- **Keep the SP-5 original and note the arcs.** Rejected: a fixture that LinuxCNC refuses cannot also serve gate 3 or as a seed for runnable files.
- **Treat the reader's acceptance as enough.** Rejected by the four arcs: syntax checking cannot see inconsistent geometry, and GC-08 (P2) is where a geometry check on exported G-code would belong.

## Consequences

- Slice 6 design refinement (gate 3's "xtask replay runner", verification plan section 6): the working method is a headless run of a trivkins XYZA configuration with exact stop, sampling the *commanded* position (the actual position lags it by a few servo cycles, which shows as false node errors at high A speed), comparing the elapsed time from the first to the last cut line with the IR. Three practical points for the runner: LinuxCNC's `configure` needs the Python that has `tkinter`; run as root, `rtapi_app` needs an unprivileged `RTAPI_UID`, a socket *file* for `RTAPI_FIFO_PATH`, and a build directory readable by that user and by group root; Ubuntu 24.04's package index in the session has no `linuxcnc` package (OI-47).
- Decision 0001 is accepted and decision 0005's status line is updated (both on the owner's instruction, 11 Oct 2026); their open items point here through the register. Decision 0001's remaining open items stay open.
- No change to `requirements.md` or `architecture.md`. No dependency enters the product.

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Apply the corrected arc lines when the post-subset file is added to `testdata/`, and add the rule that runnable fixtures read in `rs274`. (OI-46) | Slice 6 | |
| 2 | Where gate 3's runner gets LinuxCNC on CI and on a workstation: a cached source build (done in the session), the project's own package, or a container. (OI-47) | Slice 6 | A look at CI runners and the `linuxcnc.org` package route |
