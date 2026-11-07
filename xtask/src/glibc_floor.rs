// SPDX-License-Identifier: AGPL-3.0-or-later
//! `cargo xtask glibc-floor`: the Linux archive's binaries need nothing newer than the stated
//! floor (architecture section 12; decision 0031 item 5; decision 0541; OI-301).
//!
//! A Linux program built against one glibc refuses to start on a system whose glibc lacks a
//! symbol version it was linked against ("version `GLIBC_2.43' not found"). This check reads each
//! binary's version-needs table (the ELF section `.gnu.version_r`, `SHT_GNU_verneed`) itself, so no
//! dependency or system tool is needed, and fails if any needed version is newer than the floor
//! for its family (`GLIBC_`, `GLIBCXX_`, `CXXABI_`, `GCC_`). A family the binary needs with no
//! floor given is a finding too, so nothing goes unchecked. The floors are stated once, in
//! `build-archives.yml`, which also writes them into the archive's README.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

use crate::Findings;

const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;

/// One version a binary needs: the library (`libc.so.6`) and the version name (`GLIBC_2.35`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Need {
    pub file: String,
    pub version: String,
}

/// `a + b`, as an error rather than an overflow when a damaged file gives huge offsets.
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .context("an ELF offset lies outside the file")
}

fn bytes_at(data: &[u8], at: usize, len: usize) -> Result<&[u8]> {
    data.get(at..add(at, len)?)
        .context("the ELF file ends inside a header")
}

fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    let bytes: [u8; 2] = bytes_at(data, at, 2)?
        .try_into()
        .context("the ELF file ends inside a header")?;
    Ok(u16::from_le_bytes(bytes))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    let bytes: [u8; 4] = bytes_at(data, at, 4)?
        .try_into()
        .context("the ELF file ends inside a header")?;
    Ok(u32::from_le_bytes(bytes))
}

fn u64_at(data: &[u8], at: usize) -> Result<usize> {
    let bytes: [u8; 8] = bytes_at(data, at, 8)?
        .try_into()
        .context("the ELF file ends inside a header")?;
    usize::try_from(u64::from_le_bytes(bytes)).context("an ELF offset does not fit in memory")
}

fn c_string(data: &[u8], at: usize) -> Result<String> {
    let rest = data
        .get(at..)
        .context("an ELF string lies outside the file")?;
    let end = rest
        .iter()
        .position(|&b| b == 0)
        .context("an ELF string has no end")?;
    Ok(String::from_utf8_lossy(rest.get(..end).unwrap_or_default()).into_owned())
}

/// The versions an ELF file needs, from its `.gnu.version_r` section. Only 64-bit little-endian
/// ELF is read (the x86-64 Linux archive); anything else is an error. A file with no
/// version-needs section needs none.
pub fn needs(data: &[u8]) -> Result<Vec<Need>> {
    ensure!(data.get(..4) == Some(b"\x7fELF"), "not an ELF file");
    ensure!(
        data.get(4) == Some(&2) && data.get(5) == Some(&1),
        "not a 64-bit little-endian ELF file"
    );
    let shoff = u64_at(data, 0x28)?;
    let shentsize = usize::from(u16_at(data, 0x3a)?);
    let shnum = usize::from(u16_at(data, 0x3c)?);
    ensure!(
        shnum == 0 || shentsize >= 64,
        "the ELF section headers are too small"
    );
    let section = |index: usize| -> Result<usize> {
        ensure!(index < shnum, "an ELF section index is out of range");
        add(shoff, index * shentsize)
    };
    let mut out = Vec::new();
    for index in 0..shnum {
        let header = section(index)?;
        // Every section header must lie inside the file.
        bytes_at(data, header, 64)?;
        if u32_at(data, header + 4)? != SHT_GNU_VERNEED {
            continue;
        }
        let offset = u64_at(data, header + 24)?;
        let size = u64_at(data, header + 32)?;
        let strtab = section(u32_at(data, header + 40)? as usize)?;
        let strings = u64_at(data, add(strtab, 24)?)?;
        let count = u32_at(data, header + 44)?;
        ensure!(
            offset
                .checked_add(size)
                .is_some_and(|end| end <= data.len()),
            "the ELF version-needs section lies outside the file"
        );
        let mut entry = offset;
        for _ in 0..count {
            let aux_count = u16_at(data, add(entry, 2)?)?;
            let file = c_string(data, add(strings, u32_at(data, add(entry, 4)?)? as usize)?)?;
            let mut aux = add(entry, u32_at(data, add(entry, 8)?)? as usize)?;
            for _ in 0..aux_count {
                let version = c_string(data, add(strings, u32_at(data, add(aux, 8)?)? as usize)?)?;
                out.push(Need {
                    file: file.clone(),
                    version,
                });
                let next = u32_at(data, add(aux, 12)?)? as usize;
                if next == 0 {
                    break;
                }
                aux = add(aux, next)?;
            }
            let next = u32_at(data, add(entry, 12)?)? as usize;
            if next == 0 {
                break;
            }
            entry = add(entry, next)?;
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// A version name split into its family and numbers: `GLIBC_2.35` is `("GLIBC", [2, 35])`.
/// `None` for a name with no numeric part, such as `GLIBC_PRIVATE`.
pub fn split(version: &str) -> Option<(&str, Vec<u32>)> {
    let (family, number) = version.rsplit_once('_')?;
    let numbers: Option<Vec<u32>> = number.split('.').map(|n| n.parse().ok()).collect();
    numbers.map(|n| (family, n))
}

/// Parses floors given as `FAMILY=x.y[.z]`, for example `GLIBC=2.35`.
pub fn parse_floors(floors: &[String]) -> Result<BTreeMap<String, Vec<u32>>> {
    let mut out = BTreeMap::new();
    for floor in floors {
        let Some((family, version)) = floor.split_once('=') else {
            bail!("the floor `{floor}` is not FAMILY=version, for example GLIBC=2.35");
        };
        let Some((_, numbers)) = split(&format!("{family}_{version}")) else {
            bail!("the floor `{floor}` has no numeric version");
        };
        ensure!(!family.is_empty(), "the floor `{floor}` names no family");
        out.insert(family.to_owned(), numbers);
    }
    ensure!(!out.is_empty(), "no floor given");
    Ok(out)
}

/// Findings for one binary's needs against the floors.
pub fn check_needs(name: &str, needs: &[Need], floors: &BTreeMap<String, Vec<u32>>) -> Findings {
    let mut findings = Findings::new();
    for need in needs {
        let Some((family, numbers)) = split(&need.version) else {
            findings.push(format!(
                "{name} needs {} from {}, which is not a numbered version",
                need.version, need.file
            ));
            continue;
        };
        match floors.get(family) {
            None => findings.push(format!(
                "{name} needs {} from {}, and no {family} floor is stated",
                need.version, need.file
            )),
            Some(floor) if numbers > *floor => findings.push(format!(
                "{name} needs {} from {}, newer than the floor {family}_{}",
                need.version,
                need.file,
                floor
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
            )),
            Some(_) => {}
        }
    }
    findings
}

/// Checks every binary; prints the newest version each needs per family.
pub fn check(binaries: &[impl AsRef<Path>], floors: &[String]) -> Result<Findings> {
    let floors = parse_floors(floors)?;
    ensure!(!binaries.is_empty(), "no binary given");
    let mut findings = Findings::new();
    for binary in binaries {
        let path = binary.as_ref();
        let data = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let needs = needs(&data).with_context(|| format!("reading {}", path.display()))?;
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let mut newest: BTreeMap<&str, (Vec<u32>, &str)> = BTreeMap::new();
        for need in &needs {
            if let Some((family, numbers)) = split(&need.version)
                && newest.get(family).is_none_or(|(n, _)| numbers > *n)
            {
                newest.insert(family, (numbers, &need.version));
            }
        }
        let summary: Vec<&str> = newest.values().map(|(_, v)| *v).collect();
        println!("{name} needs at most: {}", summary.join(", "));
        findings.extend(check_needs(&name, &needs, &floors));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floors(list: &[&str]) -> BTreeMap<String, Vec<u32>> {
        parse_floors(&list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>()).expect("floors")
    }

    fn need(file: &str, version: &str) -> Need {
        Need {
            file: file.into(),
            version: version.into(),
        }
    }

    /// A minimal 64-bit little-endian ELF file: a null section, a string table and one
    /// version-needs section listing `libs` (each library with its versions).
    fn elf(libs: &[(&str, &[&str])]) -> Vec<u8> {
        let mut strings = vec![0u8];
        let mut add = |s: &str| {
            let at = strings.len() as u32;
            strings.extend_from_slice(s.as_bytes());
            strings.push(0);
            at
        };
        let mut verneed = Vec::new();
        for (i, (file, versions)) in libs.iter().enumerate() {
            let file_at = add(file);
            let aux_at: Vec<u32> = versions.iter().map(|v| add(v)).collect();
            let size = 16 + 16 * versions.len() as u32;
            verneed.extend_from_slice(&1u16.to_le_bytes());
            verneed.extend_from_slice(&(versions.len() as u16).to_le_bytes());
            verneed.extend_from_slice(&file_at.to_le_bytes());
            verneed.extend_from_slice(&16u32.to_le_bytes());
            let next = if i + 1 == libs.len() { 0 } else { size };
            verneed.extend_from_slice(&next.to_le_bytes());
            for (j, name) in aux_at.iter().enumerate() {
                verneed.extend_from_slice(&0u32.to_le_bytes());
                verneed.extend_from_slice(&0u16.to_le_bytes());
                verneed.extend_from_slice(&0u16.to_le_bytes());
                verneed.extend_from_slice(&name.to_le_bytes());
                let next: u32 = if j + 1 == aux_at.len() { 0 } else { 16 };
                verneed.extend_from_slice(&next.to_le_bytes());
            }
        }
        let strings_at = 64usize;
        let verneed_at = strings_at + strings.len();
        let shoff = verneed_at + verneed.len();
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(b"\x7fELF");
        data[4] = 2;
        data[5] = 1;
        data[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes());
        data[0x3a..0x3c].copy_from_slice(&64u16.to_le_bytes());
        data[0x3c..0x3e].copy_from_slice(&3u16.to_le_bytes());
        data.extend_from_slice(&strings);
        data.extend_from_slice(&verneed);
        let header = |kind: u32, offset: usize, size: usize, link: u32, info: u32| {
            let mut h = vec![0u8; 64];
            h[4..8].copy_from_slice(&kind.to_le_bytes());
            h[24..32].copy_from_slice(&(offset as u64).to_le_bytes());
            h[32..40].copy_from_slice(&(size as u64).to_le_bytes());
            h[40..44].copy_from_slice(&link.to_le_bytes());
            h[44..48].copy_from_slice(&info.to_le_bytes());
            h
        };
        data.extend(header(0, 0, 0, 0, 0));
        data.extend(header(3, strings_at, strings.len(), 0, 0));
        data.extend(header(
            SHT_GNU_VERNEED,
            verneed_at,
            verneed.len(),
            1,
            libs.len() as u32,
        ));
        data
    }

    #[test]
    fn req_nfr_02_the_version_needs_table_is_read() {
        let data = elf(&[
            ("libc.so.6", &["GLIBC_2.2.5", "GLIBC_2.43"]),
            ("libstdc++.so.6", &["GLIBCXX_3.4.29", "CXXABI_1.3.15"]),
        ]);
        assert_eq!(
            needs(&data).expect("needs"),
            [
                need("libc.so.6", "GLIBC_2.2.5"),
                need("libc.so.6", "GLIBC_2.43"),
                need("libstdc++.so.6", "CXXABI_1.3.15"),
                need("libstdc++.so.6", "GLIBCXX_3.4.29"),
            ]
        );
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[test]
    fn req_nfr_02_a_real_linux_program_needs_glibc_versions() {
        let exe = std::env::current_exe().expect("this test program");
        let data = fs::read(exe).expect("read");
        let needs = needs(&data).expect("needs");
        assert!(
            needs
                .iter()
                .any(|n| n.file == "libc.so.6" && n.version.starts_with("GLIBC_2.")),
            "{needs:?}"
        );
    }

    #[test]
    fn req_nfr_02_a_version_newer_than_the_floor_is_found() {
        let floors = floors(&["GLIBC=2.35", "GLIBCXX=3.4.30", "CXXABI=1.3.13", "GCC=7.0.0"]);
        // The nightly build of 7 Nov 2026 on Ubuntu 26.04 needed GLIBC_2.43 (decision 0541).
        let found = check_needs(
            "openchuck-ui",
            &[
                need("libc.so.6", "GLIBC_2.2.5"),
                need("libm.so.6", "GLIBC_2.43"),
                need("libgcc_s.so.1", "GCC_4.2.0"),
            ],
            &floors,
        );
        assert_eq!(
            found,
            ["openchuck-ui needs GLIBC_2.43 from libm.so.6, newer than the floor GLIBC_2.35"]
        );
        let found = check_needs(
            "openchuck-step-worker",
            &[
                need("libc.so.6", "GLIBC_2.35"),
                need("libstdc++.so.6", "GLIBCXX_3.4.30"),
                need("libstdc++.so.6", "CXXABI_1.3.15"),
            ],
            &floors,
        );
        assert_eq!(
            found,
            [
                "openchuck-step-worker needs CXXABI_1.3.15 from libstdc++.so.6, newer than the floor CXXABI_1.3.13"
            ]
        );
    }

    #[test]
    fn req_nfr_02_versions_compare_as_numbers_not_text() {
        let floors = floors(&["GLIBC=2.9"]);
        assert!(check_needs("p", &[need("libc.so.6", "GLIBC_2.10")], &floors).len() == 1);
        let floors = super::tests::floors(&["GLIBC=2.35"]);
        assert!(check_needs("p", &[need("libc.so.6", "GLIBC_2.4")], &floors).is_empty());
        assert!(check_needs("p", &[need("libc.so.6", "GLIBC_2.35")], &floors).is_empty());
        assert!(check_needs("p", &[need("libc.so.6", "GLIBC_2.35.1")], &floors).len() == 1);
    }

    #[test]
    fn req_nfr_02_a_family_with_no_floor_or_no_number_is_found() {
        let floors = floors(&["GLIBC=2.35"]);
        let found = check_needs(
            "p",
            &[
                need("libstdc++.so.6", "GLIBCXX_3.4.30"),
                need("libc.so.6", "GLIBC_PRIVATE"),
            ],
            &floors,
        );
        assert_eq!(
            found,
            [
                "p needs GLIBCXX_3.4.30 from libstdc++.so.6, and no GLIBCXX floor is stated",
                "p needs GLIBC_PRIVATE from libc.so.6, which is not a numbered version",
            ]
        );
    }

    #[test]
    fn req_nfr_07_bad_floors_and_files_are_clear_errors() {
        for bad in ["GLIBC", "GLIBC=two", "=2.35", "GLIBC=2..3"] {
            assert!(parse_floors(&[bad.to_owned()]).is_err(), "{bad}");
        }
        assert!(parse_floors(&[]).is_err());
        assert!(
            needs(b"#!/bin/sh\n")
                .expect_err("script")
                .to_string()
                .contains("not an ELF")
        );
        let mut data = elf(&[("libc.so.6", &["GLIBC_2.35"])]);
        data[4] = 1;
        assert!(
            needs(&data)
                .expect_err("32-bit")
                .to_string()
                .contains("64-bit")
        );
        let data = elf(&[("libc.so.6", &["GLIBC_2.35"])]);
        let cut = &data[..data.len() - 10];
        assert!(
            needs(cut).is_err(),
            "a truncated file is an error, not a panic"
        );
        // Every truncation is an error or an answer, never a panic.
        for len in 0..data.len() {
            let _ = needs(&data[..len]);
        }
    }

    #[test]
    fn req_nfr_02_a_file_with_no_version_needs_needs_nothing() {
        let data = elf(&[]);
        assert!(needs(&data).expect("needs").is_empty());
    }
}
