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

## Controller source check

Re-read firmware 1.1.2 source at the already-recorded commit [`b3a2e26`, Robot.cpp](https://github.com/MakeraInc/MakeraZ1Firmware/blob/b3a2e26/src/modules/robot/Robot.cpp#L666). The G92.4 handler calls `THECONVEYOR->wait_for_idle()` before the reset. In the A/S branch it reads float positions, acts only when `fabs(wa) > 360`, subtracts `wa - fmodf(wa,360.0)` and calls `reset_axis_position`; the separate R branch requests physical motion and is not used. Thus the existing supported S command synchronizes with queued motion; no extra synchronization or rewind command is added. Synchronization may add a controller stop at the clear boundary. IR segment travel/feed intent and programmed durations remain unchanged; this is not a claim of unchanged real-machine elapsed time. Float use supports the precision concern but does not establish a numeric capacity or real-machine precision guarantee. The same commit’s [`SpindleControl::on_gcode_received`](https://github.com/MakeraInc/MakeraZ1Firmware/blob/b3a2e26/src/modules/tools/spindle/SpindleControl.cpp#L18) gates its handling on M commands; the G92.4 S selector is not a spindle-speed change in that handler. No new hardware trial was performed.

## Requirement-first evidence

Before production changes, the new long-operation test failed because the post reset only at operation end; the explicit-boundary roundtrip test failed because the IR could not carry the metadata. Initial uninstrumented `cargo test` executions reported 0.13 s for the first regression and 0.51 s for the four-test run (two intended failures after correcting version masking in the unaffected golden comparison). Nextest was initially absent and installed. After posting gained boundary actions, the independent replay tests failed under nextest because replay permitted only operation-boundary resets (0.010/0.086 s). A dedicated signed/exact-boundary replay test failed before extending the comparator.

Before the environment interruption, targeted nextest runs passed 17/17 across types/replay/application, and 2/2 in path assembly/connection tests. The latter's physical-identity/limited-range test took 1.123 s; the application determinism test took 0.296 s. These are session-transcript results: the interruption discarded `/tmp` logs, not source changes. Resumed validation logs are kept under `/workspace/scratch/arot-validation/` and summarized below when complete.

The tests cover multilayer accumulation; controller counter reduction without claiming an unknown Z1 maximum; signed/exact-turn resets; following moves; no invented physical travel; unchanged complete segment sequences with and without markers, including held regions; existing limited-axis range fitting; canonical-copy/connection-index handling; separate-file context; invalid metadata; independent replay; 1/2/N determinism; and existing unaffected small-part goldens for both posts. Existing workholding/simulation tests cover stock, part, held regions and tabs. There is no new physical motion to certify.

## Validation results (26–27 October 2026)

- Uninstrumented six-crate run: **1100/1100 passed**, two pre-existing skips; 2185.841 s overall. Longest individual test 529.229 s (the existing finest-resolution simulator oracle). No executed uninstrumented case exceeded ten minutes.
- Final focused application/post run: **15/15 passed**, 2.064 s. It includes spindle preservation across a pass reset and the standalone local-clearance/limited-axis contract. New tests range from 0.012 to 5.432 s in the resumed uninstrumented runs; no new long-test registration is needed.
- Quiet first-cut end-to-end: **3/3 passed**, 364.122 s overall. The existing six-generation 1/2/N × both-post golden test took 297.416 s; the stock simulation took 66.650 s, preserving the part, held regions and tabs. Both already have long/coverage registrations. The setup-sheet check also passed. No committed project fixture was regenerated.
- Formatting, workspace all-target Clippy, dependency/licence/records/trace checks passed; trace reports 100/100 P1 requirements with tests. Library-family doctests and the locked all-target fuzz check passed. No CI, new LinuxCNC execution or hardware pass is claimed here.

### First-cut output and physical-motion audit

The uninstrumented one-worker CLI run used the restored committed first-cut project with its Plastic tool presets and shipped Makera post. It passed gate 2 over **16837** poses (tip error 0.0007 mm; A error 0.0005 degrees; programmed fed time 1727.2 s for replay and IR). Its 333.001 s generation wall time was measured while other checks ran and is not a performance benchmark.

| Operation | Previous counter range, degrees | New counter range, degrees | Previous/new reset count |
| --- | --- | --- | --- |
| Rough | 135 to 82080 | 75 to 5040 | 1 / 20 |
| Finish | −168.397 to 20325.352 | −168.397 to 20325.352 | 1 / 1 |

A separate Decimal-based decoder compared the original committed golden with the new program. After reconstructing physical A by accumulating each nonmoving reset's whole-turn offset, all **16837** programmed coordinate/motion/feed records and spindle/tool-control blocks were identical. It rejects a reset carrying XYZ or an unexpected reset form. It does not infer G28 homing, acceleration, actual stop duration or hardware behavior. [Raw hashes and results](decisions/0247-firstcut-counter-evidence.json).

The approved golden change is only `testdata/golden/first_cut/first_cut_makera-z1.nc`: 19 additional between-pass resets and corresponding A words. Removing reset lines and replacing A numeric words with a placeholder makes its bytes identical to the previous golden. The LinuxCNC first-cut golden and all other goldens remain unchanged. The single uninterrupted finishing helix still grows to 20325.352 degrees, exactly as deferred by the owner; no guessed numeric limit or precision guarantee is introduced.

### Own-crate line coverage

Each crate ran its own tests under `cargo llvm-cov nextest --profile coverage -p CRATE --locked --no-report`, followed by summary/JSON reports with test/example/benchmark directories excluded from the summary. Compact build settings disabled debug information/incremental artifacts to fit the workspace; coverage instrumentation remained enabled. Coverage timings are not used as test-cost timings. All selected instrumented tests passed:

| Crate | Selected tests passed | Own source lines covered / total | Line coverage |
| --- | ---: | ---: | ---: |
| openchuck-types | 63 | 2186 / 2806 | 77.90% |
| openchuck-path | 346 | 13657 / 14883 | 91.76% |
| openchuck-post | 56 | 945 / 1075 | 87.91% |
| openchuck-gcode | 24 | 1042 / 1139 | 91.48% |
| openchuck-app | 523 | 17344 / 18917 | 91.68% |
| openchuck-sim | 50 | 1561 / 1722 | 90.65% |

All aggregates remain below the existing report-only 95% target. The changed code was read against the reports. New executable types/post/application lines are covered. Remaining new-line gaps are the assembly refusal paths for a missing or invalid existing retract, the connection-index overflow error for malformed host metadata, initialization of `helical_pass_start = false` in pre-existing simultaneous/wrap-fit gap-splitting branches not exercised by the selected own-crate coverage tests, and replay's defensive conversion of an invalid-IR validation error. No coverage exclusion was added to hide these lines. Path coverage was collected before a final layout-only rustfmt change; final source was inspected, and the uninstrumented tests use the same implementation. Simulation's only code migration is a test-support group initializer; its product code has no new executable line.

Reports and raw run logs are retained under `/workspace/scratch/arot-validation/`. `cargo llvm-cov clean --workspace` completed afterward; reports were saved outside its disposable target directory first.

## Delivery and release hold

Phase 2 uses a **second draft PR**, as the original task instructs unless the owner requests an update to the first; no such update instruction was given. The implementation branch includes the original proposal evidence and the owner's directly accepted revision, so Merge Manager should consume this implementation rather than merge the obsolete reporting recommendation from proposal draft #359 separately.

No project/machine schema is claimed and no library version has been chosen. The public-field changes require a family minor bump under decision 0048. The owner specifically requires Merge Manager to release AROT and assign the version/merge order first; the draft intake retains 0.24.0 pending that instruction. The bump and semver checks are mandatory on the released integration push before merge. This is not a semver exemption, a claim that semver checks pass, or permission to mark ready/merge. No parallel type or function was added to avoid the breaking API change.

Generated G-code remains the user's to verify before running on a machine. The owner tests the result and closes #337.

## Independent first-cut comparison reproducer

The supplemental decoder below compares the old golden with the newly generated Makera file. Production generation is not instrumented or altered by this analysis.

```python
from decimal import Decimal as D
import pathlib,re,json,hashlib,sys

def decode(p):
    pos=[D(0)]*4;offset=D(0);kind=0;feed=None;motions=[];controls=[];stats={};op='header';op_index=0
    for line in p.read_text().splitlines():
        if line.startswith('(operation '):
            op_index+=1;op=str(op_index)+':'+line.split(', tool')[0][11:].rstrip(')')
        if line.startswith('('):continue
        words=[(a,D(b)) for a,b in re.findall(r'([A-Z])\s*([-+]?\d+(?:\.\d+)?)',line.split(';')[0])]
        w=dict(words);gs=[v for k,v in words if k=='G']
        if D('92.4') in gs:
            assert set(w)=={'G','A','S'} and gs==[D('92.4')] and w.get('S')==0 and w.get('A')==0
            if abs(pos[3])>360:
                remainder=pos[3]%360;offset+=pos[3]-remainder;pos[3]=remainder
            stats.setdefault(op,{'counter_min':None,'counter_max':None,'resets':0})['resets']+=1
            continue
        if 'M' in w:controls.append(tuple((a,v) for a,v in words if a in 'MST'))
        if 'F' in w:feed=w['F']
        for g in gs:
            if g in [0,1]:kind=int(g)
        axes=any(a in w for a in 'XYZA')
        if not axes:continue
        if D(53) in gs:
            # Compare the explicit machine-coordinate footer without inferring
            # work offsets, G28 homing travel, acceleration or hardware time.
            motions.append(('machine',tuple((a,w[a]) for a in 'XYZA' if a in w),kind));continue
        for i,a in enumerate('XYZA'):
            if a in w:pos[i]=w[a]
        stat=stats.setdefault(op,{'counter_min':None,'counter_max':None,'resets':0})
        stat['counter_min']=pos[3] if stat['counter_min'] is None else min(stat['counter_min'],pos[3])
        stat['counter_max']=pos[3] if stat['counter_max'] is None else max(stat['counter_max'],pos[3])
        state=('work',tuple(pos[:3])+ (pos[3]+offset,),kind,feed if kind==1 else None)
        if not motions or state!=motions[-1]:motions.append(state)
    return motions,stats,controls

old,new=map(pathlib.Path,sys.argv[1:]);a,sa,ca=decode(old);b,sb,cb=decode(new)
assert ca==cb, "spindle/tool control blocks changed"
if a!=b:
    for i,(x,y) in enumerate(zip(a,b)):
        if x!=y:print('first mismatch',i,x,y);break
    raise SystemExit(f'motion mismatch: {len(a)} versus {len(b)}')
report={'baseline':str(old),'new':str(new),'baseline_sha256':hashlib.sha256(old.read_bytes()).hexdigest(),'new_sha256':hashlib.sha256(new.read_bytes()).hexdigest(),'identical_programmed_coordinate_records':len(a),'identical_spindle_tool_control_blocks':len(ca),'baseline_operations':sa,'new_operations':sb,'limitations':'Explicit programmed XYZ/A/feed/motion records only; no G28 homing inference, acceleration or new hardware validation.'}
print(json.dumps(report,indent=2,default=str))
```
