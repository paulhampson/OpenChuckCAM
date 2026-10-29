# 0256: No fixed finer toolpath mesh qualifies; the default stays (lane STEPALL-T; issue #347)

- **Status:** proposed (29 Oct 2026, Europe/London). The disposition (option A) was chosen by the project owner on 28 Oct 2026 and relayed by the coordinator; this record is guidance until the owner accepts it directly in the lane session (decision 0049).
- **Date:** 28 to 29 Oct 2026 (Europe/London).
- **Requirements:** IMP-02, IMP-06, OP-03 to OP-06, OP-09, OP-12, OP-23, NFR-04, NFR-05, NFR-06, NFR-12, GC-07. Verification plan 4.1 (the 0.005 mm tessellation allocation; the job and per-operation 2-times limits).
- **Builds on:** [decision 0211](0211-step-accuracy-measured.md) (accepted: the owner's choice of a fixed finer toolpath mesh as a direction to verify, OI-834; the job and per-operation limits, OI-835); [decision 0255](0255-bounded-tool-contact-checker.md) (accepted 28 Oct 2026: the bounded checker, issue #346, merged in PR #374).
- **Open items:** OI-834 and OI-835 stay open with this evidence; OI-831 stays open (a fixed internal setting would not give IMP-06's user control in any case); OI-81 stays STEPALL-X's; OI-833 (the face-region test) and OI-839 (`ExactFace::closest` at a face's boundary) are untouched, since nothing here exercises them. OI-990 (#344) is separate and unchanged.

Numbering: the lane's allocation is 0255 to 0259 and OI-1020 to OI-1039. On 29 Oct 2026 0256 was unused on `main` and on every fetched branch (0255 is this lane's, merged in PR #374); no new open item is needed, because the blocker belongs to OI-834 and OI-835, which stay open. No project schema number, no library version change: nothing in the product changes.

## Context

Decision 0211 measured the original candidate, 0.01 mm / 0.02 rad, at 0.00570 mm against the 0.005 mm allocation (uncapped M1), and its whole-job generation at 1.78 times the import mesh but rest machining at 2.007 times, over the per-operation limit. A tighter 0.001 mm / 0.01 rad investigation met the allocation at underlying-surface stations but had no trim, toolpath or performance qualification. Issue #347 asks to qualify a fixed finer toolpath mesh with the bounded checker of #346, on the full carried-stock sequence and the corpus, against both 2-times limits, and to adopt it only if every criterion passes; otherwise to leave the default unchanged and report the blocker.

## Disposition

**Fail. No fixed finer toolpath mesh is adopted; the import's tessellation (0.01 mm, 0.1 rad) stays the toolpath mesh.** The owner chose this disposition (option A) on 28 Oct 2026, relayed by the coordinator, after the blocker below was reported, and asked that the five-run measurement of the best candidate be completed and recorded whatever its outcome. Accuracy on STEP parts is left to STEPALL-X's exact drops (OI-81).

The blocker that no fixed setting clears:

1. **Accuracy needs an angular deflection finer than 0.02 rad.** With the bounded checker the tessellation's proven share of the cut (the drop error at contact stations, from both intervals) is set by the angular deflection on the blisk: up to 0.0058 mm at 0.02 rad (full stepovers, decision 0255), 0.0039 mm at 0.015 rad and 0.0036 mm at 0.01 rad (quick stepovers), whatever the linear deflection.
2. **Time follows the triangle count.** The fewest triangles that meet the allocation, 149,642, come from 0.0025 mm / 0.015 rad; 0.01 rad gives 317,658 or more, and its one-run generation is 2.22 times the import mesh for the job and 2.80 times for rest machining.
3. **The corpus torus cannot be imported finer than about 0.03 rad.** `testdata/step/valid/torus.step` takes 6 s at 0.05 rad (127,008 triangles) and 77 s at 0.03 rad (351,122 triangles), and the STEP worker stops it at its 120 s limit at 0.025, 0.02 and 0.015 rad. A fixed 0.015 rad toolpath mesh would make a part that imports today fail. A fallback to the import mesh, or an angular deflection that depends on face size, were offered and not chosen.

## Evidence

All measurements on this lane's machine: Intel Xeon @ 2.80 GHz, 4 cores, 15 GiB, cloud container; release build of the `step_accuracy` example at source `0560e4f` (the M2 drop-error bound of decision 0255; no product source differs from `main` 33a835d); the bounded checker of decision 0255 for accuracy; `testdata/step/valid/blisk.step` as in 0211. Commands, hashes and complete output, including every timed run, are in [the evidence](0256-toolpath-mesh-evidence.txt).

### Candidate screening (bounded M2, quick stepovers, form c only)

| Linear / angular | Triangles | Proven drop error, worst operation | Deepest cut found (proven), worst operation |
| --- | ---: | ---: | --- |
| 0.001 mm / 0.02 rad | 161,266 | **0.0060** | 0.0067 (0.0071), pencil |
| 0.0025 mm / 0.015 rad | 149,642 | 0.0039 | 0.0068 (0.0073), pencil |
| 0.005 mm / 0.015 rad | 152,570 | 0.0040 | 0.0067 (0.0072), pencil |
| 0.01 mm / 0.015 rad | 256,474 | 0.0039 | 0.0068 (0.0073), pencil |
| 0.001 mm / 0.01 rad | 371,688 | 0.0035 | **0.0100 (0.0105)**, pencil |
| 0.0025 mm / 0.01 rad | 317,658 | 0.0036 | 0.0066 (0.0071), pencil |
| 0.005 mm / 0.01 rad | 448,676 | 0.0036 | 0.0066 (0.0071), pencil |
| 0.01 mm / 0.01 rad | 597,424 | 0.0036 | 0.0066 (0.0071), pencil |

For comparison, the import mesh (0.01 mm / 0.1 rad) has 15,936 triangles and the original 0.01 mm / 0.02 rad candidate 106,928, with a proven drop error of 0.0058 mm at full stepovers (decision 0255). The kernel's triangle count is not monotone in the linear deflection at a fixed angle.

At quick stepovers the deepest cut found is 0.0052 to 0.0068 mm at every setting (the fitter's quarter stations add to the drop); at 0.001 mm / 0.01 rad pencil's quarter stations cut 0.0100 mm (proven at most 0.0105 mm), over the whole tolerance. Quick stepovers are a screen, not the full-stepover qualification.

### One-run timing screens (all threads)

Generation, candidate over import mesh (form c over form b), one run each:

| Operation | 0.0025 mm / 0.01 rad | 0.0025 mm / 0.015 rad |
| --- | ---: | ---: |
| roughing | 1.48 | 1.17 |
| rest | **2.80** | 1.84 |
| pencil | 1.54 | 1.77 |
| helical | **2.49** | 1.99 |
| axial | **2.42** | 1.95 |
| indexed | **2.67** | 1.62 |
| six-operation job | **2.22** | 1.85 |

The 0.01 rad setting fails on one run; 0.0025 mm / 0.015 rad, the setting with the fewest triangles that meets the allocation, was near the limit and was measured by the approved method below.

### Five-run qualification of 0.0025 mm / 0.015 rad (the best candidate)

Medians of five successful runs, each form and thread count, the six successful operations in the original stock-carrying order, full stepovers; simultaneous finishing's known refusal is excluded explicitly, as in decision 0211. All 60 timed operation runs succeeded. Both forms of each thread count ran back to back on one host (Intel Xeon @ 2.80 GHz, 4 cores; boot `b66cd70e`), with the harness's untimed all-thread warm-up before each batch and nothing else running. Two earlier attempts were cut short by container restarts, one of them on a different host (2.10 GHz); their samples are archived in the evidence and not combined with these.

Generation, seconds:

| Operation | Import, all threads | Candidate, all threads | Ratio | Import, 1 thread | Candidate, 1 thread | Ratio | Speed-up import | Speed-up candidate |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| roughing | 1.190368 | 1.454097 | 1.221552 | 1.555053 | 2.872788 | 1.847389 | 1.31 | 1.98 |
| rest | 5.855370 | 10.148029 | 1.733115 | 9.923945 | 23.321074 | 2.349980 | 1.69 | 2.30 |
| pencil | 5.640576 | 8.603245 | 1.525242 | 14.575933 | 21.560933 | 1.479215 | 2.58 | 2.51 |
| helical | 2.432483 | 5.730085 | **2.355653** | 8.887240 | 22.090090 | 2.485596 | 3.65 | 3.86 |
| axial | 9.668202 | 19.391030 | **2.005650** | 34.278062 | 73.524992 | 2.144958 | 3.55 | 3.79 |
| indexed | 0.040388 | 0.074112 | 1.835000 | 0.088556 | 0.227795 | 2.572327 | 2.19 | 3.07 |
| job (median of run totals) | 24.161329 | 45.873368 | 1.898611 | 69.139273 | 143.659696 | 2.077815 | 2.86 | 3.13 |

Job elapsed time including simulation (medians): 372.67 s and 401.43 s at all threads, 1318.18 s and 1390.94 s at one thread.

**Against the accepted criterion (all threads): the job passes at 1.90 times, but helical finishing (2.36 times) and axial finishing (2.006 times) fail the per-operation limit.** No waiver, exemption or further sampling was applied. So the best candidate fails on time as well as on the corpus import; the one-thread ratios are reported for completeness.

### Torus import

`testdata/step/valid/torus.step`, through the STEP worker (120 s limit), whole M1 run times:

| Angular | Linear | Result |
| --- | --- | --- |
| 0.05 rad | 0.01 or 0.0025 mm | 127,008 triangles, import 5.7 to 5.9 s |
| 0.03 rad | 0.01 or 0.0025 mm | 351,122 triangles, import 77 to 82 s |
| 0.025 rad | 0.01 or 0.0025 mm | stopped at the worker's 120 s limit |
| 0.02 rad | 0.01 mm | stopped at the limit (decision 0255's corpus run) |
| 0.015 rad | 0.0025 mm | stopped at the limit |

## What this does not establish

- Full-stepover bounded M2, full-stepover determinism at 1, 2 and all threads, application and library equivalence and replay through both posts were not run for the candidate: with the import blocker the candidate cannot be adopted, so the product never uses it and there is no application path to check. The existing replay correction of #324 / decision 0234 is unchanged by this record; it was not rechecked because no product source changes.
- The screens use quick stepovers and one run; only the five-run table is the approved timing method, and it covers the blisk only.
- The torus limit is the worker's 120 s; a longer limit was not tried, and the torus's triangle count grows steeply as the angle tightens.

## Alternatives considered

- **(B) The candidate with a fallback** to the import mesh where the finer one cannot be built in time, noted in the part's notes. Not chosen: the fallback parts keep today's accuracy, the criteria have no such carve-out, and it adds a second import and a notice path.
- **(C) An angular deflection that depends on face size.** Not chosen: it needs design and investigation, and delays #347.
- **The 0.01 mm / 0.02 rad candidate of 0211.** Fails the allocation (0.0058 mm proven at the tool).
- **0.01 rad settings.** Meet the allocation but fail both time limits on one run (job 2.22 times, rest 2.80 times).

## Consequences

- **Code:** none in the product. The measurements use the `step_accuracy` example of decisions 0211 and 0255 unchanged.
- **Documents:** this record and its evidence; the decision index; `docs/open-items.md` (OI-834 and OI-835 noted, not closed); the work breakdown's STEPALL row.
