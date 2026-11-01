# 0252: FACING ends, box sides and extents

**Status:** ACCEPTED by the project owner directly in the FACING chat on 26 October 2026 after complete-record presentation: “Accept 0244”. The Merge Manager administratively renumbered this record from 0244 to 0252 on 28 October 2026 ([PR365 comment6463322786](https://github.com/paulhampson/OpenChuckCAM/pull/365#issuecomment-6463322786)); acceptance status and substantive design text are unchanged.

**Issues:** #286, #287, #288. **Branch:** `codex/lane-facing-ends-sides-and-extents`.

**Allocation:** 0252 assigned by the Merge Manager on 28 October 2026. Published GATES owns 0244; AROT owns 0247; main owns 0248–0251; the open-PR audit found no 0252 claim. This administrative renumber replaces FACING's former 0244 allocation without changing its accepted substance. No new OI or project-schema number is claimed.

## Context and integration

OP-24 already covers Facing. Existing Flat and Round modes use longitudinal roughing passes; they do not implement the requested unheld-end operation or explicit box-face controls. This record extends OP-24 without replacing those modes.

CUTOPTS #277, STEPALL-F #274, SWARF3 #320 and ROUGHPATH #340 are merged prerequisites. ROUGHPATH merge `ab863f6531d1e8fab8c2562e88d92b9277937c3f` was verified an ancestor of inspected remote main `89c232227e18251c6c6cb1daa0beb330e1505e79`. Implementation starts from then-current main. Local fetch failure does not authorise using the older checkout as that baseline.

The accepted ownership agreement gives FACING new facing planning, coverage, targets, previews and completion; ROUGHPATH owns existing roughing/layers; FINSTEP owns adaptive finishing. Use merged entry/assembly/connection interfaces. Sequence actual shared proof, parameter, refusal or migration changes with FINSTEP before editing them; there is no blanket wait for its entire feature. Never build on an unmerged prerequisite.

## 1. Modes and compatibility

Retain legacy Flat and Round, default Flat, default angles 0, their existing size meanings and equal-depth layers. Add End opposite held end and Box sides. New round-bar side-facing is deferred; this does not remove existing Round.

Whole stock remains the extent default, margin defaults to zero, extension stays off by default. Saved operations without new keys retain existing behaviour. Opting into restricted extents changes their coverage explicitly.

Legacy default paths and posted bytes must match the merged baseline at the same software version. Accepted ROUGHPATH decision0241 already permits its rotary-boundary correction to affect legacy Facing output; that exception does not permit new cutting/default/layer changes here. Report generator-version comment changes across releases separately.

## 2. Coverage, finished target and protected geometry

Keep three distinct descriptions: the region requested for coverage, its finished surface, and geometry that must remain protected. Increasing coverage never permits cutting a protected solid.

For planar Facing, Whole stock uses the stock's projected bounds. Part uses the smallest rectangle aligned with the facing plane's axes enclosing the projection of the entire placed part. Part plus tabs bounds the union of placed part and all tabs. Held regions do not enlarge that latter extent, but remain protected. Margin m expands all four rectangle edges by m; m must be finite and nonnegative.

An end plane uses Y,Z in the first X-rotary setup. A side plane uses X and the transverse tangent direction derived from shared kinematics. Project actual geometry; projecting the corners of its world-axis bounding box may give the wrong rectangle at arbitrary A. This is not a silhouette, convex hull or section at the target plane. An L-shaped projection spanning20x10mm gives a20x10 rectangle including its empty corner; margin2 gives24x14.

Legacy Flat also uses these planar extent definitions at each selected A. Its existing composite-target behaviour remains: protected model/allowance protrusions can raise the path above its nominal plane. It must identify that behaviour, rather than claim a new exact finished dimension. New End/Box sides promise the requested plane and refuse an inaccessible remainder.

For existing Round, extents select an axial X span around the full circumference: Whole stock is stock length; Part is the placed part's X span; Part plus tabs is the union's X span. Margin expands both axial ends and is labelled Axial margin. No angular patch is introduced. Existing diameter and composite-target behaviour remain.

STL bounds are bounds of the placed triangles. STEP bounds must identify their geometry source and have a conservative projection-error bound compatible with operation tolerance; a tessellation setting alone is not proof. If the required geometry or accuracy cannot be established, explain and refuse the affected mode rather than label approximate bounds exact.

All tabs remain protected under every extent. This resolves #288's conflicting suggestion of cutting tabs in favour of the owner's explicit preservation instruction.

## 3. Cutter overlap and extension

Allow only cutter overlap needed to cover the selected region, and preview the extra swept area. Compute it from the actual tool and path. It is not permission for arbitrary cutting outside the extent.

Existing extension distances remain stock-edge to the cutter's near side, from zero through cutting diameter, on explicitly selected edges. Margin is independent. If requested stock-edge run-out conflicts with restricted coverage, protection, clearance or travel, refuse and identify the conflict; do not silently widen coverage, reinterpret the distance or bypass an obstruction.

## 4. Unheld end and finished length

Exactly one nonzero held end selects the opposite end automatically. Neither held refuses with “Define which end is held before facing the opposite end.” Both held refuses because neither end is available. Re-resolve when workholding changes.

For placed part X bounds[p0,p1] and existing stock-to-leave s, start held gives target X=p1+s; far held gives X=p0-s. Display target X and Length to opposite model end=p1-p0+s, excluding tabs and held stock. Example X10..50,s0.20 gives X50.20 or9.80 and length40.20mm. Apply allowance once. No independent finished-length control is added.

An impossible target, including one outside supplied stock or cutting protected part/allowance, refuses. An already satisfied target may report No material to remove only when known stock establishes it without overcut. Missing stock evidence is not completion.

## 5. End-cutting geometry and limits

End facing uses the straight cylindrical flank of a supported flat end mill, not an assumed axial tool orientation. For outward sign q and cutter radius r, the tangent tool-centre coordinate is X=x_target+q*r. Fixed-A Y travel sweeps its active flute across the face; finite Z bands and additional indexed views cover the requested accessible area. Derive the construction through shared machine coordinates and validate linear Y travel.

Slice axial removal from the stock's unheld end towards the target. The entered stepdown is a maximum axial slice thickness for this mode; refuse a value exceeding cutting diameter or a stricter declared tool limit. Distribute the depth into equal slices not exceeding that maximum, with an exact final target. This end-mode meaning is labelled Axial stepdown. It does not change the Box sides layer option below.

Within each slice, traverse accessible bands from the outside/top towards remaining stock, using only the active flute length and validated engagement. Plan split passes and indexed views around tabs; inspect the continuous complete tool envelope and actual removal. Unsupported tools, inadequate flute/shank/holder access, or a requested face that cannot be completed refuse the entire operation. Do not enlarge a tab or omit an extra halo to disguise missed material.

For a circular cylindrical tab of radius R, N tangent indexed views have geometric radial remainder bounded by R*(sec(pi/N)-1). Allocate this together with clearance, fitting and written-motion errors within tolerance. R5,N71 gives approximately0.004898662081mm before other errors. Four views are not assumed sufficient. Multiple/off-axis tabs still require full clearance and coverage proof.

Important limitation: an outward-widening frustum can block the adjoining face for this construction. With near radius R0, slope k, length L and cutter radius r, an inaccessible radial band can reach at least k*min(2r,L). A tab growing5->7mm over10mm with a3mm cutter gives at least0.6mm. Such a request refuses clearly; default tabs may have this shape. No tab is automatically reshaped or removed. Another tool/construction would require a supported geometric design, not a fallback invented during generation.

## 6. Box sides, arbitrary A and dimensions

Provide four individual face selections plus All four, and arbitrary-A rows. Each face has its own plane offset h from the rotary axis: n.(p-o)=h. Default h is the placed part's outer support in n plus existing stock-to-leave. Preserve a distinction between model-derived defaults and explicit overrides: edits become stored overrides; Reset to part plus allowance restores derivation. Do not add allowance or cutter compensation twice.

Display the separation of opposite selected planes h(n)+h(-n): +Z8 and -Z7 gives15mm. These dimensions describe the faced region; tabs and intentionally unfaced stock may protrude. Offsets must be finite and compatible with stock and protected part/allowance.

For positive-sense X rotation, standard faces +Z,+Y,-Z,-Y correspond to A0,90,180,270. Use shared sense/origin/range conventions for actual machine A. A is fixed during each side's cut. Index at validated clearance using shared rotary/feed handling.

Selected standard faces run in that fixed order, then arbitrary rows in their stored order. Provide keyboard-accessible Move up/down controls and an explicit Sort by A angle button using stable ascending displayed-angle order. Keep each angle attached to its offset/default state. Sorting/reordering is one undoable confirmed edit, never an automatic response to another edit. Reject empty selection and conflicting duplicate geometric orientations. Preserve legacy angle-list ordering.

## 7. Box-side layers

Offer Full stepdown, shorter final pass and Equal stepdowns. The former is the default for new Box sides, including arbitrary A.

Let h0 be initial stock support, h the target and d>0 the selected maximum stepdown. Full-step levels are max(h,h0-k*d), k=1..ceil((h0-h)/d), assigning the final h once. Equal uses N=ceil((h0-h)/d), levels h0-k*(h0-h)/N, with final h exact. Five mm at maximum2 gives2,2,1 by default or three5/3 cuts with Equal.

First and final layers follow those definitions. Validate tool/flute and engagement limits independently. Prior selected faces can empty a nominal layer; skip only with conservative stock evidence. Min-cut must not suppress a required finish. Keep legacy Flat/Round equal-depth arithmetic and defaults unchanged.

## 8. Motion proof and completion

Shared services own entry, linking, retracts, timing, canonical/connected paths and file-context rotary placement. Supply conservative pass-stock snapshots and whole retained targets. Connection bounds include original stock, whole part and all protections, independently of extent. No rapid removes stock.

Check the continuous cutter, flute, shank and any defined holder sweep, every XYZ/A move and feed/travel limit, entries/reversals/final connections, precision and cancellation. Warn-only sampling, clear endpoints and AcceptAll are not clearance evidence. Existing EntrySafety certifies a restricted fixed-A mesh case; it does not certify exact STEP, all rotary motion or newly retained outside-extent stock automatically. Unsupported proof refuses; no accepting fallback or silent plunge replaces an unavailable optional ramp.

FACING's completion analysis compares required coverage with actual achieved coverage after assembly, clipping, raising and entry/link construction. Exclude the protected solid itself, not a concealed extra unreachable band. Check analytic footprint unions and bounded residuals independently of simulation resolution. Any forbidden removal or unfinished required region beyond tolerance refuses and publishes no usable partial operation.

A shared proof/refusal API extension must be coordinated with FINSTEP before editing. Any breaking library API follows0048: correct public API, workspace minor bump and changelog, rather than duplicate compatibility types.

## 9. Reporting and diagrams

Provide a separate Facing completion result. Outside-extent stock is labelled Intentionally unfaced and is not a failure of that operation. Keep whole-part deviation, its worst values and other operations' misses available and numerically counted. Always report overcut, including outside the extent and into protected geometry. Do not globally mask the deviation target.

For legacy modes, measure against their existing composite target and distinguish nominal flat/cylindrical settings from protected protrusions. New End/Box sides are checked against their explicit finished planes. Show simulation cell resolution and unmeasured regions honestly; simulation alone does not certify toolpath tolerance.

One geometry description supplies viewport and Setup diagrams: selected extent and margin, target plane/cylinder and dimensions, protected tabs/held regions, and actual additional cutter sweep. Follow existing overlay conventions. Invalid drafts show their reason; draft previews do not replace valid generated results. Persist order/default state and include mode, geometry/placement, protection, extent/margin, targets, tool and applicable machine settings in result/cache identity.

## 10. Persistence and defaults

Use the existing generic operation parameter map where it represents these semantics. Store explicit mode, extent, margin, new layer choice, standard selections, ordered arbitrary faces and offset overrides. An absent legacy mode/extent/margin retains existing mode, Whole stock and zero. An absent new-side layer choice means Full stepdown, shorter final pass. Preserve explicit values through save/load and undo/redo; reject malformed/nonfinite settings clearly.

No structural schema change or schema number is assumed. If implementation needs one, select and claim it in the work breakdown before use and sequence migrations with FINSTEP. Older generators must clearly reject unsupported settings, not regenerate a different operation silently.

## 11. Required specification changes

Preserve requirement IDs and existing OP-24 extension clauses. Append this normative Facing extension:

> Facing offers the end opposite exactly one defined held end, preserving tabs and refusing neither-held or both-held setups. The end target is the placed part's unheld end displaced outward by existing stock-to-leave; target coordinate and length to the opposite model end are displayed. Indexed box-side facing offers individual faces, All four and ordered arbitrary A angles, editable per-face target offsets and opposite-plane dimensions. It offers full stepdowns with a shortened final pass by default, or equal stepdowns within the selected maximum. Coverage is Whole stock by default, the projected part bounding rectangle, or the part-and-tabs union's projected bounding rectangle, with optional nonnegative margin defaulting to zero. Existing Round uses the corresponding full-circumference axial span and axial margin. Tabs and held regions remain protected under every extent. Necessary cutter overlap is previewed. Requested faces that cannot be reached completely with supported tool geometry, clearance and machine limits refuse without a usable partial path. Viewport and Setup show consistent extents, margin, targets, dimensions and cutter overlap. Saved default Facing behaviour is preserved relative to the merged rotary-boundary baseline.

Extend SIM-05 and the UI specification with the separately labelled operation-completion result and unchanged global deviation reporting described above. Amend architecture/work breakdown for FACING-local planning/completion and shared proof integration. Primary mapping: OP-24, OP-11/12/21, SET-06/09, GC-07 and NFR-05/06/07/12; supporting OP-14/15, SIM-05/06 and UI-03/07/25.

## 12. Verification and delivery

Write requirement-based tests first and observe failure before fixes. Independently verify either held end, both/neither, cylindrical/frustum and off-axis tabs, unreachable adjoining faces; all15 nonempty standard-side combinations and arbitrary angles under both A senses; order/movement/sort persistence; both layer choices, partial final depth and dimensions; all planar/round extents and margins; true projections rather than projected AABB corners; complete tool/flute/holder and machine limits; entry/link/retract/indexing, precision, cancellation and invalid input; clipped/raised paths and missed edges; intentional stock without hidden overcuts/other misses; viewport/Setup agreement; migration/defaults, same-version legacy identity and deterministic paths/both posts at1/2/available-N threads.

Use analytic geometry and independent swept/coverage checks in addition to simulation. Include narrow obstacles between samples and the frustum refusal witness. Do not calculate expected answers with generator helpers. Run relevant posted/replay verification at existing tolerances.

Measure representative generation costs uninstrumented, recording hardware, threads and output sizes. Classify tests over about a minute in required nextest profiles; stop and raise any individual uninstrumented test over10 minutes. Run changed-crate own-test coverage, inspect/report uncovered changed lines and below95% results honestly, then clean instrumentation. Complete formatting, applicable lint, xtask/records/trace and relevant G-code gates. No test result is claimed by this design record.

Update specifications, work-breakdown status, accepted/proposed records, registers and changelogs in the implementation PR. Use human author/committer and agent coauthor per AGENTS. After design acceptance, implementation and required local checks, push once and open a DRAFT FACING PR referencing all three issues, attach it, and report branch/head/dependencies, modes/semantics, decisions/numbers, tests/timings/coverage/output changes/limitations. Merge Manager controls readiness, release, main updates and merge through PR comments under0055; the lane does not mark ready, merge or enable auto-merge.

## 13. Verification results (FACING, #286/#287/#288, PR #365)

Verification notes moved from `docs/handoffs/facing-progress.md` on 1 Nov 2026 (DOCS-TIDY); the handoff's description of implemented behaviour repeats sections 1 to 12 above and is not kept. Branch `codex/lane-facing-ends-sides-and-extents`, started from main `e2189f07fe8dee5a1bd15300babed08f89e6e853` with CUTOPTS #277 (`ba829c1`), STEPALL-F #274 (`f9474af`), SWARF3 #320 (`e590f4d`) and ROUGHPATH #340 (`ab863f6`) merged. Released integration of main `b2278b89c252d2d2c3b67ea2e6ed2368c9d4ecba` on 28 Oct 2026 (original head `6448dc44db406e285472075690ebf52230d486e6`); library **0.25.0** (0.21 to 0.22 in the first draft, all four semver checks pass against `b2278b89`), schemas 20/6 unchanged, no OI or schema claimed. The record was accepted as 0244 and renumbered 0252 by Merge Manager (sections 1 onward byte-identical). Requirements: OP-24, GC-08, GC-17, SIM-05, with GC-07/NFR-06 determinism and NFR-07 refusals.

**Tests written first and watched failing:** missing APIs and disabled public modes, missed final coverage, deeper cutter overlap without prior removal, stepdown relaxed by finishing tolerance, tab work-budget and posted-residual failures, missing rotating sweep preview, active-schema edit constraints, footer XY home travel, extra outside-extent entry cutting, overly conservative round-stock indexing; after the #324 integration the fractional-A and all-mode thread tests failed (0.020 and 0.224 s) until Facing's word checker rounded A before computing expected XYZ. Independent analytic geometry, mutated actual paths and both real posts supplement the simulator checks. All 15 non-empty side combinations, both A senses, both layer choices and both posts (120 posting runs), ordered fractional and arbitrary angles, End with both held directions and cylindrical tabs of radius 1 and 5, and a GC-17 Slow down case in all four modes and both posts (unchanged canonical geometry, longer delivered duration) are exercised. Canonical paths and both posts match at 1/2/available-N threads for End, Flat, Round and both Box layer choices.

**Legacy identity.** Whole-stock, zero-margin legacy Flat and Round keep their original branch; the pre-scope fingerprints were Flat `2ed1e05d2c113015` / 346,775 bytes and Round `9a9c29f3ec238440` / 184,577 bytes. CI on Windows and macOS showed that a raw-IR hash is not portable (Round: 184,577 bytes / `9a9c29f3ec238440` Linux, 184,561 / `ba3bd5f98b8a414c` macOS, 184,558 / `87f565a02430a68a` Windows; Flat identical everywhere), because native transcendental functions differ (decision 0113; verification policy 7 asks for exact raw toolpaths across runs and threads on one platform and shared G-code goldens across platforms). The oracle is now a frozen test-only copy of the pre-scope Facing schema, dispatch and face entry (in `roughing::legacy_identity_tests`) compared byte for byte (IR, notes, warnings) at 1/2/4 threads against the native one-thread result; a temporary 0.01 mm finished-size mutation failed at the first differing byte (0.684 s). CI at `49c3a4d` passed the complete and end-to-end workflows on all three platforms. No platform G-code divergence was established.

**Owner-authorised review batch (28 Oct, "Fix all in one push"):** verification geometry is built only when `facing::needs_verification(operation)` (whole-stock, zero-margin legacy operations unchanged, compared with the pre-scope route; failed first at 3.750 s); imported stock is refused by SET-04 and by a direct generation fallback (no imported-stock support claimed); cancellation at the post-sweep completion boundary returned Strategy instead of Cancelled (0.005 s, fixed in `flat_completion`); replacing a scene job now ORs the unconsumed camera-fit request (failed at 0.168 s); the Facing description and Shape documentation name End/Box, indexed and arbitrary A and all coverage choices. Focused regressions 6/6 (5.845 s); selected tests 1486/1486 (491.899 s; app 543, path 443, UI 500; 44 long exclusions), longest 37.878 s.

**Results (Linux x86_64, AMD EPYC 9V74, four-CPU quota, uninstrumented):**

| Check | Result | Seconds |
| --- | --- | ---: |
| Path and G-code broad run before the final guards | 447 passed, 13 long skipped | 194.928 |
| Final nominal End/Box/motion focused run | 28 passed | 0.830 |
| Final extent guard after lint refactor | 16 passed | 0.861 |
| Application broad run | 519 passed, 21 long skipped | 445.322 |
| UI broad run | 485 passed, 2 long skipped | 260.660 |
| Simulator own regression run | 50 passed, 6 long skipped | 142.186 |
| G-code observer and timing tests | 22 passed | 0.276 |
| Expanded all-mode thread determinism (30 regeneration and post runs) | 1 passed | 18.828 |
| Released integration, seven crates | 1675/1675 passed (51 profile exclusions) | 567.153 |
| Released integration, cylindrical-tab case alone | 1 passed | 20.035 |

Five application cases exceeded a minute under contention (75.152, 61.950, 79.254, 71.006, 64.086 s) and one simulator envelope case took 75.124 s; all are registered in both nextest profiles; the cylindrical-tab case takes 22.985 s alone; no test exceeded ten minutes. Representative job costs (isolated, LinuxCNC first so the Z1 run may reuse canonical caches; not a post comparison): Box 22.6 / 18.7 ms (143 moves), End 35.5 / 32.0 ms (195), Flat 51.5 / 23.9 ms (183), Round 470.2 / 313.2 ms (2,605). Workspace clippy (warnings denied), formatting, doctests, `cargo xtask all` (99/99, later 100/100 P1), `cargo xtask golden`, cargo-deny, cargo-audit (449 dependencies, 1,378 advisories) and cargo-about pass; the fuzz workspace compiles with the 0.25 family.

**Independent LinuxCNC 2.9.10 replay** (image `ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10`, digest `sha256:22d8b9f0acb8a0fd7a02bf47be7fd9e864e671bd510ca27ab35ff214d1489546`; gate tolerance 0.1 unchanged; no controller fault):

| Program | Reached moves | Largest difference | Motion seconds |
| --- | ---: | ---: | ---: |
| Box | 144 | 0.0013 | 45.0 |
| End | 196 | 0.0013 | 60.9 |
| Flat | 170 | 0.0482 | 14.9 |
| Round | 2,606 | 0.1000 | 218.5 |
| Existing small-part reference | 29 | 0.0010 | 26.7 |

The first attempt failed before producing reports (image user `cnc` is UID 1001, the fixtures were readable only by UID 1000); making the public test fixtures readable (no content or Git-mode change) allowed the complete rerun, which is the only one counted. To reproduce: keep the programs from `req_op_24_req_gc_08_controller_programs_cover_all_modes_with_bounded_run_times` with `OPENCHUCK_FACING_PROGRAMS=/tmp/facing-programs`, then `cargo xtask replay --docker ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 --timeout 540 --keep-reports /tmp/facing-controller-reports` on those four LinuxCNC programs and `testdata/golden/post_linuxcnc_small_part.ngc`, with the machine otherwise idle.

**Own-crate line coverage at the first draft (lower bounds, report-only under 0102):** path 16,611/18,137 (91.586%), app 18,561/20,247 (91.673%), G-code 928/979 (94.791%), UI 26,187/29,923 (87.515%), simulator 1,558/1,722 (90.476%). Path and simulator text summaries warned of mismatched LLVM data (313 and 21 functions) despite a clean rebuild, so no 95% claim is made. Changed files: `facing/written.rs` 88.6% (421/469 lines in the integration run; the two new rounded-A lines are hit 85,832 times), display 90.476%, strategy 93.158%, new UI row controls 92.5%, End multi-slice verification 57.931% in the path run and 83.448% through the app run. Gaps are unsupported format and controller, budget and precision refusals, malformed metadata, footer and controller cases, display failure branches and excluded long fixtures.

**Not verified and supported-proof limits:** only exact placed triangles are supported; unknown STEP projection accuracy and unproved source error refuse; the supported flat cylindrical tool without a wider shank is required for the new final proofs; End routing supports the demonstrated cylindrical-tab construction; new End/Box stock-edge extension refuses when enabled; whole-stock Box coverage intersecting held geometry refuses (never cropped silently); round-bar new side scope is deferred; travel, feed, precision and work-budget limits can refuse otherwise geometric requests. The optional STEP-kernel feature suites, the pre-existing nightly long cases, hands-on graphics and machine verification were not run locally. Generated G-code remains the user's to verify before running on a machine (NFR-12).

**Shared hooks delivered (merge order agreed with the owner: FACING first, FINSTEP consumes them from main):** `Strategy::schema_for`, `resolve_parameters` and `validate_path` (provided methods; final validation runs after assembly on plain, region and zoned generation, errors and cancellation return no usable path) and `StrategyInput::facing: Option<&FacingSetup>`. No project-format migration: new settings use the generic parameter map.
