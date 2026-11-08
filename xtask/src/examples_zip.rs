// SPDX-License-Identifier: AGPL-3.0-or-later
//! `cargo xtask examples-zip`: one zip of the example projects under `testdata/examples/` for the
//! nightly build and the release (lane EXAMPLEZIP; decisions 0023, 0091 and 0541; NFR-06, NFR-12).
//!
//! No dependency is added: the zip is written here, with every entry stored (not compressed) and
//! a CRC-32 of our own. The same tree always gives the same bytes: entries are sorted by path,
//! every time is 1 January 1980, and there are no extra fields, comments or file attributes.
//!
//! Only `testdata/examples/` is ever read, and the zip is checked twice (on writing, and again on
//! the finished file by `--check`): every entry must sit under the one top folder, and its file
//! extension must be on [`ALLOWED_EXTENSIONS`]. A symbolic link, a name with `..`, or any other
//! file type fails the build, so a stray file (a private project, a key) cannot be packaged.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

/// The examples folder, relative to the workspace root. Nothing outside it is read.
pub const EXAMPLES_DIR: &str = "testdata/examples";
/// File types an example may hold. No G-code: examples store none (`testdata/README.md`).
/// `post` is a post-processor template, plain text (the user guide's example post,
/// `testdata/examples/minimal-post/`).
pub const ALLOWED_EXTENSIONS: &[&str] = &["stl", "step", "stp", "toml", "md", "txt", "py", "post"];
/// The index written into the zip.
pub const INDEX_NAME: &str = "EXAMPLES-README.txt";
const NOTICE_FILE: &str = "crates/openchuck-types/src/notice.rs";
/// MS-DOS date 1980-01-01, the earliest a zip can state.
const DOS_DATE: u16 = 0x0021;

/// CRC-32 (IEEE, as zip uses), bit by bit: the files are small.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Whether `path` (relative, `/`-separated) may be packaged: plain names, an allowed extension.
fn check_name(path: &str) -> Result<()> {
    // Letters, digits, `.`, `-`, `_` and the `/` separator only: a backslash, a colon (an NTFS
    // stream) or a control character can make some Windows extractors write outside the folder.
    ensure!(
        path.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'-' | b'_')),
        "{path}: the name has a character outside letters, digits, '.', '-' and '_'"
    );
    for part in path.split('/') {
        ensure!(
            !part.is_empty() && part != ".." && part != "." && !part.starts_with('.'),
            "{path}: not a plain relative path (or a hidden file)"
        );
    }
    let ext = path
        .rsplit('/')
        .next()
        .and_then(|file| file.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match ext {
        Some(ext) if ALLOWED_EXTENSIONS.contains(&ext.as_str()) => Ok(()),
        _ => bail!(
            "{path}: file type not allowed in the examples zip (allowed: {})",
            ALLOWED_EXTENSIONS.join(", ")
        ),
    }
}

/// Every file under `dir`, as `/`-separated paths relative to it, sorted. A symbolic link or
/// any entry that is neither a file nor a folder fails.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|n| anyhow::anyhow!("{n:?}: the name is not UTF-8"))?;
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(&entry.path(), &rel, out)?;
        } else if kind.is_file() {
            out.push(rel);
        } else {
            bail!("{rel}: a link or special file cannot be packaged");
        }
    }
    out.sort();
    Ok(())
}

/// The NFR-12 statement, read from `crates/openchuck-types/src/notice.rs` so it is stated once
/// (xtask does not depend on the library crates). A `\` at the end of a line joins the lines.
pub fn verify_notice(root: &Path) -> Result<String> {
    let path = root.join(NOTICE_FILE);
    let src = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let start = src
        .find("VERIFY_GCODE_NOTICE: &str = \"")
        .context("VERIFY_GCODE_NOTICE not found")?
        + "VERIFY_GCODE_NOTICE: &str = \"".len();
    let body = &src[start..];
    let end = body
        .find("\";")
        .context("VERIFY_GCODE_NOTICE is not closed")?;
    let mut text = String::new();
    for (i, line) in body[..end].lines().enumerate() {
        let line = if i == 0 { line } else { line.trim_start() };
        text.push_str(line.strip_suffix('\\').unwrap_or(line));
    }
    ensure!(!text.is_empty(), "the G-code notice is empty");
    Ok(text)
}

/// The index text: what the zip is, one line per project, and the NFR-12 wording.
fn index_text(root: &Path, files: &[String]) -> Result<String> {
    let mut projects: Vec<&str> = files
        .iter()
        .filter_map(|f| f.split_once('/').map(|(p, _)| p))
        .collect();
    projects.dedup();
    let mut text = String::from("OpenChuckCAM example projects\r\n\r\n");
    text.push_str(
        "Each folder is one example: models and, where it has one, a finished project to open\r\n\
         with File > Open project. Read the README.md in each folder first.\r\n\r\n",
    );
    for project in projects {
        let readme = root.join(EXAMPLES_DIR).join(project).join("README.md");
        let title = fs::read_to_string(&readme)
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("# ").map(|s| s.trim().to_string()))
            })
            .unwrap_or_else(|| "(no README.md)".to_string());
        text.push_str(&format!("  {project}/  {title}\r\n"));
    }
    text.push_str(&format!("\r\n{}\r\n", verify_notice(root)?));
    Ok(text)
}

struct Entry {
    name: String,
    data: Vec<u8>,
}

/// Writes the zip bytes for `entries` (already in order).
fn write_zip(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let size = u32::try_from(e.data.len()).context("file over 4 GiB")?;
        let offset = u32::try_from(out.len()).context("zip over 4 GiB")?;
        let crc = crc32(&e.data);
        let name_len = u16::try_from(e.name.len()).context("name too long")?;
        // Local header: signature, version 2.0, flags 0, stored, time 0, date, crc, sizes.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // flags, method, time
        out.extend_from_slice(&DOS_DATE.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(e.name.as_bytes());
        out.extend_from_slice(&e.data);
        // Central directory record: made by MS-DOS 2.0 (no Unix attributes).
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&DOS_DATE.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&[0u8; 12]); // extra, comment, disk, internal, external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(e.name.as_bytes());
    }
    let count = u16::try_from(entries.len()).context("too many files")?;
    let cd_offset = u32::try_from(out.len())?;
    let cd_size = u32::try_from(central.len())?;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

/// Builds the zip of everything under `<root>/testdata/examples/`, with every entry inside the
/// folder `top`, and checks it before returning it.
pub fn build(root: &Path, top: &str) -> Result<Vec<u8>> {
    check_name(&format!("{top}/x.txt")).context("the top folder name")?;
    let dir = root.join(EXAMPLES_DIR);
    ensure!(dir.is_dir(), "{} does not exist", dir.display());
    let mut files = Vec::new();
    walk(&dir, "", &mut files)?;
    ensure!(!files.is_empty(), "{EXAMPLES_DIR} holds no files");
    let mut entries = vec![Entry {
        name: format!("{top}/{INDEX_NAME}"),
        data: index_text(root, &files)?.into_bytes(),
    }];
    for rel in &files {
        check_name(rel)?;
        entries.push(Entry {
            name: format!("{top}/{rel}"),
            data: fs::read(dir.join(rel)).with_context(|| format!("reading {rel}"))?,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let bytes = write_zip(&entries)?;
    check(&bytes, top)?;
    Ok(bytes)
}

fn u16_at(b: &[u8], i: usize) -> Result<usize> {
    let s: [u8; 2] = b.get(i..i + 2).context("zip cut short")?.try_into()?;
    Ok(usize::from(u16::from_le_bytes(s)))
}
fn u32_at(b: &[u8], i: usize) -> Result<u32> {
    let s: [u8; 4] = b.get(i..i + 4).context("zip cut short")?.try_into()?;
    Ok(u32::from_le_bytes(s))
}

/// Reads the central directory of a zip written by [`write_zip`]: name, CRC, bytes.
fn read_entries(b: &[u8]) -> Result<Vec<(String, u32, Vec<u8>)>> {
    ensure!(b.len() >= 22, "not a zip file");
    let eocd = b.len() - 22;
    ensure!(
        u32_at(b, eocd)? == 0x0605_4b50,
        "no end-of-directory record (or a zip comment)"
    );
    let count = u16_at(b, eocd + 10)?;
    let mut at = usize::try_from(u32_at(b, eocd + 16)?)?;
    let mut out = Vec::new();
    for _ in 0..count {
        ensure!(
            u32_at(b, at)? == 0x0201_4b50,
            "bad central directory record"
        );
        ensure!(u16_at(b, at + 10)? == 0, "an entry is compressed");
        let crc = u32_at(b, at + 16)?;
        let size = usize::try_from(u32_at(b, at + 24)?)?;
        let nlen = u16_at(b, at + 28)?;
        let skip = u16_at(b, at + 30)? + u16_at(b, at + 32)?;
        let local = usize::try_from(u32_at(b, at + 42)?)?;
        let name = b.get(at + 46..at + 46 + nlen).context("zip cut short")?;
        let name = String::from_utf8(name.to_vec()).context("a name is not UTF-8")?;
        let data_at = local + 30 + u16_at(b, local + 26)? + u16_at(b, local + 28)?;
        let data = b
            .get(data_at..data_at + size)
            .context("zip cut short")?
            .to_vec();
        out.push((name, crc, data));
        at += 46 + nlen + skip;
    }
    Ok(out)
}

/// Checks a finished zip: every entry under `top/`, an allowed type, a correct CRC, in sorted
/// order, with the index and at least one example. This is the build's gate against packaging
/// anything that is not an example.
pub fn check(bytes: &[u8], top: &str) -> Result<()> {
    let entries = read_entries(bytes)?;
    let prefix = format!("{top}/");
    let mut previous = "";
    let mut has_example = false;
    let mut has_index = false;
    for (name, crc, data) in &entries {
        let rel = name
            .strip_prefix(&prefix)
            .with_context(|| format!("{name}: outside the folder {top}/"))?;
        check_name(rel)?;
        ensure!(crc32(data) == *crc, "{name}: the CRC-32 does not match");
        ensure!(name.as_str() > previous, "{name}: entries are not sorted");
        previous = name;
        if rel == INDEX_NAME {
            has_index = true;
        } else if rel.contains('/') {
            has_example = true;
        } else {
            bail!("{name}: a file beside the example folders");
        }
    }
    ensure!(has_index, "the zip has no {INDEX_NAME}");
    ensure!(has_example, "the zip holds no example");
    Ok(())
}

/// The command: write the zip to `out` (or only check `check_only`).
pub fn run(
    root: &Path,
    top: &str,
    out: Option<&PathBuf>,
    check_only: Option<&PathBuf>,
) -> Result<()> {
    if let Some(path) = check_only {
        check(
            &fs::read(path).with_context(|| format!("reading {}", path.display()))?,
            top,
        )?;
        println!("{}: ok", path.display());
        return Ok(());
    }
    let out = out.context("give --out <file> or --check <file>")?;
    let bytes = build(root, top)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, &bytes)?;
    println!("wrote {} ({} bytes)", out.display(), bytes.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch folder that is removed again when the test ends.
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    impl std::ops::Deref for Scratch {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    fn scratch(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("xtask-examples-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    /// A fake workspace root holding the given files under testdata/examples/.
    fn tree(name: &str, files: &[(&str, &[u8])]) -> Scratch {
        let root = scratch(name);
        let real = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let notice = root.join(NOTICE_FILE);
        fs::create_dir_all(notice.parent().unwrap()).unwrap();
        fs::copy(real.join(NOTICE_FILE), notice).unwrap();
        for (rel, data) in files {
            let p = root.join(EXAMPLES_DIR).join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, data).unwrap();
        }
        root
    }

    const GOOD: &[(&str, &[u8])] = &[
        ("zeta/README.md", b"# Zeta part\nbody"),
        ("alpha/README.md", b"# Alpha part\n"),
        ("alpha/model.stl", b"solid x\nendsolid x\n"),
        ("alpha/project/project.toml", b"a = 1\n"),
    ];

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn req_nfr_06_examples_zip_is_byte_identical_and_sorted() {
        let a = build(&tree("det-a", GOOD), "ex").unwrap();
        let b = build(&tree("det-b", GOOD), "ex").unwrap();
        assert_eq!(a, b);
        let names: Vec<String> = read_entries(&a).unwrap().into_iter().map(|e| e.0).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert_eq!(names[0], "ex/EXAMPLES-README.txt");
        assert!(names.contains(&"ex/alpha/project/project.toml".to_string()));
    }

    #[test]
    fn req_nfr_12_examples_index_lists_projects_and_the_verify_notice() {
        let z = build(&tree("index", GOOD), "ex").unwrap();
        let entries = read_entries(&z).unwrap();
        let index = String::from_utf8(entries[0].2.clone()).unwrap();
        assert!(index.contains("alpha/  Alpha part"));
        assert!(index.contains("zeta/  Zeta part"));
        let notice =
            verify_notice(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
        assert!(notice.starts_with("Generated G-code must be verified by the user"));
        assert!(index.contains(&notice));
    }

    #[test]
    fn req_nfr_06_examples_zip_keeps_the_file_bytes() {
        let z = build(&tree("bytes", GOOD), "ex").unwrap();
        let e = read_entries(&z).unwrap();
        let model = e.iter().find(|e| e.0 == "ex/alpha/model.stl").unwrap();
        assert_eq!(model.2, b"solid x\nendsolid x\n");
        assert_eq!(model.1, crc32(&model.2));
    }

    #[test]
    fn examples_zip_refuses_a_file_type_not_on_the_allow_list() {
        for bad in [
            "alpha/key.pem",
            "alpha/job.gcode",
            "alpha/noext",
            "alpha/.env",
        ] {
            let mut files = GOOD.to_vec();
            files.push((bad, b"x"));
            let err = build(&tree("bad", &files), "ex").unwrap_err().to_string();
            assert!(err.contains(bad), "{err}");
        }
    }

    #[test]
    fn examples_zip_refuses_names_a_windows_extractor_could_misread() {
        for bad in [
            "alpha/x\\..\\evil.txt",
            "alpha/a:b.txt",
            "alpha/a\nb.txt",
            "alpha/a b.txt",
        ] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        assert!(check_name("alpha/baluster-project/c0de0001-0000.toml").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn examples_zip_refuses_a_symbolic_link() {
        let root = tree("link", GOOD);
        std::os::unix::fs::symlink("/etc/passwd", root.join(EXAMPLES_DIR).join("alpha/p.txt"))
            .unwrap();
        assert!(build(&root, "ex").unwrap_err().to_string().contains("link"));
    }

    #[test]
    fn examples_zip_check_refuses_entries_outside_the_examples_folder() {
        let entries = vec![
            Entry {
                name: "ex/EXAMPLES-README.txt".into(),
                data: b"i".to_vec(),
            },
            Entry {
                name: "ex/a/m.stl".into(),
                data: b"s".to_vec(),
            },
            Entry {
                name: "other/secret.toml".into(),
                data: b"s".to_vec(),
            },
        ];
        let err = check(&write_zip(&entries).unwrap(), "ex")
            .unwrap_err()
            .to_string();
        assert!(err.contains("outside"), "{err}");
        let entries = vec![
            Entry {
                name: "ex/EXAMPLES-README.txt".into(),
                data: b"i".to_vec(),
            },
            Entry {
                name: "ex/a/key.pem".into(),
                data: b"s".to_vec(),
            },
        ];
        assert!(check(&write_zip(&entries).unwrap(), "ex").is_err());
        let entries = vec![
            Entry {
                name: "ex/EXAMPLES-README.txt".into(),
                data: b"i".to_vec(),
            },
            Entry {
                name: "ex/../x/m.stl".into(),
                data: b"s".to_vec(),
            },
        ];
        assert!(check(&write_zip(&entries).unwrap(), "ex").is_err());
    }

    #[test]
    fn examples_zip_check_detects_a_damaged_file() {
        let mut z = build(&tree("damage", GOOD), "ex").unwrap();
        let at = z.windows(5).position(|w| w == b"solid").unwrap();
        z[at] = b'X';
        assert!(check(&z, "ex").unwrap_err().to_string().contains("CRC"));
    }

    #[test]
    fn examples_zip_needs_an_example() {
        let root = scratch("empty");
        fs::create_dir_all(root.join(EXAMPLES_DIR)).unwrap();
        assert!(build(&root, "ex").is_err());
    }

    #[test]
    fn the_real_examples_pack_and_unzip_cleanly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let z = build(root, "openchuckcam-examples-test").unwrap();
        check(&z, "openchuckcam-examples-test").unwrap();
    }
}
