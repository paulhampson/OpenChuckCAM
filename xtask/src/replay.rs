// SPDX-License-Identifier: AGPL-3.0-or-later
//! `cargo xtask replay`: gate 3 of the G-code verification plan (verification
//! plan section 6; work breakdown slice 6 items 9 and 11).
//!
//! Runs LinuxCNC programs in its headless XYZA simulator and compares what the
//! simulator reached with the replay of the same file (`openchuck-cli
//! gcode-sim`). LinuxCNC comes from one of three places:
//!
//! - `--docker IMAGE`: the image built from `docker/linuxcnc/Dockerfile` (what CI uses);
//! - `LINUXCNC_RIP`: a run-in-place build, whose `scripts/rip-environment` is sourced;
//! - `linuxcnc` on the `PATH` (a package install).
//!
//! The simulator trivkins treats A as a plain number, so this proves the
//! dialect, the reached values and the absence of faults, not the A direction.
//! Run it on an otherwise idle machine: the driver samples every millisecond in
//! real time.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::Findings;

/// What to run and how.
#[derive(Debug)]
pub struct Options {
    /// Programs to run; empty means every LinuxCNC golden program.
    pub programs: Vec<PathBuf>,
    /// Run LinuxCNC in this Docker image.
    pub docker: Option<String>,
    /// Keep each JSON report in this folder.
    pub keep_reports: Option<PathBuf>,
    /// Largest per-line difference allowed, mm or degrees.
    pub tolerance: f64,
    /// Give up on one program after this many seconds.
    pub timeout_seconds: u64,
}

/// The LinuxCNC golden programs: the reference jobs and the small part.
fn default_programs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut programs = Vec::new();
    for dir in [
        root.join("testdata/golden"),
        root.join("testdata/golden/reference"),
    ] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry?.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name.contains("linuxcnc") && path.extension().is_some_and(|e| e == "ngc") {
                programs.push(path);
            }
        }
    }
    programs.sort();
    if programs.is_empty() {
        bail!("no LinuxCNC golden programs found under testdata/golden");
    }
    Ok(programs)
}

/// Builds the command that runs LinuxCNC on `checks.ini` in `work`.
fn linuxcnc_command(
    options: &Options,
    work: &Path,
    program: &Path,
    report: &Path,
) -> Result<Command> {
    if let Some(image) = &options.docker {
        // The container sees the work folder as /work; the program is copied in.
        let mut command = Command::new("docker");
        command
            .args(["run", "--rm", "--name", &container_name(work), "-v"])
            .arg(format!("{}:/work", work.display()))
            .args([
                "-w",
                "/work",
                "-e",
                "CHECK_PROGRAM=/work/program.ngc",
                "-e",
                "CHECK_REPORT=/work/report.json",
            ])
            .arg(image)
            .args(["linuxcnc", "checks.ini"]);
        std::fs::copy(program, work.join("program.ngc"))
            .with_context(|| format!("cannot copy '{}'", program.display()))?;
        return Ok(command);
    }
    let mut command;
    if let Ok(rip) = std::env::var("LINUXCNC_RIP") {
        // A run-in-place build needs its environment sourced; bash does that.
        command = Command::new("bash");
        command.arg("-c").arg(format!(
            ". \"{rip}/scripts/rip-environment\" && exec linuxcnc checks.ini"
        ));
    } else {
        command = Command::new("linuxcnc");
        command.arg("checks.ini");
    }
    command
        .current_dir(work)
        .env("CHECK_PROGRAM", program)
        .env("CHECK_REPORT", report);
    Ok(command)
}

/// The container's name for a run: the work folder's name is unique per process and program.
fn container_name(work: &Path) -> String {
    let leaf = work.file_name().and_then(|n| n.to_str()).unwrap_or("run");
    let safe: String = leaf
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("openchuck-{safe}")
}

/// Waits for `child`, killing it after `seconds`. Killing the `docker run` client does not stop
/// the container, so a Docker run's container is removed by name too: otherwise it would go on
/// using the CPU the retry needs.
fn wait_with_timeout(
    mut child: std::process::Child,
    seconds: u64,
    container: Option<&str>,
) -> Result<()> {
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(seconds) {
            child.kill().ok();
            child.wait().ok();
            if let Some(name) = container {
                Command::new("docker")
                    .args(["rm", "-f", name])
                    .output()
                    .ok();
            }
            bail!("LinuxCNC did not finish within {seconds} s");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Runs the programs and returns one finding per failure.
pub fn check(root: &Path, options: &Options) -> Result<Findings> {
    let programs = if options.programs.is_empty() {
        default_programs(root)?
    } else {
        options.programs.clone()
    };
    // The comparison is the CLI's `gcode-sim`.
    let status = Command::new("cargo")
        .args(["build", "--quiet", "-p", "openchuck-cli"])
        .current_dir(root)
        .status()
        .context("cannot run cargo")?;
    if !status.success() {
        bail!("building openchuck-cli failed");
    }
    let cli = root
        .join("target/debug")
        .join(format!("openchuck-cli{}", std::env::consts::EXE_SUFFIX));
    let config = root.join("tools/linuxcnc");
    let mut findings = Findings::new();
    for program in programs {
        let program = program
            .canonicalize()
            .with_context(|| format!("cannot find '{}'", program.display()))?;
        let name = program
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("program")
            .to_string();
        let mut outcome = run_once(options, &config, &cli, &program, &name)?;
        // The simulator runs in real time and samples every millisecond, so a loaded machine can
        // leave a gap that hides a corner. A real failure repeats; run a failing program once more
        // and report the second result, with the first one's reason.
        if let Some(first) = outcome.failure.clone() {
            println!(
                "{name}: failed ({first}); the longest sampling gap was {:.0} ms. Running it once more",
                outcome.max_gap.unwrap_or(0.0) * 1000.0
            );
            outcome = run_once(options, &config, &cli, &program, &name)?;
        }
        match outcome.failure {
            None => println!("{}", outcome.said),
            Some(why) => findings.push(format!("{name}: {why}")),
        }
    }
    Ok(findings)
}

/// What one run of one program found.
struct Outcome {
    /// The comparison's one-line result when it passed.
    said: String,
    /// Why it failed, if it did.
    failure: Option<String>,
    /// The longest pause between the driver's samples, in seconds.
    max_gap: Option<f64>,
}

/// The `max_gap` the driver wrote into its report.
fn report_max_gap(report: &Path) -> Option<f64> {
    let text = std::fs::read_to_string(report).ok()?;
    let at = text.find("\"max_gap\":")? + "\"max_gap\":".len();
    let rest = text.get(at..)?.trim_start();
    let end = rest.find([',', '}', '\n'])?;
    rest.get(..end)?.trim().parse().ok()
}

/// Runs `program` in LinuxCNC once and compares the report.
fn run_once(
    options: &Options,
    config: &Path,
    cli: &Path,
    program: &Path,
    name: &str,
) -> Result<Outcome> {
    let work = std::env::temp_dir().join(format!("openchuck-replay-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    for file in ["checks.ini", "sim.tbl", "sim_driver.py"] {
        std::fs::copy(config.join(file), work.join(file))
            .with_context(|| format!("cannot copy {file}"))?;
    }
    // The container runs as an unprivileged user that must write the report.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o777))?;
    }
    let report = work.join("report.json");
    let mut command = linuxcnc_command(options, &work, program, &report)?;
    command
        .stdout(std::fs::File::create(work.join("linuxcnc.log"))?)
        .stderr(Stdio::inherit());
    println!("{name}: running in the LinuxCNC simulator (real time)");
    let child = command.spawn().map_err(|e| {
        anyhow::anyhow!(
            "cannot start LinuxCNC ({e}): install it, set LINUXCNC_RIP, or use --docker IMAGE"
        )
    })?;
    let container = options.docker.as_ref().map(|_| container_name(&work));
    if let Err(e) = wait_with_timeout(child, options.timeout_seconds, container.as_deref()) {
        return Ok(Outcome {
            said: String::new(),
            failure: Some(e.to_string()),
            max_gap: None,
        });
    }
    if !report.is_file() || std::fs::metadata(&report)?.len() == 0 {
        let log = std::fs::read_to_string(work.join("linuxcnc.log")).unwrap_or_default();
        let tail: Vec<&str> = log.lines().rev().take(10).collect();
        return Ok(Outcome {
            said: String::new(),
            failure: Some(format!(
                "the simulator wrote no report; last output: {}",
                tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
            )),
            max_gap: None,
        });
    }
    let max_gap = report_max_gap(&report);
    if let Some(keep) = &options.keep_reports {
        std::fs::create_dir_all(keep)?;
        std::fs::copy(&report, keep.join(format!("{name}.json")))?;
    }
    let output = Command::new(cli)
        .arg("gcode-sim")
        .arg(program)
        .arg(&report)
        .args(["--tolerance", &options.tolerance.to_string()])
        .output()
        .context("cannot run openchuck-cli gcode-sim")?;
    let said = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let failure = (!output.status.success())
        .then(|| String::from_utf8_lossy(&output.stderr).trim().to_string());
    let _ = std::fs::remove_dir_all(&work);
    Ok(Outcome {
        said,
        failure,
        max_gap,
    })
}
