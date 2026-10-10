# SP-3 result: wrapped engraving path to G-code with A words

Branch `spike/sp-3-wrapped-path-gcode`. Throwaway code in `spikes/sp3/`; not for merging into the product.
Requirements touched: OP-05, OP-06, OP-13, OP-15, GC-01, GC-02, GC-07, GC-08, GC-09, GC-10, NFR-05.

## Question
Does a wrapped engraving path on a cylinder become correct A-word G-code that replays in the
LinuxCNC simulator?

## Method
Fixed path on a 30 mm diameter cylinder (axis = model X): a sine wave (amplitude 10 mm, 3 periods)
wrapping 540 degrees (1.5 turns), 0.2 mm deep, contact feed 300 mm/min. Chain: path (unrolled
x, theta) -> IR (model frame) -> shared machine model -> hard-coded LinuxCNC-style post -> text ->
`gcode` 0.7.0 reader -> replay through a separately written oracle. Three postings are compared:
G94 compensated, G93 inverse-time, and a naive G94 baseline that sends the raw contact feed.
The machine is deliberately off-origin (rotary axis at Y=2, Z=-30) so offset errors show.

Run: `cd spikes/sp3 && cargo test && cargo run`. Output goes to `out/`.

## Numbers
| Check | Result | Limit |
| --- | --- | --- |
| Shared model vs independent oracle, 17 A angles incl. negative, +-360, seam, 4 points | agree to 1e-9 mm | 1e-6 mm, 1e-6 deg (verification plan 4.1) |
| Flip A sign in shared model / in post | oracle reports error > 1 mm | must fail |
| Closed-form top-contact XYZ and A vs planner, 308 cut nodes | agree to 1e-9 | |
| Replayed node positions vs IR (via oracle), 4-decimal output | 5.1e-5 mm max | rounding only |
| Same, if numbers are read through the `gcode` crate's own f32 values | 5.2e-5 mm max | see finding 3 |
| Helix between nodes vs intended curve (chord tol 0.005) | 0.0056 mm max | 0.01 mm |
| Tool axis in machine frame, every segment | exactly +Z | |
| Cut time, replay vs IR, G94 compensated and G93 | +-0.00 % | |
| Cut time, naive raw-feed baseline | -36.9 % (cuts 1.6x too fast) | shows compensation is required |
| Contact-point speed from replayed model-frame chords | within 1 % of 300 mm/min | |
| A through the seam | continuous, max step 8.4 deg, spans 540 deg | |
| Unwind | rapid A -450 -> -90, same machine position, model tip unchanged | |
| Output identical on repeat | yes | GC-07 |

13 tests pass. 312 moves, 308 cutting.

## Not done: LinuxCNC simulator
LinuxCNC is not in the Ubuntu 24.04 archive and linuxcnc.org / Debian / GitHub were blocked by the
session's network policy. `run_linuxcnc.sh` is ready but **was not run**. The pass criterion "replays
in the LinuxCNC simulator" is therefore **not met yet**. Gate 3 needs a run on a machine with LinuxCNC.

## Findings that change the design
1. **Replay in LinuxCNC will not check the A direction.** With trivkins the A joint is just a number.
   The simulator confirms dialect, reached XYZA values and timing, not that +A turns the right way on
   the Z1. The independent oracle (and later the air cut, SP-12) are the only direction checks.
2. **G94 feed on mixed moves is fragile.** The compensated F is `XYZ length / duration` when XYZ moves,
   else degrees/min. This relies on a LinuxCNC rule I could not confirm here (feed applies to the XYZ
   path when any of XYZ moves). Near the sine peaks the axial step goes to zero, so F falls to 2.4 mm/min
   while the A axis does about 1160 deg/min; the F word is meaningless as a speed there and a controller
   with a different rule (or an A velocity cap) would cut at the wrong rate. G93 avoids this (F range
   19 to 1101 per minute, always the reciprocal of the move time). Recommendation: IR keeps contact
   feed plus duration (as the architecture says); the post must decide per move, and the Z1 post needs
   SP-12's answer on G93 and on mixed-move feed behaviour before any golden file with F words is trusted.
3. **`gcode` 0.7.0 stores numbers as f32** (`Value::Literal(f32)`). Positions here are under 100 mm so the
   f32 error was hidden by 4-decimal rounding (5.2e-5 vs 5.1e-5 mm), but it would break the 1e-6 limit
   at larger coordinates or with more decimals, and `A-450.0000` style values are only exact by luck.
   I read structure and spans from the crate and re-parsed each number from its span as f64. It
   parsed every code the post emits (G0/G1/G17/G21/G54/G90/G93/G94/M3/M5/M6/M30/T1), comments, and
   modal lines with no G word. SP-5 verdict: usable as a tokenizer, not for values; plan the thin f64
   layer (or a small in-house reader) at slice 6.
4. **IR fields that worked:** tip and tool axis in the *model* frame, an explicit *unwrapped* A, move kind,
   contact feed, duration and contact length. Machine XYZ was derived entirely in the post from the
   machine profile (axis offset and A sign). The unwind needs no special IR field: it is a rapid to the
   same model tip with A changed by whole turns. Suggest also recording the contact length (kept it
   here) so the post can compute feeds without redoing geometry.
5. **Machine profile fields that mattered:** A sign, rotary axis Y/Z offset. Both must be profile data (SET-01).
6. Chord tolerance must be set on the *unrolled* curve; half the toolpath tolerance (0.005) gave 0.0056 mm
   measured on the real helix motion, so a 0.5x factor is about right but is tight; use 0.4x or measure.

## Decision / fallback status
Pass criteria met except the LinuxCNC run. The IR fields stand as above. No fallback is triggered; the
simulator run is outstanding and should be done (by you or in an environment with LinuxCNC) before slice 6
accepts the first golden file. Keep `out/*.ngc` as fixtures only after that run and after the SP-12 sign check;
until then they must not be treated as golden files.

## NFR-14 record
- `gcode` 0.7.0 (pinned `=0.7.0`): crate-selection score 75, MIT or Apache-2.0, AGPL-compatible. Parsing only.
  Used as tokenizer; f32 limitation above.
- No other dependency.
