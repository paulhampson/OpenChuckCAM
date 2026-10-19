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

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};
use openchuck_app::pathview::DeviationBand;
use openchuck_app::setup::Millimetres;
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

/// Histogram edges, millimetres: a bin every 0.05 mm from -0.05 to +0.50.
pub const HIST_LOW: f64 = -0.05;
/// The upper end of the histogram, millimetres.
pub const HIST_HIGH: f64 = 0.50;
/// The number of histogram bins.
pub const HIST_BINS: usize = 11;

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

/// The numbers of one setup's simulation.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupResults {
    /// The setup's name.
    pub setup: String,
    /// The tolerance band: the cell size, the simulation's own error.
    pub tolerance: f64,
    /// The deepest overcut, millimetres (positive), when any sample is cut
    /// below the surface.
    pub largest_overcut: Option<f64>,
    /// The most material left, millimetres.
    pub largest_left: Option<f64>,
    /// The share of samples within the tolerance, 0 to 1, when there are samples.
    pub within: Option<f64>,
    /// Samples per bin of the histogram, from [`HIST_LOW`] to [`HIST_HIGH`];
    /// values outside the range fall in the end bins.
    pub histogram: [usize; HIST_BINS],
    /// The operations, in machining order.
    pub operations: Vec<OperationRow>,
}

/// The bin a value falls in.
pub fn bin_of(value: f64) -> usize {
    let width = (HIST_HIGH - HIST_LOW) / HIST_BINS as f64;
    let i = ((value - HIST_LOW) / width).floor();
    if i.is_nan() || i < 0.0 {
        0
    } else {
        (i as usize).min(HIST_BINS - 1)
    }
}

/// The band colour a histogram bin is drawn in: the bins left of the
/// tolerance are overcut, the bins around zero on the surface, and the rest
/// the three amounts of material left.
pub fn bin_band(bin: usize, tolerance: f64) -> DeviationBand {
    let width = (HIST_HIGH - HIST_LOW) / HIST_BINS as f64;
    let mid = HIST_LOW + width * (bin as f64 + 0.5);
    if mid < -tolerance {
        DeviationBand::Overcut
    } else if mid <= tolerance {
        DeviationBand::OnTarget
    } else if mid > 0.25 {
        DeviationBand::Medium
    } else {
        DeviationBand::Thin
    }
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
            let mut histogram = [0; HIST_BINS];
            let (mut largest_overcut, mut largest_left, mut within) = (None, None, None);
            if let Some(d) = &setup.deviation {
                for p in &d.points {
                    if let Some(slot) = histogram.get_mut(bin_of(p.value)) {
                        *slot += 1;
                    }
                }
                let n = d.points.len();
                if n > 0 {
                    let ok = d
                        .points
                        .iter()
                        .filter(|p| p.value.abs() <= tolerance)
                        .count();
                    within = Some(ok as f64 / n as f64);
                }
                largest_overcut = d.worst_overcut.map(|p| -p.value).filter(|v| *v > 0.0);
                largest_left = d.worst_remaining.map(|p| p.value).filter(|v| *v > 0.0);
            }
            SetupResults {
                setup: setup.setup.clone(),
                tolerance,
                largest_overcut,
                largest_left,
                within,
                histogram,
                operations: setup
                    .operations
                    .iter()
                    .map(|o| OperationRow {
                        name: o.operation.clone(),
                        moves: o.moves,
                        removed: o.removed_volume,
                        rapid_cuts: o.rapid_cuts.len(),
                    })
                    .collect(),
            }
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
        run: Option<&Regeneration>,
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
        match run.map(|r| (r, summarise(r))) {
            Some((_, results)) if !results.is_empty() => {
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
            let text = self
                .cell_text
                .get_or_insert_with(|| format!("{current:.3}"));
            let field = ui.add(egui::TextEdit::singleline(text).desired_width(70.0));
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

fn result_block(ui: &mut egui::Ui, r: &SetupResults, titled: bool) {
    if titled {
        ui.label(RichText::new(&r.setup).strong());
    }
    ui.label(
        RichText::new("WORST VALUES AFTER THE LAST OPERATION")
            .small()
            .color(DARK.muted),
    );
    row(
        ui,
        "Largest overcut",
        r.largest_overcut
            .map_or_else(|| "none".to_owned(), |v| format!("{v:.3} mm")),
        r.largest_overcut.map(|_| tint(DeviationBand::Overcut)),
    );
    row(
        ui,
        "Largest material left",
        r.largest_left
            .map_or_else(|| "none".to_owned(), |v| format!("{v:.3} mm")),
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
    histogram(ui, r);
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

/// The histogram, drawn in the colours of the view.
fn histogram(ui: &mut egui::Ui, r: &SetupResults) {
    let width = ui.available_width().max(120.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 70.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let peak = r.histogram.iter().copied().max().unwrap_or(0).max(1) as f32;
    let bar_w = rect.width() / HIST_BINS as f32;
    for (i, count) in r.histogram.iter().enumerate() {
        let h = (rect.height() - 4.0) * (*count as f32 / peak);
        let x = rect.left() + bar_w * i as f32;
        let bar = egui::Rect::from_min_max(
            egui::pos2(x + 1.0, rect.bottom() - h),
            egui::pos2(x + bar_w - 1.0, rect.bottom()),
        );
        painter.rect_filled(bar, 0.0, tint(bin_band(i, r.tolerance)));
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, DARK.line),
    );
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{HIST_LOW:.2}")).small().monospace());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("+{HIST_HIGH:.1} mm"))
                    .small()
                    .monospace(),
            );
        });
    });
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn req_sim_05_histogram_bins_cover_the_range_and_clamp_outliers() {
        assert_eq!(bin_of(-0.05), 0);
        assert_eq!(bin_of(-5.0), 0);
        assert_eq!(bin_of(0.0), 1);
        assert_eq!(bin_of(0.5), HIST_BINS - 1);
        assert_eq!(bin_of(40.0), HIST_BINS - 1);
        assert_eq!(bin_of(f64::NAN), 0);
        // Bins left of the tolerance are overcut, those around zero are on
        // the surface, the rest are material left.
        assert_eq!(bin_band(0, 0.05), DeviationBand::OnTarget);
        assert_eq!(bin_band(0, 0.01), DeviationBand::Overcut);
        assert_eq!(bin_band(1, 0.05), DeviationBand::OnTarget);
        assert_eq!(bin_band(1, 0.01), DeviationBand::Thin);
        assert_eq!(bin_band(3, 0.01), DeviationBand::Thin);
        assert_eq!(bin_band(HIST_BINS - 1, 0.01), DeviationBand::Medium);
    }

    #[test]
    fn req_sim_07_the_cost_line_states_the_cost_against_the_default() {
        assert!(cost_line(DEFAULT_CELL).starts_with("Default."));
        let fine = cost_line(FINEST_CELL);
        assert!(fine.contains("64.0 times the time"), "{fine}");
        assert!(fine.contains("16.0 times the memory"), "{fine}");
    }
}
