// SPDX-License-Identifier: AGPL-3.0-or-later
//! The export acknowledgement, kept per project on each computer (GC-16,
//! UI-15; specification sections 11.6 and 13; decision record 0071).
//!
//! Before the first export of a project on a computer the user confirms that
//! they will verify the G-code. The confirmation lives in the user's
//! `settings.toml` in the configuration folder (see [`crate::library`]), keyed
//! by the project's canonical folder path (OI-165), and never in the project:
//! another computer, or the project moved to another folder, asks again.
//!
//! The file may hold other users' keys written by other parts of the
//! application; this module reads and writes only `export.acknowledged` and
//! keeps the rest as it was. A missing or unreadable file gives "not
//! acknowledged" and a message, never a crash (NFR-07).

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// The file name under the configuration folder's `OpenChuckCAM/` folder.
pub const SETTINGS_FILE: &str = "settings.toml";

/// The key of the export acknowledgement in the `[export]` table.
const ACKNOWLEDGED: &str = "acknowledged";

/// The key of the acknowledgement that a project skips the stock simulation.
const STOCK_SKIPPED: &str = "stock_skipped_acknowledged";

/// The acknowledgements in one settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acknowledgements {
    file: PathBuf,
}

/// Whether a project is acknowledged, and what went wrong reading the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    /// True when the project's path is in the file.
    pub acknowledged: bool,
    /// A sentence for the user when the file could not be read.
    pub problem: Option<String>,
}

impl Acknowledgements {
    /// The settings file in `folder` (the application's folder under the
    /// configuration folder). The folder may not exist yet.
    pub fn in_folder(folder: &Path) -> Self {
        Self {
            file: folder.join(SETTINGS_FILE),
        }
    }

    /// The settings file in this computer's application configuration folder,
    /// or `None` when that folder cannot be found.
    pub fn default_location() -> Option<Self> {
        let base = crate::library::config_folder_from(crate::library::Platform::current(), |n| {
            std::env::var_os(n)
        })?;
        Some(Self::in_folder(&base.join(crate::library::APP_FOLDER)))
    }

    /// The settings file's path.
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// How a project is identified: its canonical folder path, or the path as
    /// given when it cannot be resolved.
    fn key(project: &Path) -> String {
        std::fs::canonicalize(project)
            .unwrap_or_else(|_| project.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }

    fn read(&self) -> (toml::Table, Option<String>) {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => match text.parse::<toml::Table>() {
                Ok(table) => (table, None),
                Err(e) => (
                    toml::Table::new(),
                    Some(format!(
                        "The settings file '{}' could not be read ({}), so exports ask again.",
                        self.file.display(),
                        e.message()
                    )),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (toml::Table::new(), None),
            Err(e) => (
                toml::Table::new(),
                Some(format!(
                    "The settings file '{}' could not be read ({e}), so exports ask again.",
                    self.file.display()
                )),
            ),
        }
    }

    fn listed(table: &toml::Table, field: &str) -> Vec<String> {
        table
            .get("export")
            .and_then(|e| e.get(field))
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether `project` (the project folder) is acknowledged on this computer.
    pub fn lookup(&self, project: &Path) -> Lookup {
        self.lookup_field(project, ACKNOWLEDGED)
    }

    /// Whether the user has acknowledged on this computer that `project`'s
    /// cut is not simulated while it skips the stock simulation (decision
    /// record 0168, OI-602).
    pub fn lookup_stock_skipped(&self, project: &Path) -> Lookup {
        self.lookup_field(project, STOCK_SKIPPED)
    }

    fn lookup_field(&self, project: &Path, field: &str) -> Lookup {
        let (table, problem) = self.read();
        Lookup {
            acknowledged: Self::listed(&table, field).contains(&Self::key(project)),
            problem,
        }
    }

    /// Records that the user confirmed they will verify the G-code of
    /// `project` on this computer. A settings file that cannot be read is
    /// kept beside the new one as `settings.toml.bad`, so nothing is lost.
    pub fn acknowledge(&self, project: &Path) -> Result<(), AppError> {
        self.acknowledge_field(project, ACKNOWLEDGED)
    }

    /// Records that the user acknowledged that `project` skips the stock
    /// simulation, as [`Acknowledgements::acknowledge`] does for the export.
    pub fn acknowledge_stock_skipped(&self, project: &Path) -> Result<(), AppError> {
        self.acknowledge_field(project, STOCK_SKIPPED)
    }

    fn acknowledge_field(&self, project: &Path, field: &str) -> Result<(), AppError> {
        let io = |path: &Path| {
            let path = path.to_path_buf();
            move |source| AppError::Io { path, source }
        };
        let (mut table, problem) = self.read();
        if problem.is_some() && self.file.exists() {
            let bad = self.file.with_extension("toml.bad");
            std::fs::rename(&self.file, &bad).map_err(io(&bad))?;
        }
        let mut listed = Self::listed(&table, field);
        let key = Self::key(project);
        if !listed.contains(&key) {
            listed.push(key);
        }
        listed.sort();
        let entry = table
            .entry("export")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(export) = entry.as_table_mut() {
            export.insert(
                field.to_owned(),
                toml::Value::Array(listed.into_iter().map(toml::Value::String).collect()),
            );
        } else {
            let mut export = toml::Table::new();
            export.insert(
                field.to_owned(),
                toml::Value::Array(listed.into_iter().map(toml::Value::String).collect()),
            );
            *entry = toml::Value::Table(export);
        }
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder).map_err(io(folder))?;
        }
        let text = toml::to_string_pretty(&table)
            .map_err(|e| AppError::Post(format!("cannot write the settings file: {e}")))?;
        let temp = self.file.with_extension("toml.part");
        std::fs::write(&temp, text)
            .and_then(|()| std::fs::rename(&temp, &self.file))
            .map_err(io(&self.file))
    }
}
