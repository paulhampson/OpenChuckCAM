# The topic and setting of each refusal, after decision 0644

What each group of refusal sites takes once decision record 0644 is built (lane MSGREVIEW, decision 0536 step 6). The sites themselves are listed in [before.md](before.md). Setting keys are the operation parameters Show setting opens; "–" means the text names no single operation setting (a setup or machine setting, or none).

## Pathing library (`crates/openchuck-path/src`)

| File / sites | Topic | Setting |
| --- | --- | --- |
| facing/end.rs, end/routing.rs | End facing | Stepover (slice along X), Stepdown (band along the tool), Approach distance, where named; else – |
| facing/coverage.rs, coverage/end.rs | Facing completion | – |
| facing/motion.rs, motion/protection.rs, motion/stock.rs | Facing motion | – |
| facing/plan.rs | Box facing | Tolerance, Stock to leave, Stepdown, Stepover, where named |
| facing/validation.rs, validation/end_layers.rs, legacy.rs, overlap.rs (was "Facing final validation") | Facing check | Stepover, Stepdown in end_layers.rs |
| roughing/scope.rs | Facing extent | Stepover ("increase the stepover") |
| facing.rs | Facing extent (extension and projection bound); Facing (whole-setup geometry, legacy mismatch) | – |
| facing/geometry.rs (which end is held) | End facing | – |
| program.rs (`connect_group`) | Links ("cannot connect safely: …", no operation name) | – |
| assemble/entry.rs, Ramp entry | Ramp entry | single point: Allow single-point plunge; too short: Entry; too many legs: Ramp angle; defect: Entry; turns on the axis: Maximum ramp angle or Ramp angle, as its remedy says; A range: Entry |
| assemble/entry.rs, face nubs | Face nubs | Plunge to remove face nubs |
| assemble/entry.rs `refuse()` | Lead entry (lead arcs), Helix entry (helix) | Entry; the helix segment count: Ramp angle |
| assemble.rs | Entry (stock above the retract height on the way to an entry); Links (collision, stock above the retract height); Toolpath (a non-finite point, no cut for the selection) | Entry for the first; else – |
| strategy.rs | Region (region and boundary options); Selection set (different stock per selection); Held region (cuts over a held region) | – |
| roughing.rs, field.rs | Rotary roughing; Stock (the stock model gives no size or ends); Facing (facing has no cut) | – |
| stock.rs | Stock | – |
| axial.rs, helical.rs, simultaneous.rs | Axial finishing, Helical finishing, Simultaneous 4-axis finishing; Surface ("no surface to finish", also indexed.rs) | – |
| faces.rs, extend.rs | Faces across the axis, Past the ends | – |
| wrap_fit.rs | Region | Tolerance |
| cutter.rs | Stock to leave | Stock to leave |
| rest.rs | Rest machining; Region | nothing left to clean: Minimum remaining |
| pencil.rs | Pencil clean-up; Region | Crease angle; nothing left: Minimum remaining |
| swarf.rs | Swarf; Region ("not a region") | – |
| face_flow.rs | Face-flow finishing | too many passes: Stepover |
| edge_following.rs | Edge following; Region | no edge to follow: Crease angle |
| drill.rs | Radial drilling; Region | – |
| `Parameter` (params.rs and every `PathError::parameter`) | the field's label, in the application | the parameter |
| `Surface`, `Region`, `RotaryLimit` (brep.rs, brep_edges.rs, curves.rs, drill.rs, region.rs, rotary.rs, solid.rs, surface.rs, unroll.rs) | Surface, Region, Rotary axis, in the application | – |

Kept as `PathError::Strategy`: the registry's "a strategy needs a name" and "a strategy named … is already registered" (strategy.rs), which no user sees. The `[strategy_refusals]` baseline in `xtask/messages.toml` counts these and the geometry's own errors, and only falls.

New topic constants: Stock to leave, Stock, Toolpath, Facing, Rotary roughing, Axial finishing, Helical finishing, Simultaneous 4-axis finishing, Radial drilling (with End facing, Facing completion, Facing motion, Box facing, Facing check, Facing extent, Surface, Region, Lead entry, Helix entry and Setting from the variant's commit).

## Application (`crates/openchuck-app/src`)

| Site | Topic | Setting |
| --- | --- | --- |
| operation.rs: no machine profile | Machine profile | – |
| operation.rs, pipeline.rs, opflow.rs: setup missing | Setup | – |
| operation.rs, opflow.rs: no tool, tool missing | Tool | – |
| operation.rs: no part, part missing, no extent | Part | – |
| operation.rs, pipeline.rs: no stock, facing stock bounds | Stock | – |
| operation.rs: required values the cutting data cannot fill | Cutting data | the first missing value |
| operation.rs: B-rep only, keep tangent edges; opflow.rs: unavailable on the setup | STEP part | – |
| operation.rs, selection.rs, opflow.rs: selection sets that cannot be used | Selection set | – |
| operation.rs: workholding cannot be built | Workholding | – |
| operation.rs: extended past the stock beyond the travel | Travel | – |
| operation.rs: different stock on different selections | Stock to leave | – |
| operation.rs: swarf face set | Swarf | – |
| operation.rs: a plunge into a single-point pass with a tool that cannot plunge (#663) | Ramp entry or Lead entry | Entry |
| opflow.rs: a name, the strategy; pipeline.rs: turned off | Operation | – |
| opflow.rs, preset.rs: the material | Material | – |
| form.rs: no such parameter | Setting | – |
| lib.rs: All four needs Box sides | Box facing | – |
| connections.rs | the path refusal's topic (Links) | – |
| the setup report's notes | Machine profile, Model file, Tab length (setup '<name>') | – |
| the Facing notes | Facing | – |
