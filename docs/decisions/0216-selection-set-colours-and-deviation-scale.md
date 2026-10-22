# 0216: Selection sets and the deviation scale get separate colours; sets get an outline; the deviation steps are spread apart (lane SELSET; issues 256, 258)

- **Status:** accepted by the project owner (22 Oct 2026, directly in the lane SELSET session). The colours were approved earlier as shown (option A, relayed by the coordinator session), which closed OI-871; item 8, the hidden band, is the owner's option A for OI-882 (relayed).
- **What it amends:** the interface specification ([decision 0038](0038-ui-specification-and-requirements.md)), section 7.4 (saved set), section 11.5 (the deviation colours) and section 14.1 (the `overcut` text token). It also amends [decision 0046](0046-slice-10-selection-ui-to-specification.md) item 2 (the eight swatches) and the deviation colours that [decision 0119](0119-slice-22-viewport-drawing.md) built. The band thresholds and their meaning are unchanged (decision 0040 item 10; OI-194 stays open). SIM-05, UI-05, UI-11 and NFR-15 are unchanged.
- **Date:** 22 Oct 2026.
- **Open items:** OI-871 (closed: the owner approved, 22 Oct 2026) and OI-873 are new.

## Context

- **Issue 256 (F-85).** The owner says the clash is between the blue and maroon of the selection sets and the deviation colours. The sets' first two default swatches are blue `#2d6cb5` and maroon `#a04a46`. The deviation scale uses three blues for material left and dark magenta `#a3157f` for overcut. The 40% fill of swatch 1 over the part (`#809ec2`) is only 5.6 apart (CIEDE2000) from the middle blue `#7ca7e8`.
- **Issue 258 (F-87).** The levels of material left are hard to tell apart on every screen, because the blues are very close. Measured, the neighbouring steps are 13 and 15 apart in normal vision, and 13 for tritanopia.
- **Found while measuring (a safety point, NFR-12).** For protanopia, overcut `#a3157f` and the deepest blue `#3f78d1` are only 5.7 apart: both look dark blue, so an overcut can be read as a lot of material left.
- **Constraint.** Section 16 asks graphics that carry meaning to have 3:1 contrast with their background. The view's background is `#2c3139` at the top, in both themes (14.2). So the darkest band cannot be much darker than `#4e71ea`, and the steps have to be spread with hue and chroma as well as lightness.
- **Lane SIMPOLISH** is building band visibility (SIM-15, [decision 0163](0163-band-visibility-requirement-proposal.md)) at the same time. This record changes only the colour each band is drawn in (`roles::band_colour`), not the bands, their thresholds or the histogram's bins, so the two fit together.

The board shows the scale and the swatches now and as proposed. Each is shown as seen and as simulated for protanopia, deuteranopia and tritanopia (Machado, Oliveira and Fernandes 2009, full severity), with the measured differences, and in a mock of the view at 100% and 120% scale.

## Decision (proposed)

1. **The deviation scale.** Same six bands, same thresholds, same meaning, new colours:

   | Band | Now | Proposed | What moves |
   | --- | --- | --- | --- |
   | Overcut (deeper than one cell) | Dark magenta `#a3157f` | Vermilion `#df5230` | Hue from magenta to red-orange; lightness L\* 38 to 54. It now sits on the warm side of the scale, opposite the blues, for every colour vision type |
   | Within tolerance | Grey `#9aa0a8` | Grey `#9aa0a8` | Unchanged |
   | Material left, up to 0.25 mm | Light blue `#b9d6ff` | Ice `#c6f9ff` | Lighter (L\* 85 to 95), towards cyan |
   | Material left, 0.25 to 1 mm | Blue `#7ca7e8` | Sky blue `#4bb7fc` | More saturated (chroma 37 to 44); L\* 68 to 71 |
   | Material left, more than 1 mm or a bound | Blue `#3f78d1` | Strong blue `#4e71ea` | Towards violet-blue and more saturated; L\* 51 (unchanged, at the 3:1 limit) |
   | Not measured | Grey, hatched | Grey, hatched | Unchanged |

   Material left still gets darker as more is left, so the scale reads by lightness alone. The neighbouring steps of material left are 24 and 20 apart in L\* (17 and 17 now). Each neighbouring pair is at least 20 apart in CIEDE2000 in normal vision (13 now) and at least 16 for each simulated colour vision type (13 now). Overcut is at least 29 from every other band for every type (5.7 now, against the deepest blue). Every band has at least 3:1 contrast with the view's background.
2. **The `overcut` text token** (14.1; overcut values in panels) follows the hue: dark `#ff8a66` (6.6:1 on `panel`) and light `#a8321a` (6.3:1 on `panel`), in place of `#e27ad2` and `#9c1f86`.
3. **The selection set swatches.** There are still eight, a set still takes the one for its place in the list, and a chosen colour is still stored (decision 0046 item 2):

   | Place | Now | Proposed |
   | --- | --- | --- |
   | 1 | Blue `#2d6cb5` | Teal `#239688` |
   | 2 | Maroon `#a04a46` | Plum `#8d348c` |
   | 3 | Violet `#7d5bb5` | Lime `#b1d364` |
   | 4 | Teal `#3f9c8a` | Forest green `#27653e` |
   | 5 | Ochre `#b58a2d` | Orchid `#cb94c7` |
   | 6 | Green `#6f9a3a` | Olive `#665a00` |
   | 7 | Pink `#b5568f` | Mint `#1ce0ac` |
   | 8 | Grey `#7a8591` | Sage `#7c8d51` |

   None is blue, red, orange or grey. Each is at least 20 (CIEDE2000) from every deviation colour and from every other swatch. Each is at least 14 from the working blue, the hover yellow, the edge hover magenta, the selected edge orange, the avoid red, the held region violet, the tab sand, the cut amber and the part grey, which keep their colours. With colour vision differences, no hue can be kept apart from both ends of the scale at once (only lightness and the blue-yellow axis remain). So for those users the outline of item 4 is what tells a set from deviation, as NFR-15 asks.
4. **A set is drawn with an outline** as well as its fill. Wherever a saved set is drawn on the part (on Operations and on Simulate), it gets a 40% fill and a framed outline round the edge of the set: a 2-pixel line in the set's colour inside a 4-pixel dark `#15171a` line. The outline is 2.4 and 4.8 pixels at 120%, measured on screen like the other widths (7.5). The outline is drawn round the set's boundary: the triangle sides or face edges that the set shares with what is not in it. A set of edges is already a 3-pixel line and keeps that line, in its colour. The deviation scale never draws outlines, so a set and a band cannot be mixed up without colour. The cross-hatch of held regions, the single hatch of avoid surfaces and the dashed grey ghost of a stale set (decision 0125) stay theirs alone.
5. **Legends.** Wherever both are on screen, the swatch of a shown set in the legend is drawn the way the set is drawn (the fill with the outline), next to the deviation colours: in the colour scale box on Simulate, and in the path legend on Operations, where "Selected" already is.
6. **Projects that stored an old swatch.** A set whose stored colour is one of the old eight is drawn in the new swatch at the same place in the list (old 1 as new 1, and so on). This is a display mapping only: the project file keeps what it holds until the user chooses a colour again, so nothing stored or generated changes (GC-07, NFR-06). There is no schema number or migration. Only the eight swatches can be chosen (OI-199), so every stored colour is one of the old or new eight.
8. **A hidden band** (SIM-15, lane SIMPOLISH; OI-882). A deviation band the user hides is drawn in a solid dark slate `#5d6570` of its own, not in the part's grey, which was only 8.8 from the "within tolerance" grey and could read as "on the surface". It is at least 18.8 (CIEDE2000) from every band for every simulated colour vision type, 22 from "within tolerance" and 30 from the part's grey; solid, so it reads apart from "not measured", which is hatched. The owner chose this (option A, 22 Oct 2026, relayed by the coordinator), which closes OI-882. Test: `req_sim_15_req_nfr_15_a_hidden_band_reads_apart_from_every_band_and_the_part`; the view draws the triangles of hidden bands in it once #280 is merged.
7. **Checks** (tests named for NFR-15, SIM-05 and UI-05, in `roles.rs` and `selection_panel.rs`, with the colour difference and simulation written as test helpers):
   - neighbouring deviation bands at least 20 apart (CIEDE2000) in normal vision and at least 16 under each simulated colour vision type;
   - the bands of material left darkening by at least 15 in L\* per step (24 and 20 as proposed);
   - overcut at least 25 from every other band for every type;
   - every band at least 3:1 against the view's background;
   - every swatch at least 20 from every band and from every other swatch, and at least 14 from the other roles;
   - a shown set carrying the framed outline, and no deviation band carrying one;
   - the old-to-new swatch mapping.

   These replace the existing `req_ui_05_the_swatches_are_eight_and_none_is_a_highlight_colour` and the spot checks of `req_nfr_15_role_colours_are_the_specified_hex_values` for the changed values.

## Specification entries proposed

Made in the same change, after the owner approved the colours (OI-871, 22 Oct 2026).

- **7.4, Saved set:** "Its own swatch from a fixed set of eight, none blue, red, orange or grey (decision 0216) | Fill at 40% when shown, with a framed outline round its edge: 2 pixels in its colour inside 4 pixels of `#15171a`".
- **11.5, the stock's colours:** the table becomes the six bands as built (decision 0040 item 10, until OI-194): overcut, vermilion `#df5230`; within tolerance, grey `#9aa0a8`; material left up to 0.25 mm, ice `#c6f9ff`; 0.25 to 1 mm, sky blue `#4bb7fc`; more than 1 mm or a bound, strong blue `#4e71ea`; not measured, grey hatched. Add: "Neighbouring bands differ by at least 20 (CIEDE2000), and by at least 16 for the common colour vision differences; each has 3:1 contrast with the view's background (decision 0216)."
- **14.1, `overcut` (text):** dark `#ff8a66`, light `#a8321a`.

## Alternatives considered

- **Option A, change only the set colours** and **option B, change only the scale.** Not chosen by the owner. Neither fixes both issues, and neither fixes the protanopia merge of overcut and the deepest blue.
- **Keep overcut magenta and only spread the blues.** Rejected: for protanopia and deuteranopia magenta reads as blue, which is the safety point above.
- **Darker blues for more lightness range.** Rejected: below about L\* 50 a band falls under 3:1 against the view's background (section 16), and the part's shading darkens it further.
- **A hatch for sets in place of the outline.** Rejected: the single hatch already means avoid and check, the cross-hatch means held regions, and the grey hatch means a stale set. A hatch also hides the deviation colours under a set on Simulate, where an outline does not.
- **Six swatches instead of eight.** Six can be spread further apart (29 at the closest, against 20). Not proposed, to keep decision 0046's count; the owner may prefer it.
- **Rewriting stored colours in the project (a migration).** Rejected: it would change files that the display mapping leaves alone, and it would need a schema claim.

## As built

- `roles.rs`: the deviation constants; `SET_OUTLINE_WIDTH`, `SET_FRAME` and `SET_FRAME_WIDTH`; the test helpers in `roles::tests::measure` (CIEDE2000, the Machado simulation, WCAG contrast). `theme.rs`: the `overcut` token.
- `selection_panel.rs`: `SWATCHES` and `OLD_SWATCHES`; `set_rgb` maps an old stored swatch to the new one at its place; a shown set's surfaces are drawn twice in the overlay, an outline-only mesh with the dark 4-pixel frame and then the fill with the 2-pixel outline in its colour (the outline pass tests depth with less-or-equal, in mesh order, so the colour line lies over the frame). `shown_look` gives the legends the set's name and colour, none while it is a ghost.
- `viewport_overlays.rs`: `set_swatch` and a row for the shown set in the path legend (Operations) and under the colour scale (Simulate, "Selection set “Pocket”").
- Tests: `req_sim_05_req_nfr_15_neighbouring_deviation_bands_are_clearly_apart`, `req_sim_05_req_nfr_15_material_left_darkens_in_clear_steps_with_contrast_on_the_view`, `req_ui_05_req_nfr_15_set_swatches_stay_apart_from_the_scale_and_the_roles`, `req_ui_05_an_old_stored_swatch_shows_as_the_new_one_at_its_place`, `req_ui_05_req_nfr_15_a_shown_set_joins_the_path_legend_framed`; the framed outline is checked in `req_ui_05_a_saved_set_is_highlighted_again_after_the_project_is_reopened`, and the absence of an outline on every band in the viewport's band test. The swatch test and the hex spot checks are updated.

## Consequences

- `apps/openchuck-ui`: the constants in `roles.rs` (`OVERCUT`, `LEFT_SMALL`, `LEFT_MEDIUM`, `LEFT_MORE`); `theme.rs` (`overcut` token); `SWATCHES` and `set_rgb` in `selection_panel.rs` (the mapping); the set outline in `SelectionPanel::overlay` (a boundary outline on the set's mesh, using the viewport's existing `Outline`, or two polylines); the legends in `viewport_overlays.rs`. No library change, no schema change.
- The mockup boards and images that show the old colours (`04-simulate.png`, `15-simulate-light.png`, `11-selection-sets.png`) no longer match. Redrawing them is OI-873.
- Lane SIMPOLISH (SIM-15): `band_colour` keeps its signature, so the band rows and the view keep using it. If SIMPOLISH changes the histogram rows' tint, it reads the same function.
