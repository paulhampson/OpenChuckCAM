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
//!     Resources/OpenChuckCAM.icns    the icon (CFBundleIconFile), only when one is given
//!     MacOS/openchuck-ui             the desktop application (CFBundleExecutable)
//!     MacOS/openchuck-step-worker    the STEP reader, found beside the application (OI-118)
//! ```
//!
//! The bundle is not signed with a certificate or notarised (owner's decision of 7 Nov 2026,
//! OI-135); the release build gives it an ad-hoc signature on macOS (`codesign --sign -`, no
//! certificate; decision 0541), which this command cannot do on Linux.
//!
//! The icon is optional: given a 512 by 512 PNG (`--icon`), the command wraps it as a one-image
//! `.icns` and names it in `CFBundleIconFile`; without one the bundle has no icon and macOS shows
//! its generic application icon. OpenChuckCAM has no icon of its own yet (OI-2106, lane ICON). Its tree is
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
/// The icon file inside `Contents/Resources`, as `CFBundleIconFile` names it.
pub const ICON: &str = "OpenChuckCAM.icns";
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
    /// A 512 by 512 PNG for the icon; no icon when `None`.
    pub icon: Option<PathBuf>,
}

/// An `.icns` file holding one 512 by 512 PNG (the `ic09` entry), the form macOS reads for a
/// bundle icon. The PNG must be 512 by 512 pixels.
pub fn icns(png: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        png.get(..8) == Some(b"\x89PNG\r\n\x1a\n".as_slice())
            && png.get(12..16) == Some(b"IHDR".as_slice()),
        "the icon is not a PNG file"
    );
    let size = |at: usize| -> Result<u32> {
        let bytes: [u8; 4] = png
            .get(at..at + 4)
            .context("the icon PNG ends inside its header")?
            .try_into()
            .context("the icon PNG ends inside its header")?;
        Ok(u32::from_be_bytes(bytes))
    };
    let (width, height) = (size(16)?, size(20)?);
    ensure!(
        width == 512 && height == 512,
        "the icon PNG is {width} by {height} pixels; the bundle icon needs 512 by 512"
    );
    let entry = u32::try_from(png.len() + 8).context("the icon PNG is too large")?;
    let total = entry.checked_add(8).context("the icon PNG is too large")?;
    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(b"ic09");
    out.extend_from_slice(&entry.to_be_bytes());
    out.extend_from_slice(png);
    Ok(out)
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

/// The Info.plist text: the template with its version marks replaced, and `CFBundleIconFile`
/// added when the bundle has an icon.
pub fn info_plist(template: &str, version: &str, with_icon: bool) -> Result<String> {
    check_version(version)?;
    ensure!(
        template.contains(VERSION_MARK),
        "the Info.plist template has no {VERSION_MARK} mark"
    );
    let mut text = template.replace(VERSION_MARK, version);
    ensure!(
        !text.contains("<key>CFBundleIconFile</key>"),
        "the Info.plist template names an icon; the command adds it when one is given"
    );
    if with_icon {
        let anchor = "\t<key>CFBundleIdentifier</key>";
        ensure!(
            text.contains(anchor),
            "the Info.plist template has no CFBundleIdentifier"
        );
        let stem = ICON.trim_end_matches(".icns");
        text = text.replacen(
            anchor,
            &format!("\t<key>CFBundleIconFile</key>\n\t<string>{stem}</string>\n{anchor}"),
            1,
        );
    }
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
    let plist = info_plist(&template, &version, options.icon.is_some())?;
    let icon = match &options.icon {
        Some(path) => {
            let png = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
            Some(icns(&png).with_context(|| format!("reading {}", path.display()))?)
        }
        None => None,
    };
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
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos).with_context(|| format!("creating {}", macos.display()))?;
    if let Some(icon) = &icon {
        fs::create_dir_all(&resources)
            .with_context(|| format!("creating {}", resources.display()))?;
        write_file(&resources.join(ICON), icon, 0o644)?;
    }
    write_file(&contents.join("Info.plist"), plist.as_bytes(), 0o644)?;
    write_file(&contents.join("PkgInfo"), b"APPL????", 0o644)?;
    for (from, name) in [(&options.ui, UI_EXE), (&options.worker, WORKER_EXE)] {
        let bytes = fs::read(from).with_context(|| format!("reading {}", from.display()))?;
        write_file(&macos.join(name), &bytes, 0o755)?;
    }

    // Times last, children before parents, so writing a file does not change a folder's time.
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(options.mtime);
    let mut paths = vec![
        macos.join(UI_EXE),
        macos.join(WORKER_EXE),
        contents.join("Info.plist"),
        contents.join("PkgInfo"),
        macos,
    ];
    if icon.is_some() {
        paths.extend([resources.join(ICON), resources]);
    }
    paths.extend([contents, bundle.clone()]);
    for path in &paths {
        set_time(path, time)?;
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
            icon: None,
        }
    }

    /// The start of a PNG file of the given size: signature and header chunk.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        out.extend_from_slice(&width.to_be_bytes());
        out.extend_from_slice(&height.to_be_bytes());
        out.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        out
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
        let text = info_plist(&template, "0.49.0", false).expect("plist");
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
        assert_eq!(value("CFBundleIdentifier"), "cam.openchuck.desktop");
        assert_eq!(value("CFBundleName"), "OpenChuckCAM");
        assert_eq!(value("CFBundlePackageType"), "APPL");
        assert_eq!(value("CFBundleShortVersionString"), "0.49.0");
        assert_eq!(value("CFBundleVersion"), "0.49.0");
        assert_eq!(value("LSMinimumSystemVersion"), "11.0");
        assert!(
            !text.contains("CFBundleIconFile"),
            "no icon unless one is given"
        );
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
    fn req_nfr_17_the_icon_is_a_one_image_icns_of_the_png() {
        let source = png(512, 512);
        let icns = icns(&source).expect("icns");
        let total = (source.len() + 16) as u32;
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(icns[4..8], total.to_be_bytes());
        assert_eq!(&icns[8..12], b"ic09");
        assert_eq!(icns[12..16], (total - 8).to_be_bytes());
        assert_eq!(&icns[16..], source.as_slice());
    }

    #[test]
    fn req_nfr_17_a_given_icon_is_in_the_bundle_and_named_in_the_plist() {
        let dir = scratch("icon");
        let mut options = inputs(&dir);
        let icon = dir.join("icon-512.png");
        fs::write(&icon, png(512, 512)).expect("icon");
        options.icon = Some(icon);
        let bundle = build(&root(), &options).expect("bundle");
        assert_eq!(
            fs::read(bundle.join("Contents/Resources/OpenChuckCAM.icns")).expect("icon"),
            icns(&png(512, 512)).expect("icns")
        );
        let tree = tree(&bundle);
        assert_eq!(tree["Contents/Resources/OpenChuckCAM.icns"].0, 0o644);
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(MTIME);
        assert!(
            tree.values().all(|(_, t, _)| *t == time),
            "every time is normalised"
        );
        let plist = fs::read_to_string(bundle.join("Contents/Info.plist")).expect("plist");
        assert!(
            plist.contains("<key>CFBundleIconFile</key>\n\t<string>OpenChuckCAM</string>\n\t<key>CFBundleIdentifier</key>"),
            "{plist}"
        );
        fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn req_nfr_07_an_icon_of_the_wrong_kind_or_size_is_a_clear_error() {
        let error = icns(b"GIF89a").expect_err("not a PNG").to_string();
        assert!(error.contains("not a PNG"), "{error}");
        let error = icns(&png(256, 256)).expect_err("too small").to_string();
        assert!(error.contains("256 by 256"), "{error}");
        let error = icns(&png(512, 512)[..18]).expect_err("cut").to_string();
        assert!(error.contains("ends inside"), "{error}");
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
        let error = info_plist("<plist/>", "1.0.0", false)
            .expect_err("no mark")
            .to_string();
        assert!(error.contains("@VERSION@"), "{error}");
        fs::remove_dir_all(&dir).expect("clean up");
    }
}
