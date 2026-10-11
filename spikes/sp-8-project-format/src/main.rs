// SPDX-License-Identifier: AGPL-3.0-or-later
#![allow(clippy::unit_arg)]
//! SP-8 spike runner. Usage: `sp8-project-format <work-dir>`. Builds the sample
//! project in each format and order scheme, runs the re-save, diff and real Git
//! merge checks, and prints a Markdown report.

mod model;
mod order;
mod store;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use model::{Operation, Project};
use store::{Format, Scheme, insert_op, load, move_op, save};

fn git(dir: &Path, args: &[&str]) -> Result<(bool, String)> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=sp8", "-c", "user.email=sp8@example.invalid"])
        .args(["-c", "commit.gpgsign=false", "-c", "init.defaultBranch=main"])
        .args(args)
        .output()
        .context("running git")?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((out.status.success(), text))
}

fn git_ok(dir: &Path, args: &[&str]) -> Result<String> {
    let (ok, text) = git(dir, args)?;
    if !ok {
        bail!("git {args:?} failed: {text}");
    }
    Ok(text)
}

fn fresh(dir: &Path) -> Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    Ok(())
}

fn commit_all(dir: &Path, msg: &str) -> Result<()> {
    git_ok(dir, &["add", "-A"])?;
    git_ok(dir, &["commit", "-q", "--allow-empty", "-m", msg])?;
    Ok(())
}

/// (files changed, lines added, lines removed) in the working tree against HEAD.
fn numstat(dir: &Path) -> Result<(usize, usize, usize)> {
    git_ok(dir, &["add", "-A"])?;
    let text = git_ok(dir, &["diff", "--cached", "--numstat"])?;
    let mut files = 0;
    let mut add = 0;
    let mut del = 0;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        add += parts.next().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        del += parts.next().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        files += 1;
    }
    git_ok(dir, &["reset", "-q"])?;
    Ok((files, add, del))
}

fn label(fmt: Format, scheme: Scheme) -> String {
    format!(
        "{} / {}",
        match fmt {
            Format::Json => "JSON",
            Format::Toml => "TOML",
        },
        match scheme {
            Scheme::Keys => "per-item keys",
            Scheme::List => "ordered list",
        }
    )
}

fn op_mut<'a>(p: &'a mut Project, id: &str) -> Result<&'a mut Operation> {
    match p.operations.iter_mut().find(|o| o.id == id) {
        Some(o) => Ok(o),
        None => bail!("no operation {id}"),
    }
}

fn new_op(p: &Project, id: &str, name: &str) -> Result<Operation> {
    let Some(base) = p.operations.first() else { bail!("empty project") };
    let mut o = base.clone();
    o.id = id.to_string();
    o.name = name.to_string();
    Ok(o)
}

type Edit = fn(&mut Project, Scheme) -> Result<()>;
type Check = fn(&Project) -> Result<()>;

struct Scenario {
    name: &'static str,
    /// Counts towards the SP-8 pass criterion (different items on each side).
    criterion: bool,
    a: Edit,
    b: Edit,
    check: Check,
}

fn expect_order(p: &Project, want: &[&str]) -> Result<()> {
    let got = p.op_order();
    if got != want {
        bail!("order {got:?}, expected {want:?}");
    }
    Ok(())
}

fn op(p: &Project, id: &str) -> Result<Operation> {
    match p.operations.iter().find(|o| o.id == id) {
        Some(o) => Ok(o.clone()),
        None => bail!("no operation {id}"),
    }
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "S1 reorder op-c3 to the top / edit op-b2 stepover",
            criterion: true,
            a: |p, s| move_op(p, "op-c3", 0, s).map(|_| ()),
            b: |p, _| Ok(op_mut(p, "op-b2")?.params.stepover = 0.25),
            check: |p| {
                expect_order(p, &["op-c3", "op-a1", "op-b2", "op-d4"])?;
                if op(p, "op-b2")?.params.stepover != 0.25 {
                    bail!("stepover lost");
                }
                Ok(())
            },
        },
        Scenario {
            name: "S2 reorder op-b2 down one / edit op-b2 stepover (same item)",
            criterion: true,
            a: |p, s| move_op(p, "op-b2", 2, s).map(|_| ()),
            b: |p, _| Ok(op_mut(p, "op-b2")?.params.stepover = 0.25),
            check: |p| {
                expect_order(p, &["op-a1", "op-c3", "op-b2", "op-d4"])?;
                if op(p, "op-b2")?.params.stepover != 0.25 {
                    bail!("stepover lost");
                }
                Ok(())
            },
        },
        Scenario {
            name: "S3 move op-d4 to the top / move op-a1 to the end",
            criterion: true,
            a: |p, s| move_op(p, "op-d4", 0, s).map(|_| ()),
            b: |p, s| move_op(p, "op-a1", 3, s).map(|_| ()),
            check: |p| expect_order(p, &["op-d4", "op-b2", "op-c3", "op-a1"]),
        },
        Scenario {
            name: "S4 add op-e5 after op-a1 / add op-f6 at the end",
            criterion: true,
            a: |p, s| {
                let o = new_op(p, "op-e5", "Drill")?;
                insert_op(p, o, 1, s).map(|_| ())
            },
            b: |p, s| {
                let o = new_op(p, "op-f6", "Deburr")?;
                insert_op(p, o, 4, s).map(|_| ())
            },
            check: |p| expect_order(p, &["op-a1", "op-e5", "op-b2", "op-c3", "op-d4", "op-f6"]),
        },
        Scenario {
            name: "S5 add op-e5 after op-b2 / add op-f6 after op-b2 (same spot)",
            criterion: true,
            a: |p, s| {
                let o = new_op(p, "op-e5", "Drill")?;
                insert_op(p, o, 2, s).map(|_| ())
            },
            b: |p, s| {
                let o = new_op(p, "op-f6", "Deburr")?;
                insert_op(p, o, 2, s).map(|_| ())
            },
            // Ties break by ID, so the result is deterministic.
            check: |p| expect_order(p, &["op-a1", "op-b2", "op-e5", "op-f6", "op-c3", "op-d4"]),
        },
        Scenario {
            name: "S6 delete op-c3 / edit op-d4 feed",
            criterion: true,
            a: |p, _| {
                p.operations.retain(|o| o.id != "op-c3");
                Ok(())
            },
            b: |p, _| Ok(op_mut(p, "op-d4")?.params.feed = 650.0),
            check: |p| {
                expect_order(p, &["op-a1", "op-b2", "op-d4"])?;
                if op(p, "op-d4")?.params.feed != 650.0 {
                    bail!("feed lost");
                }
                Ok(())
            },
        },
        Scenario {
            name: "S7 edit op-b2 stepover / edit op-b2 feed (same item, two lines apart)",
            criterion: false,
            a: |p, _| Ok(op_mut(p, "op-b2")?.params.stepover = 0.25),
            b: |p, _| Ok(op_mut(p, "op-b2")?.params.feed = 650.0),
            check: |p| {
                let o = op(p, "op-b2")?;
                if o.params.stepover != 0.25 || o.params.feed != 650.0 {
                    bail!("an edit was lost");
                }
                Ok(())
            },
        },
        Scenario {
            name: "S8 edit op-b2 tolerance / edit op-b2 stepover (same item, adjacent lines)",
            criterion: false,
            a: |p, _| Ok(op_mut(p, "op-b2")?.params.tolerance = 0.005),
            b: |p, _| Ok(op_mut(p, "op-b2")?.params.stepover = 0.25),
            check: |_| Ok(()),
        },
        Scenario {
            name: "S9 add face 34 / add face 55 to the same selection set",
            criterion: false,
            a: |p, _| {
                if let Some(s) = p.selections.first_mut() {
                    s.faces.push(34);
                }
                Ok(())
            },
            b: |p, _| {
                if let Some(s) = p.selections.first_mut() {
                    s.faces.push(55);
                }
                Ok(())
            },
            check: |_| Ok(()),
        },
        Scenario {
            name: "S10 move op-c3 to the top / move op-c3 to the end (true conflict)",
            criterion: false,
            a: |p, s| move_op(p, "op-c3", 0, s).map(|_| ()),
            b: |p, s| move_op(p, "op-c3", 3, s).map(|_| ()),
            check: |_| Ok(()),
        },
    ]
}

/// Runs one scenario with a real Git merge. Returns "clean", "clean, check
/// failed: ..." or "conflict in ...".
fn run_merge(work: &Path, fmt: Format, scheme: Scheme, sc: &Scenario) -> Result<String> {
    let dir = work.join("merge");
    fresh(&dir)?;
    git_ok(&dir, &["init", "-q"])?;
    save(&model::sample(), &dir, fmt, scheme)?;
    commit_all(&dir, "base")?;
    git_ok(&dir, &["branch", "b"])?;

    let mut p = load(&dir, fmt, scheme)?;
    (sc.a)(&mut p, scheme)?;
    save(&p, &dir, fmt, scheme)?;
    commit_all(&dir, "side a")?;

    git_ok(&dir, &["checkout", "-q", "b"])?;
    let mut p = load(&dir, fmt, scheme)?;
    (sc.b)(&mut p, scheme)?;
    save(&p, &dir, fmt, scheme)?;
    commit_all(&dir, "side b")?;

    git_ok(&dir, &["checkout", "-q", "main"])?;
    let (ok, _) = git(&dir, &["merge", "-q", "--no-edit", "b"])?;
    if !ok {
        let files = git_ok(&dir, &["diff", "--name-only", "--diff-filter=U"])?;
        return Ok(format!("conflict in `{}`", files.trim().replace('\n', "`, `")));
    }
    let merged = match load(&dir, fmt, scheme) {
        Ok(m) => m,
        Err(e) => return Ok(format!("clean in Git, but the merged project is invalid: {e}")),
    };
    match (sc.check)(&merged) {
        Ok(()) => {}
        Err(e) => return Ok(format!("clean, check failed: {e}")),
    }
    // A merged project must also re-save without changes.
    save(&merged, &dir, fmt, scheme)?;
    let (files, _, _) = numstat(&dir)?;
    if files != 0 {
        return Ok(format!("clean, but re-save changed {files} files"));
    }
    Ok("clean".to_string())
}

struct DiffRow {
    resave_identical: bool,
    load_save_noop: bool,
    param_edit: (usize, usize, usize),
    reorder: (usize, usize, usize),
    add: (usize, usize, usize),
    bytes: usize,
}

fn run_diffs(work: &Path, fmt: Format, scheme: Scheme) -> Result<DiffRow> {
    let one = work.join("save-1");
    let two = work.join("save-2");
    fresh(&one)?;
    fresh(&two)?;
    let p = model::sample();
    save(&p, &one, fmt, scheme)?;
    save(&load(&one, fmt, scheme)?, &two, fmt, scheme)?;
    let (same, _) = git(work, &["diff", "--no-index", "--quiet", "save-1", "save-2"])?;

    let dir = work.join("diff");
    fresh(&dir)?;
    git_ok(&dir, &["init", "-q"])?;
    save(&p, &dir, fmt, scheme)?;
    commit_all(&dir, "base")?;
    let mut bytes = 0;
    for f in git_ok(&dir, &["ls-files"])?.lines() {
        bytes += fs::metadata(dir.join(f))?.len() as usize;
    }

    save(&load(&dir, fmt, scheme)?, &dir, fmt, scheme)?;
    let noop = numstat(&dir)? == (0, 0, 0);

    let mut q = load(&dir, fmt, scheme)?;
    op_mut(&mut q, "op-b2")?.params.stepover = 0.25;
    save(&q, &dir, fmt, scheme)?;
    let param_edit = numstat(&dir)?;
    git_ok(&dir, &["checkout", "-q", "--", "."])?;

    let mut q = load(&dir, fmt, scheme)?;
    move_op(&mut q, "op-c3", 0, scheme)?;
    save(&q, &dir, fmt, scheme)?;
    let reorder = numstat(&dir)?;
    git_ok(&dir, &["checkout", "-q", "--", "."])?;

    let mut q = load(&dir, fmt, scheme)?;
    let o = new_op(&q, "op-e5", "Drill")?;
    insert_op(&mut q, o, 1, scheme)?;
    save(&q, &dir, fmt, scheme)?;
    let add = numstat(&dir)?;

    Ok(DiffRow { resave_identical: same, load_save_noop: noop, param_edit, reorder, add, bytes })
}

/// Exact f64 round trip through each format, on awkward values.
fn float_roundtrip(fmt: Format) -> Result<(usize, usize)> {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct V {
        v: f64,
    }
    let mut seed: u64 = 0x5eed;
    let mut fails = 0;
    let mut values = vec![0.1, 0.2 + 0.1, 1e-300, 5e-324, 1.7976931348623157e308, -0.0, 3.0, 1e21, 123456.789];
    for _ in 0..20000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let v = f64::from_bits(seed);
        if v.is_finite() {
            values.push(v);
        }
    }
    for &v in &values {
        let text = fmt.to_text(&V { v })?;
        let back: V = fmt.from_text(&text)?;
        if back.v.to_bits() != v.to_bits() {
            fails += 1;
        }
    }
    Ok((values.len(), fails))
}

/// toml_edit keeps a hand-edited file's comments and layout when the app
/// changes one value; the serde writer does not.
fn hand_edit() -> Result<(String, String)> {
    let original = "# Finishing pass, tuned by hand on 3 Oct.\nschema = 1\nname = \"Finish\"\n\n[params]\nstepover = 0.3   # keep below 0.35 on brass\nfeed = 800.0\n";
    let mut doc: toml_edit::DocumentMut = original.parse()?;
    // Replacing the item drops the value's own trailing comment; replacing
    // the value and keeping its decor (spacing and comment) does not.
    doc["params"]["stepover"] = toml_edit::value(0.25);
    let naive = doc.to_string();
    let mut doc: toml_edit::DocumentMut = original.parse()?;
    if let Some(v) = doc["params"]["stepover"].as_value_mut() {
        let decor = v.decor().clone();
        *v = toml_edit::Value::from(0.25);
        *v.decor_mut() = decor;
    }
    let edited = doc.to_string();
    let count = |e: &str| original.lines().zip(e.lines()).filter(|(a, b)| a != b).count();
    let kept = |e: &str| e.contains("# Finishing pass") && e.contains("# keep below");
    let diff_lines = count(&edited);
    let kept = format!(
        "{} (item replaced: {} line changed, comments kept: {})",
        kept(&edited),
        count(&naive),
        kept(&naive)
    );

    let v: toml::Value = toml::from_str(original)?;
    let serde_out = toml::to_string_pretty(&v)?;
    Ok((
        format!("toml_edit: {diff_lines} line changed, comments kept: {kept}"),
        format!("toml serde re-save: comments kept: {}", serde_out.contains('#')),
    ))
}

fn key_growth() -> Result<Vec<(String, usize)>> {
    let mut rows = Vec::new();
    for (name, pattern) in [("append", 0), ("prepend", 1), ("insert after first", 2)] {
        for n in [100usize, 1000] {
            let mut keys = vec![order::key_between(None, None)?];
            for _ in 0..n {
                match pattern {
                    0 => keys.push(order::key_between(keys.last().map(String::as_str), None)?),
                    1 => {
                        let k = order::key_between(None, keys.first().map(String::as_str))?;
                        keys.insert(0, k);
                    }
                    _ => {
                        let hi = keys.get(1).cloned();
                        let k = order::key_between(keys.first().map(String::as_str), hi.as_deref())?;
                        keys.insert(1, k);
                    }
                }
            }
            let max = keys.iter().map(String::len).max().unwrap_or(0);
            rows.push((format!("{name}, {n} times"), max));
        }
    }
    Ok(rows)
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "**no**" }
}

fn main() -> Result<()> {
    let work: PathBuf = std::env::args().nth(1).map(PathBuf::from).context("usage: sp8 <work-dir>")?;
    fs::create_dir_all(&work)?;
    let combos = [
        (Format::Json, Scheme::Keys),
        (Format::Toml, Scheme::Keys),
        (Format::Json, Scheme::List),
        (Format::Toml, Scheme::List),
    ];

    println!("## Re-save and diff\n");
    println!("Diff counts are (files, lines added, lines removed) from `git diff --numstat`.\n");
    println!("| Candidate | Re-save byte-identical | Load then save no-op | One-parameter edit | Reorder one op | Add one op | Project bytes |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for &(fmt, scheme) in &combos {
        let r = run_diffs(&work, fmt, scheme)?;
        println!(
            "| {} | {} | {} | {:?} | {:?} | {:?} | {} |",
            label(fmt, scheme),
            yes(r.resave_identical),
            yes(r.load_save_noop),
            r.param_edit,
            r.reorder,
            r.add,
            r.bytes
        );
    }

    println!("\n## Real Git merges\n");
    print!("| Scenario (side A / side B) | Pass criterion |");
    for &(fmt, scheme) in &combos {
        print!(" {} |", label(fmt, scheme));
    }
    println!("\n| --- | --- |{}", " --- |".repeat(combos.len()));
    for sc in scenarios() {
        print!("| {} | {} |", sc.name, if sc.criterion { "yes" } else { "informative" });
        for &(fmt, scheme) in &combos {
            print!(" {} |", run_merge(&work, fmt, scheme, &sc)?);
        }
        println!();
    }

    println!("\n## Float round trip\n");
    for fmt in [Format::Json, Format::Toml] {
        let (n, fails) = float_roundtrip(fmt)?;
        println!("- {}: {n} values, {fails} not bit-exact", fmt.ext());
        let nan = fmt.to_text(&std::collections::BTreeMap::from([("v", f64::NAN)]));
        println!("  - NaN is written as: {:?}", nan.map(|t| t.trim().replace('\n', " ")));
    }

    println!("\n## Hand-edited TOML\n");
    let (a, b) = hand_edit()?;
    println!("- {a}\n- {b}");

    println!("\n## Order key length (longest key)\n");
    for (name, len) in key_growth()? {
        println!("- {name}: {len} characters");
    }

    println!("\n## Sample files (per-item keys)\n");
    for fmt in [Format::Json, Format::Toml] {
        let dir = work.join(format!("sample-{}", fmt.ext()));
        fresh(&dir)?;
        save(&model::sample(), &dir, fmt, Scheme::Keys)?;
        let text = fs::read_to_string(dir.join("operations").join(format!("op-b2.{}", fmt.ext())))?;
        println!("`operations/op-b2.{}`:\n\n```{}\n{}```\n", fmt.ext(), fmt.ext(), text);
    }
    Ok(())
}
