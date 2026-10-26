# AROT pass-boundary counter resets: implementation and validation

Issue #337; accepted decision [0247](decisions/0247-long-operation-rotary-accumulation.md).

The owner accepted 0247 directly in this lane on 26 October 2026. Automatic Z1 counter resets occur at existing clear transitions between complete helical passes. They add no physical rewind, retract, re-entry or other motion. A single uninterrupted helix is deferred (OI-995); limited-axis rewind/splitting is outside this task. Generated G-code remains the user's to verify before running on a machine (NFR-12).

## Implementation and host contract

- `Pass::helical_pass_start` identifies a new complete helical traversal. Rotary roughing marks only the first fragment of a new layer. Held-region clipping propagates that identity to the first retained fragment, without marking other fragments as new helices.
- Shared assembly preserves all physical segments and marks an existing clear retract before the next complete helix. Existing local-stock clearance can qualify without changing it to global safe Z. Missing/nonretract boundaries refuse rather than add motion or reset in-cut.
- `ToolpathGroup::rotary_reset_after` stores sorted, unique segment indices. A supporting post may reset after those existing segments. Validation rejects out-of-range, final, cutting, rotary-moving or unordered markers. Empty metadata defaults on load and is omitted when serialized.
- `connect_group` shifts marker indices when prepending a connection and preserves canonical cached groups. File splitting must start from canonical groups. Simulation/playback still consume only physical segments; their motion and duration are unchanged.
- `openchuck-post` emits its existing nonmoving `RotaryBoundary` action on continuous axes at declared clear transitions. Existing renderer logic handles signed remainders, exact ±360 and all subsequent absolute A words. A rotate-only post omits bookkeeping rather than inserting a physical return. Limited axes do not receive resets.
- Independent G-code comparison tracks declared reached pass boundaries, including retracts rounded to no emitted move. It rejects undeclared resets and whole-turn errors after a valid reset.

The new public fields break struct-literal APIs. All repository hosts/fixtures have been migrated. No project or machine-profile field is added; no schema is claimed. The required shared-library minor version **has not been selected**: the owner's instruction requires Merge Manager release and version/merge-order allocation first. Changelogs carry an Unreleased migration entry. No semver exemption or parallel compatibility API is introduced.

## Requirement-first evidence

Before production changes, the new long-operation test failed because the post reset only at operation end; the explicit-boundary roundtrip test failed because the IR could not carry the metadata. Initial uninstrumented `cargo test` executions reported 0.13 s for the first regression and 0.51 s for the four-test run (two intended failures after correcting version masking in the unaffected golden comparison). Nextest was initially absent and installed. After posting gained boundary actions, the independent replay tests failed under nextest because replay permitted only operation-boundary resets (0.010/0.086 s). A dedicated signed/exact-boundary replay test failed before extending the comparator.

Before the environment interruption, targeted nextest runs passed 17/17 across types/replay/application, and 2/2 in path assembly/connection tests. The latter's physical-identity/limited-range test took 1.123 s; the application determinism test took 0.296 s. These are session-transcript results: the interruption discarded `/tmp` logs, not source changes. Resumed validation logs are kept under `/workspace/scratch/arot-validation/` and summarized below when complete.

The tests cover multilayer accumulation; controller counter reduction without claiming an unknown Z1 maximum; signed/exact-turn resets; following moves; no invented physical travel; unchanged complete segment sequences with and without markers, including held regions; existing limited-axis range fitting; canonical-copy/connection-index handling; separate-file context; invalid metadata; independent replay; 1/2/N determinism; and existing unaffected small-part goldens for both posts. Existing workholding/simulation tests cover stock, part, held regions and tabs. There is no new physical motion to certify.

## Validation status

Validation is in progress after execution-environment recovery. No Phase 2 push or implementation PR has been made. Required broad tests, coverage, affected golden review and final checks must finish before delivery. Merge Manager's version allocation is also outstanding.
