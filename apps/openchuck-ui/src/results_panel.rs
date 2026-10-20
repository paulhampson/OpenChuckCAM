// SPDX-License-Identifier: AGPL-3.0-or-later
//! The results inspector of the Simulate step (SIM-05, SIM-07; ui-specification
//! section 11.5; slice 22 lane X6, decision records 0114 C17 and 0125).
//!
//! It replaces the floating "Results" window's simulation lines with what the
//! specification lists: colour by Deviation, Remaining or Overcut, the worst
//! values, the share within tolerance, a deviation histogram in the colours of
//! the view, a table by operation and the resolution control with its cost.
//! The numbers come from [`summarise`], a plain function over the run, so they
//! are tested without a window.

use std::sync::Arc;

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};
use openchuck_app::pathview::DeviationBand;
use openchuck_app::setup::Millimetres;
use openchuck_app::sim::Deviation;
use openchuck_app::simulation::{COARSEST_CELL, DEFAULT_CELL, FINEST_CELL, SimulationSettings};
use openchuck_app::{Application, Command, Regeneration};

use crate::roles;
use crate::theme::DARK;

/// What the part is coloured by (SIM-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColourBy {
    /// Overcut and material left (the default).
    #[default]
    Deviation,
    /// Material left only; overcut shows as on the surface.
    Remaining,
    /// Overcut only; material left shows as on the surface.
    Overcut,
}

impl ColourBy {
    /// Every choice, in the order of the segmented control.
    pub const ALL: [Self; 3] = [Self::Deviation, Self::Remaining, Self::Overcut];

    /// The label of the choice.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Deviation => "Deviation",
            Self::Remaining => "Remaining",
            Self::Overcut => "Overcut",
        }
    }

    /// Whether `band` keeps its own colour under this choice; the others are
    /// drawn as on the surface.
    pub const fn shows(self, band: DeviationBand) -> bool {
        match self {
            Self::Deviation => true,
            Self::Remaining => !matches!(band, DeviationBand::Overcut),
            Self::Overcut => matches!(band, DeviationBand::Overcut | DeviationBand::Unmeasured),
        }
    }
}

/// The probe's text for a triangle of the part (SIM-11 in part; issue 208,
/// decision record 0162): the value the triangle is coloured by, for the
/// view chosen, in millimetres at three decimals, with no position. `value`
/// is the triangle's worst sampled deviation (positive for material left)
/// and whether it is only a bound of the search, or `None` where the
/// triangle has no sample.
pub fn probe_text(by: ColourBy, value: Option<(f64, bool)>, tolerance: f64) -> String {
    let Some((v, bound)) = value else {
        return "Not measured".to_owned();
    };
    let more = if bound { " or more" } else { "" };
    let within = |d: f64| {
        if d <= tolerance {
            format!(", within ±{tolerance:.3} mm")
        } else {
            String::new()
        }
    };
    let overcut = || format!("Overcut {:.3} mm{more}{}", -v, within(-v));
    let left = || format!("Material left {v:.3} mm{more}{}", within(v));
    match by {
        ColourBy::Deviation if v < 0.0 => overcut(),
        ColourBy::Deviation if v > 0.0 => left(),
        ColourBy::Deviation => "On the surface, 0.000 mm".to_owned(),
        ColourBy::Remaining if v > 0.0 => left(),
        ColourBy::Remaining => "No material left".to_owned(),
        ColourBy::Overcut if v < 0.0 => overcut(),
        ColourBy::Overcut => "No overcut".to_owned(),
    }
}

/// The fixed bin edges of the histogram, millimetres: the thresholds of the
/// part's colour bands (`DeviationBand`), mirrored below the surface for the
/// overcut. Only the edges beyond the tolerance and within the extremes are
/// used (decision record 0162).
pub const HIST_EDGES: [f64; 6] = [-1.0, -0.5, -0.25, 0.25, 0.5, 1.0];

/// How many separate overcut places the run lists at most
/// (`DeviationOptions::for_cell`).
const MAX_PLACES: usize = 10;

/// One bin of the deviation histogram (SIM-05; decision record 0162).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bin {
    /// The low end, millimetres (negative is cut below the surface). The
    /// first bin's low end is the deepest sample, so the histogram always
    /// reaches the largest overcut.
    pub low: f64,
    /// The high end, millimetres. The last bin's high end is the largest
    /// sample, so the histogram always reaches the most material left.
    pub high: f64,
    /// Samples in the bin. A bin below the surface band holds `low <= v <
    /// high`, the surface band `-tolerance <= v <= tolerance`, and a bin above
    /// it `low < v <= high`, as the colour bands are bounded.
    pub count: usize,
    /// The colour band the bin is drawn in.
    pub band: DeviationBand,
}

/// One operation's row of the table.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationRow {
    /// The operation's name.
    pub name: String,
    /// Moves simulated.
    pub moves: usize,
    /// Volume removed, cubic millimetres.
    pub removed: f64,
    /// Rapid moves that removed material (a fault).
    pub rapid_cuts: usize,
}

/// The numbers of one setup's simulation: the stock after all of the setup's
/// operations against the part (and the held regions and tabs where they
/// show), all from the same samples, so they agree with each other.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupResults {
    /// The setup's name.
    pub setup: String,
    /// The tolerance band: the cell size, the simulation's own error.
    pub tolerance: f64,
    /// The samples measured.
    pub samples: usize,
    /// The deepest cut below the surface, millimetres (positive), when any
    /// sample is cut below it, within the tolerance or not.
    pub largest_overcut: Option<f64>,
    /// Whether the deepest cut is only a bound: the search stopped there, and
    /// it may be deeper.
    pub overcut_is_bound: bool,
    /// The most material left, millimetres.
    pub largest_left: Option<f64>,
    /// Whether the most material left is only a bound: the search stopped
    /// there, and there may be more.
    pub left_is_bound: bool,
    /// The share of samples within the tolerance, 0 to 1, when there are samples.
    pub within: Option<f64>,
    /// Samples cut deeper than the tolerance.
    pub overcut_samples: usize,
    /// Separate places (2 mm apart) cut deeper than the tolerance, at most
    /// [`MAX_PLACES`].
    pub overcut_places: usize,
    /// The histogram's bins, from the deepest overcut to the most material
    /// left (see [`histogram`]).
    pub histogram: Vec<Bin>,
    /// The operations, in machining order.
    pub operations: Vec<OperationRow>,
}

impl SetupResults {
    /// The numbers of a setup from its deviation, if it has one.
    pub fn new(
        setup: String,
        tolerance: f64,
        deviation: Option<&Deviation>,
        operations: Vec<OperationRow>,
    ) -> Self {
        let mut r = Self {
            setup,
            tolerance,
            samples: 0,
            largest_overcut: None,
            overcut_is_bound: false,
            largest_left: None,
            left_is_bound: false,
            within: None,
            overcut_samples: 0,
            overcut_places: 0,
            histogram: Vec::new(),
            operations,
        };
        let Some(d) = deviation else {
            return r;
        };
        let values: Vec<f64> = d.points.iter().map(|p| p.value).collect();
        r.samples = values.len();
        r.histogram = histogram(&values, tolerance);
        if r.samples > 0 {
            let ok = values.iter().filter(|v| v.abs() <= tolerance).count();
            r.within = Some(ok as f64 / r.samples as f64);
        }
        r.overcut_samples = values.iter().filter(|v| **v < -tolerance).count();
        r.overcut_places = d.overcut_places.len();
        r.largest_overcut = d.worst_overcut.map(|p| -p.value).filter(|v| *v > 0.0);
        r.largest_left = d.worst_remaining.map(|p| p.value).filter(|v| *v > 0.0);
        r.overcut_is_bound = d.worst_overcut.is_some_and(|p| p.at_least);
        r.left_is_bound = d.worst_remaining.is_some_and(|p| p.at_least);
        r
    }
}

/// The colour band of the values between `low` and `high`, one side of the
/// surface band or within it (the thresholds of `deviation_bands`).
fn band_between(low: f64, high: f64, tolerance: f64) -> DeviationBand {
    if high <= -tolerance {
        DeviationBand::Overcut
    } else if low < tolerance {
        DeviationBand::OnTarget
    } else if high <= 0.25 {
        DeviationBand::Thin
    } else if high <= 1.0 {
        DeviationBand::Medium
    } else {
        DeviationBand::Thick
    }
}

/// The histogram of `values` (millimetres, positive for material left) for a
/// tolerance (the cell size): a bin for the surface band `±tolerance`, and
/// beyond it bins split at [`HIST_EDGES`] out to the deepest and the largest
/// value, so its range always covers both (decision record 0162). Values
/// that are not finite are not counted. Empty when no value is finite.
pub fn histogram(values: &[f64], tolerance: f64) -> Vec<Bin> {
    let finite = || values.iter().copied().filter(|v| v.is_finite());
    let (Some(min), Some(max)) = (
        finite().min_by(f64::total_cmp),
        finite().max_by(f64::total_cmp),
    ) else {
        return Vec::new();
    };
    let (low, high) = (min.min(-tolerance), max.max(tolerance));
    let mut edges = vec![low];
    edges.extend(
        HIST_EDGES
            .iter()
            .copied()
            .filter(|e| low < *e && *e < -tolerance),
    );
    if low < -tolerance {
        edges.push(-tolerance);
    }
    if high > tolerance {
        edges.push(tolerance);
    }
    edges.extend(
        HIST_EDGES
            .iter()
            .copied()
            .filter(|e| tolerance < *e && *e < high),
    );
    edges.push(high);
    let mut bins: Vec<Bin> = edges
        .iter()
        .zip(edges.iter().skip(1))
        .map(|(&low, &high)| Bin {
            low,
            high,
            count: 0,
            band: band_between(low, high, tolerance),
        })
        .collect();
    for v in finite() {
        let index = bins.iter().position(|b| {
            if b.high <= -tolerance {
                b.low <= v && v < b.high
            } else if b.low < tolerance {
                (-tolerance..=tolerance).contains(&v)
            } else {
                b.low < v && v <= b.high
            }
        });
        if let Some(bin) = index.and_then(|i| bins.get_mut(i)) {
            bin.count += 1;
        }
    }
    bins
}

/// A bin's range, millimetres at three decimals, negative below the surface (the column header
/// gives the unit). An end that is only a bound of the search is marked "≤"
/// (deeper) or "≥" (more left).
pub fn bin_range(bin: &Bin, low_is_bound: bool, high_is_bound: bool) -> String {
    let low = if low_is_bound { "≤" } else { "" };
    let high = if high_is_bound { "≥" } else { "" };
    format!("{low}{:.3} to {high}{:.3}", bin.low, bin.high)
}

/// The numbers of each setup of a simulated run; empty when the run has no
/// simulation.
pub fn summarise(run: &Regeneration) -> Vec<SetupResults> {
    let Some(simulation) = &run.simulation else {
        return Vec::new();
    };
    let tolerance = simulation.settings.cell.0;
    simulation
        .setups
        .iter()
        .map(|setup| {
            SetupResults::new(
                setup.setup.clone(),
                tolerance,
                setup.deviation.as_ref(),
                setup
                    .operations
                    .iter()
                    .map(|o| OperationRow {
                        name: o.operation.clone(),
                        moves: o.moves,
                        removed: o.removed_volume,
                        rapid_cuts: o.rapid_cuts.len(),
                    })
                    .collect(),
            )
        })
        .collect()
}

/// A sentence that states the cost of a cell size against the default
/// (SIM-07; decision 0025).
pub fn cost_line(cell: f64) -> String {
    let chosen = SimulationSettings {
        cell: Millimetres(cell),
    };
    let time = chosen.relative_time();
    let memory = chosen.relative_memory();
    if (cell - DEFAULT_CELL).abs() < 1e-12 {
        format!(
            "Default. Finest is {FINEST_CELL} mm; finer cells are slower and use more memory. \
             Deviation is accurate to about one cell."
        )
    } else {
        format!(
            "About {time:.1} times the time and {memory:.1} times the memory of the default \
             {DEFAULT_CELL} mm. Deviation is accurate to about one cell."
        )
    }
}

/// The state of the results inspector.
#[derive(Debug, Default)]
pub struct ResultsPanel {
    /// What the part is coloured by.
    pub colour_by: ColourBy,
    /// The text of the cell size field while it is being edited.
    cell_text: Option<String>,
    /// The latest run and its numbers.
    summary: Option<(Arc<Regeneration>, Vec<SetupResults>)>,
    /// Why the cell size was refused.
    pub(crate) error: Option<String>,
}

fn tint(band: DeviationBand) -> Color32 {
    roles::to_color32(roles::band_colour(band))
}

impl ResultsPanel {
    /// Draws the inspector for the latest run. Returns true when the colour
    /// choice changed this frame.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut Application,
        run: Option<&Arc<Regeneration>>,
    ) -> bool {
        let before = self.colour_by;
        ui.label(
            RichText::new("SIMULATION RESULTS")
                .strong()
                .color(DARK.muted),
        );
        ui.add_space(4.0);
        ui.label(RichText::new("Colour by").color(DARK.muted));
        ui.horizontal(|ui| {
            for choice in ColourBy::ALL {
                ui.selectable_value(&mut self.colour_by, choice, choice.name());
            }
        });
        ui.add_space(6.0);
        // The numbers are worked out once for each run, not every frame.
        if let Some(r) = run {
            if !self
                .summary
                .as_ref()
                .is_some_and(|(c, _)| Arc::ptr_eq(c, r))
            {
                self.summary = Some((Arc::clone(r), summarise(r)));
            }
        } else {
            self.summary = None;
        }
        let results = self.summary.as_ref().map(|(_, r)| r.clone());
        match results {
            Some(results) if !results.is_empty() => {
                if run.is_some_and(|r| *r.project != *app.project()) {
                    ui.colored_label(
                        DARK.warning,
                        "The project has changed since this simulation. Simulate again.",
                    );
                }
                for result in &results {
                    result_block(ui, result, results.len() > 1);
                }
            }
            Some(_) => {
                ui.label("This run was not simulated. Choose Simulate to see the results.");
            }
            None => {
                ui.label("Nothing has been generated yet.");
            }
        }
        ui.add_space(8.0);
        self.resolution(ui, app);
        before != self.colour_by
    }

    /// The resolution control: the cell size, default 0.1 mm and finest
    /// 0.025 mm, with its cost (SIM-07; decision 0025).
    fn resolution(&mut self, ui: &mut egui::Ui, app: &mut Application) {
        let current = app.project().simulation().cell.0;
        ui.label(RichText::new("RESOLUTION").strong().color(DARK.muted));
        ui.horizontal(|ui| {
            ui.label("Cell size");
            // The edit buffer lives only while the field has the keyboard (or
            // holds a refused value), so a change from outside, such as an
            // undo, is shown and never written back.
            let id = ui.make_persistent_id("results cell size");
            if !ui.memory(|m| m.has_focus(id)) && self.error.is_none() {
                self.cell_text = None;
            }
            let text = self
                .cell_text
                .get_or_insert_with(|| format!("{current:.3}"));
            let field = ui.add(egui::TextEdit::singleline(text).id(id).desired_width(70.0));
            ui.label(RichText::new("mm").color(DARK.muted));
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let reset = ui
                .add_enabled(
                    (current - DEFAULT_CELL).abs() > 1e-12,
                    egui::Button::new("Default"),
                )
                .clicked();
            if enter || field.lost_focus() || reset {
                let wanted = if reset {
                    Ok(DEFAULT_CELL)
                } else {
                    text.trim()
                        .parse::<f64>()
                        .map_err(|_| format!("“{}” is not a number of millimetres.", text.trim()))
                };
                match wanted {
                    Ok(cell) if (cell - current).abs() > 1e-12 => {
                        self.apply(app, cell);
                    }
                    Ok(_) => {
                        self.cell_text = None;
                        self.error = None;
                    }
                    Err(message) => self.error = Some(message),
                }
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(DARK.error, error);
        }
        ui.label(RichText::new(cost_line(current)).small().color(DARK.muted));
        ui.label(
            RichText::new(format!(
                "Between {FINEST_CELL} and {COARSEST_CELL} mm. A change makes the simulation stale."
            ))
            .small()
            .color(DARK.muted),
        );
    }

    pub(crate) fn apply(&mut self, app: &mut Application, cell: f64) {
        let settings = SimulationSettings {
            cell: Millimetres(cell),
        };
        if let Err(e) = settings.validate() {
            self.error = Some(e.to_string());
            return;
        }
        match app.execute(Command::SetSimulation(settings)) {
            Ok(_) => {
                self.error = None;
                self.cell_text = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
}

fn row(ui: &mut egui::Ui, label: &str, value: String, colour: Option<Color32>) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = RichText::new(value).monospace();
            ui.label(match colour {
                Some(c) => text.color(c),
                None => text,
            });
        });
    });
}

/// The "Largest overcut" value: the depth, or "none", and whether the cut
/// is within the tolerance and so not counted as an overcut (decision
/// record 0162).
pub fn overcut_text(r: &SetupResults) -> String {
    match r.largest_overcut {
        None => "none".to_owned(),
        Some(v) if v <= r.tolerance => format!("{v:.3} mm, within ±{:.3} mm", r.tolerance),
        Some(v) if r.overcut_is_bound => format!("{v:.3} mm or more"),
        Some(v) => format!("{v:.3} mm"),
    }
}

/// How many samples and places are cut deeper than the tolerance, so the
/// largest overcut can be read as one point or many (decision record 0162).
pub fn overcut_spread(r: &SetupResults) -> String {
    if r.overcut_samples == 0 {
        return "none".to_owned();
    }
    let places = match r.overcut_places {
        1 => "1 place".to_owned(),
        n if n >= MAX_PLACES => format!("{MAX_PLACES}+ places"),
        n => format!("{n} places"),
    };
    format!("{} in {places}", r.overcut_samples)
}

fn result_block(ui: &mut egui::Ui, r: &SetupResults, titled: bool) {
    if titled {
        ui.label(RichText::new(&r.setup).strong());
    }
    ui.label(
        RichText::new("WORST VALUES AFTER THE LAST OPERATION")
            .small()
            .color(DARK.muted),
    );
    let overcut = r.largest_overcut.is_some_and(|v| v > r.tolerance);
    row(
        ui,
        "Largest overcut",
        overcut_text(r),
        overcut.then(|| tint(DeviationBand::Overcut)),
    );
    row(ui, "Overcut samples", overcut_spread(r), None);
    row(
        ui,
        "Largest material left",
        r.largest_left.map_or_else(
            || "none".to_owned(),
            |v| {
                if r.left_is_bound {
                    format!("{v:.3} mm or more")
                } else {
                    format!("{v:.3} mm")
                }
            },
        ),
        None,
    );
    row(
        ui,
        &format!("Within ±{:.3} mm", r.tolerance),
        r.within
            .map_or_else(|| "n/a".to_owned(), |v| format!("{:.1} %", v * 100.0)),
        None,
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new("DEVIATION DISTRIBUTION")
            .small()
            .color(DARK.muted),
    );
    histogram_rows(ui, r);
    ui.add_space(6.0);
    ui.label(RichText::new("BY OPERATION").small().color(DARK.muted));
    egui::Grid::new(("results by operation", &r.setup))
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            for h in ["Operation", "Moves", "Removed mm³", "Status"] {
                ui.label(RichText::new(h).small().color(DARK.muted));
            }
            ui.end_row();
            for o in &r.operations {
                ui.label(&o.name);
                ui.label(RichText::new(o.moves.to_string()).monospace());
                ui.label(RichText::new(format!("{:.0}", o.removed)).monospace());
                if o.rapid_cuts > 0 {
                    ui.colored_label(DARK.error, format!("{} rapid cut(s)", o.rapid_cuts));
                } else {
                    ui.label("Clean");
                }
                ui.end_row();
            }
        });
}

/// The histogram as one labelled row per bin, in the colours of the view:
/// the range in millimetres, a bar, and the count and share. The first row
/// starts at the deepest overcut and the last ends at the most material
/// left, so the extremes are on it (decision record 0162).
fn histogram_rows(ui: &mut egui::Ui, r: &SetupResults) {
    if r.histogram.is_empty() {
        ui.label("No samples.");
        return;
    }
    ui.label(
        RichText::new("Below zero is cut under the surface; above zero is material left.")
            .small()
            .color(DARK.muted),
    );
    let peak = r
        .histogram
        .iter()
        .map(|b| b.count)
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    let last = r.histogram.len() - 1;
    egui::Grid::new(("deviation histogram", &r.setup))
        .num_columns(3)
        .spacing([4.0, 2.0])
        .show(ui, |ui| {
            for h in ["Range, mm", "", "Samples"] {
                ui.label(RichText::new(h).small().color(DARK.muted));
            }
            ui.end_row();
            for (i, bin) in r.histogram.iter().enumerate() {
                let low_bound = i == 0 && r.overcut_is_bound;
                let high_bound = i == last && r.left_is_bound;
                ui.label(
                    RichText::new(bin_range(bin, low_bound, high_bound))
                        .small()
                        .monospace(),
                );
                let (rect, _) = ui.allocate_exact_size(Vec2::new(28.0, 10.0), Sense::hover());
                let w = rect.width() * bin.count as f32 / peak;
                if bin.count > 0 {
                    let bar = egui::Rect::from_min_size(rect.min, Vec2::new(w.max(1.0), 10.0));
                    ui.painter().rect_filled(bar, 0.0, tint(bin.band));
                }
                ui.painter().line_segment(
                    [rect.left_top(), rect.left_bottom()],
                    Stroke::new(1.0, DARK.line),
                );
                let share = bin.count as f64 / r.samples.max(1) as f64 * 100.0;
                ui.label(
                    RichText::new(format!("{} ({share:.1}%)", bin.count))
                        .small()
                        .monospace(),
                );
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    // The bin ends are copied from the samples, so they compare exactly.
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn req_sim_05_colour_by_remaining_hides_overcut_and_overcut_hides_the_rest() {
        assert!(ColourBy::Deviation.shows(DeviationBand::Overcut));
        assert!(ColourBy::Deviation.shows(DeviationBand::Thick));
        assert!(!ColourBy::Remaining.shows(DeviationBand::Overcut));
        assert!(ColourBy::Remaining.shows(DeviationBand::Medium));
        assert!(ColourBy::Overcut.shows(DeviationBand::Overcut));
        assert!(!ColourBy::Overcut.shows(DeviationBand::Thin));
        assert!(!ColourBy::Overcut.shows(DeviationBand::OnTarget));
    }

    use openchuck_app::setup::Point3;
    use openchuck_app::sim::DeviationPoint;

    /// A deviation with one sample of each value, as `compare` reports it:
    /// the worst values are the extremes of the samples.
    fn deviation(values: &[(f64, bool)], tolerance: f64, places: usize) -> Deviation {
        let at = |i: usize, (value, at_least): (f64, bool)| DeviationPoint {
            point: Point3::new(i as f64, 0.0, 0.0),
            normal: Point3::new(0.0, 0.0, 1.0) - Point3::new(0.0, 0.0, 0.0),
            value,
            at_least,
        };
        let points: Vec<DeviationPoint> =
            values.iter().enumerate().map(|(i, v)| at(i, *v)).collect();
        let worst_overcut = points
            .iter()
            .filter(|p| p.value < 0.0)
            .min_by(|a, b| a.value.total_cmp(&b.value))
            .copied();
        let worst_remaining = points
            .iter()
            .filter(|p| p.value > 0.0)
            .max_by(|a, b| a.value.total_cmp(&b.value))
            .copied();
        let overcut_count = points.iter().filter(|p| -p.value > tolerance).count();
        Deviation {
            overcut_places: worst_overcut.into_iter().cycle().take(places).collect(),
            points,
            worst_remaining,
            worst_overcut,
            overcut_count,
            overcut_limit: tolerance,
        }
    }

    /// Issue 217: a largest overcut of 1.020 mm beside a histogram drawn from
    /// -0.1 to +0.5 mm. The histogram's range must reach the worst values
    /// the panel reports, whatever they are and whatever the cell size.
    #[test]
    fn req_sim_05_the_histogram_range_covers_the_reported_extremes() {
        let cases: [(&[(f64, bool)], f64); 5] = [
            // The case of issue 217: one deep overcut, material left above 0.5.
            (
                &[
                    (-1.02, false),
                    (-0.03, false),
                    (0.0, false),
                    (0.2, false),
                    (0.7, false),
                ],
                0.1,
            ),
            // Material left far past the old end of +0.5 mm, up to the search
            // limit.
            (&[(0.0, false), (3.0, true)], 0.1),
            // A search-limited overcut.
            (&[(-3.0, true), (0.05, false)], 0.025),
            // Nothing beyond the tolerance either way.
            (&[(-0.05, false), (0.04, false)], 0.1),
            // A coarse cell, wider than the first fixed edges.
            (&[(-0.6, false), (0.3, false), (1.4, false)], 1.0),
        ];
        for (values, tolerance) in cases {
            let d = deviation(values, tolerance, 1);
            let r = SetupResults::new("Setup 1".into(), tolerance, Some(&d), Vec::new());
            let first = r.histogram.first().expect("there are samples");
            let last = r.histogram.last().expect("there are samples");
            if let Some(over) = r.largest_overcut {
                assert!(
                    first.low <= -over,
                    "{values:?}: starts at {} for {over}",
                    first.low
                );
            }
            if let Some(left) = r.largest_left {
                assert!(
                    last.high >= left,
                    "{values:?}: ends at {} for {left}",
                    last.high
                );
            }
            // The ends are the extremes themselves, or the tolerance band.
            let min = values.iter().map(|v| v.0).fold(f64::INFINITY, f64::min);
            let max = values.iter().map(|v| v.0).fold(f64::NEG_INFINITY, f64::max);
            assert_eq!(first.low, min.min(-tolerance), "{values:?}");
            assert_eq!(last.high, max.max(tolerance), "{values:?}");
            // The bins are contiguous and in order.
            for w in r.histogram.windows(2) {
                assert_eq!(w[0].high, w[1].low, "{values:?}");
                assert!(w[0].low < w[0].high, "{values:?}");
            }
        }
    }

    /// The panel's numbers come from the same samples and agree: every
    /// sample is in one bin, the surface bin is the share within the
    /// tolerance, the overcut bins hold the samples counted as overcut, and
    /// each bin is drawn in the colour band its values are coloured in on
    /// the part (`deviation_bands`).
    #[test]
    fn req_sim_05_the_histogram_the_worst_values_and_the_shares_agree() {
        use openchuck_app::pathview::deviation_bands;
        let tolerance = 0.1;
        let values = [
            (-1.02, false),
            (-0.6, false),
            (-0.3, false),
            (-0.1, false),
            (0.0, false),
            (0.1, false),
            (0.25, false),
            (0.3, false),
            (0.5, false),
            (1.0, false),
            (2.0, false),
        ];
        let d = deviation(&values, tolerance, 3);
        let r = SetupResults::new("Setup 1".into(), tolerance, Some(&d), Vec::new());
        assert_eq!(r.samples, values.len());
        let total: usize = r.histogram.iter().map(|b| b.count).sum();
        assert_eq!(total, r.samples);
        let surface: Vec<&Bin> = r
            .histogram
            .iter()
            .filter(|b| b.band == DeviationBand::OnTarget)
            .collect();
        assert_eq!(surface.len(), 1);
        assert_eq!((surface[0].low, surface[0].high), (-tolerance, tolerance));
        assert_eq!(Some(surface[0].count as f64 / r.samples as f64), r.within);
        let overcut: usize = r
            .histogram
            .iter()
            .filter(|b| b.band == DeviationBand::Overcut)
            .map(|b| b.count)
            .sum();
        assert_eq!(overcut, d.overcut_count);
        assert_eq!(overcut, r.overcut_samples);
        assert_eq!(r.overcut_places, 3);
        assert_eq!(overcut_spread(&r), "3 in 3 places");
        assert_eq!(r.largest_overcut, Some(1.02));
        assert_eq!(r.largest_left, Some(2.0));
        // The edges are the colour thresholds, mirrored for the overcut.
        let edges: Vec<f64> = r.histogram.iter().map(|b| b.low).collect();
        assert_eq!(edges, [-1.02, -1.0, -0.5, -0.25, -0.1, 0.1, 0.25, 0.5, 1.0]);
        // Each value is in the bin of the band it colours the part in.
        let bands = deviation_bands(
            &values.iter().map(|v| Some(*v)).collect::<Vec<_>>(),
            tolerance,
        );
        for (t, (v, _)) in values.iter().enumerate() {
            let band = bands
                .bands
                .iter()
                .find(|(_, s)| s.contains(&(t as u32)))
                .map(|(b, _)| *b)
                .unwrap();
            let bins = histogram(&[*v], tolerance);
            let bin = bins.iter().find(|b| b.count == 1).unwrap();
            assert_eq!(bin.band, band, "{v}");
            let in_full = r
                .histogram
                .iter()
                .find(|b| b.band == band && b.low <= *v && *v <= b.high);
            assert!(in_full.is_some(), "{v}");
        }
        // Counts per bin, each boundary value on the side its band puts it.
        let counts: Vec<usize> = r.histogram.iter().map(|b| b.count).collect();
        assert_eq!(counts, [1, 1, 1, 0, 3, 1, 2, 1, 1]);
    }

    #[test]
    fn req_sim_05_the_bins_are_labelled_and_the_overcut_spread_is_stated() {
        let bin = Bin {
            low: -1.02,
            high: -1.0,
            count: 1,
            band: DeviationBand::Overcut,
        };
        assert_eq!(bin_range(&bin, false, false), "-1.020 to -1.000");
        assert_eq!(bin_range(&bin, true, false), "≤-1.020 to -1.000");
        let top = Bin {
            low: 1.0,
            high: 3.0,
            count: 1,
            band: DeviationBand::Thick,
        };
        assert_eq!(bin_range(&top, false, true), "1.000 to ≥3.000");
        let tolerance = 0.1;
        let one = SetupResults::new(
            "S".into(),
            tolerance,
            Some(&deviation(&[(-1.02, false), (0.0, false)], tolerance, 1)),
            Vec::new(),
        );
        assert_eq!(overcut_text(&one), "1.020 mm");
        assert_eq!(overcut_spread(&one), "1 in 1 place");
        let many = SetupResults::new(
            "S".into(),
            tolerance,
            Some(&deviation(
                &[(-1.02, false), (-0.5, false), (-0.4, false), (0.0, false)],
                tolerance,
                MAX_PLACES,
            )),
            Vec::new(),
        );
        assert_eq!(overcut_spread(&many), "3 in 10+ places");
        // A cut below the surface within the tolerance is not an overcut.
        let within = SetupResults::new(
            "S".into(),
            tolerance,
            Some(&deviation(&[(-0.05, false), (0.0, false)], tolerance, 0)),
            Vec::new(),
        );
        assert_eq!(overcut_text(&within), "0.050 mm, within ±0.100 mm");
        assert_eq!(overcut_spread(&within), "none");
        let deep = SetupResults::new(
            "S".into(),
            tolerance,
            Some(&deviation(&[(-3.0, true)], tolerance, 1)),
            Vec::new(),
        );
        assert_eq!(overcut_text(&deep), "3.000 mm or more");
        let none = SetupResults::new("S".into(), tolerance, None, Vec::new());
        assert_eq!(overcut_text(&none), "none");
        assert!(none.histogram.is_empty());
        assert!(histogram(&[f64::NAN], tolerance).is_empty());
    }

    #[test]
    fn req_sim_11_the_probe_gives_the_value_of_the_view_without_a_position() {
        let t = 0.1;
        let by = ColourBy::Deviation;
        assert_eq!(probe_text(by, Some((-1.02, false)), t), "Overcut 1.020 mm");
        assert_eq!(
            probe_text(by, Some((0.35, false)), t),
            "Material left 0.350 mm"
        );
        assert_eq!(
            probe_text(by, Some((-0.004, false)), t),
            "Overcut 0.004 mm, within ±0.100 mm"
        );
        assert_eq!(
            probe_text(by, Some((0.0, false)), t),
            "On the surface, 0.000 mm"
        );
        assert_eq!(
            probe_text(by, Some((3.0, true)), t),
            "Material left 3.000 mm or more"
        );
        assert_eq!(probe_text(by, None, t), "Not measured");
        // Remaining and Overcut give only their own value.
        assert_eq!(
            probe_text(ColourBy::Remaining, Some((0.35, false)), t),
            "Material left 0.350 mm"
        );
        assert_eq!(
            probe_text(ColourBy::Remaining, Some((-1.02, false)), t),
            "No material left"
        );
        assert_eq!(
            probe_text(ColourBy::Overcut, Some((-1.02, false)), t),
            "Overcut 1.020 mm"
        );
        assert_eq!(
            probe_text(ColourBy::Overcut, Some((0.35, false)), t),
            "No overcut"
        );
    }

    /// The labelled rows fit the inspector at its narrowest (theme
    /// `region::INSPECTOR`), with the longest labels a run can give.
    #[test]
    fn req_sim_05_the_labelled_histogram_fits_the_narrowest_inspector() {
        let narrowest = crate::theme::width::INSPECTOR.1;
        let values: Vec<(f64, bool)> = [-3.0, -0.6, -0.3, -0.2, 0.0, 0.2, 0.3, 0.6]
            .into_iter()
            .map(|v| (v, false))
            .chain([(3.0, true)])
            .chain(std::iter::repeat_n((0.0, false), 99_990))
            .collect();
        let mut d = deviation(&values, 0.025, MAX_PLACES);
        if let Some(p) = d.worst_overcut.as_mut() {
            p.at_least = true;
        }
        let r = SetupResults::new("Setup 1".into(), 0.025, Some(&d), Vec::new());
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::install_style(&ctx, crate::theme::Theme::Dark);
        let (mut used, mut room) = (0.0, 0.0);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(narrowest, 900.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                room = ui.max_rect().width();
                result_block(ui, &r, true);
                used = ui.min_rect().width();
            });
        });
        output.textures_delta.clear();
        assert!(used <= room + 0.5, "{used} wide in {room}");
    }

    #[test]
    fn req_sim_07_the_cost_line_states_the_cost_against_the_default() {
        assert!(cost_line(DEFAULT_CELL).starts_with("Default."));
        let fine = cost_line(FINEST_CELL);
        assert!(fine.contains("64.0 times the time"), "{fine}");
        assert!(fine.contains("16.0 times the memory"), "{fine}");
    }
}
