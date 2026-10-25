// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-user settings (UI-15; specification section 13; verification plan
//! row UI-15; decision record 0117).

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::path::PathBuf;

use openchuck_app::settings::{SettingsFile, ThemeChoice, UserSettings};
use openchuck_app::{Acknowledgements, Application, Threads};

#[test]
fn req_ui_15_req_prj_08_checkpoint_interval_is_personal_and_survives_restart() {
    let config = folder("checkpoint-interval");
    let file = SettingsFile::in_folder(&config);
    assert_eq!(file.load().settings.checkpoint_minutes, 3);
    let mut settings = UserSettings {
        checkpoint_minutes: 7,
        ..UserSettings::default()
    };
    file.save(&settings).unwrap();
    assert_eq!(file.load().settings.checkpoint_minutes, 7);
    settings.checkpoint_minutes = 0;
    assert_eq!(settings.clone().clamped().checkpoint_minutes, 1);
    settings.checkpoint_minutes = u16::MAX;
    assert_eq!(settings.clamped().checkpoint_minutes, 15);
    std::fs::write(file.file(), "[interface]\nscale = 1.0\n").unwrap();
    assert_eq!(file.load().settings.checkpoint_minutes, 3);
    std::fs::remove_dir_all(config).unwrap();
}

fn folder(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("openchuck-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn req_ui_15_req_sim_05_cut_part_opacity_is_personal_persistent_and_translucent_by_default() {
    let config = folder("cut-part-opacity");
    let file = SettingsFile::in_folder(&config);
    let defaults = toml::Value::try_from(&file.load().settings).unwrap();
    assert_eq!(
        defaults["simulate"]
            .get("cut_part_solid")
            .and_then(toml::Value::as_bool),
        Some(false)
    );
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(file.file(), "[interface.simulate]\ncut_part_solid = true\n").unwrap();
    let loaded = file.load();
    assert!(loaded.problem.is_none());
    file.save(&loaded.settings).unwrap();
    let saved: toml::Value =
        toml::from_str(&std::fs::read_to_string(file.file()).unwrap()).unwrap();
    assert_eq!(
        saved["interface"]["simulate"]["cut_part_solid"].as_bool(),
        Some(true)
    );
    // Files written before the setting existed retain today's look.
    std::fs::write(file.file(), "[interface.simulate]\nshow_cut_part = true\n").unwrap();
    let old = toml::Value::try_from(&file.load().settings).unwrap();
    assert_eq!(old["simulate"]["cut_part_solid"].as_bool(), Some(false));
    std::fs::remove_dir_all(config).unwrap();
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

#[test]
fn req_nfr_18_recording_actions_is_on_by_default_and_the_choice_is_kept() {
    let config = folder("record-actions");
    let file = SettingsFile::in_folder(&config);
    assert!(UserSettings::default().record_actions);
    assert!(file.load().settings.record_actions);
    let off = UserSettings {
        record_actions: false,
        ..UserSettings::default()
    };
    file.save(&off).unwrap();
    assert!(!file.load().settings.record_actions);
    // A file written before the field existed reads as on.
    std::fs::write(file.file(), "[interface]\nscale = 1.0\n").unwrap();
    let loaded = file.load();
    assert!(loaded.problem.is_none());
    assert!(loaded.settings.record_actions);
    let _ = std::fs::remove_dir_all(config);
}

#[test]
fn req_sim_02_turn_part_with_a_is_on_by_default_and_the_choice_is_kept() {
    // Issue 203: on for new and existing projects alike (the choice is the
    // user's, never in a project), and an untick is remembered.
    let config = folder("turn-with-a");
    let file = SettingsFile::in_folder(&config);
    assert!(UserSettings::default().simulate.turn_part_with_a);
    assert!(file.load().settings.simulate.turn_part_with_a);
    // A settings file written before the choice existed reads as on.
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(file.file(), "[interface]\nscale = 1.0\n").unwrap();
    assert!(file.load().settings.simulate.turn_part_with_a);
    let mut settings = file.load().settings;
    settings.simulate.turn_part_with_a = false;
    file.save(&settings).unwrap();
    assert!(!file.load().settings.simulate.turn_part_with_a);
    let _ = std::fs::remove_dir_all(config);
}

#[test]
fn req_sim_03_req_ui_15_the_simulate_view_choices_are_kept_and_never_empty() {
    // Issue 204: "Cut part" only by default; issue 205: the path outside the
    // window is faint unless hidden; issue 214: new projects simulate.
    let config = folder("simulate-view");
    let file = SettingsFile::in_folder(&config);
    let d = UserSettings::default().simulate;
    assert!(d.show_cut_part && !d.show_model && !d.show_toolpath);
    assert!(!d.hide_outside_window && !d.skip_stock_simulation);
    let mut settings = UserSettings::default();
    settings.simulate.show_model = true;
    settings.simulate.show_cut_part = false;
    settings.simulate.show_toolpath = true;
    settings.simulate.hide_outside_window = true;
    settings.simulate.skip_stock_simulation = true;
    file.save(&settings).unwrap();
    assert_eq!(file.load().settings.simulate, settings.simulate);
    // A file with every view box off gives "Cut part": the view is never
    // empty.
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        file.file(),
        "[interface.simulate]\nshow_model = false\nshow_cut_part = false\nshow_toolpath = false\n",
    )
    .unwrap();
    let loaded = file.load();
    assert!(loaded.problem.is_none());
    assert!(loaded.settings.simulate.show_cut_part);
    let _ = std::fs::remove_dir_all(config);
}
