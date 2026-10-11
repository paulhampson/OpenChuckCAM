// SPDX-License-Identifier: AGPL-3.0-or-later
//! Dependency records (NFR-14, work breakdown slice 0 items 3 and 12).
//!
//! - Every third-party version is declared once, in `[workspace.dependencies]`
//!   of the root manifest, as an exact `=` pin.
//! - Member crates inherit it with `workspace = true`.
//! - Every pinned crate has a row in `docs/dependencies.md` with the same version.
//!
//! The fuzz workspace (`fuzz/Cargo.toml`) needs a nightly toolchain and cannot
//! join the main workspace, so its third-party dependencies are pinned and
//! recorded in the same way in its own manifest.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use toml::Value;

use crate::Findings;

/// The register of dependency records.
pub const REGISTER: &str = "docs/dependencies.md";

/// Dependency tables in a manifest.
const DEP_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// Reads `| `name` | version | ...` rows from the register.
pub fn parse_register(text: &str) -> BTreeMap<String, String> {
    let mut rows = BTreeMap::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if let [_, name, version, ..] = cells.as_slice()
            && let Some(name) = name.strip_prefix('`').and_then(|n| n.strip_suffix('`'))
        {
            rows.insert(name.to_owned(), (*version).to_owned());
        }
    }
    rows
}

/// The version requirement of a dependency entry, or `None` for a path dependency.
fn requirement(value: &Value) -> Option<String> {
    match value {
        Value::String(v) => Some(v.clone()),
        Value::Table(t) if t.contains_key("path") => None,
        Value::Table(t) => t
            .get("version")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| Some(String::new())),
        _ => Some(String::new()),
    }
}

/// Checks pinned third-party entries against the register.
pub fn check_pins(
    source: &str,
    deps: &toml::Table,
    register: &BTreeMap<String, String>,
) -> Findings {
    let mut findings = Findings::new();
    for (name, value) in deps {
        let Some(req) = requirement(value) else {
            continue;
        };
        let Some(version) = req.strip_prefix('=') else {
            findings.push(format!(
                "{source}: {name} = {req:?} must be an exact pin such as \"=1.2.3\""
            ));
            continue;
        };
        match register.get(name) {
            None => findings.push(format!(
                "{source}: {name} has no record in {REGISTER} (NFR-14)"
            )),
            Some(recorded) if recorded != version => findings.push(format!(
                "{source}: {name} is pinned to {version} but {REGISTER} records {recorded}"
            )),
            Some(_) => {}
        }
    }
    findings
}

/// Checks that a member manifest inherits every dependency from the workspace.
pub fn check_member(source: &str, manifest: &toml::Table) -> Findings {
    let mut findings = Findings::new();
    let mut tables: Vec<(&str, &toml::Table)> = DEP_TABLES
        .iter()
        .filter_map(|t| {
            manifest
                .get(*t)
                .and_then(Value::as_table)
                .map(|tab| (*t, tab))
        })
        .collect();
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for target in targets.values().filter_map(Value::as_table) {
            for t in DEP_TABLES {
                if let Some(tab) = target.get(*t).and_then(Value::as_table) {
                    tables.push((t, tab));
                }
            }
        }
    }
    for (table, deps) in tables {
        for (name, value) in deps {
            let inherited = value
                .as_table()
                .and_then(|t| t.get("workspace"))
                .and_then(Value::as_bool)
                == Some(true);
            if !inherited {
                findings.push(format!(
                    "{source} [{table}]: {name} must use `workspace = true`; versions are declared once in the root manifest"
                ));
            }
        }
    }
    findings
}

fn read_toml(path: &Path) -> Result<toml::Table> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
}

/// Runs every record check under `root`.
pub fn check(root: &Path) -> Result<Findings> {
    let register_text = std::fs::read_to_string(root.join(REGISTER))
        .with_context(|| format!("cannot read {REGISTER}"))?;
    let register = parse_register(&register_text);
    let mut findings = Findings::new();

    let workspace = read_toml(&root.join("Cargo.toml"))?;
    let deps = workspace
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(Value::as_table)
        .context("the root manifest has no [workspace.dependencies]")?;
    findings.extend(check_pins("Cargo.toml", deps, &register));

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("cannot run cargo metadata")?;
    for package in metadata.workspace_packages() {
        let path = package.manifest_path.as_std_path();
        let shown = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        findings.extend(check_member(&shown, &read_toml(path)?));
    }

    let fuzz = root.join("fuzz/Cargo.toml");
    if fuzz.is_file() {
        let manifest = read_toml(&fuzz)?;
        for table in DEP_TABLES {
            if let Some(deps) = manifest.get(*table).and_then(Value::as_table) {
                findings.extend(check_pins("fuzz/Cargo.toml", deps, &register));
            }
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> toml::Table {
        toml::from_str(text).unwrap()
    }

    const REGISTER_SAMPLE: &str =
        "| Crate | Version |\n| --- | --- |\n| `serde` | 1.0.229 | 93 |\n";

    #[test]
    fn req_nfr_14_recorded_pin_passes() {
        let register = parse_register(REGISTER_SAMPLE);
        let deps = table(
            "serde = { version = \"=1.0.229\", features = [\"derive\"] }\nopenchuck-types = { path = \"x\", version = \"0.1.0\" }",
        );
        assert!(check_pins("Cargo.toml", &deps, &register).is_empty());
    }

    #[test]
    fn req_nfr_14_unrecorded_dependency_fails() {
        let register = parse_register(REGISTER_SAMPLE);
        let deps = table("left-pad = \"=1.0.0\"");
        let findings = check_pins("Cargo.toml", &deps, &register);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("no record"));
    }

    #[test]
    fn req_nfr_14_loose_or_mismatched_pin_fails() {
        let register = parse_register(REGISTER_SAMPLE);
        assert_eq!(
            check_pins("x", &table("serde = \"1.0\""), &register).len(),
            1
        );
        assert_eq!(
            check_pins("x", &table("serde = \"=1.0.228\""), &register).len(),
            1
        );
    }

    #[test]
    fn member_must_inherit_versions() {
        let ok = table(
            "[dependencies]\nserde.workspace = true\n[dev-dependencies]\nproptest = { workspace = true }",
        );
        assert!(check_member("m", &ok).is_empty());
        let bad = table(
            "[dependencies]\nserde = \"1\"\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"",
        );
        assert_eq!(check_member("m", &bad).len(), 2);
    }
}
