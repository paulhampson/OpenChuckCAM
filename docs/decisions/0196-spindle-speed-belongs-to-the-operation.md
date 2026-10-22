# 0196: The spindle speed belongs to the operation (lane BUG-SPINDLE; issue 249, trial finding F-79)

- **Status:** accepted on 21 Oct 2026, by the owner, directly in the lane BUG-SPINDLE session. The owner's answers to decisions 1, 2 and 3 below were relayed by the coordinator session on 21 Oct 2026 (the owner's option A in each case); decision 4 (existing projects, option A) was relayed the same way later.
- **What it amends:** the data flow of TL-05 (cutting data per tool and stock material) and GC-14 (spindle speed per tool in the post job), [decision 0014](0014-slice-4-tool-library.md) item 5 (stock material names matched exactly). Requirements TL-05, GC-14 and NFR-12 are unchanged.
- **Date:** 21 Oct 2026.
- **Open items:** OI-750 to OI-753 (answered, closed), OI-754 and OI-755 (new).

## Context

The owner's projects have `material = "Softwood"` on the setup and use the Makera Z1 tools. Generating failed: "cannot post the G-code: tool 'Z1 O-flute metal 6x17' has no cutting data for the material 'Softwood', so no spindle speed (names must match exactly)." The user had nowhere to give the operation a spindle speed, so no G-code could be written.

Cause, read from the code on `main` (9a84c01): the only source of a spindle speed was the tool's cutting data for the setup's material. `posting.rs` looked up `snapshot.cutting_data[setup.material]` per tool and returned an error when the entry or the material was missing. The value went into `ToolSetting.spindle_rpm`, keyed by the tool tag, and the post wrote `S… M3` once per operation group. `Operation` had no spindle field, so nothing could stand in for a missing entry. The same lookup gave the feeds, the stepover and the stepdown. The tool report warned of the missing data, the setup sheet printed "-", and the required-values list (decision 0173) did not know the spindle speed, so the failure showed only at the Post step.

## Decision

1. **Where the value lives (OI-750; the owner's option A).** The operation owns its spindle speed: a new optional field `spindle_speed` (rpm) on the operation. Resolution order: the operation's own value; else the tool's cutting data for the setup's material (shown as the default with a "from the tool's cutting data" hint); else missing. **There is no machine default**: the machine profile supplies none (the owner's answer 3 in issue 249).
2. **The feed follows the operation's speed.** The cutting feed taken from the cutting data is the entry's feed per tooth times the flute count times the spindle speed the operation runs at (`CuttingData::cutting_feed_at`), so a user who sets 9 000 rpm on an operation whose data says 12 000 gets the matching feed. A feed the operation sets itself still wins, with the existing note.
3. **A missing speed (OI-751; option A).** The operation still generates and simulates. It is listed as missing a required value (`RequirementField::SpindleSpeed`, the list of decision 0173), the row is marked in the operation's Tool section, the step bar shows it, and Post and export is blocked with: "The operation "X" has no spindle speed. Enter a spindle speed for this operation, or add cutting data for "Softwood" to the tool "T" in the tool library." (Without a material it says to choose the stock material in the setup and add data for it to the tool.) No G-code without an S word is ever written (NFR-12).
4. **The post gets the speed per operation.** `PostJob` and `ConvertOptions` gain `operation_spindle` (rpm by operation tag); an operation with an entry runs at it, any other at its tool's `ToolSetting::spindle_rpm`. Two operations with one tool can run at different speeds. This is a breaking change to the library family: version 0.15.0, changelogs in each affected crate (decision 0048).
5. **The inspector.** The operation's Tool section has a Spindle speed row (unit rpm, the tooltip says what it is and that the user checks it before running the program). Empty, it shows the tool's value as the hint; ↺ takes the operation's own value away. Text that is not a number above 0 is refused in plain words (NFR-07). The setup sheet's rpm column shows the speed the program uses.
6. **Project schema 16 (claimed in the work breakdown).** The operation file gains `spindle_speed`, written only when set. Additive: the migration changes only the number, and refuses a schema 15 operation file that already holds the field. A value that is not finite and above 0 is refused when the project is checked.
7. **Existing projects (OI-753; the owner's option A, relayed).** No migration of values. An older project opens unchanged and keeps taking the tool's cutting data, so its G-code is byte-identical. Only a project whose tool lacks data for the material shows the new message, and it could not be posted before either.

## G-code and golden files

The S words and every other word of a program are unchanged for a project that posted before: the speed from the cutting data is the same number, and the feed from it is the same number while the operation sets no speed of its own. **No golden file changes.** What changes is which projects can be posted (those with a speed on the operation) and the error text for a missing one.

## Not decided here

- The machine profile has no spindle maximum, and a tool has no speed limit of its own, so the issue's "checked against the machine's limits and the tool's" cannot be done (OI-754).
- Whether the setup sheet or a G-code comment should say where the speed came from (the operation or the tool's data) (OI-755).

## Tests

- `crates/openchuck-types/tests/tools.rs`: `req_tl_05_material_names_match_ignoring_case_and_repeated_spaces`, `req_tl_05_an_exact_name_wins_over_a_case_match`.
- `crates/openchuck-project/tests/persistence.rs`: `req_tl_05_req_prj_04_an_operation_spindle_speed_round_trips_and_needs_schema_16`, `req_nfr_07_a_bad_operation_spindle_speed_is_refused`.
- `crates/openchuck-post/tests/machine_program.rs`: `req_gc_14_an_operation_spindle_speed_overrides_the_tools`.
- `crates/openchuck-app/tests/posting.rs`: `req_tl_05_req_gc_14_an_operations_spindle_speed_posts_when_the_tool_has_no_cutting_data` (the owner's case: Softwood with no data), `req_nfr_12_a_missing_spindle_speed_is_refused_with_what_to_do_and_never_written_without_s`, `req_tl_05_the_cutting_feed_follows_the_operations_spindle_speed`, `req_tl_05_a_material_name_differing_in_case_or_spacing_still_finds_the_cutting_data`.
- `crates/openchuck-app/tests/tool_library.rs`: `req_tl_05_the_operations_spindle_speed_comes_from_itself_then_the_tool_then_is_missing`.
- `apps/openchuck-ui/src/shell/tool_library_tests.rs`: `req_tl_05_req_nfr_12_the_operation_has_a_spindle_speed_row`.

## Alternatives considered

- **B: the operation's value only, the cutting data never filling it.** Loses the TL-05 default. Not chosen.
- **C: the feed does not follow the operation's speed.** An override would change the chip load without saying so. Not chosen.
- **Refuse to generate without a speed.** The toolpath and the simulation do not need it. Not chosen (decision 3).
