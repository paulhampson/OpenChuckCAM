// SPDX-License-Identifier: AGPL-3.0-or-later
//! `openchuck-ui --bench`: the NFR-03 frame-time benchmark on the product
//! viewport (verification plan 4.2; work breakdown slice 8 item 7; decision
//! record 0026 item 9).
//!
//! The model is added to a new project in the temporary folder through the
//! application API, like any other model. Once the scene is drawn, the
//! camera follows the scripted path while the interval between frames is
//! recorded, with vsync off. The report goes to standard output and, with
//! `--out`, to a file. The exit code is non-zero when the run fails the 33 ms
//! p95 criterion.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use openchuck_app::{Application, Threads};
use openchuck_viewport::DisplayMode;
use openchuck_viewport::bench::{CameraPath, FrameTimes, P95_LIMIT, STANDARD_DURATION};

/// Frames drawn before measuring, so start-up and the first upload do not count.
const WARM_UP_FRAMES: u32 = 30;

/// The benchmark's command-line options.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The model file.
    pub model: PathBuf,
    /// How long the path runs.
    pub duration: Duration,
    /// The display mode (4.2: shaded with edges).
    pub mode: DisplayMode,
    /// Where to write the report as well as standard output.
    pub out: Option<PathBuf>,
}

impl Options {
    /// Reads `--bench MODEL [--seconds N] [--mode shaded|edges|wireframe]
    /// [--out FILE]`. `None` when `--bench` is absent.
    pub fn parse(args: &[String]) -> Option<Result<Self, String>> {
        let at = args.iter().position(|a| a == "--bench")?;
        Some(Self::parse_after(args, at))
    }

    fn parse_after(args: &[String], at: usize) -> Result<Self, String> {
        let model = args
            .get(at + 1)
            .filter(|a| !a.starts_with("--"))
            .ok_or("--bench needs a model file (make one with the bench_model example)")?;
        let mut options = Self {
            model: PathBuf::from(model),
            duration: STANDARD_DURATION,
            mode: DisplayMode::ShadedWithEdges,
            out: None,
        };
        let mut rest = args.iter().skip(at + 2);
        while let Some(flag) = rest.next() {
            let value = rest.next().ok_or_else(|| format!("{flag} needs a value"))?;
            match flag.as_str() {
                "--seconds" => {
                    let s: f64 = value
                        .parse()
                        .ok()
                        .filter(|s: &f64| s.is_finite() && *s > 0.0)
                        .ok_or_else(|| format!("--seconds: '{value}' is not a positive number"))?;
                    options.duration = Duration::from_secs_f64(s);
                }
                "--mode" => {
                    options.mode = match value.as_str() {
                        "shaded" => DisplayMode::Shaded,
                        "edges" => DisplayMode::ShadedWithEdges,
                        "wireframe" => DisplayMode::Wireframe,
                        _ => {
                            return Err(format!(
                                "--mode: '{value}' is not shaded, edges or wireframe"
                            ));
                        }
                    }
                }
                "--out" => options.out = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown benchmark option '{flag}'")),
            }
        }
        Ok(options)
    }
}

/// The temporary project folder of this process's benchmark run.
pub fn folder() -> PathBuf {
    std::env::temp_dir().join(format!("openchuck-bench-{}", std::process::id()))
}

/// Creates a project in [`folder`] holding a copy of the model.
pub fn project(model: &Path) -> Result<Application, String> {
    let folder = folder();
    let _ = std::fs::remove_dir_all(&folder);
    let mut app = Application::create(&folder, "NFR-03 benchmark", Threads::All)
        .map_err(|e| e.to_string())?;
    let models = folder.join("models");
    std::fs::create_dir_all(&models).map_err(|e| e.to_string())?;
    let name = model.file_name().ok_or("the model path has no file name")?;
    let copy = models.join(name);
    std::fs::copy(model, &copy).map_err(|e| format!("{}: {e}", model.display()))?;
    app.add_model(&copy, None).map_err(|e| e.to_string())?;
    Ok(app)
}

/// Where a run is.
#[derive(Debug)]
enum Stage {
    /// Waiting for the scene to be built and drawn.
    Waiting,
    /// Drawing frames before measuring.
    WarmingUp(u32),
    /// Measuring.
    Running {
        path: CameraPath,
        start: Instant,
        last: Instant,
        times: FrameTimes,
    },
    /// Finished.
    Done,
}

/// A benchmark run driven from the shell's frame loop.
#[derive(Debug)]
pub struct Run {
    options: Options,
    adapter: String,
    stage: Stage,
    /// Set to whether the run passed, once it is done.
    outcome: Arc<OnceLock<bool>>,
}

/// What the frame loop should do after [`Run::frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Keep drawing.
    Continue,
    /// The run is over: close the window.
    Finish,
}

/// What the run needs to know about the scene each frame.
#[derive(Debug, Clone, Copy)]
pub struct SceneState<'a> {
    /// The scene has been drawn at least once.
    pub ready: bool,
    /// The camera fitted to the scene.
    pub camera: &'a openchuck_viewport::Camera,
    /// Triangles drawn.
    pub triangles: usize,
    /// Feature edges drawn.
    pub feature_edges: usize,
    /// The viewport size in physical pixels.
    pub pixels: [u32; 2],
}

impl Run {
    /// A run with `options` on the adapter described by `adapter`. Whether
    /// it passed is set in `outcome` when it ends.
    pub fn new(options: Options, adapter: String, outcome: Arc<OnceLock<bool>>) -> Self {
        Self {
            options,
            adapter,
            stage: Stage::Waiting,
            outcome,
        }
    }

    /// The display mode the run uses.
    pub fn mode(&self) -> DisplayMode {
        self.options.mode
    }

    /// Advances the run by one frame and returns the camera to draw with,
    /// if the run sets it.
    pub fn frame(&mut self, scene: SceneState<'_>) -> (Step, Option<openchuck_viewport::Camera>) {
        let now = Instant::now();
        match &mut self.stage {
            Stage::Waiting => {
                if scene.ready {
                    self.stage = Stage::WarmingUp(WARM_UP_FRAMES);
                }
                (Step::Continue, None)
            }
            Stage::WarmingUp(left) => {
                if *left == 0 {
                    let path = CameraPath::new(*scene.camera, self.options.duration);
                    self.stage = Stage::Running {
                        path,
                        start: now,
                        last: now,
                        times: FrameTimes::default(),
                    };
                } else {
                    *left -= 1;
                }
                (Step::Continue, None)
            }
            Stage::Running {
                path,
                start,
                last,
                times,
            } => {
                times.push(now - *last);
                *last = now;
                let elapsed = now - *start;
                if elapsed < path.duration() {
                    return (Step::Continue, Some(path.at(elapsed)));
                }
                let times = std::mem::take(times);
                self.stage = Stage::Done;
                let report = self.report(&times, &scene);
                println_report(&report);
                if let Some(out) = &self.options.out
                    && let Err(e) = std::fs::write(out, &report)
                {
                    eprint_line(&format!("error: {}: {e}", out.display()));
                }
                (Step::Finish, None)
            }
            Stage::Done => (Step::Finish, None),
        }
    }

    fn report(&self, times: &FrameTimes, scene: &SceneState<'_>) -> String {
        let ms = |d: Duration| format!("{:.2} ms", d.as_secs_f64() * 1000.0);
        let mut lines = vec![
            "OpenChuckCAM NFR-03 frame-time benchmark (verification plan 4.2)".to_owned(),
            format!("model:         {}", self.options.model.display()),
            format!("triangles:     {}", scene.triangles),
            format!("feature edges: {}", scene.feature_edges),
            format!("display mode:  {}", self.options.mode.name()),
            format!(
                "viewport:      {} x {} pixels",
                scene.pixels[0], scene.pixels[1]
            ),
            format!("adapter:       {}", self.adapter),
            format!(
                "path:          {:.1} s scripted orbit, pan and zoom",
                self.options.duration.as_secs_f64()
            ),
            "frame time:    interval between frames, vsync off".to_owned(),
        ];
        match times.summary() {
            Some(s) => {
                let _ = self.outcome.set(s.passes());
                lines.push(format!("frames:        {}", s.frames));
                lines.push(format!("mean:          {}", ms(s.mean)));
                lines.push(format!("p50:           {}", ms(s.p50)));
                lines.push(format!(
                    "p95:           {} (limit {})",
                    ms(s.p95),
                    ms(P95_LIMIT)
                ));
                lines.push(format!("p99:           {}", ms(s.p99)));
                lines.push(format!("max:           {}", ms(s.max)));
                lines.push(format!(
                    "result:        {}",
                    if s.passes() { "PASS" } else { "FAIL" }
                ));
            }
            None => {
                let _ = self.outcome.set(false);
                lines.push("result:        FAIL (no frames measured)".to_owned());
            }
        }
        lines.join("\n") + "\n"
    }
}

#[allow(clippy::print_stdout)]
fn println_report(report: &str) {
    print!("{report}");
}

#[allow(clippy::print_stderr)]
fn eprint_line(line: &str) {
    eprintln!("{line}");
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn req_nfr_03_bench_options_parse() {
        assert!(Options::parse(&args("openchuck-ui project")).is_none());
        let o = Options::parse(&args("ui --bench m.stl")).unwrap().unwrap();
        assert_eq!(o.model, PathBuf::from("m.stl"));
        assert_eq!(o.duration, STANDARD_DURATION);
        assert_eq!(o.mode, DisplayMode::ShadedWithEdges);
        let o = Options::parse(&args(
            "ui --bench m.stl --seconds 5 --mode shaded --out r.txt",
        ))
        .unwrap()
        .unwrap();
        assert_eq!(o.duration, Duration::from_secs(5));
        assert_eq!(o.mode, DisplayMode::Shaded);
        assert_eq!(o.out, Some(PathBuf::from("r.txt")));
        for bad in [
            "ui --bench",
            "ui --bench --seconds 5",
            "ui --bench m.stl --seconds -1",
            "ui --bench m.stl --mode glossy",
            "ui --bench m.stl --frames 3",
            "ui --bench m.stl --out",
        ] {
            assert!(Options::parse(&args(bad)).unwrap().is_err(), "{bad}");
        }
    }
}
