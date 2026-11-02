// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared field tracking for form departures (UI-07, UI-10, UI-26;
//! decision 0233). This holds UI buffers only, never project values.

use std::collections::{BTreeMap, BTreeSet};
use std::{any::Any, sync::Arc};

use eframe::egui::{self, Id};

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: Id,
    pub label: String,
    pub checked: Result<String, String>,
    pub preflight: Option<Preflight>,
}

/// A dry run against a private copy of the enclosing value. Fields in the
/// same group share that copy, in the order Keep will confirm them. This
/// checks interacting constraints with the application's existing editors.
#[derive(Clone)]
pub struct Preflight(Arc<Check>);

type Candidates = BTreeMap<u64, Box<dyn Any>>;
type Check = dyn Fn(&mut Candidates, &str) -> Result<(), String> + Send + Sync;

impl std::fmt::Debug for Preflight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Preflight")
    }
}

pub fn preflight<T: Clone + Send + Sync + 'static>(
    group: Id,
    original: T,
    apply: impl Fn(&T, &str) -> Result<T, String> + Send + Sync + 'static,
) -> Preflight {
    Preflight(Arc::new(move |candidates, text| {
        let candidate = candidates
            .entry(group.value())
            .or_insert_with(|| Box::new(original.clone()));
        let Some(value) = candidate.downcast_mut::<T>() else {
            return Err("The form changed; confirm the fields again. Not applied.".to_owned());
        };
        *value = apply(value, text)?;
        Ok(())
    }))
}

pub fn validate(entries: &[Entry]) -> Result<(), (Id, String)> {
    let mut candidates = Candidates::new();
    for entry in entries {
        let text = entry
            .checked
            .as_ref()
            .map_err(|reason| (entry.id, reason.clone()))?;
        if let Some(check) = &entry.preflight {
            (check.0)(&mut candidates, text).map_err(|reason| (entry.id, reason))?;
        }
    }
    Ok(())
}

pub fn set_preflight(ctx: &egui::Context, id: Id, preflight: Option<Preflight>) {
    read(ctx, |s| {
        if let Some(entry) = s
            .fields
            .get_mut(&id.value())
            .and_then(|f| f.pending.as_mut())
        {
            entry.preflight = preflight;
        }
    });
}

#[derive(Clone)]
struct Field {
    id: Id,
    form: Id,
    layer: egui::LayerId,
    rect: egui::Rect,
    seen: u64,
    pending: Option<Entry>,
    sections: Vec<Id>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Frozen,
    /// Reveal pending sections and refresh every validator before any commit.
    PrepareKeep,
    Keep(Id),
}

#[derive(Clone, Default)]
struct State {
    form: Option<Id>,
    fields: BTreeMap<u64, Field>,
    mode: Mode,
    outside: bool,
    focus: Option<Id>,
    discard: BTreeSet<u64>,
    refused: BTreeMap<u64, String>,
    draft_original: BTreeMap<u64, String>,
    draft_errors: BTreeMap<u64, String>,
    next_ticket: u64,
    tickets: BTreeMap<u64, Option<bool>>,
    sections: Vec<Id>,
}

fn state_id() -> Id {
    Id::unique("unconfirmed form edits")
}

fn read<T>(ctx: &egui::Context, f: impl FnOnce(&mut State) -> T) -> T {
    ctx.data_mut(|d| f(d.get_temp_mut_or_default::<State>(state_id())))
}

/// Declares the enclosing form; nested dialogs temporarily replace it.
pub fn form<T>(ctx: &egui::Context, id: Id, draw: impl FnOnce() -> T) -> T {
    let previous = read(ctx, |s| s.form.replace(id));
    let out = draw();
    read(ctx, |s| s.form = previous);
    out
}

/// A field keeps its identity when conditional rows or inspection results
/// change the layout around it. Keys are unique within their enclosing form.
pub fn field_id(ui: &egui::Ui, kind: &str, key: &str) -> Id {
    read(ui.ctx(), |s| {
        s.form.unwrap_or_else(|| ui.layer_id().id).with((kind, key))
    })
}

/// Detects external focus loss before any widget or action is drawn. It
/// never stores or replays pointer events: the shell defers semantic actions.
pub fn begin(ctx: &egui::Context) -> Option<Id> {
    let pointer = ctx.input(|i| {
        i.pointer
            .any_pressed()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    let focused = ctx.memory(|m| m.focused());
    let frame = ctx.cumulative_pass_nr();
    let layer = pointer.and_then(|at| ctx.layer_id_at(at));
    read(ctx, |s| {
        s.fields
            .retain(|_, f| f.pending.is_some() || frame.saturating_sub(f.seen) <= 2);
        if s.mode == Mode::Normal
            && let Some(at) = pointer
        {
            let from = focused.and_then(|id| s.fields.get(&id.value()));
            let to = s.fields.values().find(|f| {
                Some(f.layer) == layer && frame.saturating_sub(f.seen) <= 2 && f.rect.contains(at)
            });
            s.outside = match (from, to) {
                (Some(from), Some(to)) => from.form != to.form,
                (Some(_), None) => true,
                _ => false,
            };
            if let (Some(from), Some(to)) = (from, to)
                && from.form != to.form
                && from.pending.is_some()
            {
                return Some(to.id);
            }
        }
        None
    })
}

pub fn mode(ctx: &egui::Context) -> Mode {
    read(ctx, |s| s.mode)
}

pub fn set_mode(ctx: &egui::Context, mode: Mode) {
    read(ctx, |s| {
        s.mode = mode;
        if mode == Mode::Normal {
            s.outside = false;
        }
    });
}

pub fn holds_focus_loss(ctx: &egui::Context) -> bool {
    read(ctx, |s| s.outside || s.mode != Mode::Normal)
}

/// Records every entry box, including unchanged ones needed to recognise
/// a field-to-field focus transfer within the same form.
pub fn track(
    ui: &egui::Ui,
    response: &egui::Response,
    label: &str,
    pending: Option<Result<String, String>>,
) {
    let frame = ui.ctx().cumulative_pass_nr();
    read(ui.ctx(), |s| {
        s.fields.insert(
            response.id.value(),
            Field {
                id: response.id,
                form: s.form.unwrap_or_else(|| ui.layer_id().id),
                layer: ui.layer_id(),
                rect: response.rect.intersect(ui.clip_rect()),
                seen: frame,
                pending: pending.map(|checked| Entry {
                    id: response.id,
                    label: label.to_owned(),
                    checked,
                    preflight: None,
                }),
                sections: s.sections.clone(),
            },
        );
    });
}

pub fn entries(ctx: &egui::Context) -> Vec<Entry> {
    read(ctx, |s| {
        let mut entries: Vec<Entry> = s
            .fields
            .values()
            .filter_map(|f| f.pending.clone())
            .collect();
        entries.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.value().cmp(&b.id.value())));
        entries
    })
}

pub fn is_pending(ctx: &egui::Context, id: Id) -> bool {
    read(ctx, |s| {
        s.fields
            .get(&id.value())
            .is_some_and(|f| f.pending.is_some())
    })
}

pub fn drawn(ctx: &egui::Context, id: Id) -> bool {
    let pass = ctx.cumulative_pass_nr();
    read(ctx, |s| {
        s.fields.get(&id.value()).is_some_and(|f| f.seen == pass)
    })
}

pub fn discard(ctx: &egui::Context) {
    read(ctx, |s| {
        for (id, field) in &mut s.fields {
            if field.pending.take().is_some() {
                s.discard.insert(*id);
            }
        }
        s.refused.clear();
    });
}

pub fn take_discard(ctx: &egui::Context, id: Id) -> bool {
    read(ctx, |s| s.discard.remove(&id.value()))
}

pub fn refuse(ctx: &egui::Context, id: Id, reason: String) {
    read(ctx, |s| {
        s.refused.insert(id.value(), reason);
    });
}

pub fn take_refusal(ctx: &egui::Context, id: Id) -> Option<String> {
    read(ctx, |s| s.refused.remove(&id.value()))
}

pub fn focus(ctx: &egui::Context, id: Id) {
    read(ctx, |s| s.focus = Some(id));
}

pub fn take_focus(ctx: &egui::Context, id: Id) -> bool {
    read(ctx, |s| {
        if s.focus == Some(id) {
            s.focus = None;
            true
        } else {
            false
        }
    })
}

pub fn clear(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<State>(state_id()));
}

pub fn section<T>(ctx: &egui::Context, id: Id, draw: impl FnOnce() -> T) -> T {
    read(ctx, |s| s.sections.push(id));
    let out = draw();
    read(ctx, |s| {
        s.sections.pop();
    });
    out
}

pub fn needs_section(ctx: &egui::Context, section: Id) -> bool {
    read(ctx, |s| match s.mode {
        Mode::PrepareKeep => s
            .fields
            .values()
            .any(|f| f.pending.is_some() && f.sections.contains(&section)),
        Mode::Keep(id) => s
            .fields
            .get(&id.value())
            .is_some_and(|f| f.sections.contains(&section)),
        _ => s
            .focus
            .and_then(|id| s.fields.get(&id.value()))
            .is_some_and(|f| f.sections.contains(&section)),
    })
}

/// A panel keeps its own typed semantic action; the shell owns the one
/// shared warning and resolves this ticket. No panel implements policy.
#[derive(Debug, Clone, Copy)]
pub struct Ticket(u64);

pub fn request_local(ctx: &egui::Context) -> Option<Ticket> {
    if entries(ctx).is_empty() {
        return None;
    }
    Some(read(ctx, |s| {
        s.next_ticket += 1;
        s.tickets.insert(s.next_ticket, None);
        Ticket(s.next_ticket)
    }))
}

pub fn local_requested(ctx: &egui::Context) -> bool {
    read(ctx, |s| s.tickets.values().any(Option::is_none))
}

pub fn resolve_local(ctx: &egui::Context, approved: bool) {
    read(ctx, |s| {
        for result in s.tickets.values_mut() {
            if result.is_none() {
                *result = Some(approved);
            }
        }
    });
}

pub fn local_result(ctx: &egui::Context, ticket: Ticket) -> Option<bool> {
    read(ctx, |s| {
        let result = s.tickets.get(&ticket.0).copied().flatten();
        if result.is_some() {
            s.tickets.remove(&ticket.0);
        }
        result
    })
}

/// Tracks a box whose text belongs to an enclosing draft (a path, a
/// dialog name or cutting-data row). Confirming it never saves that draft.
/// Returns true when the owner should process a confirmed value.
pub fn draft_text(
    ui: &egui::Ui,
    response: &egui::Response,
    label: &str,
    before: &str,
    text: &mut String,
    validate: impl FnOnce(&str) -> Result<String, String>,
) -> bool {
    let ctx = ui.ctx();
    let original = read(ctx, |s| {
        if response.changed() {
            s.draft_errors.remove(&response.id.value());
            s.draft_original
                .entry(response.id.value())
                .or_insert_with(|| before.to_owned());
        }
        s.draft_original.get(&response.id.value()).cloned()
    });
    // Returning to the original text ends this edit too. The same dialog
    // widget may next edit another item, whose confirmed value is different.
    if original.as_ref().is_some_and(|old| old == text) {
        read(ctx, |s| {
            s.draft_original.remove(&response.id.value());
        });
    }
    if take_discard(ctx, response.id) {
        if let Some(original) = &original {
            text.clone_from(original);
        }
        read(ctx, |s| {
            s.draft_original.remove(&response.id.value());
            s.draft_errors.remove(&response.id.value());
        });
        track(ui, response, label, None);
        return false;
    }
    if take_focus(ctx, response.id) {
        response.request_focus();
    }
    let explicit = ui.input(|i| i.key_pressed(egui::Key::Enter));
    let normal = mode(ctx) == Mode::Normal;
    let escape = normal && response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
    if escape && let Some(original) = &original {
        text.clone_from(original);
    }
    let confirmed = mode(ctx) == Mode::Keep(response.id)
        || (normal && response.lost_focus() && (explicit || !holds_focus_loss(ctx)));
    let checked = validate(text);
    if confirmed && let Ok(value) = &checked {
        text.clone_from(value);
    }
    if (confirmed && checked.is_ok()) || escape {
        read(ctx, |s| {
            s.draft_original.remove(&response.id.value());
            s.draft_errors.remove(&response.id.value());
        });
    }
    let pending = (!escape
        && original
            .as_ref()
            .is_some_and(|old| old.as_str() != text.as_str())
        && (!confirmed || checked.is_err()))
    .then_some(checked);
    track(ui, response, label, pending);
    confirmed && !escape
}

/// A plain text field of a dialog draft, with the same registration and
/// restoration as the inspector's shared field row.
pub fn text_edit(ui: &mut egui::Ui, text: &mut String, label: &str) -> egui::Response {
    checked_text_edit(ui, text, label, |text| Ok(text.to_owned()))
}

pub fn name_edit(ui: &mut egui::Ui, text: &mut String, label: &str) -> egui::Response {
    checked_text_edit(ui, text, label, |text| {
        if text.trim().is_empty() {
            Err("Give the item a name. Not applied.".to_owned())
        } else {
            Ok(text.to_owned())
        }
    })
}

fn checked_text_edit(
    ui: &mut egui::Ui,
    text: &mut String,
    label: &str,
    validate: impl FnOnce(&str) -> Result<String, String>,
) -> egui::Response {
    let before = text.clone();
    // Conditional controls can change auto IDs, stranding the original
    // draft even when its field is shown again. These dialog labels are
    // unique within their form, independent of the current layout.
    let id = field_id(ui, "draft field", label);
    let response = ui.add(crate::theme::single_line(ui, text).id(id));
    draft_text(ui, &response, label, &before, text, validate);
    if let Some(reason) = take_refusal(ui.ctx(), response.id) {
        read(ui.ctx(), |s| {
            s.draft_errors.insert(response.id.value(), reason);
        });
    }
    if let Some(reason) = read(ui.ctx(), |s| {
        s.draft_errors.get(&response.id.value()).cloned()
    }) {
        ui.colored_label(crate::theme::DARK.error, reason);
    }
    response
}

pub fn take_local<T>(ctx: &egui::Context, pending: &mut Option<(Ticket, T)>) -> Option<T> {
    let (ticket, _) = pending.as_ref()?;
    let approved = local_result(ctx, *ticket)?;
    let (_, action) = pending.take()?;
    approved.then_some(action)
}

pub fn defer_local<T>(
    ctx: &egui::Context,
    pending: &mut Option<(Ticket, T)>,
    action: T,
) -> Option<T> {
    if let Some(ticket) = request_local(ctx) {
        *pending = Some((ticket, action));
        None
    } else {
        Some(action)
    }
}
