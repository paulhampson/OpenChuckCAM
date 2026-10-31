// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-user settings on the computer, never in the project (UI-15;
//! specification section 13; slice 22 lane X2, decision record 0117).
//!
//! They live in the `[interface]` table of the same `settings.toml` as the
//! export acknowledgements ([`crate::Acknowledgements`]), in the application's
//! configuration folder (decision record 0029 item 15). This module reads and
//! writes only that table and keeps the rest of the file as it was.
//!
//! A missing file gives the defaults. A file or a table that cannot be read
//! gives the defaults and a sentence for the user, never a crash (NFR-07); a
//! value out of range is brought into range.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::acknowledgement::SETTINGS_FILE;
use crate::error::AppError;
use crate::output_names::FileNameTemplates;
use crate::tool_library::ToolLibraryView;

/// The table of `settings.toml` this module owns.
const TABLE: &str = "interface";

/// The interface scale at first start (OI-166, reopened by decision 0148).
pub const DEFAULT_SCALE: f32 = 1.0;
/// The smallest and largest interface scale (specification section 16,
/// NFR-15).
pub const SCALE_RANGE: (f32, f32) = (0.8, 2.0);
/// The smallest and largest window side kept, in logical pixels; a size
/// outside is forgotten.
pub const WINDOW_RANGE: (f32, f32) = (320.0, 16_384.0);
/// How many recent projects are kept.
pub const RECENT_KEPT: usize = 10;

/// The interface's theme (specification section 14.2). The light theme is
/// UI-20, P2; it is stored so that choosing it later needs no new file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// The default.
    #[default]
    Dark,
    /// Light panels; the view stays dark.
    Light,
}

/// A side panel's or the dock's size and whether it is collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelLayout {
    /// The width of a side panel or the height of the dock, in logical
    /// pixels at 100% scale.
    pub size: f32,
    /// Whether it is collapsed to its rail.
    pub collapsed: bool,
}

impl PanelLayout {
    const fn new(size: f32) -> Self {
        Self {
            size,
            collapsed: false,
        }
    }

    fn clamped(self, range: (f32, f32), default: f32) -> Self {
        Self {
            size: if self.size.is_finite() {
                self.size.clamp(range.0, range.1)
            } else {
                default
            },
            collapsed: self.collapsed,
        }
    }
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// The defaults and ranges of the window's regions (specification section 3),
/// as (default, smallest, largest).
pub mod region {
    /// The project panel's width.
    pub const PROJECT: (f32, f32, f32) = (264.0, 200.0, 400.0);
    /// The inspector's width.
    pub const INSPECTOR: (f32, f32, f32) = (340.0, 280.0, 520.0);
    /// The dock's height; its largest is half the window, which the front
    /// end applies.
    pub const DOCK: (f32, f32, f32) = (260.0, 120.0, 2000.0);
}

/// The Simulate step's choices one user keeps (decision records 0168 and
/// 0169). A settings file written before a field existed reads it as its
/// default: the table is additive and the file has no version number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SimulateSettings {
    /// Opaque cut stock; false retains the translucent default (issue 300).
    pub cut_part_solid: bool,
    /// "Turn part with A": the tool stands upright and the part turns, as on
    /// the machine (SIM-02). On by default, also for projects made before
    /// it was (issue 203).
    pub turn_part_with_a: bool,
    /// The view's "Model" tick box (issue 204).
    pub show_model: bool,
    /// The view's "Cut part" tick box, the simulated stock (issue 204). The
    /// only one ticked by default.
    pub show_cut_part: bool,
    /// The view's "Toolpath" tick box (issue 204).
    pub show_toolpath: bool,
    /// "Hide path outside window" on the playback bar (issue 205): off, so
    /// the rest of the path is drawn faintly.
    pub hide_outside_window: bool,
    /// Whether a new project skips the stock simulation (issue 214); each
    /// project then keeps its own choice.
    pub skip_stock_simulation: bool,
    /// The most memory playback's checkpoints may hold, in MB of 2^20
    /// bytes: [`PLAYBACK_MEMORY_MB`] steps (decision record 0322, the
    /// owner's decision D1 on issue #382).
    pub playback_memory_mb: u32,
}

/// The memory for playback checkpoints, MB of 2^20 bytes: the least, the
/// default, the most and the step.
pub const PLAYBACK_MEMORY_MB: (u32, u32, u32, u32) = (256, 1536, 8192, 256);

impl Default for SimulateSettings {
    fn default() -> Self {
        Self {
            cut_part_solid: false,
            turn_part_with_a: true,
            show_model: false,
            show_cut_part: true,
            show_toolpath: false,
            hide_outside_window: false,
            skip_stock_simulation: false,
            playback_memory_mb: PLAYBACK_MEMORY_MB.1,
        }
    }
}

impl SimulateSettings {
    /// The settings with at least one of the view's tick boxes on: a file
    /// with none gives "Cut part", so the view is never empty (issue 204).
    /// The playback memory is brought to the nearest allowed step.
    #[must_use]
    pub fn clamped(mut self) -> Self {
        if !(self.show_model || self.show_cut_part || self.show_toolpath) {
            self.show_cut_part = true;
        }
        let (least, _, most, step) = PLAYBACK_MEMORY_MB;
        let steps = (self.playback_memory_mb.saturating_add(step / 2) / step).max(1);
        self.playback_memory_mb = steps.saturating_mul(step).clamp(least, most);
        self
    }
}

/// What one user keeps on one computer (specification section 13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    /// The interface scale, 0.8 to 2.0.
    pub scale: f32,
    /// The theme.
    pub theme: ThemeChoice,
    /// The display units. Only millimetres and degrees exist in P1 (inch
    /// display is UI-18, P2), so this is "mm".
    pub units: String,
    /// The window's size in logical pixels, when it has been opened before.
    pub window: Option<[f32; 2]>,
    /// The project panel.
    pub project_panel: PanelLayout,
    /// The inspector.
    pub inspector: PanelLayout,
    /// The dock.
    pub dock: PanelLayout,
    /// Project folders opened, newest first.
    pub recent: Vec<PathBuf>,
    /// The inspector sections that are open, by kind of item ("setup",
    /// "operation").
    pub open_sections: BTreeMap<String, Vec<String>>,
    /// Whether the user's actions are recorded in the diagnostic log
    /// (NFR-18). On unless the user turns it off. A settings file written
    /// before this field existed reads as on: the field is additive and the
    /// file has no version number.
    pub record_actions: bool,
    /// Periodic recovery checkpoint interval, in whole minutes (PRJ-08).
    /// The independent interim checkpoint uses the fixed 15-second debounce.
    pub checkpoint_minutes: u16,
    /// The Simulate step's choices.
    pub simulate: SimulateSettings,
    /// The tool library's folded groups and size filter (decision record
    /// 0199; issues 192 and 193). A file written before the field existed
    /// reads it as unfolded with no filter.
    pub tool_library: ToolLibraryView,
    /// The program file and setup sheet name templates (issues 308 and 310;
    /// decision record 0267), remembered across projects. A file written
    /// before the field existed reads the defaults, which give the names
    /// written before templates existed.
    pub file_names: FileNameTemplates,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            scale: DEFAULT_SCALE,
            theme: ThemeChoice::Dark,
            units: "mm".to_owned(),
            window: None,
            project_panel: PanelLayout::new(region::PROJECT.0),
            inspector: PanelLayout::new(region::INSPECTOR.0),
            dock: PanelLayout::new(region::DOCK.0),
            recent: Vec::new(),
            open_sections: BTreeMap::new(),
            record_actions: true,
            checkpoint_minutes: 3,
            simulate: SimulateSettings::default(),
            tool_library: ToolLibraryView::default(),
            file_names: FileNameTemplates::default(),
        }
    }
}

impl UserSettings {
    /// The settings with every value in its range.
    #[must_use]
    pub fn clamped(mut self) -> Self {
        let d = Self::default();
        self.scale = if self.scale.is_finite() {
            self.scale.clamp(SCALE_RANGE.0, SCALE_RANGE.1)
        } else {
            d.scale
        };
        self.units = "mm".to_owned();
        self.checkpoint_minutes = self.checkpoint_minutes.clamp(1, 15);
        let range = |r: (f32, f32, f32)| (r.1, r.2);
        self.project_panel = self
            .project_panel
            .clamped(range(region::PROJECT), region::PROJECT.0);
        self.inspector = self
            .inspector
            .clamped(range(region::INSPECTOR), region::INSPECTOR.0);
        self.dock = self.dock.clamped(range(region::DOCK), region::DOCK.0);
        self.recent.truncate(RECENT_KEPT);
        self.simulate = self.simulate.clamped();
        self.file_names = self.file_names.clamped();
        self.window = self.window.filter(|[w, h]| {
            (WINDOW_RANGE.0..=WINDOW_RANGE.1).contains(w)
                && (WINDOW_RANGE.0..=WINDOW_RANGE.1).contains(h)
        });
        self
    }

    /// Puts `folder` first in the recent projects, once.
    pub fn opened(&mut self, folder: &Path) {
        let folder = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
        self.recent.retain(|p| *p != folder);
        self.recent.insert(0, folder);
        self.recent.truncate(RECENT_KEPT);
    }

    /// Whether the inspector section `section` of an item of `kind` is open;
    /// `default` when the user has not chosen.
    pub fn section_open(&self, kind: &str, section: &str, default: bool) -> bool {
        match self.open_sections.get(kind) {
            Some(open) => open.iter().any(|s| s == section),
            None => default,
        }
    }

    /// Remembers whether the section `section` of an item of `kind` is open.
    /// `defaults` are the sections open by default, kept when the user first
    /// chooses for this kind.
    pub fn set_section_open(&mut self, kind: &str, section: &str, open: bool, defaults: &[&str]) {
        let list = self
            .open_sections
            .entry(kind.to_owned())
            .or_insert_with(|| defaults.iter().map(|s| (*s).to_owned()).collect());
        list.retain(|s| s != section);
        if open {
            list.push(section.to_owned());
            list.sort();
        }
    }
}

/// The settings file of one user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsFile {
    file: PathBuf,
}

/// The settings read, and what went wrong reading them.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The settings; the defaults where the file could not be read.
    pub settings: UserSettings,
    /// A sentence for the user when the file could not be read.
    pub problem: Option<String>,
}

impl SettingsFile {
    /// The settings file in `folder` (the application's folder under the
    /// configuration folder). The folder may not exist yet.
    pub fn in_folder(folder: &Path) -> Self {
        Self {
            file: folder.join(SETTINGS_FILE),
        }
    }

    /// The settings file in this computer's application configuration
    /// folder, or `None` when that folder cannot be found.
    pub fn default_location() -> Option<Self> {
        let base = crate::library::config_folder_from(crate::library::Platform::current(), |n| {
            std::env::var_os(n)
        })?;
        Some(Self::in_folder(&base.join(crate::library::APP_FOLDER)))
    }

    /// The file's path.
    pub fn file(&self) -> &Path {
        &self.file
    }

    fn read_table(&self) -> Result<toml::Table, String> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => text
                .parse::<toml::Table>()
                .map_err(|e| e.message().to_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Reads the settings. Never fails: see the module notes.
    pub fn load(&self) -> Loaded {
        let problem = |reason: String| {
            Some(format!(
                "Your settings in '{}' could not be read ({reason}), so the defaults are used.",
                self.file.display()
            ))
        };
        match self.read_table() {
            Err(reason) => Loaded {
                settings: UserSettings::default(),
                problem: problem(reason),
            },
            Ok(table) => match table.get(TABLE) {
                None => Loaded {
                    settings: UserSettings::default(),
                    problem: None,
                },
                Some(value) => match value.clone().try_into::<UserSettings>() {
                    Ok(settings) => Loaded {
                        settings: settings.clamped(),
                        problem: None,
                    },
                    Err(e) => Loaded {
                        settings: UserSettings::default(),
                        problem: problem(e.message().to_owned()),
                    },
                },
            },
        }
    }

    /// Writes `settings`, keeping the file's other tables. A file that cannot
    /// be read is kept beside the new one as `settings.toml.bad`, so nothing
    /// is lost.
    pub fn save(&self, settings: &UserSettings) -> Result<(), AppError> {
        let io = |path: &Path| {
            let path = path.to_path_buf();
            move |source| AppError::Io { path, source }
        };
        let mut table = match self.read_table() {
            Ok(table) => table,
            Err(_) => {
                if self.file.exists() {
                    let bad = self.file.with_extension("toml.bad");
                    std::fs::rename(&self.file, &bad).map_err(io(&bad))?;
                }
                toml::Table::new()
            }
        };
        let value = toml::Value::try_from(settings.clone().clamped())
            .map_err(|e| settings_error(&self.file, &e.to_string()))?;
        table.insert(TABLE.to_owned(), value);
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(io(folder))?;
        }
        let text = toml::to_string_pretty(&table)
            .map_err(|e| settings_error(&self.file, &e.to_string()))?;
        let temp = self.file.with_extension("toml.part");
        std::fs::write(&temp, text)
            .and_then(|()| std::fs::rename(&temp, &self.file))
            .map_err(io(&self.file))
    }
}

fn settings_error(file: &Path, reason: &str) -> AppError {
    AppError::Io {
        path: file.to_path_buf(),
        source: std::io::Error::other(reason.to_owned()),
    }
}
