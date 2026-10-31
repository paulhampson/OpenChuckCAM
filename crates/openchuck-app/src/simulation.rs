// SPDX-License-Identifier: AGPL-3.0-or-later
//! Material removal simulation in the operation pipeline (work breakdown
//! slice 7; SIM-03, SIM-05, SIM-06, SIM-07; decision record 0025).
//!
//! Each setup's stock is simulated as a dexel stock at the project's cell.
//! Operations after the first in a setup are generated against the stock the
//! ones before them left (SIM-06), so a setup's operations are simulated
//! whenever another follows them, and all of them when a simulation is asked
//! for. The first operation of a setup is generated against the setup's
//! stock definition itself, which is exact.

use std::sync::{Arc, OnceLock};

use openchuck_project::Project;
use openchuck_project::id::{OperationId, SetupId};
use openchuck_sim::{
    Checkpoint, Deviation, DeviationOptions, DexelStock, GroupResult, RapidCut, Simulator,
    compare_classified,
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
    /// The setup's ID: names are not unique.
    pub id: SetupId,
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
    /// Playback's checkpoints, kept while simulating when the user asks for
    /// it ("Generate playback information with toolpath generation",
    /// decision record 0333): the stock at the start, inside the operations
    /// and after each one. Empty when none were kept.
    pub checkpoints: Vec<Checkpoint>,
}

/// The result of a simulation run (SIM-03, SIM-05, SIM-07).
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationReport {
    /// The settings used.
    pub settings: SimulationSettings,
    /// Each setup that has operations, in the order its first operation runs.
    pub setups: Vec<SetupSimulation>,
}

/// One setup's simulator while the pipeline runs. It keeps no checkpoints
/// unless the user asks for playback's (decision record 0333): the pipeline
/// needs only the stock each operation leaves. A copy of it after an
/// operation is what the pipeline cache keeps as the stock that operation
/// leaves (decision record 0190); the checkpoints are shared, not copied.
#[derive(Clone)]
pub(crate) struct SetupState {
    pub(crate) id: SetupId,
    simulator: Simulator,
    name: String,
    initial_volume: f64,
    operations: Vec<OperationSimulation>,
    /// The operation each entry of `operations` simulated: names are not
    /// cache inputs, so a state taken from the cache is named again from the
    /// project it is reported in.
    operation_ids: Vec<OperationId>,
    /// The stock compared with the setup's part, once a run has compared it.
    /// The cache shares this state under the key of the operation that left
    /// it, and that key fixes the part, its held regions and tabs and the
    /// cell as well as the stock, so a later run that ends the setup on the
    /// same state takes the comparison from here (OI-627). Simulating on
    /// clears it.
    compared: Arc<OnceLock<Deviation>>,
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
        checkpoints: bool,
    ) -> Result<Self, AppError> {
        settings.validate()?;
        let stock = DexelStock::from_stock(stock, Some(part_bounds), settings.cell.0)?;
        let initial_volume = stock.volume();
        let simulator = if checkpoints {
            Simulator::new(stock, machine, None)
        } else {
            Simulator::without_checkpoints(stock, machine)
        };
        Ok(Self {
            id,
            initial_volume,
            simulator,
            name: name.to_owned(),
            operations: Vec::new(),
            operation_ids: Vec::new(),
            compared: Arc::default(),
        })
    }

    /// The material left so far.
    pub(crate) fn stock(&self) -> &DexelStock {
        self.simulator.stock()
    }

    /// The memory its checkpoints hold.
    pub(crate) fn checkpoint_bytes(&self) -> usize {
        self.simulator.checkpoint_bytes()
    }

    /// Spaces the next operation's checkpoints `every` moves apart, kept
    /// while they hold no more than `budget` bytes in all (decision record
    /// 0333). Nothing for a state that keeps no checkpoints.
    pub(crate) fn space_checkpoints(&mut self, every: Option<usize>, budget: usize) {
        self.simulator.set_checkpoint_every(every);
        self.simulator.set_checkpoint_budget(Some(budget));
    }

    /// Simulates an operation's moves.
    pub(crate) fn run(
        &mut self,
        id: OperationId,
        operation: &str,
        group: &ToolpathGroup,
        tool: &RevolvedProfile,
        context: &JobContext<'_>,
    ) -> Result<GroupResult, AppError> {
        // The stock changes, so a comparison of the old one no longer holds;
        // a copy made to simulate on must not share it with the cache either.
        self.compared = Arc::default();
        let before = self.simulator.stock().volume();
        let result = self.simulator.run_group(group, tool, context)?;
        self.operation_ids.push(id);
        self.operations.push(OperationSimulation {
            operation: operation.to_owned(),
            moves: result.moves,
            poses: result.poses,
            // From the stock, so it is the same however the moves were
            // batched (checkpoints end a batch; decision record 0333).
            removed_volume: before - self.simulator.stock().volume(),
            rapid_cuts: result.rapid_cuts.clone(),
        });
        Ok(result)
    }

    /// The rapid moves that removed material in the last operation simulated.
    pub(crate) fn last_rapid_cuts(&self) -> &[RapidCut] {
        self.operations
            .last()
            .map_or(&[], |o| o.rapid_cuts.as_slice())
    }

    /// Takes the setup's and its operations' names from `project`, for a
    /// state that came from the cache (names are not cache inputs, issue 209).
    /// A name the project no longer has is left as it was.
    pub(crate) fn rename(&mut self, project: &Project) {
        if let Some(setup) = project.setup(self.id) {
            self.name.clone_from(&setup.name);
        }
        for (simulated, id) in self.operations.iter_mut().zip(&self.operation_ids) {
            if let Some(operation) = project.operation(*id) {
                simulated.operation.clone_from(&operation.name);
            }
        }
    }

    /// The stock compared with `part` (SIM-05): the comparison this state
    /// holds, or a new one. `part` is called only when the state holds none,
    /// and gives `None` when the setup has no part. The second value is true
    /// when the stock was compared in this call; the pipeline then keeps the
    /// comparison with [`SetupState::keep_comparison`] once the run is known
    /// to have read the part files it was keyed on.
    pub(crate) fn compare(
        &self,
        part: impl FnOnce() -> Result<Option<crate::workholding::DeviationTarget>, AppError>,
        settings: SimulationSettings,
        context: &JobContext<'_>,
    ) -> Result<(Option<Deviation>, bool), AppError> {
        if let Some(deviation) = self.compared.get() {
            return Ok((Some(deviation.clone()), false));
        }
        let Some(target) = part()? else {
            return Ok((None, false));
        };
        let deviation = compare_classified(
            self.stock(),
            &target.mesh,
            &DeviationOptions::for_cell(settings.cell.0),
            &|t, p, n| target.classify(t, p, n),
            context,
        )?;
        Ok((Some(deviation), true))
    }

    /// Keeps `deviation` as this stock's comparison with the setup's part,
    /// for the runs that take this state from the cache (OI-627). A state
    /// that already holds one keeps it: both are made from the same inputs
    /// (NFR-06).
    pub(crate) fn keep_comparison(&self, deviation: Deviation) {
        let _ = self.compared.set(deviation);
    }

    /// Finishes the setup's report with its comparison with the part, from
    /// [`SetupState::compare`]. The stock moves into the report; nothing is
    /// copied.
    pub(crate) fn finish(self, deviation: Option<Deviation>) -> SetupSimulation {
        let checkpoints = self.simulator.checkpoints().to_vec();
        let mut stock = self.simulator.into_stock();
        // The report holds the stock; without the slack cutting left, it holds
        // and reports the same memory whether the stock was simulated in this
        // run or copied from the cache (NFR-06; decision record 0190).
        stock.shrink_to_fit();
        SetupSimulation {
            id: self.id,
            setup: self.name,
            initial_volume: self.initial_volume,
            final_volume: stock.volume(),
            columns: stock.columns(),
            memory_bytes: stock.memory_bytes(),
            operations: self.operations,
            deviation,
            stock,
            checkpoints,
        }
    }
}

impl SetupSimulation {
    /// The stock left, as a closed mesh for the viewport (SIM-03, SIM-05).
    /// Quads do not share vertices, so the mesh is blocky at the cell size.
    pub fn stock_mesh(&self) -> openchuck_types::mesh::TriangleMesh {
        openchuck_sim::stock_mesh(&self.stock)
    }
}
