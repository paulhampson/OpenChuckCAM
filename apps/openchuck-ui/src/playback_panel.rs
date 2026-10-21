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
use openchuck_app::pathview::{DeviationBand, DeviationBands, PathClass, PathLines, PlaybackData};
use openchuck_app::playback::{Cursor, Playback, PoseSample, ToolMeshes, ViewFrame, tool_meshes};
use openchuck_app::project::id::{ModelId, OperationId, SetupId};
use openchuck_app::stockplay::StockPlayback;
use openchuck_app::{Application, JobHandle, Regeneration};
use openchuck_viewport::camera::place;
use openchuck_viewport::{Colour, Layer, LineStyle, Scene, Slot};

use crate::results_panel::ColourBy;
use crate::roles;
use crate::theme::DARK;
use crate::viewport::{ToolReadout, Viewport};

const PROBLEM: Color32 = DARK.error;

/// Points of toolpath the viewport is asked to draw at most. A million line
/// vertices is about 24 MB on the GPU and draws well within a frame on the
/// reference class of machine.
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
const STOCK_GLASS: Colour = [0.85, 0.72, 0.45, 0.30];

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

#[derive(Default)]
struct StockState {
    show: bool,
    prepare: Option<JobHandle<StockPlayback>>,
    playback: Option<Arc<StockPlayback>>,
    mesh: Option<(JobHandle<openchuck_app::scene::Mesh>, usize)>,
    /// The number of moves done that the stock on screen is for.
    shown: Option<usize>,
    error: Option<String>,
}

/// The toolpath display and playback controls.
pub struct PlaybackPanel {
    /// Whether the panel is shown.
    pub open: bool,
    run: Option<Arc<Regeneration>>,
    data_job: Option<JobHandle<PlaybackData>>,
    data: Option<PlaybackData>,
    playback: Playback,
    visible: Vec<bool>,
    view: ViewFrame,
    show_tool: bool,
    show_paths: bool,
    budget: usize,
    lines_job: Option<JobHandle<PathLines>>,
    lines_dirty: bool,
    detail: Option<(usize, usize, f64)>,
    /// The group whose tool the tool layer holds.
    tool_group: Option<usize>,
    tool_dirty: bool,
    stock: StockState,
    bands_on: bool,
    /// The deviation job running, with the setup it is for.
    bands_job: Option<(SetupId, JobHandle<(ModelId, DeviationBands)>)>,
    bands: Option<DeviationBands>,
    /// The model the bands colour.
    bands_model: Option<ModelId>,
    /// What the part is coloured by (the results inspector's choice).
    colour_by: ColourBy,
    /// The setup the bands on screen (or being made) belong to.
    bands_setup: Option<SetupId>,
    jump_move: usize,
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
            view: ViewFrame::Part,
            show_tool: true,
            show_paths: true,
            budget: DEFAULT_POINT_BUDGET,
            lines_job: None,
            lines_dirty: false,
            detail: None,
            tool_group: None,
            tool_dirty: false,
            stock: StockState::default(),
            bands_on: false,
            bands_job: None,
            bands: None,
            bands_model: None,
            colour_by: ColourBy::default(),
            bands_setup: None,
            jump_move: 1,
            error: None,
            reset_layers: false,
        }
    }
}

impl PlaybackPanel {
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
        if let Some((_, job)) = &self.bands_job {
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
            colour_by: self.colour_by,
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
            show_paths: self.show_paths,
            budget: self.budget,
            colour_by: self.colour_by,
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
    /// [`PlaybackPanel::forget_removed`]) and every operation in it is back
    /// and on, as after undoing a turn-off or a deletion (OI-692). Does
    /// nothing while the panel has a run, or when `run` still holds an
    /// operation that is gone.
    pub fn restore(&mut self, app: &Application, run: &Arc<Regeneration>) {
        let back = run.groups.iter().all(|g| {
            app.project()
                .operation(g.operation)
                .is_some_and(|o| o.enabled)
        });
        if self.run.is_none() && back && !run.groups.is_empty() {
            let open = self.open;
            self.set_run(app, Arc::clone(run));
            self.open = open;
        }
    }

    /// Whether the panel has a run to show.
    pub fn has_run(&self) -> bool {
        self.run.is_some()
    }

    /// Where playback is, once the run is prepared.
    pub fn pose(&self) -> Option<PoseSample> {
        let data = self.data.as_ref()?;
        data.timeline.sample(self.playback.cursor()).ok()
    }

    /// Works the jobs, moves the cursor by `dt` seconds of the clock, and
    /// brings the viewport up to date. Returns true while the frame should be
    /// drawn again.
    pub fn frame(&mut self, app: &Application, viewport: &mut Viewport, dt: f64) -> bool {
        let mut busy = false;
        if std::mem::take(&mut self.reset_layers) {
            for layer in [Layer::Paths, Layer::Tool, Layer::Stock] {
                viewport.set_layer(layer, Scene::default());
            }
            viewport.set_bands(None);
            viewport.set_part_turn(0.0);
            viewport.set_tool_readout(None);
        }
        busy |= self.poll_data();
        busy |= self.poll_lines(app, viewport);
        let Some(data) = self.data.clone() else {
            return busy || self.data_job.is_some();
        };
        if self.playback.advance(&data.timeline, dt) {
            busy |= self.playback.is_playing();
        }
        busy |= self.playback.is_playing();
        if let Ok(pose) = data.timeline.sample(self.playback.cursor()) {
            self.update_tool(viewport, &data, &pose);
            viewport.set_part_turn(pose.part_rotation(self.view));
            let placement = pose.placement(self.view);
            viewport.set_tool_transform(place(placement.origin, placement.axis));
            viewport.set_tool_readout(Some(ToolReadout {
                work: pose.work,
                move_number: pose.cursor.done + 1,
            }));
            busy |= self.update_stock(app, viewport, &pose);
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
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
                false
            }
        }
    }

    fn poll_lines(&mut self, app: &Application, viewport: &mut Viewport) -> bool {
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
            let visible = if self.show_paths {
                self.visible.clone()
            } else {
                vec![false; self.visible.len()]
            };
            self.lines_job =
                Some(app.start_path_lines(Arc::clone(&data.display), visible, self.budget));
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
        let stock = &mut self.stock;
        let mut busy = false;
        if let Some(job) = &stock.prepare {
            match job.try_result() {
                None => busy = true,
                Some(result) => {
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
                    let done = *done;
                    stock.mesh = None;
                    match result {
                        Ok(mesh) => {
                            let mut scene = Scene::default();
                            scene.add_mesh(&mesh, &[], STOCK_GLASS, STOCK_GLASS);
                            for item in &mut scene.meshes {
                                item.slot = Slot::Part;
                                item.translucent = true;
                            }
                            viewport.set_layer(Layer::Stock, scene);
                            stock.shown = Some(done);
                        }
                        Err(e) if e.is_cancelled() => {}
                        Err(e) => stock.error = Some(e.to_string()),
                    }
                }
            }
        }
        if !stock.show {
            if stock.shown.take().is_some() {
                viewport.set_layer(Layer::Stock, Scene::default());
            }
            return busy;
        }
        let done = pose.cursor.done;
        if let Some(playback) = &stock.playback
            && stock.mesh.is_none()
            && stock.shown != Some(done)
        {
            stock.mesh = Some((
                app.start_stock_at(
                    playback,
                    Cursor {
                        done,
                        fraction: 0.0,
                    },
                ),
                done,
            ));
            busy = true;
        }
        busy
    }

    /// Sets what the part is coloured by, and redraws the colours.
    pub fn set_colour_by(&mut self, by: ColourBy, viewport: &mut Viewport) {
        if self.colour_by != by {
            self.colour_by = by;
            self.show_bands(viewport);
        }
    }

    /// Hands the viewport the bands that `colour_by` shows: the others are
    /// left out, so those triangles keep the part's own colour.
    fn show_bands(&self, viewport: &mut Viewport) {
        let (Some(bands), Some(model)) = (&self.bands, self.bands_model) else {
            return;
        };
        let mut shown = bands.clone();
        shown.bands.retain(|(band, _)| self.colour_by.shows(*band));
        viewport.colour_by = self.colour_by;
        viewport.set_bands(Some((model, shown)));
    }

    fn update_bands(&mut self, app: &Application, viewport: &mut Viewport) -> bool {
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

    /// Whether the playback is running.
    #[cfg(test)]
    pub fn is_playing(&self) -> bool {
        self.playback.is_playing()
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

    /// Whether the operation's path is drawn, for the tree's eye button;
    /// `None` when the run has no group of it.
    pub fn is_shown(&self, operation: OperationId) -> Option<bool> {
        let data = self.data.as_ref()?;
        let group = (0..data.timeline.group_count())
            .find(|g| data.timeline.operation(*g) == Some(operation))?;
        self.visible.get(group).copied()
    }

    /// Shows or hides an operation's path, in the view only: the simulation
    /// results do not change (specification section 11.5).
    pub fn toggle_shown(&mut self, operation: OperationId) {
        let Some(data) = &self.data else { return };
        if let Some(group) = (0..data.timeline.group_count())
            .find(|g| data.timeline.operation(*g) == Some(operation))
            && let Some(v) = self.visible.get_mut(group)
        {
            *v = !*v;
            self.lines_dirty = true;
        }
    }

    /// The timeline: one segment per enabled operation sized by its moves,
    /// the ones not generated dashed amber, the playhead, and move numbers
    /// under it. Clicking seeks.
    fn timeline_bar(&mut self, ui: &mut egui::Ui, app: &Application, data: &PlaybackData) {
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
        ui.horizontal(|ui| {
            ui.label(RichText::new("0").small().monospace());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(moves.to_string()).small().monospace());
            });
        });
    }

    /// Draws the controls.
    pub fn show(&mut self, ui: &mut egui::Ui, app: &Application) {
        let Some(data) = self.data.clone() else {
            if self.data_job.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Preparing the toolpath for display…");
                });
            } else if let Some(error) = &self.error {
                ui.colored_label(PROBLEM, error);
            } else {
                ui.label("Generate toolpaths to see and play them.");
            }
            return;
        };
        let timeline = &data.timeline;
        let total = timeline.total_time();
        let moves = timeline.move_count();
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
        });
        ui.horizontal(|ui| {
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
        });
        self.timeline_bar(ui, app, &data);
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
            ui.monospace(format!(
                "{}  move {}  {kind}   X{:.3} Y{:.3} Z{:.3} A{:.3}{feed}",
                timeline.operation_name(pose.group).unwrap_or(""),
                pose.cursor.done.min(moves),
                pose.work[0],
                pose.work[1],
                pose.work[2],
                pose.work[3],
            ));
        }
        ui.horizontal_wrapped(|ui| {
            let mut changed = false;
            let mut machine = self.view == ViewFrame::Machine;
            if ui
                .checkbox(&mut machine, "Turn the part with A")
                .on_hover_text(
                    "The tool stands upright and the workpiece turns, as on the machine; \
                     otherwise the part is fixed and the tool moves round it",
                )
                .changed()
            {
                self.view = if machine {
                    ViewFrame::Machine
                } else {
                    ViewFrame::Part
                };
            }
            if ui.checkbox(&mut self.show_tool, "Tool").changed() {
                self.tool_dirty = true;
            }
            changed |= ui.checkbox(&mut self.show_paths, "Toolpaths").changed();
            let can_stock = self.run.as_ref().is_some_and(|r| r.simulation.is_some());
            ui.add_enabled_ui(can_stock, |ui| {
                self.stock_controls(ui, app, &data);
                ui.checkbox(&mut self.bands_on, "Colour the part by deviation")
                    .on_disabled_hover_text("Generate with a simulation first")
                    .on_hover_text("Overcut, on the surface, and material left");
            });
            if changed {
                self.lines_dirty = true;
            }
        });
        // The note keeps its line while it has nothing to say, so the rows
        // do not move when it appears (issue 215).
        if let Some((drawn, full, tolerance)) = self.detail {
            if drawn < full {
                ui.weak(format!(
                    "Drawing {drawn} of {full} points, simplified to within {tolerance} mm"
                ));
            } else {
                ui.weak(format!("Drawing all {full} points"));
            }
        } else {
            ui.weak(" ");
        }
        if let Some(error) = self.stock.error.as_ref().or(self.error.as_ref()) {
            ui.colored_label(PROBLEM, error);
        }
        let _ = DeviationBand::ALL;
    }

    fn stock_controls(&mut self, ui: &mut egui::Ui, app: &Application, data: &PlaybackData) {
        if self.stock.prepare.is_some() {
            ui.spinner();
            ui.label("Preparing the stock…");
            return;
        }
        if self.stock.playback.is_none() {
            if ui
                .button("Show the stock…")
                .on_hover_text(
                    "Simulates again with checkpoints so the stock can be shown at any move",
                )
                .clicked()
            {
                self.stock.error = None;
                self.stock.show = true;
                self.stock.prepare = Some(app.start_stock_playback(Arc::clone(&data.timeline)));
            }
        } else {
            ui.checkbox(&mut self.stock.show, "Stock");
        }
    }
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
    fn req_sim_05_the_stock_and_the_deviation_colours_come_from_jobs() {
        let (app, mut panel, mut viewport, dir) = prepared("panel-stock");
        let data = panel.data.clone().unwrap();
        // Ask for the stock: the job prepares the checkpoints, then the
        // mesh at the cursor is made for the moves done.
        panel.stock.show = true;
        panel.stock.prepare = Some(app.start_stock_playback(Arc::clone(&data.timeline)));
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
        assert_eq!(panel.stock.shown, Some(panel.playback.cursor().done));
        // Hiding it empties the layer.
        panel.stock.show = false;
        panel.frame(&app, &mut viewport, 0.0);
        assert!(viewport.layer(Layer::Stock).unwrap().meshes.is_empty());
        // Colouring by deviation splits the part into bands in the scene.
        panel.bands_on = true;
        while panel.frame(&app, &mut viewport, 0.0) {
            assert!(started.elapsed() < Duration::from_secs(120));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(panel.bands.is_some());
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
}
