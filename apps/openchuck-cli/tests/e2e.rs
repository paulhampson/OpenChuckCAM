// SPDX-License-Identifier: AGPL-3.0-or-later
//! System-level scenarios with the command-line tool as the harness
//! (verification plan sections 5 and 9; work breakdown slice 20 items 3, 6
//! and 7; decision record 0096).
//!
//! The sample projects are the reference job set on the Makera Z1 profile
//! ([docs/reference-jobs.md](../../../docs/reference-jobs.md)). Each is built
//! as a user would build it with the command-line tool: the generated part
//! is written as an STL file and imported, then the machine, the setup and
//! the tool are set. The one step done through the application API is adding
//! the operation, because the command-line tool has no command for it
//! (OI-310). Every run is then a separate `openchuck-cli` process, so nothing
//! is shared between thread counts but the project folder.
//!
//! The undo and redo scenario runs through the application API the tool
//! wires, since each command-line run is one command (OI-313).
//!
//! `.github/workflows/e2e.yml` runs this file with the network unavailable
//! (NFR-09).

// Test code may unwrap, index and panic (NFR-07 applies to library code).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use openchuck_app::project::id::OperationId;
use openchuck_app::project::{Item, Operation, OrderKey, ParameterValue, Placement};
use openchuck_app::{Application, Command, Event, PostSelection, Threads};
use openchuck_testkit::determinism::thread_counts;
use openchuck_testkit::parts::{Cone, HelicalGroove, LobedVase, RadialPart, Sphere};

/// The two shipped posts and the extension of their golden programs.
const POSTS: [(&str, &str); 2] = [("makera-z1", "nc"), ("linuxcnc", "ngc")];

/// One sample project: a reference job (`crates/openchuck-app/tests/reference_jobs.rs`
/// builds the same jobs through the application API; the golden programs
/// tie the two together, so a drift in either fails).
struct Sample {
    name: &'static str,
    part: Box<dyn RadialPart>,
    radius: f64,
    strategy: &'static str,
    /// Mesh stations along X and round the axis.
    mesh: (u32, u32),
    parameters: Vec<(&'static str, ParameterValue)>,
    /// How far the stock runs past the part at each end, in millimetres.
    margin: f64,
    /// How much wider the stock is than the part's widest, in millimetres.
    oversize: f64,
    /// Workholding options of `openchuck-cli setup` (SET-09, OP-21).
    holding: Vec<&'static str>,
}

fn cone() -> Box<dyn RadialPart> {
    Box::new(Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    })
}

fn angle_step(degrees: f64) -> Vec<(&'static str, ParameterValue)> {
    vec![("max_angle_step", ParameterValue::Number(degrees))]
}

fn sample(name: &'static str, part: Box<dyn RadialPart>, radius: f64) -> Sample {
    Sample {
        name,
        part,
        radius,
        strategy: "helical_finishing",
        mesh: (121, 144),
        parameters: angle_step(5.0),
        margin: 1.0,
        oversize: 2.0,
        holding: Vec::new(),
    }
}

fn samples() -> Vec<Sample> {
    let groove = HelicalGroove {
        radius: 12.0,
        length: 30.0,
        pitch: 12.0,
        depth: 1.5,
    };
    let lobed = LobedVase {
        base: 10.0,
        wave: 1.5,
        wave_length: 40.0,
        lobe: 1.0,
        lobes: 3,
        length: 20.0,
    };
    vec![
        sample("groove", Box::new(groove), 12.0),
        sample("cone", cone(), 10.0),
        sample("sphere", Box::new(Sphere { radius: 10.0 }), 10.0),
        Sample {
            strategy: "simultaneous_finishing",
            mesh: (201, 720),
            parameters: angle_step(2.0),
            ..sample("lobed", Box::new(lobed), 12.5)
        },
        Sample {
            strategy: "axial_finishing",
            parameters: vec![("max_x_step", ParameterValue::Number(3.0))],
            ..sample("cone_axial", cone(), 10.0)
        },
        Sample {
            strategy: "indexed_finishing",
            parameters: vec![
                ("angles", ParameterValue::Text("0, 90, 180, 270".into())),
                ("max_x_step", ParameterValue::Number(3.0)),
            ],
            ..sample("cone_indexed", cone(), 10.0)
        },
        Sample {
            strategy: "axial_finishing",
            parameters: vec![
                ("max_x_step", ParameterValue::Number(3.0)),
                ("entry", ParameterValue::Text("lead".into())),
            ],
            ..sample("cone_axial_lead", cone(), 10.0)
        },
        // Stock X -12 to 42, diameter 20; held 4 mm and 2 mm; a tab between
        // each held region and the part (decision record 0035).
        Sample {
            margin: 12.0,
            oversize: 0.0,
            holding: vec![
                "--held-start",
                "4",
                "--held-end",
                "2",
                "--tab",
                "cylinder=-8,8,20",
                "--tab",
                "frustum=30,10,10,20",
            ],
            ..sample("cone_held_tabs", cone(), 10.0)
        },
    ]
}

struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn cli<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> Outcome {
    let output = assert_cmd::Command::cargo_bin("openchuck-cli")
        .unwrap()
        .args(args)
        .output()
        .unwrap();
    Outcome {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Runs the tool and requires success.
fn cli_ok<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> Outcome {
    let out = cli(args);
    let shown: Vec<_> = args.iter().map(|a| a.as_ref().to_string_lossy()).collect();
    assert_eq!(
        out.code,
        Some(0),
        "openchuck-cli {}\n{}\n{}",
        shown.join(" "),
        out.stdout,
        out.stderr
    );
    out
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openchuck-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The part as ASCII STL. Shortest round-trip formatting keeps every vertex
/// exact, so the imported mesh is the generated one.
fn write_stl(part: &dyn RadialPart, mesh: (u32, u32), path: &Path) {
    use std::fmt::Write as _;
    let mesh = part.mesh(mesh.0, mesh.1);
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        let v: Vec<_> = t.iter().map(|i| mesh.vertices[*i as usize]).collect();
        let c = (v[1] - v[0]).cross(v[2] - v[0]);
        let len = c.length();
        let n = if len > 0.0 { c * (1.0 / len) } else { c };
        writeln!(
            text,
            "facet normal {:e} {:e} {:e}\n outer loop",
            n.x, n.y, n.z
        )
        .unwrap();
        for p in v {
            writeln!(text, "  vertex {:e} {:e} {:e}", p.x, p.y, p.z).unwrap();
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text).unwrap();
}

/// The reference tool as a library file (the 3 mm ball of the reference job set).
const BALL: &str = r#"# SPDX-License-Identifier: AGPL-3.0-or-later
schema = 1
id = "5f0c2d1e-7a3b-4c6d-8e9f-0000000000e2"
name = "3 mm ball"

[tool]
diameter = 3.0
flute_length = 8.0
overall_length = 40.0
shank_diameter = 3.0
flute_count = 2

[tool.shape]
type = "ball_end"

[cutting_data.oak]
spindle_speed = 12000.0
feed_per_tooth = 0.02
plunge_feed = 150.0
stepover = 2.0
stepdown = 1.0
"#;

/// Builds the sample project in `root/<name>` from the command line: new
/// project, import, machine, setup, tool. Then adds the operation through
/// the application API (OI-310). Returns the project folder.
fn build(sample: &Sample, root: &Path) -> PathBuf {
    let project = root.join(sample.name);
    let p = project.as_os_str();
    cli_ok(&["new".as_ref(), p, "--name".as_ref(), sample.name.as_ref()]);
    std::fs::create_dir_all(project.join("models")).unwrap();
    let stl = project.join("models/part.stl");
    write_stl(sample.part.as_ref(), sample.mesh, &stl);
    cli_ok(&["import".as_ref(), p, stl.as_os_str()]);
    cli_ok(&["machine".as_ref(), p, "makera-z1".as_ref()]);

    let (x0, x1) = sample.part.x_range();
    let stock = format!(
        "cylinder={},{},{}",
        2.0 * sample.radius + sample.oversize,
        x1 - x0 + 2.0 * sample.margin,
        x0 - sample.margin
    );
    let mut setup: Vec<OsString> = [
        "setup",
        "",
        "--new",
        "Setup 1",
        "--part",
        "models/part.stl",
        "--stock",
        &stock,
        "--material",
        "oak",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    setup[1] = p.to_owned();
    setup.extend(sample.holding.iter().map(OsString::from));
    let out = cli_ok(&setup);
    // Stock exactly as wide as the part draws a false "outside the stock"
    // warning: the check compares radii with no tolerance (OI-314).
    if sample.oversize > 0.0 {
        assert!(out.stdout.contains("no problems found"), "{}", out.stdout);
    }

    let library = root.join(format!("{}-library", sample.name));
    std::fs::create_dir_all(&library).unwrap();
    std::fs::write(library.join("3-mm-ball.toml"), BALL).unwrap();
    let out = cli_ok(&[
        "tool".as_ref(),
        p,
        "3 mm ball".as_ref(),
        "--add".as_ref(),
        "--library".as_ref(),
        library.as_os_str(),
    ]);
    assert!(out.stdout.contains("as T1"), "{}", out.stdout);

    let mut app = Application::open(&project, Threads::Exactly(1)).unwrap();
    let setup = app.project().setups()[0].id;
    let tool = app.project().tools()[0].id;
    let parameters: BTreeMap<String, ParameterValue> = sample
        .parameters
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    app.execute(Command::Add {
        item: Item::Operation(Operation {
            enabled: true,
            id: OperationId::new(),
            order: OrderKey::default(),
            name: "Finish".into(),
            setup,
            strategy: sample.strategy.into(),
            tool: Some(tool),
            selections: Vec::new(),
            boundary: None,
            avoid: Vec::new(),
            check: Vec::new(),
            parameters,
        }),
        placement: Placement::End,
    })
    .unwrap();
    app.save().unwrap();
    project
}

/// The text of a program as its golden file keeps it: the operation's id,
/// new for every project, is replaced by `<id>`.
fn stable(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.starts_with("(operation ") {
                "(operation <id>)\n".to_owned()
            } else {
                format!("{l}\n")
            }
        })
        .collect()
}

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden/reference")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The files in `folder` by name, with their bytes.
fn files(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            for (inner, bytes) in files(&path) {
                found.insert(format!("{name}/{inner}"), bytes);
            }
        } else {
            found.insert(name, std::fs::read(&path).unwrap());
        }
    }
    found
}

/// Regenerates with `post` at `threads` into `out`, requires gates 1 and 2
/// to pass, and returns the one program written.
fn regenerate(project: &Path, post: &str, threads: usize, out: &Path) -> String {
    let result = cli_ok(&[
        "--threads".as_ref(),
        threads.to_string().as_ref(),
        "regenerate".as_ref(),
        project.as_os_str(),
        "--post".as_ref(),
        post.as_ref(),
        "--tool-numbers".as_ref(),
        "keep".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(
        result.stdout.contains("replay matches the toolpath") && !result.stdout.contains("check:"),
        "{}",
        result.stdout
    );
    assert!(
        result.stdout.contains("must be verified by the user"),
        "{}",
        result.stdout
    );
    let written = files(out);
    assert_eq!(written.len(), 1, "{:?}", written.keys());
    String::from_utf8(written.into_values().next().unwrap()).unwrap()
}

/// The sample of that name.
fn named(name: &str) -> Sample {
    samples()
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no sample project '{name}'"))
}

/// Slice 20 items 3 and 6: the sample project runs from import to checked
/// G-code with both posts, gives byte-identical programs at 1, 2 and N
/// threads, and matches the reviewed golden programs. The project files
/// are left as they were, and loading then saving changes nothing.
fn end_to_end(name: &str) {
    let sample = named(name);
    let root = scratch(&format!("samples-{name}"));
    let project = build(&sample, &root);
    cli_ok(&["validate".as_ref(), project.as_os_str()]);
    let before = files(&project);
    for (post, ext) in POSTS {
        let mut programs = Vec::new();
        for threads in thread_counts() {
            let out = root.join(format!("{post}-{threads}"));
            programs.push((threads, regenerate(&project, post, threads, &out)));
        }
        let (_, first) = &programs[0];
        for (threads, program) in &programs {
            assert!(
                program == first,
                "{name} / {post}: the program at {threads} threads differs from the one at {} (NFR-06)",
                programs[0].0
            );
        }
        assert!(
            stable(first) == golden(&format!("{name}_{post}.{ext}")),
            "{name} / {post}: the program differs from its golden file (NFR-08); \
             `cargo test -p openchuck-app --test reference_jobs` shows the diff"
        );
    }
    let out = cli_ok(&["resave".as_ref(), project.as_os_str()]);
    assert_eq!(files(&project), before, "{name}: {}", out.stdout);
    std::fs::remove_dir_all(root).ok();
}

/// Slice 20 item 6: the material removal simulation of the sample project
/// finds no overcut and no rapid move through material (SIM-03, SIM-05).
fn simulates(name: &str) {
    let root = scratch(&format!("simulate-{name}"));
    let project = build(&named(name), &root);
    let out = cli_ok(&["simulate".as_ref(), project.as_os_str()]);
    assert!(!out.stdout.contains("no operations"), "{}", out.stdout);
    std::fs::remove_dir_all(root).ok();
}

/// One end-to-end test and one simulation test per sample project, so the
/// test runner spreads them over cores and CI shares.
macro_rules! sample_tests {
    ($($name:ident),* $(,)?) => {
        $(mod $name {
            #[test]
            fn req_gc_07_req_nfr_06_req_nfr_08_req_gc_11_runs_end_to_end_at_1_2_and_n_threads() {
                super::end_to_end(stringify!($name));
            }
            #[test]
            fn req_sim_03_req_sim_05_simulates_without_overcut() {
                super::simulates(stringify!($name));
            }
        })*
        /// Every sample project has its pair of tests above.
        const SAMPLE_TESTS: &[&str] = &[$(stringify!($name)),*];
    };
}

sample_tests!(
    groove,
    cone,
    sphere,
    lobed,
    cone_axial,
    cone_indexed,
    cone_axial_lead,
    cone_held_tabs,
);

/// Slice 20 item 3 asks for all sample projects: every reference job with
/// golden programs is a sample project with its tests, except `drilling`,
/// which needs the STEP kernel (OI-312).
#[test]
fn req_gc_07_every_reference_job_is_a_sample_project_with_its_tests() {
    let names: Vec<&str> = samples().iter().map(|s| s.name).collect();
    assert_eq!(names, SAMPLE_TESTS);
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden/reference");
    let mut jobs: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            name.strip_suffix("_makera-z1.nc").map(str::to_owned)
        })
        .filter(|job| job != "drilling")
        .collect();
    jobs.sort();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(jobs, sorted);
}

/// Starts `regenerate --out` on `project` and returns the child process
/// once it has begun generating. The tool prints the tool numbers on
/// standard error after it has loaded the project and just before it
/// generates (GC-14), so that line is the observable start of the job:
/// no fixed delay is involved.
fn spawn_generating(project: &Path, out: &Path) -> std::process::Child {
    use std::io::BufRead as _;
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("openchuck-cli"))
        .args(["--threads", "1", "regenerate"])
        .arg(project)
        .args(["--post", "linuxcnc", "--tool-numbers", "keep", "--out"])
        .arg(out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = std::io::BufReader::new(child.stderr.take().unwrap());
    let mut seen = Vec::new();
    for line in stderr.lines() {
        let line = line.unwrap();
        let started = line.trim_start().starts_with("T1 ");
        seen.push(line);
        if started {
            return child;
        }
    }
    let status = child.wait().unwrap();
    panic!("the run ended ({status}) before it began generating: {seen:?}");
}

/// Interrupts `child` as Ctrl-C would on Unix; elsewhere ends it.
fn interrupt(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let status = std::process::Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap();
        assert!(status.success());
    }
    #[cfg(not(unix))]
    child.kill().unwrap();
}

/// Slice 20 item 7, UI-09: a command-line run interrupted mid-job writes
/// no G-code and leaves the project as it was, and the next run gives the
/// same program as one that was never interrupted (the golden program).
/// The interrupt follows the job's first output, never a fixed delay; the
/// lobed job then has several seconds of generation left on one thread.
#[test]
fn req_ui_09_req_gc_11_an_interrupted_cli_run_writes_nothing_and_changes_nothing() {
    let root = scratch("interrupt");
    let lobed = named("lobed");
    let project = build(&lobed, &root);
    let before = files(&project);
    let out = root.join("out");

    let mut child = spawn_generating(&project, &out);
    assert!(
        child.try_wait().unwrap().is_none(),
        "the run finished before it could be interrupted"
    );
    interrupt(&mut child);
    let status = child.wait().unwrap();
    assert!(!status.success(), "{status}");
    assert!(files(&out).is_empty(), "{:?}", files(&out).keys());
    assert_eq!(files(&project), before);

    // The next run is unaffected.
    let program = regenerate(&project, "linuxcnc", 1, &out);
    assert!(stable(&program) == golden("lobed_linuxcnc.ngc"));
    std::fs::remove_dir_all(root).ok();
}

/// Waits for the first progress report of a job, so a cancel lands while it
/// is generating.
fn wait_for_progress(app: &Application) {
    let start = Instant::now();
    loop {
        if app
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::JobProgress { .. }))
        {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(60), "no progress");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Slice 20 item 7, UI-09, INT-04: a regeneration cancelled mid-job
/// reports cancellation, not a result, and the next run gives the same
/// G-code as one that was never cancelled.
#[test]
fn req_ui_09_a_regeneration_cancelled_mid_job_leaves_the_next_run_identical() {
    let root = scratch("cancel");
    let groove = named("groove");
    let project = build(&groove, &root);
    let mut app = Application::open(&project, Threads::Exactly(1)).unwrap();
    app.set_post(Some(PostSelection::parse("linuxcnc")));

    let job = app.start_regenerate();
    wait_for_progress(&app);
    job.cancel();
    let err = job.wait().unwrap_err();
    assert!(err.is_cancelled(), "{err}");

    let result = app.regenerate().unwrap();
    let (_, text) = &result.gcode[0];
    assert!(stable(text) == golden("groove_linuxcnc.ngc"));
    std::fs::remove_dir_all(root).ok();
}

/// Slice 20 item 7, UI-08: on a sample project, an edit changes the G-code;
/// undo gives back the G-code and the saved files byte for byte; redo gives
/// the edited G-code again.
#[test]
fn req_ui_08_undo_and_redo_on_a_sample_project_restore_the_g_code_and_the_files() {
    let root = scratch("undo");
    let cone = named("cone");
    let project = build(&cone, &root);
    let saved = files(&project);
    let mut app = Application::open(&project, Threads::Exactly(2)).unwrap();
    app.set_post(Some(PostSelection::parse("makera-z1")));
    let original = app.regenerate().unwrap().gcode;
    assert!(stable(&original[0].1) == golden("cone_makera-z1.nc"));

    let operation = app.project().operations()[0].id;
    app.execute(Command::SetParameter {
        operation,
        name: "max_angle_step".into(),
        value: Some(ParameterValue::Number(10.0)),
    })
    .unwrap();
    let edited = app.regenerate().unwrap().gcode;
    assert_ne!(edited, original);
    app.save().unwrap();
    assert_ne!(files(&project), saved);

    assert!(app.undo().unwrap());
    assert_eq!(app.regenerate().unwrap().gcode, original);
    app.save().unwrap();
    assert_eq!(files(&project), saved);

    assert!(app.redo().unwrap());
    assert_eq!(app.regenerate().unwrap().gcode, edited);
    assert!(!app.can_redo());

    // The undone state regenerated in a fresh process matches too.
    assert!(app.undo().unwrap());
    app.save().unwrap();
    let out = root.join("out");
    let program = regenerate(&project, "makera-z1", 1, &out);
    assert!(stable(&program) == golden("cone_makera-z1.nc"));
    std::fs::remove_dir_all(root).ok();
}
