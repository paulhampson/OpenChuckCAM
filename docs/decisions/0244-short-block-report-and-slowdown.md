# 0244: Configurable command demand reporting and geometry-preserving slowdown

- **Status:** accepted scope, 25 Oct 2026: the project owner instructed this session to execute the proposal presented in this conversation. Decision 0222 already accepts Report followed by Slow down; Merge remains deferred. The implementation details and verification limits below record how this session executes that scope.
- **Lane:** SHORTMOVE, `codex/lane-shortmove-smooth-running`.
- **Requirements:** GC-17, GC-04/07/08/12, SET-01, PRJ-04, NFR-05/06/07/12.
- **Numbers:** remote branches inspected 25 Oct 2026; main holds 0243, OI-990, schema 18/machine 4. Draft #284 describes unpublished reservations 19/5 and OI-991. This lane takes 0244, OI-992 and integration schemas 20/6; the fields are recognized at 19/5 to remain compatible with that reserved predecessor. Claims were written before persistence changes.

## Evidence and need

Owner [results in #284](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6399134358): OpenChuckCAM 0.20.0, Windows, Z1 firmware 1.1.2, WiFi upload, ESP32 version not supplied. Parts A and B both passed 20/10/7 ms and stuttered at 5/3/2/1 ms. The 7 ms sections measured 6.1 and 5.9 seconds against 6.0 predicted. This is the shortest tested smooth value, not an exact universal boundary or a proof of which supply stage is responsible.

The accepted investigation found the owner's pencil path demanded a 5.6 ms average over 32 blocks. Reporting and mitigation are justified. The reference jobs mostly have much longer blocks. Do not impose a universal minimum or relax the geometric tolerance to hide command demand.

## Implementation

1. Machine profile: `min_block_ms` (finite, >=0), `block_window` (1..4096), `short_block_mode` (Off, Report, Slow down), with per-field provenance. Optional post overrides independently replace each field. Zero or Off disables analysis. The window cap bounds work and allocation; default window is 1.
2. New built-in Z1: Report, 7 ms, 32 blocks. The 7 ms value records the owner's hardware evidence; 32 is source-based planner depth, not a new measurement. Existing project copies load as Off. Editing the profile preserves its library ID and marks edited fields as user-provided/unconfirmed. Mode is a policy choice, not a physical measurement.
3. Analyse actual emitted fed blocks using rounded positions, controller feed rule, feed-word rounding and post feed clamp. Consecutive groups and rapid moves split runs. A short run uses `min(N, run length)` so boundaries do not suppress all checks. An isolated short block can otherwise be absorbed by a compliant window.
4. Report changes no G-code or original duration estimate. Per operation it shows individual short moves and their longest run, deficient windows, worst mean, effective contact feed, and the predicted slowdown time including feed rounding.
5. Slow down processes windows in order. It raises the shortest durations first, sharing increases among equally short blocks with stable input-order ties. Increasing durations cannot invalidate a previously repaired window. Feed rounding is conservative: the selected written feed is at or below the calculated cap. No feed is increased, coordinate or move kind changed, merge introduced, or tolerance consumed. This is a deterministic feasible adjustment, not a claim of globally minimum added time or acceleration-optimal feed scheduling.
6. Post results return adjusted verification durations. Gate 2 checks them, and setup sheets/export use the same run's reports and time. Active command management rejects the legacy Contact feed rule because its rotary timing does not identify a controller interpretation; Off retains legacy compatibility. Makera, LinuxCNC and G93 rules retain their respective controller interpretation.
7. Public profile/post/report API additions bump the workspace to 0.22.0. Schema changes are additive; pre-field-schema files carrying the fields are refused, while genuine older files retain Off. Future GATES integration must reconcile its unpublished duplicate implementation and schema claims; no unrelated gate evidence or waiver is overwritten.

## Validation and limits

Requirement-first tests cover known demand/windows, Off/Report preservation, overlapping-window slowdown, geometry preservation, short runs, rapid boundaries, feed rounding, mixed rotary/G93, post overrides, invalid limits, deterministic output, migration, reporting and adjusted replay timing. Software implementation and local checks completed 26 Oct 2026. Current-run results and the 7 ms corpus analysis are recorded in [short-block verification](../short-block-verification.md).

Physical mitigation is pending **OI-992**. Before claiming smooth running: pass the normal static/replay/simulator gates, compare original and slowed affected paths in an attended Z1 air cut, record timing/stutter, then compare cutting behaviour and finish on representative material. Record firmware, delivery method, settings and margin; recalibrate on firmware or delivery changes. Nominal window compliance cannot guarantee smoothness against transport outages, controller acceleration/jerk constraints or feed override above the tested setting. Generated G-code remains the user's to verify (NFR-12).

No Merge, strategy minimum step, arc fitting, firmware change, streaming, LinuxCNC blending or automatic hardware operation is added. OI-932/933/934/936/937 remain unchanged.

## First-cut follow-up raised during implementation

The owner reported underflow-like movement at the far-end first-cut tab at 100% override, without a line/file reference. [The investigation](../first-cut-tail-tab-investigation.md) found no short-block demand there in the committed program, but a 72-block A-only ring and published firmware logic that skips junction blending for those moves. This is a distinct, unconfirmed planner-stop hypothesis (OI-993), not a confirmed delivery underflow or a problem this minimum-time feature fixes. Merge remains deferred; no firmware or ring-path change is made.
