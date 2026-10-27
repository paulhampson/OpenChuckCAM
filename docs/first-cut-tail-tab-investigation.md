# First-cut far-end tab: underflow-like movement at 100%

The owner reported this during SHORTMOVE implementation on 25 Oct 2026: movement near the far-end tab looked like buffer underflow, at **100% feed override**. Exact NC file, operation, line and location were not available. Track confirmation as **OI-993**. This report is preserved separately from the older #284 trial's pass/waiver record.

## What the committed first-cut program shows

Examined `testdata/golden/first_cut/first_cut_makera-z1.nc` from #284's remote head `9573685001117c4e71a236c555f34138fb21cbad` (blob `2130bf92f782bc7fa79587d41e361ce138d2ca07`). This may differ from the program actually run. Timing uses the Z1 rule, including its A-only radius/perimeter term; small XYZ increments alone do not establish short block times.

The setup has stock X0..100, a head-held region X0..68, a tab X68..73, the part offset73, and the 10mm-diameter tail-held section X98..100. Both committed tools are 3.175mm diameter. The last tool-tip X95.912 leaves its maximal cylindrical envelope at X97.4995, before the held section; the ball's contact envelope depends on Z.

| Region (blocks touching X >=90) | Fed blocks | Blocks below 7 ms | Shortest nominal block | Lowest32-block mean |
| --- | ---: | ---: | ---: | ---: |
| Roughing | 4,058 | 0 | 83.327ms | 83.328ms |
| Finishing | 940 | 0 | 83.322ms | 83.327ms |

Whole program: 16,400 fed blocks; one below 7 ms, at line12957, X71.413 near the head-side region, nominal 2.279 ms. Lowest32-block mean78.122ms. A7ms/32-block throughput mitigation would not change this program.

The end of roughing is especially relevant:

- Line12663: last mixed X/Z/A motion, ending at X95.912.
- Lines12664..12735: **72 A-only blocks of 5 degrees each**, making a full extra turn at fixed XYZ. The nominal ring duration is about 6.606 s under the Z1 rule.
- Line12736: fed retract to Z18.509 at F300 (nominal 2.555 s).
- Lines12737..12741: rapid to Z50, non-moving rotary-counter reset, spindle stop and tool change.

This ring is intentional: `openchuck-path/src/roughing.rs` adds an end turn where a held region truncates a continuous pass (decision0161). It is a geometric/routing fact, not evidence of missing commands.

## A stronger alternative hypothesis: A-only junction stops

Published Makera source at the known 1.1.2 revision `b3a2e26` has the same Robot and Planner file blobs as current main:

- [Robot.cpp:1724](https://github.com/MakeraInc/MakeraZ1Firmware/blob/b3a2e26/src/modules/robot/Robot.cpp#L1724) passes `nullptr` instead of a direction vector to the planner for auxiliary A-only motion.
- [Planner.cpp:172](https://github.com/MakeraInc/MakeraZ1Firmware/blob/b3a2e26/src/modules/robot/Planner.cpp#L172) initializes junction speed to minimum planner speed; its junction-blending calculation runs only with a non-null direction vector. Minimum planner speed defaults to zero.
- The firmware's documented build uses `PAXIS=3`; its upgrade notes warn that A/B/C do not participate in primary-axis junction handling.

**Inference:** the tail ring may stop/start at every5-degree boundary with a full buffer. This would resemble underflow while having a different cause. With the published A acceleration 360 degrees/s², a 5-degree rest-to-rest triangular move takes approximately 236 ms instead of its nominal 92 ms: about 4.2 pulses/s and 17 s for the ring rather than 6.6 s. These are source-based predictions, not measured times or confirmation of the installed binary/configuration.

The block demand here is about 12 commands/s, versus roughly 143/s at the tested 7 ms setting. That makes sustained short-move supply starvation unlikely for this committed file. A delivery interruption, different posted file, firmware mismatch or another mechanical/planner effect remains possible.

The #284 block-rate test exercises X-only motion and mixed X/A motion. Both supply primary-axis vectors, so passing its7ms sections does not validate A-only junction blending.

## Next verification

First establish whether the symptom occurs during roughing's stationary-X tail ring, finishing, or the retract/tool-change transition. Record line/X/A and video/timing if reproduced.

A diagnostic comparison should use identical safe fixed XYZ and total A travel, with the same feed, once as repeated small A-only blocks and once as one monotone A-only block. Pass normal static/replay/simulator gates, then use the existing attended, no-tool/no-material Z1 air-cut procedure. Pulsing only in the segmented version would support the junction-stop hypothesis; it would not by itself validate a general merge implementation.

Do not automatically merge the ring or change firmware under GC-17. Merge remains deferred by 0222. If the hypothesis is confirmed, assess an exact constant-XYZ rotary-span coalescing change separately, with independent swept-path verification and pause/hold behaviour reviewed. Increasing block time alone cannot restore missing junction blending.
