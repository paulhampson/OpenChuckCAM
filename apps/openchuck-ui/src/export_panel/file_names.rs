// SPDX-License-Identifier: AGPL-3.0-or-later
//! The export dialog's file name templates (issues 308 and 310; decision
//! record 0267). The templates are the user's, kept in the settings file by
//! the shell; the names, the checks and the collisions are worked out in
//! `openchuck-app`, so the preview is the list of names that is written.

use openchuck_app::output_names::{
    DEFAULT_GCODE_TEMPLATE, DEFAULT_SHEET_TEMPLATE, NameField, fields_list, template_problem,
};
use openchuck_app::{FileNameTemplates, OutputFiles};

use super::*;

/// The program file template field's label, also its pending-edit name.
pub(super) const GCODE_LABEL: &str = "Program file name";
/// The setup sheet template field's label, also its pending-edit name.
pub(super) const SHEET_LABEL: &str = "Setup sheet name";

/// The names worked out for one run and one pair of templates, kept until
/// either changes so the preview does not split the toolpath every frame.
pub(super) struct Named {
    run: Arc<Regeneration>,
    templates: FileNameTemplates,
    pub(super) result: Result<OutputFiles, String>,
}

impl ExportPanel {
    /// Uses the user's templates, from the settings file (the shell calls
    /// this when the dialog opens).
    pub fn set_file_names(&mut self, templates: FileNameTemplates) {
        self.file_names = templates;
        self.named = None;
    }

    /// The templates to keep in the user's settings, or `None` while one is
    /// being typed and not yet confirmed (decision 0233: a draft is not
    /// applied until it is confirmed).
    pub fn file_names_to_keep(&self, ctx: &egui::Context) -> Option<&FileNameTemplates> {
        let drafting = crate::pending_edits::entries(ctx)
            .iter()
            .any(|e| e.label == GCODE_LABEL || e.label == SHEET_LABEL);
        (!drafting).then_some(&self.file_names)
    }

    /// The final names of the current files, worked out again only when the
    /// run or the templates change.
    pub(super) fn named(&mut self, app: &Application) -> Option<&Result<OutputFiles, String>> {
        let run = Arc::clone(&self.files_current(app)?.run);
        // Keyed on the templates the application names with, which the
        // dialog gives it at the start of each frame.
        let templates = app.file_name_templates();
        let fresh = self
            .named
            .as_ref()
            .is_some_and(|named| Arc::ptr_eq(&named.run, &run) && named.templates == *templates);
        if !fresh {
            let result = app.output_files(&run).map_err(|e| e.to_string());
            self.named = Some(Named {
                run,
                templates: templates.clone(),
                result,
            });
        }
        self.named.as_ref().map(|named| &named.result)
    }

    /// Why the names stop writing, if they do: a template that cannot be
    /// read, or names that cannot all be written.
    pub(super) fn names_problem(&mut self, app: &Application) -> Option<String> {
        if let Some(problem) = template_problem(&self.file_names.gcode, false)
            .or_else(|| template_problem(&self.file_names.setup_sheet, true))
        {
            return Some(problem);
        }
        self.named(app)?.as_ref().err().cloned()
    }

    /// The two template fields with their fields list, reset buttons and
    /// the problem that stops writing, under the output folder.
    pub(super) fn file_names_section(&mut self, ui: &mut egui::Ui, app: &Application) {
        ui.label("File names");
        for (label, sheet, default) in [
            (GCODE_LABEL, false, DEFAULT_GCODE_TEMPLATE),
            (SHEET_LABEL, true, DEFAULT_SHEET_TEMPLATE),
        ] {
            ui.horizontal(|ui| {
                ui.label(if sheet {
                    "Setup sheet"
                } else {
                    "Program files"
                });
                let text = if sheet {
                    &mut self.file_names.setup_sheet
                } else {
                    &mut self.file_names.gcode
                };
                crate::pending_edits::text_edit(ui, text, label).on_hover_text(if sheet {
                    "The setup sheet's name, written as .html and .pdf."
                } else {
                    "Each program file's name; the post adds the extension."
                });
                if ui
                    .add_enabled(text != default, egui::Button::new("Reset"))
                    .on_hover_text(format!("Use the default, {default}."))
                    .clicked()
                {
                    *text = default.to_owned();
                }
            });
        }
        if let Some(problem) = self.names_problem(app) {
            ui.add(egui::Label::new(RichText::new(problem).color(PROBLEM)).wrap());
        }
        egui::CollapsingHeader::new("Fields")
            .id_salt("export name fields")
            .show(ui, |ui| {
                for field in NameField::ALL {
                    ui.horizontal_wrapped(|ui| {
                        ui.monospace(format!("{{{}}}", field.as_str()));
                        ui.label(field.description());
                    });
                }
                ui.weak(format!(
                    "The setup sheet can use {}. Write {{{{ or }}}} for a brace. Each field is written in lower case with hyphens, and the whole name is made safe for Windows, macOS and Linux.",
                    fields_list(true)
                ));
            });
        ui.weak("These names are kept for every project on this computer.");
    }
}
