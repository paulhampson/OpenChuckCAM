// SPDX-License-Identifier: AGPL-3.0-or-later
//! Playback: the tool at any point along the generated toolpath, with the
//! workpiece turned by A, and the controls that move along it (SIM-02,
//! SIM-04; work breakdown slice 9 items 4 and 6; decision record 0038).
//!
//! A [`Timeline`] lays the moves of a regeneration end to end in the time
//! the program would take. A [`Cursor`] says how many moves are done and how
//! far into the next one, and [`Timeline::sample`] gives the pose there as the
//! controller would reach it: linear in machine X, Y, Z and A together, so the
//! tool tip in the part's frame follows a curve when A turns, as in the
//! simulation's sampling (decision 0025 item 3). [`Playback`] holds the
//! controls (play, pause, step, speed, jump to a move or an operation) and
//! has no clock: the front end gives it the seconds that passed.
//!
//! The pose is given for both views the viewport can show: the part's frame,
//! where the part is fixed and the tool moves round it, and the machine view,
//! where the tool stands upright and the part (with its toolpath and stock)
//! turns by `sign * A` about the rotary axis. Both come from the shared
//! kinematics, so they cannot disagree with the posts (architecture
//! section 5.1).

use std::sync::Arc;

use openchuck_project::Project;
use openchuck_project::id::OperationId;
use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::ir::{MoveKind, ToolPose};
use openchuck_types::kinematics::{
    MachinePosition, RotaryFrame, machine_xyz, tool_axis_in_model, tool_tip_in_model,
};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::tool::Tool;
use openchuck_types::units::Degrees;

use crate::tool_edit::{profile_outline, starting_profile};
use crate::{AppError, Regeneration};

/// Sides used to turn a tool's outline into a solid.
const TOOL_SIDES: usize = 48;

/// How far through the program: whole moves done, and the part of the next
/// move that is. `fraction` is 0 at a move's start and when all are done.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Cursor {
    /// Moves done, counted across all groups in machining order.
    pub done: usize,
    /// How far into move number `done`, from 0 up to 1.
    pub fraction: f64,
}

#[derive(Debug, Clone)]
struct TimelineGroup {
    operation: OperationId,
    name: String,
    tool: Option<Tool>,
    /// The work frame: the coordinates the program is written in, so a pose's
    /// X, Y, Z and A read as the G-code lines do.
    work: RotaryFrame,
    /// The same frame with its origin at the part's, for drawing.
    view: RotaryFrame,
}

/// The moves of a regeneration laid end to end in time.
#[derive(Debug, Clone)]
pub struct Timeline {
    run: Arc<Regeneration>,
    groups: Vec<TimelineGroup>,
    /// Elapsed program time at the end of each move, across groups.
    ends: Vec<f64>,
    /// Index of the first move of each group.
    first: Vec<usize>,
}

/// The tool and the part at one point along the path (SIM-02).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoseSample {
    /// Where this is.
    pub cursor: Cursor,
    /// The group (operation) the pose is in.
    pub group: usize,
    /// The move in progress (or the last one done), within its group; `None`
    /// when the group has no moves yet.
    pub segment: Option<usize>,
    /// What that move is for.
    pub kind: Option<MoveKind>,
    /// The tool tip in the part's frame.
    pub tip: Point3,
    /// The unit tool axis in the part's frame, from the tip towards the spindle.
    pub axis: Vec3,
    /// The rotary angle, degrees, unwrapped.
    pub a: f64,
    /// X, Y, Z and A as the program's lines give them (work coordinates).
    pub work: [f64; 4],
    /// The programmed feed at the contact, mm/min, for a move that has one.
    pub feed: Option<f64>,
    /// Program time at this point, seconds.
    pub time: f64,
    /// How much the part has turned in the machine view: `sign * A`, degrees,
    /// about the rotary axis.
    pub part_turn: f64,
    /// The tool tip in the machine view, where the part has turned.
    pub turned_tip: Point3,
}

/// Where to put a tool built with its tip at the origin and its axis along +Z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolPlacement {
    /// Where its tip goes.
    pub origin: [f64; 3],
    /// Where its axis points.
    pub axis: [f64; 3],
}

/// Which frame to draw in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewFrame {
    /// The part is fixed and the tool moves round it.
    #[default]
    Part,
    /// The tool stands upright and the part turns with A.
    Machine,
}

impl PoseSample {
    /// Where the tool goes in `view`.
    pub fn placement(&self, view: ViewFrame) -> ToolPlacement {
        match view {
            ViewFrame::Part => ToolPlacement {
                origin: [self.tip.x, self.tip.y, self.tip.z],
                axis: [self.axis.x, self.axis.y, self.axis.z],
            },
            ViewFrame::Machine => ToolPlacement {
                origin: [self.turned_tip.x, self.turned_tip.y, self.turned_tip.z],
                axis: [0.0, 0.0, 1.0],
            },
        }
    }

    /// How far the part is turned in `view`, degrees: nothing in the part's
    /// frame, `sign * A` in the machine view.
    pub fn part_rotation(&self, view: ViewFrame) -> f64 {
        match view {
            ViewFrame::Part => 0.0,
            ViewFrame::Machine => self.part_turn,
        }
    }
}

impl Timeline {
    /// The timeline of `run`, made from `project`. Fails when the run was not
    /// made by this application version's pipeline (no group information), or
    /// the project has no machine.
    pub fn new(project: &Project, run: Arc<Regeneration>) -> Result<Self, AppError> {
        let machine = project
            .machine()
            .ok_or_else(|| AppError::Playback("the project has no machine profile".to_owned()))?;
        if run.groups.len() != run.toolpath.groups.len() {
            return Err(AppError::Playback(
                "the toolpath does not say which operation made each group; regenerate it"
                    .to_owned(),
            ));
        }
        let mut groups = Vec::with_capacity(run.groups.len());
        let mut ends = Vec::new();
        let mut first = Vec::with_capacity(run.groups.len());
        let mut time = 0.0_f64;
        for (path, info) in run.toolpath.groups.iter().zip(&run.groups) {
            let setup = project.setup(info.setup);
            let work = setup.map_or_else(
                || RotaryFrame::of_profile(machine),
                |s| RotaryFrame::work(machine, &s.frame),
            );
            let view = RotaryFrame {
                origin: Point3::ORIGIN,
                ..work
            };
            let tool = project
                .operation(info.operation)
                .and_then(|o| o.tool)
                .and_then(|id| project.tools().iter().find(|t| t.id == id))
                .map(|t| t.tool.clone());
            // The IR's tags are the application's IDs as text; the name the
            // user knows the operation by comes from the project.
            let name = project
                .operation(info.operation)
                .map_or_else(|| path.tags.operation.clone(), |o| o.name.clone());
            groups.push(TimelineGroup {
                operation: info.operation,
                name,
                tool,
                work,
                view,
            });
            first.push(ends.len());
            for segment in &path.segments {
                time += segment.duration.0.max(0.0);
                ends.push(time);
            }
        }
        Ok(Self {
            run,
            groups,
            ends,
            first,
        })
    }

    /// The regeneration this is made from.
    pub fn run(&self) -> &Arc<Regeneration> {
        &self.run
    }

    /// Number of groups (operations with a toolpath).
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// The operation a group came from.
    pub fn operation(&self, group: usize) -> Option<OperationId> {
        self.groups.get(group).map(|g| g.operation)
    }

    /// The name of the operation a group came from.
    pub fn operation_name(&self, group: usize) -> Option<&str> {
        self.groups.get(group).map(|g| g.name.as_str())
    }

    /// The tool a group cuts with.
    pub fn tool(&self, group: usize) -> Option<&Tool> {
        self.groups.get(group).and_then(|g| g.tool.as_ref())
    }

    /// Number of moves in all.
    pub fn move_count(&self) -> usize {
        self.ends.len()
    }

    /// The program's time, seconds.
    pub fn total_time(&self) -> f64 {
        self.ends.last().copied().unwrap_or(0.0)
    }

    /// The index across all moves of the first move of `group`.
    pub fn first_move(&self, group: usize) -> Option<usize> {
        self.first.get(group).copied()
    }

    /// The group and the move within it that move number `flat` is.
    pub fn locate(&self, flat: usize) -> Option<(usize, usize)> {
        if flat >= self.ends.len() {
            return None;
        }
        let group = self.first.partition_point(|&f| f <= flat).checked_sub(1)?;
        Some((group, flat - self.first.get(group)?))
    }

    /// Program time at the start of move `flat`.
    fn start_of(&self, flat: usize) -> f64 {
        flat.checked_sub(1)
            .and_then(|i| self.ends.get(i))
            .copied()
            .unwrap_or(0.0)
    }

    /// Program time at a cursor, seconds.
    pub fn time_at(&self, cursor: Cursor) -> f64 {
        let start = self.start_of(cursor.done);
        match self.ends.get(cursor.done) {
            Some(end) => start + cursor.fraction.clamp(0.0, 1.0) * (end - start),
            None => self.total_time(),
        }
    }

    /// The cursor at program time `t` (clamped to the program).
    pub fn cursor_at_time(&self, t: f64) -> Cursor {
        if !t.is_finite() || t <= 0.0 {
            return Cursor::default();
        }
        let done = self.ends.partition_point(|&e| e <= t);
        match self.ends.get(done) {
            None => Cursor {
                done: self.ends.len(),
                fraction: 0.0,
            },
            Some(end) => {
                let start = self.start_of(done);
                let span = end - start;
                Cursor {
                    done,
                    fraction: if span > 0.0 { (t - start) / span } else { 0.0 },
                }
            }
        }
    }

    /// The pose at `cursor`, as the controller would reach it: X, Y, Z and A
    /// move together in a straight line in machine coordinates (decision
    /// 0025 item 3). Fails for a program with no groups.
    pub fn sample(&self, cursor: Cursor) -> Result<PoseSample, AppError> {
        let empty = || AppError::Playback("there is no toolpath to play".to_owned());
        let path = &self.run.toolpath;
        let total = self.ends.len();
        let done = cursor.done.min(total);
        let (group, from, to, kind, feed, fraction, segment) = if done >= total {
            // Everything done: stay at the end of the last move, or at the
            // start of a program with no moves.
            let g = path.groups.iter().rposition(|g| !g.segments.is_empty());
            match g.and_then(|g| path.groups.get(g).map(|p| (g, p))) {
                Some((g, p)) => {
                    let last = p.segments.last().ok_or_else(empty)?;
                    (
                        g,
                        last.end,
                        last.end,
                        Some(last.kind),
                        last.contact_feed.map(|f| f.0),
                        0.0,
                        Some(p.segments.len() - 1),
                    )
                }
                None => {
                    let p = path.groups.first().ok_or_else(empty)?;
                    (0, p.start, p.start, None, None, 0.0, None)
                }
            }
        } else {
            let (g, s) = self.locate(done).ok_or_else(empty)?;
            let p = path.groups.get(g).ok_or_else(empty)?;
            let seg = p.segments.get(s).ok_or_else(empty)?;
            let from = s
                .checked_sub(1)
                .and_then(|i| p.segments.get(i))
                .map_or(p.start, |prev| prev.end);
            (
                g,
                from,
                seg.end,
                Some(seg.kind),
                seg.contact_feed.map(|f| f.0),
                cursor.fraction.clamp(0.0, 1.0),
                Some(s),
            )
        };
        let info = self.groups.get(group).ok_or_else(empty)?;
        let (tip, axis, a, work) = between(&info.work, &from, &to, fraction);
        let turned_tip = machine_xyz(&info.view, tip, Degrees(a));
        Ok(PoseSample {
            cursor: Cursor { done, fraction },
            group,
            segment,
            kind,
            tip,
            axis,
            a,
            work,
            feed,
            time: self.time_at(Cursor { done, fraction }),
            part_turn: info.work.sense.sign() * a,
            turned_tip,
        })
    }

    /// The work frame of a group, for the code that draws its path.
    pub(crate) fn work_frame(&self, group: usize) -> Option<RotaryFrame> {
        self.groups.get(group).map(|g| g.work)
    }

    /// The program's X, Y, Z and A (work coordinates) at the end of move
    /// `flat`: where a G-code line that writes that move leaves the machine,
    /// to the digits the post writes.
    pub fn work_after(&self, flat: usize) -> Option<[f64; 4]> {
        let (g, s) = self.locate(flat)?;
        let seg = self.run.toolpath.groups.get(g)?.segments.get(s)?;
        let frame = &self.groups.get(g)?.work;
        let p = machine_xyz(frame, seg.end.tip, seg.end.a);
        Some([p.x, p.y, p.z, seg.end.a.0])
    }
}

/// The pose a fraction `t` of the way from `from` to `to`, moving linearly in
/// machine coordinates: the part's frame tip, unit axis, A, and the work
/// coordinates X, Y, Z and A.
pub(crate) fn between(
    frame: &RotaryFrame,
    from: &ToolPose,
    to: &ToolPose,
    t: f64,
) -> (Point3, Vec3, f64, [f64; 4]) {
    let (pose, t) = if t <= 0.0 {
        (Some(from), 0.0)
    } else if t >= 1.0 {
        (Some(to), 1.0)
    } else {
        (None, t)
    };
    if let Some(p) = pose {
        let w = machine_xyz(frame, p.tip, p.a);
        return (p.tip, p.axis.get(), p.a.0, [w.x, w.y, w.z, p.a.0]);
    }
    let m0 = machine_xyz(frame, from.tip, from.a);
    let m1 = machine_xyz(frame, to.tip, to.a);
    let a = from.a.0 + t * (to.a.0 - from.a.0);
    let xyz = m0 + (m1 - m0) * t;
    let tip = tool_tip_in_model(frame, &MachinePosition { xyz, a: Degrees(a) });
    let axis = tool_axis_in_model(frame, Degrees(a)).get();
    (tip, axis, a, [xyz.x, xyz.y, xyz.z, a])
}

/// The playback controls (SIM-04). They hold a [`Cursor`] and move it; the
/// front end calls [`advance`](Self::advance) with the seconds that passed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playback {
    cursor: Cursor,
    playing: bool,
    speed: f64,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            cursor: Cursor::default(),
            playing: false,
            speed: 1.0,
        }
    }
}

impl Playback {
    /// Paused at the start, at real speed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Where playback is.
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Whether it is playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Program seconds per second of the clock.
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Sets the speed (program seconds per clock second). A value that is not
    /// finite and positive is ignored.
    pub fn set_speed(&mut self, speed: f64) {
        if speed.is_finite() && speed > 0.0 {
            self.speed = speed;
        }
    }

    /// The speed at which the whole program plays in `seconds`.
    pub fn speed_to_finish_in(timeline: &Timeline, seconds: f64) -> f64 {
        if seconds > 0.0 && timeline.total_time() > 0.0 {
            timeline.total_time() / seconds
        } else {
            1.0
        }
    }

    /// Starts playing; at the end it starts again from the beginning.
    pub fn play(&mut self, timeline: &Timeline) {
        if self.cursor.done >= timeline.move_count() {
            self.cursor = Cursor::default();
        }
        self.playing = true;
    }

    /// Pauses.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Moves along by `seconds` of the clock when playing, and stops at the
    /// end. Returns whether the cursor moved.
    pub fn advance(&mut self, timeline: &Timeline, seconds: f64) -> bool {
        if !self.playing || !(seconds.is_finite() && seconds > 0.0) {
            return false;
        }
        let t = timeline.time_at(self.cursor) + seconds * self.speed;
        let next = timeline.cursor_at_time(t);
        let moved = next != self.cursor;
        self.cursor = next;
        if self.cursor.done >= timeline.move_count() {
            self.playing = false;
        }
        moved
    }

    /// Steps by whole moves: positive forward, negative back. Playback pauses.
    pub fn step(&mut self, timeline: &Timeline, moves: isize) {
        self.playing = false;
        let done = self.cursor.done;
        // Stepping back from part-way into a move goes to that move's start.
        let target = if moves < 0 && self.cursor.fraction > 0.0 {
            done.saturating_sub(moves.unsigned_abs() - 1)
        } else if moves < 0 {
            done.saturating_sub(moves.unsigned_abs())
        } else {
            done.saturating_add(moves.unsigned_abs())
        };
        self.cursor = Cursor {
            done: target.min(timeline.move_count()),
            fraction: 0.0,
        };
    }

    /// Jumps to just after move `flat` (counted across all groups), where
    /// the machine stands when the G-code line that writes it is done
    /// (SIM-04). Playback pauses.
    pub fn jump_to_move(&mut self, timeline: &Timeline, flat: usize) {
        self.playing = false;
        self.cursor = Cursor {
            done: flat.saturating_add(1).min(timeline.move_count()),
            fraction: 0.0,
        };
    }

    /// Jumps to the start of an operation's moves (SIM-04). Playback pauses.
    pub fn jump_to_group(&mut self, timeline: &Timeline, group: usize) {
        self.playing = false;
        self.cursor = Cursor {
            done: timeline.first_move(group).unwrap_or(0),
            fraction: 0.0,
        };
    }

    /// Jumps to a point in the program's time, `fraction` of the way from the
    /// start to the end. Playback pauses.
    pub fn seek(&mut self, timeline: &Timeline, fraction: f64) {
        self.playing = false;
        let f = if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.cursor = timeline.cursor_at_time(f * timeline.total_time());
    }
}

/// A tool as two solids built with the tip at the origin and the axis along
/// +Z: the cutter, up to the flute length, and the shank above it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolMeshes {
    /// The flutes: the tool's profile turned about its axis.
    pub cutter: TriangleMesh,
    /// The shank, when the tool is longer than its flutes.
    pub shank: Option<TriangleMesh>,
}

/// The solids to draw for `tool` (SIM-02). `None` when the tool is not valid.
pub fn tool_meshes(tool: &Tool) -> Option<ToolMeshes> {
    tool.validate().ok()?;
    let outline = profile_outline(&starting_profile(tool)?);
    let cutter = revolve(&outline);
    let flute = tool.flute_length.0;
    let overall = tool.overall_length.0;
    let radius = tool.shank_diameter.0 / 2.0;
    let shank = (overall - flute > 1e-9 && radius > 0.0)
        .then(|| revolve(&[[radius, flute], [radius, overall], [0.0, overall]]));
    Some(ToolMeshes { cutter, shank })
}

/// The outline of `(radius, height)` points turned about the Z axis, wound so
/// that the triangles face outward for an outline that runs from the bottom
/// up.
fn revolve(outline: &[[f64; 2]]) -> TriangleMesh {
    let mut mesh = TriangleMesh::default();
    for point in outline {
        for side in 0..TOOL_SIDES {
            let angle = std::f64::consts::TAU * side as f64 / TOOL_SIDES as f64;
            mesh.vertices.push(Point3::new(
                point[0] * angle.cos(),
                point[0] * angle.sin(),
                point[1],
            ));
        }
    }
    let ring = |i: usize, side: usize| (i * TOOL_SIDES + side % TOOL_SIDES) as u32;
    for i in 0..outline.len().saturating_sub(1) {
        for side in 0..TOOL_SIDES {
            let (a, b) = (ring(i, side), ring(i, side + 1));
            let (c, d) = (ring(i + 1, side), ring(i + 1, side + 1));
            mesh.triangles.push([a, b, c]);
            mesh.triangles.push([b, d, c]);
        }
    }
    mesh
}
