// SPDX-License-Identifier: AGPL-3.0-or-later
//! The order of an operation's passes (OP-14, OP-15, SIM-08; issue #469;
//! decision record 0520).
//!
//! A strategy that cuts extra passes after its main ones (the turns at faces
//! across the rotary axis, decision record 0297, and the contours on surfaces
//! hidden from the drive, decision record 0368) gives them here as groups.
//! With [`BY_REGION`] each group is cut beside the main passes of its own
//! region along X, instead of after all of them, so the tool does not go
//! back along the part for them; with [`AFTER_MAIN`] they are cut after the
//! main passes, as before.
//!
//! What may not change is fixed by the strategy, not here: the main passes
//! keep their order, and every pass keeps its points, its direction and its
//! way round (OP-15, no reversal in a cut). A group moves as one, so a turn
//! split into pieces, or the contours of one hidden area, stay together, and
//! never goes before the first main pass. Every link is still made and
//! checked by the shared assembly (OP-14, SET-09).
//!
//! A group goes where the links it changes travel least along X, A the short
//! way round deciding a tie (the assembly fits each pass by whole turns to
//! the one before). The number of links does not change, so neither does the
//! time to leave, retract and enter: only the travel between passes does.
//! The new order is kept only when its links travel less along X in all, and
//! take no more than one hundredth of the operation's estimated time longer
//! than before (the owner's choice, decision record 0520; SIM-08): the
//! links' travel from the machine's rapid rates along X and in A, moving
//! together, and the estimate from the cutting at the programmed feed plus
//! each link out to the clearance radius and back. Otherwise the order is
//! as before.
//!
//! Deterministic (GC-07, NFR-06): the groups are placed one after another
//! in the order given, and a tie goes to the earliest place.

use openchuck_types::machine::MaxFeeds;

use crate::params::{ParameterKind, ParameterSpec};
use crate::strategy::Pass;

/// The parameter's name.
pub const PARAMETER: &str = "pass_order";
/// The extra passes are cut after the main ones, as before the option: the
/// default, so an operation saved without the parameter is unchanged.
pub const AFTER_MAIN: &str = "after_main";
/// Each extra pass is cut beside the main passes of its region, where that
/// saves time. A new operation is given this (decision record 0520).
pub const BY_REGION: &str = "by_region";

/// The parameter, for the strategies that have extra passes.
pub fn spec() -> ParameterSpec {
    ParameterSpec {
        name: PARAMETER.to_owned(),
        label: "Pass order".to_owned(),
        description: "By region: the turns at faces across the axis and the passes on hidden \
                      surfaces are cut beside the passes of their region, where that saves \
                      time between passes. After main passes: they are cut after all the \
                      main passes, as before. The passes themselves are the same either way."
            .to_owned(),
        kind: ParameterKind::Choice {
            options: vec![BY_REGION.to_owned(), AFTER_MAIN.to_owned()],
            default: AFTER_MAIN.to_owned(),
        },
    }
}

/// The machine's rapid rates and the clearance radius, for the cost of a
/// link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rates {
    /// Along X, mm per minute.
    x: f64,
    /// In A, degrees per minute.
    a: f64,
    /// Towards and away from the axis (the machine's Z), mm per minute.
    z: f64,
    /// The fastest A may turn while cutting, degrees per minute.
    turning: f64,
    /// The radius links retract to, mm (SET-06).
    clearance: f64,
}

impl Rates {
    /// The rapid rates of `feeds`. A rate that is not positive and finite
    /// counts as so slow that any move along that axis costs the most
    /// (NFR-07): the order then avoids it, and never divides by zero.
    pub(crate) fn of(feeds: &MaxFeeds, clearance: f64) -> Self {
        let rate = |r: f64| {
            if r.is_finite() && r > 0.0 {
                r
            } else {
                f64::MIN_POSITIVE
            }
        };
        Self {
            x: rate(feeds.rapid_linear.0),
            a: rate(feeds.rapid_rotary.0),
            z: rate(feeds.rapid_z.0),
            turning: rate(feeds.rotary.0),
            clearance: if clearance.is_finite() {
                clearance
            } else {
                0.0
            },
        }
    }

    /// Minutes for a link from `p` to `q`.
    fn link(self, p: End, q: End) -> f64 {
        let turn = (q.a - p.a).rem_euclid(360.0);
        let turn = turn.min(360.0 - turn);
        ((q.x - p.x).abs() / self.x).max(turn / self.a)
    }
}

/// Where a pass starts or ends: its tip's X, its A and its distance from
/// the rotary axis.
#[derive(Debug, Clone, Copy, PartialEq)]
struct End {
    x: f64,
    a: f64,
    r: f64,
}

/// The first and last of a run of passes that have points.
fn ends(passes: &[Pass]) -> Option<(End, End)> {
    let first = passes.iter().find_map(|p| p.points.first())?;
    let last = passes.iter().rev().find_map(|p| p.points.last())?;
    let end = |c: &crate::strategy::CutPoint| End {
        x: c.tip.x,
        a: c.a.0,
        r: c.tip.y.hypot(c.tip.z),
    };
    Some((end(first), end(last)))
}

/// Millimetres along X that count as much as one degree of A when two
/// places travel the same along X: a tie-break only.
const TIE: f64 = 1e-4;

/// How much longer the links may take in all, as a share of the cutting
/// time (the owner's choice: one per cent, decision record 0520).
const ALLOWANCE: f64 = 0.01;

impl End {
    /// The travel along X from `self` to `q`, with A as the tie-break.
    fn travel(self, q: End) -> f64 {
        let turn = (q.a - self.a).rem_euclid(360.0);
        (q.x - self.x).abs() + TIE * turn.min(360.0 - turn)
    }
}

/// The sum of `cost` over the links between consecutive blocks.
fn total(blocks: &[Vec<Pass>], cost: impl Fn(End, End) -> f64) -> f64 {
    let ends: Vec<(End, End)> = blocks.iter().filter_map(|b| ends(b)).collect();
    ends.windows(2)
        .map(|w| match w {
            [(_, p), (q, _)] => cost(*p, *q),
            _ => 0.0,
        })
        .sum()
}

/// The estimated minutes of `blocks`, no more than the assembled path
/// takes: each cut at `feed` or as fast as A may turn, whichever is slower,
/// and each link between passes out to the clearance radius, across and
/// back in, at the rapid rates. It is the base of the allowance only, so
/// the allowance is never more than one hundredth of the real time.
fn estimate(blocks: &[Vec<Pass>], rates: Rates, feed: f64) -> f64 {
    let passes: Vec<Vec<Pass>> = blocks.iter().flatten().map(|p| vec![p.clone()]).collect();
    let radial = |p: End, q: End| {
        ((rates.clearance - p.r).max(0.0) + (rates.clearance - q.r).max(0.0)) / rates.z
    };
    cutting(blocks, rates, feed) + total(&passes, |p, q| radial(p, q) + rates.link(p, q))
}

/// Minutes to cut `blocks`: each move at `feed` mm per minute along the
/// tips or at the fastest A may turn, whichever is slower; a feed that is
/// not positive and finite counts as no time (NFR-07).
fn cutting(blocks: &[Vec<Pass>], rates: Rates, feed: f64) -> f64 {
    let feed = if feed.is_finite() && feed > 0.0 {
        feed
    } else {
        f64::INFINITY
    };
    blocks
        .iter()
        .flatten()
        .map(|p| {
            p.points
                .windows(2)
                .map(|w| match w {
                    [a, b] => {
                        ((b.tip - a.tip).length() / feed).max((b.a.0 - a.a.0).abs() / rates.turning)
                    }
                    _ => 0.0,
                })
                .sum::<f64>()
        })
        .sum()
}

/// The passes in their order, and what the order did, for the user.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Arranged {
    /// The passes, in cutting order.
    pub passes: Vec<Pass>,
    /// A note saying what cutting by region did or why it did not, when it
    /// was asked for and there was something to move.
    pub note: Option<String>,
}

/// `main` then `groups`, in the order `order` (the parameter's value) asks
/// for, the cutting at `feed` mm per minute. `after_main` (and any value
/// but `by_region`) keeps them as given, with no note.
pub(crate) fn arrange(
    order: &str,
    main: Vec<Pass>,
    groups: Vec<Vec<Pass>>,
    rates: Rates,
    feed: f64,
) -> Arranged {
    if order == BY_REGION {
        by_region(main, groups, rates, feed)
    } else {
        Arranged {
            passes: main
                .into_iter()
                .chain(groups.into_iter().flatten())
                .collect(),
            note: None,
        }
    }
}

/// Each group placed after the main pass (or group already placed) where
/// its links travel least along X, never before the first main pass; kept
/// as given unless that travels less in all within the time allowed.
fn by_region(main: Vec<Pass>, groups: Vec<Vec<Pass>>, rates: Rates, feed: f64) -> Arranged {
    let mut old: Vec<Vec<Pass>> = main.into_iter().map(|p| vec![p]).collect();
    // A main pass with no points cannot anchor a group: it keeps its place
    // and the groups go round it.
    let first = old.iter().position(|b| ends(b).is_some());
    let mut placed = old.clone();
    let mut moved = false;
    for group in groups {
        old.push(group.clone());
        let (Some(first), Some((s, f))) = (first, ends(&group)) else {
            placed.push(group);
            continue;
        };
        let mut best: Option<(f64, usize)> = None;
        for at in first + 1..=placed.len() {
            let (before, after) = placed.split_at(at);
            let prev = before.iter().rev().find_map(|b| ends(b)).map(|e| e.1);
            let next = after.iter().find_map(|b| ends(b)).map(|e| e.0);
            let cost = match (prev, next) {
                (Some(p), Some(n)) => p.travel(s) + f.travel(n) - p.travel(n),
                (Some(p), None) => p.travel(s),
                _ => continue,
            };
            if best.is_none_or(|(c, _)| cost < c) {
                best = Some((cost, at));
            }
        }
        let at = best.map_or(placed.len(), |b| b.1);
        moved |= at != placed.len();
        placed.insert(at, group);
    }
    let time = |b: &[Vec<Pass>]| total(b, |p, q| rates.link(p, q));
    let x = |b: &[Vec<Pass>]| total(b, |p, q| (q.x - p.x).abs());
    let (x0, x1) = (x(&old), x(&placed));
    let longer = 60.0 * (time(&placed) - time(&old));
    let allowed = 60.0 * ALLOWANCE * estimate(&old, rates, feed);
    let (chosen, note) = if !moved || x1 >= x0 {
        (old, None)
    } else if longer <= allowed {
        let note = format!(
            "the turns at faces across the rotary axis and the passes on surfaces hidden from \
             the drive are cut beside the passes of their region (pass order by region): the \
             tool travels {x1:.1} mm along the part between passes instead of {x0:.1} mm, and \
             the moves between passes take about {longer:+.1} s"
        );
        (placed, Some(note))
    } else {
        let note = format!(
            "the turns at faces across the rotary axis and the passes on surfaces hidden from \
             the drive are cut after the main passes, although the pass order is by region: \
             beside their region the moves between passes would take about {longer:.1} s \
             longer, more than one hundredth of the operation's estimated time ({allowed:.1} s)"
        );
        (old, Some(note))
    };
    Arranged {
        passes: chosen.into_iter().flatten().collect(),
        note,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]

    use openchuck_types::geometry::Point3;
    use openchuck_types::machine::MachineProfile;
    use openchuck_types::units::Degrees;

    use super::*;
    use crate::strategy::CutPoint;

    fn point(x: f64, a: f64) -> CutPoint {
        let tip = Point3::new(x, 0.0, 10.0);
        CutPoint {
            tip,
            a: Degrees(a),
            contact: tip,
        }
    }

    /// A pass along X from `x0` to `x1`, A turning from `a0` to `a1`.
    fn pass(x0: f64, x1: f64, a0: f64, a1: f64) -> Pass {
        Pass {
            points: (0..=10)
                .map(|i| {
                    let t = f64::from(i) / 10.0;
                    point(x0 + t * (x1 - x0), a0 + t * (a1 - a0))
                })
                .collect(),
            ..Pass::default()
        }
    }

    /// A whole turn at `x`, from `a`.
    fn turn(x: f64, a: f64) -> Pass {
        pass(x, x, a, a + 360.0)
    }

    fn arrange_passes(
        order: &str,
        main: Vec<Pass>,
        groups: Vec<Vec<Pass>>,
        rates: Rates,
        feed: f64,
    ) -> Vec<Pass> {
        arrange(order, main, groups, rates, feed).passes
    }

    /// A finishing feed, mm per minute.
    const FEED: f64 = 600.0;

    fn rates() -> Rates {
        Rates::of(&MachineProfile::example_x_rotary().max_feeds, 20.0)
    }

    fn label(p: &Pass) -> (f64, f64) {
        let (s, f) = ends(std::slice::from_ref(p)).unwrap();
        (s.x, f.x)
    }

    #[test]
    fn req_gc_07_after_main_keeps_the_order_as_given() {
        let main = vec![pass(0.0, 10.0, 0.0, 3600.0), pass(10.0, 20.0, 0.0, 3600.0)];
        let groups = vec![vec![turn(5.0, 0.0)], vec![turn(15.0, 0.0)]];
        let out = arrange_passes(AFTER_MAIN, main.clone(), groups.clone(), rates(), FEED);
        let expected: Vec<Pass> = main
            .into_iter()
            .chain(groups.into_iter().flatten())
            .collect();
        assert_eq!(out, expected);
        // An unknown value is not a reason to change a saved operation.
        let unknown = arrange_passes("other", expected.clone(), Vec::new(), rates(), FEED);
        assert_eq!(unknown, expected);
    }

    #[test]
    fn req_op_14_req_sim_08_a_turn_is_cut_between_the_main_passes_of_its_region() {
        // Three zones of a spiral, each ending at A 0 (by whole turns), and a
        // turn at the face between the first and the second, at A 0.
        let main = vec![
            pass(0.0, 10.0, 0.0, 3600.0),
            pass(10.0, 20.0, 0.0, 3600.0),
            pass(20.0, 30.0, 0.0, 3600.0),
        ];
        let groups = vec![vec![turn(10.0, 0.0)], vec![turn(20.0, 0.0)]];
        let out = arrange_passes(BY_REGION, main, groups, rates(), FEED);
        let xs: Vec<(f64, f64)> = out.iter().map(label).collect();
        assert_eq!(
            xs,
            vec![
                (0.0, 10.0),
                (10.0, 10.0),
                (10.0, 20.0),
                (20.0, 20.0),
                (20.0, 30.0)
            ]
        );
    }

    #[test]
    fn req_op_15_passes_keep_their_points_and_the_main_passes_their_order() {
        let main = vec![
            pass(0.0, 10.0, 0.0, 3600.0),
            pass(10.0, 20.0, 15.0, 3615.0),
            pass(20.0, 30.0, 40.0, 3640.0),
        ];
        // A group of two pieces of one turn, which must stay together.
        let groups = vec![
            vec![
                pass(20.0, 20.0, 40.0, 200.0),
                pass(20.0, 20.0, 220.0, 400.0),
            ],
            vec![turn(10.0, 15.0)],
        ];
        let out = arrange_passes(BY_REGION, main.clone(), groups.clone(), rates(), FEED);
        // Every pass is there once, unchanged (none reversed).
        assert_eq!(out.len(), 6);
        for p in main.iter().chain(groups.iter().flatten()) {
            assert_eq!(out.iter().filter(|q| *q == p).count(), 1);
        }
        // The main passes in their order.
        let at: Vec<usize> = main
            .iter()
            .map(|p| out.iter().position(|q| q == p).unwrap())
            .collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");
        // The two pieces of the first group one after the other.
        let first = out.iter().position(|q| *q == groups[0][0]).unwrap();
        assert_eq!(out[first + 1], groups[0][1]);
        // Nothing before the first main pass.
        assert_eq!(out[0], main[0]);
    }

    #[test]
    fn req_sim_08_a_group_stays_after_the_main_passes_when_moving_it_saves_nothing() {
        // The turn is at the far end, where the main passes end: moving it
        // anywhere else costs more.
        let main = vec![pass(0.0, 10.0, 0.0, 3600.0), pass(10.0, 20.0, 0.0, 3600.0)];
        let groups = vec![vec![turn(20.0, 0.0)]];
        let out = arrange_passes(BY_REGION, main.clone(), groups.clone(), rates(), FEED);
        let expected: Vec<Pass> = main
            .into_iter()
            .chain(groups.into_iter().flatten())
            .collect();
        assert_eq!(out, expected);
    }

    #[test]
    fn req_sim_08_the_links_never_travel_further_nor_take_much_longer() {
        // Deterministic pseudo-random layouts, A included: whatever the
        // places chosen, the links travel no further along X in all, and
        // take no longer than one hundredth of the cutting time more.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1_u64 << 53) as f64
        };
        let r = rates();
        let mut changed = 0;
        for _ in 0..200 {
            let main: Vec<Pass> = (0..5)
                .map(|k| {
                    let x = f64::from(k) * 8.0;
                    let a = 360.0 * next();
                    pass(x, x + 8.0 * next(), a, a + 720.0 * next())
                })
                .collect();
            let groups: Vec<Vec<Pass>> = (0..4)
                .map(|_| vec![turn(40.0 * next(), 360.0 * next())])
                .collect();
            let before: Vec<Vec<Pass>> = main
                .iter()
                .cloned()
                .map(|p| vec![p])
                .chain(groups.iter().cloned())
                .collect();
            let out = arrange_passes(BY_REGION, main, groups, r, FEED);
            let after: Vec<Vec<Pass>> = out.into_iter().map(|p| vec![p]).collect();
            let x = |b: &[Vec<Pass>]| total(b, |p, q| (q.x - p.x).abs());
            let time = |b: &[Vec<Pass>]| total(b, |p, q| r.link(p, q));
            assert!(x(&after) <= x(&before) + 1e-12);
            assert!(time(&after) <= time(&before) + ALLOWANCE * estimate(&before, r, FEED) + 1e-12);
            changed += usize::from(after != before);
        }
        assert!(changed > 50, "{changed}");
    }

    #[test]
    fn req_sim_08_a_place_that_costs_more_than_the_allowance_is_not_taken() {
        // The turn belongs between the two passes along X, but there it
        // would need A to turn half a turn twice; at a fast feed the cutting
        // time is short, so the allowance does not cover it.
        let main = vec![pass(0.0, 10.0, 0.0, 0.0), pass(10.0, 20.0, 0.0, 0.0)];
        let groups = vec![vec![pass(10.0, 10.0, 180.0, 180.0)]];
        let kept = arrange_passes(BY_REGION, main.clone(), groups.clone(), rates(), 1.0e6);
        let expected: Vec<Pass> = main
            .iter()
            .cloned()
            .chain(groups.iter().flatten().cloned())
            .collect();
        assert_eq!(kept, expected);
        // At a slow feed the same move is within it.
        let moved = arrange_passes(BY_REGION, main.clone(), groups.clone(), rates(), 1.0);
        assert_eq!(moved[1], groups[0][0]);
    }

    #[test]
    fn req_sim_08_the_note_says_what_the_order_did() {
        let main = vec![pass(0.0, 10.0, 0.0, 0.0), pass(10.0, 20.0, 0.0, 0.0)];
        let near = vec![vec![pass(10.0, 10.0, 0.0, 0.0)]];
        let after = arrange(AFTER_MAIN, main.clone(), near.clone(), rates(), FEED);
        assert_eq!(after.note, None);
        let moved = arrange(BY_REGION, main.clone(), near, rates(), FEED);
        let note = moved.note.unwrap();
        assert!(
            note.contains("travels 0.0 mm along the part between passes instead of 10.0 mm"),
            "{note}"
        );
        let far = vec![vec![pass(10.0, 10.0, 180.0, 180.0)]];
        let kept = arrange(BY_REGION, main.clone(), far, rates(), 1.0e6);
        let note = kept.note.unwrap();
        assert!(note.contains("are cut after the main passes"), "{note}");
        let last = vec![vec![pass(20.0, 20.0, 0.0, 0.0)]];
        assert_eq!(arrange(BY_REGION, main, last, rates(), FEED).note, None);
    }

    #[test]
    fn req_nfr_07_passes_without_points_and_bad_rates_do_not_fail() {
        let main = vec![Pass::default(), pass(0.0, 10.0, 0.0, 360.0)];
        let groups = vec![vec![Pass::default()], vec![turn(5.0, 0.0)]];
        let mut feeds = MachineProfile::example_x_rotary().max_feeds;
        feeds.rapid_linear.0 = 0.0;
        feeds.rapid_rotary.0 = f64::NAN;
        let out = arrange_passes(
            BY_REGION,
            main,
            groups,
            Rates::of(&feeds, f64::NAN),
            f64::NAN,
        );
        assert_eq!(out.len(), 4);
        let out = arrange_passes(
            BY_REGION,
            Vec::new(),
            vec![vec![turn(5.0, 0.0)]],
            rates(),
            FEED,
        );
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn req_op_15_a_is_measured_the_short_way_round() {
        let r = rates();
        let p = End {
            x: 0.0,
            a: 10.0,
            r: 0.0,
        };
        let q = End {
            x: 0.0,
            a: 3950.0,
            r: 0.0,
        }; // 350 degrees on, 20 back
        let back = End {
            x: 0.0,
            a: -10.0,
            r: 0.0,
        };
        assert!((r.link(p, q) - r.link(p, back)).abs() < 1e-12);
        assert!((p.travel(q) - p.travel(back)).abs() < 1e-12);
        assert!((p.travel(back) - 20.0 * TIE).abs() < 1e-12);
    }
}
