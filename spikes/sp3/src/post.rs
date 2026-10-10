//! Hard-coded LinuxCNC-dialect post (the throwaway stand-in for SP-10 / slice 6).
//! IR (model frame) -> machine XYZA via the shared model -> text.

use crate::ir::{Kind, Segment};
use crate::machine::Machine;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FeedMode {
    /// G94, units per minute, "compensated": F chosen so the contact point moves at the IR feed.
    Compensated,
    /// G93, inverse time: F = 1 / minutes for the move.
    InverseTime,
    /// G94 with the raw contact feed, ignoring rotation. A deliberately naive baseline.
    NaiveContactFeed,
}

fn fmt(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

const POS_DEC: usize = 4;
const A_DEC: usize = 4;
const F_DEC: usize = 3;

/// Feed word for a G94 move, given machine deltas and the IR duration.
/// Assumed LinuxCNC rule (UNCONFIRMED here, no simulator available): if any of X/Y/Z moves,
/// the feed applies to the XYZ path length; if only A moves, it is degrees per minute.
pub fn g94_feed(dxyz: f64, da: f64, t_s: f64) -> f64 {
    if dxyz > 1e-9 { dxyz / t_s * 60.0 } else { da.abs() / t_s * 60.0 }
}

pub struct Posted {
    pub text: String,
}

pub fn post(segments: &[Segment], m: &Machine, mode: FeedMode) -> Posted {
    let mut out = String::new();
    out.push_str("(SP-3 spike: wrapped engraving, throwaway; verify before running any machine, NFR-12)\n");
    out.push_str("G21 G90 G17 G54 G94\n");
    if mode == FeedMode::InverseTime {
        out.push_str("G93\n");
    }
    out.push_str("T1 M6\nS10000 M3\n");

    let mut cur: Option<([f64; 3], f64)> = None; // last *rounded* machine position, as the control sees it
    let mut motion = "";
    let mut last_f = String::new();
    for s in segments {
        let xyz = m.to_machine(s.tip, s.a_deg);
        let r = |v: f64, d: usize| fmt(v, d);
        let (x, y, z, a) = (r(xyz[0], POS_DEC), r(xyz[1], POS_DEC), r(xyz[2], POS_DEC), r(s.a_deg, A_DEC));
        let rounded = [x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0), z.parse().unwrap_or(0.0)];
        let a_r: f64 = a.parse().unwrap_or(0.0);
        let rapid = s.contact_feed.is_none();
        let want = if rapid { "G0" } else { "G1" };
        let mut line = String::new();
        if want != motion {
            line.push_str(want);
            motion = want;
        }
        let prev = cur;
        let push = |line: &mut String, w: &str| {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(w);
        };
        // Modal output: only words that changed (at output precision).
        let changed = |i: usize| prev.map_or(true, |(p, _)| fmt(p[i], POS_DEC) != [&x, &y, &z][i].as_str());
        if changed(0) { push(&mut line, &format!("X{x}")); }
        if changed(1) { push(&mut line, &format!("Y{y}")); }
        if changed(2) { push(&mut line, &format!("Z{z}")); }
        if prev.map_or(true, |(_, pa)| fmt(pa, A_DEC) != a) { push(&mut line, &format!("A{a}")); }
        if !rapid {
            let t = s.duration_s.unwrap_or(1.0);
            let f = match mode {
                FeedMode::NaiveContactFeed => s.contact_feed.unwrap_or(0.0),
                FeedMode::InverseTime => 60.0 / t,
                FeedMode::Compensated => {
                    let (p, pa) = prev.unwrap_or((rounded, a_r));
                    let d = ((rounded[0] - p[0]).powi(2) + (rounded[1] - p[1]).powi(2) + (rounded[2] - p[2]).powi(2)).sqrt();
                    g94_feed(d, a_r - pa, t)
                }
            };
            let fs = fmt(f, F_DEC);
            // G93 needs F on every block; G94 only when it changes.
            if mode == FeedMode::InverseTime || fs != last_f {
                push(&mut line, &format!("F{fs}"));
                last_f = fs;
            }
        }
        cur = Some((rounded, a_r));
        if !line.is_empty() {
            out.push_str(&line);
            out.push('\n');
        }
        let _ = Kind::Rapid;
    }
    out.push_str("M5\nM30\n");
    Posted { text: out }
}
