# 0530: Minimum clearance for the workholding, and clearance from stock (lane CLEARANCE)

- **Status:** proposed by lane CLEARANCE on 1 Nov 2026. The coordinator session approved the plan on 1 Nov 2026. The owner answered the four choices: D1 and D2 directly in the lane session (decision 0049), and D3 and D4 relayed by the coordinator, all on 1 Nov 2026. Items 3 and 6 are the lane's refinements, which the owner accepts or changes with this record.
- **Date:** 1 Nov 2026.
- **Branch:** `claude/lane-clearance`. Issue [#336](https://github.com/paulhampson/OpenChuckCAM/issues/336) ("Clearance considerations for tool length").
- **Requirements:** SET-06 (clearance geometry), SET-09 (the held region), SET-10 (the machine form), OP-14 (links use the clearance), UI-07, UI-13, NFR-07, NFR-12, GC-07 and NFR-06. No requirement text changes: the minimum and Clearance from stock are part of SET-06's "define clearance geometry" (the coordinator agreed, 1 Nov 2026).
- **What it amends:** nothing in `requirements.md` or `architecture.md`. The verification plan's SET-06 row, the UI specification (11.3 and 12.5) and the work breakdown (lane rows and the schema claim) are edited in the same pull request.
- **Records it relies on:** [0011](0011-slice-3-machine-stock-setup.md) (profiles and setups), [0161](0161-tools-keep-clear-of-the-gripped-length.md) (the held region), [0317](0317-z1-measured-axis-height-and-safe-z.md) and [0318](0318-z1-spindle-height-warning-switched-on.md) (the spindle-height check), [0380](0380-machine-library-in-the-application.md) (the machine form), [0048](0048-pre-1-0-api-changes.md) (pre-release API changes).
- **Schema and versions:** project schema 26 and machine profile schema 8 (claimed 1 Nov 2026, confirmed by the coordinator). The library family takes a minor bump, provisionally 0.34.0, which the coordinator confirms at merge (CRANKEXCESS and SWARF-FLOOR take 0.32.0 and 0.33.0 first). It is a breaking change: `MachineProfile` and `SetupCheck` gain fields, and `ProfileField::ALL` grows.

## Context

The clearance is a safe cylinder radius about the rotary axis and a safe Z (SET-06). The machine profile gives a default and a setup can set its own. The setup checks warn when the cylinder is inside the stock or the part, or when the safe Z is outside the Z travel. Decision 0318 warns when a tool would take the spindle above the top of the Z travel at the safe Z, on a machine that measures tool lengths and gives its spindle-to-axis distance.

Nothing modelled the chuck body, the jaws, the headstock or the tailstock. The held region (SET-09) is the stock the chuck grips, not the hardware around it, so a clearance that clears the stock could still pass through the jaws. Issue #336 asks for:

- a machine-based minimum clearance for that hardware;
- expanding the clearance from the stock size when the user wants;
- a warning when a clearance set by hand conflicts with the minimum;
- the tool length considered, with a warning when the machine may travel too far.

## Decision

1. **Where the minimum lives (D1; the owner chose "machine plus setup override").**
   - The machine profile gains the optional `min_clearance_radius`: the smallest safe cylinder radius about the rotary axis that clears the chuck, the jaws, the headstock and the tailstock. It has UI-13 provenance (`ProfileField::MinClearanceRadius`). It is listed in the Clearance group of the machine form (SET-10) as "Minimum clearance radius", blank allowed.
   - A setup gains the optional `clearance_rules.min_radius`, which replaces the machine's minimum for that setup's workholding.
   - Blank on both means no minimum and no check.
2. **Warn, never refuse (D2; the owner chose "warn").**
   - When the setup's safe cylinder is inside the minimum by more than 0.0005 mm (the G-code's three places), the setup checks and Messages warn, under the setup's Work offset and clearance section.
   - The warning names both radii and where the minimum comes from ("from the machine" or "set for this setup"), and names an unconfirmed machine minimum.
   - Generation and export still run, as for every other clearance check.
   - The words: "the safe cylinder (radius *r* mm) is inside the minimum clearance (radius *m* mm, *source*): links may hit the chuck, jaws, headstock or tailstock. Raise the setup's clearance or switch on Clearance from stock, and check the clearance on your machine before running."
   - Nothing says the G-code is safe or verified (NFR-12).
3. **Clearance from stock (D3; the owner chose option A).**
   - A setup gains the optional `clearance_rules.from_stock`, the margin. Absent means off, which is the default. Switching it on in the inspector starts the margin at 5 mm. The margin may be 0 mm or more.
   - When it is on, the safe cylinder radius is the largest of: the base radius (the setup's own clearance, else the machine's), the minimum, and the stock's radius plus the margin. The stock's radius counts when it can be measured; bounding-box stock measures the part.
   - The safe Z is the larger of the base safe Z and that radius.
   - It is worked out again whenever the stock, the part, the minimum or the base clearance changes, like the automatic tabs of [decision 0202](0202-head-and-tail-tabs-placed-automatically.md).
   - **Lane refinement, for the owner to accept with this record:** D3 gave the radius as max(minimum, stock radius + margin). The lane keeps the base radius in the max as well, so switching Clearance from stock on never lowers a clearance (the coordinator agreed, 1 Nov 2026).
4. **One effective clearance.**
   - `openchuck_types::setup::effective_clearance` is a pure, deterministic function. The application's `setup::effective_clearance` wraps it for a project setup and its part bounds.
   - Every consumer takes the clearance from it: the strategies (`StrategyInput::clearance`), the links between operations (OP-14), the post's safe Z, the program check, the setup checks, the inspector's "In use" line and the CLI's `setup` report.
   - With no rules set it returns exactly the clearance used before. Nothing in this record changes the G-code of an existing project (GC-07, NFR-06). The reference goldens are unchanged.
5. **Defaults (D4; the owner chose option A).**
   - Everything starts blank or off: no machine minimum, no setup minimum, Clearance from stock off.
   - The built-in Makera Z1 leaves the minimum blank until the owner measures it (OI-1780). It will then ship marked not confirmed on the machine (UI-13), as decision 0317 did for the Z1's other values.
   - An older project or profile reads with neither field. The migrations (project 25 to 26, profile 7 to 8) change no value, and refuse an older file that already holds a new field rather than guess.
6. **Tool length beyond decision 0318.**
   - a. *Built:* the 0318 spindle-height check takes the setup's effective safe Z. Without this, a tool that fits at the base safe Z would pass after Clearance from stock lifts it.
   - b. *Built (lane refinement, for the owner to accept with this record):* when a tool's highest tip (the spindle-to-axis distance less its assumed stick-out) is inside the minimum clearance radius, no safe Z clears both the workholding and the top of the Z travel. Decision 0318's advice, "Lower the setup's safe Z and clearance to *D − S* mm or less", would then point inside the chuck, so in this case the check says instead:

     "Tool *n* (*name*) may be too long to clear the workholding within the Z travel: assuming it sticks out *S* mm (flute length + 5 mm), its tip can rise at most *D − S* mm above the axis, inside the minimum clearance radius of *m* mm, so no safe Z clears both the chuck, jaws, headstock or tailstock and the top of the Z travel. Use a shorter tool, and check the clearance over the chuck and the Z travel on your machine before running."

     The recommended-minimum sentence of 0318 still follows when it applies. This changes 0318's wording only in this new case, which cannot happen until a minimum is set. Machines that do not measure tool lengths still get no guess.
   - c. *Not built:* a short tool cutting deep near the axis may need the spindle below the bottom of the Z travel (OI-1781).
   - d. *Not built:* machines without tool length measurement or a spindle-to-axis distance, and a real stick-out per tool in place of the flute + 5 mm assumption (OI-1782).
7. **Interfaces.**
   - The setup inspector's Work offset and clearance section gains:
     - "Minimum clearance" (empty uses the Machine setting, shown as the hint);
     - the "Clearance from stock" checkbox;
     - "Margin over stock" (shown while the checkbox is on);
     - a read-only line, "In use: safe radius *r* mm, safe Z *z* mm (Clearance from stock)".
   - The CLI's `setup` command gains `--min-clearance MM|machine` and `--clearance-from-stock MM|off`, and prints the minimum and the clearance in use.

## Consequences

- The setup checks can warn about hardware that was not modelled. How useful this is depends on a measured minimum, which the Z1 lacks until OI-1780 closes.
- The clearance a setup uses can now differ from the values typed into it. The inspector and the CLI show the clearance in use, so the user sees the value the program will use.
- Library API: `MachineProfile::min_clearance_radius`, `ProfileField::MinClearanceRadius` (`ALL` has 15 entries), `ClearanceRules`, `DEFAULT_STOCK_MARGIN`, `MinimumSource`, `EffectiveClearance`, `effective_clearance`, `SetupCheck::rules` and `SetupWarningKind::ClearanceBelowMinimum`. This is a minor bump under decision 0048.

## Open items

- OI-1780: the Z1's measured minimum clearance radius (new).
- OI-1781: the bottom of the Z travel for short tools (new).
- OI-1782: tool length without measurement, and a stick-out per tool (new).
