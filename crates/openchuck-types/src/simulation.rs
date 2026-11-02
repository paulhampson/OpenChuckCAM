// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simulation settings (SIM-07; decision record 0022 item 3).
//!
//! The default cell is 0.1 mm. The user may choose a finer one, at a cost in
//! time and memory, down to [`FINEST_CELL`]; a finer setting is never the
//! default. The setting belongs to the project, because later operations
//! are generated against the simulated stock (SIM-06), so the G-code depends
//! on it and must follow from the project alone (GC-07).

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::units::Millimetres;

/// The default simulation cell, in millimetres.
pub const DEFAULT_CELL: f64 = 0.1;

/// The finest cell a user may choose, in millimetres (OI-100).
pub const FINEST_CELL: f64 = 0.025;

/// The coarsest cell a user may choose, in millimetres.
pub const COARSEST_CELL: f64 = 1.0;

/// How the material removal simulation runs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulationSettings {
    /// Size of a stock cell: the resolution (SIM-07).
    pub cell: Millimetres,
}

impl Default for SimulationSettings {
    fn default() -> Self {
        Self {
            cell: Millimetres(DEFAULT_CELL),
        }
    }
}

impl SimulationSettings {
    /// True when every value is the default (files leave the table out then).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Fails unless the cell is between [`FINEST_CELL`] and [`COARSEST_CELL`].
    pub fn validate(&self) -> Result<()> {
        let c = self.cell.0;
        if !c.is_finite() || !(FINEST_CELL..=COARSEST_CELL).contains(&c) {
            return Err(Error::invalid(
                "simulation.cell",
                format!(
                    "{c} mm is outside the supported range {FINEST_CELL} to {COARSEST_CELL} mm \
                     (the default is {DEFAULT_CELL} mm)"
                ),
            ));
        }
        Ok(())
    }

    /// How much slower than the default this setting is expected to run: the
    /// poses per move grow as 1 / cell and the columns each pose meets as
    /// 1 / cell squared, so time grows about as the cube.
    pub fn relative_time(&self) -> f64 {
        (DEFAULT_CELL / self.cell.0).powi(3)
    }

    /// How much more memory than the default the stock takes: columns grow as
    /// 1 / cell squared.
    pub fn relative_memory(&self) -> f64 {
        (DEFAULT_CELL / self.cell.0).powi(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_sim_07_default_is_a_tenth_and_finer_is_opt_in() {
        let s = SimulationSettings::default();
        assert_eq!(s.cell, Millimetres(0.1));
        assert!(s.is_default());
        s.validate().unwrap();
        let fine = SimulationSettings {
            cell: Millimetres(FINEST_CELL),
        };
        fine.validate().unwrap();
        assert!((fine.relative_time() - 64.0).abs() < 1e-9);
        assert!((fine.relative_memory() - 16.0).abs() < 1e-9);
        for bad in [0.0, 0.01, 2.0, f64::NAN] {
            assert!(
                SimulationSettings {
                    cell: Millimetres(bad)
                }
                .validate()
                .is_err()
            );
        }
    }
}
