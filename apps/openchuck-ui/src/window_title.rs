// SPDX-License-Identifier: AGPL-3.0-or-later
//! The text of the operating-system window title: the application, the
//! project's name with a "*" for unsaved changes, and the project's folder
//! (#218, decision record 0165). The path stays in the window title, the
//! title-bar tooltip and the About window; it is never written to the G-code
//! or the setup sheet.

/// The application's name, the whole title when no project is open.
pub const APP_NAME: &str = "OpenChuckCAM";

/// The longest folder path, in characters, the title shows before it is
/// shortened in the middle.
pub const MAX_PATH_CHARS: usize = 70;

/// The window title for a project called `name` in `path`, or the
/// application's name alone when there is no project.
pub fn title(project: Option<(&str, bool, &str)>) -> String {
    match project {
        None => APP_NAME.to_owned(),
        Some((name, modified, path)) => {
            let mark = if modified { "*" } else { "" };
            format!(
                "{APP_NAME} - {name}{mark} - {}",
                shorten_middle(path, MAX_PATH_CHARS)
            )
        }
    }
}

/// The save-state marker of the in-app title bar and its tooltip. Nothing
/// saves the project by itself, so unsaved changes say the file on disk is
/// unchanged (FIRSTRUN item 4, the owner's answer).
pub fn save_state(modified: bool) -> (&'static str, Option<&'static str>) {
    if modified {
        (
            "Edited, not saved",
            Some(
                "Changes are kept in memory only. File > Save (Ctrl S) writes them to the \
                 project.",
            ),
        )
    } else {
        ("\u{b7} saved", None)
    }
}

/// `path` as it is if it fits in `max` characters; otherwise its first part
/// (the drive or the root), "…" and as many of its last parts as fit, so the
/// project's own folder always shows.
pub fn shorten_middle(path: &str, max: usize) -> String {
    if path.chars().count() <= max {
        return path.to_owned();
    }
    let separator = if path.contains('\\') { '\\' } else { '/' };
    let parts: Vec<&str> = path.split(separator).collect();
    let Some((first, rest)) = parts.split_first() else {
        return path.to_owned();
    };
    if parts.len() < 3 {
        return path.to_owned();
    }
    let join = |tail: usize| {
        let (_, kept) = rest.split_at(rest.len() - tail);
        format!(
            "{first}{separator}…{separator}{}",
            kept.join(&separator.to_string())
        )
    };
    let mut best = join(1);
    for tail in 2..rest.len() {
        let candidate = join(tail);
        if candidate.chars().count() > max {
            break;
        }
        best = candidate;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_prj_08_req_ui_03_unsaved_changes_say_they_are_not_saved() {
        let (text, tip) = save_state(true);
        assert_eq!(text, "Edited, not saved");
        let tip = tip.unwrap();
        assert!(tip.contains("memory only") && tip.contains("File > Save (Ctrl S)"));
        assert_eq!(save_state(false), ("\u{b7} saved", None));
    }

    #[test]
    fn req_ui_01_no_project_shows_the_application_name_only() {
        assert_eq!(title(None), "OpenChuckCAM");
    }

    #[test]
    fn req_ui_01_the_title_names_the_project_the_path_and_unsaved_changes() {
        assert_eq!(
            title(Some(("Blisk", false, "C:\\Users\\paul\\Blisk"))),
            "OpenChuckCAM - Blisk - C:\\Users\\paul\\Blisk"
        );
        assert_eq!(
            title(Some(("Blisk", true, "/home/paul/Blisk"))),
            "OpenChuckCAM - Blisk* - /home/paul/Blisk"
        );
    }

    #[test]
    fn req_ui_01_a_long_path_is_shortened_in_the_middle_keeping_drive_and_folder() {
        let path = "C:\\Users\\paul_\\OneDrive\\Documents\\Work\\Machining\\Jobs\\2026\\Blisk";
        let short = shorten_middle(path, 40);
        assert!(short.starts_with("C:\\…\\"), "{short}");
        assert!(short.ends_with("\\Blisk"), "{short}");
        assert!(short.chars().count() <= 40, "{short}");
        assert!(short.chars().count() < path.chars().count());
        // Forward slashes work the same.
        let unix = shorten_middle("/home/paul/a/very/long/path/to/the/Blisk", 24);
        assert!(
            unix.starts_with("/…/") && unix.ends_with("/Blisk"),
            "{unix}"
        );
        // A path that fits, or has too few parts, is unchanged.
        assert_eq!(shorten_middle("C:\\a\\b", 40), "C:\\a\\b");
        assert_eq!(
            shorten_middle("C:\\averyveryverylongfolder", 5),
            "C:\\averyveryverylongfolder"
        );
    }

    #[test]
    fn req_ui_01_the_folder_always_shows_even_when_it_alone_is_too_long() {
        let short = shorten_middle("D:\\x\\y\\z\\ProjectWithAVeryLongFolderName", 10);
        assert_eq!(short, "D:\\…\\ProjectWithAVeryLongFolderName");
    }
}
