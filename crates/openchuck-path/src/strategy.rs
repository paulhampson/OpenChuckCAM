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

/// The material as it is when a pass starts, for a strategy that removes
/// material in stages (rotary roughing, decision record 0033). The shared
/// services use it for the pass's entry, its leave and the link from the
/// pass before it (OP-14), in place of the setup's stock, which does not
/// know what the operation itself has already cut.
#[derive(Clone)]
pub struct PassStock(pub Arc<dyn StockModel>);

impl std::fmt::Debug for PassStock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl PartialEq for PassStock {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// A run of cut points cut without lifting the tool.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pass {
    /// The points, in cutting order.
    pub points: Vec<CutPoint>,
    /// The material when this pass starts, if the strategy tracks it. Passes
    /// that have it are linked without a retract to the clearance cylinder
    /// where the link is clear of it (decision record 0033 item 6).
    pub stock: Option<PassStock>,
    /// Indices into `points` of the points reached by a rapid move instead of
    /// at the cutting feed: the retract and the return of a peck in a
    /// drilled hole (OP-10; decision record 0051 item 6). Point 0 is where
    /// the entry ends, so an index of 0 has no move. A pass with rapid points
    /// keeps its contact points as given instead of smoothing them (the path
    /// doubles back on itself).
    pub rapids: Vec<usize>,
}

/// A strategy's passes with what it found worth telling the user, as
/// sentences (decision record 0036 item 9). The notes are added to the
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
    /// Model X intervals, as (low, high), that the tool's tip never enters
    /// while it cuts: each held region of the setup widened on both sides by
    /// the tool's largest radius, so no part of the tool reaches over the
    /// gripped length (SET-09; decision record 0161). The tool is always
    /// across the rotary axis, so its body spans its tip's X plus and minus
    /// that radius. Passes are cut short where they would enter one, and a
    /// path whose entry, exit or link at feed still would is refused. Empty
    /// for a setup with no held region.
    pub keep_out: &'a [(f64, f64)],
}

impl std::fmt::Debug for StrategyInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrategyInput")
            .field("surface", &self.surface)
            .field("stock", &self.stock)
            .field("rotary", &self.rotary)
            .field("clearance", &self.clearance)
            .field("tags", self.tags)
            .field("keep_out", &self.keep_out)
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

    /// Whether the strategy works on the material earlier operations left
    /// (rest machining, pencil clean-up; OP-09). The host says so in the
    /// operation's notes when no earlier operation of the setup has made the
    /// stock it is given, because the strategy then sees the setup's own stock.
    fn needs_earlier_operations(&self) -> bool {
        false
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
    let (passes, notes): (Vec<Pass>, Vec<String>) = if zones.is_empty() {
        let output = strategy.passes_with_notes(input, region, &parameters, context)?;
        (output.passes, output.notes)
    } else {
        let most = zones
            .iter()
            .fold(common.stock_to_leave.0, |m, z| m.max(z.leave));
        common.stock_to_leave = openchuck_types::units::Millimetres(most);
        let passes = strategy.passes_in_zones(input, region, zones, &parameters, context)?;
        (passes, Vec::new())
    };
    context.check_cancelled()?;
    let (passes, notes) = keep_clear(passes, notes, input.keep_out)?;
    let mut path = assemble_with_entry(&passes, input, &common, &entry, context)?;
    check_clear(&path, input.keep_out)?;
    if !notes.is_empty() {
        let mut notes: Vec<PathNote> = notes
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

/// How far a position may lie inside a keep-out interval and still count as
/// clear: rounding in the strategies' positions, far below the written
/// resolution (millimetres).
const KEEP_OUT_SLACK: f64 = 1e-6;

/// True when a move whose tip spans `lo..=hi` along X reaches into one of
/// `keep_out`.
fn reaches_in(keep_out: &[(f64, f64)], lo: f64, hi: f64) -> bool {
    keep_out
        .iter()
        .any(|&(a, b)| hi > a + KEEP_OUT_SLACK && lo < b - KEEP_OUT_SLACK)
}

/// The passes with every stretch that reaches into `keep_out` left out
/// (SET-09; decision record 0161): a pass is cut into the runs of points
/// that, with the moves between them, stay clear, and a run of one point
/// from a longer pass is dropped. A pass with rapid points (a drilled hole)
/// is kept whole or dropped whole. A note says what was left out; if nothing
/// is left of passes that cut, the operation is refused.
fn keep_clear(
    passes: Vec<Pass>,
    mut notes: Vec<String>,
    keep_out: &[(f64, f64)],
) -> Result<(Vec<Pass>, Vec<String>)> {
    if keep_out.is_empty() || passes.is_empty() {
        return Ok((passes, notes));
    }
    let clear = |p: &CutPoint| !reaches_in(keep_out, p.tip.x, p.tip.x);
    let move_clear = |a: &CutPoint, b: &CutPoint| {
        !reaches_in(keep_out, a.tip.x.min(b.tip.x), a.tip.x.max(b.tip.x))
    };
    let (mut kept, mut cut_short, mut holes) = (Vec::new(), false, 0usize);
    for pass in passes {
        if !pass.rapids.is_empty() {
            let whole = pass.points.iter().all(clear)
                && pass.points.windows(2).all(|w| match w {
                    [a, b] => move_clear(a, b),
                    _ => true,
                });
            if whole {
                kept.push(pass);
            } else {
                holes += 1;
            }
            continue;
        }
        let single = pass.points.len() == 1;
        let mut run: Vec<CutPoint> = Vec::new();
        let mut runs: Vec<Vec<CutPoint>> = Vec::new();
        for point in &pass.points {
            let joins = run.last().is_some_and(|last| move_clear(last, point));
            if !joins && !run.is_empty() {
                runs.push(std::mem::take(&mut run));
            }
            if clear(point) {
                run.push(*point);
            }
        }
        runs.push(run);
        let pieces: Vec<Vec<CutPoint>> = runs
            .into_iter()
            .filter(|r| r.len() >= 2 || (single && r.len() == 1))
            .collect();
        let left: usize = pieces.iter().map(Vec::len).sum();
        if left < pass.points.len() || pieces.len() > 1 {
            cut_short = true;
        }
        for points in pieces {
            kept.push(Pass {
                points,
                stock: pass.stock.clone(),
                rapids: Vec::new(),
            });
        }
    }
    if kept.is_empty() {
        return Err(PathError::Strategy(
            "every cut of this operation would reach over a held region, where the workholding \
             grips the stock, so nothing is left to cut"
                .to_owned(),
        ));
    }
    if cut_short {
        notes.push(
            "Nothing within one tool radius of a held region is cut: no part of the tool \
             reaches over the gripped length of the stock."
                .to_owned(),
        );
    }
    if holes > 0 {
        notes.push(format!(
            "{holes} {} not drilled: the tool would reach over a held region.",
            if holes == 1 { "hole is" } else { "holes are" }
        ));
    }
    Ok((kept, notes))
}

/// Refuses an assembled path with a move at feed that reaches into
/// `keep_out` (an entry, an exit or a link the passes alone did not show),
/// so such a path is never written (SET-09, NFR-12; decision record 0161).
fn check_clear(path: &GeneratedPath, keep_out: &[(f64, f64)]) -> Result<()> {
    if keep_out.is_empty() {
        return Ok(());
    }
    let mut from = path.group.start.tip;
    for segment in &path.group.segments {
        let to = segment.end.tip;
        if segment.kind.is_fed() && reaches_in(keep_out, from.x.min(to.x), from.x.max(to.x)) {
            return Err(PathError::Strategy(format!(
                "a move at X {:.3} mm would reach over a held region, where the workholding \
                 grips the stock; change the entry or the link settings",
                to.x
            )));
        }
        from = to;
    }
    Ok(())
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

    /// The strategies this library provides: helical, axial and indexed
    /// finishing (OP-04), rotary roughing (OP-03) and simultaneous 4-axis
    /// finishing (OP-05, OP-06), and rest machining and pencil clean-up
    /// (OP-09), radial drilling (OP-10), edge and curve following (OP-08)
    /// and swarf (OP-07).
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        // The names are fixed and distinct, so these cannot fail.
        let _ = registry.register(Arc::new(crate::helical::HelicalFinishing));
        let _ = registry.register(Arc::new(crate::axial::AxialFinishing));
        let _ = registry.register(Arc::new(crate::indexed::IndexedFinishing));
        let _ = registry.register(Arc::new(crate::roughing::RotaryRoughing));
        let _ = registry.register(Arc::new(crate::simultaneous::SimultaneousFinishing));
        let _ = registry.register(Arc::new(crate::rest::RestMachining));
        let _ = registry.register(Arc::new(crate::pencil::PencilFinishing));
        let _ = registry.register(Arc::new(crate::drill::RadialDrilling));
        let _ = registry.register(Arc::new(crate::edge_following::EdgeFollowing::default()));
        let _ = registry.register(Arc::new(crate::swarf::Swarf));
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use openchuck_types::geometry::UnitVec3;
    use openchuck_types::ir::{MoveKind, PathTags, Segment, ToolPose, ToolpathGroup};
    use openchuck_types::units::Seconds;

    use super::*;

    fn point(x: f64) -> CutPoint {
        CutPoint {
            tip: Point3::new(x, 0.0, 12.0),
            a: Degrees(0.0),
            contact: Point3::new(x, 0.0, 10.0),
        }
    }

    fn pass(xs: &[f64]) -> Pass {
        Pass {
            points: xs.iter().map(|&x| point(x)).collect(),
            stock: None,
            rapids: Vec::new(),
        }
    }

    fn xs(p: &Pass) -> Vec<f64> {
        p.points.iter().map(|c| c.tip.x).collect()
    }

    /// A held region at X -12 to -8 seen by a tool of radius 1.5.
    const KEEP_OUT: [(f64, f64); 1] = [(-13.5, -6.5)];

    #[test]
    fn req_set_09_a_pass_is_cut_short_where_it_would_reach_over_a_held_region() {
        // Into the keep-out and out again: two runs, the point inside and
        // the one-point run beside it dropped.
        let (passes, notes) = keep_clear(
            vec![
                pass(&[-20.0, -16.0, -14.0, -10.0, -6.5, -4.0, 0.0]),
                pass(&[0.0, 10.0]),
                pass(&[-11.0, -7.0, -2.0]),
            ],
            vec!["first".to_owned()],
            &KEEP_OUT,
        )
        .unwrap();
        let runs: Vec<Vec<f64>> = passes.iter().map(xs).collect();
        assert_eq!(
            runs,
            vec![
                vec![-20.0, -16.0, -14.0],
                vec![-6.5, -4.0, 0.0],
                vec![0.0, 10.0]
            ]
        );
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0], "first");
        assert!(notes[1].contains("held region"), "{}", notes[1]);
        // A move that jumps over the whole keep-out is not kept either.
        let (passes, _) =
            keep_clear(vec![pass(&[-20.0, -16.0, 0.0, 4.0])], Vec::new(), &KEEP_OUT).unwrap();
        assert_eq!(
            passes.iter().map(xs).collect::<Vec<_>>(),
            vec![vec![-20.0, -16.0], vec![0.0, 4.0]]
        );
    }

    #[test]
    fn req_set_09_nothing_changes_without_a_held_region_or_when_clear() {
        let given = vec![pass(&[-20.0, -16.0]), pass(&[0.0, 10.0])];
        let (same, notes) = keep_clear(given.clone(), Vec::new(), &[]).unwrap();
        assert_eq!(same, given);
        assert!(notes.is_empty());
        let (same, notes) = keep_clear(vec![pass(&[0.0, 10.0])], Vec::new(), &KEEP_OUT).unwrap();
        assert_eq!(same, vec![pass(&[0.0, 10.0])]);
        assert!(notes.is_empty());
        // A pass of one point that is clear stays.
        let (same, _) = keep_clear(vec![pass(&[2.0])], Vec::new(), &KEEP_OUT).unwrap();
        assert_eq!(same, vec![pass(&[2.0])]);
    }

    #[test]
    fn req_set_09_req_op_10_a_drilled_hole_over_a_held_region_is_dropped_whole() {
        let mut hole = pass(&[-10.0, -10.0, -10.0]);
        hole.rapids = vec![2];
        let mut clear = pass(&[5.0, 5.0, 5.0]);
        clear.rapids = vec![2];
        let (passes, notes) = keep_clear(vec![hole, clear.clone()], Vec::new(), &KEEP_OUT).unwrap();
        assert_eq!(passes, vec![clear]);
        assert_eq!(
            notes,
            vec!["1 hole is not drilled: the tool would reach over a held region."]
        );
    }

    #[test]
    fn req_set_09_req_nfr_07_an_operation_wholly_over_a_held_region_is_refused() {
        let error = keep_clear(vec![pass(&[-12.0, -9.0])], Vec::new(), &KEEP_OUT).unwrap_err();
        assert!(
            error.to_string().contains("nothing is left to cut"),
            "{error}"
        );
    }

    fn pose(x: f64) -> ToolPose {
        ToolPose {
            tip: Point3::new(x, 0.0, 12.0),
            axis: UnitVec3::Z,
            a: Degrees(0.0),
        }
    }

    fn path(moves: &[(MoveKind, f64)]) -> GeneratedPath {
        GeneratedPath {
            group: ToolpathGroup {
                tags: PathTags {
                    operation: "op".into(),
                    setup: "setup".into(),
                    tool: "tool".into(),
                },
                start: pose(0.0),
                segments: moves
                    .iter()
                    .map(|&(kind, x)| Segment {
                        kind,
                        end: pose(x),
                        contact_feed: None,
                        duration: Seconds(0.0),
                        contact_length: openchuck_types::units::Millimetres(0.0),
                    })
                    .collect(),
            },
            notes: Vec::new(),
        }
    }

    #[test]
    fn req_set_09_req_nfr_12_an_assembled_move_at_feed_that_reaches_in_refuses_the_path() {
        // A rapid over the held region (at the clearance height) is allowed.
        let fine = path(&[
            (MoveKind::Cutting, -2.0),
            (MoveKind::Rapid, -20.0),
            (MoveKind::Cutting, -16.0),
        ]);
        check_clear(&fine, &KEEP_OUT).unwrap();
        // An entry that runs into it is not.
        let entry = path(&[(MoveKind::LeadIn, -7.0), (MoveKind::Cutting, -2.0)]);
        let error = check_clear(&entry, &KEEP_OUT).unwrap_err();
        assert!(error.to_string().contains("X -7.000"), "{error}");
        // A link at feed that crosses it is not either.
        let link = path(&[(MoveKind::Linking, -20.0)]);
        assert!(check_clear(&link, &KEEP_OUT).is_err());
        check_clear(&link, &[]).unwrap();
    }
}
