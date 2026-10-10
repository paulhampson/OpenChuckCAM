mod ir;
mod machine;
mod oracle;
mod path;
mod post;
mod reader;
#[cfg(test)]
mod tests;

use std::fs;

use ir::Kind;
use machine::Machine;
use oracle::{ang_diff, Oracle};
use post::FeedMode;

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

pub fn oracle_for(m: &Machine) -> Oracle {
    Oracle { sigma: m.a_sign, axis_y: m.axis_yz[0], axis_z: m.axis_yz[1] }
}

/// Assumed LinuxCNC time for one replayed move, minutes (see post::g94_feed). UNCONFIRMED.
pub fn assumed_time_min(mv: &reader::Move) -> Option<f64> {
    let f = mv.f?;
    if mv.inverse_time {
        return Some(1.0 / f);
    }
    let d = dist(mv.from.xyz, mv.to.xyz);
    let da = (mv.to.a - mv.from.a).abs();
    Some(if d > 1e-9 { d / f } else { da / f })
}

fn main() -> Result<(), String> {
    let m = Machine::REFERENCE;
    let plan = path::plan(m.a_sign);
    fs::create_dir_all("out").map_err(|e| e.to_string())?;
    for (name, mode) in [("g94", FeedMode::Compensated), ("g93", FeedMode::InverseTime), ("naive", FeedMode::NaiveContactFeed)] {
        let p = post::post(&plan.segments, &m, mode);
        fs::write(format!("out/sp3_{name}.ngc"), &p.text).map_err(|e| e.to_string())?;
        let rep = reader::replay(&p.text)?;
        let o = oracle_for(&m);
        let mut max_dev: f64 = 0.0;
        let mut max_f32: f64 = 0.0;
        let mut max_a_step: f64 = 0.0;
        let (mut t_ir, mut t_replay) = (0.0, 0.0);
        let feed_moves: Vec<_> = rep.moves.iter().filter(|mv| mv.kind == reader::MoveKind::Feed).collect();
        let _ = feed_moves;
        for (mv, seg) in rep.moves.iter().zip(plan.segments.iter()) {
            let tip = o.tool_tip_in_model(mv.to.xyz[0], mv.to.xyz[1], mv.to.xyz[2], mv.to.a);
            max_dev = max_dev.max(dist(tip, seg.tip));
            let tip32 = o.tool_tip_in_model(mv.to_f32.xyz[0], mv.to_f32.xyz[1], mv.to_f32.xyz[2], mv.to_f32.a);
            max_f32 = max_f32.max(dist(tip32, seg.tip));
            max_a_step = max_a_step.max((mv.to.a - mv.from.a).abs());
            if let (Some(d), Some(t)) = (seg.duration_s, assumed_time_min(mv)) {
                if seg.kind == Kind::Cut {
                    t_ir += d;
                    t_replay += t * 60.0;
                }
            }
        }
        println!(
            "{name}: moves={} max_node_dev={:.2e} mm  max_dev_if_f32={:.2e} mm  max_A_step={:.3} deg  cut_time_ir={:.3}s replay={:.3}s ({:+.2}%)",
            rep.moves.len(), max_dev, max_f32, max_a_step, t_ir, t_replay, (t_replay / t_ir - 1.0) * 100.0
        );
        let _ = ang_diff(0.0, 0.0);
    }
    Ok(())
}
