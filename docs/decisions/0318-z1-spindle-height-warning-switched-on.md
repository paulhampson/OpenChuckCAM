# 0318: The Makera Z1's spindle-height warning is switched on: the spindle is 73.1 mm above the axis at home (lane Z1AXIS; issue #395; closes OI-1290 and OI-1291)

- **Status:** proposed 30 Oct 2026 by lane Z1AXIS. The owner chose option A of D9 (switch the warning on inside pull request #396) and supplied the probe's stick-out, both relayed by the coordinator session (decision 0049) on 30 Oct 2026. The record is proposed until the owner accepts it.
- **What it amends:** [decision 0317](0317-z1-measured-axis-height-and-safe-z.md) item 6 (accepted), "The spindle-height warning ships off". The warning is now on for a machine that measures tool lengths and gives its spindle-to-axis distance, and the built-in Z1 gives it.

  SET-06 and NFR-12 are unchanged. The G-code, the golden programs, the project and machine file formats and the library API are unchanged: the fields are decision 0317's.
- **Date:** 30 Oct 2026.
- **Open items** ([docs/open-items.md](../open-items.md)): closes OI-1290 (the warning) and OI-1291 (the distance). Opens OI-1292: check how the Z1 behaves at the top of the Z travel (the home switch and the soft limits).

Numbering: in lane Z1AXIS's range (records 0317 to 0321, OI-1290 to OI-1309), checked free on `main` (65de89d) and every pushed branch on 30 Oct 2026.

## Context

Decision 0317 recorded −48.6 mm as the machine Z of the axis for the probe as the reference tool. A tool that sticks out further than the probe raises the spindle by the difference, because the controller offsets each tool by its length difference from the probe (firmware 1.1.2, `ATCHandler.cpp:1909-1919`, `Robot.cpp:498-506`). The tool library holds no stick-out, so the owner set it as the flute length + 5 mm. The warning needed the distance D from the spindle at home (machine Z 0, the top of the travel) down to the axis.

The owner measured the probe's stick-out from the collet as 24.5 mm, from the same reference as a tool's stick-out is meant from (30 Oct 2026, relayed). So D = 48.6 + 24.5 = 73.1 mm. This is derived, not measured directly.

## Decision

1. **The value.** The built-in Z1 gives `spindle_to_axis = 73.1`, **unconfirmed** (derived; the owner did not mark it confirmed). Its source text names the probe's stick-out, the reference it is measured from, and the sum.
2. **The check.** For a machine that measures tool lengths and gives D, each tool used by an enabled operation of a setup is checked once:
   - The tool's assumed stick-out is S = flute length + 5 mm.
   - The highest its tip reaches above the axis is D − S.
   - When the setup's safe Z (its own clearance, else the profile's) is above that by more than 0.0005 mm (rounding to the G-code's three places), the setup checks and Messages warn.
   - The check is made in millimetres and is the same at any thread count (GC-07).
   - A machine without tool length measurement, or without D, gets no such warning. The software does not guess.
3. **The words** (NFR-12): "Tool *n* (*name*) may take the spindle above the top of the Z travel at the safe Z: assuming it sticks out *S* mm (flute length + 5 mm), the spindle would be *x* mm past the top. Lower the setup's safe Z and clearance to *D − S* mm or less, or use a shorter tool, and check the clearance over the chuck and the Z travel on your machine before running."

   When D − S is under the profile's recommended minimum safe Z, the text adds "The recommended minimum is 30 mm. This must be checked before machining." (the owner's wording, decision 0317 item 7).
4. **The worked cases** (the owner's, checked):

   | Tool | Stick-out S | D − S | At safe Z 40 |
   | --- | --- | --- | --- |
   | 104 (flute 25) | 30 | 43.1 | no warning; spindle at −3.1 |
   | 204 (flute 22) | 27 | 46.1 | no warning; spindle at −6.1 |
   | 42 mm flute default | 47 | 26.1 | warns: 13.9 mm past the top; fits at 26.1, under 30, so the recommended-minimum line shows |

   At the old safe Z 50, tool 104 would have taken the spindle 6.9 mm past the top. The gate 5 cut raised no alarm, but the Z1's soft limits are off by default (`configZ1.default:429`), so this does not contradict the value. How the machine behaves at the top of the travel is OI-1292.

## Consequences

- `crates/openchuck-project/machines/makera-z1.toml`: `spindle_to_axis = 73.1` and its provenance.
- `openchuck-app` `setup.rs`: `STICK_OUT_BEYOND_FLUTES`, `SpindleHeight`, `spindle_height` and `spindle_height_text`. `overview.rs`: the setup check and the message, once per setup and tool.
- The existing tests whose expectations change:
  - the built-in Z1 profile test (`spindle_to_axis` is 73.1, no longer unknown);
  - the machine view test ("73.100 mm" instead of "Not known");
  - the command line `machine` test ("spindle to axis 73.1 mm").

  No golden program and no other test changes. The sample projects' machine copies hold no distance, so they give no such warning.
- Documents: OI-1290 and OI-1291 closed, OI-1292 new. The UI specification's dock text, and the safe practice checklist and air-cut procedure lines, now mention the warning.

## Tests

Written before the code. Each failed first, except the no-warning test, which the empty stub already passed:

- `openchuck-app` `setup.rs`:
  - `req_set_06_req_nfr_12_the_spindle_height_follows_the_owners_worked_cases`: the three tools, and gate 5 at safe Z 50.
  - `req_set_06_the_spindle_height_is_checked_at_its_boundary`: a tool with flute 28.1 just fits at 40; 28.101 warns by 0.001.
  - `req_set_06_req_nfr_12_no_distance_or_no_measurement_gives_no_spindle_height_warning`.
  - `req_nfr_12_the_spindle_height_warning_says_what_to_check`: the text, and the recommended-minimum line only when the fit is under it.
- `openchuck-app/tests/overview.rs` `req_set_06_req_nfr_12_a_tool_too_long_for_the_safe_z_warns_on_the_z1`:
  - one check and one message per tool;
  - the same checks on a second run;
  - the setup's clearance override clears it;
  - a turned-off operation does not warn.
