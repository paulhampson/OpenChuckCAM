// SPDX-License-Identifier: AGPL-3.0-or-later
//! Where the text labels of a drawing go so that none overlaps another
//! (UI-25, UI-16; interface specification 7.5 and 11.3; lane SL, issue 142).
//!
//! A label asks for a spot beside its feature. When another label already
//! holds that spot, it is moved to the nearest free one and joined to its
//! feature by a leader line (a callout). The function is pure: it works on
//! rectangles only, so it is tested without a display.
//!
//! The rule, in order:
//! 1. Labels are placed by priority ([`Priority`]), then by their position in
//!    the request list, so the same request always gives the same layout.
//! 2. A label keeps its preferred spot (shifted to stay inside the bounds)
//!    when that spot is free.
//! 3. Otherwise it takes the first free candidate round its anchor: eight
//!    directions at three distances, nearest distance first.
//! 4. Otherwise it takes the free slot of the bounds nearest its anchor.
//! 5. Otherwise a [`Priority::Essential`] label keeps its preferred spot (the
//!    only case where labels may overlap), and any other label is hidden.

use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};

/// Space kept free round every label, in pixels.
const PAD: f32 = 2.0;
/// Distances of the candidate spots from the anchor, in pixels.
const RINGS: [f32; 3] = [14.0, 34.0, 60.0];
/// A label nearer than this to its anchor has no leader line.
const LEADER_MIN: f32 = 10.0;
/// The step of the slot scan, in pixels.
const SCAN_STEP: f32 = 8.0;

/// How much a label matters; the lower variants are placed first and are the
/// last to be hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Text that never moves (a title, a chip); it only blocks the others.
    Fixed,
    /// A label that must always be visible: the dimensions, the safe Z, the
    /// travel limit, the axis and the angles.
    Essential,
    /// A label of a held region or a tab.
    Normal,
    /// A label of a default tab that is offered and not added.
    Low,
}

/// One label to place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    /// What the caller calls it, returned unchanged.
    pub id: usize,
    /// The point on the feature the label describes.
    pub anchor: Pos2,
    /// Where the label would go if nothing were in the way.
    pub preferred: Rect,
    /// How much it matters.
    pub priority: Priority,
}

/// Where a label went.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// The request's `id`.
    pub id: usize,
    /// The label's rectangle.
    pub rect: Rect,
    /// A line from the anchor to the nearest point of the rectangle, when
    /// the label has moved away from its feature.
    pub leader: Option<[Pos2; 2]>,
}

/// The result: the labels placed, in request order, and the ids of those
/// hidden for want of room.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// The labels shown.
    pub placed: Vec<Placed>,
    /// The ids hidden, in request order.
    pub hidden: Vec<usize>,
}

fn free(rect: Rect, bounds: Rect, taken: &[Rect]) -> bool {
    bounds.contains_rect(rect) && taken.iter().all(|t| !t.expand(PAD).intersects(rect))
}

/// `rect` moved by the least amount that puts it inside `bounds`.
fn shifted_inside(rect: Rect, bounds: Rect) -> Rect {
    let dx = (bounds.min.x - rect.min.x).max(0.0) + (bounds.max.x - rect.max.x).min(0.0);
    let dy = (bounds.min.y - rect.min.y).max(0.0) + (bounds.max.y - rect.max.y).min(0.0);
    rect.translate(vec2(dx, dy))
}

fn nearest_point(rect: Rect, p: Pos2) -> Pos2 {
    pos2(
        p.x.clamp(rect.min.x, rect.max.x),
        p.y.clamp(rect.min.y, rect.max.y),
    )
}

/// The candidate rectangles for a label of `size` round `anchor`.
fn candidates(anchor: Pos2, size: Vec2) -> impl Iterator<Item = Rect> {
    const DIRECTIONS: [(f32, f32); 8] = [
        (0.0, -1.0),
        (1.0, -1.0),
        (1.0, 0.0),
        (1.0, 1.0),
        (0.0, 1.0),
        (-1.0, 1.0),
        (-1.0, 0.0),
        (-1.0, -1.0),
    ];
    RINGS.into_iter().flat_map(move |d| {
        DIRECTIONS.into_iter().map(move |(x, y)| {
            let centre = anchor + vec2(x * (size.x / 2.0 + d), y * (size.y / 2.0 + d));
            Rect::from_center_size(centre, size)
        })
    })
}

/// The free slot of `bounds` nearest `anchor`, found by scanning rows.
fn nearest_slot(anchor: Pos2, size: Vec2, bounds: Rect, taken: &[Rect]) -> Option<Rect> {
    let mut best: Option<(f32, Rect)> = None;
    let mut y = bounds.min.y;
    while y + size.y <= bounds.max.y + 1e-3 {
        let mut x = bounds.min.x;
        while x + size.x <= bounds.max.x + 1e-3 {
            let rect = Rect::from_min_size(pos2(x, y), size);
            if free(rect, bounds, taken) {
                let d = (rect.center() - anchor).length_sq();
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, rect));
                }
            }
            x += SCAN_STEP;
        }
        y += SCAN_STEP;
    }
    best.map(|(_, r)| r)
}

/// Places `requests` inside `bounds`.
#[must_use]
pub fn place(bounds: Rect, requests: &[Request]) -> Layout {
    let mut order: Vec<(usize, &Request)> = requests.iter().enumerate().collect();
    order.sort_by_key(|&(i, r)| (r.priority, i));
    let mut taken: Vec<Rect> = Vec::new();
    let mut spots: Vec<Option<Rect>> = vec![None; requests.len()];
    for (i, r) in order {
        let spot = if r.priority == Priority::Fixed {
            Some(r.preferred)
        } else {
            let size = r.preferred.size();
            let wanted = shifted_inside(r.preferred, bounds);
            let found = std::iter::once(wanted)
                .chain(candidates(r.anchor, size))
                .find(|&c| free(c, bounds, &taken))
                .or_else(|| nearest_slot(r.anchor, size, bounds, &taken));
            found.or((r.priority == Priority::Essential).then_some(wanted))
        };
        if let Some(rect) = spot {
            taken.push(rect);
        }
        if let Some(slot) = spots.get_mut(i) {
            *slot = spot;
        }
    }
    let mut layout = Layout::default();
    for (r, spot) in requests.iter().zip(spots) {
        match spot {
            Some(rect) => {
                let end = nearest_point(rect, r.anchor);
                let leader = ((end - r.anchor).length() > LEADER_MIN
                    && r.priority != Priority::Fixed)
                    .then_some([r.anchor, end]);
                layout.placed.push(Placed {
                    id: r.id,
                    rect,
                    leader,
                });
            }
            None => layout.hidden.push(r.id),
        }
    }
    layout
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;

    /// The monospace label font at 11 pixels is about 6.6 wide and 13 tall.
    fn size_of(text: &str) -> Vec2 {
        vec2(6.6 * text.chars().count() as f32 + 4.0, 15.0)
    }

    fn above(id: usize, text: &str, anchor: Pos2, priority: Priority) -> Request {
        Request {
            id,
            anchor,
            preferred: Rect::from_min_size(
                pos2(
                    anchor.x - size_of(text).x / 2.0,
                    anchor.y - 4.0 - size_of(text).y,
                ),
                size_of(text),
            ),
            priority,
        }
    }

    /// The front pane's size (logical pixels) for a window: the project tree
    /// and the inspector take 560, the end view 270 and the rest of the
    /// chrome 230 in height (11.3).
    fn pane(window: (f32, f32), scale: f32) -> Rect {
        Rect::from_min_size(
            pos2(0.0, 0.0),
            vec2(window.0 / scale - 560.0 - 270.0, window.1 / scale - 230.0),
        )
    }

    const WINDOWS: [((f32, f32), f32); 4] = [
        ((1920.0, 1080.0), 1.0),
        ((1920.0, 1080.0), 1.2),
        ((1366.0, 768.0), 1.0),
        ((1366.0, 768.0), 1.2),
    ];

    fn assert_clean(bounds: Rect, layout: &Layout, context: &str) {
        for (i, a) in layout.placed.iter().enumerate() {
            assert!(
                bounds.contains_rect(a.rect),
                "{context}: {a:?} leaves {bounds:?}"
            );
            for b in &layout.placed[i + 1..] {
                assert!(
                    !a.rect.intersects(b.rect),
                    "{context}: {a:?} overlaps {b:?}"
                );
            }
        }
    }

    /// The blisk trial (issue 142): "Tab 1" sits above its tab and "Held
    /// 5.000" above the held region next to it, so they collide.
    fn blisk() -> Vec<Request> {
        vec![
            above(0, "Held 5.000", pos2(300.0, 120.0), Priority::Essential),
            above(1, "Tab 1", pos2(318.0, 124.0), Priority::Normal),
            above(2, "Safe Z +20", pos2(500.0, 60.0), Priority::Essential),
            above(3, "A axis", pos2(520.0, 200.0), Priority::Essential),
            above(4, "40.000", pos2(520.0, 192.0), Priority::Essential),
        ]
    }

    #[test]
    fn req_ui_25_the_blisk_labels_no_longer_overlap_and_the_moved_one_has_a_leader() {
        let bounds = pane((1920.0, 1080.0), 1.0);
        let requests = blisk();
        assert!(requests[0].preferred.intersects(requests[1].preferred));
        let layout = place(bounds, &requests);
        assert_clean(bounds, &layout, "blisk");
        assert!(layout.hidden.is_empty());
        // The essential label keeps its place; the tab's label is the one
        // that moves, and says what it belongs to with a leader line.
        assert_eq!(layout.placed[0].rect, requests[0].preferred);
        assert!(layout.placed[0].leader.is_none());
        let tab = layout.placed[1];
        assert_ne!(tab.rect, requests[1].preferred);
        let [from, to] = tab.leader.unwrap();
        assert_eq!(from, requests[1].anchor);
        assert!(tab.rect.contains(to));
    }

    #[test]
    fn req_ui_16_the_blisk_labels_fit_at_every_window_size_and_scale() {
        for (window, scale) in WINDOWS {
            let bounds = pane(window, scale);
            let layout = place(bounds, &blisk());
            assert_clean(bounds, &layout, &format!("{window:?} at {scale}"));
            assert!(layout.hidden.is_empty(), "{window:?} at {scale}");
        }
    }

    /// A label whose preferred spot is free stays there with no leader.
    #[test]
    fn req_ui_25_a_free_label_stays_where_it_was_asked_for() {
        let bounds = pane((1920.0, 1080.0), 1.0);
        let request = above(7, "Tab 1", pos2(200.0, 200.0), Priority::Normal);
        let layout = place(bounds, &[request]);
        assert_eq!(
            layout.placed,
            vec![Placed {
                id: 7,
                rect: request.preferred,
                leader: None
            }]
        );
    }

    /// A label asked for outside the pane is shifted in, not lost.
    #[test]
    fn req_ui_16_a_label_beyond_the_edge_is_shifted_inside() {
        let bounds = pane((1366.0, 768.0), 1.0);
        let request = above(
            0,
            "Z+ travel limit",
            pos2(bounds.max.x + 30.0, 1.0),
            Priority::Essential,
        );
        let layout = place(bounds, &[request]);
        assert_clean(bounds, &layout, "edge");
        assert_eq!(layout.placed.len(), 1);
    }

    /// Worst case: many tabs side by side with their labels over them.
    #[test]
    fn req_ui_25_many_tabs_never_overlap_and_the_essentials_stay() {
        for (window, scale) in WINDOWS {
            let bounds = pane(window, scale);
            for tabs in [1_usize, 4, 8, 16, 40] {
                let mut requests = vec![
                    above(1000, "Held 5.000", pos2(60.0, 120.0), Priority::Essential),
                    above(
                        1001,
                        "Safe Z +20",
                        pos2(bounds.max.x - 50.0, 40.0),
                        Priority::Essential,
                    ),
                    above(
                        1002,
                        "A axis",
                        pos2(bounds.max.x - 40.0, 200.0),
                        Priority::Essential,
                    ),
                ];
                for t in 0..tabs {
                    let text = format!("Tab {}", t + 1);
                    requests.push(above(
                        t,
                        &text,
                        pos2(70.0 + 9.0 * t as f32, 124.0),
                        Priority::Normal,
                    ));
                }
                let context = format!("{window:?} at {scale}, {tabs} tabs");
                let layout = place(bounds, &requests);
                assert_clean(bounds, &layout, &context);
                for id in 1000..1003 {
                    assert!(
                        layout.placed.iter().any(|p| p.id == id),
                        "{context}: {id} hidden"
                    );
                }
                if tabs <= 8 && window.0 / scale >= 1500.0 {
                    assert!(layout.hidden.is_empty(), "{context}: {:?}", layout.hidden);
                }
                assert_eq!(layout.placed.len() + layout.hidden.len(), requests.len());
            }
        }
    }

    /// When the pane is full the lower priorities go first, the last tab
    /// before the first, and an offered default tab before any tab.
    #[test]
    fn req_ui_25_when_the_pane_is_full_the_lowest_priority_labels_are_hidden_first() {
        let bounds = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 40.0));
        let mut requests = vec![above(
            0,
            "Held 5.000",
            pos2(60.0, 30.0),
            Priority::Essential,
        )];
        requests.push(above(1, "Tab 1", pos2(60.0, 30.0), Priority::Normal));
        requests.push(above(2, "Tab 2", pos2(60.0, 30.0), Priority::Normal));
        requests.push(above(
            3,
            "Default tab, not added",
            pos2(60.0, 30.0),
            Priority::Low,
        ));
        let layout = place(bounds, &requests);
        assert_clean(bounds, &layout, "full");
        assert!(layout.hidden.contains(&3), "{layout:?}");
        assert!(layout.placed.iter().any(|p| p.id == 0));
        // Nothing of higher priority is hidden while a lower one is shown.
        let shown = |id: usize| layout.placed.iter().any(|p| p.id == id);
        assert!(shown(1) || !shown(3), "{layout:?}");
        assert!(shown(2) || !shown(3), "{layout:?}");
    }

    /// An essential label is never hidden, even when it can only keep its
    /// own spot (the one case where labels may touch).
    #[test]
    fn req_ui_25_an_essential_label_is_never_hidden() {
        let bounds = Rect::from_min_size(pos2(0.0, 0.0), vec2(80.0, 16.0));
        let requests = [
            above(0, "Safe Z +20", pos2(40.0, 18.0), Priority::Essential),
            above(1, "A axis", pos2(40.0, 18.0), Priority::Essential),
        ];
        let layout = place(bounds, &requests);
        assert!(layout.hidden.is_empty());
        assert_eq!(layout.placed.len(), 2);
    }

    /// Fixed text blocks the others and does not move.
    #[test]
    fn req_ui_25_fixed_text_is_kept_clear() {
        let bounds = pane((1920.0, 1080.0), 1.0);
        let fixed = Request {
            id: 9,
            anchor: pos2(12.0, 12.0),
            preferred: Rect::from_min_size(pos2(12.0, 12.0), vec2(120.0, 15.0)),
            priority: Priority::Fixed,
        };
        let label = above(1, "Tab 1", pos2(70.0, 36.0), Priority::Normal);
        assert!(fixed.preferred.intersects(label.preferred));
        let layout = place(bounds, &[label, fixed]);
        assert_clean(bounds, &layout, "fixed");
        assert_eq!(layout.placed[1].rect, fixed.preferred);
        assert!(layout.placed[1].leader.is_none());
    }

    /// The same request gives the same layout, and the order of the result
    /// is the order of the request.
    #[test]
    fn req_ui_25_the_layout_is_deterministic_and_in_request_order() {
        let bounds = pane((1366.0, 768.0), 1.2);
        let mut requests = blisk();
        for t in 0..12 {
            let text = format!("Tab {}", t + 1);
            requests.push(above(
                10 + t,
                &text,
                pos2(100.0 + 7.0 * t as f32, 124.0),
                Priority::Normal,
            ));
        }
        let first = place(bounds, &requests);
        for _ in 0..5 {
            assert_eq!(place(bounds, &requests), first);
        }
        let ids: Vec<usize> = first.placed.iter().map(|p| p.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn req_ui_25_no_requests_give_an_empty_layout() {
        assert_eq!(place(pane((1920.0, 1080.0), 1.0), &[]), Layout::default());
    }
}
