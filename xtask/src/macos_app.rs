// SPDX-License-Identifier: AGPL-3.0-or-later
//! `cargo xtask macos-app`: the macOS application bundle for `openchuck-ui` (architecture
//! section 12; decision 0031 item 4; decision 0541; OI-300).
//!
//! Builds `OpenChuckCAM.app` from the template `xtask/templates/macos/Info.plist`, with no
//! dependency added:
//!
//! ```text
//! OpenChuckCAM.app/
//!   Contents/
//!     Info.plist                     the template, with the workspace version
//!     PkgInfo                        "APPL????"
//!     MacOS/openchuck-ui             the desktop application (CFBundleExecutable)
//!     MacOS/openchuck-step-worker    the STEP reader, found beside the application (OI-118)
//! ```
//!
//! The bundle is not signed or notarised (owner's decision of 7 Nov 2026, OI-135). Its tree is
//! deterministic: the same inputs give the same files, bytes, modes and modification times
//! (every time is set to the given one, normally the commit time), so packing it with a sorted,
//! owner-free tar gives the same archive.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail, ensure};

/// The bundle's folder name.
pub const BUNDLE: &str = "OpenChuckCAM.app";
/// The application executable inside `Contents/MacOS`, as `CFBundleExecutable` names it.
pub const UI_EXE: &str = "openchuck-ui";
/// The STEP reader inside `Contents/MacOS`, beside the application.
pub const WORKER_EXE: &str = "openchuck-step-worker";
/// The template, relative to the workspace root.
pub const TEMPLATE: &str = "xtask/templates/macos/Info.plist";
const VERSION_MARK: &str = "@VERSION@";

/// What to bundle.
#[derive(Debug, Clone)]
pub struct Options {
    /// The `openchuck-ui` executable.
    pub ui: PathBuf,
    /// The `openchuck-step-worker` executable.
    pub worker: PathBuf,
    /// The folder the bundle is written into; `<out>/OpenChuckCAM.app` is replaced.
    pub out: PathBuf,
    /// The version for `CFBundleShortVersionString` and `CFBundleVersion`; the workspace
    /// version when `None`.
    pub version: Option<String>,
    /// Modification time of every file and folder, seconds since 1970 (normally the commit
    /// time, `git log -1 --format=%ct`).
    pub mtime: u64,
}

/// A version macOS accepts in `CFBundleShortVersionString`: one to three non-negative integers
/// separated by dots, so a pre-release suffix such as `-alpha` is refused.
pub fn check_version(version: &str) -> Result<()> {
    let parts: Vec<&str> = version.split('.').collect();
    ensure!(
        (1..=3).contains(&parts.len())
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
        "the bundle version `{version}` is not one to three numbers separated by dots, as macOS \
         needs in CFBundleShortVersionString"
    );
    Ok(())
}

/// The Info.plist text: the template with its version marks replaced.
pub fn info_plist(template: &str, version: &str) -> Result<String> {
    check_version(version)?;
    ensure!(
        template.contains(VERSION_MARK),
        "the Info.plist template has no {VERSION_MARK} mark"
    );
    let text = template.replace(VERSION_MARK, version);
    for key in [
        "CFBundleExecutable",
        "CFBundleIdentifier",
        "CFBundlePackageType",
        "CFBundleShortVersionString",
    ] {
        ensure!(
            text.contains(&format!("<key>{key}</key>")),
            "the Info.plist template has no {key}"
        );
    }
    ensure!(
        text.contains(&format!("<string>{UI_EXE}</string>")),
        "the Info.plist template does not name {UI_EXE} as the executable"
    );
    Ok(text)
}

/// The workspace version from the root `Cargo.toml` (`[workspace.package] version`).
pub fn workspace_version(root: &Path) -> Result<String> {
    let path = root.join("Cargo.toml");
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let manifest: toml::Value =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .context("Cargo.toml has no [workspace.package] version")
}

/// Builds the bundle and returns its path.
pub fn build(root: &Path, options: &Options) -> Result<PathBuf> {
    let version = match &options.version {
        Some(v) => v.clone(),
        None => workspace_version(root)?,
    };
    let template_path = root.join(TEMPLATE);
    let template = fs::read_to_string(&template_path)
        .with_context(|| format!("reading {}", template_path.display()))?;
    let plist = info_plist(&template, &version)?;
    for (what, path) in [
        ("openchuck-ui", &options.ui),
        ("the STEP reader", &options.worker),
    ] {
        if !path.is_file() {
            bail!("{what} is not a file: {}", path.display());
        }
    }
    ensure!(
        options.out.is_dir(),
        "the output folder is not a folder: {}",
        options.out.display()
    );

    let bundle = options.out.join(BUNDLE);
    if bundle.exists() {
        fs::remove_dir_all(&bundle).with_context(|| format!("removing {}", bundle.display()))?;
    }
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    fs::create_dir_all(&macos).with_context(|| format!("creating {}", macos.display()))?;
    write_file(&contents.join("Info.plist"), plist.as_bytes(), 0o644)?;
    write_file(&contents.join("PkgInfo"), b"APPL????", 0o644)?;
    for (from, name) in [(&options.ui, UI_EXE), (&options.worker, WORKER_EXE)] {
        let bytes = fs::read(from).with_context(|| format!("reading {}", from.display()))?;
        write_file(&macos.join(name), &bytes, 0o755)?;
    }

    // Times last, children before parents, so writing a file does not change a folder's time.
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(options.mtime);
    for path in [
        macos.join(UI_EXE),
        macos.join(WORKER_EXE),
        contents.join("Info.plist"),
        contents.join("PkgInfo"),
        macos,
        contents,
        bundle.clone(),
    ] {
        set_time(&path, time)?;
    }
    Ok(bundle)
}

fn write_file(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))?;
    set_mode(path, mode)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("setting the mode of {}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    bail!("the macOS bundle is built on macOS or Linux, where file modes exist")
}

fn set_time(path: &Path, time: SystemTime) -> Result<()> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    file.set_modified(time)
        .with_context(|| format!("setting the time of {}", path.display()))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    const MTIME: u64 = 1_793_000_000;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-macos-app-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch folder");
        dir
    }

    /// Fake executables in `dir`, and the options to bundle them into `dir/out`.
    fn inputs(dir: &Path) -> Options {
        let ui = dir.join("openchuck-ui");
        let worker = dir.join("openchuck-step-worker");
        fs::write(&ui, b"ui program").expect("ui");
        fs::write(&worker, b"worker program").expect("worker");
        let out = dir.join("out");
        fs::create_dir_all(&out).expect("out");
        Options {
            ui,
            worker,
            out,
            version: Some("1.2.3".into()),
            mtime: MTIME,
        }
    }

    /// Every path under `dir`, relative, with its mode, time and (for files) bytes.
    fn tree(dir: &Path) -> BTreeMap<String, (u32, SystemTime, Option<Vec<u8>>)> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(path) = stack.pop() {
            let meta = fs::metadata(&path).expect("metadata");
            let rel = path
                .strip_prefix(dir)
                .expect("under dir")
                .to_string_lossy()
                .into_owned();
            let bytes = if meta.is_dir() {
                for entry in fs::read_dir(&path).expect("read_dir") {
                    stack.push(entry.expect("entry").path());
                }
                None
            } else {
                Some(fs::read(&path).expect("read"))
            };
            out.insert(
                rel,
                (
                    meta.permissions().mode() & 0o7777,
                    meta.modified().expect("mtime"),
                    bytes,
                ),
            );
        }
        out
    }

    #[test]
    fn req_nfr_17_the_bundle_has_the_layout_macos_launches() {
        let dir = scratch("layout");
        let options = inputs(&dir);
        let bundle = build(&root(), &options).expect("bundle");
        assert_eq!(bundle, options.out.join("OpenChuckCAM.app"));
        let files: Vec<String> = tree(&bundle).into_keys().collect();
        assert_eq!(
            files,
            [
                "",
                "Contents",
                "Contents/Info.plist",
                "Contents/MacOS",
                "Contents/MacOS/openchuck-step-worker",
                "Contents/MacOS/openchuck-ui",
                "Contents/PkgInfo",
            ]
        );
        assert_eq!(
            fs::read(bundle.join("Contents/MacOS/openchuck-ui")).expect("ui"),
            b"ui program"
        );
        assert_eq!(
            fs::read(bundle.join("Contents/MacOS/openchuck-step-worker")).expect("worker"),
            b"worker program"
        );
        assert_eq!(
            fs::read(bundle.join("Contents/PkgInfo")).expect("PkgInfo"),
            b"APPL????"
        );
        fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn req_nfr_17_the_executables_run_and_the_data_files_do_not() {
        let dir = scratch("modes");
        let bundle = build(&root(), &inputs(&dir)).expect("bundle");
        let tree = tree(&bundle);
        let mode = |p: &str| tree[p].0;
        assert_eq!(mode("Contents/MacOS/openchuck-ui"), 0o755);
        assert_eq!(mode("Contents/MacOS/openchuck-step-worker"), 0o755);
        assert_eq!(mode("Contents/Info.plist"), 0o644);
        assert_eq!(mode("Contents/PkgInfo"), 0o644);
        fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn req_nfr_02_the_info_plist_names_the_program_identifier_and_version() {
        let template = fs::read_to_string(root().join(TEMPLATE)).expect("template");
        let text = info_plist(&template, "0.49.0").expect("plist");
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(text.contains("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\""));
        assert!(!text.contains("@VERSION@"));
        let value = |key: &str| {
            let at = text
                .find(&format!("<key>{key}</key>"))
                .unwrap_or_else(|| panic!("no {key}"));
            let rest = &text[at..];
            let start = rest.find("<string>").expect("string") + "<string>".len();
            let end = rest.find("</string>").expect("end");
            rest[start..end].to_owned()
        };
        assert_eq!(value("CFBundleExecutable"), "openchuck-ui");
        assert_eq!(
            value("CFBundleIdentifier"),
            "io.github.paulhampson.openchuckcam"
        );
        assert_eq!(value("CFBundleName"), "OpenChuckCAM");
        assert_eq!(value("CFBundlePackageType"), "APPL");
        assert_eq!(value("CFBundleShortVersionString"), "0.49.0");
        assert_eq!(value("CFBundleVersion"), "0.49.0");
        assert_eq!(value("LSMinimumSystemVersion"), "11.0");
        // Balanced: one plist, one dict, and every key has its value.
        assert_eq!(text.matches("<dict>").count(), 1);
        assert_eq!(text.matches("</dict>").count(), 1);
        assert_eq!(text.matches("</plist>").count(), 1);
        assert_eq!(
            text.matches("<key>").count(),
            text.matches("<string>").count() + text.matches("<true/>").count()
        );
    }

    #[test]
    fn req_nfr_02_the_bundle_takes_the_workspace_version_by_default() {
        let dir = scratch("version");
        let mut options = inputs(&dir);
        options.version = None;
        let bundle = build(&root(), &options).expect("bundle");
        let plist = fs::read_to_string(bundle.join("Contents/Info.plist")).expect("plist");
        let version = workspace_version(&root()).expect("version");
        assert!(plist.contains(&format!("<string>{version}</string>")));
        fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn req_nfr_06_the_same_inputs_give_the_same_bundle() {
        let first = scratch("same-a");
        let second = scratch("same-b");
        let a = build(&root(), &inputs(&first)).expect("first");
        // A second build over an old bundle replaces it entirely.
        fs::write(a.join("Contents/stale"), b"left over").expect("stale");
        let a = build(&root(), &inputs(&first)).expect("rebuild");
        let b = build(&root(), &inputs(&second)).expect("second");
        let (ta, tb) = (tree(&a), tree(&b));
        assert_eq!(ta, tb);
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(MTIME);
        assert!(
            ta.values().all(|(_, t, _)| *t == time),
            "every time is normalised"
        );
        fs::remove_dir_all(&first).expect("clean up");
        fs::remove_dir_all(&second).expect("clean up");
    }

    #[test]
    fn req_nfr_07_a_version_macos_refuses_is_a_clear_error() {
        for bad in ["", "1.2.3.4", "0.49.0-alpha", "v1", "1..2", "1.x"] {
            let error = check_version(bad).expect_err(bad).to_string();
            assert!(error.contains("CFBundleShortVersionString"), "{error}");
        }
        for good in ["1", "0.49", "0.49.0", "2026.1.0"] {
            check_version(good).expect(good);
        }
    }

    #[test]
    fn req_nfr_07_a_missing_input_is_a_clear_error() {
        let dir = scratch("missing");
        let mut options = inputs(&dir);
        options.worker = dir.join("no-such-worker");
        let error = build(&root(), &options)
            .expect_err("missing worker")
            .to_string();
        assert!(error.contains("STEP reader is not a file"), "{error}");
        let mut options = inputs(&dir);
        options.out = dir.join("no-such-folder");
        let error = build(&root(), &options)
            .expect_err("missing out")
            .to_string();
        assert!(error.contains("output folder"), "{error}");
        let error = info_plist("<plist/>", "1.0.0")
            .expect_err("no mark")
            .to_string();
        assert!(error.contains("@VERSION@"), "{error}");
        fs::remove_dir_all(&dir).expect("clean up");
    }
}
