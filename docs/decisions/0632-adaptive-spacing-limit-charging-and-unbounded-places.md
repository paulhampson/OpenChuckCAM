# 0632: Adaptive spacing charges its query limit station by station, and refines a place it cannot bound (OP-03, OP-04, OP-05, GC-07, NFR-06, NFR-07, NFR-12; issues #462 and #467; lane PATHBUGS)

- **Status:** proposed (2 Nov 2026) by lane PATHBUGS. Item 2 is the owner's choice (option B1 on OI-1510, 2 Nov 2026), relayed by the coordinator (decision 0049); item 4 is the owner's decision on OI-1800, relayed the same day. Item 1 implements decision 0239 item 8 as written (the coordinator's answer). Only the owner accepts this record.
- **Date:** 2 Nov 2026.
- **Issues:** #462 (helical finishing with adaptive spacing on, the default, ends with a partial path on the tutorial's turned piece), #467 (a proptest of the B-spline closest point missed its tolerance on macOS).
- **Amends:** [0239](0239-adaptive-pass-spacing.md) items 8 and 11 (adaptive pass spacing); [0344](0344-finishing-stepdown-and-free-ends.md) item 2 (the wording of a free end's missing Stepdown).
- **Open items:** closes OI-1510. Raises OI-1985 (a B-spline face closed on itself without a period: a closest-point search that sees the seam). Settles OI-1800 (the register row is on lane POLISH's branch, #479).

## Context

The tutorial's turned piece (`testdata/examples/baluster/baluster-project`: a 61 mm part with a bead, a cove, a vase with six flutes, a neck and a ball finial) was finished with helical finishing and the defaults of decision 0239: adaptive spacing on, Allowed excess 0.01 mm. The path came back partial (decision 0239 item 10): 20 places, and it had to be acknowledged before export. Lane DOCSUSER turned adaptive spacing off in the tutorial.

Diagnosis on `main` `484e4b2`, release build, the command line, four threads:

1. **The limit was not used up.** The measure charges the most queries a station can make (decision 0239 item 8: 63 drops) before it measures. It charged a whole interval at once: the upper half after the first added pass has 11,967 stations, so 753,921 queries were charged at 342,833 made, past the 1,000,000 limit, and that half of the helix was given up (14 places). The stations make about 7.7 drops each. With the limit lifted, the whole run made 507,371 queries.
2. **The charge was not a true bound.** Doubling a station's samples measured every sample again: up to 7 + 15 + 31 + 63 = 116 drops for a charge of 63.
3. **Places beside a step were never bounded.** With the limit lifted, 12 places stayed unverified (item 11). The flutes' edges, the bead and the finial's point make the material left between passes step from a plateau to nothing. Beside the step, the bound between two plateau stations, `(p + q + d) / 2` with `d` the step, stays above the Allowed excess whenever the plateau is above about two thirds of it, however fine the stations. OI-1510 recorded the same at rotary roughing's end edges.

The default stepover of the piece's 3.175 mm ball (0.4536 mm) leaves a nominal 0.016 mm, above the 0.01 mm Allowed excess, so adaptive spacing adds a pass along most of the helix: that is the setting working, not the fault.

## Decision

1. **The limit is charged station by station (implements decision 0239 item 8).** The stations of an interval are measured in runs whose most fits what is left of the limit; each run is charged after the real count of the runs before it. Work is still charged before it is done, and where the limit is met is the same at any thread count (GC-07, NFR-06). Doubling keeps the samples a station has (sample `k` of `n` is sample `2k` of `2n`, the same position exactly), so a station makes at most the 63 drops it is charged and the values measured are unchanged.
2. **A place the measure cannot bound is refined where a pass fits (amends decision 0239 item 11; the owner's choice B1 on OI-1510).** Where the samples meet the Allowed excess but the bound between them does not, a pass is added halfway, as where too much is found, and measured again. Such a place is reported as unverified only where no pass can be added: at the levels, the Smallest spacing, a tool that cannot reach or a limit. The note now says the passes were added where more than the Allowed excess "was left or could not be ruled out".
3. **The B-spline closest point is held to its stated tolerance in the tests (#467).** The proptest's sphere is a clamped rational patch whose `u = 0` and `u = 1` edges meet at +X, and a B-spline face carries no period. At the saved input, 0.0027 mm off that seam, the search answers on the `u = 0` edge, 8.9e-8 mm further than the sphere's nearest point: within the 1e-6 mm `SEARCH_TOLERANCE` of decision 0260, never nearer. The property now asserts that tolerance; the input has its own test; the seed is kept. A search that sees such a seam is OI-1985.
4. **A free end's missing Stepdown is said in plain words (OI-1800, amends decision 0344 item 2's wording).** "Set Stepdown to the depth of each layer, or add cutting data for this tool and material in the tool library and then choose Update from library in the operation's tool section: the project keeps its own copy of the tool, so a change in the library reaches it only then." (TL-05, TL-07.)

## Alternatives

- **Raise the 1,000,000 query limit.** It is the owner's limit (item 8), and item 1 alone keeps this piece at half of it.
- **Keep item 11 and make an unverified place a note that needs no acknowledgement (B2).** It weakens item 10 for the places most likely to hold excess; not chosen.
- **Adaptive spacing off by default for helical finishing (B3).** It reverses item 6; not chosen.
- **Refine the stations along the pass instead of adding a pass.** Beside a real step the bound never settles, whatever the station spacing.

## Consequences

- With adaptive spacing off, the output is byte-identical (item 13). Saved operations are unchanged (the migration rules of items 6 and 0239's second pull request stand).
- With it on, the output changes only where a run met the query limit early or reported a place it could not bound, in every family (the measure is shared): helical, axial, indexed and simultaneous finishing, rotary roughing's axial forms and flat Facing. The default output for a new operation changes, so the workspace's minor version is bumped (item 13; the number is assigned at release).
- Code: `crates/openchuck-path/src/adaptive.rs` (the charge, the samples, the refinement and its note), `finishing.rs` (the Stepdown wording). Tests in `openchuck-path` (`adaptive.rs`, `tests/adaptive.rs`, `tests/brep_faces.rs` and its regression file, `tests/helical_faces.rs`) and the new `crates/openchuck-app/tests/adaptive_example.rs`.

## Verification

To be completed before the pull request.
