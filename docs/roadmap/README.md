# Roadmap after Phase 1 (planning notes)

Planning session of 9 Nov 2026 (owner: Paul Hampson; drafted with Claude Code). This folder is a **working reference** for continuing the roadmap planning. It is not a source-of-truth document, so it has no precedence in `docs/README.md`. The requirements, the architecture, the decision records and `docs/open-items.md` still govern. Adopting the roadmap formally (a Phase 2 work breakdown, a decision record) is a separate, later step.

- [issue-map.md](issue-map.md) maps each open item (`OI-`) and requirement to its GitHub issue and milestone. Use it to update issues without reviewing them all again.

## Assumptions

- Phase 1 ends with **2026.1** (the 2026.1-alpha release) and every P1 requirement is covered.
- The milestones map to releases:

  | Milestone | Number | Release |
  | --- | --- | --- |
  | `2026.1` | 1 | Phase 1 / alpha |
  | `2026.2` | 2 | Release 2, "Trustworthy beta" |
  | `2026.3` | 3 | Release 3, "Safe setups" |
  | `2026.4` | 4 | Release 4, "Capability and ecosystem" |

- **Do not add issues to `2026.1` or remove them from it without the owner's permission** (owner, 9 Nov 2026).

## Sources used

- `docs/requirements.md`: 73 P2 and P3 requirements, and the section 6 suggestions (Include or Defer).
- `docs/work-breakdown.md`: slice status, slice 20 (hardening and first release) and the spike table.
- `docs/open-items.md`: 346 rows in the open table on 9 Nov 2026 (294 open, 50 deferred, 2 proposed), grouped by theme.
- GitHub issues: 81 open on 9 Nov 2026.

## How features, quality of life and hardening were weighed

1. **Hardening goes first (2026.2).** Users must be able to trust the G-code. The alpha ships with these concessions:
   - no hand testing on macOS (#588);
   - unsigned binaries;
   - the Z1 X datum (OI-60) is not settled;
   - the reference-hardware budgets are not measured;
   - several sev2 bugs on entry and exit moves, the post, accessibility and the fuzzers.
2. **Collision comes before new strategies (2026.3).** The biggest real-world risk on a 4-axis machine is the holder, collet or head hitting the chuck or the part. COL-01 to COL-07 are all P2, and several open items wait on COL-05.
3. **Quality of life is spread across every release.** The P2 UI items are small and independent, so they fit parallel lanes.
4. **Ecosystem comes last (2026.4):** bindings, post scripting, version control and MCP. These pay off once the engine and its API have settled.

An alternative was to swap the main themes of releases 2 and 3 for growth over trust. It was not recommended.

## Release 2: 2026.2 "Trustworthy beta" (hardening, with moderate quality of life)

**Owner's changes, 9 Nov 2026:**
- OI-23 (lawyer review of the CLA and the OpenCASCADE notice) is dropped from 2026.2.
- #687 is already being worked on and stays in 2026.1.
- #437 is closed; other issues handled it.
- Signing (#696) is **nice to have** for 2026.2.

These themes go in 2026.2:
- **Close the alpha concessions.** Z1 hardware validation: #688 to #693. Reference-hardware budgets: #694. Cross-platform bit identity: #695. Signing (nice to have): #696. Installer and extra builds: #697.
- **Sev2 bugs:** #572, #619, #628, #598, #599, #603. Accessibility: #577 to #580 (#579 is OI-2110). #642 stays in 2026.1.
- **User-reported strategy bugs:** #650, #500, #510, and the blisk finishing limits #710. #687 and #641 are in 2026.1.
- **Performance:** #454, #538, #698 (posting again and posting large toolpaths), #699.
- **CI health:** #700 (coverage nightly), #701 (required checks), #669, #666. #660 is in 2026.1.
- **Quality of life:**
  - #702 PRJ-08 auto-save (an owner decision), #681;
  - #703 GC-08 line length;
  - #704 UI-18 inches, #705 UI-22 undo history, #706 UI-23 start page;
  - #707 SIM-13 summary;
  - documentation: #708 NFR-11 post guide, #709 NFR-19 and #441;
  - wording bugs: #607, #612 to #615, #631, #633.

**Exit:** no sev1 or sev2 bug is open, gates 4 and 5 pass on the release candidate, and signed binaries if achievable.

## Release 3: 2026.3 "Safe setups" (features about safety and setup) — accepted as is by the owner

- **Collision:** a spike first (#711), then:
  - COL-01 (#712);
  - COL-02 and TL-08 (#713);
  - COL-03 and COL-04 (#714);
  - COL-05 (#715, with #540);
  - COL-06 and COL-07 (#716).
- **Setups:** SET-07 multiple setups (#717), SET-04 custom stock (#718), SET-08 rotary acceleration (#719).
- **Re-import:** IMP-07, PRJ-07 and UI-24 (#720).
- **Tools:** TL-09 material library (#721), TL-10 tool tables (#722), HSMLib import (#545).
- **Operation control:** OP-25 cutting direction (#251), OP-22 bounding volumes (#236), canned cycles (#723).
- **Quality of life:**
  - UI-17 (#724), options search (#525);
  - UI-19 (#725), UI-21 (#726), UI-20 (#727);
  - SIM-11 (#728), SIM-12 (#729), SIM-14 (#730).

## Release 4: 2026.4 "Capability and ecosystem" (features) — accepted as is by the owner

- **Strategies:**
  - OP-17 adaptive (#731, with #529 and #527);
  - OP-19 (#732);
  - #573 clean-up with a different tool;
  - #415 the per-section contour drive;
  - TL-13 thread mills (#733).
- **Inputs and machines:** IMP-08 DXF and SVG (#734), SET-02 rotary axis about Y (#735).
- **Verification:** SIM-09 (#736), COL-08 (#737).
- **Ecosystem:** GC-13 post scripting (#738), INT-05 Python (#739), INT-03, INT-04 and INT-08 (#740), VC-01 to VC-06 (#741).
- **Stretch:** OP-18 (#742), SIM-10 (#743), the MCP server (#594).

## Suggested features (not yet in the requirements, awaiting triage, no milestone)

| # | Suggestion | Issue |
| --- | --- | --- |
| 1 | Operation templates and presets | #744 |
| 2 | Several parts on one stock along X | #745 |
| 3 | Indexed 3+1 2.5D operations at a fixed A | #746 |
| 4 | Probing routines for the rotary centreline and stock datum | #747 |
| 5 | Tailstock and between-centres workholding | #748 |
| 6 | Text engraving with a font chooser | #749 |
| 7 | More posts and machine profiles, community post trust (OI-87) | #751 |
| 8 | Toolpath editing | #752 |
| 9 | Export the simulated stock | #753 |
| 10 | Diagnostic bundle | #754 |
| 11 | Undo in the command line (OI-313); simulation from the command line is #601 | #755 |
| 12 | Forced regenerate | already #559 (not duplicated) |
| 13 | Results filters; exact unreachable areas | already #686 and #683 (not duplicated) |
| 14 | Localisation | #756 |
| — | Automatic operation planning (owner's addition) | #757 |

Accepting any of these needs a new requirement ID (CLAUDE.md: do not edit `requirements.md` to fit; propose, agree, then record).

## Open points for the next planning session

- Triage the suggestions (#744 to #757, plus #559, #683 and #686) and pick a milestone for each one accepted.
- **Not scheduled** (not in any milestone yet):
  - the remaining sev3 bugs and housekeeping issues: #685, #682, #680, #677, #676, #662, #658, #657, #645, #632, #629, #624, #621, #618, #609, #608, #589, #582, #581, #574, #563, #461;
  - #421 STEPRAMP, #516 deviation by area, #528 tab management, #559, #601, #683, #686, #368 (Discussions).
- **Open-item themes with no ticket yet:**
  - STEP robustness: OI-25, OI-26, OI-73, OI-74, OI-81, OI-960, OI-121, OI-831, OI-1350, OI-1351;
  - post details: OI-89, OI-68, OI-936, OI-937, OI-934, OI-92;
  - the rest of the CI items;
  - stale mock-ups: OI-342, OI-872, OI-873.
- No open item covers SIM-08 or OP-17. Check SIM-08's built state before scheduling it. Parts of PRJ-08, SIM-08, SIM-11, SIM-15, GC-08, UI-26, TL-09 and NFR-18 already have tests on `main`, so for those the 2026.2 and 2026.3 work is "finish", not "build".
- Formal adoption: a Phase 2 work breakdown and a decision record, under the rules in `AGENTS.md`.
