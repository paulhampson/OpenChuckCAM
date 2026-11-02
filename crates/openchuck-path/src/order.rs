// SPDX-License-Identifier: AGPL-3.0-or-later
//! The order of an operation's passes (OP-14, OP-15, SIM-08; issue #469;
//! decision record 0639).
//!
//! A strategy that cuts extra passes after its main ones (the turns at faces
//! across the rotary axis, decision record 0297, and the contours on surfaces
//! hidden from the drive, decision record 0368) gives them here as groups.
//! With [`BY_REGION`] each group is cut where it costs the least travel
//! between passes, beside the main passes of its own region, instead of
//! after all of them; with [`AFTER_MAIN`] they are cut after the main passes,
//! as before.
//!
//! What may not change is fixed by the strategy, not here: the main passes
//! keep their order, and every pass keeps its points, its direction and its
//! way round (OP-15, no reversal in a cut). A group moves as one, so a turn
//! split into pieces, or the contours of one hidden area, stay together.
//! Every link is still made and checked by the shared assembly (OP-14,
//! SET-09). The cost of a place is the time of the links that it changes,
//! from the machine's rapid rates along X and in A, moving together, A the
//! short way round, as the assembly fits each pass by whole turns to the
//! one before. The number of links does not change, so neither does the
//! time to leave, retract and enter. The order chosen is kept only when its
//! links take less time in all than those of the old order, so the
//! estimated time is never longer (SIM-08).
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
/// saves time. A new operation is given this (decision record 0639).
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

/// The machine's rapid rates, for the cost of a link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rates {
    /// Along X, mm per minute.
    x: f64,
    /// In A, degrees per minute.
    a: f64,
}

impl Rates {
    /// The rapid rates of `feeds`. A rate that is not positive and finite
    /// counts as so slow that any move along that axis costs the most
    /// (NFR-07): the order then avoids it, and never divides by zero.
    pub(crate) fn of(feeds: &MaxFeeds) -> Self {
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
        }
    }

    /// Minutes for a link from `p` to `q`.
    fn link(self, p: End, q: End) -> f64 {
        let turn = (q.a - p.a).rem_euclid(360.0);
        let turn = turn.min(360.0 - turn);
        ((q.x - p.x).abs() / self.x).max(turn / self.a)
    }
}

/// Where a pass starts or ends: its tip's X and its A.
#[derive(Debug, Clone, Copy, PartialEq)]
struct End {
    x: f64,
    a: f64,
}

/// The first and last of a run of passes that have points.
fn ends(passes: &[Pass]) -> Option<(End, End)> {
    let first = passes.iter().find_map(|p| p.points.first())?;
    let last = passes.iter().rev().find_map(|p| p.points.last())?;
    let end = |c: &crate::strategy::CutPoint| End {
        x: c.tip.x,
        a: c.a.0,
    };
    Some((end(first), end(last)))
}

/// The time of the links between consecutive blocks.
fn links(blocks: &[Vec<Pass>], rates: Rates) -> f64 {
    let ends: Vec<(End, End)> = blocks.iter().filter_map(|b| ends(b)).collect();
    ends.windows(2)
        .map(|w| match w {
            [(_, p), (q, _)] => rates.link(*p, *q),
            _ => 0.0,
        })
        .sum()
}

/// `main` then `groups`, in the order `order` (the parameter's value) asks
/// for. `after_main` (and any value but `by_region`) keeps them as given.
pub(crate) fn arrange(
    order: &str,
    main: Vec<Pass>,
    groups: Vec<Vec<Pass>>,
    rates: Rates,
) -> Vec<Pass> {
    if order == BY_REGION {
        by_region(main, groups, rates)
    } else {
        main.into_iter()
            .chain(groups.into_iter().flatten())
            .collect()
    }
}

/// Each group placed after the main pass (or group already placed) where
/// its links cost the least, never before the first main pass; kept as
/// given unless that saves time in all.
fn by_region(main: Vec<Pass>, groups: Vec<Vec<Pass>>, rates: Rates) -> Vec<Pass> {
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
            let prev = placed[..at].iter().rev().find_map(|b| ends(b)).map(|e| e.1);
            let next = placed[at..].iter().find_map(|b| ends(b)).map(|e| e.0);
            let cost = match (prev, next) {
                (Some(p), Some(n)) => rates.link(p, s) + rates.link(f, n) - rates.link(p, n),
                (Some(p), None) => rates.link(p, s),
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
    let chosen = if moved && links(&placed, rates) < links(&old, rates) {
        placed
    } else {
        old
    };
    chosen.into_iter().flatten().collect()
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

    fn rates() -> Rates {
        Rates::of(&MachineProfile::example_x_rotary().max_feeds)
    }

    fn label(p: &Pass) -> (f64, f64) {
        let (s, f) = ends(std::slice::from_ref(p)).unwrap();
        (s.x, f.x)
    }

    #[test]
    fn req_gc_07_after_main_keeps_the_order_as_given() {
        let main = vec![pass(0.0, 10.0, 0.0, 3600.0), pass(10.0, 20.0, 0.0, 3600.0)];
        let groups = vec![vec![turn(5.0, 0.0)], vec![turn(15.0, 0.0)]];
        let out = arrange(AFTER_MAIN, main.clone(), groups.clone(), rates());
        let expected: Vec<Pass> = main
            .into_iter()
            .chain(groups.into_iter().flatten())
            .collect();
        assert_eq!(out, expected);
        // An unknown value is not a reason to change a saved operation.
        let unknown = arrange("other", expected.clone(), Vec::new(), rates());
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
        let out = arrange(BY_REGION, main, groups, rates());
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
        let out = arrange(BY_REGION, main.clone(), groups.clone(), rates());
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
        let out = arrange(BY_REGION, main.clone(), groups.clone(), rates());
        let expected: Vec<Pass> = main
            .into_iter()
            .chain(groups.into_iter().flatten())
            .collect();
        assert_eq!(out, expected);
    }

    #[test]
    fn req_sim_08_the_links_never_take_longer_than_after_the_main_passes() {
        // Deterministic pseudo-random layouts, A included: whatever the
        // places chosen, the links in all never take longer than before.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1_u64 << 53) as f64
        };
        let r = rates();
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
            let out = arrange(BY_REGION, main, groups, r);
            let after: Vec<Vec<Pass>> = out.into_iter().map(|p| vec![p]).collect();
            assert!(links(&after, r) <= links(&before, r) + 1e-12);
        }
    }

    #[test]
    fn req_nfr_07_passes_without_points_and_bad_rates_do_not_fail() {
        let main = vec![Pass::default(), pass(0.0, 10.0, 0.0, 360.0)];
        let groups = vec![vec![Pass::default()], vec![turn(5.0, 0.0)]];
        let mut feeds = MachineProfile::example_x_rotary().max_feeds;
        feeds.rapid_linear.0 = 0.0;
        feeds.rapid_rotary.0 = f64::NAN;
        let out = arrange(BY_REGION, main, groups, Rates::of(&feeds));
        assert_eq!(out.len(), 4);
        let out = arrange(BY_REGION, Vec::new(), vec![vec![turn(5.0, 0.0)]], rates());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn req_op_15_a_is_measured_the_short_way_round() {
        let r = rates();
        let p = End { x: 0.0, a: 10.0 };
        let q = End { x: 0.0, a: 3950.0 }; // 350 degrees on, 20 back
        let back = End { x: 0.0, a: -10.0 };
        assert!((r.link(p, q) - r.link(p, back)).abs() < 1e-12);
    }
}
