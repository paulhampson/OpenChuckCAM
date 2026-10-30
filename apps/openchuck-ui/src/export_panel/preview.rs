// SPDX-License-Identifier: AGPL-3.0-or-later
//! The read-only G-code preview in the export dialog (GC-05, GC-07, NFR-03,
//! NFR-12, NFR-15, UI-16; decision 0327, issue 312).
//!
//! Only the rows in view are drawn, from a line index, so a program of a
//! million lines costs the same per frame as one of a hundred. The position
//! is a line number held as `f64`, not a pixel offset, because `f32` pixels
//! lose whole rows past about a million lines of 16 pixels.

use eframe::egui::{self, Color32, FontId, Key, RichText, Sense, Vec2};
use openchuck_app::output_preview::PreviewFile;

/// How many rows the preview shows at once.
pub(super) const VISIBLE_ROWS: usize = 16;
const TRACK_WIDTH: f32 = 12.0;

/// Where the preview is looking.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Scroll {
    /// The first line in view, counting from 0, with a fraction while a
    /// scroll is between two rows.
    pub top: f64,
    /// How many characters the lines are shifted left.
    pub left: usize,
}

impl Scroll {
    fn last_top(total: usize) -> f64 {
        total.saturating_sub(VISIBLE_ROWS) as f64
    }

    /// Moves by `lines` (negative goes up), staying inside the file.
    pub fn by(&mut self, lines: f64, total: usize) {
        self.top = (self.top + lines).clamp(0.0, Self::last_top(total));
    }

    /// Puts line `line` (counting from 1) in the middle of the view.
    pub fn go_to(&mut self, line: usize, total: usize) {
        let middle = (VISIBLE_ROWS / 2) as f64;
        self.top = (line.saturating_sub(1) as f64 - middle).clamp(0.0, Self::last_top(total));
    }

    pub fn first_row(&self) -> usize {
        self.top.max(0.0) as usize
    }
}

/// The line a typed "go to" names, counting from 1, or why it names none.
pub(super) fn parse_line(text: &str, total: usize) -> Result<usize, String> {
    let text = text.trim();
    if total == 0 {
        return Err("The file has no lines.".to_owned());
    }
    match text.parse::<usize>() {
        Ok(n) if (1..=total).contains(&n) => Ok(n),
        Ok(_) => Err(format!("Choose a line from 1 to {total}.")),
        Err(_) if text.is_empty() => Err("Type a line number.".to_owned()),
        Err(_) => Err(format!("'{text}' is not a line number.")),
    }
}

/// What the preview remembers between frames.
#[derive(Debug, Default)]
pub(super) struct PreviewView {
    pub scroll: Scroll,
    pub goto: String,
    pub problem: Option<String>,
    /// Which file the position belongs to: the run's address and the index.
    shown: Option<(usize, usize)>,
    /// A frame asked for the go-to box to take the keyboard.
    focus_goto: bool,
    pub needle: String,
    /// The line the last search found, counting from 0.
    pub hit: Option<usize>,
}

impl PreviewView {
    /// Forgets the position when another file or run is shown.
    pub fn follow(&mut self, run: usize, file: usize) {
        if self.shown != Some((run, file)) {
            self.shown = Some((run, file));
            self.scroll = Scroll::default();
            self.problem = None;
            self.hit = None;
        }
    }

    /// Draws the viewer of `file`.
    pub fn show(&mut self, ui: &mut egui::Ui, file: &PreviewFile) {
        let total = file.line_count();
        let font = FontId::monospace(13.0);
        let (char_width, row_height) =
            ui.fonts_mut(|f| (f.glyph_width(&font, '0'), f.row_height(&font) + 2.0));
        let digits = total.max(1).to_string().len();
        let gutter = char_width * (digits as f32 + 1.5);
        self.goto_row(ui, total);
        self.search_row(ui, file);
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(width, row_height * VISIBLE_ROWS as f32 + 4.0),
            Sense::click_and_drag(),
        );
        if response.clicked() || response.dragged() {
            response.request_focus();
        }
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    tab: false,
                    escape: false,
                },
            );
        });
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Group,
                true,
                format!("G-code preview, {total} lines"),
            )
        });
        self.keys(ui, &response, total);
        // The wheel belongs to the preview while the pointer is over it, so
        // the dialog's own scroll area does not move too.
        // At the top or the bottom the wheel passes on to the dialog, so the
        // pointer is never trapped over the preview.
        let last = Scroll::last_top(total);
        let can_move = |delta: Vec2| {
            delta.x != 0.0
                || (delta.y < 0.0 && self.scroll.top < last)
                || (delta.y > 0.0 && self.scroll.top > 0.0)
        };
        if response.hovered() && total > VISIBLE_ROWS {
            let delta = ui.input(|i| i.smooth_scroll_delta);
            if delta != Vec2::ZERO && can_move(delta) {
                ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
                self.scroll.by(f64::from(-delta.y / row_height), total);
                let across = -delta.x / char_width;
                self.scroll.left = (self.scroll.left as f32 + across).max(0.0) as usize;
            }
        }
        let track = egui::Rect::from_min_max(
            egui::pos2(rect.right() - TRACK_WIDTH, rect.top()),
            rect.right_bottom(),
        );
        self.track(ui, track, total);
        let body =
            egui::Rect::from_min_max(rect.min, egui::pos2(track.left() - 2.0, rect.bottom()));
        let painter = ui.painter_at(body);
        painter.rect_filled(body, 2.0, ui.visuals().extreme_bg_color);
        let text_colour = ui.visuals().text_color();
        let gutter_colour = ui.visuals().weak_text_color();
        let first = self.scroll.first_row();
        let shift = ((self.scroll.top - first as f64) as f32) * row_height;
        for row in 0..=VISIBLE_ROWS {
            let n = first + row;
            let Some(line) = file.line(n) else { break };
            let y = body.top() + 2.0 + row as f32 * row_height - shift;
            if self.hit == Some(n) {
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(body.left(), y - 1.0),
                        Vec2::new(body.width(), row_height),
                    ),
                    0.0,
                    ui.visuals().selection.bg_fill,
                );
            }
            painter.text(
                egui::pos2(body.left() + gutter - char_width * 0.5, y),
                egui::Align2::RIGHT_TOP,
                (n + 1).to_string(),
                font.clone(),
                gutter_colour,
            );
            let shown: String = line.chars().skip(self.scroll.left).collect();
            painter.text(
                egui::pos2(body.left() + gutter + char_width * 0.5, y),
                egui::Align2::LEFT_TOP,
                shown,
                font.clone(),
                text_colour,
            );
        }
        let edge = if response.has_focus() {
            ui.visuals().selection.stroke
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke
        };
        ui.painter()
            .rect_stroke(body, 2.0, edge, egui::StrokeKind::Inside);
        let _: Color32 = text_colour;
    }

    fn keys(&mut self, ui: &egui::Ui, response: &egui::Response, total: usize) {
        if !response.has_focus() {
            return;
        }
        let page = (VISIBLE_ROWS - 1) as f64;
        let (mut up, mut down, mut goto) = (false, false, false);
        ui.input_mut(|i| {
            let none = egui::Modifiers::NONE;
            if i.consume_key(none, Key::ArrowDown) {
                self.scroll.by(1.0, total);
            }
            if i.consume_key(none, Key::ArrowUp) {
                self.scroll.by(-1.0, total);
            }
            if i.consume_key(none, Key::PageDown) {
                self.scroll.by(page, total);
            }
            if i.consume_key(none, Key::PageUp) {
                self.scroll.by(-page, total);
            }
            if i.consume_key(none, Key::Home) {
                self.scroll.top = 0.0;
                self.scroll.left = 0;
            }
            if i.consume_key(none, Key::End) {
                self.scroll.top = Scroll::last_top(total);
            }
            if i.consume_key(none, Key::ArrowRight) {
                self.scroll.left += 4;
            }
            if i.consume_key(none, Key::ArrowLeft) {
                self.scroll.left = self.scroll.left.saturating_sub(4);
            }
            goto = i.consume_key(egui::Modifiers::COMMAND, Key::G);
        });
        let _ = (&mut up, &mut down);
        if goto {
            self.focus_goto = true;
        }
    }

    fn goto_row(&mut self, ui: &mut egui::Ui, total: usize) {
        ui.horizontal(|ui| {
            ui.label("Go to line");
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.goto)
                    .desired_width(80.0)
                    .hint_text(format!("1 to {total}")),
            );
            if std::mem::take(&mut self.focus_goto) {
                edit.request_focus();
            }
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let go = ui.button("Go").clicked();
            if enter || go {
                match parse_line(&self.goto, total) {
                    Ok(line) => {
                        self.scroll.go_to(line, total);
                        self.problem = None;
                    }
                    Err(problem) => self.problem = Some(problem),
                }
            }
            if let Some(problem) = &self.problem {
                ui.label(RichText::new(problem).color(super::PROBLEM));
            }
        });
    }

    /// Find text: Enter or Next goes down, Shift+Enter or Previous goes up,
    /// wrapping round the file. Letter case counts.
    fn search_row(&mut self, ui: &mut egui::Ui, file: &PreviewFile) {
        ui.horizontal(|ui| {
            ui.label("Find");
            let edit = ui.add(egui::TextEdit::singleline(&mut self.needle).desired_width(140.0));
            let entered = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if entered {
                // Enter ends the edit; keep the box so Enter can go on.
                edit.request_focus();
            }
            let back_key = entered && ui.input(|i| i.modifiers.shift);
            let previous = ui.button("Previous").clicked() || back_key;
            let next = ui.button("Next").clicked() || (entered && !back_key);
            if next || previous {
                self.search(file, next);
            }
        });
    }

    /// Goes to the next (or previous) line holding the typed text and marks
    /// it, or says it is not there.
    pub fn search(&mut self, file: &PreviewFile, forward: bool) {
        match file.find(&self.needle, self.hit, forward) {
            Some(line) => {
                self.hit = Some(line);
                self.scroll.go_to(line + 1, file.line_count());
                self.problem = None;
            }
            None => {
                self.hit = None;
                self.problem = Some(if self.needle.is_empty() {
                    "Type the text to find.".to_owned()
                } else {
                    format!("'{}' is not in this file.", self.needle)
                });
            }
        }
    }

    /// The scroll bar: a track the thumb can be dragged along.
    fn track(&mut self, ui: &mut egui::Ui, track: egui::Rect, total: usize) {
        let response = ui.interact(
            track,
            ui.scope_id().with("preview track"),
            Sense::click_and_drag(),
        );
        let travel = Scroll::last_top(total);
        let visible = (VISIBLE_ROWS as f64 / total.max(1) as f64).min(1.0) as f32;
        let thumb_height = (track.height() * visible).max(18.0).min(track.height());
        let room = (track.height() - thumb_height).max(1.0);
        if let Some(pointer) = response
            .interact_pointer_pos()
            .filter(|_| (response.dragged() || response.clicked()) && travel > 0.0)
        {
            let at = ((pointer.y - track.top() - thumb_height / 2.0) / room).clamp(0.0, 1.0);
            self.scroll.top = f64::from(at) * travel;
        }
        let at = if travel > 0.0 {
            (self.scroll.top / travel) as f32
        } else {
            0.0
        };
        let thumb = egui::Rect::from_min_size(
            egui::pos2(track.left(), track.top() + at * room),
            Vec2::new(TRACK_WIDTH, thumb_height),
        );
        let painter = ui.painter();
        painter.rect_filled(track, 2.0, ui.visuals().faint_bg_color);
        painter.rect_filled(thumb, 2.0, ui.visuals().widgets.inactive.fg_stroke.color);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]
    use super::*;

    #[test]
    fn req_nfr_07_a_typed_line_is_checked_against_the_file() {
        assert_eq!(parse_line(" 12 ", 100), Ok(12));
        assert_eq!(parse_line("1", 1), Ok(1));
        assert!(parse_line("0", 100).unwrap_err().contains("1 to 100"));
        assert!(parse_line("101", 100).unwrap_err().contains("1 to 100"));
        assert!(parse_line("-3", 100).unwrap_err().contains("not a line"));
        assert!(parse_line("x", 100).unwrap_err().contains("'x'"));
        assert!(parse_line("", 100).unwrap_err().contains("Type"));
        assert!(parse_line("1", 0).unwrap_err().contains("no lines"));
    }

    #[test]
    fn req_nfr_03_the_position_stays_inside_the_file_and_holds_a_million_lines() {
        let mut scroll = Scroll::default();
        scroll.by(-5.0, 100);
        assert_eq!(scroll.top, 0.0);
        scroll.by(1e9, 100);
        assert_eq!(scroll.top, (100 - VISIBLE_ROWS) as f64);
        // One row of a ten-million-line file is still told apart.
        scroll.go_to(5_000_000, 10_000_000);
        let before = scroll.first_row();
        scroll.by(1.0, 10_000_000);
        assert_eq!(scroll.first_row(), before + 1);
        // A short file never scrolls.
        scroll.by(10.0, 3);
        assert_eq!(scroll.top, 0.0);
    }

    #[test]
    fn req_nfr_15_go_to_puts_the_line_in_view_near_the_middle() {
        let mut scroll = Scroll::default();
        scroll.go_to(500, 1000);
        let first = scroll.first_row();
        assert!((first..first + VISIBLE_ROWS).contains(&499));
        scroll.go_to(1, 1000);
        assert_eq!(scroll.top, 0.0);
        scroll.go_to(1000, 1000);
        assert_eq!(scroll.first_row(), 1000 - VISIBLE_ROWS);
    }
}
