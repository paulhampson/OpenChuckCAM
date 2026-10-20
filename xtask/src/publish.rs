// SPDX-License-Identifier: AGPL-3.0-or-later
//! Publish readiness of the library family (INT-01, INT-07, INT-09, NFR-10; architecture
//! section 12, "Names and tags"; decision 0138, OI-302).
//!
//! Every library-family crate must be one that `cargo publish` accepts and that builds from its
//! package alone: the metadata crates.io shows (description, licence, repository, readme, keywords
//! and categories, the Rust version), an `include` list, every file the sources embed inside the
//! crate and covered by that list, every normal or build dependency carrying a version, and no
//! dependency that cargo would keep in the published manifest on a crate that is never published.
//! `cargo publish --dry-run` is the final word; this check runs on every pull request, without a
//! build, and names the rule that broke.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use crate::Findings;
use crate::deps::LIBRARY_FAMILY;
use crate::licences::PROJECT_LICENCE;

/// The most keywords and categories crates.io accepts on one crate.
const MAX_TAGS: usize = 5;
/// The longest keyword crates.io accepts.
const MAX_KEYWORD_CHARS: usize = 20;

/// The kind of a dependency, as cargo reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    /// A `[dependencies]` entry.
    Normal,
    /// A `[build-dependencies]` entry.
    Build,
    /// A `[dev-dependencies]` entry.
    Dev,
}

/// One dependency entry of a crate.
#[derive(Debug, Clone)]
pub struct Dep {
    /// The dependency's package name.
    pub name: String,
    /// Which table it is in.
    pub kind: DepKind,
    /// True when the entry gives a version requirement (cargo keeps the entry when it publishes).
    pub has_version: bool,
    /// True for a path dependency.
    pub path: bool,
    /// For a workspace member, whether it may be published; `None` for a third-party crate.
    pub member_publishable: Option<bool>,
}

/// A file the crate's sources embed with `include_str!` or `include_bytes!`.
#[derive(Debug, Clone)]
pub struct Embedded {
    /// The source file and the literal path, for the finding.
    pub site: String,
    /// The embedded file relative to the crate directory, or `None` when it is outside the crate.
    pub relative: Option<String>,
}

/// What the check reads of one crate's manifest and sources.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    /// The package name.
    pub name: String,
    /// False when the manifest says `publish = false`.
    pub publish: bool,
    pub description: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    /// The readme, if one is named, and whether that file is inside the crate.
    pub readme: Option<(String, bool)>,
    pub rust_version: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    /// The manifest's `include` list.
    pub include: Vec<String>,
    pub dependencies: Vec<Dep>,
    pub embedded: Vec<Embedded>,
}

/// True when `relative` (a path inside the crate, with `/` separators) is matched by an
/// `include` entry. The library family uses only two forms: `/dir/**` and `/file`.
fn included(include: &[String], relative: &str) -> bool {
    include.iter().any(|pattern| {
        let pattern = pattern.trim_start_matches('/');
        match pattern.strip_suffix("/**") {
            Some(dir) => relative.starts_with(&format!("{dir}/")),
            None => relative == pattern,
        }
    })
}

/// True for a keyword crates.io accepts: ASCII letters, digits, `_`, `-` and `+`, starting with
/// a letter or digit, at most 20 characters.
fn valid_keyword(keyword: &str) -> bool {
    keyword.len() <= MAX_KEYWORD_CHARS
        && keyword
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && keyword
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
}

/// The publish-readiness findings for one library-family crate.
pub fn check_manifest(m: &Manifest) -> Findings {
    let mut findings = Findings::new();
    let name = &m.name;
    let mut need = |what: &str, ok: bool| {
        if !ok {
            findings.push(format!("{name}: {what} (decision 0138)"));
        }
    };
    need(
        "is a library-family crate and must not say `publish = false`",
        m.publish,
    );
    need(
        "needs a description, which crates.io requires",
        m.description
            .as_deref()
            .is_some_and(|d| !d.trim().is_empty()),
    );
    need(
        &format!("needs license = \"{PROJECT_LICENCE}\" (NFR-10)"),
        m.license.as_deref() == Some(PROJECT_LICENCE),
    );
    need("needs a repository", m.repository.is_some());
    need(
        "needs a readme inside the crate, which crates.io shows",
        m.readme.as_ref().is_some_and(|(_, inside)| *inside),
    );
    need("needs a rust-version", m.rust_version.is_some());
    need(
        &format!("needs 1 to {MAX_TAGS} keywords"),
        (1..=MAX_TAGS).contains(&m.keywords.len()),
    );
    need(
        &format!("needs 1 to {MAX_TAGS} categories"),
        (1..=MAX_TAGS).contains(&m.categories.len()),
    );
    need(
        "needs an `include` list, so the package holds only what the library needs",
        !m.include.is_empty(),
    );
    for keyword in &m.keywords {
        if !valid_keyword(keyword) {
            findings.push(format!(
                "{name}: keyword {keyword:?} is not one crates.io accepts (ASCII letters, digits, \
                 `_`, `-` or `+`, starting with a letter or digit, at most {MAX_KEYWORD_CHARS} characters)"
            ));
        }
    }
    if let Some((readme, true)) = &m.readme
        && !m.include.is_empty()
        && !included(&m.include, readme)
    {
        findings.push(format!(
            "{name}: the readme {readme} is not in the `include` list"
        ));
    }
    for e in &m.embedded {
        match &e.relative {
            None => findings.push(format!(
                "{name}: {} embeds a file outside the crate, which a published crate cannot \
                 build without (OI-302, decision 0103)",
                e.site
            )),
            Some(relative) if !included(&m.include, relative) => findings.push(format!(
                "{name}: {} embeds {relative}, which the `include` list leaves out of the package",
                e.site
            )),
            Some(_) => {}
        }
    }
    for d in &m.dependencies {
        let unpublished = d.member_publishable == Some(false);
        match d.kind {
            DepKind::Normal | DepKind::Build => {
                if d.path && !d.has_version {
                    findings.push(format!(
                        "{name}: the path dependency {} needs a version, or cargo cannot publish it",
                        d.name
                    ));
                }
                if unpublished {
                    findings.push(format!(
                        "{name}: depends on {}, which is never published",
                        d.name
                    ));
                }
            }
            DepKind::Dev => {
                if unpublished && d.has_version {
                    findings.push(format!(
                        "{name}: the dev-dependency {} is never published, so its entry must have \
                         no version and cargo leaves it out of the published manifest (OI-302)",
                        d.name
                    ));
                }
            }
        }
    }
    findings
}

/// Lexically normalises a path (`..` and `.`), without touching the file system.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// The literal paths of `include_str!` and `include_bytes!` in `source`.
fn embedded_literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for macro_name in ["include_str!(\"", "include_bytes!(\""] {
        let mut rest = source;
        while let Some(at) = rest.find(macro_name) {
            rest = rest.get(at + macro_name.len()..).unwrap_or_default();
            if let Some(end) = rest.find('"') {
                out.push(rest.get(..end).unwrap_or_default().to_owned());
                rest = rest.get(end..).unwrap_or_default();
            }
        }
    }
    out
}

/// The files the sources under `dir/src` embed.
fn embedded_files(dir: &Path) -> Result<Vec<Embedded>> {
    let mut sources = Vec::new();
    let mut pending = vec![dir.join("src")];
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(&folder)
            .with_context(|| format!("cannot read {}", folder.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    let crate_dir = normalise(dir);
    let mut out = Vec::new();
    for source in sources {
        let text = std::fs::read_to_string(&source)
            .with_context(|| format!("cannot read {}", source.display()))?;
        let parent = source.parent().unwrap_or(dir);
        let shown = source.strip_prefix(dir).unwrap_or(&source).display();
        for literal in embedded_literals(&text) {
            let target = normalise(&parent.join(&literal));
            let relative = target.strip_prefix(&crate_dir).ok().map(|r| {
                r.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            });
            out.push(Embedded {
                site: format!("{shown} ({literal})"),
                relative,
            });
        }
    }
    Ok(out)
}

/// Reads the library family's manifests through `cargo metadata` and checks each.
pub fn check(root: &Path) -> Result<Findings> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .context("cannot run cargo metadata")?;
    let publishable =
        |p: &cargo_metadata::Package| p.publish.as_ref().is_none_or(|r| !r.is_empty());
    let members: BTreeMap<String, bool> = metadata
        .workspace_packages()
        .into_iter()
        .map(|p| (p.name.to_string(), publishable(p)))
        .collect();
    let mut findings = Findings::new();
    for family in LIBRARY_FAMILY {
        let Some(package) = metadata
            .workspace_packages()
            .into_iter()
            .find(|p| p.name.as_str() == *family)
        else {
            findings.push(format!("{family}: not a workspace member"));
            continue;
        };
        let dir = package
            .manifest_path
            .parent()
            .context("a manifest has no folder")?
            .as_std_path()
            .to_path_buf();
        let raw: toml::Table = toml::from_str(
            &std::fs::read_to_string(&package.manifest_path)
                .with_context(|| format!("cannot read {}", package.manifest_path))?,
        )
        .with_context(|| format!("cannot parse {}", package.manifest_path))?;
        let include = raw
            .get("package")
            .and_then(|p| p.get("include"))
            .and_then(toml::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(toml::Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let readme = package.readme.as_ref().map(|r| {
            let path = normalise(&dir.join(r.as_std_path()));
            let inside = path.starts_with(normalise(&dir)) && path.is_file();
            (r.to_string(), inside)
        });
        let dependencies = package
            .dependencies
            .iter()
            .map(|d| Dep {
                name: d.name.clone(),
                kind: match d.kind {
                    cargo_metadata::DependencyKind::Development => DepKind::Dev,
                    cargo_metadata::DependencyKind::Build => DepKind::Build,
                    _ => DepKind::Normal,
                },
                has_version: d.req.to_string() != "*",
                path: d.path.is_some(),
                member_publishable: members.get(&d.name).copied(),
            })
            .collect();
        let manifest = Manifest {
            name: package.name.to_string(),
            publish: publishable(package),
            description: package.description.clone(),
            license: package.license.clone(),
            repository: package.repository.clone(),
            readme,
            rust_version: package.rust_version.as_ref().map(ToString::to_string),
            keywords: package.keywords.clone(),
            categories: package.categories.clone(),
            include,
            dependencies,
            embedded: embedded_files(&dir)?,
        };
        findings.extend(check_manifest(&manifest));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A manifest shaped like `openchuck-post` once it is ready to publish.
    fn ready() -> Manifest {
        Manifest {
            name: "openchuck-post".into(),
            publish: true,
            description: Some("Post-processing.".into()),
            license: Some(PROJECT_LICENCE.into()),
            repository: Some("https://github.com/paulhampson/OpenChuckCAM".into()),
            readme: Some(("README.md".into(), true)),
            rust_version: Some("1.97".into()),
            keywords: vec!["cnc".into(), "g-code".into()],
            categories: vec!["science".into()],
            include: vec!["/src/**".into(), "/posts/**".into(), "/README.md".into()],
            dependencies: vec![
                Dep {
                    name: "openchuck-types".into(),
                    kind: DepKind::Normal,
                    has_version: true,
                    path: true,
                    member_publishable: Some(true),
                },
                Dep {
                    name: "openchuck-testkit".into(),
                    kind: DepKind::Dev,
                    has_version: false,
                    path: true,
                    member_publishable: Some(false),
                },
            ],
            embedded: vec![Embedded {
                site: "src/builtin.rs (../posts/linuxcnc/post.toml)".into(),
                relative: Some("posts/linuxcnc/post.toml".into()),
            }],
        }
    }

    #[test]
    fn req_int_01_req_nfr_10_a_ready_manifest_passes() {
        assert_eq!(check_manifest(&ready()), Findings::new());
    }

    /// OI-302's first cause: a source embeds a file outside the crate, or one its package
    /// leaves out.
    #[test]
    fn req_int_01_an_embedded_file_outside_the_crate_or_the_package_fails() {
        let mut m = ready();
        m.embedded[0].relative = None;
        let findings = check_manifest(&m);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("outside the crate"), "{findings:?}");

        let mut m = ready();
        m.include.retain(|p| p != "/posts/**");
        let findings = check_manifest(&m);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("leaves out"), "{findings:?}");
    }

    /// OI-302's second cause: a dev-dependency on the never-published testkit that carries a
    /// version, so cargo keeps it in the published manifest.
    #[test]
    fn req_int_01_an_unpublished_dev_dependency_with_a_version_fails() {
        let mut m = ready();
        m.dependencies[1].has_version = true;
        let findings = check_manifest(&m);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("openchuck-testkit"), "{findings:?}");
    }

    #[test]
    fn req_int_01_a_normal_dependency_must_carry_a_version_and_be_published() {
        let mut m = ready();
        m.dependencies[0].has_version = false;
        assert_eq!(check_manifest(&m).len(), 1);
        let mut m = ready();
        m.dependencies[0].name = "openchuck-buildinfo".into();
        m.dependencies[0].member_publishable = Some(false);
        let findings = check_manifest(&m);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains("never published"), "{findings:?}");
        let mut m = ready();
        m.dependencies[0].kind = DepKind::Build;
        m.dependencies[0].has_version = false;
        assert_eq!(check_manifest(&m).len(), 1);
    }

    #[test]
    fn req_nfr_10_the_licence_must_be_the_project_licence() {
        let mut m = ready();
        m.license = Some("MIT".into());
        let findings = check_manifest(&m);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].contains(PROJECT_LICENCE), "{findings:?}");
    }

    /// A named change that breaks one rule.
    type Break = (&'static str, fn(&mut Manifest));

    /// Each piece of metadata crates.io requires or shows, removed in turn, gives one finding.
    #[test]
    fn req_int_07_req_int_08_each_missing_piece_of_metadata_fails() {
        let breaks: [Break; 10] = [
            ("publish", |m| m.publish = false),
            ("description", |m| m.description = Some("  ".into())),
            ("repository", |m| m.repository = None),
            ("readme", |m| m.readme = None),
            ("readme outside", |m| {
                m.readme = Some(("../README.md".into(), false))
            }),
            ("rust-version", |m| m.rust_version = None),
            ("keywords", |m| m.keywords.clear()),
            ("categories", |m| m.categories = vec!["science".into(); 6]),
            ("include", |m| {
                m.include.clear();
                m.embedded.clear();
            }),
            ("readme not included", |m| {
                m.include.retain(|p| p != "/README.md")
            }),
        ];
        for (what, change) in breaks {
            let mut m = ready();
            change(&mut m);
            let findings = check_manifest(&m);
            assert_eq!(findings.len(), 1, "{what}: {findings:?}");
        }
    }

    #[test]
    fn req_int_08_keywords_follow_the_crates_io_rules() {
        for good in ["cnc", "g-code", "4-axis", "c++", "tool_path"] {
            assert!(valid_keyword(good), "{good}");
        }
        for bad in ["-cnc", "", "g code", "a-very-long-keyword-indeed", "café"] {
            assert!(!valid_keyword(bad), "{bad}");
        }
        let mut m = ready();
        m.keywords.push("g code".into());
        assert_eq!(check_manifest(&m).len(), 1);
    }

    #[test]
    fn embedded_literals_are_found_and_resolved() {
        let text = r#"const A: &str = include_str!("../posts/a.toml");
            const B: &[u8] = include_bytes!("../../../posts/b.post");"#;
        assert_eq!(
            embedded_literals(text),
            vec!["../posts/a.toml", "../../../posts/b.post"]
        );
        assert_eq!(
            normalise(Path::new("/r/crates/c/src/../posts/a")),
            PathBuf::from("/r/crates/c/posts/a")
        );
    }

    /// The real workspace: the library family is ready to publish (OI-302 fixed).
    #[test]
    fn req_int_01_req_nfr_10_the_library_family_is_ready_to_publish() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert_eq!(check(&root).unwrap(), Findings::new());
    }
}
