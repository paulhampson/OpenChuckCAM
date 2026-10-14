// SPDX-License-Identifier: AGPL-3.0-or-later
//! The NFR-03 frame-time benchmark: the scripted camera path and the
//! frame-time statistics (verification plan section 4.2; work breakdown
//! slice 8 item 7; decision record 0029 item 9).
//!
//! The path is a function of time only, so every run moves the camera the
//! same way whatever the frame rate. It orbits for the first third, pans for
//! the second and zooms for the last, starting from a camera fitted to the
//! scene. The front end measures the interval between frames and hands the
//! times to [`FrameTimes`].

use std::time::Duration;

use crate::camera::Camera;

/// The frame time the 4.2 test allows at the 95th percentile: 33 ms, about
/// 30 frames per second (NFR-03).
pub const P95_LIMIT: Duration = Duration::from_millis(33);

/// The length of the 4.2 camera path.
pub const STANDARD_DURATION: Duration = Duration::from_secs(60);

/// A scripted camera path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPath {
    start: Camera,
    seconds: f64,
}

impl CameraPath {
    /// A path of `duration` starting from `start` (fitted to the scene).
    pub fn new(start: Camera, duration: Duration) -> Self {
        Self {
            start,
            seconds: duration.as_secs_f64().max(f64::MIN_POSITIVE),
        }
    }

    /// The path's length.
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.seconds)
    }

    /// The camera at `elapsed` from the start. Past the end it stays at the
    /// end.
    pub fn at(&self, elapsed: Duration) -> Camera {
        let u = (elapsed.as_secs_f64() / self.seconds).clamp(0.0, 1.0);
        let mut camera = self.start;
        let phase = |from: f64| ((u - from) * 3.0).clamp(0.0, 1.0);
        let tau = std::f64::consts::TAU;

        // Orbit: two turns of yaw while the pitch swings 30 degrees either way.
        let o = phase(0.0);
        camera.orbit(-720.0 * o, 30.0 * (tau * o).sin());

        // Pan: the target goes once round a circle of a third of the scene
        // radius, in the view plane, and back to the centre.
        let p = phase(1.0 / 3.0);
        let (right, up, _) = camera.basis();
        let r = camera.scene_radius / 3.0 * (std::f64::consts::PI * p).sin();
        let (c, s) = ((tau * p).cos(), (tau * p).sin());
        for i in 0..3 {
            let (Some(t), Some(ri), Some(ui)) = (camera.target.get_mut(i), right.get(i), up.get(i))
            else {
                continue;
            };
            *t += r * (c * ri + s * ui);
        }

        // Zoom: in to about a third of the distance, out to almost three
        // times it, and back.
        let z = phase(2.0 / 3.0);
        camera.zoom(2.0f64.powf(-1.5 * (tau * z).sin()));
        camera
    }
}

/// Frame times collected during a run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameTimes {
    times: Vec<Duration>,
}

/// Summary of a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSummary {
    /// Frames measured.
    pub frames: usize,
    /// Mean frame time.
    pub mean: Duration,
    /// Median frame time.
    pub p50: Duration,
    /// 95th percentile frame time, the 4.2 pass figure.
    pub p95: Duration,
    /// 99th percentile frame time.
    pub p99: Duration,
    /// Longest frame time.
    pub max: Duration,
}

impl FrameSummary {
    /// Whether the run passes the 4.2 criterion: p95 of 33 ms or less.
    pub fn passes(&self) -> bool {
        self.frames > 0 && self.p95 <= P95_LIMIT
    }
}

impl FrameTimes {
    /// Records one frame time.
    pub fn push(&mut self, time: Duration) {
        self.times.push(time);
    }

    /// Number of frames recorded.
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// True when nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// The summary, or `None` with no frames. Percentiles use the nearest
    /// rank: the smallest time at or above that share of the frames.
    pub fn summary(&self) -> Option<FrameSummary> {
        let mut sorted = self.times.clone();
        sorted.sort_unstable();
        let n = sorted.len();
        let max = *sorted.last()?;
        let rank = |p: f64| {
            let k = ((p * n as f64).ceil() as usize).clamp(1, n);
            sorted.get(k - 1).copied().unwrap_or(max)
        };
        let total: Duration = sorted.iter().sum();
        Some(FrameSummary {
            frames: n,
            mean: total / u32::try_from(n).unwrap_or(u32::MAX),
            p50: rank(0.50),
            p95: rank(0.95),
            p99: rank(0.99),
            max,
        })
    }
}
