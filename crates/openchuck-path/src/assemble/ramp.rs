// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bounded fixed-axis ramp descent planning (decision 0239). Geometry must
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
    let max_drop = length * slope;
    let count = (rise / max_drop).ceil();
    if !count.is_finite() || count > 10_000.0 {
        return Err(Refusal::TooMany);
    }
    let mut legs = Vec::new();
    let mut remaining = rise;
    let mut from = 0.0;
    let mut forward = true;
    while remaining > 0.0 {
        if cancelled() {
            return Err(Refusal::Cancelled);
        }
        if legs.len() >= 10_000 {
            return Err(Refusal::TooMany);
        }
        let drop = remaining.min(max_drop);
        let distance = if remaining <= max_drop {
            remaining / slope
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
