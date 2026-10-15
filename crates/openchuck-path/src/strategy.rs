// SPDX-License-Identifier: AGPL-3.0-or-later
//! The `Strategy` interface and registry (OP-16; architecture section 5.2;
//! decision record 0018 item 6).
//!
//! A strategy turns its inputs into **passes**: ordered cut points, each a
//! tool tip, an A angle and the cutter contact point. Everything a pass needs
//! after that is a shared service every strategy gets the same way
//! ([`crate::assemble`]): A-axis management (OP-15), linking and retracts
//! through the clearance geometry (OP-14), and the contact feed and segment
//! durations written into the IR (OP-13). A new strategy implements
//! [`Strategy`] and is added to a [`Registry`]; nothing else changes.

use std::collections::BTreeMap;
use std::sync::Arc;

use openchuck_types::geometry::Point3;
use openchuck_types::ir::PathTags;
use openchuck_types::job::JobContext;
use openchuck_types::machine::{Clearance, MaxFeeds};
use openchuck_types::tool::profile::RevolvedProfile;
use openchuck_types::units::Degrees;

use crate::assemble::{GeneratedPath, assemble_with_entry};
use crate::error::{PathError, Result};
use crate::params::{
    CommonParameters, EntryParameters, ParameterSchema, ParameterValue, Parameters,
};
use crate::region::RegionConstraint;
use crate::rotary::RotaryMapping;
use crate::stock::{CollisionOracle, StockModel};
use crate::surface::SurfaceSource;

/// One point of a pass, in the model frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutPoint {
    /// The tool tip.
    pub tip: Point3,
    /// The rotary angle. Any equivalent by whole turns will do: the assembly
    /// makes A continuous and fits it to the machine's range (OP-15).
    pub a: Degrees,
    /// The cutter contact point, where the feed applies (OP-13).
    pub contact: Point3,
}

/// A run of cut points cut without lifting the tool.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pass {
    /// The points, in cutting order.
    pub points: Vec<CutPoint>,
}

/// Everything a strategy is given, as plain data and ports (INT-02).
#[derive(Clone, Copy)]
pub struct StrategyInput<'a> {
    /// The surfaces to machine.
    pub surface: &'a dyn SurfaceSource,
    /// The material still to be cut.
    pub stock: &'a dyn StockModel,
    /// Collision queries for links.
    pub collision: &'a dyn CollisionOracle,
    /// The tool's profile (decision record 0012).
    pub tool: &'a RevolvedProfile,
    /// The rotary axis.
    pub rotary: RotaryMapping,
    /// The setup's clearance geometry (SET-06).
    pub clearance: Clearance,
    /// The machine's feed limits.
    pub max_feeds: MaxFeeds,
    /// Tags for the toolpath group.
    pub tags: &'a PathTags,
}

impl std::fmt::Debug for StrategyInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrategyInput")
            .field("surface", &self.surface)
            .field("stock", &self.stock)
            .field("rotary", &self.rotary)
            .field("clearance", &self.clearance)
            .field("tags", self.tags)
            .finish_non_exhaustive()
    }
}

/// A part of the surface that is left its own stock to leave in place of the
/// operation's (OP-11 per selection; decision record 0034 item 4). The
/// zones given to a strategy partition the part: where they overlap the
/// largest amount applies, so the host assigns each triangle to one.
#[derive(Clone, Copy)]
pub struct LeaveZone<'a> {
    /// The part of the surface.
    pub surface: &'a dyn SurfaceSource,
    /// The stock to leave on it, in millimetres.
    pub leave: f64,
}

impl std::fmt::Debug for LeaveZone<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LeaveZone")
            .field("leave", &self.leave)
            .finish_non_exhaustive()
    }
}

/// A machining strategy (OP-16).
pub trait Strategy: Send + Sync + std::fmt::Debug {
    /// The registry name an operation stores (`helical_finishing`). Stable:
    /// renaming it breaks saved projects.
    fn name(&self) -> &str;

    /// The name shown to the user.
    fn label(&self) -> &str;

    /// The parameters, including the common ones
    /// ([`ParameterSchema::with_common`]).
    fn schema(&self) -> ParameterSchema;

    /// Things to tell the user about `parameters` for `tool` that do not stop
    /// the operation: a setting the strategy cannot use with this tool and
    /// ignores, for example. The host shows them with the operation's notes.
    fn parameter_warnings(&self, _tool: &RevolvedProfile, _parameters: &Parameters) -> Vec<String> {
        Vec::new()
    }

    /// Whether the strategy limits its cuts to a region
    /// ([`passes_in_region`](Self::passes_in_region), OP-02). A strategy that
    /// does not is refused for an operation with selections, rather than
    /// cutting outside them.
    fn honours_region(&self) -> bool {
        false
    }

    /// As [`passes`](Self::passes), cutting only where `region` allows
    /// (OP-02). The default refuses, so a strategy that has not been written
    /// for regions never cuts outside one.
    fn passes_in_region(
        &self,
        _input: &StrategyInput<'_>,
        _region: &RegionConstraint<'_>,
        _parameters: &Parameters,
        _context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Err(PathError::Strategy(format!(
            "the strategy '{}' cannot be limited to a region",
            self.name()
        )))
    }

    /// Whether the strategy leaves each zone its own stock to leave
    /// ([`passes_in_zones`](Self::passes_in_zones)). A strategy that does not
    /// is refused for zones, rather than leaving the wrong amount.
    fn honours_leave_zones(&self) -> bool {
        false
    }

    /// As [`passes_in_region`](Self::passes_in_region), or as
    /// [`passes`](Self::passes) without a region, leaving each of `zones` its
    /// own stock to leave. The default refuses.
    fn passes_in_zones(
        &self,
        _input: &StrategyInput<'_>,
        _region: Option<&RegionConstraint<'_>>,
        _zones: &[LeaveZone<'_>],
        _parameters: &Parameters,
        _context: &JobContext<'_>,
    ) -> Result<Vec<Pass>> {
        Err(PathError::Strategy(format!(
            "the strategy '{}' cannot leave different stock on different selections",
            self.name()
        )))
    }

    /// The passes for `input` and the resolved `parameters`. Long work
    /// checks `context` for cancellation and reports progress through it.
    fn passes(
        &self,
        input: &StrategyInput<'_>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<Vec<Pass>>;
}

/// Runs `strategy` and assembles its passes into a toolpath group: checks
/// the parameter values against its schema, makes the passes, then applies
/// the shared services (A-axis management, linking, timing).
pub fn generate(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<GeneratedPath> {
    generate_with(strategy, input, None, &[], values, context)
}

/// As [`generate`], cutting only where `region` allows (OP-02). Fails for a
/// strategy that does not honour regions.
pub fn generate_in_region(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    region: &RegionConstraint<'_>,
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<GeneratedPath> {
    generate_with(strategy, input, Some(region), &[], values, context)
}

/// As [`generate`] or [`generate_in_region`], leaving each of `zones` its own
/// stock to leave (OP-11 per selection; decision record 0034 item 4). The
/// zones cover the part; the operation's own value is only the default for a
/// host that gives none. The entry checks use the largest amount, so an entry
/// never comes closer to any part than its stock to leave. Fails for a
/// strategy that does not honour zones, or for a negative or non-finite
/// amount.
pub fn generate_in_zones(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<GeneratedPath> {
    generate_with(strategy, input, region, zones, values, context)
}

fn generate_with(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    zones: &[LeaveZone<'_>],
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<GeneratedPath> {
    let parameters = strategy.schema().resolve(values)?;
    let mut common = CommonParameters::read(&parameters)?;
    let entry = EntryParameters::read(&parameters)?;
    if let Some(bad) = zones.iter().find(|z| !z.leave.is_finite() || z.leave < 0.0) {
        return Err(PathError::Strategy(format!(
            "a stock to leave of {} mm is not zero or more",
            bad.leave
        )));
    }
    context.check_cancelled()?;
    let passes = if zones.is_empty() {
        match region {
            Some(region) => strategy.passes_in_region(input, region, &parameters, context)?,
            None => strategy.passes(input, &parameters, context)?,
        }
    } else {
        let most = zones
            .iter()
            .fold(common.stock_to_leave.0, |m, z| m.max(z.leave));
        common.stock_to_leave = openchuck_types::units::Millimetres(most);
        strategy.passes_in_zones(input, region, zones, &parameters, context)?
    };
    context.check_cancelled()?;
    assemble_with_entry(&passes, input, &common, &entry, context)
}

/// The strategies a host offers, by name.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    strategies: BTreeMap<String, Arc<dyn Strategy>>,
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// The strategies this library provides: helical, axial and indexed finishing (OP-04).
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        // The name is fixed and the registry is empty, so this cannot fail.
        let _ = registry.register(Arc::new(crate::helical::HelicalFinishing));
        let _ = registry.register(Arc::new(crate::axial::AxialFinishing));
        let _ = registry.register(Arc::new(crate::indexed::IndexedFinishing));
        registry
    }

    /// Adds `strategy`. Fails if a strategy of the same name is registered.
    pub fn register(&mut self, strategy: Arc<dyn Strategy>) -> Result<()> {
        let name = strategy.name().to_owned();
        if name.trim().is_empty() {
            return Err(PathError::Strategy("a strategy needs a name".to_owned()));
        }
        if self.strategies.contains_key(&name) {
            return Err(PathError::Strategy(format!(
                "a strategy named \"{name}\" is already registered"
            )));
        }
        self.strategies.insert(name, strategy);
        Ok(())
    }

    /// The strategy called `name`.
    pub fn get(&self, name: &str) -> Result<&dyn Strategy> {
        self.strategies
            .get(name)
            .map(|s| s.as_ref())
            .ok_or_else(|| PathError::UnknownStrategy(name.to_owned()))
    }

    /// Every registered strategy, in name order.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Strategy> {
        self.strategies.values().map(|s| s.as_ref())
    }
}
