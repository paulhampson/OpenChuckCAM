//! A candidate *neutral machine program* (architecture 5.3, stage 1): motion, spindle, tool
//! change, comments, with machine-axis positions already computed by the shared kinematics.
//! Throwaway shape; slice 6 designs the real one.

use serde::Serialize;

use crate::sp3_ir::{Kind, Segment};
use crate::sp3_machine::Machine;

#[derive(Clone, Debug, Serialize)]
pub struct Item {
    /// "rapid", "feed" or "unwind" (whole turns removed from A, tool position unchanged).
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub a: f64,
    /// Contact-point feed in mm/min (None for rapids), and the move time in seconds.
    pub contact_feed: Option<f64>,
    pub duration_s: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tool {
    pub number: u32,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Program {
    pub title: String,
    pub tool: Tool,
    pub spindle_rpm: u32,
    pub items: Vec<Item>,
}

pub fn from_segments(segments: &[Segment], m: &Machine, title: &str) -> Program {
    let n = segments.len();
    let items = segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let xyz = m.to_machine(s.tip, s.a_deg);
            let rapid = s.contact_feed.is_none();
            let unwind = rapid && i == n - 1 && s.kind == Kind::Rapid;
            Item {
                kind: if unwind {
                    "unwind"
                } else if rapid {
                    "rapid"
                } else {
                    "feed"
                },
                x: xyz[0],
                y: xyz[1],
                z: xyz[2],
                a: s.a_deg,
                contact_feed: s.contact_feed,
                duration_s: s.duration_s,
            }
        })
        .collect();
    Program {
        title: title.to_string(),
        tool: Tool {
            number: 1,
            name: "SP-10 engraver".into(),
        },
        spindle_rpm: 10000,
        items,
    }
}
