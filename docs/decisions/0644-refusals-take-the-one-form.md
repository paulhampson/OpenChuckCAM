# 0644: Every refusal to generate takes the one form, and the Facing notes and the setup checks too (decision 0536 step 6; lane MSGREVIEW; issue #567)

- **Status:** proposed by lane MSGREVIEW on 9 Nov 2026, on the owner's answers of 9 Nov 2026 relayed by the coordinator (decision 0049): (3) the Facing line and the other note without a topic always take the standard form; (4) OI-360 closes for now; (5) the strategy refusals take the standard form before the freeze, in a follow-up to decision 0536 step 4 (#659). The number is provisional (the Merge Manager assigns it at release), as is the library-family version.
- **Amends:** [decision 0536](0536-message-levels-operation-reference-and-setting-links.md) section 7: adds build step 6 (section 7's item 6, decisions 0538 to 0540, is built in #623, #591 and #584). Builds on RAMPENTRY's `AppError::Refused` ([decision 0543](0543-a-ramp-entry-follows-the-toolpath.md), #663), which stays the one mechanism.

## Context

Decision 0536 gave every message one form, `<Topic> (<items>): <what>. <why>. <what to change>.` Steps 2 to 5 brought the warnings, the information, and the setup, model and export messages to it. A refusal to generate an operation still read `operation '<name>' cannot be generated: <reason>`, and its reason was whatever the pathing library wrote:

- often a prefix of its own ("End facing: …", "Facing completion: …", "Facing final validation: …");
- a schema name ("parameter stepover_mm: …");
- a variant's word ("surface: …", "A axis: …");
- the operation a second time ("operation 'X': cannot connect safely: …");
- or nothing at all.

RAMPENTRY (#663) added `AppError::Refused`, which shows a refusal in the one form, but it found the topic by matching the start of the text against a list. The two Facing notes ("Facing covered the end faces it was asked to." and "Intentionally unfaced: …") had no topic. The command line's setup listing printed the setup checks without a topic.

The inventory of the refusals on `main`, #656 and #663 (about 270 sites in the pathing library and 60 in the application), and the topic each now takes, is in [docs/archive/0644-refusals-take-the-one-form/](../archive/0644-refusals-take-the-one-form/).

## Decision (proposed)

1. **One library variant for a refusal.** `openchuck-path` gets `PathError::Refused { topic, text, setting }`:
   - the check's topic, a constant of `openchuck_path::topic`;
   - the sentences (what, why, what to change);
   - the key of the operation's setting to change, if one does.

   `PathError::refused(topic, text)` and `.with_setting(key)` build it. Every refusal of a strategy moves to it:
   - each file's `refuse()` helper takes its old prefix as the topic: End facing, Facing completion, Facing motion, Box facing, Facing extent, and **Facing check** for "Facing final validation" (the coordinator's scope of 9 Nov 2026);
   - a connection refusal takes Links, and no longer names the operation in its text;
   - the entry refusals take Ramp entry, Entry, Face nubs, Lead entry and Helix entry, each with the setting its first remedy names (RAMPLEAD's lead refusals of #665 too, with Lead radius or Entry);
   - the texts with no prefix take the topic of the check they make where one exists (Held region, Region, Selection set, Surface, Stock, Stock to leave, Links, Faces across the axis, Past the ends), and otherwise their strategy's (Rotary roughing, Axial finishing, Helical finishing, Simultaneous 4-axis finishing, Rest machining, Pencil clean-up, Swarf, Face-flow finishing, Edge following, Radial drilling, Facing, Toolpath).

   The words of each text are kept. `PathError::Parameter`, `Surface`, `Region` and `RotaryLimit` stay, for the geometry's own errors. Adding the variant and the topics is an addition (`PathError` is `#[non_exhaustive]`), but the refusal texts change: the library family takes the next minor version at release (decision 0048).
2. **The application shows it.** `AppError::Refused { topic, name, text, setting }` is built from `PathError::Refused` directly, not by matching text, so RAMPENTRY's list of topics goes. Other path errors are mapped:
   - a refused parameter takes its field's label as its topic and its key as the setting ("Stepover (operation 'X'): must be above 0 mm.");
   - `Surface`, `Region` and `RotaryLimit` take the topics Surface, Region and Rotary axis;
   - anything else is a Generation refusal.

   `AppError::Operation` goes. The application's own refusals take topics: Machine profile, Setup, Tool, Part, Stock, Material, Operation, Selection set, STEP part, Workholding, Travel, Stock to leave, Cutting data, Setting and Box facing.

   A refusal that lists passes names them as every note that lists passes does (`openchuck_path::passes_named`, made public): "pass 149 is entered by a straight plunge …", "88 passes, from pass 1 to pass 88 (1, 2, 3 ... 86, 87, 88), are entered …", not "88 passes, the first pass 1, are entered …" (OWNERCHECKS2, relayed on 9 Nov 2026).

   A refusal is displayed through `lead_in::in_form`, so every route (Messages, the inspector, the command line) reads it alike, with one full stop. The run's record keeps the refusal's parts: Messages shows it under its own topic, not under Generation, and Show setting opens the operation at the setting (decision 0536 section 4). A selection-set or STEP-part refusal said before generating is not said again after a run refused with it.
3. **The Facing notes take the form** (the owner's answer (3)), under the topic Facing:
   - "Facing (operation 'X'): facing covered the end faces it was asked to." The Results tab adds "; the simulation shows what it removed" for a simulated run.
   - "Facing (operation 'X'): intentionally unfaced: …".

   The owner's words are kept. Only the lead-in is added, and the first letter follows the form (lower case after the lead-in), as the owner asked for the standard form in full.
4. **The setup checks on the command line** take the form. The setup report's notes (no machine profile, an unreadable part, a tab narrower than a tool) are made in it, and `SetupReport::warning_lines` gives the setup's warnings as Messages says them. `openchuck-cli setup`, `report` and the warnings before a run print them, each with its topic, the setup and one full stop.
5. **Checks.**
   - One guard test runs the command line's setup, validate and report on a project with warnings. It fails on a line at a level, or a listed check, that has no topic and items, no full stop, or "..", ".;" or ".:".
   - `cargo xtask deps`' topics check also fails on a `PathError::refused` whose topic is not a constant of `openchuck_path::topic`.
   - The same check counts, per file, the refusals made with a variant that carries no topic, against a `[strategy_refusals]` baseline in `xtask/messages.toml` that only falls.
   - **No internal references** (OWNERCHECKS2, relayed by the coordinator on 9 Nov 2026, which found "… so links could cross it (OP-14); enlarge the setup's clearance"). `cargo xtask messages` fails on a string literal of product code that names a requirement ("(OP-14)", "SET-02"), an open item ("OI-360") or a decision record ("decision 0495"), counted per file against an `[ids]` baseline that only falls, and the command line's guard test fails on one too. Five texts lose theirs: the links refusal ("… so links could cross it. Enlarge the setup's clearance."), the stock to leave on followed edges (OP-11), two swarf refusals (decision record 0021, OI-94), the Entry setting's description (decision record 0026) and the Tool number tooltip (TL-06). Left in the baseline, as they are not messages: the frame-time benchmark's labels, the header written into the default tools file ("(TL-12). Do not edit.", for the owner to decide at the end, the coordinator says) and the determinism checker's test-support panic. The scan also no longer reads a test module whose `#[cfg(test)]` has other attributes after it.
6. **OI-360 closes** (the owner's answer (4)): closed on 9 Nov 2026 by the owner, relayed; the read of every message a dialog can show is not done.

## Before and after (examples)

| Check | Before | After |
| --- | --- | --- |
| End facing | operation 'Face' cannot be generated: End facing: the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter | End facing (operation 'Face'): the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter, with Show setting at the Stepover |
| Facing check | operation 'Face' cannot be generated: Facing final validation: … | Facing check (operation 'Face'): … |
| Parameter | operation 'Rough' cannot be generated: parameter stepdown: 4.000 mm is more than the tool's 3.000 mm of flute … | Stepdown (operation 'Rough'): 4.000 mm is more than the tool's 3.000 mm of flute …, with Show setting at Stepdown |
| Surface | operation 'Finish' cannot be generated: surface: the mesh has no triangles | Surface (operation 'Finish'): the mesh has no triangles. |
| Rotary axis | operation 'Finish' cannot be generated: A axis: … | Rotary axis (operation 'Finish'): … |
| Links | operation 'X' cannot be generated: operation 'X': cannot connect safely: … | Links (operation 'X'): cannot connect safely: … |
| Links, the retract height | operation 'X' cannot be generated: the stock reaches R mm from the rotary axis, beyond the retract height of H mm, so links could cross it (OP-14); enlarge the setup's clearance | Links (operation 'X'): the stock reaches R mm from the rotary axis, beyond the retract height of H mm, so links could cross it. Enlarge the setup's clearance. |
| Ramp entry (#663) | Ramp entry (operation 'X'): pass 1 is a single point … | the same words, now from the variant, with Show setting at Allow single-point plunge |
| A face set | operation 'Floor finish' cannot be generated: Face-flow finishing needs a face set: … | Selection set (operation 'Floor finish'): face-flow finishing needs a face set: … |
| Machine profile | operation 'X' cannot be generated: the project has no machine profile | Machine profile (operation 'X'): the project has no machine profile. |
| Tool | operation 'X' cannot be generated: it has no tool | Tool (operation 'X'): it has no tool. |
| Facing note | Facing covered the end faces it was asked to. | Facing (operation 'Face'): facing covered the end faces it was asked to. |
| Setup listing | `  - the safe cylinder (radius 6.000 mm) does not enclose the stock …` | `  - Clearance (setup 'Setup 1'): the safe cylinder (radius 6.000 mm) does not enclose the stock ….` |

The full list, with each refusal's topic and setting, is in the archive folder.

## Alternatives considered

- Keep matching the text's start against a list of topics (RAMPENTRY's mechanism): every new prefix needs an edit in the application, and a refusal with no prefix stays out of the form.
- A topic per strategy for every refusal: it would not say which check refused, so the check's topic is used where there is one.
- Reword the strategy texts as well: the texts are kept, so that this step changes the form only; a text that repeats its topic ("Swarf (operation 'X'): swarf cannot cut …") reads acceptably and is left.

## Consequences

- A library-family minor version, sequenced by the Merge Manager with RAMPENTRY's and RINGSTEP's.
- Tests that read refusal texts follow the form.
- OI-360 closes. No new open item.

## Open items

None.

## Verification

See [decision 0536](0536-message-levels-operation-reference-and-setting-links.md), Verification, build step 6.
