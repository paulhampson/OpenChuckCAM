## 10. Results

Evidence: [owner comment of 25 Oct 2026](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6399134358), comment ID 6399134358, reproduced verbatim in 10.3. The owner reviewed as PH on 25 Oct 2026. Gate 5 was run on 24 Oct 2026 by PH. Gate 4's date and operator fields are blank; do not infer them from gate 5. The tested commit is from main’s history, not this lane’s old head; missing program files and setup fingerprints prevent a claim that the exact committed golden programs were run. Both forms name OpenChuckCAM 0.20.0, clean commit `dc610e26dd260d69b5fc859270e7bf5fc913a3bb`, built 2026-10-24T20:29:47Z, rustc 1.97.0 (2d8144b78 2026-07-07), x86_64-pc-windows-msvc. Z1 firmware: 1.1.2. The block-rate controller is identified only as ESP32, without its firmware version; WiFi upload; video exists but was not shared.

Classification follows the pass conditions above and verification plan section 6: **passed** means the submitted evidence supports that check; **failed** means an observed pass-condition violation; **waived** means explicitly omitted by the owner (not a pass); **not run** means no completed verification of that step is established by the submission, including blank or insufficient fields; it does not assert that an unreported physical action never occurred. **Not applicable** means a stated condition does not apply. A partial observation is retained in Notes, never used to infer the rest of the check.

### 10.1 Gate 4

Sphere and cone are reported passed at 50%, then 100%, with no alarms, motion and end positions as simulated, and timing matching predictions. No numerical reference-program predicted time, measured time, ratio, timing-band width or unwind DRO values were supplied. Eight other reference programs were explicitly waived for time. The first-cut air cut is not separately recorded. Gate 4 as a whole is **not recorded as passed**. The sphere/cone motion observations passed at their actual settings, while the full G4-2/G4-4/G4-7 instructions are not established because the required lower overrides were unavailable; both facts are retained.

Decision 0221 changes gate 5's part and tolerance, but explicitly retains the reference set at gate 4; it does not authorize omitting eight air cuts. Decision 0106 makes the trial a manual owner release step and permits release with unconfirmed Z1 values; it does not turn omitted checks into passes. The owner’s explicit reference-program waivers are retained as release exceptions in the release report. They do not amend the accepted records or establish a pass. Decision 0232 expressly requires K4 first; the submitted K4 waiver departs from it. The owner explicitly chose “Retain K4 as a release waiver” on 25 Oct 2026 in this takeover session. K4 is an approved release exception for this trial only; it remains waived, not passed. Decision 0232 and policy 6 still govern subsequent trials. The observed minimum passing block time is **7 ms**, limited to this program, machine, firmware and transport. No limit between the tested values, for other firmware or for other controllers is inferred.

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| P1 | passed | Clean build and full provenance supplied. | OI-471 |
| P2 | not run | No separate completed-check evidence supplied. | OI-471 |
| P3 | not run | No separate completed-check evidence supplied. | OI-471 |
| P4 | not run | No separate completed-check evidence supplied. | OI-471 |
| P5 | passed | Firmware 1.1.2. | OI-471 |
| P6 | not run | No separate completed-check evidence supplied. | OI-471 |
| P7 | not run | T1 M6: “requests tool change”; separate spindle observation absent. | OI-471 |
| P8 | not run | Axis height −48.6 mm after probing; profile correction/regeneration not evidenced. Safe Z50 would be machine Z+1.4 at this height, so clearance requires reconciliation. | OI-471; OI-60 |
| P9 | not run | X0: “Setup on in control software”; A0: “default position via makera setup.” G54 machine X/Y/Z blank. | OI-471 |
| P10 | not run | No separate completed-check evidence supplied. | OI-471 |
| R1 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R2 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R3 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R4 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R5 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R6 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R7 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R8 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R9 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R10 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R11 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| R12 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S1 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S2 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S3 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S4 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S5 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S6 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S7 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| S8 | passed | Sphere and cone: grouped R1–R12/S1–S8 response “No issues”. Individual values and fingerprints not supplied; waived jobs are not covered. | OI-471 |
| G4-0 | passed | Sphere read: “No issues”. | OI-471 |
| G4-1 | not applicable | Firmware is 1.1.2; no new SP-12 run required. | OI-471 |
| G4-2 | not run | Partial evidence: sphere motion passed at 50%, then 100%, with no alarms and expected end position; G28 went to clearance. The required 10% run was not completed (reported minimum 50%). | OI-471: override disposition |
| G4-3 | passed | Sphere: “Matched predicted”; agreed band “yes”, numerical band and times absent. | OI-471 |
| G4-4 | not run | Partial evidence: cone motion passed at 50%, then 100%, with no alarms and expected end position. The required 10% run was not completed (reported minimum 50%). | OI-471: override disposition |
| G4-5 | waived | Groove not run; no groove unwind DRO evidence. | OI-471 |
| G4-6 | not applicable | G93 recheck conditional on firmware other than 1.1.2. | OI-471 |
| G4-6b | not run | Owner: “Not checked”. OI-267 stays open. | OI-267 |
| G4-7 | not run | Partial evidence: sphere and cone raised from 50% to 100%; controller minimum override reported 50%. The full instructed 25%/50%/100% sequence was not completed. | OI-471: override disposition |
| G4-8 | failed | Six sections passed; eight visibly stuttered and failed. K4 was waived. See individual section results. | OI-930 |
| K1 | passed | Owner confirms pinned 69d532f and 27,966 lines. | OI-930 |
| K2 | passed | Header/footer checks: yes. | OI-930 |
| K3 | passed | Sections 1 and 8 checked: yes. | OI-930 |
| K4 | waived | Owner: “pass - gate 3 waived”; moves reached and faults N/A. Classified waived, not passed, under decision 0232. | OI-930 |
| B0 | not run | Firmware 1.1.2, ESP32, WiFi upload; video yes, not shared. ESP32 firmware version and explicit playback-from-machine confirmation absent. | OI-471: complete controller/transport provenance |

| Program | Result | Evidence | Follow-up |
| --- | --- | --- | --- |
| groove | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| lobed | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_axial | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_indexed | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_axial_lead | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| cone_held_tabs | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| drilling | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| swarf | waived | “Only cone and sphere were run due to time. Others were waived.” | OI-471: retain owner waiver; program remains physically unverified |
| first_cut air cut | not run | Gate 4 issue field blank; no separate air-cut result supplied. | OI-471: obtain evidence or run |

| Section | Part | Block time | Predicted s | Measured s | Within 5 %? | Stutter or pause? | Pass / fail |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | A | 20 ms | 6.0 | 5.9 | yes | no | pass |
| B2 | A | 10 ms | 6.0 | 6.1 | yes | no | pass |
| B3 | A | 7 ms | 6.0 | 6.1 | yes | no | pass |
| B4 | A | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B5 | A | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B6 | A | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B7 | A | 1 ms | 7.9 | N/A | N/A | yes | fail |
| B8 | B | 20 ms | 6.0 | 6.2 | yes | no | pass |
| B9 | B | 10 ms | 6.0 | 5.8 | yes | no | pass |
| B10 | B | 7 ms | 6.0 | 5.9 | yes | no | pass |
| B11 | B | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B12 | B | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B13 | B | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B14 | B | 1 ms | 7.9 | N/A | N/A | yes | fail |

Result: the shortest block time at which both parts pass ___7___ ms (for `min_block_ms`, OI-930)

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| B15 | not applicable | No 20 or 10 ms section failed; stop condition not triggered. | OI-930 |
| B16 | passed | Minimum passing value in both parts and all longer sections: 7 ms. | OI-930: measured value implemented as 7 ms/window 32/Report |

All eight failed sections (B4–B7, B11–B14) are linked to OI-930. Their measured times remain N/A. The failure at shorter times is evidence for the measured threshold, not evidence that all block-rate sections passed. B15 is not applicable: none of the 20 or 10 ms sections failed. B16 (record the minimum) passed: 7 ms.

### 10.2 Gate 5

The owner reports the tooling-board first cut **passed** at decision 0221’s ±0.1 mm limit: tabs whole, gripped length unmarked, length and tab positions within limit, surface as simulated. “All measurements of resulting machined stock were as expected.” About 0.8 mm remained adjacent to tabs as simulated, with no overcut reported. This is the submitted first-cut outcome, not a claim that every procedural field or five-station measurement table was completed. No station diameters, instrument, numerical errors, setup fingerprint, G54 machine coordinates, photographs or screenshots were supplied.

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| F1 | not run | First-cut gate 4 issue and result absent. | OI-471 |
| F2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| F3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-41 |
| F4 | passed | ±0.1 mm, decision 0221. | OI-471 |
| F5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| F6 | passed | Tools agreed by PH, 2026-10-24. | OI-471 |
| G0 | passed | Geometry agreed by PH, 2026-10-24. | OI-471 |
| W1 | passed | Grip 5 mm, within the agreed maximum 10 mm; 68 mm held region remains the agreed free-stock workaround, not a measured jaw depth. | OI-471 |
| W2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| W3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| W4 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| W5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| T0 | passed | Tools agreed by PH, 2026-10-24. | OI-471 |
| T1 | passed | 104 single flute; 204 two flutes. | OI-471 |
| T2 | passed | Plastic and tool-data speed source reported; one 10000 rpm entry supplied. Individual operation speeds were not recorded. | OI-471 |
| T3 | not applicable | Both selected tools have Plastic presets. | OI-471 |
| T4 | not run | Cutting/plunge feed 1000/300 supplied; individual F-word limit check not reported. | OI-471 |
| T5 | not run | Stick-out “as with Makera standard collet spacer”; numerical depth/margin not supplied. | OI-471 |
| V1 | not run | Simulation reported, cell “default”; completion/current-result check not separately evidenced. | OI-471 |
| V2 | not run | No overcut reported; separate rapid-removal check not supplied. | OI-471 |
| V3 | passed | ~0.8 mm adjacent to tabs. | OI-471 |
| V4 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| V5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| V6 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M1 | passed | Stock measured 35x35x100 mm. | OI-471 |
| M2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M4 | not run | X0 chuck jaw, A0 default; G54 machine coordinates absent. | OI-60 |
| M5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| M6 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C1 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C2 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C3 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-41 |
| C4 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C5 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C6 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| C7 | not run | No separate completed-check evidence supplied; successful cut alone does not establish this procedural check. | OI-471 |
| I1 | passed | Tabs whole, gripped length unmarked; four-side photos not supplied. | OI-471 |
| I2 | not run | General measurement statement only; at least five station diameters and errors not supplied. | OI-471 |
| I3 | not run | Partially evidenced: length and tab positions were reported within ±0.1 mm; individual dimensions and tab diameters were not supplied, so the full step is not established. | OI-471 |
| I4 | passed | Surface agrees with simulation; ~0.8 mm adjacent to tabs, no overcut reported. | OI-471 |
| I5 | passed | “as simulation”. | OI-471 |

### 10.3 Verbatim owner submission

The following is source evidence, including the owner’s “all steps passed” shorthand. That shorthand is not the audited classification; sections 10.1 and 10.2 govern this report. Blank fields and N/A are unchanged; trailing whitespace is normalised.

<details>
<summary>Owner comment 6399134358, 25 Oct 2026</summary>

````text
Here are the results:


### 8.1 Gate 4 results

```text
Gate 4 (air cut)                          Date:            Operator:
Build information:

OpenChuckCAM 0.20.0
Commit: dc610e26dd260d69b5fc859270e7bf5fc913a3bb
Uncommitted changes: no
Built: 2026-10-24T20:29:47Z
Rust: rustc 1.97.0 (2d8144b78 2026-07-07)
Target: x86_64-pc-windows-msvc
Crates:
  openchuck-app 0.20.0
  openchuck-buildinfo 0.20.0
  openchuck-cli 0.20.0
  openchuck-gcode 0.20.0
  openchuck-geom 0.20.0
  openchuck-import 0.20.0
  openchuck-path 0.20.0
  openchuck-post 0.20.0
  openchuck-project 0.20.0
  openchuck-sim 0.20.0
  openchuck-types 0.20.0
  openchuck-ui 0.20.0
  openchuck-viewport 0.20.0

Firmware: 1.1.2                                Axis height measured (mm): -48.6 (after probing routine)
G54 in machine coordinates X / Y / Z:
Programs from: project folders (build in P1) / golden files (commit)
What the controller did on T1 M6 with no tool: requests tool change
X0 taken from:  Setup on in control software                          A0 set by: default position via makera setup.
```

| Program | Read (R1-R12, S1-S8) | Override % | Alarms or refused lines (line) | Motion as simulated | Unwind: DRO A before / after, moved? | End position as program | Timed: predicted s x 100/override, measured s, ratio | Pass / fail | Issue |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sphere | No issues | 50 then 100% | None | Yes | As expected | Yes | Matched predicted | Pass | N/A |
| cone | No issues | 50 then 100%  | None | Yes | As expected | Yes | Matched predicted | Pass | N/A |
| groove | | | | | | | | | |
| lobed | | | | | | | | | |
| cone_axial | | | | | | | | | |
| cone_indexed | | | | | | | | | |
| cone_axial_lead | | | | | | | | | |
| cone_held_tabs | | | | | | | | | |
| drilling | | | | | | | | | |
| swarf | | | | | | | | | |

Deviation: Only cone and sphere were run due to time. Others were waived.
Deviation: Minimum override is 50%. Sphere was run first at 50% then 100%, same with cone.

| Check | Result |
| --- | --- |
| `G93` on this firmware (only if not 1.1.2) | Running 1.1.2 |
| OI-267: A-only move with F above 1200 | Not checked |
| `G28` at the end: where the machine went | To clearance |
| Agreed band for the timed operation | yes |

Block-rate air cut (4.8): firmware 1.1.2 ; controller (ESP32) __ESP32__ ; file sent by __Wifi Upload____ ; video: yes - not shared

Checks before running (decision 0232): K1 fetched from 69d532f, 27,966 lines: yes ; K2 header and footer as listed: yes ; K3 sections 1 and 8 as listed: yes ; K4 gate 3 on the LinuxCNC copy: moves reached __N/A__ , faults __N/A___ , pass - gate 3 waived.

| Section | Part | Block time | Predicted s | Measured s | Within 5 %? | Stutter or pause? | Pass / fail |
| --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | A | 20 ms | 6.0 | 5.9 | yes | no | pass |
| B2 | A | 10 ms | 6.0 | 6.1 | yes | no | pass |
| B3 | A | 7 ms | 6.0 | 6.1 | yes | no | pass |
| B4 | A | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B5 | A | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B6 | A | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B7 | A | 1 ms | 7.9 | N/A | N/A | yes | fail |
| B8 | B | 20 ms | 6.0 | 6.2 | yes | no | pass |
| B9 | B | 10 ms | 6.0 | 5.8 | yes | no | pass |
| B10 | B | 7 ms | 6.0 | 5.9 | yes | no | pass |
| B11 | B | 5 ms | 6.0 | N/A | N/A | yes | fail |
| B12 | B | 3 ms | 6.0 | N/A | N/A | yes | fail |
| B13 | B | 2 ms | 6.0 | N/A | N/A | yes | fail |
| B14 | B | 1 ms | 7.9 | N/A | N/A | yes | fail |

Result: the shortest block time at which both parts pass ___7___ ms (for `min_block_ms`, OI-930)

### 8.2 Gate 5 results

```text
Gate 5 (first cut)                        Date:   2026-10-24         Operator: PH
Build information:

OpenChuckCAM 0.20.0
Commit: dc610e26dd260d69b5fc859270e7bf5fc913a3bb
Uncommitted changes: no
Built: 2026-10-24T20:29:47Z
Rust: rustc 1.97.0 (2d8144b78 2026-07-07)
Target: x86_64-pc-windows-msvc
Crates:
  openchuck-app 0.20.0
  openchuck-buildinfo 0.20.0
  openchuck-cli 0.20.0
  openchuck-gcode 0.20.0
  openchuck-geom 0.20.0
  openchuck-import 0.20.0
  openchuck-path 0.20.0
  openchuck-post 0.20.0
  openchuck-project 0.20.0
  openchuck-sim 0.20.0
  openchuck-types 0.20.0
  openchuck-ui 0.20.0
  openchuck-viewport 0.20.0

Gate 4 issue for this program:
Program:  first_cut                          Setup sheet fingerprint:
Material: tooling board, setup material Plastic
Stock measured (35 x 35 x 100 mm): 35x35x100
Geometry agreed with the owner (G0), by and date: PH / 2026-10-24
Tools agreed with the owner (T0), by and date: PH / 2026-10-24
Grip length at the chuck: 5mm                Tabs (5 mm; 2 mm x 10 mm tailstock tab):
X0 taken from: Chuck jaw                           A0 set by: default
Tools: roughing 104 (single flute), finishing 204 (TWO flutes); stick-out of each: as with Makera standard collet spacer
Spindle rpm and Speed from: 10000 As tool library        Cutting feed / plunge feed: 1000 / 300
First-cut limit: ±0.1 mm (decision 0221)
Simulation: cell (mm), worst material left (mm, where), overcut: cell: default, ~0.8mm adjacent to tabs, no overcut
```

All measurements of resulting machined stock were as expected.

| Check | Result |
| --- | --- |
| Tabs whole | yes |
| Gripped length unmarked | yes |
| Length and tab positions within the limit | yes |
| Surface notes | as simulation |
| Pass / fail, issue | pass |

## 10. Results

Empty until the owner submits the results of gates 4 and 5. They are then added here, one row per step ID, by the lane that folds them in, with the issue that holds the evidence; a step that did not pass names its defect issue. A surprise that changes an instruction is fixed in the step above in the same change, and the row says so.

### 10.1 Gate 4

As above - all steps passed

### 10.2 Gate 5

As above - all steps passed

## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | PH |
| Date | 2026-10-25 |
| Machine and firmware | As above |
| Changes required | No |
| Reviewed: yes / no | Yes |
````

</details>

### 10.4 Initial takeover verification (before implementation, 25 Oct 2026)

This is historical preparation evidence; post-implementation results are recorded in 10.6. These checks concern the old lane head `9573685001117c4e71a236c555f34138fb21cbad` plus uncommitted preparation. Main `dc610e26dd260d69b5fc859270e7bf5fc913a3bb` was inspected, not merged. They do not verify omitted physical trials.

| Check | Outcome | Time |
| --- | --- | --- |
| `cargo test -p openchuck-app --test output_options --locked` | 43 passed | 19.41 s tests; 75.582 s command/build |
| `cargo test -p openchuck-post -p openchuck-project --tests --locked -- --skip req_set_01_req_gc_09_z1_records_the_measured_block_time_and_queue_window` | Existing baseline passed; new red test explicitly excluded only for this baseline comparison | 7.108 s |
| `cargo test -p openchuck-project --test z1_block_time --locked` | Expected TDD failure before implementation: absent min_block_ms (`None` versus `Some(7.0)`); not ready | <0.01 s test; 55.380 s command/build |
| `cargo test -p xtask replay --locked` | Six passed, including first-cut replay selection | 2.469 s |
| `cargo test -p openchuck-cli --test cli --locked` | 27 passed | 0.19 s tests; 11.919 s command/build |
| `cargo xtask all` | Passed: dependency rules, licences, records, testdata, conflicts, publishing checks and traceability (99/99 P1) | 107.106 s |
| `cargo clippy -p openchuck-project -p openchuck-post -p openchuck-app -p openchuck-cli --all-targets --locked -- -D warnings` | Passed | 19.468 s |
| `cargo fmt --all -- --check`; `git diff --check` | Passed | 1.471 s formatting |
| `cargo audit`; `cargo deny check` | Commands unavailable: cargo-audit and cargo-deny not installed | 0.039 / 0.044 s |
| Changed-crate coverage | Not run: cargo-llvm-cov and nextest not installed; no product implementation added yet, only the requirement test | — |
| Full e2e/first-cut determinism, full application/UI suites and STEP kernel checks | Not rerun yet; required before final intake | — |
| Native Windows/macOS and Z1 trials | No access in this Linux session; original Windows/Z1 evidence is the owner submission | — |

No test observed so far exceeded ten minutes. No instrumented artifacts were created. Current main has the clipboard-win 5.4.3 dependency fix; this old lane still has 5.4.1. Preserve the main fix during the Merge Manager’s released main update. Do not push a speculative dependency update merely to begin takeover.

Local K4 attempt uses the exact Z1 file at `69d532f` with only the footer replacement required by 0232, and image `ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10` at digest `sha256:22d8b9f0acb8a0fd7a02bf47be7fd9e864e671bd510ca27ab35ff214d1489546`. Initial `cargo xtask replay` failed twice with no report (48.226 s command). Temporary copied files were mode 600/700 and unreadable by the image’s unprivileged user. Changing only those temporary copies to 644/755 let LinuxCNC complete. The comparison then failed at line 2215, target `[63.25, 0, 50, 0]`, with a 46 ms sampling gap. LinuxCNC reported no errors and final position approximately `[0, 0, 50, 439.331]`. This is not a K4 pass. First report SHA-256: `2737b10a7e85f3f46b3ff278f275e27e8680824e8cb9375a6c170d64aa0407de`. Raw report is temporary local evidence (8.8 MB), not committed as test data.

The one subsequent repeat, with the build workload stopped, **passed the local comparison**: all 7,559 moves recognised by `gcode-sim` reached; largest difference 0.0379 mm at tolerance 0.05 mm; 68.8 s of reported motion; no fault. Docker wall time: 153.711 s; comparison: 0.487 s. Maximum sampling gap: 25.8 ms; final position approximately `[0, 0, 50, 439.331]`, G92 offset zero. Report SHA-256: `48fdb6ec67c3fd74d8166bed41d3e4dc73875a1edca9e8af732359be8ba0f6c8`.

Reproduction after the temporary-file permissions adjustment: `docker run --rm -v /tmp/openchuck-replay-1237-block_rate_linuxcnc.ngc:/work -w /work -e CHECK_PROGRAM=/work/program.ngc -e CHECK_REPORT=/work/report.json ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 linuxcnc checks.ini`, then `target/debug/openchuck-cli gcode-sim /tmp/block_rate_linuxcnc.ngc /tmp/openchuck-replay-1237-block_rate_linuxcnc.ngc/report.json --tolerance 0.05`. The work directory contains unmodified repository checks.ini, sim.tbl and sim_driver.py, plus the pinned program with the prescribed footer substitution. The exact digest above pins the image. No tolerance was increased to obtain a pass.

This subsequent replay supplies current simulator evidence. It does not show that K4 preceded the owner’s machine run, does not change the historical K4 row from waived to passed, and does not pass any omitted physical reference program. The owner retained the sequence deviation as a release waiver on 25 Oct 2026. The approved 7 ms profile/report implementation and schema allocation are completed locally. Post-implementation verification is recorded below; the subsequent bounded first-cut evidence is recorded in 10.8; this initial-stage account does not claim a full uninterrupted replay.

### 10.5 First-cut replay duration flag (owner-authorized bounded replay)

The stored first-cut LinuxCNC golden has **1,727.333 s of nominal fed motion**, derived from its modal G0/G1 positions and F words under the LinuxCNC feed rule. This is an analysis estimate, not a replay result, and excludes rapids, acceleration and tool changes. Both the original wrapper and driver imposed 900 s, so an unchanged full replay could not finish. AGENTS.md requires an owner disposition for an uninstrumented test over ten minutes.

The owner directly instructed “Apply your suggested approach to allow support of the test” on 25 Oct 2026, authorizing bounded chunks preserving every original move with explicit modal-state and continuity checks. At that stage the complete detailed record [0244](../../decisions/0244-bounded-first-cut-linuxcnc-replay.md) was proposed; acceptance was not inferred from the earlier instruction. The owner separately accepted the complete record on 26 Oct 2026 (section 10.11). No reacceptance of 0221, 0222 or 0232 is requested.

Implementation targets at most 300 s estimated simulator time per fixture, including startup, with a hard limit of at most 600 s in both driver and wrapper. It expands before CI sharding. Each fixture initializes simulated joints at the original boundary coordinates, restores G modes, F, tool and spindle state, and checks actual start/end coordinates and modes at simulator-only M0 stops followed by `M66 E0 L0` queue barriers. The barriers prevent interpreter read-ahead from changing F/modes before the paused snapshot. Original contiguous source lines are retained exactly once across all chunks; production G-code and comparison tolerances are unchanged. Unsupported modes/syntax and atomic moves that cannot fit fail closed. Separate fixtures add stops and fresh interpreter initialization, so their results cannot establish uninterrupted full-program dynamics or the original pre-cut gate ordering.

OI-991 is closed by the thirteen passing bounded fixtures and verified source/boundary continuity in 10.8. OI-471 retains the physical-trial and missing-measurement gaps. The K4 release waiver and the supported Gate 5 physical outcome are unchanged.

### 10.6 Implementation and local verification, 25 Oct 2026

The requirement-first Z1 profile test failed before the fields existed (`None` versus `Some(7.0)`). Report/setup-sheet and longest-run-feed checks also exposed failures before their implementations were corrected. Accepted 0222 now has an executable **Report** implementation: the new built-in Z1 uses **7 ms / 32 blocks / Report**, and optional post fields override the profile independently. A complete sliding window is required; rapids and operation boundaries reset it. Reports identify affected blocks, the longest run, its effective contact feed, the worst window mean and estimated added time. An incomplete window is identified explicitly. The estimate does not change path durations or G-code. Slow down and Merge remain unbuilt.

Older project machine copies and other profiles default to **0 ms / 1 / Off**; no migration substitutes the new built-in profile. Tests cover old standalone profiles/project copies, schema guards, disabled-profile warnings, both posts' byte-identical Off/Report output, deterministic reports, overrides, invalid input, rapid boundaries, and application warnings plus HTML/PDF setup sheets. CLI inspection shows the actual setting. The public API additions bump the workspace/library version to **0.22.0** (the next free minor after released main 0.21.0). The intentional G-code text change is the generator version comment; motion/feed words remain unchanged. Existing golden tests normalise that comment and passed at 1, 2 and N threads.

Project schema **19** and standalone machine schema **5** were claimed before use after checking main and all remote branches. The released main integration replaces this branch’s reserved 18/4 predecessor steps with OPSPEED’s actual migrations and preserves its operation-material and spindle-limit fields. Current main's clipboard-win 5.4.3 and tokio 1.53.3 dependency fixes are retained locally; no unrelated dependency upgrade was made.

| Local check | Result | Uninstrumented time / scope |
| --- | --- | --- |
| Product tests: types, project, post, application and CLI, including e2e golden regeneration and first-cut determinism | All **775 currently selected tests** have passing evidence across the baseline and final affected-test reruns; no unresolved product-test failure | Final affected/remaining batch: 325/325, 488.246 s; core batch: 234/234, 2.506 s; subsequent focused additions passed. Longest individual test: first-cut determinism, **369.528 s**. |
| UI-26 numeric-entry tests | 18/18 passed | 0.75 s |
| xtask replay-selection tests | 6/6 passed | 0.01 s final execution (build excluded) |
| Clippy, all targets, warnings denied | Passed for types, project, post, app, CLI, UI and xtask | 9.023 s final command (earlier all-target run 32.60 s) |
| `cargo xtask all` | Passed: dependencies, licences, records, testdata, conflicts, publishing checks and traceability; **100/100 P1** requirements have tests | 22.051 s final command |
| `cargo audit` | Passed; 1,352 advisories / 449 dependencies, no vulnerability reported after the tokio fix | 5.448 s command |
| `cargo deny check` | Passed advisories, bans, licences and sources; existing duplicate-version/unmatched-exception warnings retained | 2.371 s command |
| Formatting and whitespace | `cargo fmt --all -- --check` and `git diff --check` passed | 1.465 s formatting; whitespace clean |
| Fuzz manifest/lockfile consistency | Locked offline metadata passed; not a fuzz campaign | Scope only |

Tests were run uninstrumented before coverage. Every newly observed test over about a minute is listed in both the `coverage` exclusions and `long` selections of `.config/nextest.toml`, with measured times. None of the executed uninstrumented product tests exceeded ten minutes. The first-cut duration flag now has an owner-authorized bounded implementation (10.5); the subsequent execution evidence is recorded separately.

Changed-crate **own-test** line coverage is a lower bound, not the nightly whole-suite result:

| Crate | Covered / executable lines | Line coverage |
| --- | --- | --- |
| openchuck-types | 2,043 / 2,621 | 77.95% |
| openchuck-project | 3,276 / 3,608 | 90.80% |
| openchuck-post | 652 / 740 | 88.11% |
| openchuck-app | 15,495 / 16,786 | 92.31% |
| openchuck-cli | 929 / 1,400 | 66.36% (27 CLI command tests; e2e coverage not included in this lower bound) |

These totals **do not meet the 95% development target**; it is not a PR merge gate (0102). No source was excluded to improve them. The new `block_time.rs` module is **121/123 lines, 98.37%** on its own crate's tests. Its unexecuted lines are a loop closing brace and the defensive zero-duration effective-feed fallback; normal posted moving blocks require a positive duration. Other new own-crate gaps are the types' field-name/provenance plumbing (covered by consuming application/CLI tests), a project migration closing brace, UI/profile label branches and setup-sheet fallback branches. These are recorded rather than filled with implementation-mirroring tests. Application coverage ran all 463 selected own tests (708.717 s instrumented suite time; 10 long tests skipped by the coverage profile). CLI e2e tests passed uninstrumented. Instrumented artifacts were cleaned with `cargo llvm-cov clean --workspace` and the separate instrumented target directory removed; JSON summaries were retained outside it.

The K4 replay outcome and hashes are in 10.4. Reference-program gate-3 replays passed separately on the idle Docker host at the repository's unchanged **0.1 mm** comparison tolerance; their final outcomes are recorded in 10.7. They cannot pass any omitted physical air cut. The K4 repeat used 0.05 mm, stricter than that repository default.

Facilities unavailable here: native Windows/macOS execution, direct Z1 access, and an installed OpenCASCADE 7.8.1 kernel. The existing lane's earlier ten passing STEP-pipeline kernel tests remain historical evidence; this prepared code has not rerun them. Current integrated-head kernel CI and platform checks remain required. The owner's Windows/Z1 0.20.0 results are preserved independently from this Linux 0.21.0 verification. Generated G-code remains the user's responsibility to verify (NFR-12).

### 10.7 Local reference-program LinuxCNC replay results before bounded first-cut support

All **11 selected programs** (ten reference jobs plus the small-part fixture) passed on the managed Docker host using the image/digest in 10.4 and unchanged repository tolerance **0.1 mm** (or degrees for A); final positions retain the checker's separate 0.001 tolerance. This is the default `xtask` program selection with only `first_cut_linuxcnc.ngc` withheld under OI-991. At this earlier verification stage the full first-cut gate-3 replay remained **not run**; the subsequent owner-authorized bounded implementation and evidence are recorded below. These passes do not supply hardware evidence for the eight waived air cuts.

The temporary reproduction harness copied unmodified `checks.ini`, `sim.tbl` and `sim_driver.py`, and the exact committed program into each `/tmp/gates-gate3/<filename>/` directory, with permissions readable by the image's unprivileged user. It ran the same `docker ... linuxcnc checks.ini` command and then `target/debug/openchuck-cli gcode-sim <program> <report> --tolerance 0.1`. Heavy build/coverage workloads were stopped for the final replays. Reports/logs are temporary local artifacts; the following summary and report hashes preserve the evidence identity in the repository.

| LinuxCNC program | Result | Recognised moves reached | Largest difference | Reported motion s | Docker wall s | Report SHA-256 |
| --- | --- | --- | --- | --- | --- | --- |
| post_linuxcnc_small_part.ngc | passed; no fault | 30 | 0.1000 | 29.6 | 44.215 | `a85d9a4f3dedbece16d749fbac7b339110366b7a840009668f078e5dd333aa15` |
| cone_axial_lead_linuxcnc.ngc | passed; no fault | 1345 | 0.0536 | 286.4 | 317.552 | `ab886c0e0af81be494c3d62bbad02d1c75227758f426be1ef6ffd5f7f67ee8a9` |
| cone_axial_linuxcnc.ngc | passed; no fault | 609 | 0.0999 | 271.2 | 288.515 | `2a12342e5b4ec0768cfbe880f0b715eb86cf0dc83bce07914272e512bcfa3c01` |
| cone_held_tabs_linuxcnc.ngc | passed; no fault | 1591 | 0.1000 | 171.1 | 196.048 | `256e5f473510031222ce86169b2db38009f9c121e9fcfccd1a5fcb46c43a09cf` |
| cone_indexed_linuxcnc.ngc | passed; no fault | 77 | 0.0067 | 36.2 | 57.916 | `1b2288559573a0d91b1f5e350c53fd8153be8baa1606ad366c8698c443259760` |
| cone_linuxcnc.ngc | passed; no fault | 1086 | 0.0150 | 117.7 | 141.676 | `b03943da8a666267b2e7ab7992a3c54a5fab0c599997c8d95133e33b072effa5` |
| drilling_linuxcnc.ngc | passed; no fault | 15 | 0.0010 | 23.3 | 43.096 | `2a69ef2432baa21e3d1f6c8f6686de64f468a21555b6c7ddd7cd553f1c0e1258` |
| groove_linuxcnc.ngc | passed; no fault | 1104 | 0.0150 | 152.9 | 176.608 | `d33f237a7b2f78e8b7dcb00ac00e9ac17e7d8b9bb43ac3a22e85f34187fe40b0` |
| lobed_linuxcnc.ngc | passed; no fault | 1806 | 0.0920 | 105.6 | 131.348 | `6908a091013f9f6d498039e54e7852a319bed1637531955437ea64829bdb2902` |
| sphere_linuxcnc.ngc | passed; no fault | 726 | 0.0150 | 87.8 | 109.496 | `cedd62cfe3c198337df4aa0e42d5ea2f5cf2625e0aaf92b43b35c60e162ff7a4` |
| swarf_linuxcnc.ngc | passed; no fault | 54 | 0.0041 | 105.6 | 126.108 | `1b713a8e0cfa6bff6d17761f5ad621501ea0fb5882ec183ced26138caaf7ae69` |

An earlier cone-axial attempt failed comparison at line 314, expected `[0, 0, 45, 180]`, with no LinuxCNC errors, maximum sampling gap 97.8 ms and wall time 295.818 s. One attempt was interrupted when the host was busy with coverage; it supplied no result. The idle-host repeat in the table passed. The earlier failure is retained; neither a tolerance change nor a product fix was used to obtain the repeat pass. An initial temporary wrapper also mistakenly selected a Z1 `.ngc` file; LinuxCNC rejected its Z1-specific `G92.4`. Selection was corrected to the repository's `linuxcnc` filename rule before these final reference results. It is not a LinuxCNC-post product failure.

### 10.8 Owner-authorized bounded first-cut follow-through

The owner instructed “Apply your suggested approach to allow support of the test” after the explanation of the first-cut gate-3 duration gap. This authorizes the bounded approach. At completion the complete record [0244](../../decisions/0244-bounded-first-cut-linuxcnc-replay.md) remained proposed; the owner accepted its full text separately on 26 Oct 2026 (section 10.11). Decisions 0221, 0222 and 0232 remain accepted.

The first-cut source expands to **13 deterministic simulator fixtures**. Their contiguous source ranges reconstruct every original line exactly once, in order. Both driver and wrapper enforce a limit of at most 600 s per unit, or a smaller requested timeout; the planner targets 300 s including startup. Each fixture restores and checks its original boundary state. The independent `gcode-sim --source-lines FIRST:LAST` path replays the original unsplit prefix, checks both actual boundary states and compares the original moves in that range. It therefore checks the fixture against the original program. Production G-code and comparison tolerances are unchanged. The public verification API additions use the library version 0.22.0 after released main integration; ordinary report JSON remains compatible through default-empty boundary data. No machine/project schema is added.

**Observed initial tooling failure:** part 1 passed in 237.846 s. Parts 2–5 each failed twice at their starting state check, with the following values. Part 6 was interrupted while the issue was investigated; it supplies no result. Parts 7–13 were not run in that attempt. The observations and first reports are retained under the initial-attempt evidence; none is retroactively passed.

| Part | Expected boundary F | Reported F | Attempt 1 wall s | Attempt 2 wall s |
| --- | --- | --- | --- | --- |
| 2 | 16.7 | 17.3 | 20.234 | 20.229 |
| 3 | 18.1 | 299.9 | 19.218 | 18.621 |
| 4 | 19.7 | 20.7 | 19.019 | 18.422 |
| 5 | 21.7 | 21.6 | 19.022 | 19.625 |

The reported F matched the next source line, with about a 1 ms sampling gap. The conclusion is that interpreter-reported settings look ahead across M0; this is a checkpoint assumption in the test tooling, not a new physical machine result. Each checkpoint now has a LinuxCNC-only `M66 E0 L0` queue barrier after M0, preventing that read-ahead before the paused state is checked. A focused real-simulator probe then checked F60 before an original line changed it to F120 and passed the independent original-source comparison. No feed, coordinate or comparison tolerance was relaxed. A separate negative probe deliberately supplied a wrong expected tool and stopped before original motion.

Local regressions passed: 19 gcode tests, 28 CLI command tests, 97 xtask tests, and the Python boundary-helper tests. Uninstrumented own-test execution was under a second per suite (build time excluded); no new Rust/nextest case exceeded a minute. The external simulator units are timed separately and distributed by the gate-3 harness rather than nextest. The legacy whole-program small-part replay passed in 52.168 s; a short fixture exercised the complete bounded harness in 23.435 s. Audit and deny passed in 5.798 s and 1.862 s. Final document-freeze checks passed: `cargo xtask all` 6.815 s (records, testdata and 100/100 P1 traceability included), all-target Clippy for eight relevant crates with warnings denied 9.054 s, formatting 1.505 s, whitespace clean. The final own-test executions passed: gcode 19 (including 0.25 s reader properties), CLI command tests 28 in 0.17 s, xtask 97 in 0.21 s.

Own-test line-coverage lower bounds for this follow-through are gcode 861/1,116 (77.15%), CLI command tests 946/1,406 (67.28%), and xtask 2,528/3,364 (75.15%); these whole-crate totals remain below the 95% development target. The new independent range module covers 163/171 lines (95.32%); the planner covers 315/322 lines (97.83%). Remaining new gaps are defensive parse/impossible-state paths and the external execution paths exercised by uninstrumented LinuxCNC runs. Focused Python unit/short-runtime coverage is 36/37 executable lines (97.30%) for the boundary helper and 98/106 (92.45%) for the driver. These standard-library trace counts include the unexecuted synthetic module-prologue line 0; it is not source code. Other driver gaps are defensive timeout/homing/extra-checkpoint paths and legacy-control paths exercised uninstrumented. LLVM-instrumented targets were cleaned after reporting; trace evidence is retained outside the repository. Instrumented runtime timings are not used as performance evidence.

The synchronized bounded run passed all thirteen fixtures on their first attempts. All 16,853 original lines reconstruct byte for byte, every adjacent expected boundary agrees, and all twelve actual boundary transitions match. The independent comparisons reached all 16,827 recognised original moves with no faults. Maximum per-unit wall time: 270.312 s; sum of unit wall times: 2,441.792 s. The sequential command batch took 2,448.126 s (40 min 48.126 s), including repeated harness setup. No execution unit exceeded ten minutes. OI-991 is closed. This software evidence does not close the physical reference-program waivers, the override discrepancy, OI-267 or the unrecorded Gate 5 measurements. Native Windows/macOS, installed OpenCASCADE kernel execution and direct Z1 access remain unavailable locally.



The bounded evidence was also assembled in original-source order and compared once against the complete original program: all 16,827 recognised moves reached, largest difference 0.1000, no fault (3.126 s comparison command). This assembled comparison is subsequent bounded evidence, not an uninterrupted simulator execution. Actual transition checks retain the original coordinates/feed/tool/spindle state across all twelve junctions.

Software-fixture SHA-256: `d42a2e229b631794fb37b25086e464164b8c4dd364cc2111172a9e78b287bd7e`. This identifies the stored program verified here; it does not fill the owner's blank physical program/setup fingerprints. The stored fixture includes S10000 roughing and S12000 finishing. The owner's single 10000 rpm entry does not identify every physical operation's speed; per-operation physical settings remain unrecorded under OI-471.

| Part | Original source lines | Recognised moves | Largest difference | Wall s | Report SHA-256 |
| --- | --- | --- | --- | --- | --- |
| 1 | 1–872 | 862 | 0.0175 | 236.797 | `be80eb1876ef7772af791dddf3d82727045134e4475f65de93c4abeedec71aa4` |
| 2 | 873–2020 | 1139 | 0.1000 | 224.992 | `d0d621aef9f5e28de212c091f1906d5cb4760c01f25ee72e92b7fae5756b7d10` |
| 3 | 2021–3497 | 1477 | 0.1000 | 209.736 | `3ed65b52a6815824eeadd2872221217d952cff79b04efc6a0ed7d15e7551b389` |
| 4 | 3498–5309 | 1812 | 0.0442 | 192.267 | `f9301c609ca59aa058850e1392009286024e0f40f9ff730cc441cfc9a26c8905` |
| 5 | 5310–7183 | 1874 | 0.0150 | 191.052 | `a2e2ce32e8e07aecc795c453b6965535831b68b8ea86277cb4aa24e9263edfd3` |
| 6 | 7184–9052 | 1869 | 0.1000 | 190.289 | `feee2b44465f756a526b76acc9fc84cfb1f9a01c547ab61711834e04340be654` |
| 7 | 9053–10893 | 1841 | 0.0150 | 191.661 | `b7ab8b6ce79528329f2495765af9206b7568f1f9f19285ed79431a5e7e303cc6` |
| 8 | 10894–12598 | 1705 | 0.0150 | 196.482 | `7a7d4d4a09042c1374cab0d638bf5245ea128fe07fd676b6d014764d5733399e` |
| 9 | 12599–12738 | 140 | 0.0150 | 46.411 | `c698c619fac565a83a0bd8f32d28a2ebecbf59b456d7cce9641394d1ee4aef06` |
| 10 | 12739–12909 | 167 | 0.0150 | 270.312 | `67c70babea208fd7fc2e8d6bd5bf39cb9f773ca66effc684b48dbffc17e67f3f` |
| 11 | 12910–14643 | 1733 | 0.1000 | 197.087 | `9c959febe80d02023497437d82a1a7f81802b6d5a707d57c3e9af7135ba2ab09` |
| 12 | 14644–16552 | 1909 | 0.1000 | 189.055 | `f21335a0161f7436be42c98a732425a525f50fc8782d726d805d27dcfa7e1955` |
| 13 | 16553–16853 | 299 | 0.0150 | 105.651 | `209cab39f8e421a92dc141f681e1f63bfa36fd7486fef2cffb3040c99bfab63a` |

Historical input for this section: [first-cut LinuxCNC source at preparation commit d409e24](https://github.com/paulhampson/OpenChuckCAM/blob/d409e2483bf978b8d86a46ed6b5f3a75fef78052/testdata/golden/first_cut/first_cut_linuxcnc.ngc), SHA-256 `d42a2e229b631794fb37b25086e464164b8c4dd364cc2111172a9e78b287bd7e`. Extract that revision as `first_cut_linuxcnc.ngc` to reproduce these historical ranges; released main integration changes this input under accepted decision 0241 (10.9). Reproduction on an otherwise idle host: `cargo xtask replay --docker ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 --keep-reports target/replay --timeout 600 testdata/golden/first_cut/first_cut_linuxcnc.ngc`. The local run used each execution share exactly once (`2/13`, `3/13`, `1/13`, then `4/13` through `13/13`) to check the previously failing ranges first. All share assignments use the same deterministic expansion; CI distributes the full reference/first-cut set over its existing three shares. Raw reports, expected-state/source manifests, simulator INIs, per-attempt timings and the assembled evidence remain temporary local artifacts; the summaries/hashes above preserve their identity in the repository. The Docker image/digest is the one recorded in 10.4.

### 10.9 Released main integration, 26 Oct 2026

The [Merge Manager’s release comment 6410493723](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6410493723) marks the PR ready before one main merge and one push under decision 0055. Main `e2189f07fe8dee5a1bd15300babed08f89e6e853` is integrated once. The union retains every upstream decision/open-item ID and newer behavior, including OPSPEED’s real schema-18/machine-4 migrations and spindle limits, CHECKPOINT, SIMUI, OUTPUT-1, rotary boundaries, and the dependency fixes. Duplicate and resurrected rows are removed and the registers are ordered numerically. Library **0.22.0** is the next free minor after main 0.21.0; project **19**, machine **5**, decision **0244** and **OI-991** remain free across main and unmerged remote branches. The work-breakdown claim precedes the version edit.

Accepted decision **0241** intentionally changes the LinuxCNC first-cut golden: remove the inter-operation `A0.000` physical rewind and the terminal `A165.352` physical unwind, and add **82,080 degrees** to every finishing A word so rotary position continues from roughing. A mechanical comparison of the normalized goldens verifies that every other source word, feed, and comment is unchanged. The Makera first-cut golden is unchanged. The new LinuxCNC source has **16,851 lines**, SHA-256 **`2811025b2db1122d153647f8e64aeaad0a7e73d057d524181bd259274cf078d7`**. Section 10.8 retains the earlier source, failures and passing results; those counts are not attributed to the new source. This is current software verification and does not identify the owner’s un-fingerprinted physical program. Report itself still changes no G-code.

The first integrated run completed 973 tests in 1,033.580 s: 970 passed, with three failures. Two were old fixture assumptions: schema 18 hard-coded as current, and a pre-schema-5 spindle fixture containing the new GC-17 fields. Their corrections retain spindle-schema refusal and future-project-schema refusal; the recovery test also checks the current Z1 profile’s block-time/spindle fields together with the schema-18 ramp option. All 170 project tests then passed (0.745 s test time, 1.433 s including compilation), with changed-project all-target Clippy passing (0.300 s). The third failure was the expected first-cut LinuxCNC golden difference described above; its six-process determinism check then **passed in 293.759 s** against the reviewed new golden (309.261 s including compilation). All 973 selected test cases therefore have a passing result across the initial run and focused reruns. All **483 UI tests passed** in **363.005 s**, maximum **128.495 s**, already classified long.

Current integrated own-test coverage was rerun for types, project, post, gcode and xtask after uninstrumented tests: **384 tests passed**, 1.272 s instrumented test time / 41.657 s including compilation. Lower bounds are types **82.64%**, project **90.63%**, post **87.41%**, gcode **91.14%**, xtask **75.15%**. New Report analysis is **121/123 lines (98.37%)**, original-range comparison **163/171 (95.32%)**, and fixture planner **315/322 (97.83%)**. Whole-crate figures remain below the 95% development target; it is not a PR merge gate. Prior app/CLI coverage is retained as preparation evidence, with the new executable modules unchanged at integration. The dedicated instrumented target was cleaned and then removed, including its dependency artifacts. Two public documentation examples also passed (20.748 s including compilation).


#### Integrated-input LinuxCNC results

All **22 execution units passed**: **11 bounded first-cut fixtures** and **11 reference/small-part programs**. The default full replay finished in **65 min 32.958 s**; 23 attempts total, maximum **313.064 s**, all below 600 s. The Docker image/digest and 0.1 mm/degrees path / 0.001 final-position tolerances are unchanged. No physical gate waiver or failed physical block-rate section is converted to a pass.

First-cut fixture 1 initially failed its path comparison at source line 542 (`[79.711, 0.0, 23.721, 11015.0]` not observed after the previous move), with a **66.9 ms** maximum sampling gap and **233.708 s** wall time. Its existing single retry passed all 862 moves in **247.342 s**. The failed report remains retained (SHA-256 `1c2c2343d7d751223f03cf77f0e1fc179c9d8de3b549b188312f62137a88b07a`); it is not retroactively passed. Every other execution unit passed its first attempt.

All **16,851 original lines reconstruct exactly**, and all **16,825 recognised moves** pass the independent source-range comparisons. Across ten actual transitions, positions agree within 0.001, modal F/S within 1e-6, and tool, spindle state/direction and effective coordinate offsets agree. Required G modes match. An initial auxiliary comparison of complete NML G-code vectors flagged **G92.2 (922) at each end versus unset at each fresh start**. Both effective G92 offsets are exactly zero; every other active code agrees. The supported source subset contains no G92 commands, so this flag cannot affect this input. This distinction remains in the raw snapshots and collected observations; full opaque interpreter-state identity is not claimed, and no production check or tolerance was relaxed.

The concatenated actual path also passes the ordinary whole-source comparison: all **16,825 moves**, maximum **0.1000**, **1,812.5 s** reported motion, **2.763 s** comparison wall time. Assembled report SHA-256 `93bf4acc76a65caae7ee506e8366916ad46853760b19521a0be55cbf37ea2cdb`. This combines separate fixtures; uninterrupted dynamics and historical pre-cut ordering remain unverified.

| Fixture | Original lines | Moves | Attempts | Final wall s | Max difference | Final report SHA-256 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 1–872 | 862 | 2 | 247.342 | 0.0150 | `78fc3b0f3c43daf208ee8094ab6e9b099a3ded0067cf1918c30b1af7cc84ff54` |
| 2 | 873–2020 | 1139 | 1 | 234.868 | 0.1000 | `730b41bc1bf224b7a8d6e2cf7448e68af79382d9b94214a239510d7ddd7aa5c7` |
| 3 | 2021–3497 | 1477 | 1 | 208.338 | 0.0255 | `208ed1931dedfb3b69a99d9b36d5344114b3be2e6ae0a31c72eef559b37b1fe3` |
| 4 | 3498–5309 | 1812 | 1 | 193.758 | 0.0150 | `61ac86e6325ce54c8ceae6f2e787cd029172e9a3cc980978d3ff87034eac76be` |
| 5 | 5310–7183 | 1874 | 1 | 189.662 | 0.0150 | `e03c85b09fb2f6c9817aa689cefab5bba6f25101e972a70aeff688649ff4adf3` |
| 6 | 7184–9052 | 1869 | 1 | 189.904 | 0.1000 | `8224747e662b0c7512a3c6726aaa1172ddf2a061f856bfc23d6a0e4d3534d03f` |
| 7 | 9053–10893 | 1841 | 1 | 193.044 | 0.0150 | `156fb43051b981830bd4794d7c69797d1809b6a691845d84236cbdcfad3439b9` |
| 8 | 10894–12598 | 1705 | 1 | 194.639 | 0.0150 | `1c37879995f1b08fba5c20f1e09cdca5da79bce101a8037d7a0a084d4294b548` |
| 9 | 12599–14229 | 1626 | 1 | 200.718 | 0.0257 | `0a2232f940bf6b536791255c3190d2b81bb1244fbb8bcfd86ae86d4e7dd81896` |
| 10 | 14230–16120 | 1891 | 1 | 189.075 | 0.0150 | `8d57261f0adc8e50b9ab423e9f56cffd4cd2999aa5f4476cc4afa1cdc27cca02` |
| 11 | 16121–16851 | 729 | 1 | 86.202 | 0.0150 | `b091105ea4458b37a74102e8c9b10172f69a523cc8736c7be66ca9928b2e2045` |

| Other program | Moves | Wall s | Max difference | Report SHA-256 |
| --- | --- | --- | --- | --- |
| `post_linuxcnc_small_part.ngc` | 29 | 46.140 | 0.0010 | `7008df5ff5882b6c0892f36ac0746dd6c29e18c379c716d677b356e46261f61a` |
| `cone_axial_lead_linuxcnc.ngc` | 1344 | 313.064 | 0.0448 | `a6052229b7802dffd39b8723fba27cf73ad7165cbbe8858b3cc850c70e68d966` |
| `cone_axial_linuxcnc.ngc` | 608 | 295.146 | 0.0132 | `6b674e8cff8018886f9287ca9e31f2a6d502aecbcd713ecd258dd524671bf5b6` |
| `cone_held_tabs_linuxcnc.ngc` | 1590 | 174.537 | 0.1000 | `a56050ca04ac6107f2e2c8a3204b217be22147f79d12aa578eb0577909ae1890` |
| `cone_indexed_linuxcnc.ngc` | 76 | 56.581 | 0.0091 | `e35106738ab2d2d53baa6a423711968af79e4372ea40cc4f78084c0354a3968f` |
| `cone_linuxcnc.ngc` | 1085 | 125.106 | 0.0230 | `13395a0a5d21d6bea15e3f986930ee384c0d4496a50bb93252d8eea3a7a6c4d2` |
| `drilling_linuxcnc.ngc` | 15 | 42.129 | 0.0012 | `647a37e3af579c70bf7431f541ae2f468d96d0fa7edac093f656ea34b8081ba3` |
| `groove_linuxcnc.ngc` | 1103 | 162.075 | 0.1000 | `bde10a5657525948c25d4ac54827d180811e7a95e458c82fb971f98763147579` |
| `lobed_linuxcnc.ngc` | 1805 | 121.735 | 0.1000 | `ef67db6577bc20e3549285533bad191ad9393dee11e9be3f98352c2a86963d5f` |
| `sphere_linuxcnc.ngc` | 725 | 101.580 | 0.0150 | `05c67d34bce0c87a29c29c090777cc49e07d9f6c9d16f6b6f2cecc9c4d1fa90c` |
| `swarf_linuxcnc.ngc` | 54 | 127.088 | 0.0119 | `13087df27d8f0c07db038f303f3a7b2cb5da117acd5900affcc625c1d525cd83` |

Reproduction: `cargo xtask replay --docker ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 --keep-reports target/replay --timeout 600`. The full default set expands before sharding. Current local raw reports, source/state manifests, copied fixtures/INIs, attempt timings, assembled report and collected observations are retained under `/workspace/shared/gates-integrated-replay-20261026`; source identities, outcomes, timings and report hashes are preserved here.

Final released-integration checks: `cargo xtask all` **6.793 s** (records, dependencies, licences, conflicts, test data, publish checks and **100/100 P1 traceability**); `cargo fmt --all --check` **1.685 s**; all-target Clippy for the eight relevant crates with warnings denied **11.755 s**; Python boundary tests **2 passed**. Audit **6.783 s** and deny **2.882 s** passed with the unchanged dependency lock. Whitespace passes for the GATES text/code diff; native STEP/PDF format whitespace is preserved. At push preparation, **25 current-main/unmerged refs** were re-audited: main remains the released `e2189f0`, and library 0.22.0 / project 19 / machine 5 / decision 0244 / OI-991 have no competing allocation. No accepted decision, K4 waiver, remaining physical-evidence item or unavailable-platform claim changes.


### 10.10 CI review and correction batch, 26 Oct 2026

The [Merge Manager’s correction instruction](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6413054226) authorizes one further push after reviewing CI on `f9d947b573c05be48dd037f8677148168860bdd4`; it requests no main update or owner-policy change. Completed [CI run 40301293452](https://github.com/paulhampson/OpenChuckCAM/actions/runs/40301293452) has two failure causes:

- Fuzz smoke C cannot compile `post_definition`: its `PostJob` constructor omits `block_time`. The separate fuzz workspace was omitted from the earlier local workspace checks. The local all-target check reproduced E0063 before the fix. The constructor now supplies default **0 ms / 1 / Off**, preserving its prior behavior.
- All four Windows/macOS test shards fail the nine generated committed-project validations (eight reference jobs plus first cut), because their freshly generated STL bytes do not match the hash recorded on the fixture-authoring platform. Native floating-point math can vary those bytes. The exporter now hashes the file it just created and updates only the new exported project through the ordinary project API. Committed fixtures, IDs, parameters, copied STEP models and all G-code goldens remain unchanged. This does not auto-repair an existing user project or disable PRJ-02.

The new requirement regression first failed with a valid STL byte variation and then passed. It verifies the exported model’s actual hash, every other project value/ID, unchanged committed source, and detection of an edit made after export. All **ten focused cases passed** (the nine affected cases plus the regression) in **313.713 s**; first-cut both-post/1/2/N-thread determinism passed in **301.461 s**, already classified long. **28 CLI command tests** passed in **0.204 s**.

Every **11-target** fuzz bin compiles (`cargo check --locked --all-targets --manifest-path fuzz/Cargo.toml`, **0.111 s**) and benchmark consumers including both benchmarks compile (`--manifest-path benches/Cargo.toml`, **2.979 s**). CLI/benchmark all-target Clippy with warnings denied passes (**8.120 s**); changed `post_definition` Clippy also passes (**0.096 s**). An additional optional whole-fuzz lint run reports the pre-existing `collapsible_if` in CHECKPOINT’s `project_recovery.rs:21`, byte-identical to released main; that unrelated style issue is left unchanged.

Nightly **1.101.0-nightly (76456a688, 2026-10-25)** with cargo-fuzz **0.13.2** and ASan completed the post-definition smoke run: **311,080 executions in 61 s**, no crash or timeout; **161.133 s** including compilation. Local smoke minutes are not added to the nightly CPU-hour totals. CLI own-test coverage rerun passed **29 cases** (28 commands plus the new regression) in **0.328 s**, **56.224 s** including compilation. Product CLI coverage is **948/1306 lines, 72.59%**, a lower bound below the report-only target. The shared test/example helper is excluded from the standard product report; its new behavior is directly requirement-tested. No product library or CLI implementation source changes in this batch. Dedicated ASan and coverage build directories were cleaned and removed. Final `cargo xtask all` passed in **14.842 s**, including records, test-data validation and **100/100 P1 traceability**; root formatting passed in **1.743 s**, separate fuzz formatting and whitespace also pass. Dependency manifests and locks are unchanged from the audited parent.

The completed parent run passed all three Gate 3 shares, all three STEP-kernel jobs, all three library-isolation jobs, UI smoke on all three platforms, Linux build/test shards, fast checks, fuzz A/B, and the separate CLI end-to-end workflow. These are parent-head results; Windows/macOS validation on the correction head still needs CI. The three new CodeRabbit findings (source-label overrides, pause-resume race and symlinked-temp boundary lookup) remain **deferred**, not fixed, as instructed; the trivial future-enum hint is also deferred. In particular, symlinked temporary-directory contexts remain an unverified boundary-check limitation. At this correction handoff, gate observations, waivers, measured 7 ms/window 32/Report, remaining physical items and proposed 0244 status were unchanged. The subsequent acceptance is recorded in section 10.11.


### 10.11 Complete-record acceptance and merged lane, 26 Oct 2026

The owner directly instructed **“Accept 0244”** in the GATES takeover session on 26 Oct 2026. Decision 0244 is accepted in full; this is separate from the earlier implementation authorization. The decision index, architecture, verification entry, work breakdown and OI-991 closure reference record that acceptance. The decision's substance, watchdogs, tolerances and documented limitations are unchanged. Acceptance does not turn waived or unexecuted physical checks into passes, establish uninterrupted execution or historical pre-cut ordering, or expand the K4 waiver. Generated G-code remains the user's responsibility to verify (NFR-12).

The [Merge Manager's final comment 6419593383](https://github.com/paulhampson/OpenChuckCAM/pull/284#issuecomment-6419593383) records all **26 CI jobs green** on the final attempt and merges PR #284 as `0620cd0f73c89d3b56c120cdcb784847f70b67b1`, from lane head `0bf14f5338d7ed01fd046368776df0b5d884c9c7`. The CHECKPOINT revision-namespace test failed once on Linux and passed the permitted rerun; that flake remains CHECKPOINT's follow-up. Both Windows and both macOS test shards, all three Gate 3 shares, all three STEP-kernel jobs, fuzz A/B/C, fast checks, library isolation, UI smoke, CLI end-to-end, CLA and required checks passed. These final results supersede section 10.10's pending-CI status. Physical evidence gaps, remaining open items and deferred review findings retain their recorded status.


## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | PH |
| Date | 2026-10-25 |
| Machine and firmware | Makera Z1, 1.1.2 |
| Changes required | Owner: No. Evidence audit: unresolved exceptions above. |
| Reviewed: yes / no | Yes (owner submission); local selected checks and bounded first-cut replay passed; physical evidence and unavailable-platform gaps remain |
