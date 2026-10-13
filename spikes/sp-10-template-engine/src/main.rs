//! SP-10 throwaway spike. `cargo run` renders both posts into `out/` and prints the summary;
//! `cargo test` runs the pass-criteria checks.
#![allow(dead_code)]

#[path = "sp3_ir.rs"]
mod sp3_ir;
#[path = "sp3_machine.rs"]
mod sp3_machine;
#[path = "sp3_path.rs"]
mod sp3_path;
#[path = "sp3_post.rs"]
mod sp3_post;

mod engine;
mod post;
mod program;
#[cfg(test)]
mod tests;

use std::fs;

pub fn sp3_program() -> program::Program {
    let m = sp3_machine::Machine::REFERENCE;
    let plan = sp3_path::plan(m.a_sign);
    program::from_segments(
        &plan.segments,
        &m,
        "SP-3 spike: wrapped engraving, throwaway; verify before running any machine, NFR-12",
    )
}

pub fn render_post(
    prog: &program::Program,
    opt: &post::Options,
) -> Result<String, minijinja::Error> {
    let mut env = engine::environment();
    engine::render(
        &mut env,
        "rotary",
        post::ROTARY_POST,
        &post::Ctx { program: prog, opt },
    )
}

fn bench() {
    let o = post::makera();
    for n in [1_000usize, 10_000, 100_000, 1_000_000] {
        let base = sp3_program();
        let cut: Vec<_> = base
            .items
            .iter()
            .filter(|i| i.kind == "feed")
            .cloned()
            .collect();
        let mut items = vec![base.items[0].clone()];
        for i in 0..n {
            let mut it = cut[i % cut.len()].clone();
            it.x += (i / cut.len()) as f64 * 0.001;
            items.push(it);
        }
        let prog = program::Program { items, ..base };
        let t = std::time::Instant::now();
        let mut env = engine::environment();
        env.set_fuel(None);
        let out = engine::render(
            &mut env,
            "rotary",
            post::ROTARY_POST,
            &post::Ctx {
                program: &prog,
                opt: &o,
            },
        )
        .unwrap();
        if n == 100_000 {
            // Where the time goes: formatting only (a Rust stage would do modal state and feed).
            let lean = "{% for it in program.items %}G1 X{{ it.x|fixed(3) }} Y{{ it.y|fixed(3) }} Z{{ it.z|fixed(3) }} A{{ it.a|fixed(3) }}\n{% endfor %}";
            let t = std::time::Instant::now();
            let mut env = engine::environment();
            env.set_fuel(None);
            let out = engine::render(
                &mut env,
                "lean",
                lean,
                &post::Ctx {
                    program: &prog,
                    opt: &o,
                },
            )
            .unwrap();
            println!(
                "  formatting-only template, {n} moves: {:.1} ms ({} bytes), {:.2} us per move",
                t.elapsed().as_secs_f64() * 1e3,
                out.len(),
                t.elapsed().as_secs_f64() * 1e6 / n as f64
            );
            let t = std::time::Instant::now();
            let ctx = minijinja::value::Value::from(minijinja::value::Serde(&post::Ctx {
                program: &prog,
                opt: &o,
            }));
            println!(
                "  serialising the context alone: {:.1} ms",
                t.elapsed().as_secs_f64() * 1e3
            );
            drop(ctx);
        }
        println!(
            "{n:>9} moves: {:>8.1} ms, {} lines, {:.2} us per move",
            t.elapsed().as_secs_f64() * 1e3,
            out.lines().count(),
            t.elapsed().as_secs_f64() * 1e6 / n as f64
        );
    }
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("bench") {
        return bench();
    }
    fs::create_dir_all("out").unwrap();
    let prog = sp3_program();
    for (name, opt) in [
        ("linuxcnc_g94", post::sp3("g94_linuxcnc")),
        ("linuxcnc_g93", post::sp3("g93")),
        ("linuxcnc_naive", post::sp3("naive")),
        ("makera", post::makera()),
    ] {
        match render_post(&prog, &opt) {
            Ok(text) => {
                fs::write(format!("out/{name}.ngc"), &text).unwrap();
                println!(
                    "{name}: {} lines, {} bytes",
                    text.lines().count(),
                    text.len()
                );
            }
            Err(e) => println!("{name}: ERROR {e:#}"),
        }
    }
}
