// SPDX-License-Identifier: AGPL-3.0-or-later
//! Conservative induction over ACTUAL across-going swept cutter capsules.
//! The previous layer must contain the complete centreline, so equal-radius
//! capsules (including their end caps outside the requested rectangle) nest.
//! Unions without this direct witness are conservatively unsupported.

use super::refuse;
use crate::RotaryMapping;
use crate::error::Result;
use openchuck_types::ir::ToolPose;

#[derive(Debug, Clone, Copy)]
pub(super) struct Footprint {
    x: f64,
    low: f64,
    high: f64,
}
impl Footprint {
    pub(super) fn of(
        from: ToolPose,
        to: ToolPose,
        rotary: RotaryMapping,
        arithmetic: f64,
    ) -> Result<Self> {
        if from.a != to.a || (from.tip.x - to.tip.x).abs() > arithmetic {
            return Err(refuse(
                "Box layer engagement requires a supported fixed-X across-going cut",
            ));
        }
        let u0 = rotary.to_axis(from.tip, from.a).across;
        let u1 = rotary.to_axis(to.tip, to.a).across;
        Ok(Self {
            x: from.tip.x + 0.5 * (to.tip.x - from.tip.x),
            low: u0.min(u1),
            high: u0.max(u1),
        })
    }
}
#[derive(Default)]
pub(super) struct Footprints(Vec<Footprint>);
impl Footprints {
    pub(super) fn insert(&mut self, footprint: Footprint) {
        self.0.push(footprint);
    }
    pub(super) fn ordered(mut self) -> Self {
        self.0
            .sort_by(|a, b| a.x.total_cmp(&b.x).then(a.low.total_cmp(&b.low)));
        self
    }
    pub(super) fn covers(
        &self,
        query: Footprint,
        arithmetic: f64,
        budget: &mut usize,
    ) -> Result<bool> {
        let first = self.0.partition_point(|p| p.x < query.x - arithmetic);
        for p in self
            .0
            .iter()
            .skip(first)
            .take_while(|p| p.x <= query.x + arithmetic)
        {
            *budget = budget
                .checked_sub(1)
                .ok_or_else(|| refuse("Box overlap engagement work budget exceeded"))?;
            if p.low <= query.low + arithmetic && p.high >= query.high - arithmetic {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
