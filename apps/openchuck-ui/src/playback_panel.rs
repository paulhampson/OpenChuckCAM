// SPDX-License-Identifier: AGPL-3.0-or-later
//! Toolpath display and playback in the viewport (SIM-01, SIM-02, SIM-04,
//! SIM-05; work breakdown slice 9 items 3 to 6 and 8; decision record 0040).
//!
//! After a run the panel prepares its toolpath for display as a job, then
//! draws it in the viewport's path layer, one colour for each class of move
//! and one switch for each operation. Playback moves a cursor along the
//! moves; the tool follows it, drawn once and placed by a transform each
//! frame, and in the machine view the workpiece turns with A. The stock at
//! the cursor comes from the simulator's checkpoints, and the part can be
//! coloured by what the simulation found. Everything slow is a job; the
//! panel only polls them.

use std::sync::Arc;

use eframe::egui::{self, Color32, RichText};
use openchuck_app::pathview::{
    DeviationBand, DeviationBands, PathClass, PathLines, PlaybackData, zoom_level,
};
use openchuck_app::playback::{
    Cursor, MoveWindow, Playback, PoseSample, ToolMeshes, ViewFrame, continuous_a_text,
    tool_meshes, wrapped_a_text,
};
use openchuck_app::project::id::{ModelId, SetupId};
use openchuck_app::stockplay::{StockPlayback, StockSurface};
use openchuck_app::{Application, JobHandle, Regeneration};
use openchuck_viewport::camera::place;
use openchuck_viewport::{Colour, Layer, LineStyle, Scene, Slot};

use crate::results_panel::{BandFilter, ColourBy};
use crate::roles;
use crate::theme::DARK;
use crate::viewport::{ToolReadout, Viewport};

const PROBLEM: Color32 = DARK.error;

/// Points of toolpath the viewport is asked to draw at most. A million
/// points is about 31 MB on the GPU (two 16-byte vertices per segment;
/// measured with the `path_frames` example, decision record 0166). Its
/// frame time on the reference machines is still to be measured (OI-176,
/// OI-590).
pub const DEFAULT_POINT_BUDGET: usize = 1_000_000;

/// Program seconds per second of the clock the speed list offers; the last
/// two play the whole program in a set time.
const SPEEDS: [(&str, f64); 5] = [
    ("1×", 1.0),
    ("10×", 10.0),
    ("100×", 100.0),
    ("1000×", 1000.0),
    ("10 000×", 10_000.0),
];

/// The tool: a lilac tip (the cutter) on a grey body (the shank), 7.4.
const CUTTER: Colour = roles::TOOL_TIP;
const SHANK: Colour = roles::TOOL_BODY;

/// The faint trace of the path outside the playback window: grey, thin and
/// see-through (issue 205; decision record 0169).
pub const TRACE: Colour = [0.62, 0.64, 0.68, 0.35];

/// The tooltip's words on the A readout (issue 207): the angle the G-code
/// gives, which keeps counting past 360 degrees.
pub const CONTINUOUS_A: &str = "Continuous A in the G-code";

/// The tick box that hides the path outside the playback window (issue 205).
pub const HIDE_OUTSIDE: &str = "Hide path outside window";

/// The tick box that turns the project's stock simulation off (issue 214).
pub const SKIP_STOCK: &str = "Skip stock simulation";

/// How a class of move is drawn (SIM-01; specification 7.4): its colour,
/// width and line style. Cut moves amber and solid; link moves green, solid
/// and thinner; rapid moves grey and dotted; entry and exit moves lilac.
pub fn class_look(class: PathClass) -> (Colour, f32, LineStyle) {
    match class {
        PathClass::Rapid => (roles::RAPID, roles::LINE_WIDTH, LineStyle::Dotted),
        PathClass::Link => (roles::LINK, roles::LINE_WIDTH, LineStyle::Solid),
        PathClass::Entry => (roles::ENTRY, roles::PATH_WIDTH, LineStyle::Solid),
        PathClass::Cut => (roles::CUT, roles::PATH_WIDTH, LineStyle::Solid),
    }
}

/// The scene of path lines: one polyline each, drawn as its class is.
pub fn paths_scene(lines: &PathLines) -> Scene {
    let mut scene = Scene::default();
    for line in &lines.lines {
        let (colour, width, style) = class_look(line.class);
        scene.add_styled_polyline(&line.points, colour, false, width, style);
        if let Some(added) = scene.lines.last_mut() {
            added.slot = Slot::Part;
        }
    }
    scene
}

/// The faint trace of the path: every line in one batch, in one colour, so
/// it is one draw call however many runs the path has (decision record
/// 0169).
pub fn trace_scene(lines: &PathLines) -> Scene {
    let mut scene = Scene::default();
    let segments: Vec<[f32; 3]> = lines
        .lines
        .iter()
        .flat_map(|line| {
            line.points.windows(2).flat_map(|pair| {
                pair.iter()
                    .map(|p| [p.x as f32, p.y as f32, p.z as f32])
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    if !segments.is_empty() {
        scene.lines.push(openchuck_viewport::LineItem {
            segments,
            colour: TRACE,
            on_top: false,
            slot: Slot::Part,
            width: 0.0,
            style: LineStyle::Solid,
        });
    }
    scene
}

/// The scene of a tool: its cutter and shank, built at the origin along +Z
/// and in the tool slot, so one transform per frame puts them in place.
pub fn tool_scene(meshes: &ToolMeshes) -> Scene {
    let mut scene = Scene::default();
    scene.add_mesh(&meshes.cutter, &[], CUTTER, CUTTER);
    if let Some(shank) = &meshes.shank {
        scene.add_mesh(shank, &[], SHANK, SHANK);
    }
    for mesh in &mut scene.meshes {
        mesh.slot = Slot::Tool;
    }
    scene
}

/// What a drag on the playback window's strip moves.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WindowDrag {
    /// An end, the other held at the strip boundary `fixed` (moves done):
    /// the start's boundary when the end is dragged, the boundary after the
    /// last move when the start is. Held for the whole drag, so an end
    /// dragged past the other keeps its distance from the right one.
    End { fixed: usize },
    /// The whole window, held this many moves from its start.
    Body(isize),
}

/// How near to an end of the window, in pixels, a drag takes that end.
const HANDLE_REACH: f32 = 6.0;

/// The window `drag` makes on a strip of `moves` moves when the pointer is
/// at boundary `at`, the number of moves done there (issue 205). An end
/// takes the moves between the held boundary and the pointer, at least one:
/// with the pointer on the held boundary, the move on the window's side of
/// it. Dragging the body keeps its length.
fn dragged_window(
    window: Option<MoveWindow>,
    drag: WindowDrag,
    at: usize,
    moves: usize,
) -> Option<MoveWindow> {
    let last_move = moves.checked_sub(1)?;
    let w = window.unwrap_or(MoveWindow {
        first: 0,
        last: last_move,
    });
    Some(match drag {
        WindowDrag::End { fixed } => {
            // On the held boundary, a window before it keeps its last move.
            let at = if at == fixed && w.last < fixed {
                at - 1
            } else {
                at
            };
            let first = fixed.min(at).min(last_move);
            let end = fixed.max(at).clamp(first + 1, moves);
            MoveWindow {
                first,
                last: end - 1,
            }
        }
        WindowDrag::Body(offset) => {
            let at = at.min(last_move);
            let length = w.last - w.first;
            let first = at.saturating_add_signed(-offset).min(last_move - length);
            MoveWindow {
                first,
                last: first + length,
            }
        }
    })
}

#[derive(Default)]
struct StockState {
    show: bool,
    prepare: Option<JobHandle<StockPlayback>>,
    playback: Option<Arc<StockPlayback>>,
    mesh: Option<(JobHandle<StockSurface>, Cursor)>,
    surface: Option<Arc<StockSurface>>,
    /// The cursor of the cut part on screen. While playing the tool follows
    /// the clock and this may be behind it; when still they match.
    shown: Option<Cursor>,
    /// The tool follows the clock: set while playing, and kept after a pause
    /// until the cut part has caught up with the tool, so pausing never
    /// moves the tool back (the owner's decision D3 on issue #382).
    trailing: bool,
    error: Option<String>,
}

/// The job measuring the part for a run and one of its setups.
type PartJob = (
    Arc<Regeneration>,
    SetupId,
    JobHandle<(ModelId, DeviationBands)>,
);

/// The part's own deviation colours on Simulate (SIM-05, decision 0492): the
/// measuring job, what it found for each setup of the run (kept, so going
/// back to a setup costs nothing), and what the viewport has been given.
#[derive(Default)]
struct PartColours {
    /// The job running, for a run and one of its setups.
    job: Option<PartJob>,
    /// The run `done` belongs to.
    run: Option<Arc<Regeneration>>,
    /// The viewport's scene `done` was found for (`Viewport::scene_count`):
    /// a scene read again may be of a model file changed behind the
    /// application, whose triangles the old colours do not fit.
    scene: u64,
    /// What the job found for each setup: the part's bands, or `None` when
    /// the model file is no longer the one the run was made with.
    done: Vec<(SetupId, Option<(ModelId, DeviationBands)>)>,
    /// What the viewport shows, and for which run (its address; the run is
    /// kept alive in `run`), so a frame sends it only a change.
    pushed: (usize, PartShown),
}

/// What the viewport is told about the part's colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PartShown {
    /// The part as it is.
    #[default]
    Plain,
    /// The part as it is, with the note that it has changed.
    Changed,
    /// The part coloured by the values found for this setup.
    Bands(SetupId),
}

/// What the cut-part bar under the position bar shows (decision record
/// 0322; the owner's decision D3 on issue #382): where the cut part on screen
/// is, how far it is behind the play head while playing, and the progress of
/// the cut part being made or of preparing playback.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CutPartBar {
    /// Moves done at the cut part on screen.
    shown: Option<usize>,
    /// Moves done at the play head.
    head: usize,
    /// Program seconds the cut part is behind the play head, while playing.
    behind: Option<f64>,
    /// A cut part being made: its moves done, and the share of its replay
    /// done when the job has reported one.
    loading: Option<(usize, Option<f32>)>,
    /// Playback being prepared, and the share done.
    preparing: Option<Option<f32>>,
}

impl CutPartBar {
    /// The words at the bar's right end, if any.
    fn text(&self) -> Option<String> {
        let percent = |f: Option<f32>| {
            f.map_or_else(String::new, |f| format!(" {}%", (f * 100.0).floor() as u32))
        };
        if let Some(fraction) = self.preparing {
            return Some(format!("Preparing the cut part{}", percent(fraction)));
        }
        if let Some(seconds) = self.behind {
            return Some(format!(
                "Cut part {} s behind",
                seconds.round().max(1.0) as u64
            ));
        }
        self.loading
            .map(|(_, fraction)| format!("Updating cut part{}", percent(fraction)))
    }
}

/// Actual job progress shown immediately below the playback position bar.
/// Only the pipeline's monotonic, fixed-total reports supply a percentage;
/// preparation jobs without that contract show activity without an estimate.
pub struct PreparationProgress {
    label: &'static str,
    fraction: Option<f32>,
    percent: Option<u64>,
}

impl PreparationProgress {
    fn show(&self, ui: &mut egui::Ui) {
        let text = self.percent.map_or_else(
            || self.label.to_owned(),
            |percent| format!("{} · {percent} %", self.label),
        );
        if let Some(fraction) = self.fraction {
            ui.add(
                egui::ProgressBar::new(fraction)
                    .text(text)
                    .desired_height(18.0),
            );
        } else {
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::hover());
            ui.put(
                egui::Rect::from_min_size(rect.min, egui::vec2(18.0, 18.0)),
                egui::Spinner::new().size(14.0),
            );
            ui.painter_at(rect).text(
                rect.left_center() + egui::vec2(22.0, 0.0),
                egui::Align2::LEFT_CENTER,
                text,
                egui::FontId::proportional(12.0),
                DARK.text,
            );
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// The toolpath display and playback controls.
pub struct PlaybackPanel {
    /// Whether the panel is shown.
    pub open: bool,
    run: Option<Arc<Regeneration>>,
    data_job: Option<JobHandle<PlaybackData>>,
    data: Option<PlaybackData>,
    playback: Playback,
    /// Which groups' paths are drawn, as the project says (issue 213), as
    /// last given to the path jobs.
    visible: Vec<bool>,
    view: ViewFrame,
    show_tool: bool,
    /// "Hide path outside window" (issue 205).
    hide_outside: bool,
    /// The user's "Memory for playback checkpoints", bytes (decision record
    /// 0322).
    playback_memory: usize,
    /// The window as drawn last, to tell when the path must be made again.
    drawn_window: Option<MoveWindow>,
    /// The faint trace of the whole path, made once per path and visible
    /// groups, and whether it is on screen.
    trace_job: Option<(Vec<bool>, JobHandle<PathLines>)>,
    trace: Option<(Vec<bool>, Arc<PathLines>)>,
    trace_shown: bool,
    /// What the trace costs: (points drawn, points of the path).
    trace_cost: Option<(usize, usize)>,
    /// The window's ends as the number fields hold them, counted from 1.
    window_from: usize,
    window_to: usize,
    budget: usize,
    lines_job: Option<JobHandle<PathLines>>,
    lines_dirty: bool,
    /// The level of detail the zoom asks for ([`zoom_level`]; decision
    /// record 0167).
    zoom_level: usize,
    detail: Option<(usize, usize, f64)>,
    /// The group whose tool the tool layer holds.
    tool_group: Option<usize>,
    tool_dirty: bool,
    stock: StockState,
    /// The part's own colours (decision 0492).
    part: PartColours,
    bands_on: bool,
    bands_surface: Option<Arc<StockSurface>>,
    /// The deviation job running, with the setup it is for.
    bands_job: Option<(SetupId, JobHandle<(ModelId, DeviationBands)>)>,
    bands: Option<DeviationBands>,
    /// The model the bands colour.
    bands_model: Option<ModelId>,
    /// What the part is coloured by (the results inspector's choice).
    colour_by: ColourBy,
    /// The deviation bands shown (the results inspector's choice, SIM-15).
    band_filter: BandFilter,
    /// The setup the bands on screen (or being made) belong to.
    bands_setup: Option<SetupId>,
    jump_move: usize,
    /// What a drag on the window strip moves (issue 205).
    window_drag: Option<WindowDrag>,
    /// Where the window strip was drawn last, and the left and width of
    /// the timeline it maps moves on.
    strip: (egui::Rect, f32, f32),
    error: Option<String>,
    /// The layers need clearing before the next run is drawn.
    reset_layers: bool,
}

impl Default for PlaybackPanel {
    fn default() -> Self {
        Self {
            open: false,
            run: None,
            data_job: None,
            data: None,
            playback: Playback::new(),
            visible: Vec::new(),
            // "Turn part with A" is on unless the user turned it off (issue
            // 203); the shell gives the user's choice.
            view: ViewFrame::Machine,
            show_tool: true,
            hide_outside: false,
            playback_memory: openchuck_app::stockplay::DEFAULT_PLAYBACK_MEMORY,
            drawn_window: None,
            trace_job: None,
            trace: None,
            trace_shown: false,
            trace_cost: None,
            window_from: 1,
            window_to: 1,
            budget: DEFAULT_POINT_BUDGET,
            lines_job: None,
            lines_dirty: false,
            zoom_level: 0,
            detail: None,
            tool_group: None,
            tool_dirty: false,
            stock: StockState::default(),
            part: PartColours::default(),
            bands_on: false,
            bands_surface: None,
            bands_job: None,
            bands: None,
            bands_model: None,
            colour_by: ColourBy::default(),
            band_filter: BandFilter::default(),
            bands_setup: None,
            jump_move: 1,
            window_drag: None,
            strip: (egui::Rect::NOTHING, 0.0, 0.0),
            error: None,
            reset_layers: false,
        }
    }
}

impl PlaybackPanel {
    /// The pipeline or initial display preparation still in progress.
    pub fn preparation_progress(
        &self,
        jobs: &openchuck_app::JobLog,
        pipeline: Option<openchuck_app::JobId>,
        simulates: bool,
    ) -> Option<PreparationProgress> {
        if let Some(job) = pipeline {
            let entry = jobs.get(job);
            let monotonic = entry.filter(|e| {
                e.progress
                    .as_ref()
                    .is_some_and(|p| p.total == Some(openchuck_app::RUN_PROGRESS_TOTAL))
            });
            return Some(PreparationProgress {
                label: if simulates {
                    "Preparing simulation"
                } else {
                    "Preparing toolpaths"
                },
                fraction: monotonic.and_then(openchuck_app::JobEntry::fraction),
                percent: monotonic.and_then(openchuck_app::JobEntry::percent),
            });
        }
        // The cut part's own preparation is in the cut-part bar (decision
        // record 0322).
        let label = if self.data_job.is_some() {
            "Preparing toolpath display"
        } else {
            return None;
        };
        Some(PreparationProgress {
            label,
            fraction: None,
            percent: None,
        })
    }

    /// What the cut-part bar shows, with the progress the jobs reported;
    /// `None` when the cut part is not shown (decision record 0322).
    pub fn cut_part_bar(&self, jobs: &openchuck_app::JobLog) -> Option<CutPartBar> {
        let data = self.data.as_ref()?;
        if !self.stock.show || self.stock.error.is_some() {
            return None;
        }
        let fraction = |id| jobs.get(id).and_then(openchuck_app::JobEntry::fraction);
        let head = self.playback.cursor();
        let shown = self.stock.shown;
        let behind = shown
            .filter(|s| self.stock.trailing && s.done < head.done)
            .map(|s| data.timeline.time_at(head) - data.timeline.time_at(s));
        Some(CutPartBar {
            shown: shown.map(|c| c.done),
            head: head.done,
            behind,
            loading: self
                .stock
                .mesh
                .as_ref()
                .map(|(job, cursor)| (cursor.done, fraction(job.id()))),
            preparing: self.stock.prepare.as_ref().map(|job| fraction(job.id())),
        })
    }

    /// Draws the cut-part bar under the position bar, on its scale: a solid
    /// fill to the cut part on screen with a tick there, an outline up to the
    /// play head while it is behind, and a lighter fill growing with the
    /// progress of the cut part being made or of preparing playback.
    fn cut_part_strip(ui: &mut egui::Ui, bar: &CutPartBar, left: f32, made: f32, moves: usize) {
        let width = ui.available_width().max(160.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, 14.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let stock = crate::roles::to_color32(crate::roles::STOCK);
        let x = |done: usize| left + made * (done as f32 / moves.max(1) as f32).clamp(0.0, 1.0);
        let strip = |a: f32, b: f32| {
            egui::Rect::from_min_max(
                egui::pos2(a.min(b), rect.top() + 3.0),
                egui::pos2(a.max(b), rect.bottom() - 3.0),
            )
        };
        painter.rect_stroke(
            strip(left, left + made),
            1.0,
            egui::Stroke::new(1.0, DARK.selected),
            egui::StrokeKind::Inside,
        );
        let shown = bar.shown.map(x);
        if let Some(fraction) = bar.preparing {
            let end = left + made * fraction.unwrap_or(0.0);
            painter.rect_filled(strip(left, end), 1.0, stock.gamma_multiply(0.45));
        }
        if let Some(end) = shown {
            painter.rect_filled(strip(left, end), 1.0, stock);
            painter.vline(end, rect.y_range(), egui::Stroke::new(2.0, stock));
            if bar.behind.is_some() {
                painter.rect_stroke(
                    strip(end, x(bar.head)),
                    1.0,
                    egui::Stroke::new(1.0, stock),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if let Some((target, fraction)) = bar.loading {
            let from = shown.unwrap_or(left);
            let to = x(target);
            match fraction {
                Some(f) => {
                    painter.rect_filled(
                        strip(from, from + (to - from) * f),
                        1.0,
                        stock.gamma_multiply(0.45),
                    );
                }
                None => {
                    // No total yet: a stripe moving over the stretch.
                    let t = ui.input(|i| i.time) as f32;
                    let span = (to - from).abs().max(8.0);
                    let at = from.min(to) + (t * 60.0) % span;
                    painter.rect_filled(
                        strip(at, (at + 6.0).min(from.max(to))),
                        1.0,
                        stock.gamma_multiply(0.45),
                    );
                    ui.ctx().request_repaint();
                }
            }
        }
        if let Some(text) = bar.text() {
            painter.text(
                rect.right_center() - egui::vec2(4.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                text,
                egui::FontId::proportional(11.0),
                DARK.text,
            );
        }
        response.on_hover_text(
            "Where the cut part shown is in the program. While playing, it may follow behind \
             the tool; when you pause, it catches up.",
        );
    }

    fn simulation_choices(&mut self, ui: &mut egui::Ui, app: &Application) -> Option<bool> {
        let skip = skip_box(ui, app);
        ui.separator();
        let mut machine = self.view == ViewFrame::Machine;
        if ui.checkbox(&mut machine, "Turn part with A")
            .on_hover_text("The tool stands upright and the workpiece turns, as on the machine; otherwise the part is fixed and the tool moves round it")
            .changed()
        {
            self.view = if machine { ViewFrame::Machine } else { ViewFrame::Part };
        }
        skip
    }
    /// Asks every job the panel has running to stop: a job handle does not
    /// cancel its job when dropped, and an obsolete stock preparation would
    /// go on using the pool and holding checkpoint memory.
    fn cancel_jobs(&self) {
        if let Some(job) = &self.data_job {
            job.cancel();
        }
        if let Some(job) = &self.lines_job {
            job.cancel();
        }
        if let Some((_, job)) = &self.trace_job {
            job.cancel();
        }
        if let Some((_, job)) = &self.bands_job {
            job.cancel();
        }
        if let Some((.., job)) = &self.part.job {
            job.cancel();
        }
        if let Some(job) = &self.stock.prepare {
            job.cancel();
        }
        if let Some((job, _)) = &self.stock.mesh {
            job.cancel();
        }
    }

    /// Forgets everything: a project was closed or opened.
    pub fn clear(&mut self) {
        self.cancel_jobs();
        *self = Self {
            open: self.open,
            view: self.view,
            hide_outside: self.hide_outside,
            playback_memory: self.playback_memory,
            colour_by: self.colour_by,
            band_filter: self.band_filter.clone(),
            reset_layers: true,
            ..Self::default()
        };
    }

    /// Takes in a new run: prepares it for display, and opens the panel.
    pub fn set_run(&mut self, app: &Application, run: Arc<Regeneration>) {
        self.cancel_jobs();
        *self = Self {
            open: true,
            view: self.view,
            show_tool: self.show_tool,
            hide_outside: self.hide_outside,
            playback_memory: self.playback_memory,
            budget: self.budget,
            colour_by: self.colour_by,
            band_filter: self.band_filter.clone(),
            reset_layers: true,
            run: Some(Arc::clone(&run)),
            data_job: Some(app.start_playback_data(run)),
            ..Self::default()
        };
    }

    /// Drops the run when it holds the path of an operation the project no
    /// longer has or has turned off, so that path leaves the view, the
    /// playback and the comparison with the model at once (issue 196, NFR-12).
    /// A run made before an ordinary edit stays, marked stale by the tree.
    pub fn forget_removed(&mut self, app: &Application) {
        let gone = self.run.as_ref().is_some_and(|run| {
            run.groups.iter().any(|g| {
                !app.project()
                    .operation(g.operation)
                    .is_some_and(|o| o.enabled)
            })
        });
        if gone {
            self.clear();
        }
    }

    /// Shows `run` again when the panel dropped it (see
    /// [`PlaybackPanel::forget_removed`]) and every operation in it is back,
    /// on and up to date, as after undoing a turn-off or a deletion (OI-692).
    /// Does nothing while the panel has a run, or when `run` holds an
    /// operation that is gone or has changed since it was made, so a run the
    /// tree marks stale is never shown again (OI-697).
    pub fn restore(&mut self, app: &Application, run: &Arc<Regeneration>) {
        if self.run.is_none() && openchuck_app::run_is_current(app.project(), run) {
            let open = self.open;
            self.set_run(app, Arc::clone(run));
            self.open = open;
        }
    }

    /// Whether the panel has a run to show.
    pub fn has_run(&self) -> bool {
        self.run.is_some()
    }

    /// Whether the part turns with A in the view ("Turn part with A", issue
    /// 203).
    pub fn turns_part_with_a(&self) -> bool {
        self.view == ViewFrame::Machine
    }

    /// Sets "Turn part with A".
    pub fn set_turn_part_with_a(&mut self, turn: bool) {
        self.view = if turn {
            ViewFrame::Machine
        } else {
            ViewFrame::Part
        };
    }

    /// Whether the path outside the playback window is hidden (issue 205).
    pub fn hides_outside_window(&self) -> bool {
        self.hide_outside
    }

    /// Sets "Hide path outside window".
    pub fn set_hide_outside_window(&mut self, hide: bool) {
        self.hide_outside = hide;
    }

    /// The memory playback's checkpoints may hold, in bytes.
    #[cfg(test)]
    pub(crate) fn playback_memory(&self) -> usize {
        self.playback_memory
    }

    /// Sets the memory playback's checkpoints may hold, in bytes (the user's
    /// setting; decision record 0322). It applies the next time the cut part
    /// is prepared.
    pub fn set_playback_memory(&mut self, bytes: usize) {
        self.playback_memory = bytes;
    }

    /// The playback window, if one is set (issue 205).
    pub fn window(&self) -> Option<MoveWindow> {
        self.playback.window()
    }

    /// Sets the playback window, or lifts it with `None`.
    pub fn set_window(&mut self, window: Option<MoveWindow>) {
        if let Some(data) = &self.data {
            self.playback.set_window(&data.timeline, window);
            self.window_numbers();
        }
    }

    /// Lifts the playback window: it is not remembered, so leaving Simulate
    /// resets it (the owner's answer on issue 205).
    pub fn reset_window(&mut self) {
        self.set_window(None);
    }

    /// The number fields follow the window, or the whole program.
    fn window_numbers(&mut self) {
        let moves = self.data.as_ref().map_or(0, |d| d.timeline.move_count());
        let (from, to) = match self.playback.window() {
            Some(w) => (w.first + 1, w.last + 1),
            None => (1, moves.max(1)),
        };
        self.window_from = from;
        self.window_to = to;
    }

    /// Where playback is, once the run is prepared.
    pub fn pose(&self) -> Option<PoseSample> {
        let data = self.data.as_ref()?;
        data.timeline.sample(self.playback.cursor()).ok()
    }

    /// Leaving Simulate keeps the play head at the result being inspected.
    pub fn pause(&mut self) {
        self.playback.pause();
    }

    /// Works the jobs, moves the cursor by `dt` seconds of the clock, and
    /// brings the viewport up to date. Returns true while the frame should be
    /// drawn again.
    pub fn frame(&mut self, app: &Application, viewport: &mut Viewport, dt: f64) -> bool {
        let mut busy = false;
        if std::mem::take(&mut self.reset_layers) {
            viewport.set_stock_surface(None);
            for layer in [Layer::Paths, Layer::Trace, Layer::Tool, Layer::Stock] {
                viewport.set_layer(layer, Scene::default());
            }
            viewport.set_bands(None);
            viewport.set_part_bands(None);
            viewport.set_part_changed(false);
            viewport.set_part_turn(0.0);
            viewport.set_tool_readout(None);
        }
        busy |= self.poll_data();
        // The cut part is prepared whenever it can be shown, on Simulate,
        // where "Cut part" draws it: the stock simulation is on by default
        // (issue 214), and no other step pays for it.
        self.stock.show = viewport.simulate_show().cut_part
            && viewport.step() == crate::viewport::ViewStep::Simulate;
        self.start_stock(app);
        self.follow_project(app);
        busy |= self.poll_lines(app, viewport);
        busy |= self.poll_trace(app, viewport);
        let Some(data) = self.data.clone() else {
            return busy || self.data_job.is_some();
        };
        if self.playback.advance(&data.timeline, dt) {
            busy |= self.playback.is_playing();
        }
        busy |= self.playback.is_playing();
        if let Ok(requested) = data.timeline.sample(self.playback.cursor()) {
            busy |= self.update_stock(app, viewport, &requested);
            // Publish the tool transform and cut surface for the same cursor.
            // Keep the last complete pair while the next surface is prepared.
            // While playing, the tool follows the clock and the cut part shows
            // the newest one made, behind it; when still, the tool waits for
            // the cut part at its cursor, as issue #333 asked (the owner's
            // decision D3 on issue #382; decision record 0322).
            if self.playback.is_playing() {
                self.stock.trailing = true;
            } else if self.stock.shown == Some(requested.cursor) || !self.stock.show {
                self.stock.trailing = false;
            }
            let cursor = if self.stock.show && !self.stock.trailing {
                self.stock.shown.unwrap_or(requested.cursor)
            } else {
                requested.cursor
            };
            let Ok(pose) = data.timeline.sample(cursor) else {
                return busy;
            };
            self.update_tool(viewport, &data, &pose);
            viewport.set_part_turn(pose.part_rotation(self.view));
            let placement = pose.placement(self.view);
            viewport.set_tool_transform(place(placement.origin, placement.axis));
            viewport.set_tool_readout(Some(ToolReadout {
                work: pose.work,
                move_number: pose.cursor.done + 1,
            }));
        }
        busy |= self.update_bands(app, viewport);
        busy
    }

    fn poll_data(&mut self) -> bool {
        let Some(job) = &self.data_job else {
            return false;
        };
        match job.try_result() {
            None => true,
            Some(result) => {
                self.data_job = None;
                match result {
                    Ok(data) => {
                        self.visible = vec![true; data.timeline.group_count()];
                        self.lines_dirty = true;
                        self.tool_dirty = true;
                        self.data = Some(data);
                        self.window_numbers();
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
                false
            }
        }
    }

    /// Takes which paths are drawn from the project (issue 213), and asks for
    /// the path again when that or the window changed.
    fn follow_project(&mut self, app: &Application) {
        let Some(data) = &self.data else { return };
        let timeline = &data.timeline;
        let visible: Vec<bool> = (0..timeline.group_count())
            .map(|g| {
                timeline
                    .operation(g)
                    .is_none_or(|op| app.project().path_shown(op))
            })
            .collect();
        if visible != self.visible {
            self.visible = visible;
            self.lines_dirty = true;
        }
        if self.playback.window() != self.drawn_window {
            self.drawn_window = self.playback.window();
            self.lines_dirty = true;
        }
    }

    /// Starts preparing the cut part (the stock at the play head) when the
    /// run was simulated and it is to be shown (issue 214).
    fn start_stock(&mut self, app: &Application) {
        let can = self.run.as_ref().is_some_and(|r| r.simulation.is_some());
        if let Some(data) = &self.data
            && can
            && self.stock.show
            && self.stock.playback.is_none()
            && self.stock.prepare.is_none()
            && self.stock.error.is_none()
        {
            self.stock.prepare =
                Some(app.start_stock_playback(Arc::clone(&data.timeline), self.playback_memory));
        }
    }

    /// Keeps the faint trace of the path outside the window in its layer: made
    /// once per path and visible groups, shown only while a window is set and
    /// the path outside it is not hidden (issue 205).
    fn poll_trace(&mut self, app: &Application, viewport: &mut Viewport) -> bool {
        let mut busy = false;
        if let Some((for_visible, job)) = &self.trace_job {
            match job.try_result() {
                None => busy = true,
                Some(result) => {
                    let for_visible = for_visible.clone();
                    self.trace_job = None;
                    match result {
                        Ok(lines) => {
                            self.trace_cost = Some((lines.points, lines.full_points));
                            self.trace = Some((for_visible, Arc::new(lines)));
                            self.trace_shown = false;
                        }
                        Err(e) if e.is_cancelled() => {}
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            }
        }
        let wanted = self.playback.window().is_some() && !self.hide_outside;
        let current = self
            .trace
            .as_ref()
            .filter(|(visible, _)| *visible == self.visible);
        match (wanted, current) {
            (true, Some((_, lines))) => {
                if !self.trace_shown {
                    viewport.set_layer(Layer::Trace, trace_scene(lines));
                    self.trace_shown = true;
                }
            }
            (true, None) => {
                if let Some(data) = &self.data
                    && self.trace_job.is_none()
                {
                    self.trace_job = Some((
                        self.visible.clone(),
                        app.start_trace_lines(
                            Arc::clone(&data.display),
                            self.visible.clone(),
                            self.budget,
                        ),
                    ));
                    busy = true;
                }
            }
            (false, _) => {
                if self.trace_shown {
                    viewport.set_layer(Layer::Trace, Scene::default());
                    self.trace_shown = false;
                }
            }
        }
        busy
    }

    fn poll_lines(&mut self, app: &Application, viewport: &mut Viewport) -> bool {
        // The level follows the zoom: a new one is made by a job only when
        // the zoom crosses into another level, and the path drawn stays
        // until the job hands over the next (decision record 0167, OI-177).
        let wanted = viewport
            .mm_per_pixel()
            .map_or(0, |mm| zoom_level(self.zoom_level, mm));
        if wanted != self.zoom_level {
            self.zoom_level = wanted;
            self.lines_dirty = true;
        }
        if let Some(job) = &self.lines_job {
            match job.try_result() {
                None => return true,
                Some(result) => {
                    self.lines_job = None;
                    match result {
                        Ok(lines) => {
                            self.detail = Some((lines.points, lines.full_points, lines.tolerance));
                            viewport.set_layer(Layer::Paths, paths_scene(&lines));
                        }
                        Err(e) if e.is_cancelled() => {}
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            }
        }
        if self.lines_dirty
            && let Some(data) = &self.data
        {
            self.lines_dirty = false;
            if let Some(job) = self.lines_job.take() {
                job.cancel();
            }
            let visible = self.visible.clone();
            // The detail follows the zoom in the window too (decision
            // records 0167 and 0169).
            self.lines_job = Some(match self.playback.window() {
                // Only the window's part, at full detail when it fits.
                Some(window) => app.start_window_lines(
                    Arc::clone(&data.display),
                    visible,
                    window.moves(),
                    self.budget,
                    self.zoom_level,
                ),
                None => app.start_path_lines(
                    Arc::clone(&data.display),
                    visible,
                    self.budget,
                    self.zoom_level,
                ),
            });
            return true;
        }
        false
    }

    fn update_tool(&mut self, viewport: &mut Viewport, data: &PlaybackData, pose: &PoseSample) {
        let group = self.show_tool.then_some(pose.group);
        if group == self.tool_group && !self.tool_dirty {
            return;
        }
        self.tool_group = group;
        self.tool_dirty = false;
        let scene = group
            .and_then(|g| data.timeline.tool(g))
            .and_then(tool_meshes)
            .map(|m| tool_scene(&m))
            .unwrap_or_default();
        viewport.set_layer(Layer::Tool, scene);
    }

    fn update_stock(
        &mut self,
        app: &Application,
        viewport: &mut Viewport,
        pose: &PoseSample,
    ) -> bool {
        let playing = self.playback.is_playing();
        let stock = &mut self.stock;
        let mut busy = false;
        if let Some(job) = &stock.prepare {
            match job.try_result() {
                None => busy = true,
                Some(result) => {
                    // Layout has already captured the viewport and dock for
                    // this frame. Draw again to show readiness or the error.
                    busy = true;
                    stock.prepare = None;
                    match result {
                        Ok(playback) => stock.playback = Some(Arc::new(playback)),
                        Err(e) => {
                            stock.error = Some(e.to_string());
                            stock.show = false;
                        }
                    }
                }
            }
        }
        if let Some((job, done)) = &stock.mesh {
            match job.try_result() {
                None => busy = true,
                Some(result) => {
                    // The paint callback still holds the previous surface.
                    // Even at the end, publish this one in another frame.
                    busy = true;
                    let done = *done;
                    stock.mesh = None;
                    match result {
                        Ok(surface) => {
                            let surface = Arc::new(surface);
                            viewport.set_stock_surface(Some(Arc::clone(&surface)));
                            stock.surface = Some(surface);
                            stock.shown = Some(done);
                        }
                        Err(e) if e.is_cancelled() => {}
                        Err(e) => {
                            stock.error = Some(e.to_string());
                            stock.surface = None;
                            stock.shown = None;
                            viewport.set_stock_surface(None);
                        }
                    }
                }
            }
        }
        if !stock.show {
            if stock.shown.take().is_some() {
                stock.surface = None;
                viewport.set_stock_surface(None);
            }
            return busy;
        }
        let done = pose.cursor;
        // While playing, frames are made without their picker, so they are
        // ready sooner; once playback is still, the frame shown is made
        // again with one, so pointing at it works (the owner's decision D5
        // on issue #382; decision record 0322).
        let cannot_pick = stock.surface.as_ref().is_some_and(|s| !s.can_pick());
        if let Some(playback) = &stock.playback
            && stock.mesh.is_none()
            && stock.error.is_none()
            && (stock.shown != Some(done) || (!playing && cannot_pick))
        {
            let job = if playing {
                app.start_playing_stock_surface(playback, done)
            } else {
                app.start_stock_surface(playback, done)
            };
            stock.mesh = Some((job, done));
            busy = true;
        }
        busy
    }

    /// Sets what the part is coloured by, and redraws the colours.
    pub fn set_colour_by(&mut self, by: ColourBy, viewport: &mut Viewport) {
        if self.colour_by != by {
            self.colour_by = by;
            self.show_bands(viewport);
            self.show_part_bands(viewport);
        }
    }

    /// The inspector's display choice stays accessible while results are
    /// stale, independently of the playback dock's height (issue 293).
    pub fn show_deviation_choice(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let available = self.run.as_ref().is_some_and(|r| r.simulation.is_some());
        let response = ui
            .add_enabled(
                available && self.stock.error.is_none(),
                egui::Checkbox::new(&mut self.bands_on, "Deviation colours"),
            )
            .on_hover_text(
                "Show overcut, on the surface, and material left; untick for the plain cut part",
            )
            .on_disabled_hover_text(self.stock.error.as_deref().unwrap_or(NO_STOCK_SIMULATION));
        if let Some(error) = &self.stock.error {
            ui.weak(format!("Deviation colours unavailable: {error}"));
        }
        response
    }

    /// Sets the deviation bands shown (SIM-15), and redraws the colours.
    pub fn set_band_filter(&mut self, filter: &BandFilter, viewport: &mut Viewport) {
        if self.band_filter != *filter {
            self.band_filter = filter.clone();
            self.show_bands(viewport);
            self.show_part_bands(viewport);
        }
    }

    /// Hands the viewport the triangles that `colour_by` and the band filter
    /// show, in their bands: the others are left out, so those triangles keep
    /// the part's own colour (SIM-15).
    fn show_bands(&self, viewport: &mut Viewport) {
        let (Some(bands), Some(model)) = (&self.bands, self.bands_model) else {
            return;
        };
        viewport.colour_by = self.colour_by;
        viewport.band_filter = self.band_filter.clone();
        viewport.set_bands(Some((
            model,
            shown_bands(bands, self.colour_by, &self.band_filter),
        )));
    }

    /// Gives the viewport the part's colours again, after the colour choice
    /// or the band filter changed (SIM-15).
    fn show_part_bands(&mut self, viewport: &mut Viewport) {
        let (_, shown) = self.part.pushed;
        self.part.pushed = (0, PartShown::Plain);
        if let PartShown::Bands(_) = shown {
            self.push_part(viewport, shown);
        }
    }

    /// The part's own measured colours on Simulate (SIM-05; decision 0492),
    /// for the setup the playback is in. They are made by a job off the
    /// interface thread, once for each run and setup, and shown only while
    /// the deviation colours are on, the part is ticked and the cut part has
    /// not failed (decision 0330: no colours anywhere then). A part that has
    /// changed since the run is drawn plain with a note, never with the old
    /// values (UI-11). Returns true while the job runs.
    fn update_part_colours(
        &mut self,
        app: &Application,
        viewport: &mut Viewport,
        setup: Option<SetupId>,
    ) -> bool {
        let run = self
            .run
            .clone()
            .filter(|run| run.simulation.is_some())
            .filter(|_| {
                self.bands_on && self.stock.error.is_none() && viewport.simulate_show().model
            });
        let (Some(run), Some(setup)) = (run, setup) else {
            if let Some((.., job)) = self.part.job.take() {
                job.cancel();
            }
            self.push_part(viewport, PartShown::Plain);
            return false;
        };
        if !self
            .part
            .run
            .as_ref()
            .is_some_and(|known| Arc::ptr_eq(known, &run))
        {
            if let Some((.., job)) = self.part.job.take() {
                job.cancel();
            }
            self.part.done.clear();
            self.part.run = Some(Arc::clone(&run));
        }
        // A scene read again is checked again: the job reads the file and
        // refuses one that is no longer the run's (decision 0492 item 7).
        if self.part.scene != viewport.scene_count() {
            if let Some((.., job)) = self.part.job.take() {
                job.cancel();
            }
            self.part.done.clear();
            self.part.scene = viewport.scene_count();
        }
        // The job on a setup the playback has left is not wanted now.
        if self
            .part
            .job
            .as_ref()
            .is_some_and(|(_, for_setup, _)| *for_setup != setup)
            && let Some((.., job)) = self.part.job.take()
        {
            job.cancel();
        }
        if openchuck_app::stockplay::part_changed_since(app.project(), &run, setup) {
            if let Some((.., job)) = self.part.job.take() {
                job.cancel();
            }
            self.push_part(viewport, PartShown::Changed);
            return false;
        }
        let mut busy = false;
        if let Some((.., job)) = &self.part.job {
            match job.try_result() {
                None => busy = true,
                Some(result) => {
                    self.part.job = None;
                    match result {
                        Ok(found) => self.part.done.push((setup, Some(found))),
                        // The file is not the run's: no colours for it.
                        Err(openchuck_app::AppError::PartChanged(_)) => {
                            self.part.done.push((setup, None));
                        }
                        Err(e) if e.is_cancelled() => {}
                        Err(e) => {
                            self.error = Some(e.to_string());
                            self.bands_on = false;
                        }
                    }
                }
            }
        }
        let found = self.part.done.iter().find(|(id, _)| *id == setup);
        match found {
            Some((_, Some(_))) => self.push_part(viewport, PartShown::Bands(setup)),
            Some((_, None)) => self.push_part(viewport, PartShown::Changed),
            None => {
                self.push_part(viewport, PartShown::Plain);
                if self.part.job.is_none() && self.bands_on {
                    self.part.job = Some((
                        Arc::clone(&run),
                        setup,
                        app.start_deviation_bands(Arc::clone(&run), setup),
                    ));
                    busy = true;
                }
            }
        }
        busy
    }

    /// Tells the viewport how the part is drawn, when that is not what it
    /// has been told for this run.
    fn push_part(&mut self, viewport: &mut Viewport, shown: PartShown) {
        let run = self
            .part
            .run
            .as_ref()
            .map_or(0, |r| Arc::as_ptr(r) as usize);
        let key = (run, shown);
        if self.part.pushed == key {
            return;
        }
        self.part.pushed = key;
        match shown {
            PartShown::Plain | PartShown::Changed => {
                viewport.set_part_bands(None);
                viewport.set_part_changed(shown == PartShown::Changed);
            }
            PartShown::Bands(setup) => {
                if let Some((_, Some((model, bands)))) =
                    self.part.done.iter().find(|(id, _)| *id == setup)
                {
                    viewport.colour_by = self.colour_by;
                    viewport.band_filter = self.band_filter.clone();
                    viewport.set_part_bands(Some((
                        *model,
                        shown_bands(bands, self.colour_by, &self.band_filter),
                    )));
                    viewport.set_part_changed(false);
                }
            }
        }
    }

    fn update_bands(&mut self, app: &Application, viewport: &mut Viewport) -> bool {
        if viewport.step() == crate::viewport::ViewStep::Simulate {
            if let Some((_, job)) = self.bands_job.take() {
                job.cancel();
            }
            let setup = self
                .run
                .as_ref()
                .zip(self.pose())
                .and_then(|(run, pose)| run.groups.get(pose.group).map(|g| g.setup));
            self.bands_setup = self.bands_on.then_some(setup).flatten();
            let busy = self.update_part_colours(app, viewport, setup);
            let surface = self
                .stock
                .surface
                .as_ref()
                .filter(|s| self.bands_on && Some(s.setup) == setup);
            if let Some(surface) = surface {
                if !self
                    .bands_surface
                    .as_ref()
                    .is_some_and(|old| Arc::ptr_eq(old, surface))
                {
                    self.bands = surface.bands.clone();
                    self.bands_model = surface.model;
                    self.bands_surface = Some(Arc::clone(surface));
                    self.show_bands(viewport);
                }
            } else {
                self.bands = None;
                self.bands_surface = None;
                self.bands_model = None;
                viewport.set_bands(None);
            }
            return busy;
        }
        // Off Simulate the part's own colours are not drawn.
        if let Some((.., job)) = self.part.job.take() {
            job.cancel();
        }
        self.push_part(viewport, PartShown::Plain);
        if self.bands_surface.take().is_some() {
            self.bands = None;
            self.bands_model = None;
            viewport.set_bands(None);
        }
        if !self.bands_on {
            if let Some((_, job)) = self.bands_job.take() {
                job.cancel();
            }
            self.bands_setup = None;
            if self.bands.take().is_some() {
                viewport.set_bands(None);
            }
            return false;
        }
        // The bands belong to the setup the playback is in: when it changes,
        // the old bands are dropped and the old job stopped, so the previous
        // setup's colours are never shown on the next one.
        let setup = self
            .run
            .as_ref()
            .zip(self.pose())
            .and_then(|(run, pose)| run.groups.get(pose.group).map(|info| info.setup));
        if self.bands_setup != setup {
            if let Some((_, job)) = self.bands_job.take() {
                job.cancel();
            }
            if self.bands.take().is_some() {
                viewport.set_bands(None);
            }
            self.bands_setup = setup;
        }
        if let Some((for_setup, job)) = &self.bands_job {
            let for_setup = *for_setup;
            return match job.try_result() {
                None => true,
                Some(result) => {
                    self.bands_job = None;
                    match result {
                        Ok((model, bands)) if Some(for_setup) == self.bands_setup => {
                            self.bands = Some(bands);
                            self.bands_model = Some(model);
                            self.show_bands(viewport);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            self.error = Some(e.to_string());
                            self.bands_on = false;
                        }
                    }
                    false
                }
            };
        }
        if self.bands.is_none()
            && let (Some(run), Some(setup)) = (&self.run, setup)
        {
            self.bands_job = Some((setup, app.start_deviation_bands(Arc::clone(run), setup)));
            return true;
        }
        false
    }

    /// Whether a run is being prepared for display. The controls under the
    /// transport row, the cut part's note among them, are drawn only once it
    /// is ready (OI-840).
    #[cfg(test)]
    pub fn is_preparing(&self) -> bool {
        self.data_job.is_some()
    }

    /// Whether the playback is running.
    #[cfg(test)]
    pub fn is_playing(&self) -> bool {
        self.playback.is_playing()
    }

    /// Whether group `group`'s path is drawn, as the project says.
    #[cfg(test)]
    pub fn draws_group(&self, group: usize) -> bool {
        self.visible.get(group).copied().unwrap_or(false)
    }

    /// Plays or pauses (Space).
    pub fn toggle_play(&mut self, timeline: &openchuck_app::playback::Timeline) {
        if self.playback.is_playing() {
            self.playback.pause();
        } else {
            self.playback.play(timeline);
        }
    }

    fn previous_operation(&mut self, timeline: &openchuck_app::playback::Timeline) {
        if let Some(g) = self.pose().map(|p| p.group).filter(|g| *g > 0) {
            self.playback.jump_to_group(timeline, g - 1);
        }
    }

    fn next_operation(&mut self, timeline: &openchuck_app::playback::Timeline) {
        if let Some(g) = self.pose().map(|p| p.group)
            && g + 1 < timeline.group_count()
        {
            self.playback.jump_to_group(timeline, g + 1);
        }
    }

    /// Space plays or pauses, the arrows step by a move and Shift with an
    /// arrow by an operation (SIM-04; specification section 16). Does
    /// nothing while a text field has the keyboard.
    pub fn handle_keys(&mut self, ctx: &egui::Context) {
        let Some(data) = self.data.clone() else {
            return;
        };
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let timeline = &data.timeline;
        let key = |k: egui::Key, shift: bool| {
            ctx.input_mut(|i| {
                i.consume_key(
                    if shift {
                        egui::Modifiers::SHIFT
                    } else {
                        egui::Modifiers::NONE
                    },
                    k,
                )
            })
        };
        if key(egui::Key::Space, false) {
            self.toggle_play(timeline);
        }
        if key(egui::Key::ArrowLeft, true) {
            self.previous_operation(timeline);
        } else if key(egui::Key::ArrowRight, true) {
            self.next_operation(timeline);
        } else if key(egui::Key::ArrowLeft, false) {
            self.playback.step(timeline, -1);
        } else if key(egui::Key::ArrowRight, false) {
            self.playback.step(timeline, 1);
        }
    }

    /// The timeline: one segment per enabled operation sized by its moves,
    /// the ones not generated dashed amber, the playhead, and move numbers
    /// under it. Clicking seeks.
    fn timeline_bar(
        &mut self,
        ui: &mut egui::Ui,
        app: &Application,
        data: &PlaybackData,
        progress: Option<&PreparationProgress>,
        bar: Option<&CutPartBar>,
    ) {
        let timeline = &data.timeline;
        let groups = timeline.group_count();
        let moves = timeline.move_count().max(1);
        // Operations enabled in the project that the run did not make.
        let made: Vec<_> = (0..groups).filter_map(|g| timeline.operation(g)).collect();
        let missing: Vec<&str> = app
            .project()
            .operations()
            .iter()
            .filter(|o| o.enabled && !made.contains(&o.id))
            .map(|o| o.name.as_str())
            .collect();
        let width = ui.available_width().max(160.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, 26.0), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let weights: Vec<f32> = (0..groups)
            .map(|g| {
                let start = timeline.first_move(g).unwrap_or(0);
                let end = timeline.first_move(g + 1).unwrap_or(moves);
                end.saturating_sub(start).max(1) as f32
            })
            .collect();
        let made_total: f32 = weights.iter().sum::<f32>().max(1.0);
        let slot = if groups > 0 {
            made_total / groups as f32
        } else {
            made_total
        };
        let all = made_total + slot * missing.len() as f32;
        let mut x = rect.left();
        let current = self.pose().map(|p| p.group);
        for (g, w) in weights.iter().enumerate() {
            let seg_w = rect.width() * w / all;
            let seg = egui::Rect::from_min_size(
                egui::pos2(x, rect.top()),
                egui::vec2(seg_w, rect.height()),
            );
            let fill = if current == Some(g) {
                DARK.accent
            } else {
                DARK.selected
            };
            painter.rect_filled(seg.shrink2(egui::vec2(0.5, 0.0)), 2.0, fill);
            painter.text(
                seg.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{} {}", g + 1, timeline.operation_name(g).unwrap_or("")),
                egui::FontId::proportional(12.0),
                DARK.text,
            );
            x += seg_w;
        }
        for name in &missing {
            let seg_w = rect.width() * slot / all;
            let seg = egui::Rect::from_min_size(
                egui::pos2(x, rect.top()),
                egui::vec2(seg_w, rect.height()),
            );
            painter.add(egui::Shape::dashed_line(
                &[
                    seg.left_top(),
                    seg.right_top(),
                    seg.right_bottom(),
                    seg.left_bottom(),
                    seg.left_top(),
                ],
                egui::Stroke::new(1.0, DARK.warning),
                4.0,
                3.0,
            ));
            painter.text(
                seg.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                format!("{name}: not generated"),
                egui::FontId::proportional(12.0),
                DARK.warning,
            );
            x += seg_w;
        }
        let made_width = rect.width() * made_total / all;
        // The playhead sits at the cursor's move, since segments are sized by moves.
        let done = self.pose().map_or(0, |p| p.cursor.done) as f32;
        let head = rect.left() + made_width * (done / moves as f32).clamp(0.0, 1.0);
        painter.vline(
            head,
            rect.y_range().expand(3.0),
            egui::Stroke::new(2.0, DARK.text),
        );
        if (response.clicked() || response.dragged())
            && let Some(pos) = response.interact_pointer_pos()
        {
            let f = ((pos.x - rect.left()) / made_width.max(1.0)).clamp(0.0, 1.0);
            self.playback
                .jump_to_move(timeline, (f * moves as f32) as usize);
        }
        // The pipeline's progress, directly under the position bar, takes the
        // cut-part bar's place while it is shown: the cut part on screen is
        // then from the run being replaced (decision record 0322 item 8).
        if let Some(progress) = progress {
            progress.show(ui);
        } else if let Some(bar) = bar {
            Self::cut_part_strip(ui, bar, rect.left(), made_width, moves);
        }
        self.window_strip(ui, timeline, rect.left(), made_width);
        ui.horizontal(|ui| {
            ui.label(RichText::new("0").small().monospace());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(moves.to_string()).small().monospace());
            });
        });
    }

    /// The playback window's strip under the timeline (issue 205): the window
    /// as a band with a handle at each end. Dragging an end resizes it,
    /// dragging the band moves it, and dragging elsewhere starts a window
    /// there. The number fields under it do the same from the keyboard.
    fn window_strip(
        &mut self,
        ui: &mut egui::Ui,
        timeline: &openchuck_app::playback::Timeline,
        left: f32,
        width: f32,
    ) {
        let moves = timeline.move_count();
        let full = ui.available_width().max(160.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(full, 8.0), egui::Sense::click_and_drag());
        let response = response.on_hover_text(
            "The playback window: drag its ends to resize it, or its middle to move it",
        );
        let painter = ui.painter_at(rect);
        let width = width.max(1.0);
        self.strip = (rect, left, width);
        let x_of = |m: usize| left + width * (m as f32 / moves.max(1) as f32);
        let window = self.playback.window();
        let (first, end) = window.map_or((0, moves), |w| (w.first, w.last + 1));
        let band = egui::Rect::from_x_y_ranges(x_of(first)..=x_of(end), rect.y_range());
        painter.rect_filled(rect, 2.0, DARK.selected.gamma_multiply(0.4));
        painter.rect_filled(band, 2.0, DARK.accent.gamma_multiply(0.6));
        for x in [band.left(), band.right()] {
            painter.rect_filled(
                egui::Rect::from_center_size(egui::pos2(x, rect.center().y), egui::vec2(4.0, 10.0)),
                1.0,
                DARK.text,
            );
        }
        let at = |pos: egui::Pos2| {
            (((pos.x - left) / width).clamp(0.0, 1.0) * moves as f32).round() as usize
        };
        // A drag is told from a click only once the pointer has moved, so it
        // starts where the button was pressed.
        if response.drag_started()
            && let Some(pos) = ui
                .input(|i| i.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos())
        {
            self.window_drag = Some(if (pos.x - band.left()).abs() <= HANDLE_REACH {
                WindowDrag::End { fixed: end }
            } else if (pos.x - band.right()).abs() <= HANDLE_REACH {
                WindowDrag::End { fixed: first }
            } else if band.x_range().contains(pos.x) && window.is_some() {
                WindowDrag::Body(at(pos) as isize - first as isize)
            } else {
                // A new window from here, made by dragging its end.
                let fixed = at(pos);
                self.playback.set_window(
                    timeline,
                    dragged_window(None, WindowDrag::End { fixed }, fixed, moves),
                );
                WindowDrag::End { fixed }
            });
        }
        if response.dragged()
            && let (Some(drag), Some(pos)) = (self.window_drag, response.interact_pointer_pos())
        {
            let next = dragged_window(self.playback.window(), drag, at(pos), moves);
            self.playback.set_window(timeline, next);
            self.window_numbers();
        }
        if response.drag_stopped() {
            self.window_drag = None;
            self.window_numbers();
        }
    }

    /// The window's ends as numbers, from the keyboard (issue 205).
    fn window_fields(&mut self, ui: &mut egui::Ui, timeline: &openchuck_app::playback::Timeline) {
        let moves = timeline.move_count().max(1);
        {
            ui.separator();
            ui.label("Window: moves");
            let from = ui
                .add(
                    egui::DragValue::new(&mut self.window_from)
                        .range(1..=moves)
                        .speed(1.0),
                )
                .on_hover_text("The first move in the playback window");
            ui.label("to");
            let to = ui
                .add(
                    egui::DragValue::new(&mut self.window_to)
                        .range(1..=moves)
                        .speed(1.0),
                )
                .on_hover_text("The last move in the playback window");
            if from.changed() || to.changed() {
                self.playback.set_window(
                    timeline,
                    Some(MoveWindow {
                        first: self.window_from.saturating_sub(1),
                        last: self.window_to.saturating_sub(1),
                    }),
                );
            }
            if from.lost_focus() || to.lost_focus() {
                self.window_numbers();
            }
            if ui
                .add_enabled(
                    self.playback.window().is_some(),
                    egui::Button::new("Whole program"),
                )
                .on_hover_text("Play and show every move again")
                .on_disabled_hover_text("The window already holds every move")
                .clicked()
            {
                self.playback.set_window(timeline, None);
                self.window_numbers();
            }
        }
    }

    /// Draws the controls: the strip that does not scroll, then the rest,
    /// as the dock does. Returns the project's new "Skip stock simulation"
    /// choice when the user changed it.
    #[cfg(test)]
    pub fn show(&mut self, ui: &mut egui::Ui, app: &Application) -> Option<bool> {
        let bar = self.cut_part_bar(&openchuck_app::JobLog::default());
        let skip = self.show_strip(ui, app, None, bar.as_ref());
        self.show_rest(ui, app);
        skip
    }

    /// The controls used most, which the dock keeps in a strip that does not
    /// scroll (issue 262): the transport row with "Skip stock simulation",
    /// and the timeline with the window strip. Returns the tick box's new
    /// choice when it was clicked.
    pub fn show_strip(
        &mut self,
        ui: &mut egui::Ui,
        app: &Application,
        progress: Option<&PreparationProgress>,
        bar: Option<&CutPartBar>,
    ) -> Option<bool> {
        let mut skip = None;
        let Some(data) = self.data.clone() else {
            // The transport row keeps its place while there is nothing to
            // play, so the tab does not move when a run lands (issue 215).
            let skip = self.waiting_row(ui, app);
            if let Some(progress) = progress {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 26.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 2.0, DARK.selected);
                progress.show(ui);
            } else if let Some(error) = &self.error {
                ui.colored_label(PROBLEM, error);
            } else {
                ui.label("Generate toolpaths to see and play them.");
            }
            return skip;
        };
        let timeline = &data.timeline;
        let current = self.pose().map(|p| p.group);
        ui.horizontal(|ui| {
            let group_count = timeline.group_count();
            let first = current.is_none_or(|g| g == 0);
            if ui
                .add_enabled(!first, egui::Button::new("⏮"))
                .on_hover_text("Previous operation (Shift ←)")
                .clicked()
            {
                self.previous_operation(timeline);
            }
            if ui
                .button("\u{23f4}")
                .on_hover_text("Step back one move (←)")
                .clicked()
            {
                self.playback.step(timeline, -1);
            }
            let label = if self.playback.is_playing() {
                "⏸"
            } else {
                "\u{23f5}"
            };
            if ui
                .button(label)
                .on_hover_text("Play or pause (Space)")
                .clicked()
            {
                self.toggle_play(timeline);
            }
            if ui
                .button("\u{23f5}|")
                .on_hover_text("Step on one move (→)")
                .clicked()
            {
                self.playback.step(timeline, 1);
            }
            let last = current.is_none_or(|g| g + 1 >= group_count);
            if ui
                .add_enabled(!last, egui::Button::new("⏭"))
                .on_hover_text("Next operation (Shift →)")
                .clicked()
            {
                self.next_operation(timeline);
            }
            ui.separator();
            skip = self.simulation_choices(ui, app);
        });
        self.timeline_bar(ui, app, &data, progress, bar);
        skip
    }

    /// The rest of the controls, under the strip: the speeds, go to move and
    /// the window, the readout, the view's tick boxes and the notes. Nothing
    /// while there is nothing to play.
    pub fn show_rest(&mut self, ui: &mut egui::Ui, app: &Application) {
        let Some(data) = self.data.clone() else {
            return;
        };
        let timeline = &data.timeline;
        let total = timeline.total_time();
        let moves = timeline.move_count();
        ui.horizontal_wrapped(|ui| {
            ui.label("Speed");
            let speed = self.playback.speed();
            for (name, value) in SPEEDS {
                if ui
                    .selectable_label((speed - value).abs() < 1e-9, name)
                    .clicked()
                {
                    self.playback.set_speed(value);
                }
            }
            ui.label("Whole program");
            for seconds in [10.0, 60.0] {
                let value = Playback::speed_to_finish_in(timeline, seconds);
                if ui
                    .selectable_label((speed - value).abs() < 1e-9, format!("{seconds:.0} s"))
                    .clicked()
                {
                    self.playback.set_speed(value);
                }
            }
            ui.separator();
            if ui.checkbox(&mut self.show_tool, "Tool").changed() {
                self.tool_dirty = true;
            }
            ui.add_enabled(
                self.playback.window().is_some(),
                egui::Checkbox::new(&mut self.hide_outside, HIDE_OUTSIDE),
            )
            .on_hover_text("Draw only the moves in the window; otherwise the rest of the path is drawn faintly. Drawing less is quicker on slow graphics")
            .on_disabled_hover_text("Set a window first, by dragging under the timeline");
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} / {}",
                clock(timeline.time_at(self.playback.cursor())),
                clock(total)
            ));
            ui.label("Go to move");
            ui.add(
                egui::DragValue::new(&mut self.jump_move)
                    .range(1..=moves.max(1))
                    .speed(1.0),
            );
            ui.label(format!("of {moves}"));
            if ui.button("Go").clicked() {
                self.playback
                    .jump_to_move(timeline, self.jump_move.saturating_sub(1));
            }
            self.window_fields(ui, timeline);
        });
        // The readout keeps its line before there is a pose (issue 215).
        if self.pose().is_none() {
            ui.monospace(" ");
        }
        if let Some(pose) = self.pose() {
            let kind = pose.kind.map_or("-", |k| match k {
                openchuck_app::playback::MoveKind::Rapid => "rapid",
                openchuck_app::playback::MoveKind::Linking => "link",
                openchuck_app::playback::MoveKind::Cutting => "cut",
                openchuck_app::playback::MoveKind::Plunge => "plunge",
                openchuck_app::playback::MoveKind::LeadIn => "lead-in",
                openchuck_app::playback::MoveKind::LeadOut => "lead-out",
            });
            let feed = pose
                .feed
                .map_or_else(String::new, |f| format!("   Feed {f:.0} mm/min"));
            // A reads 0 to 360 degrees; the tooltip gives the continuous
            // value the G-code holds, to compare with it (issue 207).
            ui.monospace(format!(
                "{}  move {}  {kind}   X{:.3} Y{:.3} Z{:.3} A{}{feed}",
                timeline.operation_name(pose.group).unwrap_or(""),
                pose.cursor.done.min(moves),
                pose.work[0],
                pose.work[1],
                pose.work[2],
                wrapped_a_text(pose.work[3], 3),
            ))
            .on_hover_text(continuous_a_tip(pose.work[3], 3));
        }
        ui.horizontal_wrapped(|ui| {
            self.stock_status(ui, app);
            if let Some((drawn, full, tolerance)) = self.detail {
                if drawn < full {
                    ui.weak(format!(
                        "Drawing {drawn} of {full} points, simplified to within {tolerance} mm"
                    ));
                } else {
                    ui.weak(format!("Drawing all {full} points"));
                }
            }
        });
        if let Some(error) = self.stock.error.as_ref().or(self.error.as_ref()) {
            ui.colored_label(PROBLEM, error);
        }
        let _ = DeviationBand::ALL;
    }

    /// What the cut part is doing: being prepared, or why there is none.
    fn stock_status(&self, ui: &mut egui::Ui, app: &Application) {
        if self.run.as_ref().is_some_and(|r| r.simulation.is_none()) {
            ui.weak(if app.project().skip_stock_simulation() {
                "No cut part: the stock simulation is skipped for this project"
            } else {
                "No cut part: generate again to simulate the stock"
            });
        }
    }
}

/// The triangles of `bands` that keep their deviation colour: those of the
/// bands `by` shows, whose values `filter` shows (SIM-05, SIM-15). Each
/// triangle's value is kept whole, for the probe.
pub fn shown_bands(bands: &DeviationBands, by: ColourBy, filter: &BandFilter) -> DeviationBands {
    let mut shown = bands.clone();
    shown.bands.retain(|(band, _)| by.shows(*band));
    if filter.hides_any() {
        for (_, triangles) in &mut shown.bands {
            triangles.retain(|t| {
                filter.shows(
                    bands.values.get(*t as usize).copied().flatten(),
                    bands.tolerance,
                )
            });
        }
        shown.bands.retain(|(_, triangles)| !triangles.is_empty());
    }
    shown
}

/// The transport row with nothing to play: the buttons in their places but
/// disabled, then the "Skip stock simulation" tick box, where the row with a
/// run has them (issues 215 and 259). Returns the tick box's new choice when
/// it was clicked.
impl PlaybackPanel {
    fn waiting_row(&mut self, ui: &mut egui::Ui, app: &Application) -> Option<bool> {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(false, |ui| {
                for button in ["⏮", "\u{23f4}", "\u{23f5}", "\u{23f5}|", "⏭"] {
                    ui.button(button)
                        .on_disabled_hover_text("There is nothing to play yet");
                }
            });
            ui.separator();
            self.simulation_choices(ui, app)
        })
        .inner
    }
}

/// The project's "Skip stock simulation" tick box (issue 214; decision
/// record 0168). Returns the new choice when it was clicked.
pub fn skip_box(ui: &mut egui::Ui, app: &Application) -> Option<bool> {
    let mut skip = app.project().skip_stock_simulation();
    ui.checkbox(&mut skip, SKIP_STOCK)
        .on_hover_text(
            "Generate only the toolpaths, for a quicker result. The cut is then not simulated, \
             so there is no cut part, no deviation and no simulated check before posting",
        )
        .changed()
        .then_some(skip)
}

/// Why the colours by deviation are unavailable: the run has no stock
/// simulation.
const NO_STOCK_SIMULATION: &str = "There is no stock simulation for this toolpath";

/// The A readout's tooltip: the continuous angle the G-code gives (issue
/// 207).
pub fn continuous_a_tip(a: f64, places: usize) -> String {
    format!("{CONTINUOUS_A}: {}°", continuous_a_text(a, places))
}

/// `h:mm:ss` for a number of seconds.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!(
        "{}:{:02}:{:02}",
        total / 3600,
        (total / 60) % 60,
        total % 60
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    /// The colour of a class of move.
    fn class_colour(class: PathClass) -> Colour {
        class_look(class).0
    }

    #[test]
    fn req_sim_04_the_clock_reads_hours_minutes_and_seconds() {
        assert_eq!(clock(0.0), "0:00:00");
        assert_eq!(clock(59.6), "0:01:00");
        assert_eq!(clock(3725.0), "1:02:05");
        assert_eq!(clock(-4.0), "0:00:00");
        assert_eq!(clock(f64::NAN), "0:00:00");
    }

    #[test]
    fn req_sim_01_each_class_of_move_has_its_own_colour() {
        let mut seen = Vec::new();
        for class in PathClass::ALL {
            let colour = class_colour(class);
            assert!(!seen.contains(&colour), "{class:?} repeats a colour");
            seen.push(colour);
        }
        // And more than colour (7.4): cut solid, link solid and thinner,
        // rapid dotted, entry solid; the tool a lilac tip on a grey body.
        let (cut, link, rapid) = (
            class_look(PathClass::Cut),
            class_look(PathClass::Link),
            class_look(PathClass::Rapid),
        );
        assert_eq!(cut.2, LineStyle::Solid);
        assert!(link.1 < cut.1 && link.2 == LineStyle::Solid);
        assert_eq!(rapid.2, LineStyle::Dotted);
        assert_eq!(class_look(PathClass::Entry).0, roles::ENTRY);
        assert_eq!(cut.0, roles::CUT);
        assert_eq!((CUTTER, SHANK), (roles::TOOL_TIP, roles::TOOL_BODY));
    }

    use std::time::{Duration, Instant};

    use openchuck_viewport::camera::{rigid_inverse, rotation_x};

    use crate::run_panel::RunPanel;
    use crate::testing::cube_project;
    use crate::viewport::SimulateShow;

    /// Generates with a simulation and prepares the display, polling as the
    /// shell does, until everything is ready.
    fn prepared(name: &str) -> (Application, PlaybackPanel, Viewport, std::path::PathBuf) {
        let (dir, app, _, _) = cube_project(name);
        let ctx = egui::Context::default();
        let mut run = RunPanel::default();
        run.start(&app, true);
        let started = Instant::now();
        let finished = loop {
            if let Some((r, _)) = run.poll(&openchuck_app::JobLog::default(), &ctx) {
                break r;
            }
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut panel = PlaybackPanel::default();
        let mut viewport = Viewport::new(true);
        panel.set_run(&app, finished);
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        (app, panel, viewport, dir)
    }

    #[test]
    fn req_sim_01_a_run_is_drawn_as_path_lines_in_the_colours_of_its_classes() {
        let (_app, panel, viewport, dir) = prepared("panel-paths");
        assert!(panel.open && panel.has_run());
        let paths = viewport.layer(Layer::Paths).expect("the path layer is set");
        assert!(!paths.lines.is_empty());
        // Every line is in a class colour, in the workpiece's slot, and
        // tested against depth (so the part can hide the path behind it).
        let colours: Vec<Colour> = PathClass::ALL.iter().map(|c| class_colour(*c)).collect();
        for line in &paths.lines {
            assert!(colours.contains(&line.colour), "{:?}", line.colour);
            assert_eq!(line.slot, Slot::Part);
            assert!(!line.on_top);
        }
        let (drawn, full, tolerance) = panel.detail.unwrap();
        assert_eq!(drawn, full, "a small job is drawn in full");
        assert!(tolerance.abs() < 1e-12);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Draws frames until the path-lines job is done.
    fn settle(app: &Application, panel: &mut PlaybackPanel, viewport: &mut Viewport) {
        let started = Instant::now();
        while panel.lines_job.is_some() || panel.lines_dirty {
            panel.frame(app, viewport, 0.0);
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn req_nfr_03_req_nfr_04_the_path_detail_follows_the_zoom_off_the_interface_thread() {
        // Decision 0167 (OI-177): zoomed far out, the path is drawn at a
        // coarser level; the level is made by a job, the old path stays drawn
        // until the new one arrives, and a zoom within a level makes nothing.
        let (app, mut panel, mut viewport, dir) = prepared("panel-zoom");
        let pose = panel.pose();
        assert_eq!(panel.zoom_level, 0, "no frame size yet: every point");
        let before = viewport.layer(Layer::Paths).unwrap() as *const Scene;
        viewport.set_pixels_for_tests([800, 600]);
        viewport.camera.projection = openchuck_viewport::Projection::Orthographic;
        viewport.camera.distance = 1.0e6;
        panel.frame(&app, &mut viewport, 0.0);
        let coarsest = openchuck_app::pathview::LEVEL_TOLERANCES.len() - 1;
        assert_eq!(panel.zoom_level, coarsest);
        // The frame only started the job; the path drawn is still the old one.
        assert!(panel.lines_job.is_some());
        assert_eq!(
            viewport.layer(Layer::Paths).unwrap() as *const Scene,
            before
        );
        settle(&app, &mut panel, &mut viewport);
        let (_, _, tolerance) = panel.detail.unwrap();
        assert_eq!(
            tolerance,
            openchuck_app::pathview::LEVEL_TOLERANCES[coarsest]
        );
        let coarse = viewport.layer(Layer::Paths).unwrap() as *const Scene;
        assert_ne!(coarse, before);
        // A zoom within the level: no job, the same path.
        viewport.camera.distance = 1.2e6;
        panel.frame(&app, &mut viewport, 0.0);
        assert!(panel.lines_job.is_none() && !panel.lines_dirty);
        assert_eq!(
            viewport.layer(Layer::Paths).unwrap() as *const Scene,
            coarse
        );
        // Zoomed right in, every point again.
        viewport.camera.distance = 0.01;
        panel.frame(&app, &mut viewport, 0.0);
        assert_eq!(panel.zoom_level, 0);
        settle(&app, &mut panel, &mut viewport);
        assert_eq!(panel.detail.unwrap().2, 0.0);
        // The level is for drawing only: playback is where it was.
        assert_eq!(panel.pose(), pose);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_nfr_12_req_ui_06_removing_or_disabling_an_operation_clears_its_path_from_the_view() {
        // Issue 196: a toolpath whose operation is gone is not drawn, played
        // or compared with the model.
        let (mut app, mut panel, mut viewport, dir) = prepared("panel-gone");
        let op = app.project().operations()[0].id;
        assert!(!viewport.layer(Layer::Paths).unwrap().lines.is_empty());

        // Nothing changed: the run stays.
        panel.forget_removed(&app);
        panel.frame(&app, &mut viewport, 0.0);
        assert!(panel.has_run());
        assert!(!viewport.layer(Layer::Paths).unwrap().lines.is_empty());

        // Turned off: the path goes from the view at once.
        app.execute(openchuck_app::Command::SetOperationEnabled {
            operation: op,
            enabled: false,
        })
        .unwrap();
        panel.forget_removed(&app);
        panel.frame(&app, &mut viewport, 0.0);
        assert!(!panel.has_run());
        assert!(viewport.layer(Layer::Paths).unwrap().lines.is_empty());
        assert!(viewport.layer(Layer::Tool).unwrap().meshes.is_empty());
        let _ = std::fs::remove_dir_all(dir);

        // Removed, rather than turned off.
        let (mut app, mut panel, mut viewport, dir) = prepared("panel-gone-2");
        let op = app.project().operations()[0].id;
        app.delete_operation(op).unwrap();
        panel.forget_removed(&app);
        panel.frame(&app, &mut viewport, 0.0);
        assert!(!panel.has_run());
        assert!(viewport.layer(Layer::Paths).unwrap().lines.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_02_the_tool_follows_the_cursor_and_the_machine_view_turns_the_part() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-tool");
        // "Turn part with A" is on at first (issue 203); this test starts in
        // the part's frame.
        assert!(panel.turns_part_with_a());
        panel.set_turn_part_with_a(false);
        // The tool is in its layer, in the tool slot: a cutter and a shank.
        let tool = viewport.layer(Layer::Tool).expect("the tool layer is set");
        assert_eq!(tool.meshes.len(), 2);
        assert!(tool.meshes.iter().all(|m| m.slot == Slot::Tool));
        let data = panel.data.clone().unwrap();
        // In the part's frame the part does not turn and the tool leans.
        panel.playback.seek(&data.timeline, 0.5);
        panel.frame(&app, &mut viewport, 0.0);
        let pose = panel.pose().unwrap();
        let (part, tool_at) = viewport.transforms();
        assert_eq!(part, openchuck_viewport::camera::IDENTITY);
        let placement = pose.placement(ViewFrame::Part);
        let expected = place(placement.origin, placement.axis);
        assert_eq!(tool_at, expected);
        // In the machine view the part turns by sign * A and the tool stands
        // upright at the turned point.
        panel.view = ViewFrame::Machine;
        panel.frame(&app, &mut viewport, 0.0);
        let (part, tool_at) = viewport.transforms();
        let turn = rotation_x(pose.part_turn.to_radians());
        for (c, column) in part.iter().enumerate() {
            for (r, value) in column.iter().enumerate() {
                assert!((value - turn[c][r]).abs() < 1e-12);
            }
        }
        // The tool's transform maps the tip (the origin) to the turned tip and
        // keeps +Z as the axis.
        let turned = pose.turned_tip;
        assert!((tool_at[3][0] - turned.x).abs() < 1e-12);
        assert!((tool_at[3][1] - turned.y).abs() < 1e-12);
        assert!((tool_at[3][2] - turned.z).abs() < 1e-12);
        assert!((tool_at[2][2] - 1.0).abs() < 1e-12);
        // Rigid: its inverse undoes it.
        let back = rigid_inverse(&tool_at);
        assert!(
            (back[3][0]
                + (tool_at[3][0] * back[0][0]
                    + tool_at[3][1] * back[1][0]
                    + tool_at[3][2] * back[2][0]))
                .abs()
                < 1e-9
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_04_playing_moves_the_cursor_with_the_frame_time_and_stops_at_the_end() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-play");
        // Without the cut part, whose meshes would keep the frame busy.
        viewport.set_simulate_show(SimulateShow {
            model: true,
            cut_part: false,
            toolpath: true,
        });
        let data = panel.data.clone().unwrap();
        panel
            .playback
            .set_speed(Playback::speed_to_finish_in(&data.timeline, 1.0));
        panel.playback.play(&data.timeline);
        // The frame asks to be drawn again while it plays.
        assert!(panel.frame(&app, &mut viewport, 0.25));
        let quarter = panel.playback.cursor();
        assert!(quarter.done > 0 && quarter.done < data.timeline.move_count());
        let t = data.timeline.time_at(quarter);
        assert!((t / data.timeline.total_time() - 0.25).abs() < 0.02, "{t}");
        // Past the end it stops, and the frame does not ask for more.
        panel.frame(&app, &mut viewport, 5.0);
        assert_eq!(panel.playback.cursor().done, data.timeline.move_count());
        assert!(!panel.playback.is_playing());
        assert!(!panel.frame(&app, &mut viewport, 0.1));
        // Clearing forgets the run and empties the layers on the next frame.
        panel.clear();
        panel.frame(&app, &mut viewport, 0.0);
        assert!(!panel.has_run());
        assert!(viewport.layer(Layer::Paths).unwrap().lines.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_03_req_sim_04_final_cut_part_is_published_after_jump_to_end() {
        final_cut_part_at_end(0);
    }

    #[test]
    fn req_sim_03_req_sim_04_final_cut_part_is_published_after_step_to_end() {
        final_cut_part_at_end(1);
    }

    #[test]
    fn req_sim_03_req_sim_04_final_cut_part_is_published_after_play_to_end() {
        final_cut_part_at_end(2);
    }

    #[test]
    fn req_sim_03_req_sim_04_req_ui_02_final_cut_part_is_drawn_after_dragging_playhead_to_end() {
        final_cut_part_at_end(3);
    }

    fn final_cut_part_at_end(route: usize) {
        let (app, mut panel, mut viewport, dir) = prepared(&format!("panel-final-stock-{route}"));
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        let data = panel.data.clone().unwrap();
        // Allow coverage instrumentation overhead without changing the work performed.
        let deadline = Instant::now() + Duration::from_secs(600);
        for case in 0..4 {
            panel.bands_on = case >= 2;
            assert!(openchuck_app::run_is_current(
                app.project(),
                panel.run.as_ref().unwrap()
            ));
            panel.playback.seek(&data.timeline, 0.0);
            while panel.frame(&app, &mut viewport, 0.0) {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            if case % 2 == 0 {
                // Also reach the end before the first stock surface is ready.
                panel.cancel_jobs();
                panel.stock = StockState::default();
                viewport.set_stock_surface(None);
            } else {
                // Reach the end while an earlier cursor's surface job is pending.
                panel.playback.seek(&data.timeline, 0.4);
                panel.frame(&app, &mut viewport, 0.0);
                assert!(panel.stock.mesh.is_some());
            }
            match route {
                0 => panel
                    .playback
                    .jump_to_move(&data.timeline, data.timeline.move_count() - 1),
                1 => panel
                    .playback
                    .step(&data.timeline, data.timeline.move_count() as isize),
                2 => {
                    panel.playback.play(&data.timeline);
                    panel.frame(&app, &mut viewport, data.timeline.total_time() + 1.0);
                }
                _ => {
                    let ctx = egui::Context::default();
                    let draw = |panel: &mut PlaybackPanel, events| {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(900.0, 400.0),
                                )),
                                events,
                                ..Default::default()
                            },
                            |ui| panel.timeline_bar(ui, &app, &data, None, None),
                        );
                        output.textures_delta.clear();
                    };
                    draw(&mut panel, Vec::new());
                    let (strip, left, width) = panel.strip;
                    let y = strip.top() - ctx.style_of(ctx.theme()).spacing.item_spacing.y - 13.0;
                    let from = egui::pos2(left + width * 0.4, y);
                    let to = egui::pos2(left + width + 2.0, y);
                    let button = |pos, pressed| egui::Event::PointerButton {
                        pos,
                        pressed,
                        button: egui::PointerButton::Primary,
                        modifiers: egui::Modifiers::NONE,
                    };
                    draw(
                        &mut panel,
                        vec![egui::Event::PointerMoved(from), button(from, true)],
                    );
                    draw(&mut panel, vec![egui::Event::PointerMoved(to)]);
                    draw(&mut panel, vec![button(to, false)]);
                }
            }
            // Shell::draw lays out the viewport before run_frame polls playback.
            // Stop drawing when playback stops requesting frames, just as the UI does.
            let ctx = egui::Context::default();
            let drawn_surface = loop {
                let drawn_surface = viewport.stock_surface().cloned();
                let mut busy = false;
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    viewport.show(ui);
                    busy = panel.frame(&app, &mut viewport, 0.0);
                });
                output.textures_delta.clear();
                if !busy {
                    break drawn_surface;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(panel.playback.cursor().done, data.timeline.move_count());
            assert_eq!(panel.stock.shown, Some(panel.playback.cursor()));
            let expected = app
                .start_stock_surface(
                    panel.stock.playback.as_ref().unwrap(),
                    panel.playback.cursor(),
                )
                .wait()
                .unwrap();
            assert_eq!(viewport.stock_surface().unwrap().as_ref(), &expected);
            assert!(
                drawn_surface
                    .as_ref()
                    .is_some_and(|s| s.as_ref() == &expected),
                "the final cut surface must be handed to the renderer without another user action"
            );
            assert!(!viewport.layer(Layer::Stock).unwrap().meshes.is_empty());
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_05_the_stock_and_the_deviation_colours_come_from_jobs() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-stock");
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        let data = panel.data.clone().unwrap();
        // Ask for the stock: the job prepares the checkpoints, then the
        // mesh at the cursor is made for the moves done.
        panel.stock.show = true;
        panel.stock.prepare =
            Some(app.start_stock_playback(Arc::clone(&data.timeline), panel.playback_memory));
        panel.playback.seek(&data.timeline, 0.6);
        let started = Instant::now();
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        let stock = viewport
            .layer(Layer::Stock)
            .expect("the stock layer is set");
        assert_eq!(stock.meshes.len(), 1);
        assert!(stock.meshes[0].translucent && stock.meshes[0].slot == Slot::Part);
        assert_eq!(panel.stock.shown, Some(panel.playback.cursor()));
        // Unticking "Cut part" empties the layer.
        viewport.set_simulate_show(SimulateShow {
            model: true,
            cut_part: false,
            toolpath: false,
        });
        panel.frame(&app, &mut viewport, 0.0);
        assert!(viewport.layer(Layer::Stock).unwrap().meshes.is_empty());
        // With Cut part hidden its colours are hidden too. Showing it again
        // maps the final samples onto the actual stock surface.
        panel.bands_on = true;
        panel.frame(&app, &mut viewport, 0.0);
        assert!(panel.bands.is_none());
        viewport.set_simulate_show(SimulateShow::default());
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(panel.bands.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The owner's decision D5 on issue #382 (decision record 0322): frames
    /// shown while playing have no picker; once playback is still, the frame
    /// shown is made again with one, and picks as before.
    #[test]
    fn req_sim_11_req_nfr_04_the_cut_part_is_pickable_once_playback_is_still() {
        let (app, mut panel, mut viewport, dir) = prepared("pick-when-still");
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        let data = panel.data.clone().unwrap();
        panel.stock.show = true;
        panel.stock.prepare =
            Some(app.start_stock_playback(Arc::clone(&data.timeline), panel.playback_memory));
        panel.playback.seek(&data.timeline, 0.3);
        panel.playback.set_speed(1.0);
        panel.playback.play(&data.timeline);
        let started = Instant::now();
        // Play until a frame made while playing is shown (the next one is
        // asked for at once, so a job is always running).
        while panel.stock.surface.is_none() {
            assert!(started.elapsed() < Duration::from_secs(120));
            panel.frame(&app, &mut viewport, 0.001);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(panel.playback.is_playing());
        let playing = panel.stock.surface.clone().unwrap();
        assert!(!playing.can_pick());
        assert!(
            playing
                .pick([0.0, 0.0, 50.0], [0.0, 0.0, -1.0])
                .unwrap()
                .is_none()
        );
        // Pause: the frame at the cursor is made again with its picker.
        panel.playback.pause();
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        let cursor = panel.playback.cursor();
        assert_eq!(panel.stock.shown, Some(cursor));
        let still = panel.stock.surface.clone().unwrap();
        assert!(still.can_pick());
        let expected = app
            .start_stock_surface(panel.stock.playback.as_ref().unwrap(), cursor)
            .wait()
            .unwrap();
        assert_eq!(still.as_ref(), &expected);
        assert_eq!(viewport.stock_surface().unwrap().as_ref(), &expected);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The owner's decision D3 on issue #382 (decision record 0322): while
    /// playing, the tool follows the clock and the cut part shows the newest
    /// one made, behind it; the bar says how far. After a pause the tool
    /// stays where it is and the cut part catches up with it.
    #[test]
    fn req_sim_02_req_sim_04_while_playing_the_tool_follows_the_clock() {
        let (app, mut panel, mut viewport, dir) = prepared("tool-follows-clock");
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        let data = panel.data.clone().unwrap();
        panel.stock.show = true;
        panel.stock.playback = Some(Arc::new(
            app.start_stock_playback(Arc::clone(&data.timeline), panel.playback_memory)
                .wait()
                .unwrap(),
        ));
        panel.playback.seek(&data.timeline, 0.2);
        let started = Instant::now();
        while panel.stock.shown.is_none() {
            assert!(started.elapsed() < Duration::from_secs(120));
            panel.frame(&app, &mut viewport, 0.0);
            std::thread::sleep(Duration::from_millis(5));
        }
        // Block the next cut part, then play: the tool moves on with the
        // clock while the cut part on screen stays where it was.
        let shown = panel.stock.shown.unwrap();
        let pool = openchuck_app::JobManager::new(openchuck_app::Threads::Exactly(1)).unwrap();
        let (events, _) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel::<()>();
        let surface = app
            .start_playing_stock_surface(panel.stock.playback.as_ref().unwrap(), shown)
            .wait()
            .unwrap();
        panel.stock.mesh = Some((
            pool.submit("delayed surface", events, move |_| {
                blocked.recv().unwrap();
                Ok(surface)
            }),
            shown,
        ));
        panel
            .playback
            .set_speed(Playback::speed_to_finish_in(&data.timeline, 10.0));
        panel.playback.play(&data.timeline);
        for _ in 0..5 {
            panel.frame(&app, &mut viewport, 0.2);
        }
        let head = panel.playback.cursor();
        assert!(panel.playback.is_playing() && head.done > shown.done);
        assert_eq!(panel.stock.shown, Some(shown));
        assert_eq!(
            viewport.tool_readout.as_ref().unwrap().move_number,
            head.done + 1,
            "the tool is at the play head, not held at the cut part"
        );
        let bar = panel
            .cut_part_bar(&openchuck_app::JobLog::default())
            .unwrap();
        assert_eq!((bar.shown, bar.head), (Some(shown.done), head.done));
        assert!(
            bar.text().unwrap().starts_with("Cut part ")
                && bar.text().unwrap().ends_with(" s behind")
        );
        // Pause: the tool stays at the play head, and the cut part catches up.
        panel.playback.pause();
        let paused = panel.playback.cursor();
        release.send(()).unwrap();
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            assert_eq!(
                viewport.tool_readout.as_ref().unwrap().move_number,
                paused.done + 1
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(panel.stock.shown, Some(paused));
        assert!(panel.stock.surface.as_ref().unwrap().can_pick());
        let bar = panel
            .cut_part_bar(&openchuck_app::JobLog::default())
            .unwrap();
        assert_eq!(bar.text(), None, "current: no words");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_04_the_cut_part_bar_says_what_it_shows() {
        let bar = |behind, loading, preparing| CutPartBar {
            shown: Some(10),
            head: 20,
            behind,
            loading,
            preparing,
        };
        assert_eq!(bar(None, None, None).text(), None);
        assert_eq!(
            bar(Some(12.4), Some((15, Some(0.4))), None).text().unwrap(),
            "Cut part 12 s behind"
        );
        assert_eq!(
            bar(Some(0.2), None, None).text().unwrap(),
            "Cut part 1 s behind"
        );
        assert_eq!(
            bar(None, Some((20, Some(0.409))), None).text().unwrap(),
            "Updating cut part 40%"
        );
        assert_eq!(
            bar(None, Some((20, None)), None).text().unwrap(),
            "Updating cut part"
        );
        assert_eq!(
            bar(None, None, Some(Some(0.42))).text().unwrap(),
            "Preparing the cut part 42%"
        );
        assert_eq!(
            bar(None, None, Some(None)).text().unwrap(),
            "Preparing the cut part"
        );
    }

    #[test]
    fn req_sim_02_req_sim_03_tool_and_stock_share_a_cursor_while_a_job_is_pending() {
        let (app, mut panel, mut viewport, dir) = prepared("synchronized-stock");
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        let data = panel.data.clone().unwrap();
        let started = Instant::now();
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        let shown = panel.stock.shown.unwrap();
        let old_pose = data.timeline.sample(shown).unwrap();
        panel.playback.seek(&data.timeline, 0.6);
        let target = panel.playback.cursor();
        assert_ne!(shown, target);
        let surface = app
            .start_stock_surface(panel.stock.playback.as_ref().unwrap(), target)
            .wait()
            .unwrap();
        let pool = openchuck_app::JobManager::new(openchuck_app::Threads::Exactly(1)).unwrap();
        let (events, _) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        panel.stock.mesh = Some((
            pool.submit("delayed surface", events, move |_| {
                blocked.recv().unwrap();
                Ok(surface)
            }),
            target,
        ));
        panel.frame(&app, &mut viewport, 0.0);
        assert_eq!(panel.stock.shown, Some(shown));
        assert_eq!(viewport.tool_readout.as_ref().unwrap().work, old_pose.work);
        release.send(()).unwrap();
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(panel.stock.shown, Some(target));
        assert_eq!(
            viewport.tool_readout.as_ref().unwrap().work,
            data.timeline.sample(target).unwrap().work
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_05_req_ui_11_req_nfr_12_req_gc_15_failed_surface_explains_colours() {
        failed_stock_preparation_explains_colours(false);
    }

    #[test]
    fn req_sim_05_req_ui_11_req_nfr_12_req_gc_15_checkpoint_memory_failure_explains_colours() {
        failed_stock_preparation_explains_colours(true);
    }

    fn failed_stock_preparation_explains_colours(checkpoint: bool) {
        let (mut app, mut panel, mut viewport, dir) = prepared(if checkpoint {
            "panel-checkpoint-failure"
        } else {
            "panel-surface-failure"
        });
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        let data = panel.data.clone().unwrap();
        let export_before = app
            .export_checks(panel.run.as_deref(), None, None)
            .can_write();
        let message = if checkpoint {
            "stock playback would keep at least 2000 MB of checkpoints across the setups, over the allowed memory; make the simulation cell coarser or play the toolpath without the stock"
        } else {
            "The cut surface could not be prepared"
        };
        let pool = openchuck_app::JobManager::new(openchuck_app::Threads::Exactly(1)).unwrap();
        let (events, _) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        if checkpoint {
            // Inject the memory-bound error at the checkpoint job boundary;
            // allocating 1.5 GiB is neither needed nor safe for this UI test.
            panel.stock.prepare = Some(pool.submit("prepare playback", events, move |_| {
                blocked.recv().unwrap();
                Err(openchuck_app::AppError::Playback(message.to_owned()))
            }));
        } else {
            panel.stock.playback = Some(Arc::new(
                app.start_stock_playback(Arc::clone(&data.timeline), panel.playback_memory)
                    .wait()
                    .unwrap(),
            ));
            panel.stock.mesh = Some((
                pool.submit("stock at move", events, move |_| {
                    blocked.recv().unwrap();
                    Err(openchuck_app::AppError::Playback(message.to_owned()))
                }),
                Cursor::default(),
            ));
        }
        panel.frame(&app, &mut viewport, 0.0);
        // The cut part being prepared is shown in the cut-part bar
        // (decision record 0322 item 8).
        let bar = panel
            .cut_part_bar(&openchuck_app::JobLog::default())
            .unwrap();
        assert!(bar.preparing.is_some() || bar.loading.is_some());
        assert!(bar.text().unwrap().contains("cut part"));
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        while panel.stock.error.is_none() {
            panel.frame(&app, &mut viewport, 0.0);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let run = panel.run.clone().unwrap();
        for stale in [false, true] {
            if stale {
                let operation = app.project().operations()[0].id;
                app.set_parameter(
                    operation,
                    "stepover",
                    Some(openchuck_app::project::ParameterValue::Number(0.6)),
                )
                .unwrap();
            }
            for _ in 0..3 {
                panel.frame(&app, &mut viewport, 0.0);
            }
            assert!(
                panel.stock.prepare.is_none() && panel.stock.mesh.is_none(),
                "no automatic retry"
            );
            assert!(
                panel
                    .preparation_progress(&openchuck_app::JobLog::default(), None, false)
                    .is_none()
            );
            assert!(panel.stock.error.as_ref().unwrap().contains(message));
            assert!(viewport.stock_surface().is_none() && viewport.bands().is_none());
            assert_eq!(openchuck_app::run_is_current(app.project(), &run), !stale);
            if stale {
                assert!(!app.export_checks(Some(&run), None, None).can_write());
            } else {
                assert_eq!(
                    app.export_checks(Some(&run), None, None).can_write(),
                    export_before
                );
            }
            assert!(Arc::ptr_eq(panel.run.as_ref().unwrap(), &run));
            // Check the actual inspector and dock text, not just the error field.
            let ctx = egui::Context::default();
            let mut enabled = true;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                enabled = panel.show_deviation_choice(ui).enabled();
                panel.show_rest(ui, &app);
            });
            output.textures_delta.clear();
            assert!(
                !enabled,
                "a failed cut surface cannot carry deviation colours"
            );
            fn texts(shape: &egui::Shape, out: &mut String) {
                match shape {
                    egui::Shape::Text(t) => out.push_str(t.galley.text()),
                    egui::Shape::Vec(v) => {
                        for s in v {
                            texts(s, out);
                        }
                    }
                    _ => {}
                }
            }
            let mut text = String::new();
            for shape in output.shapes {
                texts(&shape.shape, &mut text);
            }
            assert!(text.contains("Deviation colours unavailable"), "{text}");
            assert!(
                text.contains(message),
                "the preparation error stays visible: {text}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Bands belong to the setup playback is in: when it changes, the old
    /// bands are dropped from the viewport and the old job is stopped.
    #[test]
    fn req_sim_05_colour_by_leaves_out_the_bands_it_does_not_show() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-colour-by");
        panel.bands_on = true;
        let started = Instant::now();
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        let all = viewport.bands().unwrap().bands.len();
        assert!(all > 0);
        panel.set_colour_by(ColourBy::Overcut, &mut viewport);
        let shown = viewport.bands().unwrap();
        assert!(
            shown
                .bands
                .iter()
                .all(|(b, _)| matches!(b, DeviationBand::Overcut | DeviationBand::Unmeasured))
        );
        panel.set_colour_by(ColourBy::Deviation, &mut viewport);
        assert_eq!(viewport.bands().unwrap().bands.len(), all);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_05_the_colour_choice_survives_a_new_run_and_a_clear() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-colour-keeps");
        panel.set_colour_by(ColourBy::Remaining, &mut viewport);
        let run = Arc::clone(panel.run.as_ref().unwrap());
        panel.clear();
        assert_eq!(panel.colour_by, ColourBy::Remaining);
        panel.set_run(&app, run);
        assert_eq!(panel.colour_by, ColourBy::Remaining);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_05_deviation_bands_do_not_outlive_their_setup() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-bands-setup");
        let started = Instant::now();
        panel.bands_on = true;
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(panel.bands.is_some() && panel.bands_setup.is_some());
        let first = panel.bands_setup;
        // The playback moves into another setup (the run says the group's
        // setup is a different one): the bands for the first are dropped at
        // once, and the panel works for the new setup.
        let mut moved = (**panel.run.as_ref().unwrap()).clone();
        let other = SetupId::new();
        moved.groups[0].setup = other;
        panel.run = Some(Arc::new(moved));
        panel.frame(&app, &mut viewport, 0.0);
        assert!(panel.bands.is_none(), "the old setup's bands are gone");
        assert_ne!(panel.bands_setup, first);
        assert_eq!(panel.bands_setup, Some(other));
        // Turning the colouring off clears everything.
        panel.bands_on = false;
        panel.frame(&app, &mut viewport, 0.0);
        assert!(panel.bands_job.is_none() && panel.bands_setup.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A prepared run on Simulate with only the Part ticked, so no cut part
    /// is made, and the deviation colours on.
    fn part_only(name: &str) -> (Application, PlaybackPanel, Viewport, std::path::PathBuf) {
        let (app, mut panel, mut viewport, dir) = prepared(name);
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        viewport.set_simulate_show(SimulateShow {
            model: true,
            cut_part: false,
            toolpath: false,
        });
        panel.bands_on = true;
        (app, panel, viewport, dir)
    }

    /// Decision 0492: the part is coloured by the values measured on its own
    /// triangles (decisions 0108, 0194), by a job, and the cut part keeps its
    /// own projected values beside them.
    #[test]
    fn req_sim_05_the_part_is_coloured_by_its_own_measured_values() {
        let (app, mut panel, mut viewport, dir) = part_only("panel-part-own");
        settle_panel(&app, &mut panel, &mut viewport);
        let setup = app.project().setups()[0].id;
        let run = Arc::clone(panel.run.as_ref().unwrap());
        let (model, measured) = app
            .start_deviation_bands(Arc::clone(&run), setup)
            .wait()
            .unwrap();
        let shown = shown_bands(&measured, ColourBy::default(), &BandFilter::default());
        assert_eq!(viewport.part_bands(), Some(&shown));
        assert!(viewport.part_coloured() && !viewport.part_changed());
        let _ = model;
        // One value for each of the cube's twelve triangles: the part's
        // own, not the cut surface's projected values.
        assert_eq!(shown.values.len(), 12);
        assert!(viewport.bands().is_none(), "no cut part is asked for");
        // The job ran once: another frame does not start another.
        panel.frame(&app, &mut viewport, 0.0);
        assert!(panel.part.job.is_none());
        // Ticking Cut part prepares it and gives it its projected values,
        // while the part keeps its own.
        viewport.set_simulate_show(SimulateShow {
            model: true,
            cut_part: true,
            toolpath: false,
        });
        settle_panel(&app, &mut panel, &mut viewport);
        let cut = viewport.bands().expect("the cut part is coloured too");
        assert!(
            cut.values.len() > 100,
            "the cut surface has its own triangles"
        );
        assert_eq!(viewport.part_bands(), Some(&shown));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Decision 0492, items 1 and 2 of its colours: with the part and the
    /// cut part shown, the cut part is drawn plain at the stock's own
    /// translucency, even in Solid, so the coloured part shows through.
    #[test]
    fn req_sim_05_with_part_and_cut_part_shown_the_cut_part_is_plain_and_translucent() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-cut-plain");
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        viewport.set_simulate_show(SimulateShow {
            model: true,
            cut_part: true,
            toolpath: false,
        });
        panel.bands_on = true;
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(viewport.part_coloured() && viewport.bands().is_some());
        for solid in [false, true] {
            viewport.set_cut_part_solid(solid);
            let stock = viewport.layer(Layer::Stock).unwrap();
            assert_eq!(
                stock.meshes.len(),
                1,
                "one plain mesh, no bands (solid={solid})"
            );
            let mesh = &stock.meshes[0];
            assert!(mesh.translucent, "solid={solid}");
            assert_eq!(mesh.colour, [0.85, 0.72, 0.45, 0.30], "solid={solid}");
            assert!(mesh.hatch.is_none());
        }
        // With the part unticked the cut part carries the colours again, as
        // it did before.
        viewport.set_simulate_show(SimulateShow {
            model: false,
            cut_part: true,
            toolpath: false,
        });
        panel.frame(&app, &mut viewport, 0.0);
        let stock = viewport.layer(Layer::Stock).unwrap();
        assert!(!viewport.part_coloured());
        assert!(
            stock.meshes.len() > 1 || stock.meshes[0].colour != [0.85, 0.72, 0.45, 0.30],
            "the cut part is coloured by its bands"
        );
        assert!(stock.meshes.iter().all(|m| !m.translucent), "solid is kept");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Decision 0330 and 0492: when the cut part could not be prepared the
    /// deviation colours are off everywhere, the part's included.
    #[test]
    fn req_sim_05_no_colours_anywhere_when_cut_part_preparation_failed() {
        let (app, mut panel, mut viewport, dir) = part_only("panel-no-colours");
        panel.stock.error = Some("the cut part could not be prepared".to_owned());
        for _ in 0..3 {
            panel.frame(&app, &mut viewport, 0.0);
        }
        assert!(panel.part.job.is_none(), "no measuring is started");
        assert!(viewport.part_bands().is_none() && viewport.bands().is_none());
        assert!(!viewport.part_changed());
        // Colours already shown go with the failure.
        panel.stock.error = None;
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(viewport.part_bands().is_some());
        panel.stock.error = Some("the cut part could not be prepared".to_owned());
        panel.frame(&app, &mut viewport, 0.0);
        assert!(viewport.part_bands().is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// SIM-15 on the part: the colour choice and the band filter leave out
    /// the bands and values they hide, as on the cut part.
    #[test]
    fn req_sim_15_band_filters_apply_to_the_part() {
        let (app, mut panel, mut viewport, dir) = part_only("panel-part-filter");
        settle_panel(&app, &mut panel, &mut viewport);
        let all = viewport.part_bands().unwrap().bands.len();
        assert!(all > 0);
        panel.set_colour_by(ColourBy::Overcut, &mut viewport);
        let shown = viewport.part_bands().unwrap();
        assert!(
            shown
                .bands
                .iter()
                .all(|(b, _)| matches!(b, DeviationBand::Overcut | DeviationBand::Unmeasured))
        );
        panel.set_colour_by(ColourBy::Deviation, &mut viewport);
        assert_eq!(viewport.part_bands().unwrap().bands.len(), all);
        let filter = BandFilter {
            to: Some(-100.0),
            ..BandFilter::default()
        };
        panel.set_band_filter(&filter, &mut viewport);
        let kept: usize = viewport
            .part_bands()
            .unwrap()
            .bands
            .iter()
            .map(|(_, t)| t.len())
            .sum();
        assert_eq!(kept, 0, "a filter that shows no value leaves none coloured");
        assert!(viewport.part_bands().unwrap().values.len() == 12);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// UI-11, decision 0492 item 7: a change to the toolpath or the settings
    /// leaves the part's colours as they are.
    #[test]
    fn req_ui_11_a_stale_toolpath_keeps_the_part_colours() {
        let (app, mut panel, mut viewport, dir) = part_only("panel-stale-colours");
        settle_panel(&app, &mut panel, &mut viewport);
        let before = viewport.part_bands().cloned().unwrap();
        let mut app = app;
        let operation = app.project().operations()[0].id;
        app.set_parameter(
            operation,
            "stepover",
            Some(openchuck_app::project::ParameterValue::Number(0.6)),
        )
        .unwrap();
        assert!(!openchuck_app::run_is_current(
            app.project(),
            panel.run.as_ref().unwrap()
        ));
        for _ in 0..3 {
            panel.frame(&app, &mut viewport, 0.0);
        }
        assert_eq!(viewport.part_bands(), Some(&before));
        assert!(!viewport.part_changed());
        assert!(panel.part.job.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// UI-11, decision 0492 item 7: a part that changed since the run is
    /// drawn plain with the note, whichever way it changed, and the colours
    /// come back with the old part.
    #[test]
    fn req_ui_11_a_changed_part_is_drawn_plain_with_the_note() {
        use openchuck_app::{Command, project::Item};
        let (app, mut panel, mut viewport, dir) = part_only("panel-part-changed");
        settle_panel(&app, &mut panel, &mut viewport);
        let mut app = app;
        assert!(viewport.part_bands().is_some());
        let model = app.project().models()[0].id;
        let setup = app.project().setups()[0].id;
        let changes: Vec<Command> = vec![
            // Reimported: another file hash.
            Command::Replace(Item::Model({
                let mut m = app.project().models()[0].clone();
                m.hash = openchuck_app::project::ContentHash::of_bytes(b"another file");
                m
            })),
            // Placed another way.
            Command::SetModelTransform {
                model,
                transform: vec![
                    openchuck_app::model::TransformStep::CentreOnRotaryAxis,
                    openchuck_app::model::TransformStep::Translate {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ],
            },
            // Read in other units.
            Command::SetModelUnits {
                model,
                units: Some(openchuck_app::model::LengthUnit::Inch),
            },
            // The setup's placement.
            Command::Replace(Item::Setup({
                let mut s = app.project().setup(setup).unwrap().clone();
                s.frame.part_offset = openchuck_app::setup::Millimetres(2.0);
                s
            })),
        ];
        for change in changes {
            app.execute(change).unwrap();
            panel.frame(&app, &mut viewport, 0.0);
            assert!(viewport.part_bands().is_none(), "the part is plain");
            assert!(viewport.part_changed(), "the note is on");
            assert!(panel.part.job.is_none(), "no colours are made for it");
            app.undo().unwrap();
            panel.frame(&app, &mut viewport, 0.0);
            assert!(
                viewport.part_bands().is_some(),
                "the old part's colours return"
            );
            assert!(!viewport.part_changed());
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// UI-11, decision 0492 item 7: the job reads the model file, so a file
    /// edited behind the application gets no colours either.
    #[test]
    fn req_ui_11_a_part_file_edited_behind_the_application_is_drawn_plain_with_the_note() {
        let (app, mut panel, mut viewport, dir) = part_only("panel-part-file");
        let file = dir.join("models/cube.stl");
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, text.replacen("solid cube", "solid cube two", 1)).unwrap();
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(viewport.part_bands().is_none());
        assert!(viewport.part_changed());
        assert!(panel.error.is_none(), "this is the note, not an error");
        assert!(panel.bands_on, "the colours stay on for the cut part");
        // It is not measured again every frame.
        for _ in 0..3 {
            assert!(!panel.frame(&app, &mut viewport, 0.0));
        }
        assert!(panel.part.job.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// UI-11, decision 0492 item 7 (review of #472): the colours already
    /// found are not kept for a scene read again after the file changed
    /// behind the application. The scene is built from the file as it is
    /// now, so the old colours would fall on its new triangles.
    #[test]
    fn req_ui_11_a_part_file_edited_after_the_colours_is_drawn_plain_once_the_scene_is_read_again()
    {
        let (app, mut panel, mut viewport, dir) = part_only("panel-part-file-later");
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(
            viewport.part_bands().is_some(),
            "the part is coloured first"
        );
        let file = dir.join("models/cube.stl");
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, text.replacen("solid cube", "solid cube two", 1)).unwrap();
        // The scene is read again (as after any edit of the project).
        viewport.set_scene(app.start_scene(None).wait().unwrap(), false);
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(
            viewport.part_bands().is_none(),
            "no old colours on the new file"
        );
        assert!(viewport.part_changed());
        assert!(panel.error.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Draws frames until nothing the panel runs is busy.
    fn settle_panel(app: &Application, panel: &mut PlaybackPanel, viewport: &mut Viewport) {
        let started = Instant::now();
        while panel.frame(app, viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn points(scene: &Scene) -> usize {
        scene.lines.iter().map(|l| l.segments.len()).sum()
    }

    #[test]
    fn req_sim_01_req_sim_04_the_window_draws_its_part_over_a_faint_trace_or_alone() {
        // Issue 205 (decision record 0169).
        let (app, mut panel, mut viewport, dir) = prepared("panel-window");
        settle_panel(&app, &mut panel, &mut viewport);
        let whole = points(viewport.layer(Layer::Paths).unwrap());
        let moves = panel.data.as_ref().unwrap().timeline.move_count();
        assert_eq!((panel.window_from, panel.window_to), (1, moves));
        let window = MoveWindow {
            first: moves / 3,
            last: moves / 2,
        };
        panel.set_window(Some(window));
        assert_eq!(panel.window(), Some(window));
        assert_eq!(
            (panel.window_from, panel.window_to),
            (window.first + 1, window.last + 1)
        );
        assert_eq!(
            panel.playback.cursor().done,
            window.first,
            "the play head is inside"
        );
        settle_panel(&app, &mut panel, &mut viewport);
        let windowed = points(viewport.layer(Layer::Paths).unwrap());
        assert!(windowed > 0 && windowed < whole, "{windowed} of {whole}");
        // The rest is a faint trace: one batch, coarser than the path.
        let trace = viewport.layer(Layer::Trace).unwrap();
        assert_eq!(trace.lines.len(), 1);
        assert_eq!(trace.lines[0].colour, TRACE);
        let (drawn, full) = panel.trace_cost.unwrap();
        assert!(drawn <= full);
        // Hidden: only the window's part is drawn.
        panel.set_hide_outside_window(true);
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(viewport.layer(Layer::Trace).unwrap().lines.is_empty());
        assert_eq!(points(viewport.layer(Layer::Paths).unwrap()), windowed);
        // Moving the window makes the path again but never the trace.
        panel.set_hide_outside_window(false);
        settle_panel(&app, &mut panel, &mut viewport);
        let trace_before = viewport.layer(Layer::Trace).unwrap() as *const Scene;
        panel.set_window(Some(MoveWindow {
            first: window.first + 1,
            last: window.last + 1,
        }));
        settle_panel(&app, &mut panel, &mut viewport);
        assert_eq!(
            viewport.layer(Layer::Trace).unwrap() as *const Scene,
            trace_before
        );
        // Reset: the whole path, no trace.
        panel.reset_window();
        settle_panel(&app, &mut panel, &mut viewport);
        assert_eq!(panel.window(), None);
        assert_eq!(points(viewport.layer(Layer::Paths).unwrap()), whole);
        assert!(viewport.layer(Layer::Trace).unwrap().lines.is_empty());
        // The window never changes what the run holds (GC-07).
        assert!(panel.run.as_ref().unwrap().simulation.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_04_dragging_the_window_moves_and_resizes_it() {
        // Moves 10 to 20: the boundaries 10 and 21 on the strip.
        let w = Some(MoveWindow {
            first: 10,
            last: 20,
        });
        let drag = |d, at| dragged_window(w, d, at, 100);
        // The start, held at the end's boundary 21.
        let start = WindowDrag::End { fixed: 21 };
        assert_eq!(drag(start, 5), Some(MoveWindow { first: 5, last: 20 }));
        // Past the end, the held boundary becomes the start.
        assert_eq!(
            drag(start, 30),
            Some(MoveWindow {
                first: 21,
                last: 29
            })
        );
        // The end, held at the start's boundary 10.
        let end = WindowDrag::End { fixed: 10 };
        assert_eq!(
            drag(end, 51),
            Some(MoveWindow {
                first: 10,
                last: 50
            })
        );
        assert_eq!(drag(end, 2), Some(MoveWindow { first: 2, last: 9 }));
        // At the held boundary, the window keeps the one move on its side.
        assert_eq!(
            drag(start, 21),
            Some(MoveWindow {
                first: 20,
                last: 20
            })
        );
        // Back over the held boundary, the window keeps one move.
        assert_eq!(
            drag(end, 10),
            Some(MoveWindow {
                first: 10,
                last: 10
            })
        );
        // The held boundary stays held after a crossing: a later move of the
        // pointer is measured from it, not from the window's new start.
        let crossed = dragged_window(w, end, 4, 100);
        assert_eq!(
            dragged_window(crossed, end, 3, 100),
            Some(MoveWindow { first: 3, last: 9 })
        );
        // Back onto the held boundary from either side, the window keeps
        // the move on the side it is on.
        assert_eq!(
            dragged_window(crossed, end, 10, 100),
            Some(MoveWindow { first: 9, last: 9 })
        );
        let crossed = dragged_window(w, start, 30, 100);
        assert_eq!(
            dragged_window(crossed, start, 21, 100),
            Some(MoveWindow {
                first: 21,
                last: 21
            })
        );
        // A one-move window keeps its move with either end on the other.
        let one = Some(MoveWindow {
            first: 20,
            last: 20,
        });
        assert_eq!(dragged_window(one, start, 21, 100), one);
        assert_eq!(
            dragged_window(one, WindowDrag::End { fixed: 20 }, 20, 100),
            one
        );
        // The body keeps its length and stays inside the program.
        assert_eq!(
            drag(WindowDrag::Body(3), 13),
            Some(MoveWindow {
                first: 10,
                last: 20
            })
        );
        assert_eq!(
            drag(WindowDrag::Body(3), 50),
            Some(MoveWindow {
                first: 47,
                last: 57
            })
        );
        assert_eq!(
            drag(WindowDrag::Body(3), 500),
            Some(MoveWindow {
                first: 89,
                last: 99
            })
        );
        assert_eq!(
            drag(WindowDrag::Body(3), 0),
            Some(MoveWindow { first: 0, last: 10 })
        );
        // The ends stay inside the program.
        assert_eq!(
            dragged_window(None, WindowDrag::End { fixed: 0 }, 400, 100),
            Some(MoveWindow { first: 0, last: 99 })
        );
        assert_eq!(
            dragged_window(None, WindowDrag::End { fixed: 100 }, 100, 100),
            Some(MoveWindow {
                first: 99,
                last: 99
            })
        );
        // A new window pressed on the first or the last boundary.
        assert_eq!(
            dragged_window(None, WindowDrag::End { fixed: 0 }, 0, 100),
            Some(MoveWindow { first: 0, last: 0 })
        );
        assert_eq!(
            dragged_window(None, WindowDrag::End { fixed: 100 }, 0, 100),
            Some(MoveWindow { first: 0, last: 99 })
        );
        assert_eq!(dragged_window(w, end, 4, 0), None);
    }

    #[test]
    fn req_sim_03_the_cut_part_is_prepared_without_being_asked_for() {
        // Issue 214 (decision record 0168): a simulated run shows the cut
        // part by default ("Cut part" is ticked), with no button to press.
        let (app, mut panel, mut viewport, dir) = prepared("panel-cut-part");
        // Not on another step, where it is not drawn.
        viewport.set_step(Some(crate::viewport::ViewStep::Operations));
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(panel.stock.playback.is_none() && panel.stock.prepare.is_none());
        viewport.set_step(Some(crate::viewport::ViewStep::Simulate));
        settle_panel(&app, &mut panel, &mut viewport);
        assert!(panel.stock.playback.is_some());
        let stock = viewport.layer(Layer::Stock).expect("the cut part is drawn");
        assert_eq!(stock.meshes.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn req_sim_02_the_a_readout_tooltip_gives_the_continuous_angle() {
        // Issue 207: the readout wraps; the tooltip keeps the G-code's value.
        assert_eq!(
            continuous_a_tip(725.5, 3),
            "Continuous A in the G-code: 725.500°"
        );
        assert_eq!(
            continuous_a_tip(-0.0, 2),
            "Continuous A in the G-code: 0.00°"
        );
    }

    #[test]
    fn req_nfr_12_a_run_changed_since_is_not_shown_again() {
        // OI-697: a turn-off drops the run; after another edit and the turn-on
        // by hand (not an undo), the old run is stale and stays off screen.
        let (mut app, mut panel, mut viewport, dir) = prepared("panel-stale-restore");
        let run = Arc::clone(panel.run.as_ref().unwrap());
        let op = app.project().operations()[0].id;
        app.execute(openchuck_app::Command::SetOperationEnabled {
            operation: op,
            enabled: false,
        })
        .unwrap();
        panel.forget_removed(&app);
        assert!(!panel.has_run());
        let mut edited = app.project().operation(op).unwrap().clone();
        edited.parameters.insert(
            "stepover".into(),
            openchuck_app::project::ParameterValue::Number(0.6),
        );
        app.execute(openchuck_app::Command::Replace(
            openchuck_app::project::Item::Operation(edited),
        ))
        .unwrap();
        app.execute(openchuck_app::Command::SetOperationEnabled {
            operation: op,
            enabled: true,
        })
        .unwrap();
        panel.restore(&app, &run);
        assert!(!panel.has_run(), "the stale run is not shown again");
        // Undoing back to what the run was made from shows it again (OI-692).
        app.undo().unwrap();
        app.undo().unwrap();
        app.undo().unwrap();
        panel.restore(&app, &run);
        assert!(panel.has_run());
        panel.frame(&app, &mut viewport, 0.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Draws the dock's controls once in `ctx` with `events`.
    fn show_frame(
        ctx: &egui::Context,
        panel: &mut PlaybackPanel,
        app: &Application,
        events: Vec<egui::Event>,
    ) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 400.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            panel.show(ui, app);
        });
        output.textures_delta.clear();
    }

    #[test]
    fn req_sim_04_the_window_strip_is_dragged_and_the_numbers_follow() {
        // Issue 205: dragging on the strip starts a window, its ends resize
        // it and its band moves it; "Whole program" lifts it.
        let (app, mut panel, mut viewport, dir) = prepared("panel-strip");
        settle_panel(&app, &mut panel, &mut viewport);
        let ctx = egui::Context::default();
        show_frame(&ctx, &mut panel, &app, Vec::new());
        let moves = panel.data.as_ref().unwrap().timeline.move_count();
        let (rect, left, width) = panel.strip;
        let x = |f: f32| left + width * f;
        let y = rect.center().y;
        let drag = |panel: &mut PlaybackPanel, from: f32, to: f32| {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let (a, b) = (egui::pos2(from, y), egui::pos2(to, y));
            show_frame(&ctx, panel, &app, vec![egui::Event::PointerMoved(a)]);
            show_frame(&ctx, panel, &app, vec![button(a, true)]);
            for step in 1..=6 {
                let p = a + (b - a) * (step as f32 / 6.0);
                show_frame(&ctx, panel, &app, vec![egui::Event::PointerMoved(p)]);
            }
            show_frame(&ctx, panel, &app, vec![button(b, false)]);
            show_frame(&ctx, panel, &app, Vec::new());
        };
        let at = |f: f32| (f * moves as f32).round() as usize;
        // A new window, from 40% to 60% of the moves.
        drag(&mut panel, x(0.4), x(0.6));
        let w = panel.window().expect("a window is set");
        // The start is the boundary pressed; the end the move before the
        // boundary released, so the band ends where the pointer is.
        assert_eq!((w.first, w.last), (at(0.4), at(0.6) - 1), "{w:?}");
        assert_eq!(
            (panel.window_from, panel.window_to),
            (w.first + 1, w.last + 1)
        );
        // Its start dragged left resizes it.
        let start = x(w.first as f32 / moves as f32);
        drag(&mut panel, start, x(0.2));
        let resized = panel.window().unwrap();
        assert_eq!(
            (resized.first, resized.last),
            (at(0.2), w.last),
            "{resized:?}"
        );
        // Its end dragged right resizes it, and the band ends at the pointer.
        let end = x((resized.last + 1) as f32 / moves as f32);
        drag(&mut panel, end, x(0.7));
        let longer = panel.window().unwrap();
        assert_eq!(
            (longer.first, longer.last),
            (at(0.2), at(0.7) - 1),
            "{longer:?}"
        );
        let resized = longer;
        // A new window dragged leftwards, from 95% to 80% (outside the band),
        // covers the moves between the two boundaries.
        drag(&mut panel, x(0.95), x(0.8));
        let leftwards = panel.window().unwrap();
        assert_eq!(
            (leftwards.first, leftwards.last),
            (at(0.8), at(0.95) - 1),
            "{leftwards:?}"
        );
        let data = panel.data.as_ref().unwrap();
        panel.playback.set_window(&data.timeline, Some(resized));
        // The band dragged right keeps its length.
        let middle = x((resized.first + resized.last) as f32 / 2.0 / moves as f32);
        drag(&mut panel, middle, middle + width * 0.1);
        let moved = panel.window().unwrap();
        assert_eq!(moved.last - moved.first, resized.last - resized.first);
        assert!(moved.first > resized.first);
        // The play head is inside it.
        assert!(
            moved.moves().contains(&panel.playback.cursor().done)
                || panel.playback.cursor().done == moved.last + 1
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
