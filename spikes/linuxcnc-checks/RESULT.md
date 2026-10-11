# LinuxCNC checks of the SP-3 and SP-5 fixtures

Branch `spike/linuxcnc-fixture-checks`. Throwaway code in `spikes/linuxcnc-checks/`; not for merging into the product.
Settles OI-01 and OI-31 (docs branch `docs/linuxcnc-checks-oi-01-oi-31`, decision record 0009).

LinuxCNC 2.9.10 (tag `2.9`, built from source in the session; `rs274` and the headless trivkins XYZA simulator). Reproduce with `run_checks.sh`.

## Interpreter pass (`rs274`), `out/interpreter_results.txt`

| File | Result |
| --- | --- |
| `sp3_g94.ngc`, `sp3_g93.ngc`, `sp3_naive.ngc` | Read without error (G93 accepted; its F is converted per move). |
| `linuxcnc_post_subset.ngc` (SP-5, as written) | **Rejected at N160**: "Radius to end of arc differs from radius to start" (r1 = 5.0, r2 = 14.76). The three later arcs (N170, N180, N190) have the same defect. |
| `linuxcnc_post_subset_corrected.ngc` | Reads without error. Only the four arc lines changed (end points moved onto the circle). Everything else in the file, including N, `%`, both comment forms, `(MSG,...)`, A alone, negative and unwrapped A (-12345.6789), G93/G94, G43 H, M0 to M9 and M30, is accepted as written. |

`T2 M6` needs tool 2 in the tool table; the spike's table has tools 1 to 9. That is a property of the machine configuration, not of the file.

## Dialect file, one statement at a time

| Statement | LinuxCNC 2.9.10 |
| --- | --- |
| `#1 = 5.0`, `#<depth> = -1.5`, `G1 X#1 Y[2 * 3] Z#<depth> F100` | Evaluated (the reader refuses these and names the feature, which is the intended difference). |
| `g1 x1.0 y2.0 a3.0` | Read as `G1 X1 Y2 A3` (lowercase is legal). |
| `G 1 X 1 . 5 A 2 0` | Read as `G1 X1.5 A20` (spaces inside words are legal). |
| `/ G0 X99` | Executed: with the block-delete switch off the line runs. |
| `o100 sub` alone | Error: the subroutine is not closed before the end of the file. This is why the file as a whole fails: its `o100 sub` has no `endsub`. |
| `G38.2`, `G33.1` (spindle running), `G10 L2`, `M30` | Accepted. |

## Simulator, SP-3 files, `out/simulator_results.txt`

Exact stop (G61), commanded position sampled at about 1 kHz, A limited to 360 deg/s as in Makera's Z1 file.

| File | Cut time, first to last cut line | IR cut time | Difference |
| --- | --- | --- | --- |
| `sp3_g94.ngc` (compensated) | 37.859 s | 38.002 s | -0.37 % |
| `sp3_g93.ngc` (inverse time) | 37.860 s | 38.002 s | -0.37 % |
| `sp3_naive.ngc` (raw contact feed) | 24.042 s | 38.002 s | -36.74 % (SP-3 predicted -36.85 %) |

Every file ends at X30 Y2 Z-10 A-90, as the file says, and no run reports an error. Node positions agree with the file to within one sample step: 10 of 1,244 node checks fall outside the window between the last sample of a line and the first of the next, by at most 0.0026 (mm or degrees) for the two compensated files. The naive file has 44, by at most 0.34, because its A axis moves at up to 353 deg/s and one 1 ms sample is then 0.35 degrees. The plunge and retract (6.2 s for the three G1 Z moves at F100) are outside the cut time.

## Limits

- trivkins treats A as a plain number, so the simulator confirms dialect, reached values and timing, **not** the A direction (decision 0001). Only the oracle and the SP-12 air cut do that.
- LinuxCNC's feed rule is not the Z1's (decision 0007): this run says nothing about the Z1's timing.
- Timing is measured from the sampled commanded position; the sampling step (1 ms) bounds the node positions above.
