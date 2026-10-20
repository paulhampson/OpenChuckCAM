// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-user settings (UI-15; specification section 13; verification plan
//! row UI-15; decision record 0117).

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::path::PathBuf;

use openchuck_app::settings::{SettingsFile, ThemeChoice, UserSettings};
use openchuck_app::{Acknowledgements, Application, Threads};

fn folder(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("openchuck-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn req_ui_15_settings_round_trip_in_the_configuration_folder() {
    let config = folder("round-trip");
    let file = SettingsFile::in_folder(&config);
    // No file yet: the defaults, and no message.
    let loaded = file.load();
    assert_eq!(loaded.settings, UserSettings::default());
    assert!(loaded.problem.is_none());
    assert_eq!(loaded.settings.scale, 1.0);

    let mut settings = UserSettings {
        scale: 0.9,
        ..UserSettings::default()
    };
    settings.project_panel.size = 300.0;
    settings.window = Some([1366.0, 768.0]);
    settings.inspector.collapsed = true;
    settings.dock.size = 240.0;
    settings.opened(&config);
    settings.set_section_open("setup", "workholding", false, &["machine", "workholding"]);
    file.save(&settings).unwrap();
    let loaded = file.load();
    assert!(loaded.problem.is_none());
    assert_eq!(loaded.settings, settings);
    assert!(loaded.settings.section_open("setup", "machine", false));
    assert!(!loaded.settings.section_open("setup", "workholding", true));
    assert!(loaded.settings.section_open("operation", "geometry", true));
    let mut reopened = loaded.settings.clone();
    reopened.set_section_open("setup", "workholding", true, &[]);
    assert!(reopened.section_open("setup", "workholding", false));

    // The export acknowledgements in the same file are kept, both ways.
    let project = folder("round-trip-project");
    std::fs::create_dir_all(&project).unwrap();
    let acks = Acknowledgements::in_folder(&config);
    acks.acknowledge(&project).unwrap();
    file.save(&settings).unwrap();
    assert!(acks.lookup(&project).acknowledged);
    assert_eq!(file.load().settings, settings);
    let _ = std::fs::remove_dir_all(config);
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn req_ui_15_a_damaged_file_gives_defaults_and_a_message() {
    let config = folder("damaged");
    std::fs::create_dir_all(&config).unwrap();
    let file = SettingsFile::in_folder(&config);
    std::fs::write(file.file(), "this is [not toml").unwrap();
    let loaded = file.load();
    assert_eq!(loaded.settings, UserSettings::default());
    let problem = loaded.problem.unwrap();
    assert!(problem.contains("could not be read"), "{problem}");
    // Saving keeps the damaged file aside.
    file.save(&UserSettings::default()).unwrap();
    assert!(config.join("settings.toml.bad").exists());
    assert!(file.load().problem.is_none());

    // A table of the wrong shape: defaults and a message.
    std::fs::write(file.file(), "[interface]\nscale = \"big\"\n").unwrap();
    let loaded = file.load();
    assert_eq!(loaded.settings, UserSettings::default());
    assert!(loaded.problem.is_some());

    // Values out of range are brought into range.
    std::fs::write(
        file.file(),
        "[interface]\nscale = 9.0\ntheme = \"light\"\n[interface.project_panel]\nsize = 5.0\n",
    )
    .unwrap();
    let loaded = file.load();
    assert!(loaded.problem.is_none());
    assert_eq!(loaded.settings.scale, 2.0);
    assert_eq!(loaded.settings.theme, ThemeChoice::Light);
    assert_eq!(loaded.settings.project_panel.size, 200.0);
    assert_eq!(loaded.settings.inspector.size, 340.0);
    // A window size that cannot be right is forgotten.
    std::fs::write(file.file(), "[interface]\nwindow = [10.0, 99999.0]\n").unwrap();
    assert_eq!(file.load().settings.window, None);
    let _ = std::fs::remove_dir_all(config);
}

#[test]
fn req_ui_15_nothing_is_written_to_the_project_folder() {
    let config = folder("nothing-config");
    let project = folder("nothing-project");
    let app = Application::create(&project, "Settings", Threads::Exactly(1)).unwrap();
    app.save().unwrap();
    let listing = |dir: &PathBuf| {
        let mut names: Vec<_> = walk(dir);
        names.sort();
        names
    };
    let before = listing(&project);
    let mut settings = UserSettings {
        scale: 1.5,
        ..UserSettings::default()
    };
    settings.opened(app.location());
    SettingsFile::in_folder(&config).save(&settings).unwrap();
    assert_eq!(listing(&project), before);
    assert!(!app.is_modified());
    let _ = std::fs::remove_dir_all(config);
    let _ = std::fs::remove_dir_all(project);
}

fn walk(dir: &PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        }
        out.push(path);
    }
    out
}
