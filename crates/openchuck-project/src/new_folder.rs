// SPDX-License-Identifier: AGPL-3.0-or-later
//! The folder a new project is created in (#219, decision record 0165): a
//! folder named after the project, made inside a parent folder the user
//! picks. The name is cleaned so every operating system accepts it, and a
//! folder that already exists is never written over (NFR-07): saving a
//! project removes files it does not own in its own places (`machine.toml`,
//! the item folders), so a folder that holds any of them is refused.

use std::path::{Path, PathBuf};

use crate::file::{FileKind, MACHINE_FILE, PROJECT_FILE};

/// The longest folder name, in characters, that is made. Operating systems
/// allow 255 bytes per name, but Windows limits a whole path to about 260
/// characters, so a shorter name leaves room for the folders round it.
pub const MAX_NAME_CHARS: usize = 100;

/// The name used when nothing of the project's name is left after cleaning.
pub const FALLBACK_NAME: &str = "Untitled";

/// Characters no common file system allows in a name.
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Windows device names, which cannot name a file, with or without an
/// extension, whatever their case.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
    "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³", "CONIN$", "CONOUT$",
];

/// Characters that show nothing but change what a name looks like or how
/// text runs: zero-width and joiner characters, bidirectional controls, the
/// soft hyphen, the byte-order mark and the invisible operators.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

/// The folder name for a project called `name`: the same on every platform
/// (NFR-02), so a project folder moves between them.
///
/// - Characters no system allows (`< > : " / \ | ? *` and control characters)
///   become `_`.
/// - Invisible and look-alike spacing characters are removed or become an
///   ordinary space.
/// - Leading and trailing spaces and dots go (Windows drops a trailing one;
///   a leading dot hides the folder; `.` and `..` are not names).
/// - A Windows device name (`CON`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`,
///   with or without an extension) gets a `_` in front.
/// - At most [`MAX_NAME_CHARS`] characters are kept.
/// - Nothing left gives [`FALLBACK_NAME`].
pub fn folder_name(name: &str) -> String {
    let mut cleaned = String::with_capacity(name.len());
    for c in name.chars() {
        if is_invisible(c) {
            continue;
        }
        if c.is_control() || FORBIDDEN.contains(&c) {
            cleaned.push('_');
        } else if c.is_whitespace() {
            cleaned.push(' ');
        } else {
            cleaned.push(c);
        }
    }
    let trim = |s: &str| s.trim_matches(|c: char| c == ' ' || c == '.').to_owned();
    let mut cleaned = trim(&cleaned);
    if cleaned.chars().count() > MAX_NAME_CHARS {
        cleaned = trim(&cleaned.chars().take(MAX_NAME_CHARS).collect::<String>());
    }
    if cleaned.is_empty() {
        return FALLBACK_NAME.to_owned();
    }
    let stem = cleaned.split('.').next().unwrap_or_default().trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        cleaned.insert(0, '_');
    }
    cleaned
}

/// What is at the place a new project's folder would be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderState {
    /// The parent folder does not exist or is not a folder.
    NoParent,
    /// Nothing is there: the folder can be made.
    Free,
    /// A folder is there and holds none of the names a project owns: it can
    /// be used, and its other files stay as they are.
    ExistsUsable,
    /// A folder is there and holds names a project owns (`project.toml`,
    /// `machine.toml`, an item folder): saving there could overwrite or
    /// delete them, so it is refused. The names are listed.
    ExistsWithProject(Vec<String>),
    /// A file is there, not a folder.
    ExistsAsFile,
}

/// The folder a new project would get, and what is there now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFolder {
    /// The cleaned folder name the dialog shows.
    pub name: String,
    /// `parent` joined with [`Self::name`].
    pub path: PathBuf,
    /// What is at `path` now.
    pub state: FolderState,
}

/// The names a project owns inside its folder.
fn owned_names() -> Vec<&'static str> {
    let mut names = vec![PROJECT_FILE, MACHINE_FILE];
    for kind in [
        FileKind::Setup,
        FileKind::Tool,
        FileKind::SelectionSet,
        FileKind::Operation,
    ] {
        names.extend(kind.folder());
    }
    names
}

/// The names a project owns that are in `folder`.
pub fn owned_names_in(folder: &Path) -> Vec<String> {
    owned_names()
        .into_iter()
        .filter(|n| folder.join(n).symlink_metadata().is_ok())
        .map(str::to_owned)
        .collect()
}

/// Looks at `parent` for a new project called `project_name`. Nothing is
/// created.
pub fn inspect(parent: &Path, project_name: &str) -> NewFolder {
    let name = folder_name(project_name);
    let path = parent.join(&name);
    let state = if !parent.is_dir() {
        FolderState::NoParent
    } else {
        match path.symlink_metadata() {
            Err(_) => FolderState::Free,
            Ok(meta) if meta.is_dir() => {
                let owned = owned_names_in(&path);
                if owned.is_empty() {
                    FolderState::ExistsUsable
                } else {
                    FolderState::ExistsWithProject(owned)
                }
            }
            Ok(_) => FolderState::ExistsAsFile,
        }
    };
    NewFolder { name, path, state }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_prj_01_forbidden_characters_become_underscores() {
        assert_eq!(folder_name("a<b>c:d\"e/f\\g|h?i*j"), "a_b_c_d_e_f_g_h_i_j");
        assert_eq!(folder_name("tab\there\u{7f}\u{85}"), "tab_here__");
        assert_eq!(folder_name("Blisk"), "Blisk");
        assert_eq!(folder_name("Mount 2 (v3) - ok"), "Mount 2 (v3) - ok");
    }

    #[test]
    fn req_prj_01_reserved_windows_names_are_prefixed_with_or_without_extension() {
        for name in [
            "CON",
            "con",
            "Prn",
            "AUX",
            "nul",
            "COM1",
            "com9",
            "LPT1",
            "lpt9",
            "COM¹",
            "LPT³",
            "CON.txt",
            "nul.tar.gz",
            "Com3 .x",
        ] {
            assert!(folder_name(name).starts_with('_'), "{name}");
        }
        // Names that only start like a device name are fine.
        for name in ["CONSOLE", "COM10", "COMX", "LPT", "AUXILIARY", "NULL"] {
            assert_eq!(folder_name(name), name, "{name}");
        }
    }

    #[test]
    fn req_prj_01_trailing_and_leading_dots_and_spaces_go() {
        assert_eq!(folder_name("Blisk. . "), "Blisk");
        assert_eq!(folder_name("  Blisk"), "Blisk");
        assert_eq!(folder_name(".hidden"), "hidden");
        assert_eq!(folder_name(".."), FALLBACK_NAME);
        assert_eq!(folder_name("."), FALLBACK_NAME);
        assert_eq!(folder_name("a.b"), "a.b");
    }

    #[test]
    fn req_prj_01_an_empty_result_gets_the_fallback_name() {
        for name in ["", "   ", "...", "\u{200B}\u{FEFF}", " . "] {
            assert_eq!(folder_name(name), FALLBACK_NAME, "{name:?}");
        }
    }

    #[test]
    fn req_prj_01_invisible_characters_are_removed_and_odd_spaces_normalised() {
        assert_eq!(folder_name("Bl\u{200B}isk\u{202E}"), "Blisk");
        assert_eq!(folder_name("A\u{00A0}B\u{3000}C\u{2003}D"), "A B C D");
        assert_eq!(folder_name("soft\u{00AD}hyphen"), "softhyphen");
        // A reserved name hidden behind an invisible character is still caught.
        assert_eq!(folder_name("C\u{200B}ON"), "_CON");
    }

    #[test]
    fn req_prj_01_unicode_text_is_kept() {
        assert_eq!(folder_name("Schaufelrad Größe 3"), "Schaufelrad Größe 3");
        assert_eq!(folder_name("叶轮 – 2"), "叶轮 – 2");
        assert_eq!(folder_name("pièce 🛠"), "pièce 🛠");
    }

    #[test]
    fn req_prj_01_a_long_name_is_cut_on_a_character_boundary() {
        let long = "é".repeat(300);
        let name = folder_name(&long);
        assert_eq!(name.chars().count(), MAX_NAME_CHARS);
        // The cut is cleaned again: no trailing space or dot is left.
        let spaced = format!("{} .", "a".repeat(MAX_NAME_CHARS - 1));
        assert_eq!(folder_name(&spaced), "a".repeat(MAX_NAME_CHARS - 1));
    }

    #[test]
    fn req_prj_01_the_name_is_the_same_on_every_platform() {
        // The rules do not look at the platform (NFR-02).
        assert_eq!(folder_name("a:b"), "a_b");
        assert_eq!(folder_name("x\\y"), "x_y");
    }

    /// A fresh empty folder for one test.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oc-newfolder-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn req_prj_01_nothing_there_means_free() {
        let parent = scratch("free");
        let found = inspect(&parent, "Blisk: 2");
        assert_eq!(found.name, "Blisk_ 2");
        assert_eq!(found.path, parent.join("Blisk_ 2"));
        assert_eq!(found.state, FolderState::Free);
        assert!(!found.path.exists(), "inspect creates nothing");
        std::fs::remove_dir_all(&parent).unwrap();
    }

    #[test]
    fn req_nfr_07_a_missing_parent_is_reported() {
        let parent = scratch("noparent").join("missing");
        assert_eq!(inspect(&parent, "x").state, FolderState::NoParent);
    }

    #[test]
    fn req_nfr_07_an_existing_empty_folder_is_usable_and_a_file_is_not() {
        let parent = scratch("exists");
        std::fs::create_dir(parent.join("Blisk")).unwrap();
        assert_eq!(inspect(&parent, "Blisk").state, FolderState::ExistsUsable);
        // Other files in it do not stop its use, and are not touched.
        std::fs::write(parent.join("Blisk").join("part.step"), "x").unwrap();
        assert_eq!(inspect(&parent, "Blisk").state, FolderState::ExistsUsable);
        std::fs::write(parent.join("Gear"), "x").unwrap();
        assert_eq!(inspect(&parent, "Gear").state, FolderState::ExistsAsFile);
        std::fs::remove_dir_all(&parent).unwrap();
    }

    #[test]
    fn req_nfr_07_a_folder_with_project_names_is_refused() {
        let parent = scratch("project");
        let folder = parent.join("Blisk");
        std::fs::create_dir(&folder).unwrap();
        for name in ["project.toml", "machine.toml"] {
            std::fs::write(folder.join(name), "x").unwrap();
        }
        std::fs::create_dir(folder.join("setups")).unwrap();
        assert_eq!(
            inspect(&parent, "Blisk").state,
            FolderState::ExistsWithProject(vec![
                "project.toml".to_owned(),
                "machine.toml".to_owned(),
                "setups".to_owned()
            ])
        );
        std::fs::remove_dir_all(&parent).unwrap();
    }

    #[test]
    fn req_nfr_07_creating_never_removes_a_file_the_project_would_not_keep() {
        let parent = scratch("create");
        let folder = parent.join("Blisk");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("machine.toml"), "precious").unwrap();
        let err = crate::file::create(&folder, "Blisk").unwrap_err();
        assert!(err.to_string().contains("machine.toml"), "{err}");
        assert_eq!(
            std::fs::read_to_string(folder.join("machine.toml")).unwrap(),
            "precious"
        );
        assert!(!folder.join("project.toml").exists());
        // A usable folder takes the project and keeps its other files.
        let usable = parent.join("Gear");
        std::fs::create_dir(&usable).unwrap();
        std::fs::write(usable.join("part.step"), "keep").unwrap();
        crate::file::create(&usable, "Gear").unwrap();
        assert!(usable.join("project.toml").is_file());
        assert_eq!(
            std::fs::read_to_string(usable.join("part.step")).unwrap(),
            "keep"
        );
        std::fs::remove_dir_all(&parent).unwrap();
    }
}
