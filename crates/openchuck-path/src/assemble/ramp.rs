// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bounded fixed-axis ramp descent planning (decision 0242). Geometry must
//! certify the corridor and every emitted sweep before these legs are used.

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Leg {
    pub from: f64,
    pub to: f64,
    pub drop: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Refusal {
    Invalid,
    TooShort,
    TooMany,
    NoProgress,
    Cancelled,
}

pub(super) fn plan(
    rise: f64,
    length: f64,
    diameter: f64,
    angle: f64,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<Leg>, Refusal> {
    if cancelled() {
        return Err(Refusal::Cancelled);
    }
    if ![rise, length, diameter, angle]
        .iter()
        .all(|v| v.is_finite() && *v > 0.0)
        || angle >= 90.0
    {
        return Err(Refusal::Invalid);
    }
    if length < diameter {
        return Err(Refusal::TooShort);
    }
    let slope = angle.to_radians().tan();
    // Both endpoints may round by half a written unit. Reserve the full
    // difference at 3 dp plus the supported float-controller uncertainty
    // (ULP <=0.0001 mm per word). An exact one-unit drop can otherwise
    // round to equal endpoints at a half-unit tie.
    const WRITTEN_Z: f64 = 0.0012;
    const WRITTEN_XY: f64 = 0.0022;
    let max_drop = (length - WRITTEN_XY) * slope - WRITTEN_Z;
    if rise < WRITTEN_Z || !max_drop.is_finite() || max_drop < WRITTEN_Z {
        return Err(Refusal::NoProgress);
    }
    let count = (rise / max_drop).ceil();
    if !count.is_finite() || count > 10_000.0 {
        return Err(Refusal::TooMany);
    }
    // Distribute descent rather than leaving an arbitrarily tiny final
    // drop which a controller would round into a horizontal reversal.
    let full_drop = rise / count;
    if full_drop < WRITTEN_Z {
        return Err(Refusal::NoProgress);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = count as usize;
    let mut legs = Vec::new();
    let mut remaining = rise;
    let mut from = 0.0;
    let mut forward = true;
    for index in 0..count {
        if cancelled() {
            return Err(Refusal::Cancelled);
        }
        if legs.len() >= 10_000 {
            return Err(Refusal::TooMany);
        }
        let last = index + 1 == count;
        let drop = if last { remaining } else { full_drop };
        let distance = if last {
            (drop + WRITTEN_Z) / slope + WRITTEN_XY
        } else {
            length
        };
        let to = if forward {
            from + distance
        } else {
            from - distance
        };
        let next = remaining - drop;
        if next >= remaining || (to - from).abs() <= f64::EPSILON * from.abs().max(1.0) {
            return Err(Refusal::NoProgress);
        }
        legs.push(Leg { from, to, drop });
        remaining = next;
        from = to;
        forward = !forward;
    }
    Ok(legs)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]
    use super::*;

    #[test]
    fn req_op_11_req_op_13_multiple_descending_legs_and_partial_final_leg() {
        let legs = plan(2.0, 3.175, 3.175, 3.0, || false).unwrap();
        assert_eq!(legs.len(), 13);
        let mut previous = 0.0;
        let mut height = 2.0;
        for leg in &legs {
            assert_eq!(leg.from, previous);
            let distance = (leg.to - leg.from).abs();
            assert!(distance > 0.0 && leg.drop > 0.0);
            assert!(leg.drop / distance <= 3.0_f64.to_radians().tan() + 1e-12);
            assert!((0.0..=3.175).contains(&leg.to));
            height -= leg.drop;
            previous = leg.to;
        }
        assert!(height.abs() < 1e-12);
        let last = legs.last().unwrap();
        assert!((last.to - last.from).abs() < 3.175);
    }

    #[test]
    fn req_op_11_posted_three_decimal_legs_never_exceed_ramp_angle() {
        let written = |v: f64| format!("{v:.3}").parse::<f64>().unwrap();
        for rise in [0.01, 1.0, 2.0, 6.0] {
            let legs = plan(rise, 3.175, 3.175, 3.0, || false).unwrap();
            let mut height = 20.0;
            for leg in legs {
                let next = height - leg.drop;
                let run = (written(leg.to) - written(leg.from)).abs();
                let drop = written(height) - written(next);
                assert!(
                    drop <= run * 3.0_f64.to_radians().tan() + 1e-12,
                    "written slope exceeds 3 degrees: run={run} drop={drop}"
                );
                height = next;
            }
        }
    }

    #[test]
    fn req_op_11_every_written_leg_descends_and_sub_resolution_entries_refuse() {
        let written = |v: f64| format!("{v:.3}").parse::<f64>().unwrap();
        let almost_multiple =
            ((3.175 - 0.002) * 3.0_f64.to_radians().tan() - 0.001) * 4.0 + 0.00001;
        let mut height = 20.0;
        for leg in plan(almost_multiple, 3.175, 3.175, 3.0, || false).unwrap() {
            let next = height - leg.drop;
            assert!(
                written(next) < written(height),
                "a reversal leg lost all descent at written precision"
            );
            height = next;
        }
        assert_eq!(
            plan(0.0001, 3.175, 3.175, 3.0, || false),
            Err(Refusal::NoProgress)
        );
        assert_eq!(
            plan(1.0, 0.001, 0.001, 3.0, || false),
            Err(Refusal::NoProgress)
        );
    }

    #[test]
    fn req_op_11_half_unit_rounding_must_not_remove_all_descent() {
        assert_eq!(
            plan(0.001, 3.175, 3.175, 3.0, || false),
            Err(Refusal::NoProgress)
        );
        let written = |v: f64| f64::from(format!("{v:.3}").parse::<f32>().unwrap());
        for start in [0.0095, 20.0005, 50.0005, 800.0005] {
            let mut z = start;
            for leg in plan(0.0013, 3.175, 3.175, 3.0, || false).unwrap() {
                let next = z - leg.drop;
                assert!(written(next) < written(z));
                let run = (written(leg.to) - written(leg.from)).abs();
                assert!(written(z) - written(next) <= run * 3.0_f64.to_radians().tan() + 1e-12);
                z = next;
            }
        }
    }

    #[test]
    fn req_op_11_minimum_length_is_inclusive_and_impossible_entries_refuse() {
        assert!(plan(1.0, 3.175, 3.175, 3.0, || false).is_ok());
        assert_eq!(
            plan(1.0, 3.174999, 3.175, 3.0, || false),
            Err(Refusal::TooShort)
        );
        assert_eq!(
            plan(1e10, 3.175, 3.175, 3.0, || false),
            Err(Refusal::TooMany)
        );
        for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert_eq!(
                plan(1.0, 3.175, 3.175, bad, || false),
                Err(Refusal::Invalid)
            );
        }
    }

    #[test]
    fn req_nfr_04_ramp_planning_cancels_during_generation() {
        let mut checks = 0;
        let result = plan(2.0, 3.175, 3.175, 3.0, || {
            checks += 1;
            checks > 3
        });
        assert_eq!(result, Err(Refusal::Cancelled));
    }
}
