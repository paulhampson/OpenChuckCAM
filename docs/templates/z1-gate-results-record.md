# Z1 gates 4 and 5: results and review record (template)

Copy this file to `docs/archive/<NNNN>-<name>/` (or into the Verification section of the decision record that takes the results) when the owner submits the results of a run of [gates 4 and 5](../z1-gates-4-5-test-instructions.md), and fill it in. It is laid out like the section 10 and Review record that were moved out of the instructions to [archive/0244-z1-gates-4-5](../archive/0244-z1-gates-4-5/README.md). Replace every `<...>`; delete rows that do not apply only with a note saying why. The step IDs are those of the instructions (P1 to P10, R1 to R12, S1 to S8, G4-0 to G4-8, B1 to B14 for gate 4; F1 to F6, G0, W1 to W5, T0 to T5, V1 to V6, M1 to M6, C1 to C7, I1 to I5 for gate 5).

**Generated G-code is the user's to verify before running on a machine (NFR-12).** A passing result here lowers the risk; it does not say that a program is safe or correct for any machine, tool or setup.

## Results

Evidence: <link to the owner's submission, with its comment ID>, reproduced verbatim in the section "Verbatim owner submission". Operator and date of gate 4: <name, date, or "blank in the submission; do not infer from gate 5">. Operator and date of gate 5: <name, date>. Software: OpenChuckCAM <version>, commit `<full hash>`, built <date and time>, rustc <version>, <target>. Z1 firmware: <version>. Controller / upload path: <e.g. ESP32 firmware version, WiFi or USB>. Video: <shared / not shared>. State here whether the tested commit is the one under review and whether the exact programs and setup fingerprints were recorded; if not, say that the claim "the committed golden programs were run" cannot be made.

Classification follows the pass conditions of the instructions and verification plan section 6: **passed** means the submitted evidence supports that check; **failed** means an observed pass-condition violation (name the defect issue); **waived** means explicitly omitted by the owner (not a pass; name the owner's words and date); **not run** means no completed verification of that step is established by the submission, including blank or insufficient fields (it does not assert that an unreported physical action never occurred); **not applicable** means a stated condition does not apply. A partial observation is kept in Notes and never used to infer the rest of the check. Record a surprise that changes an instruction in the step above and in the instructions in the same change, and say so in the row.

### Gate 4

<Summary: which programs were run, at which override, alarms, whether motion and end positions matched the simulation, timing against the prediction (numbers, not "as predicted"), and anything the owner reported that contradicts the accepted checklist.>

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| P1 | passed / failed / waived / not run / not applicable | | |
| P2 | passed / failed / waived / not run / not applicable | | |
| P3 | passed / failed / waived / not run / not applicable | | |
| P4 | passed / failed / waived / not run / not applicable | | |
| P5 | passed / failed / waived / not run / not applicable | | |
| P6 | passed / failed / waived / not run / not applicable | | |
| P7 | passed / failed / waived / not run / not applicable | | |
| P8 | passed / failed / waived / not run / not applicable | | |
| P9 | passed / failed / waived / not run / not applicable | | |
| P10 | passed / failed / waived / not run / not applicable | | |
| R1 | passed / failed / waived / not run / not applicable | | |
| R2 | passed / failed / waived / not run / not applicable | | |
| R3 | passed / failed / waived / not run / not applicable | | |
| R4 | passed / failed / waived / not run / not applicable | | |
| R5 | passed / failed / waived / not run / not applicable | | |
| R6 | passed / failed / waived / not run / not applicable | | |
| R7 | passed / failed / waived / not run / not applicable | | |
| R8 | passed / failed / waived / not run / not applicable | | |
| R9 | passed / failed / waived / not run / not applicable | | |
| R10 | passed / failed / waived / not run / not applicable | | |
| R11 | passed / failed / waived / not run / not applicable | | |
| R12 | passed / failed / waived / not run / not applicable | | |
| S1 | passed / failed / waived / not run / not applicable | | |
| S2 | passed / failed / waived / not run / not applicable | | |
| S3 | passed / failed / waived / not run / not applicable | | |
| S4 | passed / failed / waived / not run / not applicable | | |
| S5 | passed / failed / waived / not run / not applicable | | |
| S6 | passed / failed / waived / not run / not applicable | | |
| S7 | passed / failed / waived / not run / not applicable | | |
| S8 | passed / failed / waived / not run / not applicable | | |
| G4-0 | passed / failed / waived / not run / not applicable | | |
| G4-1 | passed / failed / waived / not run / not applicable | | |
| G4-2 | passed / failed / waived / not run / not applicable | | |
| G4-3 | passed / failed / waived / not run / not applicable | | |
| G4-4 | passed / failed / waived / not run / not applicable | | |
| G4-5 | passed / failed / waived / not run / not applicable | | |
| G4-6 | passed / failed / waived / not run / not applicable | | |
| G4-7 | passed / failed / waived / not run / not applicable | | |
| G4-8 | passed / failed / waived / not run / not applicable | | |
| B1 | passed / failed / waived / not run / not applicable | | |
| B2 | passed / failed / waived / not run / not applicable | | |
| B3 | passed / failed / waived / not run / not applicable | | |
| B4 | passed / failed / waived / not run / not applicable | | |
| B5 | passed / failed / waived / not run / not applicable | | |
| B6 | passed / failed / waived / not run / not applicable | | |
| B7 | passed / failed / waived / not run / not applicable | | |
| B8 | passed / failed / waived / not run / not applicable | | |
| B9 | passed / failed / waived / not run / not applicable | | |
| B10 | passed / failed / waived / not run / not applicable | | |
| B11 | passed / failed / waived / not run / not applicable | | |
| B12 | passed / failed / waived / not run / not applicable | | |
| B13 | passed / failed / waived / not run / not applicable | | |
| B14 | passed / failed / waived / not run / not applicable | | |

### Gate 5

<Summary: the job, material and tools, the result against the limit of the accepted decision, measurements, what remained uncut, and anything unexpected.>

| Step | Result | Notes and measurements | Follow-up |
| --- | --- | --- | --- |
| F1 | passed / failed / waived / not run / not applicable | | |
| F2 | passed / failed / waived / not run / not applicable | | |
| F3 | passed / failed / waived / not run / not applicable | | |
| F4 | passed / failed / waived / not run / not applicable | | |
| F5 | passed / failed / waived / not run / not applicable | | |
| F6 | passed / failed / waived / not run / not applicable | | |
| G0 | passed / failed / waived / not run / not applicable | | |
| W1 | passed / failed / waived / not run / not applicable | | |
| W2 | passed / failed / waived / not run / not applicable | | |
| W3 | passed / failed / waived / not run / not applicable | | |
| W4 | passed / failed / waived / not run / not applicable | | |
| W5 | passed / failed / waived / not run / not applicable | | |
| T0 | passed / failed / waived / not run / not applicable | | |
| T1 | passed / failed / waived / not run / not applicable | | |
| T2 | passed / failed / waived / not run / not applicable | | |
| T3 | passed / failed / waived / not run / not applicable | | |
| T4 | passed / failed / waived / not run / not applicable | | |
| T5 | passed / failed / waived / not run / not applicable | | |
| V1 | passed / failed / waived / not run / not applicable | | |
| V2 | passed / failed / waived / not run / not applicable | | |
| V3 | passed / failed / waived / not run / not applicable | | |
| V4 | passed / failed / waived / not run / not applicable | | |
| V5 | passed / failed / waived / not run / not applicable | | |
| V6 | passed / failed / waived / not run / not applicable | | |
| M1 | passed / failed / waived / not run / not applicable | | |
| M2 | passed / failed / waived / not run / not applicable | | |
| M3 | passed / failed / waived / not run / not applicable | | |
| M4 | passed / failed / waived / not run / not applicable | | |
| M5 | passed / failed / waived / not run / not applicable | | |
| M6 | passed / failed / waived / not run / not applicable | | |
| C1 | passed / failed / waived / not run / not applicable | | |
| C2 | passed / failed / waived / not run / not applicable | | |
| C3 | passed / failed / waived / not run / not applicable | | |
| C4 | passed / failed / waived / not run / not applicable | | |
| C5 | passed / failed / waived / not run / not applicable | | |
| C6 | passed / failed / waived / not run / not applicable | | |
| C7 | passed / failed / waived / not run / not applicable | | |
| I1 | passed / failed / waived / not run / not applicable | | |
| I2 | passed / failed / waived / not run / not applicable | | |
| I3 | passed / failed / waived / not run / not applicable | | |
| I4 | passed / failed / waived / not run / not applicable | | |
| I5 | passed / failed / waived / not run / not applicable | | |

### Verbatim owner submission

Source evidence, including any shorthand such as "all steps passed". The shorthand is not the audited classification; the two tables above govern. Keep blank fields and N/A unchanged; trailing whitespace may be normalised.

<details>
<summary>Owner comment <ID>, <date></summary>

````text
<paste the submission unchanged>
````

</details>

### Register audit after the results

| Item | State retained | Evidence and next action |
| --- | --- | --- |
| OI-<n> | Open / Closed | <evidence; next action> |

### Verification of the folding lane

| Check | Outcome | Time |
| --- | --- | --- |
| <check> | <outcome> | <seconds> |

## Review record

| | |
| --- | --- |
| Reviewed by (Z1 operator) | <name> |
| Date | <yyyy-mm-dd> |
| Machine and firmware | <machine, firmware> |
| Changes required | <none / list, with the issue for each> |
| Reviewed: yes / no | <yes / no, and what remains> |
