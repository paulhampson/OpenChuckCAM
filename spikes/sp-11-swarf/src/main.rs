// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prints the SP-11 report (markdown) to stdout.
use sp11_swarf::cases::{TOL, cases};
use sp11_swarf::detect::{Family, detect};
use sp11_swarf::place::*;

fn main() {
    println!("# SP-11 report\n");
    println!(
        "Tolerance {TOL} mm. Detection: 9 lines x 17 samples per family. 91 poses, 15 ruling samples, 60 x 30 check grid.\n"
    );
    println!("## Detection\n");
    println!(
        "| Case | Ruling family | Residual along v (mm) | along u (mm) | Twist (deg) | Verdict |"
    );
    println!("| --- | --- | --- | --- | --- | --- |");
    let all = cases();
    for c in &all {
        let d = detect(c.surface.as_ref(), TOL, 9, 17);
        let (fam, tw) = match d.chosen {
            Some(f) => (format!("{:?}", f.family), format!("{:.3}", f.twist_deg)),
            None => ("-".into(), format!("{:.3}", d.best().twist_deg)),
        };
        let verdict = d
            .refusal()
            .map(|m| format!("refused: {m}"))
            .unwrap_or("ruled".into());
        println!(
            "| {} | {} | {:.6} | {:.6} | {} | {} |",
            c.id, fam, d.v.residual, d.u.residual, tw, verdict
        );
    }
    println!("\n## Placement and deviation (poses from fitted rulings)\n");
    println!(
        "| Case | Mode | Max tilt (deg) | A travel (deg) | Max A step (deg) | Gouge (mm) | Left (mm) |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for c in &all {
        let det = detect(c.surface.as_ref(), TOL, 9, 17);
        let fam = det.chosen.map(|f| f.family).unwrap_or(det.best().family);
        let plan = plan_for(c.surface.as_ref(), c.tool, fam, c.side, 17);
        for mode in [Mode::Ideal, Mode::Axis4] {
            match toolpath(c.surface.as_ref(), c.tool, plan, 91, mode, 17) {
                Ok(p) => {
                    let m = measure(c.surface.as_ref(), c.tool, &p, 60, 30);
                    let (tr, st) = a_travel(&p);
                    let tilt = p.iter().map(|x| x.tilt_deg).fold(0.0, f64::max);
                    println!(
                        "| {} | {:?} | {:.3} | {:.1} | {:.2} | {:.4} | {:.4} |",
                        c.id, mode, tilt, tr, st, m.gouge, m.left
                    );
                }
                Err(e) => println!("| {} | {:?} | - | - | - | {:?} | |", c.id, mode, e),
            }
        }
    }
    println!("\n## Convergence with pose count (Axis4)\n");
    println!("| Case | Poses | Gouge (mm) | Left (mm) |");
    println!("| --- | --- | --- | --- |");
    for id in [
        "cylinder",
        "cone, matching taper",
        "helicoid p=1.5",
        "spline bump 0.004",
    ] {
        let c = all.iter().find(|c| c.id == id).unwrap();
        let plan = plan_for(c.surface.as_ref(), c.tool, Family::AlongV, c.side, 17);
        for n in [11, 21, 41, 81, 161, 321, 641] {
            let p = toolpath(c.surface.as_ref(), c.tool, plan, n, Mode::Axis4, 17).unwrap();
            let m = measure(c.surface.as_ref(), c.tool, &p, 60, 30);
            println!("| {} | {} | {:.5} | {:.5} |", id, n, m.gouge, m.left);
        }
    }
    println!("\n## Detection residual against sampling density (mm)\n");
    println!("| Case | 5 x 9 | 9 x 17 | 17 x 33 | 33 x 65 |");
    println!("| --- | --- | --- | --- | --- |");
    for id in [
        "cylinder",
        "helicoid p=1.5",
        "spline bump 0.004",
        "spline bump 0.05",
        "sphere",
        "torus",
    ] {
        let c = all.iter().find(|c| c.id == id).unwrap();
        let r: Vec<String> = [(5, 9), (9, 17), (17, 33), (33, 65)]
            .iter()
            .map(|&(l, s)| {
                format!(
                    "{:.6}",
                    detect(c.surface.as_ref(), TOL, l, s).best().residual
                )
            })
            .collect();
        println!("| {} | {} |", id, r.join(" | "));
    }
    println!("\n## Helicoid pitch sweep (straight tool r 3, 641 poses, Axis4)\n");
    println!("| Pitch (mm/rad) | Twist (deg) | Gouge (mm) | Left (mm) |");
    println!("| --- | --- | --- | --- |");
    for pitch in [0.3, 1.5, 3.0, 6.0, 12.0] {
        let h = sp11_swarf::surface::Helicoid {
            pitch,
            u: (0.2, 2.9),
            v: (12.0, 30.0),
        };
        let tool = Tool {
            radius: 3.0,
            half_angle: 0.0,
        };
        let tw = detect(&h, TOL, 9, 17).v.twist_deg;
        let plan = plan_for(&h, tool, Family::AlongV, 1.0, 17);
        let p = toolpath(&h, tool, plan, 641, Mode::Axis4, 17).unwrap();
        let m = measure(&h, tool, &p, 60, 30);
        println!("| {} | {:.3} | {:.5} | {:.5} |", pitch, tw, m.gouge, m.left);
    }
}
