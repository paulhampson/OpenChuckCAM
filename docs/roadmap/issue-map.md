# Roadmap issue map

Snapshot of 9 Nov 2026 (see [README.md](README.md)). It maps each open item and requirement to its GitHub issue and milestone, so a later session can update issues without reviewing them all. When you change an issue's milestone, or close an issue or an `OI-`, update the row here too.

Milestone numbers: `2026.1` = 1, `2026.2` = 2, `2026.3` = 3, `2026.4` = 4, `Future` = 5. "—" means no milestone.

## Open item → issue

| OI | Issue | Milestone |
| --- | --- | --- |
| OI-02 | #688 | 2026.2 |
| OI-03 | #688 | 2026.2 |
| OI-21 | #688 | 2026.2 |
| OI-28 | #695 | 2026.2 |
| OI-30 | #740 | 2026.4 |
| OI-36 | #741 | 2026.4 |
| OI-41 | #690 | 2026.2 |
| OI-60 | #688 | 2026.2 |
| OI-61 | #721 | 2026.3 |
| OI-64 | #695 | 2026.2 |
| OI-75 | #695 | 2026.2 |
| OI-87 | #751 | 2026.4 |
| OI-102 | #715 | 2026.3 |
| OI-104 | #696 | 2026.2 (nice to have) |
| OI-105 | #691 | 2026.2 |
| OI-106 | #732 | 2026.4 |
| OI-107 | #719 | 2026.3 |
| OI-113 | #717 | 2026.3 |
| OI-135 | #697 | 2026.2 |
| OI-149 | #701 | 2026.2 |
| OI-153 | #713 | 2026.3 |
| OI-160 | #714 | 2026.3 |
| OI-172 | #720 | 2026.3 |
| OI-173 | #694 | 2026.2 |
| OI-176 | #694 | 2026.2 |
| OI-179 | #735 | 2026.4 |
| OI-208 | #734 | 2026.4 |
| OI-216 | #723 | 2026.3 |
| OI-237 | #713 | 2026.3 |
| OI-267 | #689 | 2026.2 |
| OI-281 | #733 | 2026.4 |
| OI-313 | #755 | closed, not planned (owner, 9 Nov 2026) |
| OI-315 | #701 | 2026.2 |
| OI-318 | #710 | 2026.2 |
| OI-337 | #710 | 2026.2 |
| OI-369 | #720 | 2026.3 |
| OI-371 | #730 | 2026.3 |
| OI-540 | #694 | 2026.2 |
| OI-551 | #718 | 2026.3 |
| OI-560 | #728 | 2026.3 |
| OI-563 | #728 | 2026.3 |
| OI-700 | #236 | 2026.3 |
| OI-701 | #236 | 2026.3 |
| OI-702 | #236 | 2026.3 |
| OI-856 | #251 | 2026.3 |
| OI-881 | #699 | 2026.2 |
| OI-923 | #236 | 2026.3 |
| OI-933 | #703 | 2026.2 |
| OI-981 | #721 | 2026.3 |
| OI-992 | #693 | 2026.2 |
| OI-993 | #693 | 2026.2 |
| OI-1232 | #702 | 2026.2 |
| OI-1250 | #741 | 2026.4 |
| OI-1292 | #692 | 2026.2 |
| OI-1450 | #415 | 2026.4 |
| OI-1522 | #708 | 2026.2 |
| OI-1523 | #709 | 2026.2 |
| OI-1526 | #709 | 2026.2 |
| OI-1780 | #692 | 2026.2 |
| OI-1781 | #692 | 2026.2 |
| OI-1988 | #683 | 2026.3 |
| OI-1993 | #698 | 2026.2 |
| OI-2004 | #573 | 2026.4 |
| OI-2022 | #698 | 2026.2 |
| OI-2025 | #540 | 2026.3 |
| OI-2031 | #700 | 2026.2 |
| OI-2044 | #715 | 2026.3 |
| OI-2110 | #579 | 2026.2 |
| OI-2135 | #727 | 2026.3 |

Dropped by the owner from 2026.2: OI-23 (lawyer review). It has no issue.

## Issue → milestone, requirements and open items

### 2026.2 (release 2, "Trustworthy beta")

| Issue | Kind | Requirements | OIs | Note |
| --- | --- | --- | --- | --- |
| #688 | new | SET-01, GC-09 | OI-60, OI-02, OI-03, OI-21 | Z1 X datum |
| #689 | new | OP-13, GC-04 | OI-267 | A-only feed above 1200 mm/min |
| #690 | new | GC-09 | OI-41 | tool change, gate 5 |
| #691 | new | NFR-05 | OI-105 | rotary precision |
| #692 | new | SET-01, OP-14 | OI-1292, OI-1780, OI-1781 | Z limits, clearance radius |
| #693 | new | GC-09, OP-13 | OI-992, OI-993 | short-block motion |
| #694 | new | NFR-03, NFR-04 | OI-173, OI-176, OI-540 | reference-hardware budgets |
| #695 | new | GC-07, NFR-06 | OI-28, OI-64, OI-75 | cross-platform bit identity |
| #696 | new | — | OI-104 | signing, nice to have |
| #697 | new | NFR-02 | OI-135 | installer and extra builds |
| #698 | new | NFR-04 | OI-2022, OI-1993 | posting speed and reuse |
| #699 | new | NFR-04 | OI-881 | deviation colours reuse |
| #700 | new | — | OI-2031 | coverage nightly |
| #701 | new | NFR-09 | OI-149, OI-315 | repository settings |
| #702 | new | PRJ-08 | OI-1232 | auto-save, owner decision |
| #703 | new | GC-08 | OI-933 | line length |
| #704 | new | UI-18 | | inches |
| #705 | new | UI-22 | | undo history |
| #706 | new | UI-23 | | start page |
| #707 | new | SIM-13 | | operation summary |
| #708 | new | NFR-11 | OI-1522 | post guide |
| #709 | new | NFR-19 | OI-1523, OI-1526 | versioned guide |
| #710 | new | OP-05, OP-20 | OI-318, OI-337 | blisk finishing |
| #572 #619 #628 #598 #599 #603 | existing | | | sev2 bugs |
| #577 #578 #579 #580 | existing | UI-14, NFR-15 | OI-2110 (#579) | accessibility sev2 |
| #650 #500 #510 | existing | | | strategy bugs |
| #454 #538 | existing | NFR-04 | | performance |
| #669 #666 | existing | | | CI flakes |
| #681 | existing | PRJ-08 | | recovery warnings |
| #607 #612 #613 #614 #615 #631 #633 | existing | | | wording |
| #441 | existing | NFR-11 | | documentation |
| #754 | request | — | | diagnostic bundle |
| #559 | existing | — | | forced regenerate |

Left in 2026.1 (not moved): #687, #660, #641, #642 (and #640, #643, #644, #586, #587, #588, #668, #750).

### 2026.3 (release 3, "Safe setups")

| Issue | Kind | Requirements | OIs |
| --- | --- | --- | --- |
| #711 | new (spike) | COL-01 to COL-06 | |
| #712 | new | COL-01 | |
| #713 | new | COL-02, TL-08 | OI-153, OI-237 |
| #714 | new | COL-03, COL-04 | OI-160 |
| #715 | new | COL-05 | OI-2044, OI-102 |
| #716 | new | COL-06, COL-07 | |
| #717 | new | SET-07 | OI-113 |
| #718 | new | SET-04 | OI-551 |
| #719 | new | SET-08 | OI-107 |
| #720 | new | IMP-07, PRJ-07, UI-24 | OI-172, OI-369 |
| #721 | new | TL-09 | OI-61, OI-981 |
| #722 | new | TL-10 | |
| #723 | new | OP-10, GC-03 | OI-216 |
| #724 | new | UI-17 | |
| #725 | new | UI-19 | |
| #726 | new | UI-21 | |
| #727 | new | UI-20 | OI-2135 |
| #728 | new | SIM-11 | OI-560, OI-563 |
| #729 | new | SIM-12 | |
| #730 | new | SIM-14 | OI-371 |
| #236 | existing | OP-22 | OI-700, OI-701, OI-702, OI-923 |
| #251 | existing | OP-25 | OI-856 |
| #540 | existing | COL-05 | OI-2025 |
| #545 | existing | TL-10 (HSMLib) | |
| #525 | existing | UI-17 (options search) | |
| #745 | request | — (SET-07 related) | |
| #747 | request | — | |
| #748 | request | — (COL-03 related) | |
| #753 | request | — | |
| #758 | new (groundwork for #756) | — | |
| #686 | existing request | — | |
| #683 | existing request | — | OI-1988 |

### 2026.4 (release 4, "Capability and ecosystem")

| Issue | Kind | Requirements | OIs |
| --- | --- | --- | --- |
| #731 | new | OP-17 | |
| #732 | new | OP-19 | OI-106 |
| #733 | new | TL-13 | OI-281 |
| #734 | new | IMP-08 | OI-208 |
| #735 | new | SET-02 | OI-179 |
| #736 | new | SIM-09 | |
| #737 | new | COL-08 | |
| #738 | new | GC-13 | |
| #739 | new | INT-05, INT-09 | |
| #740 | new | INT-03, INT-04, INT-08 | OI-30 |
| #741 | new | VC-01 to VC-06 | OI-36, OI-1250 |
| #742 | new (stretch) | OP-18 | |
| #743 | new (stretch) | SIM-10 | |
| #573 | existing | OP-28 follow-up | OI-2004 |
| #415 | existing | OP-05, OP-20 | OI-1450 |
| #529 #527 | existing | OP-17 related | |
| #594 | existing (stretch) | — (MCP server) | |
| #744 | request | — | |
| #749 | request | — (IMP-08 related) | |
| #751 | request | — | OI-87 |

### Future (milestone 5, backlog)

| Issue | Kind | Note |
| --- | --- | --- |
| #746 | request | indexed 2.5D; likely part of a later plan to add 3-axis machining |
| #752 | request | toolpath editing |
| #756 | request | localisation (groundwork #758 is in 2026.3) |
| #757 | request | automatic operation planning |

### Closed by triage

#755 (CLI undo, OI-313): closed as not planned.

## Requirement → issue (P2 and P3 marked Include)

| Requirement | Issue |
| --- | --- |
| IMP-07 | #720 |
| IMP-08 | #734 |
| UI-17 | #724 |
| UI-18 | #704 |
| UI-19 | #725 |
| UI-20 | #727 |
| UI-21 | #726 |
| UI-22 | #705 |
| UI-23 | #706 |
| UI-24 | #720 |
| SET-02 | #735 |
| SET-04 | #718 |
| SET-07 | #717 |
| SET-08 | #719 |
| TL-08 | #713 |
| TL-09 | #721 |
| TL-10 | #722 |
| TL-13 | #733 |
| OP-17 | #731 |
| OP-18 | #742 |
| OP-19 | #732 |
| OP-22 | #236 |
| OP-25 | #251 |
| SIM-08 | none: partly built, check it |
| SIM-09 | #736 |
| SIM-10 | #743 |
| SIM-11 | #728 |
| SIM-12 | #729 |
| SIM-13 | #707 |
| SIM-14 | #730 |
| SIM-15 | none: partly built, check it |
| GC-08 | #703 |
| GC-13 | #738 |
| PRJ-07 | #720 |
| PRJ-08 | #702 |
| COL-01 | #712 |
| COL-02 | #713 |
| COL-03 | #714 |
| COL-04 | #714 |
| COL-05 | #715 |
| COL-06 | #716 |
| COL-07 | #716 |
| COL-08 | #737 |
| VC-01 to VC-06 | #741 |
| INT-03, INT-04, INT-08 | #740 |
| INT-05, INT-09 | #739 |
| NFR-11 | #708, #441 |
| NFR-19 | #709 |
