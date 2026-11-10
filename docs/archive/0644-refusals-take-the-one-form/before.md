## Refusal-site inventory: `origin/main`, #656 (`lane-pathfeed-641-wip`) and #663 (`lane-rampentry-pr1`)

I read everything with `git show` / `git grep` / `git diff`. Nothing was edited or checked out.

**How the branches relate to main**
- **#656** is based on #648 (`645256c3`). After that point main changed some Facing wording from "verified/certified" to "checked", in `facing/validation.rs:85,140`, `validation/end_layers.rs:52` and `validation/legacy.rs`. #656 still has the old wording.
- **#663** is based on #652 (`01aa5e32`). Since then main changed only `assemble.rs`, `facing.rs`, `roughing.rs` and `strategy.rs`, and none of those changes touch refusal text. Line numbers in those four files are slightly shifted on #663.
- Any file not named in a branch section below is the same on all three refs, so the main line numbers apply.

**`PathError` variants** (`crates/openchuck-path/src/error.rs`, identical on all three refs):
- `Data(types::Error)` – transparent
- `Parameter{name,reason}` – displays as "parameter {name}: {reason}"; the shorthand is `PathError::parameter(..)`
- `UnknownStrategy(String)` – "no strategy named \"{0}\" is available"
- `Surface` – "surface: {0}"
- `Region` – "region: {0}"
- `RotaryLimit` – "A axis: {0}"
- `Strategy` – "{0}", no prefix
- `Cancelled`

So the only built-in topic-like prefixes are `parameter X:`, `surface:`, `region:` and `A axis:`. Every other prefix is part of the text itself.

**How the app shows them on main and #656** (`operation.rs:360` `named()`): every `AppError::Path` except Cancelled becomes `AppError::Operation{name, reason: err.to_string()}`, which displays as "operation '{name}' cannot be generated: {reason}".

**Column key for the tables:** P = leads with a prefix/topic. Key = the operation setting the text tells the user to change ("–" means none named).

---

### A. `origin/main`, `crates/openchuck-path/src`

**Prefix wrappers.** Each of these files has a private helper (`fn refuse(reason:&str)->PathError` unless named otherwise) that wraps its texts as `PathError::Strategy(format!("<Prefix>: {reason}"))`:

| File:line of helper | Prefix | Call sites on main |
|---|---|---|
| facing/end.rs:442 | "End facing: " | 20 |
| facing/end/routing.rs | "End facing: " (`end.rs`'s `refuse`) | 12, plus `too_deep` at :245/:252 called from :207 |
| facing/coverage.rs:119 | "Facing completion: " | 7 |
| facing/coverage/end.rs | "Facing completion: " (same helper) | 11 |
| facing/motion.rs:323 | "Facing motion: " | 18 |
| facing/motion/protection.rs | "Facing motion: " | 12 |
| facing/motion/stock.rs | "Facing motion: " | 10 |
| facing/plan.rs:276 | "Box facing: " | 16 |
| facing/validation.rs:55 | "Facing final validation: " | 18 |
| facing/validation/end_layers.rs | "Facing final validation: " | 11 |
| facing/validation/legacy.rs | "Facing final validation: " | 11 |
| facing/validation/overlap.rs | "Facing final validation: " | 2 |
| roughing/scope.rs:157 | "Facing extent: " | 9 |
| program.rs:139 `refuse(group, ..)` | "operation '{op}': cannot connect safely: " | 15 |
| facing/geometry.rs:380 `invalid(field, ..)` | none; becomes `PathError::parameter(field, ..)` with free-text field names | 17, including `nonnegative()` |
| assemble/ramp_refusal.rs (main only) | none | 10 String builders |
| entry_safety.rs | none | 1 |

**A1. End facing (prefix "End facing: ")**

| Site | Text | Key |
|---|---|---|
| end.rs:54 | expected End settings | – |
| :119 | held intervals must be finite with positive length | – |
| :125 | this straight-flank construction requires a flat end mill with no larger shank | tool |
| :133 | axial stepdown must be above zero and no greater than cutting diameter | `roughing::STEPDOWN` (label "Axial stepdown") |
| :141 | flute band spacing must be above zero and no greater than flute length | `names::STEPOVER` (label "Flute band spacing") |
| :149 | end view angles must be finite with positive maximum spacing | – |
| :158 | invalid stock allowance or tolerance | – |
| :161 | the approach distance must be finite and not negative | `names::APPROACH_DISTANCE` (implied) |
| :169 | the whole placed source has no established projection-error bound | – |
| :172 | the source projection bound exceeds its geometric error budget | – |
| :193 | the stock ends at the part's end at X {..}: there is no stock past this end to face. If the real blank is longer, set the stock's length… | setup stock length |
| :202 | the target end plane is outside supplied stock | – |
| :214 | the requested end has no positive-area extent | – |
| :247 | end facing pass budget exceeded | – |
| :268 | cannot establish complete-tool axial air clearance at these coordinates | – |
| :290 | flank compensation overflowed | – |
| :388 | tab access bounds overflowed | – |
| :409 | tab access bounds overflowed | – |
| :434 | an outward-widening tab has an inaccessible {..} mm radial band…; this straight flank cannot finish it… | – |
| routing.rs:58 | complete end routing for this tab placement is not established | – |
| :73 | complete routing around a non-cylindrical tab is not established… | – |
| :89 | tab-facing view budget exceeded at this tolerance | – |
| :96 | the tab-facing remainder exceeds its geometric budget | – |
| :103 | complete end routing around multiple tabs is not established | – |
| :145 | end-view precision bound overflowed | – |
| :152 | end view angle overflowed | – |
| :163 | end view angles have no geometric precision bound… | – |
| :195 | end view stock top overflowed | – |
| :210 | end view projection overflowed | – |
| :217 | the flutes are too short to overlap the bands by the written-motion allowance | tool |
| :234 | end flute-band budget exceeded | – |
| :252 (`too_deep`, called from :207) | "End facing: the end at X {..} needs the tool {..} mm below the top of the stock at A {..}, more than the tool's {..} mm of flute, {from}, so the tool cannot face it. Use a tool with longer flutes, or face the stock down with Box facing first" | tool / Box facing |

**A2. Facing completion (prefix "Facing completion: ")**

coverage.rs:
- :46 invalid target, flat cutting radius or tolerance
- :49 coverage work limit exceeded
- :57 a flat raster sweep must keep physical A fixed
- :64 the cutting disc has no supported facing projection
- :72 a supplied sweep overcuts the requested plane
- :105 an unfinished strip remains inside the requested extent
- :114 the requested face is not completely covered by final-depth sweeps

coverage/end.rs:
- :48 unsupported end tool or invalid coverage/error budget
- :53 end coverage move budget exceeded
- :67 end coverage must have positive area
- :73 end coverage start is not finite
- :82 an end cut goes behind the finished target plane
- :117 end coverage sweep budget exceeded
- :126 end coverage coordinates have no adequate arithmetic precision
- :134 invalid end coverage slab
- :215 end coverage polygon budget exceeded
- :259 the actual end sweeps leave an unfinished required region in an axial layer
- :313 end coverage work budget exceeded

No key is named in any of these.

**A3. Facing motion (prefix "Facing motion: ")**

motion.rs:
- :25 radial clearance requires a whole-source projection bound
- :49 setup has no finite radial bound
- :71 rapid envelope has no supported complete-tool bound
- :76 rapid clearance margin must be finite and nonnegative
- :83 rapid clearance requires a whole-source projection bound
- :98 rapid geometry has no finite whole-setup bound
- :146 rapid projection work budget exceeded
- :159 rapid support is unavailable
- :173 rapid support is unavailable
- :220 rapid move {..} has no continuous complete-tool separation from stock, the whole part and protected geometry
- :246 motion verification budget exceeded
- :252 a motion pose is not finite
- :257 a motion pose exceeds A travel
- :260 the tool axis does not match the fixed spindle at this A
- :266 a motion pose exceeds machine linear travel
- :303 this rotary feed rule has no verification bound
- :312 move {..} is timed at {..} s but needs at least {..} s for the machine's axis and coupled feed limits

protection.rs:
- :35 protection proof requires a supported flat tool
- :38 protection tolerance and margin must be finite and nonnegative
- :45 protection proof requires a whole-source projection bound
- :68 protection geometry has no finite bound
- :88 protection proof geometry budget exceeded
- :96 protection geometry has no finite arithmetic bound
- :110 complete-tool envelope has no finite bound
- :122 protection proof geometry budget exceeded
- :129 whole-source projection is unavailable
- :136 move {index} has no continuous complete-tool separation from the whole part
- :143 move {index} … from protected geometry

stock.rs:
- :37 shank proof requires a supported flat tool
- :40 shank error margin must be finite and nonnegative
- :111 rotary shank move has no continuous remaining-stock separation
- :142 move {..} shank reaches remaining stock without actual prior removal
- :150 shank capsule budget exceeded
- :180 shank proof removal budget exceeded
- :213 shank proof has no finite geometry bound
- :248 invalid shank proof slab
- :264 invalid shank proof slab
- :294 shank proof union budget exceeded

No key is named in any of these.

**A4. Box facing (prefix "Box facing: "), plan.rs**

| Site | Text | Key |
|---|---|---|
| :73 | expected Box sides settings | – |
| :78 | unknown layer distribution | – |
| :135 | Box sides requires box/block stock | – |
| :138 | this box-side construction requires a flat end mill | – |
| :143 | the larger shank has no supported complete-tool bound… | – |
| :148 | tolerance must be finite and above zero | `names::TOLERANCE` |
| :151 | stock to leave must be finite and nonnegative | `names::STOCK_TO_LEAVE` |
| :157 | the whole placed source has no established projection-error bound | – |
| :160 | the source projection bound exceeds its geometric error budget | – |
| :168 | stepdown must be above zero and no greater than flute length | `roughing::STEPDOWN` |
| :176 | stepover must be above zero and no greater than flat cutting diameter | `names::STEPOVER` |
| :182 | held intervals must be finite with positive length | – |
| :215 | the target plane lies outside supplied stock | – |
| :223 | the requested side has no positive-area extent | – |
| :229 | raster row budget exceeded | – |
| :236 | facing pass budget exceeded | – |

**A5. Facing final validation (prefix "Facing final validation: ")**

validation.rs:
- :32 expected End geometry
- :66 whole placed setup geometry is required
- :71 the actual tool profile or rotary frame differs from the whole setup used for verification
- :83 the whole source has no established projection bound
- :85 nominal Facing completion currently requires exact placed triangles; nonzero source projection uncertainty is not checked
- :91 held intervals must be finite with positive length
- :118 unsupported Box stock/tool/engagement settings
- :121 expected Box geometry
- :124 Box completion work budget exceeded
- :140 an already satisfied Box plane has no checked removal evidence
- :156 Box tolerance/stepdown is below the supported arithmetic resolution
- :172 invalid Box layer index
- :194 selected Box views cannot be distinguished at this motion precision
- :212 a Box move goes below its requested finished plane
- :234 an actual Box cut exceeds the selected maximum stepdown
- :246 deeper Box cutter overlap has no actual prior full-footprint removal…
- :259 Box layers do not reach the final target
- :314 entry/link would remove stock outside selected Facing coverage and its necessary cutter overlap

end_layers.rs:
- :43 End axial/flute spacing exceeds the supported cutting limits
- :52 End has no independently checked removal slice
- :60 End tolerance/stepdown is below the supported arithmetic resolution
- :90 invalid End slice
- :93 End slice verification work budget exceeded
- :109 End has no actual preceding flute sweep
- :118 End cut goes behind the finished target
- :121 an actual End cut exceeds the selected maximum axial stepdown (key: `roughing::STEPDOWN` on main)
- :144 End overlap engagement work budget exceeded
- :157 deeper End flute overlap has no actual prior full-band removal at this view
- :172 End actual slices do not reach the final target

legacy.rs:
- :60 expected legacy Flat/Round geometry
- :91 Round completion has no supported interpolation/flute precision budget at this tolerance
- :97 Round cutter is below its interpolation precision budget
- :111 Round cut overcuts the nominal radius
- :118 raised/off-axis Round composite-target completion is not checked
- :129 Round segment exceeds one supported physical turn
- :160 Round coverage work budget exceeded
- :164 invalid Round phase slab
- :191 Round has an unfinished axial/angular strip; full circumference is required
- :197 Round has no checked full-circumference finish
- :215 restricted legacy Flat has an unfinished or raised composite-target region; … not checked

overlap.rs:
- :26 Box layer engagement requires a supported fixed-X across-going cut
- :65 Box overlap engagement work budget exceeded

**A6. Facing extent (prefix "Facing extent: "), roughing/scope.rs**

| Site | Text | Key |
|---|---|---|
| :27 | Flat coverage must have one rectangle per listed A | – |
| :35 | coverage geometry does not match the Facing mode | – |
| :50 | selected extent does not reach a stock edge requested for extension | – |
| :70 | held limits must be finite and ordered | – |
| :80 | selected extent crosses an internal held interval requiring unsupported split coverage | – |
| :86 | selected extent has no reachable axial span outside held regions | – |
| :111 | selected extent does not reach the stock sides requested for extension | – |
| :125 | scoped Flat line budget exceeded; increase stepover | `names::STEPOVER` |
| :153 | coverage bounds must be finite with positive width | – |

**A7. Facing, other sites with no wrapper**

| Site | Variant | Text | P | Key |
|---|---|---|---|---|
| facing.rs:312 | Strategy | Stock-edge extension for this new Facing construction is not yet supported; … Disable extension or use legacy Flat/Round. | no | extension / shape |
| facing.rs:341 | Strategy | Facing requires whole-setup geometry for this mode | starts with "Facing" (no colon) | – |
| facing.rs:346 | Strategy | Facing extent requires an established whole-source projection bound | same | – |
| facing.rs:359 | Strategy | Legacy Facing coverage does not match the selected mode | no | – |
| facing/geometry.rs:39 | Strategy | Define which end is held before facing the opposite end. | no | setup workholding |
| facing/geometry.rs:42 | Strategy | Cannot face an unheld end: both ends are held. | no | – |
| facing/description.rs:99 | parameter("projection error") | must be finite and nonnegative, or explicitly unknown | parameter | – |
| facing/description.rs:167 | parameter(`parameter::SHAPE`) | unknown facing mode | parameter | SHAPE |
| facing/extents.rs:33 | parameter("extent") | unknown facing extent | parameter | – |
| facing/extents.rs:77 | parameter("axial margin") | must be finite and nonnegative | parameter | – |
| facing/extents.rs:86 | parameter("axial margin") | expanded extent overflowed | parameter | – |
| facing/extents.rs:135 | parameter("facing extent") | this stock type has no supported projection | parameter | – |
| facing/extents.rs:148 | parameter("facing geometry") | the whole placed mesh has no triangles | parameter | – |

facing/geometry.rs `invalid(field, ..)` → `PathError::parameter`:
- :52 ("part bounds") must be finite and ordered
- `nonnegative("stock to leave")`
- :61 ("target end") coordinate or finished length overflowed
- :86 ("facing layers") target must be finite and at or below stock support
- :92 ("stepdown") must be finite and above zero
- :97 ("stepdown") would require more than 100000 facing layers
- :118 ("stepdown") cannot make progress…
- :195 / :209 / :268 / :303 / :335 / :374 / :400 ("facing geometry") point projection is not finite / planned point not finite / every placed vertex must be finite (×3) / projection overflowed / cannot project empty geometry
- :229 ("facing angle") must be finite
- :263 `nonnegative("facing margin")`
- :292 ("facing margin") expanded projection overflowed
- :388 generic "must be finite and nonnegative"

facing/selection.rs `PathError::parameter`:
- :38 ("arbitrary_faces") row data exceeds 1 MiB
- :44 ("arbitrary_faces") invalid indexed-face rows: {e}
- :54 ("arbitrary_faces") {e}
- :59 ("arbitrary_faces") more than 4096 arbitrary faces
- :151 ("facing sides") the row to move no longer exists
- :169 ("stock to leave") must be finite and nonnegative
- :175 ("facing sides") more than 4096 arbitrary faces
- :191 ("facing sides") select at least one face
- :206 ("face offset") derived target overflowed
- :212 ("face offset") would cut into the part or its stock allowance
- :223 ("facing sides") duplicate orientation has conflicting plane offsets
- :247 ("facing angle") must be finite
- :252 ("face offset") must be finite

roughing.rs facing paths:
- :545 Strategy: facing needs the stock's ends, and the stock model gives none
- :553 `Parameter{facing::parameter::SIZE}`: the face would be {..} mm from the rotary axis, and the stock reaches {..} mm: there is nothing to face
- :575 Strategy: facing has no cut to make: no material stands more than {..} mm above the face
- :1550 parameter(`facing::parameter::ANGLES`): at least one angle to face at

**A8. Entry and ramp (main only: all of these are removed on #663)**

The `assemble/ramp_refusal.rs` builders are all wrapped in `PathError::Strategy` and none has a prefix. They all name the Entry setting `names::ENTRY` and/or `names::RAMP_BACK_AND_FORTH`, through `ENTRY_FIX` = "In the operation's Entry, links and retract settings, turn off Back-and-forth ramp or set Entry to Plunge, Lead or Helix."

| Builder (:line) | Called from entry.rs | Text | Extra key |
|---|---|---|---|
| `missing_inputs` :14 | :293 | The back-and-forth ramp cannot check its clearance in this operation: it needs the whole part… {ENTRY_FIX} | – |
| `no_direction` :24 | :296 | Pass {..} {is a single point / only turns the part…}, so the back-and-forth ramp … has no straight line to ramp along. {ENTRY_FIX} | – |
| `no_stock_size` :39 | :301 | The back-and-forth ramp starts above the whole stock… set the stock's shape and size, or change the entry: … | setup stock |
| `above_clearance` :50 | :312 | Pass {..}: the back-and-forth ramp starts {..} mm from the rotary axis… set {field} to at least {..} mm; or … lower Approach distance, turn off Back-and-forth ramp or set Entry to… | setup clearance `{field}`, `names::APPROACH_DISTANCE` |
| `nothing_to_descend` :63 | :321 | Pass {..} starts {..} mm above the stock, so the back-and-forth ramp has nothing to ramp down into… | – |
| `corridor` :76 | :361 | Back-and-forth ramp needs at least {..} mm of usable tool-centre travel; only 0.000 mm could be verified: within one tool diameter…, {cause}. {fix} | – |
| `plan` :89 (4 arms: TooMany / NoProgress / TooShort / Invalid) | :370 | Pass {n}: the back-and-forth ramp would need more than 10000 runs… / descends too little… / has less than one tool diameter… / cannot be planned… | `names::RAMP_ANGLE` |
| `blocked` :117 | :344, :428 | Pass {..}: the back-and-forth ramp cannot be used here: {cause}. {fix} | `Blocked::fix()` names setup Workholding, Machine Travel, A range, or the tool |
| `into_part` :127 | :437 via `ramp_refused` :646 | Pass {..}: the back-and-forth ramp cannot be used here: the tool would cut {..} mm into the finished part… | – |
| `over_held` :136 | :437 via `ramp_refused` :646 | Pass {..}: … would run over {held} at X {..} mm… move the held region… | setup workholding |

Other entry sites on main:

| Site | Variant | Text | P | Key | Status |
|---|---|---|---|---|---|
| entry_safety.rs:68 | Strategy | The back-and-forth ramp cannot be used here: {cause}. {fix} | no | – | main only |
| assemble/entry.rs:506 | Strategy | pass {..}: the ramp starts {..} mm back from the first cut point, where the stock is {..} mm higher…; use a steeper angle, another entry style or a larger approach distance | no | `RAMP_ANGLE`, `ENTRY`, `APPROACH_DISTANCE` | main only (not in #663's `entry.rs`) |
| assemble/entry.rs:547 | Strategy | pass {..}: the helix entry needs {..} segments for {..} turns; use a larger ramp angle or a larger helix diameter | no | `RAMP_ANGLE`, `HELIX_DIAMETER` | all refs (#663 :545) |
| entry.rs `refuse()` :671 → :675 | Strategy | pass {..}: the {name} needs {size}, and it would run over {held} at X {..} mm, where the workholding grips the stock; use a plunge entry, or a size or angle that keeps it clear… | no | `ENTRY`, `LEAD_RADIUS` / `RAMP_ANGLE` / `HELIX_DIAMETER` | all refs (#663 :1054) |
| entry.rs `refuse()` :671 → :683 | Strategy | pass {..}: the {name} needs {size}, and the tool would cut {..} mm into the finished surface at position {..}…; try another entry style, a smaller size or a steeper angle | no | same | all refs (#663 :1062) |

The `refuse()` above is called from:
- main: :186 and :269 (lead arcs, size "an arc of radius {r} mm"), :481 (ramp, "a run of {..} mm to descend {..} mm at {..} degrees"), :566 (helix, "a circle {..} mm across and {..} turns…")
- #663: :201, :294, :564

**A9. Other strategies on main (all have no prefix unless the Variant column shows Surface/Region/RotaryLimit/parameter)**

| Site | Variant | Text | Key |
|---|---|---|---|
| adaptive.rs:240 | parameter(height) | the allowed excess must be a finite number of millimetres, at least 0 | adaptive height key |
| adaptive.rs:247 | parameter(`MIN_SPACING`) | the smallest spacing must be … above 0 | `MIN_SPACING` |
| assemble.rs:202 | Strategy | move {..} ({kind}) collides, and no way round it is known | – |
| assemble.rs:371 | Strategy | pass {..} point {..} is not a finite position | – |
| assemble.rs:422 | Strategy | the stock reaches {..} mm from the rotary axis, beyond the retract height of {..} mm, so links could cross it (OP-14); enlarge the setup's clearance | setup clearance |
| assemble.rs:481 | Strategy | the strategy produced no cut for this selection | – |
| axial.rs:320 | `Parameter{STEPOVER}` | the stepover needs too many passes; increase it | `STEPOVER` |
| axial.rs:385 | Strategy | the part has no length along the rotary axis to finish | – |
| axial.rs:393 | Strategy | the part has no radius about the rotary axis to finish | – |
| axial.rs:410 | `Parameter{MAX_X_STEP}` | {..} points per pass is too many; increase the step | `MAX_X_STEP` |
| axial.rs:449 | `Parameter{MAX_X_STEP}` | {..} passes of {..} points each is too many; increase the stepover or the largest step | `MAX_X_STEP`, `STEPOVER` |
| axial.rs:883, helical.rs:847, indexed.rs:521, simultaneous.rs:1376 | Strategy | no surface to finish | – |
| brep.rs:85 (`bad()`) | Surface | face {..}: {what} | – |
| brep.rs:1081 | Surface | b-spline cannot be evaluated | – |
| brep.rs:2141 | Surface | face {..} is not in the model | – |
| brep.rs:2180 | Surface | the model has no faces | – |
| brep_edges.rs:170 | Surface | edge {..} is not in the model | – |
| brep_edges.rs:179 | Surface | none of the chosen edges can be followed: they are seams or have no length | – |
| brep_edges.rs:262, curves.rs:91 | Surface | the crease angle and the shortest chain must be finite numbers | crease angle, shortest chain |
| curves.rs:119 | Surface | every point of a chosen edge must be finite | – |
| curves.rs:124 | Surface | none of the chosen edges has length | – |
| curves.rs:150 | Surface | the shortest chain must be a finite number | – |
| cutter.rs:791 | Strategy | the stock to leave {..} mm must be finite and not negative | `STOCK_TO_LEAVE` |
| cutter.rs:799 | Strategy | stock to leave is not supported for this tool: its profile is not convex…; set the stock to leave to 0 or use a convex tool | `STOCK_TO_LEAVE` |
| drill.rs:267 | Strategy | radial drilling takes its holes from the selected faces, not from a region | – |
| drill.rs:316 | Strategy | there is no hole to drill: radial drilling finds round holes… | – |
| drill.rs:368 | Strategy | no hole can be drilled with this tool: {..} | – |
| drill.rs:437 | RotaryLimit | {which} needs A = {..} degrees, which the machine's range of {..} to {..} does not reach | machine A range |
| drill.rs:504 | Strategy | {site} has no depth to drill | – |
| drill.rs:524 | parameter(`PECK_DEPTH`) | {..} mm would make {..} pecks in {..} | `PECK_DEPTH` |
| edge_following.rs:356 | Strategy | the strategy '{NAME}' cannot be limited to a region; select the edges to follow | – |
| edge_following.rs:374 | Strategy | edge following has no edge to follow: the operation's own edges gave no chain | – |
| edge_following.rs:379 | Strategy | edge following has no edge to follow: the part has no edge turning {..} degrees…; lower {} or {} | `CREASE_ANGLE`, `MIN_CHAIN_LENGTH` (via `parameter::` consts) |
| edge_following.rs:442 | parameter(`LINE_WIDTH`) | {..} mm is wider than the tool's {..} mm cutting diameter | `LINE_WIDTH` |
| edge_following.rs:452 | parameter(`LINE_WIDTH`) | {..} mm is not wider than this tool's tip…; ask for a wider line or use a tool with a finer point | `LINE_WIDTH` |
| edge_following.rs:465 | parameter(`DEPTH`) | a depth of {..} mm is more than the tool's {..} mm flute length | `DEPTH` |
| edge_following.rs:480 | parameter(`DEPTH_PER_PASS`) | {..} levels are needed; increase the depth per pass | `DEPTH_PER_PASS` |
| edge_following.rs:494 | parameter(`CARVE_WIDTH`) | a carve needs a width greater than zero | `CARVE_WIDTH` |
| edge_following.rs:501 | parameter(`CARVE_WIDTH`) | {..} mm is narrower than the {..} mm line…; widen the carve or use a thinner tool | `CARVE_WIDTH` |
| edge_following.rs:516 | parameter(`names::STEPOVER`) | {..} lines across the carve are needed; increase the stepover | `STEPOVER` |
| edge_following.rs:700 | Strategy | a chain passes within {..} mm of the rotary axis at (..), where A cannot face it… | – |
| edge_following.rs:707 | Strategy | a chain point lies on the rotary axis | – |
| edge_following.rs:717 | Strategy | a point of a curve at (..) does not project onto the surface along the tool axis | – |
| edge_following.rs:804 | parameter(`names::ENTRY`) | edge following cuts below the surface, where a lead, ramp or helix entry cannot be checked…; use a plunge entry | `ENTRY` |
| edge_following.rs:818 | parameter(`MAX_STEP`) | {..} points are needed along the chains; increase the largest step, the depth per pass or the stepover | `MAX_STEP`, `DEPTH_PER_PASS`, `STEPOVER` |
| edge_following.rs:868 | Strategy | every chain runs along the tool axis…; engrave it / edge following found no chain with two points to cut | – |
| extend.rs:145 | `Parameter{EXTEND_DISTANCE}` | {..} mm is more than the tool's diameter ({..} mm); the most is the diameter | `EXTEND_DISTANCE` |
| extend.rs:159 | Strategy | {label} cannot extend past the stock: the stock model gives no ends | – |
| extend.rs:173 | `Parameter{EXTEND}` | the {which} of the stock is held by the workholding, so {label} cannot extend past it (SET-09); extend past the other end only, or not at all | `EXTEND` |
| face_flow.rs:296 | Strategy | face-flow finishing needs the exact faces of a STEP model and the faces to cut… | – |
| face_flow.rs:494 | Strategy | edge {..} of face {..} has no points to follow | – |
| face_flow.rs:526 | Strategy | the boundary of face {..} does not close… | – |
| face_flow.rs:665 | Strategy | face {..} has no boundary edges to follow | – |
| face_flow.rs:730 | Strategy | the guide edges {..} of face {..} are not joined end to end into one chain | – |
| face_flow.rs:738 | Strategy | the guide of face {..} is its whole boundary: choose fewer edges… | – |
| face_flow.rs:761 | Strategy | face {..} has an empty side | – |
| face_flow.rs:941, :1668, :1716, :1726 | Strategy | the model has no face {..} | – |
| face_flow.rs:954 | Strategy | the boundary of face {..} could not be put on its surface | – |
| face_flow.rs:965 | Strategy | the guide of face {..} is empty | – |
| face_flow.rs:968 | Strategy | the opposite side of face {..} is empty | – |
| face_flow.rs:1120 | parameter(`names::STEPOVER`) | the stepover (or scallop height) is too small for this face to be cut | `STEPOVER` |
| face_flow.rs:1134 | Strategy | face {..} would need more than {MAX_PASSES} passes: raise the stepover | `STEPOVER` |
| face_flow.rs:1663 | Strategy | face-flow finishing needs at least one face to cut: select the faces | – |
| face_flow.rs:1675 | Strategy | the model has no edge {..} to use as the guide | – |
| face_flow.rs:1688 | Strategy | guide edge {..} is not on the boundary of the selected faces… | – |
| face_flow.rs:1695 | Strategy | face-flow finishing needs a tool whose profile is convex | – |
| face_flow.rs:2035 | Strategy | face-flow finishing cannot reach any of the selected faces: {why} | – |
| faces.rs:360 | Strategy | the part has {..} faces across the rotary axis; the most this strategy turns at is {..} | – |
| field.rs:66 | Strategy | the roughing field needs a positive length, radius and cell size | – |
| finishing.rs:502 | parameter(`FINISH_ALLOWANCE`) | must be less than the Stepdown, {..} mm; it is {..} mm | `FINISH_ALLOWANCE` |
| finishing.rs:509 | parameter(`STEPDOWN`) | {..} mm is more than the tool's {..} mm of flute… | `STEPDOWN` |
| helical.rs:357 | Strategy | the part has no length along the rotary axis to finish | – |
| helical.rs:380 | `Parameter{STEPOVER}` | {..} mm over {..} mm of part needs {..} points at {..} degrees; increase the stepover or the largest angle step | `STEPOVER` |
| indexed.rs:281 | parameter(`COUNT`) | at least one position | `COUNT` |
| params.rs:151 | parameter(unknown) | this strategy has no parameter of that name | – |
| params.rs:167–335 | parameter(name) | generic type/range validation: expected {..} / a value is required / not a finite number / {v} {unit} must be {relation} {lo} / must be at most {hi} / must be from {min} to {max} / "{v}" is not one of {..} / angle-list errors / expected a number / whole number / true or false / text | the named key itself |
| params.rs:593 (#663 :667) | parameter(`names::ENTRY`) | this tool cannot cut straight down (it is not centre-cutting); choose ramp or helix entry, or state on the tool that it can plunge | `ENTRY` |
| pencil.rs:625 | Strategy | the strategy '{NAME}' cannot yet be limited to a region | – |
| pencil.rs:672 | Strategy | pencil clean-up has no inside corner to follow: {..}; lower {} | `CREASE_ANGLE` |
| pencil.rs:683 | parameter(`MAX_STEP`) | {..} points are needed along the creases; increase the largest step | `MAX_STEP` |
| pencil.rs:703 | Strategy, `rest::nothing_left("pencil clean-up", ..)` | – | – |
| pencil.rs:785 | Strategy | pencil clean-up found material left but the tool cannot rest in any corner that holds it | – |
| program.rs:62 | `refuse(group, ..)` "operation '{op}': cannot connect safely: " | the enclosing radius is not a finite non-negative bound | – |
| program.rs:68 | same prefix | a connection cannot cross a setup/program boundary | – |
| program.rs:123 | same prefix | invalid pass boundary index | – |
| program.rs:157 | same prefix | an A coordinate is not finite | – |
| program.rs:170 | same prefix | the complete operation cannot fit within the A limits by whole turns | – |
| program.rs:178 | same prefix | the required A is not equivalent to the operation's start orientation | – |
| program.rs:186 | same prefix | the preceding A coordinate is not finite | – |
| program.rs:197 | same prefix | the required placement exceeds the A limits | – |
| program.rs:209 | same prefix | a placed coordinate is not finite | – |
| program.rs:214 | same prefix | a placed coordinate exceeds the A limits | – |
| program.rs:231 | same prefix | connection position (..) exceeds the machine's linear travel | – |
| program.rs:243 | same prefix | the operation boundary is below the required clear plane; enlarge the setup's clearance | setup clearance |
| program.rs:249 | same prefix | the boundary tool axis is not the supported vertical orientation | – |
| region.rs:75 | Surface | a boundary vertex is missing | – |
| region.rs:83 | Surface | the containment boundary has no triangles | – |
| rest.rs:118 | Strategy | remaining-material detection needs a threshold of zero or more and a positive spacing and reach | – |
| rest.rs:151 | Strategy | … more than the {MAX} allowed | – |
| rest.rs:425 | Strategy | the strategy '{NAME}' cannot yet be limited to a region | – |
| rest.rs:445 | Strategy | the part has no length along the rotary axis to clean | – |
| rest.rs:462 | Strategy, `nothing_left("rest machining", ..)` | – | – |
| rest.rs:474 | `Parameter{STEPOVER}` | {..} mm over {..} mm of part needs {..} points…; increase the stepover or the largest angle step | `STEPOVER` |
| rest.rs:593 | Strategy | rest machining found material left but the tool cannot reach any of it… | – |
| rotary.rs:188 | RotaryLimit | {name} turns A through {..} degrees, more than the machine's range of {..} | machine A range |
| rotary.rs:201 | RotaryLimit | {name} cannot be placed inside the machine's A range (..) by whole turns | machine A range |
| roughing.rs:481 | Strategy | rotary roughing has no cut to make for this selection: no material stands more than {..} mm above the part… (two variants) | – |
| roughing.rs:703 | Strategy | rotary roughing needs the stock's size, and the stock model gives none | – |
| roughing.rs:712 | parameter(`STEPDOWN`) | {..} mm is more than the tool's {..} mm of flute… Use a smaller Stepdown or a tool with… | `STEPDOWN` |
| roughing.rs:729 | Strategy | the part has no length along the rotary axis to rough | – |
| roughing.rs:884 | Strategy | there is no material to rough: the stock reaches {..} mm and the part… reaches down to {..} mm | – |
| roughing.rs:892 | `Parameter{STEPDOWN}` | {..} mm of depth in steps of {..} mm is {..} layers; the most is {MAX_LAYERS} | `STEPDOWN` |
| roughing.rs:1127 | `Parameter{STEPOVER}` | {..} mm of length at {..} mm needs {..} points per layer…; increase the stepover or the largest angle step | `STEPOVER` |
| roughing.rs:1434 | `Parameter{INDEX_ANGLE}` | {..} positions of {..} lines of {..} points is too many; use a larger stepover or a larger index angle | `INDEX_ANGLE` |
| roughing.rs:1478 | `Parameter{STEPOVER}` | {..} lines of {..} points around a stock of {..} mm radius is too many; use a larger stepover | `STEPOVER` |
| roughing.rs:1517 | `Parameter{INDEX_ANGLE}` | same text as :1434 | `INDEX_ANGLE` |
| roughing.rs:1576 | `Parameter{STEPOVER}` | {..} faces of {..} lines of {..} points is too many; use a larger stepover | `STEPOVER` |
| roughing.rs:1754 | Strategy | the selection is nowhere under the tool, so there is no floor to rough down to / … | – |
| roughing.rs:1769 (`too_many_rings`) | `Parameter{STEPOVER}` | {..} mm of length in rings at {..} mm needs {..} points per layer; increase the stepover or the largest angle step | `STEPOVER` |
| simultaneous.rs:558 | Strategy | the part has no length along the rotary axis to finish | – |
| simultaneous.rs:566 | Strategy | the part has no extent round the rotary axis to finish | – |
| simultaneous.rs:596 | parameter(`LEAD_ANGLE`) | axial passes travel along the rotary axis, where A cannot lean the tool; set the lead angle to 0 or use the spiral pattern | `LEAD_ANGLE` |
| simultaneous.rs:1726 (`too_many`) | parameter(`STEPOVER`) | {..} mm over {..} mm of part needs {..} surface samples; increase the stepover | `STEPOVER` |
| simultaneous.rs:2362 | Strategy | simultaneous finishing cannot keep the move to X {..}, A {..} within the tolerance… | – |
| solid.rs:75 | Surface | the {name} {value} must be a finite number above zero | – |
| solid.rs:98 | Surface | the solid's base point must be finite | – |
| solid.rs:116 | Surface | the solid's axis is degenerate | – |
| solid.rs:136 | Surface | the box's corners must be finite | – |
| stock.rs:317 | Strategy | bounding-box stock needs the part, and the setup has none | – |
| stock.rs:327 | Strategy | imported stock is not supported yet (SET-04) | – |
| strategy.rs:272 | parameter(name) | this strategy has no parameter of that name | – |
| strategy.rs:362 | Strategy | the strategy '{..}' cannot be limited to a region | – |
| strategy.rs:409 | Strategy | the strategy '{..}' cannot leave different stock on different selections | – |
| strategy.rs:499 | Strategy | a stock to leave of {..} mm is not zero or more | – |
| strategy.rs:511 | Strategy | {label} takes no options on its boundary set… | – |
| strategy.rs:763 | Strategy | every cut of this operation would reach over {names}, where the workholding grips the stock…; make the held region shorter or choose a selection away from it | – |
| strategy.rs:806 | Strategy | a move at X {..} mm would reach over {names}…; change the entry or the link settings | – |
| strategy.rs:871 | Strategy | a strategy needs a name | – |
| strategy.rs:874 | Strategy | a strategy named "{..}" is already registered | – |
| strategy.rs:888 | UnknownStrategy | – | – |
| surface.rs:270 | Surface | the mesh has no triangles | – |
| surface.rs:275 | Surface | triangle {i} refers to a missing vertex | – |
| surface.rs:285 | Surface | cannot index the mesh: {e} | – |
| swarf.rs:448 | Strategy | the tool has no straight flank to cut with… | – |
| swarf.rs:1228 | Strategy | a ruling at parameter {..} runs along its own normal | – |
| swarf.rs:1237 | Strategy | its rulings run along the rotary axis… | – |
| swarf.rs:1375 | Strategy | swarf on face {..} needs {..} mm of flank… | – |
| swarf.rs:1985 | Strategy, `fail` closure | swarf cannot cut face {n}: {what} | – |
| swarf.rs:2984 | Strategy | swarf cuts the selected faces, not a region… | – |
| swarf.rs:2992 | Strategy | swarf needs the exact faces of a STEP model… | – |
| unroll.rs (prefix "region: " from the variant) | Region | 19 sites at :115 :182 :222 :265 :272 :277 :284 :308 :313 :323 :327 :514 :581 :618 :682 :725 :732 :774 :815, e.g. "{loop} crosses itself", "the region has no outer boundary", "the islands grown by {..} mm join into a ring…" | – |
| wrap_fit.rs:602 | Strategy | the region needs {..} points at a spacing of {..} mm; increase the tolerance | `TOLERANCE` |

---

### B. #656 (`origin/claude/lane-pathfeed-641-wip`): differences from main

Path changes on #656 are only in `facing/end.rs`, `facing/end/routing.rs`, `facing/validation/end_layers.rs` and `facing.rs`, plus `plunge_fed` plumbing in other files.

**Setting-key swap.** `EndFacingParameters::read` now takes `axial_stepdown` from `names::STEPOVER` and `flute_step` from `roughing::STEPDOWN` (end.rs ~:59–64). The schema labels in `facing.rs` (~:218) become "Stepover (slice thickness along X)" and "Stepdown (band along the tool)".

| Site on #656 | Text | Key | Status |
|---|---|---|---|
| facing/end.rs:143 | End facing: the Stepover (slice thickness along X) must be above zero and no greater than the cutting diameter | `names::STEPOVER` | reworded, only on #656 |
| facing/end.rs:151 | End facing: the Stepdown (band along the tool) must be above zero and no greater than the flute length | `roughing::STEPDOWN` | reworded, only on #656 |
| other end.rs refuses | same texts as main | – | shifted to :56, :129, :135, :159, :168, :171, :179, :182, :203, :212, :224, :257, :278, :317, :440, :461, :486 |
| facing/end/routing.rs:246 | End facing: end flute-band budget exceeded | – | new site, only on #656 (main's equivalent is :265 on #656) |
| routing.rs | `too_deep` | – | moved to :325/:332, called from :208 |
| facing/validation/end_layers.rs:132 | Facing final validation: an actual End cut is deeper along X than the Stepover (slice thickness along X) | `names::STEPOVER` | reworded, only on #656 |
| facing/validation/end_layers.rs:176 | Facing final validation: an End band cuts {..} mm deep along the tool at A {..}, more than the Stepdown (band along the tool) of {..} mm, so the flutes would be overloaded. Lower the Stepdown, or face the stock above the requested area first | `roughing::STEPDOWN` | new refusal, only on #656 |
| end_layers.rs :95 :98 :114 :124 :190 :203 :218 | same texts as main | – | shifted |

- **Old wording kept on #656** (it predates main's rewording): end_layers :52 "…independently verified removal slice"; validation.rs:85 "…not certified"; validation.rs:140 "…no verified removal evidence"; legacy.rs "…not certified" and "…no verified full-circumference finish".
- **App crate:** no refusal changes on #656. The only `src` change is a test field in `pipeline.rs`.

---

### C. #663 (`origin/claude/lane-rampentry-pr1`): differences from main

**Removed (exist on main only):**
- `assemble/ramp_refusal.rs` (whole file, all 10 builders in A8) and `assemble/ramp.rs`
- `entry_safety.rs` and `entry_safety/tests.rs` (including the :68 refusal)
- `assemble/entry.rs` main :293–437 ramp_refusal calls, `ramp_refused` (:646), and the :506 "ramp starts {..} mm back…" refusal
- `crates/openchuck-app/src/ramp_entry.rs` (its two refusal texts and its call at operation.rs main :461)
- the entry_safety plumbing in `operation.rs` (main :718–760, including `missing(.., "the whole target has no bounds")` at main :745)

**Added in the path library (only on #663; all `PathError::Strategy`, all with a prefix):**

| Site | Text (abbreviated) | Keys named |
|---|---|---|
| assemble/entry.rs:372 | Ramp entry: pass {..} is a single point, so there is no path for the ramp to follow. Turn on Allow single-point plunge, or set Entry to Plunge or Helix. | `names::ALLOW_SINGLE_POINT_PLUNGE` (new), `names::ENTRY` |
| entry.rs:436, `path_ramp::Refusal::TooShort` arm | Ramp entry: pass {..} starts with a {line/stretch} of {..} mm, shorter than the {..} mm (the tool radius) a ramp needs… Set Entry to Plunge or Helix. | `ENTRY` |
| entry.rs:~443, `TooManyLegs` arm | Ramp entry: pass {..} would need {legs} legs to ramp down its {..} of {..} mm, more than the {MAX_LEGS} allowed. Raise Ramp angle, or set Entry to Plunge or Helix. | `RAMP_ANGLE`, `ENTRY` |
| entry.rs:492 | Ramp entry: pass {..} ramps over its own path, yet the tool would cut {..} mm into the finished surface at X {..} mm. This should not happen: please report it as a defect, and set Entry to Plunge or Helix meanwhile. | `ENTRY` |
| entry.rs:631 | Ramp entry: pass {..} starts on the rotary axis, where the tool descends while the part turns; {angle_words}, measured at the tool's radius of {..} mm, that takes {..} turns, more than the {MAX_TURNS} allowed. {fix_words} | see note below |
| entry.rs:685 | Ramp entry: pass {..} turns round the part with its tip {..} mm from the rotary axis, so {angle_words} a ramp round it would take {..} turns, more than the {..} allowed. {fix_words} | see note below |
| entry.rs:886 (`in_a_range`) | Ramp entry: pass {..} needs A from {..} degrees to ramp round the part, outside the machine's A range of {..} to {..} degrees. Widen A range in the setup's Machine settings, or set Entry to Plunge or Helix. | machine A range, `ENTRY` |
| entry.rs:817 | Face nubs: the nub at X {..} mm needs a whole turn of A from {..} degrees, outside the machine's A range of {..} to {..} degrees. Widen A range in the setup's Machine settings, or turn off Plunge to remove face nubs. | machine A range, `names::PLUNGE_FACE_NUBS` (new) |
| assemble.rs:1128 | Entry: the move to an entry's start would pass through stock that reaches {..} mm from the rotary axis, at or above the retract height of {..} mm. Raise the setup's clearance, or set Entry to Plunge. | setup clearance, `ENTRY` |

Note on entry.rs:631 and :685: `fix_words` is either "Raise Maximum ramp angle, or set Entry to Plunge or Helix." or "Raise Ramp angle, or turn on Steeper ramp near the axis, or set Entry to Plunge or Helix." So the keys named are `names::RAMP_ANGLE_MAX`, `RAMP_ANGLE`, `RAMP_STEEPER_NEAR_AXIS` and `ENTRY`.

- **Kept on #663 with no prefix:** entry.rs `refuse()` :1043 (texts at :1054 and :1062), called from :201 and :294 (lead arcs) and :564 (helix); and the helix segment refusal at :545.
- **`path_ramp.rs` and `ring_ramp.rs`** only define `Refusal` enums: `TooShort{length}`, `TooManyLegs{legs}`, `NothingToDescend`, `TooManyTurns{radius,turns}`. They contain no user text.
- **`params.rs`** adds the keys `allow_single_point_plunge`, `ramp_steeper_near_axis`, `ramp_angle_max`, `plunge_face_nubs` and `face_nub_diameter`, with no new refusals. `RAMP_BACK_AND_FORTH` is kept but now has no effect.

---

### D. App crate (`crates/openchuck-app/src`)

**`AppError` variants.** On all refs: `Operation{name,reason}`, which displays as "operation '{name}' cannot be generated: {reason}". On #663 only, `error.rs` adds `Refused{topic,name,text}`, which displays as "{topic} (operation '{name}'): {text}".

**The `missing()` helper:** `operation.rs:37` `fn missing(operation, what) -> AppError::Operation{name, reason: what}`. Line numbers below are main's. #656 is the same up to ~:943; after that #656 is −5. On #663 the lines are main +24 before :461 and −~10 after :745 (see the #663 column).

| Site (main) | #663 | Text |
|---|---|---|
| :184 (direct `AppError::Operation`) | :184 | {keys} {is/are} required: set {it/them} on the operation, or fix the cause: {why} |
| :447 `missing` | :471 | `brep_only()` (:1269): "{label} is B-rep only ({why}): the setup's part '{part}' is a mesh; add the part as a STEP file" |
| :456 | :480 | `step_sets::refusals` text (step_sets.rs :95–137), 6 forms, e.g. "The set '{n}' selects triangles, but this setup's part is now a STEP file…", "{label} does not use a boundary, avoid or check set. Take the set '{n}' out…", "{label} works on the whole part and cannot be limited to chosen faces…" |
| :461 | **removed on #663** | `ramp_entry::refusal` (main only): "The back-and-forth ramp is not available on a STEP part yet: … To generate this operation, in the operation's Entry, links and retract settings, turn off Back-and-forth ramp or set Entry to Plunge, Lead or Helix." / "The back-and-forth ramp cannot check its clearance with the tool '{..}': its shank ({..} mm) is wider than its cutting diameter… Choose a tool whose shank is no wider than its cutter, or {ENTRY_FIX}." |
| :473 | :492 | {label} works on the whole part and cannot be limited to chosen triangles. Take the selection sets out of the operation. |
| :484 | :503 | the project has no machine profile |
| :487 | :506 | its setup is missing |
| :490 | :509 | it has no tool |
| :495 | :514 | its tool is missing |
| :498 | :517 | its setup has no part |
| :503 | :522 | its setup's part is missing |
| :507 | :526 | its setup has no stock |
| :517 | :536 | keeping tangent edges is a choice for the B-rep edges of a STEP part; its part is a mesh… |
| :570, :585 | :589, :604 | its setup's part has no extent |
| :598, :608, :943 | :617, :627, :954 | {workholding error}.to_string() |
| :663 | :682 | the whole facing part has no extent |
| :666 | :685 | the facing stock has no established bounds |
| :745 | removed | the whole target has no bounds |
| :796 | :778 | the strategy '{label}' cannot leave different stock on different selections; clear the stock to leave of the selection sets |
| :1347 | :1358 | its face set has faces swarf cannot cut; take them out of the set ({..}) |
| :1393 | :1404 | extended past the stock, its path goes {..} mm beyond the machine's travel at the {end} of the rotary axis; shorten the distance past the stock or extend past the other end only |
| :1619 `fail`/`bad` (brep_selection, :1666–1824) | +11 | ~16 texts. `bad` prefixes "selection set '{n}' cannot be used: ". Texts include: "it was made on a different model…", "…different version of '{..}'…", "it selects no faces; select the ruled walls…", "it selects no edges; …", "face {..} is not in the model", "edge {..} is not in the model", "it selects triangles, and a STEP model is selected by face and edge", "it selects edges of a mesh's triangles…", `SWARF_NEEDS_FACES` (:1286), `FACE_FLOW_NEEDS_FACES` (:1291), "face {..} is in the face set it cuts and in the {role} set '{..}'; take it out of one of them", "it selects {kind}, and a boundary, avoid or check set of a STEP model selects faces" |
| `named()` :360 | :369 | wraps every `PathError` as `AppError::Operation`; see #663 notes below |
| — | :920, only on #663 | `AppError::Refused{topic: topic::RAMP_ENTRY or "Lead entry"}`: "a pass is entered by a straight plunge (a single point, or a ring on the rotary axis, where the tool descends while the part turns), and the tool '{..}' cannot cut straight down (it is not centre cutting). Set Entry to Helix, or choose a tool that can plunge" |

Other app sites building `AppError::Operation` (unchanged across refs apart from line shifts):

| Site | Text |
|---|---|
| lib.rs:1177 (#656 :1153, #663 :1176) | All four requires Box sides facing |
| form.rs:421 (#663 :442) | its strategy has no parameter '{name}' |
| opflow.rs:161 | an operation needs a name |
| opflow.rs:166 `fail` | there is no strategy '{..}' (available: ..) (:176) |
| | its setup is not in the project (:182) |
| | {unavailable_on_setup why} (:191) |
| | its tool is not in the project (:196) |
| | its selection set is not in the project (:206) |
| | its selection set '{..}' was made on a different model… (:213) |
| | the strategy '{..}' ignores selections, so it cannot take one (:223) |
| pipeline.rs:54 `fail` | its setup is missing (:60) / its setup has no stock (:64) / the project has no machine profile (:67) |
| pipeline.rs:270 | it is turned off, so it is not generated; turn it on first |
| connections.rs:72 | wraps a `connect_group` `PathError`, i.e. the program.rs "operation '{op}': cannot connect safely: …" texts. These are therefore shown as "operation 'X' cannot be generated: operation 'X': cannot connect safely: …" |
| preset.rs:261 | `chosen_material` reason |
| selection.rs:886 (`mesh_edge_segments`) | `fail` prefix "selection set '{n}' cannot be used: ", 9 texts (:905–1004): "it selects edges, which can only be followed, not used as a boundary…", "it selects edges, and the strategy '{..}' does not use selected edges…", different model / different version, "it selects no edges", "a stock to leave per selection set is not used on followed edges (OP-11)…", "triangle {..} does not exist…", "it selects triangles, and edge following follows…", "edge following on chosen edges cannot yet honour a boundary, avoid or check set…" |
| selection.rs:1272 (`resolve region`) | same prefix, 6 texts (:1287–1332): no faces / different model / version / "face {..} is not in the model" / "it selects no {kind}" / "triangle {..} does not exist…" |

`AppError::Post` refusals (not `Operation`) in `facing/written.rs:19`: `refuse()` gives "Facing written-motion verification: {reason}". It has 15 call sites (:52–358), e.g. "program exceeds the verification work limit", "unreadable program", "line {..}: G{..} has no supported complete-motion check result", "the Z1 home position (machine XYZ0) lies outside configured travel", "work/machine coordinates exceed the supported float precision; change the work offset or post". It is called from `app/facing.rs` :440 :531 :563 :575 :588. `facing/display.rs:276` and :287 also have `AppError::Post` texts: "Facing part has no extent" and "Facing stock has no bounds". These are the same on all refs.

---

### E. #663: `REFUSAL_TOPICS`, `named()` and `notice.rs` topics

**`REFUSAL_TOPICS`** (`operation.rs:357`, only on #663):
```rust
const REFUSAL_TOPICS: [&str; 4] = [
    openchuck_path::topic::RAMP_ENTRY,   // "Ramp entry"
    "Lead entry",                        // string literal, no const; no path text leads with "Lead entry: "
    openchuck_path::topic::ENTRY,        // "Entry"
    openchuck_path::topic::FACE_NUBS,    // "Face nubs"
];
```

**`named()`** (`operation.rs:369`, #663):
- `Cancelled` passes through.
- Any other `AppError::Path` becomes `reason = err.to_string()`. The first topic `t` where `reason.strip_prefix("{t}: ")` succeeds gives `AppError::Refused{topic: t, name, text: rest}`. Otherwise the result is `AppError::Operation{name, reason}`.
- Non-path errors are unchanged.
- So on #663 only the "Ramp entry: ", "Entry: " and "Face nubs: " path texts reach the one form. "End facing: ", "Facing …: ", "Box facing: " and the rest still go to `Operation`.
- On main and #656, `named()` (:360) has no topic match.

**`notice.rs` `pub mod topic`.** Main has 32 consts:

| Const | String |
|---|---|
| PARTIAL_PATH | "Partial path" |
| AVOID_SET | "Avoid set" |
| CHECK_SET | "Check set" |
| SELECTION_SET | "Selection set" |
| MATERIAL_LEFT | "Material left" |
| UNREACHED_AREAS | "Unreached areas" |
| FLUTE_LENGTH | "Flute length" |
| TOOL_HOLDER | "Tool holder" |
| SHANK_CLEARANCE | "Shank clearance" |
| HELD_REGION | "Held region" |
| OVERCUT | "Overcut" |
| INSIDE_CORNERS | "Inside corners" |
| FREE_ENDS | "Free ends" |
| PART_CUT_FREE | "Part cut free" |
| FACES_ACROSS_THE_AXIS | "Faces across the axis" |
| HOLES_LEFT_OUT | "Holes left out" |
| CHAINS_LEFT_OUT | "Chains left out" |
| FEED_LIMIT | "Feed limit" |
| ROTARY_AXIS | "Rotary axis" |
| LINKS | "Links" |
| ADAPTIVE_SPACING | "Adaptive spacing" |
| SHOULDERS | "Shoulders" |
| PASS_ORDER | "Pass order" |
| RINGS | "Rings" |
| FINISHING_LAYERS | "Finishing layers" |
| PAST_THE_ENDS | "Past the ends" |
| TOOL_TILT | "Tool tilt" |
| TESSELLATION | "Tessellation" |
| OFF_AXIS_SECTION | "Off-axis section" |
| HIDDEN_SURFACES | "Hidden surfaces" |
| REST_MACHINING | "Rest machining" |
| EDGE_FOLLOWING | "Edge following" |
| FACE_FLOW | "Face-flow finishing" |
| PENCIL | "Pencil clean-up" |
| SWARF | "Swarf" |

Topics added by the branches:
- **#656 adds:** `STOCK_ABOVE_THE_AREA = "Stock above the area"`. It is used by an information notice in `facing.rs` `above_area_note`, not by a refusal.
- **#663 adds:** `RAMP_ENTRY = "Ramp entry"`, `ENTRY = "Entry"`, `FACE_NUBS = "Face nubs"`.
- **No const exists** on any ref for "End facing", "Box facing", "Facing completion", "Facing motion", "Facing final validation", "Facing extent" or "Lead entry".

---

### F. Distinct prefixes/topics seen on refusals

- **Text prefixes ("X: " inside the message):**
  - "End facing" (end.rs and routing.rs; all refs)
  - "Facing completion"
  - "Facing motion"
  - "Box facing"
  - "Facing final validation"
  - "Facing extent"
  - "operation '{op}': cannot connect safely" (program.rs)
  - "Ramp entry" (#663 only, 7 sites)
  - "Entry" (#663 only, assemble.rs:1128)
  - "Face nubs" (#663 only, entry.rs:817)
  - "Facing written-motion verification" (app, `AppError::Post`)
- **Prefixes that come from the variant's Display:** "parameter {name}: ", "surface: ", "region: ", "A axis: " (RotaryLimit).
- **App-side prefixes:**
  - "operation '{name}' cannot be generated: " (`AppError::Operation`)
  - "{topic} (operation '{name}'): " (`AppError::Refused`, #663 only; topics: "Ramp entry", "Lead entry", "Entry", "Face nubs")
  - "selection set '{n}' cannot be used: " (inside reasons)
- **No prefix:** all remaining `PathError::Strategy` texts. This includes main's back-and-forth ramp refusals ("The back-and-forth ramp …", "Pass {n}: the back-and-forth ramp …", "Back-and-forth ramp needs …"), the lead/helix "pass {n}: the {name} needs …" texts, and "Cannot face an unheld end: both ends are held." (a sentence, not a topic).
