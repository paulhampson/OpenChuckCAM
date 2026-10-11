// SPDX-License-Identifier: AGPL-3.0-or-later
//! Dependency rules (architecture sections 3, 4 and 12; NFR-01, NFR-09, INT-01).
//!
//! The rules run over a plain [`Graph`] built from `cargo metadata`, so the
//! unit tests can break each rule deliberately on a synthetic graph.
//! A deliberate exception goes in `xtask/allow-list.toml` with its decision
//! record and NFR-14 justification; anything else fails.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::Findings;

/// The reusable library family (architecture section 12).
pub const LIBRARY_FAMILY: &[&str] = &[
    "openchuck-types",
    "openchuck-path",
    "openchuck-post",
    "openchuck-gcode",
];

/// Project crates each crate may depend on (normal and build dependencies),
/// from the "Project dependencies" column of architecture section 3.
/// `openchuck-testkit` and `openchuck-benches` are test-only additions
/// (decision record 0003).
pub const ALLOWED_INTERNAL: &[(&str, &[&str])] = &[
    ("openchuck-types", &[]),
    ("openchuck-path", &["openchuck-types"]),
    ("openchuck-post", &["openchuck-types"]),
    ("openchuck-gcode", &["openchuck-types"]),
    ("openchuck-geom", &["openchuck-types"]),
    ("openchuck-import", &["openchuck-types", "openchuck-geom"]),
    ("openchuck-sim", &["openchuck-types", "openchuck-path"]),
    ("openchuck-project", &["openchuck-types"]),
    ("openchuck-vcs", &["openchuck-project"]),
    (
        "openchuck-app",
        &[
            "openchuck-types",
            "openchuck-path",
            "openchuck-post",
            "openchuck-gcode",
            "openchuck-geom",
            "openchuck-import",
            "openchuck-sim",
            "openchuck-project",
            "openchuck-vcs",
        ],
    ),
    ("openchuck-viewport", &["openchuck-types"]),
    ("openchuck-cli", &["openchuck-app"]),
    ("openchuck-ui", &["openchuck-app", "openchuck-viewport"]),
    ("openchuck-testkit", &["openchuck-types"]),
    (
        "openchuck-benches",
        &[
            "openchuck-types",
            "openchuck-path",
            "openchuck-post",
            "openchuck-gcode",
            "openchuck-geom",
            "openchuck-import",
            "openchuck-sim",
        ],
    ),
    ("xtask", &[]),
];

/// The only crate that may be a dev-dependency without appearing above.
pub const TEST_ONLY: &[&str] = &["openchuck-testkit"];

/// Third-party crates only some project crates may reach through their own
/// third-party dependency tree (architecture section 4).
pub const RESTRICTED: &[(&str, &[&str], &[&str])] = &[
    (
        "UI or GPU",
        &[
            "egui",
            "eframe",
            "egui-wgpu",
            "egui_glow",
            "egui-winit",
            "wgpu",
            "wgpu-core",
            "wgpu-hal",
            "winit",
            "glow",
        ],
        &["openchuck-ui", "openchuck-viewport"],
    ),
    (
        "STEP kernel",
        &[
            "opencascade",
            "opencascade-sys",
            "occt-sys",
            "cadrum",
            "truck-stepio",
            "brepkit-io",
        ],
        &["openchuck-import"],
    ),
    ("Git", &["git2", "libgit2-sys", "gix"], &["openchuck-vcs"]),
];

/// Network clients and telemetry: banned everywhere (NFR-09). Names ending
/// in `*` match by prefix.
pub const BANNED: &[&str] = &[
    "reqwest",
    "hyper",
    "ureq",
    "curl",
    "curl-sys",
    "isahc",
    "surf",
    "attohttpc",
    "h2",
    "h3",
    "quinn",
    "tungstenite",
    "websocket",
    "native-tls",
    "openssl",
    "openssl-sys",
    "rustls",
    "sentry*",
    "opentelemetry*",
    "posthog*",
    "mixpanel*",
    "segment",
    "datadog*",
    "newrelic*",
];

/// `git2` features that would add network access (NFR-09, architecture section 4).
pub const GIT2_NETWORK_FEATURES: &[&str] = &["https", "ssh", "ssh_key_from_memory"];

/// How a dependency is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// Normal or build dependency: part of what ships.
    Normal,
    /// Dev-dependency: tests, examples and benchmarks only.
    Dev,
}

/// A dependency graph by package name.
#[derive(Debug, Default, Clone)]
pub struct Graph {
    /// Workspace member names.
    pub members: BTreeSet<String>,
    /// Package name to (dependency name, kind).
    pub edges: BTreeMap<String, BTreeSet<(String, Kind)>>,
    /// Package name to enabled features.
    pub features: BTreeMap<String, BTreeSet<String>>,
}

impl Graph {
    /// Adds an edge.
    pub fn add(&mut self, from: &str, to: &str, kind: Kind) {
        self.edges
            .entry(from.to_owned())
            .or_default()
            .insert((to.to_owned(), kind));
    }

    fn deps(&self, name: &str, kind: Kind) -> impl Iterator<Item = &str> {
        self.edges
            .get(name)
            .into_iter()
            .flatten()
            .filter(move |(_, k)| *k == kind)
            .map(|(n, _)| n.as_str())
    }

    /// Third-party crates reachable from `member` through normal dependencies
    /// without passing through another workspace member.
    fn own_third_party(&self, member: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<&str> = self
            .deps(member, Kind::Normal)
            .filter(|d| !self.members.contains(*d))
            .collect();
        while let Some(name) = stack.pop() {
            if seen.insert(name.to_owned()) {
                stack.extend(
                    self.deps(name, Kind::Normal)
                        .filter(|d| !self.members.contains(*d)),
                );
            }
        }
        seen
    }

    /// Every package reachable from any member through any dependency kind.
    fn everything(&self) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<&str> = self.members.iter().map(String::as_str).collect();
        while let Some(name) = stack.pop() {
            if seen.insert(name.to_owned()) {
                stack.extend(self.deps(name, Kind::Normal));
                stack.extend(self.deps(name, Kind::Dev));
            }
        }
        seen
    }
}

/// A recorded exception to a rule.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exception {
    /// The crate allowed to break the rule.
    pub krate: String,
    /// The dependency it may take.
    pub dependency: String,
    /// The decision record that justifies it, for example `docs/decisions/0007-...md`.
    pub record: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowList {
    #[serde(default)]
    exception: Vec<Exception>,
}

fn banned(name: &str) -> bool {
    BANNED.iter().any(|b| match b.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == *b,
    })
}

/// Applies every rule to `graph`, skipping recorded exceptions.
pub fn check_graph(graph: &Graph, exceptions: &[Exception]) -> Findings {
    let excepted = |krate: &str, dep: &str| {
        exceptions
            .iter()
            .any(|e| e.krate == krate && e.dependency == dep)
    };
    let allowed: BTreeMap<&str, &[&str]> = ALLOWED_INTERNAL.iter().copied().collect();
    let mut findings = Findings::new();

    for member in &graph.members {
        // Internal layering, including the library-family rule.
        match allowed.get(member.as_str()) {
            None => findings.push(format!(
                "{member} is not in the architecture's crate table (architecture section 3); add it there and to xtask before using it"
            )),
            Some(allowed) => {
                for dep in graph.deps(member, Kind::Normal) {
                    if graph.members.contains(dep) && !allowed.contains(&dep) && !excepted(member, dep) {
                        let why = if LIBRARY_FAMILY.contains(&member.as_str()) {
                            "the library family depends only on its own members (architecture section 12)"
                        } else {
                            "this dependency is not in architecture section 3"
                        };
                        findings.push(format!("{member} must not depend on {dep}: {why}"));
                    }
                }
                for dep in graph.deps(member, Kind::Dev) {
                    if graph.members.contains(dep)
                        && !allowed.contains(&dep)
                        && !TEST_ONLY.contains(&dep)
                        && dep != member
                        && !excepted(member, dep)
                    {
                        findings.push(format!(
                            "{member} must not take {dep} as a dev-dependency: it is not an allowed dependency"
                        ));
                    }
                }
            }
        }
        // Test-only crates never ship.
        for dep in graph.deps(member, Kind::Normal) {
            if TEST_ONLY.contains(&dep) && !excepted(member, dep) {
                findings.push(format!(
                    "{member} must not depend on {dep} except as a dev-dependency (it is test-only)"
                ));
            }
        }
        // Restricted third-party crates.
        let reach = graph.own_third_party(member);
        for (what, crates, owners) in RESTRICTED {
            if owners.contains(&member.as_str()) {
                continue;
            }
            for name in crates.iter().filter(|c| reach.contains(**c)) {
                if !excepted(member, name) {
                    findings.push(format!(
                        "{member} must not depend on {name}: only {} may use {what} crates (architecture section 4)",
                        owners.join(" and ")
                    ));
                }
            }
        }
    }

    // Network and telemetry crates anywhere in the tree (NFR-09).
    for name in graph.everything() {
        if banned(&name) && !excepted("*", &name) {
            findings.push(format!(
                "{name} is a network or telemetry crate and is banned everywhere (NFR-09)"
            ));
        }
    }
    if let Some(features) = graph.features.get("git2") {
        for feature in GIT2_NETWORK_FEATURES {
            if features.contains(*feature) {
                findings.push(format!(
                    "git2 must be built without the `{feature}` feature (NFR-09)"
                ));
            }
        }
    }
    findings
}

/// Builds the graph from `cargo metadata` and applies the rules.
pub fn check(root: &Path) -> Result<Findings> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .exec()
        .context("cannot run cargo metadata")?;
    let names: BTreeMap<_, _> = metadata
        .packages
        .iter()
        .map(|p| (p.id.clone(), p.name.to_string()))
        .collect();
    let mut graph = Graph::default();
    for id in &metadata.workspace_members {
        if let Some(name) = names.get(id) {
            graph.members.insert(name.clone());
        }
    }
    let resolve = metadata
        .resolve
        .context("cargo metadata gave no resolve graph")?;
    for node in &resolve.nodes {
        let Some(from) = names.get(&node.id) else {
            continue;
        };
        graph
            .features
            .entry(from.clone())
            .or_default()
            .extend(node.features.iter().map(ToString::to_string));
        for dep in &node.deps {
            let Some(to) = names.get(&dep.pkg) else {
                continue;
            };
            for info in &dep.dep_kinds {
                let kind = match info.kind {
                    cargo_metadata::DependencyKind::Development => Kind::Dev,
                    _ => Kind::Normal,
                };
                graph.add(from, to, kind);
            }
        }
    }
    let allow_path = root.join("xtask/allow-list.toml");
    let allow: AllowList = toml::from_str(
        &std::fs::read_to_string(&allow_path)
            .with_context(|| format!("cannot read {}", allow_path.display()))?,
    )
    .with_context(|| format!("cannot parse {}", allow_path.display()))?;
    let mut findings = check_graph(&graph, &allow.exception);
    for e in &allow.exception {
        if !root.join(&e.record).is_file() {
            findings.push(format!(
                "allow-list entry {} -> {} names a missing record {}",
                e.krate, e.dependency, e.record
            ));
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A graph shaped like the real workspace, which passes every rule.
    fn healthy() -> Graph {
        let mut g = Graph::default();
        for (name, deps) in ALLOWED_INTERNAL {
            g.members.insert((*name).to_owned());
            for d in *deps {
                if *d != "openchuck-vcs" {
                    g.add(name, d, Kind::Normal);
                }
            }
        }
        g.members.remove("openchuck-vcs");
        g.add("openchuck-types", "serde", Kind::Normal);
        g.add("openchuck-types", "openchuck-testkit", Kind::Dev);
        g.add("openchuck-viewport", "wgpu", Kind::Normal);
        g.add("wgpu", "wgpu-core", Kind::Normal);
        g.add("openchuck-ui", "eframe", Kind::Normal);
        g.add("eframe", "egui", Kind::Normal);
        g
    }

    #[test]
    fn healthy_graph_passes() {
        assert_eq!(check_graph(&healthy(), &[]), Findings::new());
    }

    #[test]
    fn req_nfr_01_library_family_rule_fails_when_broken() {
        let mut g = healthy();
        g.add("openchuck-path", "openchuck-geom", Kind::Normal);
        let findings = check_graph(&g, &[]);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("library family"), "{findings:?}");
    }

    #[test]
    fn req_nfr_01_family_must_not_depend_on_application() {
        let mut g = healthy();
        g.add("openchuck-post", "openchuck-app", Kind::Normal);
        assert_eq!(check_graph(&g, &[]).len(), 1);
    }

    #[test]
    fn layering_rule_fails_when_broken() {
        let mut g = healthy();
        g.add("openchuck-cli", "openchuck-sim", Kind::Normal);
        assert_eq!(check_graph(&g, &[]).len(), 1);
    }

    #[test]
    fn unknown_crate_fails() {
        let mut g = healthy();
        g.members.insert("openchuck-surprise".to_owned());
        assert_eq!(check_graph(&g, &[]).len(), 1);
    }

    #[test]
    fn req_nfr_01_gpu_crate_outside_ui_fails() {
        let mut g = healthy();
        g.add("openchuck-path", "some-renderer", Kind::Normal);
        g.add("some-renderer", "wgpu", Kind::Normal);
        let findings = check_graph(&g, &[]);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("openchuck-path must not depend on wgpu")),
            "{findings:?}"
        );
    }

    #[test]
    fn req_nfr_01_egui_in_app_fails() {
        let mut g = healthy();
        g.add("openchuck-app", "egui", Kind::Normal);
        assert_eq!(check_graph(&g, &[]).len(), 1);
    }

    #[test]
    fn step_kernel_outside_import_fails() {
        let mut g = healthy();
        g.add("openchuck-geom", "opencascade", Kind::Normal);
        assert_eq!(check_graph(&g, &[]).len(), 1);
        let mut g = healthy();
        g.add("openchuck-import", "opencascade", Kind::Normal);
        assert_eq!(check_graph(&g, &[]), Findings::new());
    }

    #[test]
    fn git_outside_vcs_fails() {
        let mut g = healthy();
        g.add("openchuck-project", "git2", Kind::Normal);
        assert_eq!(check_graph(&g, &[]).len(), 1);
    }

    #[test]
    fn req_nfr_09_network_crate_anywhere_fails() {
        let mut g = healthy();
        g.add("openchuck-cli", "something", Kind::Normal);
        g.add("something", "reqwest", Kind::Normal);
        let findings = check_graph(&g, &[]);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("NFR-09"));
        let mut g = healthy();
        g.add("openchuck-app", "sentry-core", Kind::Dev);
        assert_eq!(check_graph(&g, &[]).len(), 1);
    }

    #[test]
    fn req_nfr_09_git2_network_feature_fails() {
        let mut g = healthy();
        g.features
            .entry("git2".to_owned())
            .or_default()
            .insert("https".to_owned());
        assert_eq!(check_graph(&g, &[]).len(), 1);
    }

    #[test]
    fn testkit_as_normal_dependency_fails() {
        let mut g = healthy();
        g.add("openchuck-sim", "openchuck-testkit", Kind::Normal);
        // Both the layering rule and the test-only rule report it.
        assert_eq!(check_graph(&g, &[]).len(), 2);
    }

    #[test]
    fn recorded_exception_passes() {
        let mut g = healthy();
        g.add("openchuck-path", "clipper2", Kind::Normal);
        g.add("clipper2", "opencascade", Kind::Normal);
        let exception = Exception {
            krate: "openchuck-path".to_owned(),
            dependency: "opencascade".to_owned(),
            record: "docs/decisions/0099-example.md".to_owned(),
        };
        assert_eq!(check_graph(&g, &[exception]), Findings::new());
    }
}
