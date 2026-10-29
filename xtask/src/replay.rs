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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::Findings;

mod chunks;

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
    /// Run only one share of the programs, as `I/N`.
    pub shard: Option<String>,
}

/// Largest per-line difference allowed by default, mm or degrees.
pub const TOLERANCE: f64 = 0.1;

/// Seconds after which one program is given up by default.
pub const TIMEOUT_SECONDS: u64 = 900;

/// Seconds a LinuxCNC golden program takes in the simulator, which runs in real time, measured
/// on hosted runners (decision 0052). They only balance the shards: a program missing from this
/// list, or a changed time, makes a share longer or shorter and never drops a program.
const EXPECTED_SECONDS: &[(&str, u64)] = &[
    ("cone_axial_lead_linuxcnc.ngc", 295),
    ("cone_axial_linuxcnc.ngc", 278),
    ("cone_held_tabs_linuxcnc.ngc", 223),
    ("groove_linuxcnc.ngc", 159),
    ("cone_linuxcnc.ngc", 124),
    ("lobed_linuxcnc.ngc", 114),
    ("swarf_linuxcnc.ngc", 110),
    ("sphere_linuxcnc.ngc", 93),
    ("cone_indexed_linuxcnc.ngc", 40),
    ("post_linuxcnc_small_part.ngc", 35),
    ("drilling_linuxcnc.ngc", 30),
];

/// Expected time of a program that is not in the list.
const DEFAULT_SECONDS: u64 = 150;

/// Reads `I/N` (1-based) into `(index, count)`.
fn parse_shard(text: &str) -> Result<(usize, usize)> {
    let bad =
        || anyhow::anyhow!("--shard takes I/N with 1 <= I <= N, for example 2/3; got '{text}'");
    let (i, n) = text.split_once('/').ok_or_else(bad)?;
    let i: usize = i.trim().parse().map_err(|_| bad())?;
    let n: usize = n.trim().parse().map_err(|_| bad())?;
    if i == 0 || i > n {
        return Err(bad());
    }
    Ok((i, n))
}

/// Share `index` of `count` of `programs`, refusing a count that leaves a share with no program
/// (that share would pass without checking anything) before any share is built.
fn select_share(programs: &[PathBuf], index: usize, count: usize) -> Result<Vec<PathBuf>> {
    if count > programs.len() {
        bail!(
            "{count} shares for {} programs would leave a share with nothing to run: use at most {} shares",
            programs.len(),
            programs.len()
        );
    }
    let mine = shard_programs(programs, index, count);
    if mine.is_empty() {
        bail!("share {index}/{count} has no programs: use fewer shares");
    }
    Ok(mine)
}

/// Deals `programs` over `count` shares, longest first onto the share with the least expected
/// time (ties by name, so every runner computes the same shares from the same files), and
/// returns share `index` (1-based). Every program is in exactly one share.
fn shard_programs(programs: &[PathBuf], index: usize, count: usize) -> Vec<PathBuf> {
    let seconds = |p: &PathBuf| {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if name.starts_with("first_cut_linuxcnc.chunk-") {
            return chunks::SECONDS as u64;
        }
        EXPECTED_SECONDS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(DEFAULT_SECONDS, |(_, s)| *s)
    };
    let mut order: Vec<&PathBuf> = programs.iter().collect();
    order.sort_by(|a, b| {
        seconds(b)
            .cmp(&seconds(a))
            .then_with(|| a.file_name().cmp(&b.file_name()))
            .then_with(|| a.cmp(b))
    });
    let mut load = vec![0u64; count];
    let mut mine = Vec::new();
    for program in order {
        let (share, _) = load
            .iter()
            .enumerate()
            .min_by_key(|(i, l)| (**l, *i))
            .unwrap_or((0, &0));
        if let Some(slot) = load.get_mut(share) {
            *slot += seconds(program);
        }
        if share + 1 == index {
            mine.push(program.clone());
        }
    }
    mine.sort();
    mine
}

/// The LinuxCNC golden programs: the reference jobs, the small part and the
/// gate 5 first-cut project (decision record 0221, OI-914).
fn default_programs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut programs = Vec::new();
    for dir in [
        root.join("testdata/golden"),
        root.join("testdata/golden/reference"),
        root.join("testdata/golden/first_cut"),
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

/// Temporary verification fixtures; production programs are never rewritten.
struct PreparedPrograms {
    programs: Vec<PathBuf>,
    boundaries: BTreeMap<PathBuf, PathBuf>,
    work: PathBuf,
}
impl Drop for PreparedPrograms {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

fn prepare_programs(programs: Vec<PathBuf>) -> Result<PreparedPrograms> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let work = std::env::temp_dir().join(format!(
        "openchuck-replay-plan-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&work)?;
    // `check` looks programs up by canonical path; a symlinked temp folder
    // (macOS `/var`) would otherwise lose the boundary checks silently.
    let work = work.canonicalize()?;
    let mut out = PreparedPrograms {
        programs: Vec::new(),
        boundaries: BTreeMap::new(),
        work,
    };
    for program in programs {
        let program = program
            .canonicalize()
            .with_context(|| format!("cannot find '{}'", program.display()))?;
        if program
            .file_name()
            .is_none_or(|n| n != "first_cut_linuxcnc.ngc")
        {
            out.programs.push(program);
            continue;
        }
        let original = std::fs::read_to_string(&program)?;
        let parts = chunks::plan(&original, chunks::SECONDS)?;
        let total = parts.len();
        for (i, part) in parts.into_iter().enumerate() {
            let name = format!("first_cut_linuxcnc.chunk-{:03}-of-{total:03}", i + 1);
            let fixture = out.work.join(format!("{name}.ngc"));
            let expected = out.work.join(format!("{name}.boundaries.json"));
            std::fs::write(&fixture, &part.text)?;
            let manifest = chunks::Manifest {
                original_program: program.clone(),
                source_program: "first_cut_linuxcnc.ngc".into(),
                first_line: part.first_line,
                last_line: part.last_line,
                original_lines: part.source,
                estimated_seconds: part.estimated_seconds,
                states: part.boundaries,
                timeout_seconds: None,
            };
            std::fs::write(&expected, serde_json::to_vec_pretty(&manifest)?)?;
            out.boundaries.insert(fixture.clone(), expected);
            out.programs.push(fixture);
        }
        println!(
            "first_cut_linuxcnc.ngc: {total} bounded verification chunks; original lines and boundary continuity preserved"
        );
    }
    Ok(out)
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
            ]);
        if work.join("boundaries.json").is_file() {
            command.args(["-e", "CHECK_BOUNDARIES=/work/boundaries.json"]);
        }
        command.arg(image).args(["linuxcnc", "checks.ini"]);
        std::fs::copy(program, work.join("program.ngc"))
            .with_context(|| format!("cannot copy '{}'", program.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                work.join("program.ngc"),
                std::fs::Permissions::from_mode(0o644),
            )?;
        }
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
    if work.join("boundaries.json").is_file() {
        command.env("CHECK_BOUNDARIES", work.join("boundaries.json"));
    }
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
    // Expand before dealing shares: each bounded first-cut chunk is checked once.
    let prepared = prepare_programs(programs)?;
    let programs = match &options.shard {
        None => prepared.programs.clone(),
        Some(text) => {
            let (index, count) = parse_shard(text)?;
            let mine = select_share(&prepared.programs, index, count)?;
            println!(
                "share {index}/{count}: {} of {} programs",
                mine.len(),
                prepared.programs.len()
            );
            mine
        }
    };
    // The comparison is the CLI's `gcode-sim`.
    let status = Command::new("cargo")
        .args(["build", "--quiet", "--locked", "-p", "openchuck-cli"])
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
        let boundary = prepared.boundaries.get(&program).map(PathBuf::as_path);
        let mut outcome = timed_run(options, &config, &cli, &program, &name, boundary, 1)?;
        // The simulator runs in real time and samples every millisecond, so a loaded machine can
        // leave a gap that hides a corner. A real failure repeats; run a failing program once more
        // and report the second result, with the first one's reason.
        if let Some(first) = outcome.failure.clone() {
            println!(
                "{name}: failed ({first}); the longest sampling gap was {:.0} ms. Running it once more",
                outcome.max_gap.unwrap_or(0.0) * 1000.0
            );
            outcome = timed_run(options, &config, &cli, &program, &name, boundary, 2)?;
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
    /// A bounded chunk exceeded its limit; do not silently retry a long test.
    timed_out: bool,
}

fn timed_run(
    options: &Options,
    config: &Path,
    cli: &Path,
    program: &Path,
    name: &str,
    boundary: Option<&Path>,
    attempt: usize,
) -> Result<Outcome> {
    let started = Instant::now();
    let outcome = run_once(options, config, cli, program, name, boundary)?;
    let seconds = started.elapsed().as_secs_f64();
    println!("{name}: attempt {attempt} wall time {seconds:.3} s");
    if let Some(keep) = &options.keep_reports {
        std::fs::create_dir_all(keep)?;
        std::fs::write(
            keep.join(format!("{name}.attempt-{attempt}.json")),
            serde_json::to_vec_pretty(
                &serde_json::json!({"wall_seconds":seconds,"failure":outcome.failure,"max_gap":outcome.max_gap,"timed_out":outcome.timed_out}),
            )?,
        )?;
        if attempt == 1 && outcome.failure.is_some() && keep.join(format!("{name}.json")).is_file()
        {
            std::fs::copy(
                keep.join(format!("{name}.json")),
                keep.join(format!("{name}.first-report.json")),
            )?;
        }
    }
    if outcome.timed_out || (boundary.is_some() && seconds > 600.0) {
        bail!(
            "{name}: bounded replay exceeded its limit; stopped without further retry for an owner long-test decision"
        );
    }
    Ok(outcome)
}

/// Read only the small evidence header, skipping large path/row arrays.
#[derive(serde::Deserialize)]
struct ReportChecks {
    #[serde(default)]
    max_gap: Option<f64>,
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    boundaries: Vec<serde_json::Value>,
}

/// Runs `program` in LinuxCNC once and compares the report.
fn run_once(
    options: &Options,
    config: &Path,
    cli: &Path,
    program: &Path,
    name: &str,
    boundary: Option<&Path>,
) -> Result<Outcome> {
    let work = std::env::temp_dir().join(format!("openchuck-replay-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    for file in [
        "checks.ini",
        "sim.tbl",
        "sim_driver.py",
        "boundary_checks.py",
    ] {
        std::fs::copy(config.join(file), work.join(file))
            .with_context(|| format!("cannot copy {file}"))?;
    }
    let mut original_range = None;
    let timeout = if let Some(boundary) = boundary {
        let mut manifest: chunks::Manifest =
            serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(boundary)?))?;
        let timeout = options.timeout_seconds.min(600);
        original_range = Some((
            manifest.original_program.clone(),
            manifest.first_line,
            manifest.last_line,
        ));
        manifest.timeout_seconds = Some(timeout);
        let start = manifest.states.first().context("missing start boundary")?;
        let ini = std::fs::read_to_string(work.join("checks.ini"))?;
        std::fs::write(work.join("checks.ini"), chunks::start_config(&ini, start)?)?;
        std::fs::write(
            work.join("boundaries.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        timeout
    } else {
        options.timeout_seconds
    };
    // The container runs as an unprivileged user that must write the report.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o777))?;
        for file in [
            "checks.ini",
            "sim.tbl",
            "boundary_checks.py",
            "boundaries.json",
        ] {
            let path = work.join(file);
            if path.is_file() {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))?;
            }
        }
        std::fs::set_permissions(
            work.join("sim_driver.py"),
            std::fs::Permissions::from_mode(0o755),
        )?;
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
    if let Err(e) = wait_with_timeout(child, timeout, container.as_deref()) {
        return Ok(Outcome {
            said: String::new(),
            failure: Some(e.to_string()),
            max_gap: None,
            timed_out: boundary.is_some(),
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
            timed_out: false,
        });
    }
    let report_data: ReportChecks =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(&report)?))?;
    let max_gap = report_data.max_gap;
    let timed_out = boundary.is_some() && report_data.errors.iter().any(|e| e == "timeout");
    if let Some(keep) = &options.keep_reports {
        std::fs::create_dir_all(keep)?;
        std::fs::copy(&report, keep.join(format!("{name}.json")))?;
        if boundary.is_some() {
            std::fs::copy(program, keep.join(format!("{name}.program.ngc")))?;
            std::fs::copy(
                work.join("boundaries.json"),
                keep.join(format!("{name}.boundaries.json")),
            )?;
            std::fs::copy(
                work.join("checks.ini"),
                keep.join(format!("{name}.checks.ini")),
            )?;
        }
    }
    if boundary.is_some() && report_data.boundaries.len() != 2 {
        return Ok(Outcome {
            said: String::new(),
            failure: Some(format!(
                "simulator did not verify both source boundaries: {}",
                report_data.errors.join("; ")
            )),
            max_gap,
            timed_out,
        });
    }
    let mut compare = Command::new(cli);
    compare.arg("gcode-sim");
    if let Some((original, first, last)) = original_range {
        compare
            .arg(original)
            .arg(&report)
            .args(["--source-lines", &format!("{first}:{last}")]);
    } else {
        compare.arg(program).arg(&report);
    }
    let output = compare
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
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn programs(names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|n| PathBuf::from(format!("testdata/golden/reference/{n}")))
            .collect()
    }

    fn all_known() -> Vec<PathBuf> {
        let names: Vec<&str> = EXPECTED_SECONDS.iter().map(|(n, _)| *n).collect();
        programs(&names)
    }

    #[test]
    fn gate_3_every_program_is_in_exactly_one_share() {
        // The known programs plus two new ones that have no expected time.
        let mut list = all_known();
        list.extend(programs(&["new_a_linuxcnc.ngc", "new_b_linuxcnc.ngc"]));
        for count in 1..=list.len() {
            let mut seen: Vec<PathBuf> = Vec::new();
            for index in 1..=count {
                seen.extend(shard_programs(&list, index, count));
            }
            seen.sort();
            let mut want = list.clone();
            want.sort();
            assert_eq!(seen, want, "{count} shares");
        }
    }

    #[test]
    fn gate_3_shares_do_not_depend_on_the_order_of_the_list() {
        let list = all_known();
        let mut reversed = list.clone();
        reversed.reverse();
        for index in 1..=3 {
            assert_eq!(
                shard_programs(&list, index, 3),
                shard_programs(&reversed, index, 3)
            );
        }
    }

    #[test]
    fn gate_3_three_shares_are_balanced_and_none_is_empty() {
        let list = all_known();
        let total: u64 = EXPECTED_SECONDS.iter().map(|(_, s)| *s).sum();
        for index in 1..=3 {
            let mine = shard_programs(&list, index, 3);
            assert!(!mine.is_empty(), "share {index} is empty");
            let time: u64 = mine
                .iter()
                .map(|p| {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                    EXPECTED_SECONDS
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map_or(0, |(_, s)| *s)
                })
                .sum();
            // Within 40 % of an even split, so no runner carries most of the work.
            assert!(
                time * 3 * 10 <= total * 14,
                "share {index} takes {time} s of {total} s"
            );
        }
    }

    #[test]
    fn gate_3_more_shares_than_programs_is_refused_before_any_share_is_built() {
        let list = all_known();
        assert!(select_share(&list, 1, list.len()).is_ok());
        assert!(select_share(&list, 1, list.len() + 1).is_err());
        // A huge count must not be allocated.
        assert!(select_share(&list, 1, 1_000_000_000).is_err());
    }

    #[test]
    fn req_gc_10_the_replay_runs_the_first_cut_project_with_the_reference_jobs() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let programs = default_programs(&root).unwrap();
        let names: Vec<String> = programs
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_owned))
            .collect();
        for expected in ["first_cut_linuxcnc.ngc", "cone_linuxcnc.ngc"] {
            assert!(names.iter().any(|n| n == expected), "{expected}: {names:?}");
        }
        assert!(names.iter().all(|n| !n.contains("makera")), "{names:?}");
    }

    #[test]
    fn req_gc_10_req_nfr_06_first_cut_is_expanded_before_sharding_without_losing_a_chunk() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let prepared = prepare_programs(default_programs(&root).unwrap()).unwrap();
        assert!(prepared.boundaries.len() > 1);
        assert!(
            !prepared
                .programs
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == "first_cut_linuxcnc.ngc"))
        );
        let mut all = Vec::new();
        for index in 1..=3 {
            all.extend(select_share(&prepared.programs, index, 3).unwrap());
        }
        all.sort();
        let mut expected = prepared.programs.clone();
        expected.sort();
        assert_eq!(all, expected);
        assert_eq!(prepared.programs.len(), 11 + prepared.boundaries.len());
    }

    #[test]
    fn req_gc_10_req_nfr_07_actual_boundary_checks_reject_discontinuities() {
        let python = ["python3", "python"]
            .into_iter()
            .find(|name| {
                Command::new(name).arg("--version").output().is_ok_and(|o| {
                    o.status.success()
                        && String::from_utf8_lossy(&o.stdout).starts_with("Python 3.")
                })
            })
            .expect("Python 3 is required for LinuxCNC verification-tooling tests");
        let status = Command::new(python)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .args(["-m", "unittest", "discover", "-s", "tools/linuxcnc/tests"])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join(".."))
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn gate_3_shard_argument_is_checked() {
        assert_eq!(parse_shard("2/3").ok(), Some((2, 3)));
        assert_eq!(parse_shard("1/1").ok(), Some((1, 1)));
        for bad in ["0/3", "4/3", "2", "a/3", "2/0", "", "2/3/4"] {
            assert!(parse_shard(bad).is_err(), "'{bad}' was accepted");
        }
    }
}
