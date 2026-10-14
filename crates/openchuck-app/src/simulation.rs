// SPDX-License-Identifier: AGPL-3.0-or-later
//! Material removal simulation in the operation pipeline (work breakdown
//! slice 7; SIM-03, SIM-05, SIM-06, SIM-07; decision record 0023).
//!
//! Each setup's stock is simulated as a dexel stock at the project's cell.
//! Operations after the first in a setup are generated against the stock the
//! ones before them left (SIM-06), so a setup's operations are simulated
//! whenever another follows them, and all of them when a simulation is asked
//! for. The first operation of a setup is generated against the setup's
//! stock definition itself, which is exact.

use openchuck_project::id::SetupId;
use openchuck_sim::{
    Deviation, DeviationOptions, DexelStock, GroupResult, RapidCut, Simulator, compare,
};
use openchuck_types::geometry::Aabb;
use openchuck_types::ir::ToolpathGroup;
use openchuck_types::job::JobContext;
use openchuck_types::machine::MachineProfile;
pub use openchuck_types::simulation::{
    COARSEST_CELL, DEFAULT_CELL, FINEST_CELL, SimulationSettings,
};
use openchuck_types::stock::Stock;
use openchuck_types::tool::RevolvedProfile;

use crate::AppError;

/// What simulating one operation did.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationSimulation {
    /// The operation's name.
    pub operation: String,
    /// Moves simulated.
    pub moves: usize,
    /// Poses the moves were sampled into.
    pub poses: usize,
    /// Volume removed, in cubic millimetres.
    pub removed_volume: f64,
    /// Rapid moves that removed material (a fault in the toolpath).
    pub rapid_cuts: Vec<RapidCut>,
}

/// The simulation of one setup's operations, in machining order.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupSimulation {
    /// The setup's name.
    pub setup: String,
    /// Material volume before the first operation, in cubic millimetres.
    pub initial_volume: f64,
    /// Material volume after the last operation simulated.
    pub final_volume: f64,
    /// Columns along X and Y.
    pub columns: (usize, usize),
    /// Bytes the stock holds now.
    pub memory_bytes: usize,
    /// The operations simulated, in order.
    pub operations: Vec<OperationSimulation>,
    /// The stock against the setup's part (SIM-05), when a full simulation ran.
    pub deviation: Option<Deviation>,
    /// The stock left, as plain data for the viewport and later queries.
    pub stock: DexelStock,
}

/// The result of a simulation run (SIM-03, SIM-05, SIM-07).
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationReport {
    /// The settings used.
    pub settings: SimulationSettings,
    /// Each setup that has operations, in the order its first operation runs.
    pub setups: Vec<SetupSimulation>,
}

/// One setup's simulator while the pipeline runs.
pub(crate) struct SetupState {
    pub(crate) id: SetupId,
    simulator: Simulator,
    report: SetupSimulation,
}

impl std::fmt::Debug for SetupState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetupState")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl SetupState {
    /// A simulator for a setup's stock at the project's cell.
    pub(crate) fn new(
        id: SetupId,
        name: &str,
        stock: &Stock,
        part_bounds: Aabb,
        machine: &MachineProfile,
        settings: SimulationSettings,
    ) -> Result<Self, AppError> {
        settings.validate()?;
        let stock = DexelStock::from_stock(stock, Some(part_bounds), settings.cell.0)?;
        let report = SetupSimulation {
            setup: name.to_owned(),
            initial_volume: stock.volume(),
            final_volume: stock.volume(),
            columns: stock.columns(),
            memory_bytes: stock.memory_bytes(),
            operations: Vec::new(),
            deviation: None,
            stock: stock.clone(),
        };
        Ok(Self {
            id,
            simulator: Simulator::new(stock, machine, None),
            report,
        })
    }

    /// The material left so far.
    pub(crate) fn stock(&self) -> &DexelStock {
        self.simulator.stock()
    }

    /// Simulates an operation's moves.
    pub(crate) fn run(
        &mut self,
        operation: &str,
        group: &ToolpathGroup,
        tool: &RevolvedProfile,
        context: &JobContext<'_>,
    ) -> Result<GroupResult, AppError> {
        let result = self.simulator.run_group(group, tool, context)?;
        self.report.operations.push(OperationSimulation {
            operation: operation.to_owned(),
            moves: result.moves,
            poses: result.poses,
            removed_volume: result.removed_volume,
            rapid_cuts: result.rapid_cuts.clone(),
        });
        Ok(result)
    }

    /// Finishes the setup's report, comparing the stock with `part` when given.
    pub(crate) fn finish(
        self,
        part: Option<&openchuck_types::mesh::TriangleMesh>,
        settings: SimulationSettings,
    ) -> Result<SetupSimulation, AppError> {
        let mut report = self.report;
        let stock = self.simulator.stock();
        report.final_volume = stock.volume();
        report.memory_bytes = stock.memory_bytes();
        if let Some(mesh) = part {
            report.deviation = Some(compare(
                stock,
                mesh,
                &DeviationOptions::for_cell(settings.cell.0),
            )?);
        }
        report.stock = stock.clone();
        Ok(report)
    }
}
