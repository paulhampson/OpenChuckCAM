// SPDX-License-Identifier: AGPL-3.0-or-later
//! The inspector's field row, drawn once for every form (UI-07; interface
//! specification section 8, "Fields"; decision record 0029 item 11; slice 22
//! lane X4, decision 0114 C10).
//!
//! A row is a label in a 120-pixel column, with a reset button (↺) at its
//! right end when the value can be reset, and the value to its right. A
//! number field draws its unit inside the field on the right, and gives the
//! allowed range in the field's tooltip. An empty field shows the value that
//! applies as a hint, with its source.
//!
//! Typed text is kept here until it is committed: on Enter or when the field
//! loses focus. Esc puts back the last applied text. A refused value stays in
//! its field with a red border and the reason in red under it. The caller
//! applies a committed value (one undoable command) and reports the outcome
//! with [`FieldState::result`].

use std::collections::BTreeMap;

use eframe::egui::{self, Key, RichText, Stroke};

use crate::theme::{DARK, RADIUS, width};

/// Text being typed into fields, and refused values with their reasons, by
/// the row's key.
#[derive(Debug, Default)]
pub struct FieldState {
    text: BTreeMap<String, String>,
    errors: BTreeMap<String, String>,
}

impl FieldState {
    /// Forgets typed text and errors, for another item or after an undo.
    pub fn clear(&mut self) {
        self.text.clear();
        self.errors.clear();
    }

    /// Records the outcome of applying a committed value of `key`: accepted
    /// drops the typed text, refused keeps it with the reason.
    pub fn result(&mut self, key: &str, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.text.remove(key);
                self.errors.remove(key);
            }
            Err(e) => {
                self.errors.insert(key.to_owned(), e);
            }
        }
    }

    /// Marks `key` refused with `reason`, keeping `typed` in the field.
    pub fn refuse(&mut self, key: &str, typed: &str, reason: String) {
        self.text.insert(key.to_owned(), typed.to_owned());
        self.errors.insert(key.to_owned(), reason);
    }

    /// The reason a field's value was refused, if it was.
    pub fn error(&self, key: &str) -> Option<&str> {
        self.errors.get(key).map(String::as_str)
    }

    /// The text typed into `key` and not yet applied, if any.
    pub fn typed(&self, key: &str) -> Option<&str> {
        self.text.get(key).map(String::as_str)
    }

    /// True while a field holds a refused value.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// What the user did with a row this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to apply.
    None,
    /// Apply this text (Enter, or the field lost focus with changed text).
    Commit(String),
    /// Reset the value, so the default or the source applies (↺).
    Reset,
}

/// A text or number row.
#[derive(Debug, Clone, Default)]
pub struct TextRow<'a> {
    /// Identifies the row's typed text and refusal; unique within the form.
    pub key: &'a str,
    /// The label, in sentence case.
    pub label: &'a str,
    /// What the value does, for the label's tooltip.
    pub description: &'a str,
    /// The unit drawn inside the field ("mm", "°", "mm/min"), or empty.
    pub unit: &'a str,
    /// The allowed range in words, for the field's tooltip.
    pub range: Option<&'a str>,
    /// The text of the applied value; empty when the field is empty.
    pub applied: String,
    /// The value that applies while the field is empty, with its source:
    /// "0.020 · from the tool's cutting data".
    pub hint: Option<String>,
    /// A refusal or check to show under the field, besides the one this
    /// state holds (a draft's check, 11.7).
    pub problem: Option<&'a str>,
    /// Offer ↺ in the label column.
    pub reset: bool,
    /// False draws the row read-only.
    pub enabled: bool,
    /// Ask for keyboard focus this frame (a new tab's Start X, 11.7).
    pub focus: bool,
    /// The operation's path was made with a warning that points at this
    /// setting, in a plain sentence: an amber border and, under the field,
    /// the warning shape and the sentence, so the mark does not rest on
    /// colour alone (decision record 0175; the marks of decision record
    /// 0173). Shown whatever the field holds, unlike a refusal.
    pub warned: Option<&'a str>,
    /// A required value that is missing, in a few plain words ("Enter feed"):
    /// an amber border and, under the field, the same warning shape the tree
    /// uses and these words, so the mark does not rest on colour alone
    /// (UI-11; issue 224). Calmer than `problem`, which is for a refusal.
    pub needs: Option<&'a str>,
    /// Scroll this row into view this frame (the "Show setting" and "Show
    /// first" jumps).
    pub scroll_to: bool,
}

/// Shows `text` as the tooltip of `response` on hover and while it has
/// keyboard focus (section 16: tooltips open on focus as well as hover).
pub fn tip(response: egui::Response, text: &str) -> egui::Response {
    if response.has_focus() {
        response.show_tooltip_text(text);
    }
    response.on_hover_text(text)
}

/// A small outlined chip ("Not applied"): measured first and drawn in one
/// allocated rectangle, so its place does not depend on the layout it is in
/// (a frame in a right-to-left row lay outside the pane and widened it).
pub fn chip(ui: &mut egui::Ui, text: &str, colour: egui::Color32) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::TextStyle::Small.resolve(ui.style()),
        colour,
    );
    let size = galley.size() + egui::vec2(10.0, 2.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, DARK.line),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, colour);
    response
}

/// The label column: the label with its tooltip, and ↺ at its right end.
/// Returns true when ↺ was clicked.
fn label_cell(ui: &mut egui::Ui, label: &str, description: &str, reset: bool) -> bool {
    let height = ui.spacing().interact_size.y;
    let size = egui::vec2(width::LABEL_COLUMN, height);
    // Allocated and drawn in one step, so the cell is where the cursor is and
    // the value follows it directly.
    ui.allocate_ui_with_layout(
        size,
        egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Min),
        |ui| {
            // The cell is the full column whatever the label's width.
            ui.set_min_size(size);
            let mut clicked = false;
            if reset {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = ui
                        .add(
                            egui::Button::new(RichText::new("\u{21ba}").color(DARK.accent)).small(),
                        )
                        .on_hover_text("Reset: the default or the tool's cutting data applies");
                    button.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            format!("Reset {label}"),
                        )
                    });
                    clicked = button.clicked();
                    label_text(ui, label, description);
                });
            } else {
                label_text(ui, label, description);
            }
            clicked
        },
    )
    .inner
}

fn label_text(ui: &mut egui::Ui, label: &str, description: &str) {
    let response = ui.add(
        egui::Label::new(RichText::new(label).color(DARK.muted))
            .truncate()
            .sense(egui::Sense::hover()),
    );
    if !description.is_empty() {
        response.on_hover_text(description);
    }
}

/// The reason in red under the value, indented past the label column.
fn reason_line(ui: &mut egui::Ui, reason: &str, colour: egui::Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(width::LABEL_COLUMN + ui.spacing().item_spacing.x);
        ui.label(RichText::new(reason).color(colour));
    });
}

/// Draws a text or number row. Returns what the user did.
pub fn text_row(ui: &mut egui::Ui, state: &mut FieldState, row: &TextRow<'_>) -> Outcome {
    let mut outcome = Outcome::None;
    let error = state.error(row.key).map(str::to_owned);
    let marked = error.is_some() || row.problem.is_some();
    let needed = row.needs.is_some() && !marked && row.applied.trim().is_empty();
    let warned = row.warned.is_some() && !marked;
    if row.scroll_to {
        ui.scroll_to_cursor(Some(egui::Align::Center));
    }
    ui.horizontal(|ui| {
        if label_cell(ui, row.label, row.description, row.reset && row.enabled) {
            outcome = Outcome::Reset;
        }
        let mut typed = state
            .typed(row.key)
            .map_or_else(|| row.applied.clone(), str::to_owned);
        // The box takes the rest of the row as one fixed rectangle, so what is
        // typed in it cannot change its size or the size of the pane around
        // it (a row that grew with its content made the inspector widen).
        let margin = egui::vec2(6.0, 2.0);
        let box_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0 * margin.y;
        let (frame_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().max(60.0), box_height),
            egui::Sense::hover(),
        );
        let background = ui.painter().add(egui::Shape::Noop);
        let response = {
            let mut inner = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(frame_rect.shrink2(margin))
                    .layout(
                        egui::Layout::left_to_right(egui::Align::Center)
                            .with_main_align(egui::Align::Min),
                    ),
            );
            let ui = &mut inner;
            let unit = (!row.unit.is_empty()).then(|| {
                ui.painter().layout_no_wrap(
                    row.unit.to_owned(),
                    egui::TextStyle::Body.resolve(ui.style()),
                    DARK.muted,
                )
            });
            let unit_width = unit
                .as_ref()
                .map_or(0.0, |g| g.size().x + ui.spacing().item_spacing.x);
            let mut edit = egui::TextEdit::singleline(&mut typed)
                .frame(egui::Frame::NONE)
                .font(egui::TextStyle::Monospace)
                .desired_width((ui.available_width() - unit_width).max(0.0))
                .interactive(row.enabled);
            if let Some(hint) = &row.hint {
                edit = edit.hint_text(RichText::new(hint).color(DARK.muted));
            }
            let response = ui.add(edit);
            if let Some(unit) = unit {
                let (rect, _) = ui.allocate_exact_size(unit.size(), egui::Sense::hover());
                ui.painter().galley(rect.min, unit, DARK.muted);
            }
            response
        };
        let border = if marked {
            DARK.error
        } else if response.has_focus() {
            DARK.accent
        } else if needed || warned {
            DARK.warning
        } else {
            DARK.line
        };
        ui.painter().set(
            background,
            egui::epaint::RectShape::new(
                frame_rect,
                RADIUS,
                DARK.field,
                Stroke::new(
                    if response.has_focus() || needed || warned {
                        2.0
                    } else {
                        1.0
                    },
                    border,
                ),
                egui::StrokeKind::Inside,
            ),
        );
        let response = match row.range {
            Some(range) => response.on_hover_text(format!("Allowed: {range}.")),
            None => response,
        };
        if row.focus {
            response.request_focus();
        }
        if response.changed() {
            state.text.insert(row.key.to_owned(), typed.clone());
        }
        if response.lost_focus() {
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                // Esc puts back the last applied text (section 8).
                state.text.remove(row.key);
                state.errors.remove(row.key);
            } else if state.text.contains_key(row.key) {
                if typed.trim() == row.applied.trim() && error.is_none() {
                    state.text.remove(row.key);
                } else {
                    outcome = Outcome::Commit(typed);
                }
            }
        }
    });
    if let Some(reason) = error.as_deref().or(row.problem) {
        reason_line(ui, reason, DARK.error);
    } else if let (true, Some(text)) = (needed, row.needs) {
        warning_line(ui, text);
    } else if let Some(text) = row.warned {
        warning_line(ui, text);
    }
    outcome
}

/// A warning under a field or a section: the warning shape the tree uses and
/// a plain sentence, indented past the label column. It says what is missing
/// (issue 224; UI-11) or why a setting is marked by its path's warning
/// (decision record 0175).
pub fn warning_line(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(width::LABEL_COLUMN + ui.spacing().item_spacing.x);
        crate::shell::mark(ui, openchuck_app::overview::ItemState::Warning, text);
        ui.label(RichText::new(text).color(DARK.warning));
    });
}

/// The inspector header's name, in an editable field with no label (section
/// 8): drawn as the operation name until it has focus. It commits as a row
/// does, Esc puts the name back, and a refusal shows under it.
pub fn name_field(ui: &mut egui::Ui, state: &mut FieldState, key: &str, applied: &str) -> Outcome {
    let mut outcome = Outcome::None;
    let mut typed = state
        .typed(key)
        .map_or_else(|| applied.to_owned(), str::to_owned);
    let response = ui.add(
        egui::TextEdit::singleline(&mut typed)
            .font(egui::TextStyle::Heading)
            .desired_width(ui.available_width() - 90.0),
    );
    response.widget_info(|| egui::WidgetInfo::text_edit(true, applied, &typed, "Name"));
    if response.changed() {
        state.text.insert(key.to_owned(), typed.clone());
    }
    if response.lost_focus() {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            state.text.remove(key);
            state.errors.remove(key);
        } else if state.text.contains_key(key) {
            if typed == applied {
                state.text.remove(key);
                state.errors.remove(key);
            } else {
                outcome = Outcome::Commit(typed);
            }
        }
    }
    outcome
}

/// The reason a header's name was refused, in red under it.
pub fn name_problem(ui: &mut egui::Ui, state: &FieldState, key: &str) {
    if let Some(reason) = state.error(key) {
        ui.label(RichText::new(reason).color(DARK.error));
    }
}

/// Draws a read-only row: the label and the value as text, with an optional
/// note under it.
pub fn read_only_row(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    value: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.horizontal(|ui| {
        label_cell(ui, label, description, false);
        ui.add(egui::Label::new(value).wrap())
    })
    .inner
}

/// A derived line under a row, indented past the label column ("Small end
/// towards the part").
pub fn note_line(ui: &mut egui::Ui, text: &str, colour: egui::Color32) {
    reason_line(ui, text, colour);
}

/// Draws a check box row. Returns the new value when it was toggled.
pub fn flag_row(
    ui: &mut egui::Ui,
    label: &str,
    description: &str,
    value: bool,
    enabled: bool,
) -> Option<bool> {
    let mut on = value;
    let changed = ui
        .horizontal(|ui| {
            label_cell(ui, label, description, false);
            let response = ui.add_enabled(enabled, egui::Checkbox::without_text(&mut on));
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, label)
            });
            response.changed()
        })
        .inner;
    changed.then_some(on)
}

/// Draws a segmented control of `options`, the selected one filled with the
/// `selected` token (two to four exclusive choices, section 8). Returns the
/// index chosen this frame.
pub fn segmented(
    ui: &mut egui::Ui,
    options: &[&str],
    current: Option<usize>,
    enabled: bool,
) -> Option<usize> {
    let mut chosen = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, option) in options.iter().enumerate() {
            let on = current == Some(i);
            let button = egui::Button::new(RichText::new(*option).color(if on {
                DARK.text
            } else {
                DARK.muted
            }))
            .fill(if on { DARK.selected } else { DARK.field })
            .stroke(Stroke::new(1.0, DARK.line))
            .corner_radius(0.0)
            .selected(on);
            if ui.add_enabled(enabled, button).clicked() && !on {
                chosen = Some(i);
            }
        }
    });
    chosen
}

/// The width the segmented control of `options` needs, so a row can fall
/// back to a drop-down list when the pane is too narrow for it (a rigid
/// control that does not fit holds the pane open).
fn segmented_width(ui: &egui::Ui, options: &[&str]) -> f32 {
    options
        .iter()
        .map(|option| {
            ui.painter()
                .layout_no_wrap(
                    (*option).to_owned(),
                    egui::TextStyle::Button.resolve(ui.style()),
                    DARK.text,
                )
                .size()
                .x
                + 2.0 * ui.spacing().button_padding.x
                + 2.0
        })
        .sum()
}

/// A row of a choice: a segmented control for two to four options, else a
/// drop-down list. Returns the index chosen this frame.
pub fn choice_row(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    description: &str,
    options: &[&str],
    current: Option<usize>,
    enabled: bool,
) -> Option<usize> {
    ui.horizontal(|ui| {
        label_cell(ui, label, description, false);
        if (2..=4).contains(&options.len()) && segmented_width(ui, options) <= ui.available_width()
        {
            segmented(ui, options, current, enabled)
        } else {
            let mut chosen = None;
            let shown = current
                .and_then(|i| options.get(i))
                .copied()
                .unwrap_or_default();
            ui.add_enabled_ui(enabled, |ui| {
                egui::ComboBox::from_id_salt(("choice", key))
                    .selected_text(shown)
                    .width(ui.available_width().max(80.0))
                    .show_ui(ui, |ui| {
                        for (i, option) in options.iter().enumerate() {
                            if ui.selectable_label(current == Some(i), *option).clicked()
                                && current != Some(i)
                            {
                                chosen = Some(i);
                            }
                        }
                    });
            });
            chosen
        }
    })
    .inner
}

/// The decimal places a quantity is shown with (14.3): positions and lengths
/// in mm to 3, degrees to 2, feeds whole; a plain number as it is.
pub fn places(unit: &str) -> Option<usize> {
    match unit {
        "mm" => Some(3),
        "°" | "deg" => Some(2),
        "mm/min" | "rpm" => Some(0),
        _ => None,
    }
}

/// `value` with `places` decimals, or more where fewer would show another
/// value, so a stored value is never misread (14.3). With no places, the
/// shortest text that reads back as `value`.
pub fn number(value: f64, places: Option<usize>) -> String {
    let Some(places) = places else {
        return format!("{value}");
    };
    (places..=9)
        .map(|p| format!("{value:.p$}"))
        .find(|t| {
            t.parse::<f64>()
                .is_ok_and(|v| (v - value).abs() <= 1e-12 * value.abs().max(1.0))
        })
        .unwrap_or_else(|| format!("{value}"))
}

/// What a section of the inspector gave back this frame.
pub struct Section<R> {
    /// What the body returned, when it is open.
    pub body: Option<R>,
    /// Whether it is open after this frame.
    pub open: bool,
    /// The header row, to scroll to.
    pub header: egui::Response,
}

/// A collapsible section of the inspector (section 8): its title upper case,
/// small, in the heading colour, with `extra` drawn after it (chips, a
/// status mark). Clicking the title or the arrow opens or closes it; the
/// caller remembers the open state per item kind. `force_open` opens it this
/// frame (a tree row or a message chose it).
pub fn section<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    title: &str,
    default_open: bool,
    force_open: bool,
    extra: impl FnOnce(&mut egui::Ui),
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Section<R> {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        default_open,
    );
    if force_open {
        state.set_open(true);
    }
    let mut clicked = false;
    let mut header = state.show_header(ui, |ui| {
        let title = ui.add(
            egui::Label::new(
                RichText::new(title.to_uppercase())
                    .small()
                    .strong()
                    .color(DARK.heading),
            )
            .sense(egui::Sense::click()),
        );
        extra(ui);
        clicked = title.clicked();
    });
    if clicked {
        header.toggle();
    }
    let open = header.is_open();
    let (_, header, body) = header.body(body);
    Section {
        body: body.map(|b| b.inner),
        open,
        header: header.response,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Runs one frame of a row, with `events` as the frame's input.
    fn frame(
        ctx: &egui::Context,
        state: &mut FieldState,
        row: &TextRow<'_>,
        events: Vec<egui::Event>,
    ) -> Outcome {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 300.0),
            )),
            events,
            ..Default::default()
        };
        let mut outcome = Outcome::None;
        let mut output = ctx.run_ui(input, |ui| {
            outcome = text_row(ui, state, row);
        });
        output.textures_delta.clear();
        outcome
    }

    fn key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn stepover(focus: bool) -> TextRow<'static> {
        TextRow {
            key: "stepover",
            label: "Stepover",
            unit: "mm",
            range: Some("above 0 mm"),
            applied: "0.500".to_owned(),
            enabled: true,
            focus,
            ..TextRow::default()
        }
    }

    /// Focuses the row and types `text` over its value.
    fn type_text(ctx: &egui::Context, state: &mut FieldState, text: &str) {
        frame(ctx, state, &stepover(true), Vec::new());
        frame(
            ctx,
            state,
            &stepover(false),
            vec![
                key(Key::End),
                egui::Event::Key {
                    key: Key::Home,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::SHIFT,
                },
            ],
        );
        frame(
            ctx,
            state,
            &stepover(false),
            vec![egui::Event::Text(text.to_owned())],
        );
    }

    #[test]
    fn req_ui_07_a_value_is_applied_on_enter_not_while_typing() {
        let ctx = egui::Context::default();
        let mut state = FieldState::default();
        type_text(&ctx, &mut state, "0.25");
        assert_eq!(state.typed("stepover"), Some("0.25"));
        // Typing applies nothing; Enter commits the typed text.
        let outcome = frame(&ctx, &mut state, &stepover(false), vec![key(Key::Enter)]);
        assert_eq!(outcome, Outcome::Commit("0.25".to_owned()));
        state.result("stepover", Ok(()));
        assert_eq!(state.typed("stepover"), None);
    }

    #[test]
    fn req_ui_07_esc_puts_back_the_last_applied_text() {
        let ctx = egui::Context::default();
        let mut state = FieldState::default();
        type_text(&ctx, &mut state, "9");
        let outcome = frame(&ctx, &mut state, &stepover(false), vec![key(Key::Escape)]);
        assert_eq!(outcome, Outcome::None);
        assert_eq!(state.typed("stepover"), None);
        assert!(state.typed("stepover").is_none());
    }

    #[test]
    fn req_ui_07_a_refused_value_stays_with_its_reason_until_accepted() {
        let ctx = egui::Context::default();
        let mut state = FieldState::default();
        type_text(&ctx, &mut state, "-1");
        let outcome = frame(&ctx, &mut state, &stepover(false), vec![key(Key::Enter)]);
        assert_eq!(outcome, Outcome::Commit("-1".to_owned()));
        state.result(
            "stepover",
            Err("Must be above 0 mm. Not applied.".to_owned()),
        );
        // The typed text stays in the field, the reason under it.
        assert_eq!(state.typed("stepover"), Some("-1"));
        assert_eq!(
            state.error("stepover"),
            Some("Must be above 0 mm. Not applied.")
        );
        state.result("stepover", Ok(()));
        assert_eq!(state.error("stepover"), None);
        assert!(state.typed("stepover").is_none());
    }

    #[test]
    fn req_ui_07_numbers_have_fixed_places_and_never_misread() {
        assert_eq!(number(0.5, places("mm")), "0.500");
        assert_eq!(number(90.0, places("°")), "90.00");
        assert_eq!(number(1200.0, places("mm/min")), "1200");
        // More places where the fixed ones would show another value.
        assert_eq!(number(0.0005, places("mm")), "0.0005");
        assert_eq!(number(0.1, None), "0.1");
    }
}
