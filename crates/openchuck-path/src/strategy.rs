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

use crate::assemble::{GeneratedPath, PathNote, assemble_with_entry};
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

/// A strategy's passes with what it found worth telling the user, as
/// sentences (decision record 0033 item 9). The notes are added to the
/// generated path's notes, against its first segment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StrategyOutput {
    /// The passes, as [`Strategy::passes`] gives them.
    pub passes: Vec<Pass>,
    /// Notes about the whole toolpath.
    pub notes: Vec<String>,
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

    /// The passes, limited to `region` when given, with notes for the user.
    /// The default runs [`passes`](Self::passes) or
    /// [`passes_in_region`](Self::passes_in_region) and has no notes.
    fn passes_with_notes(
        &self,
        input: &StrategyInput<'_>,
        region: Option<&RegionConstraint<'_>>,
        parameters: &Parameters,
        context: &JobContext<'_>,
    ) -> Result<StrategyOutput> {
        let passes = match region {
            Some(region) => self.passes_in_region(input, region, parameters, context)?,
            None => self.passes(input, parameters, context)?,
        };
        Ok(StrategyOutput {
            passes,
            notes: Vec::new(),
        })
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
    generate_with(strategy, input, None, values, context)
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
    generate_with(strategy, input, Some(region), values, context)
}

fn generate_with(
    strategy: &dyn Strategy,
    input: &StrategyInput<'_>,
    region: Option<&RegionConstraint<'_>>,
    values: &BTreeMap<String, ParameterValue>,
    context: &JobContext<'_>,
) -> Result<GeneratedPath> {
    let parameters = strategy.schema().resolve(values)?;
    let common = CommonParameters::read(&parameters)?;
    let entry = EntryParameters::read(&parameters)?;
    context.check_cancelled()?;
    let output = strategy.passes_with_notes(input, region, &parameters, context)?;
    context.check_cancelled()?;
    let mut path = assemble_with_entry(&output.passes, input, &common, &entry, context)?;
    if !output.notes.is_empty() {
        let mut notes: Vec<PathNote> = output
            .notes
            .into_iter()
            .map(|message| PathNote {
                segment: 0,
                message,
            })
            .collect();
        notes.append(&mut path.notes);
        path.notes = notes;
    }
    Ok(path)
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

    /// The strategies this library provides: helical finishing (OP-04) and
    /// simultaneous 4-axis finishing (OP-05, OP-06).
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        // The names are fixed and distinct, so these cannot fail.
        let _ = registry.register(Arc::new(crate::helical::HelicalFinishing));
        let _ = registry.register(Arc::new(crate::simultaneous::SimultaneousFinishing));
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
