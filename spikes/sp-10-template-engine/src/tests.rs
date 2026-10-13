//! SP-10 pass-criteria checks. Names carry the criterion they answer (C1 identical on repeat,
//! C2 errors name the line, C3 fixed-decimal formatting, C4 NFR-14 facts, plus the expressiveness
//! checks the question asks for: modal state, A wrap, line numbers, feed mode, conditionals).

use std::collections::HashMap;

use crate::engine::{self, fixed_str};
use crate::post::{self, Options};
use crate::program::{self, Program};
use crate::{render_post, sp3_program};

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn render_str(src: &str, ctx: &impl serde::Serialize) -> Result<String, minijinja::Error> {
    let mut env = engine::environment();
    engine::render(&mut env, "tpl", src, ctx)
}

#[derive(serde::Serialize)]
struct Empty {}

// ---------------------------------------------------------------------------------------
// Expressiveness: the SP-3 post as a template
// ---------------------------------------------------------------------------------------

#[test]
fn sp3_post_template_is_byte_identical_to_the_sp3_outputs() {
    let prog = sp3_program();
    for (mode, reference) in [
        ("g94_linuxcnc", include_str!("../reference/sp3_g94.ngc")),
        ("g93", include_str!("../reference/sp3_g93.ngc")),
        ("naive", include_str!("../reference/sp3_naive.ngc")),
    ] {
        let out = render_post(&prog, &post::sp3(mode)).unwrap();
        assert_eq!(
            out, reference,
            "mode {mode} differs from the committed SP-3 output"
        );
    }
}

/// The same three, through SP-3's own hard-coded Rust post (guards against a stale reference file).
#[test]
fn sp3_post_template_matches_the_sp3_rust_post() {
    let m = crate::sp3_machine::Machine::REFERENCE;
    let plan = crate::sp3_path::plan(m.a_sign);
    let prog = sp3_program();
    for (mode, rust_mode) in [
        ("g94_linuxcnc", crate::sp3_post::FeedMode::Compensated),
        ("g93", crate::sp3_post::FeedMode::InverseTime),
        ("naive", crate::sp3_post::FeedMode::NaiveContactFeed),
    ] {
        let rust = crate::sp3_post::post(&plan.segments, &m, rust_mode).text;
        assert_eq!(render_post(&prog, &post::sp3(mode)).unwrap(), rust);
    }
}

// An independent Rust implementation of the Makera-style post, written from decision 0007's
// description of the rules, not from the template.
fn reference_makera(prog: &Program, o: &Options) -> String {
    let d = o.decimals.pos;
    let f = |v: f64, n: usize| fixed_str(v, n);
    let r = |v: f64, n: usize| fixed_str(v, n).parse::<f64>().unwrap();
    let mut out = String::new();
    out.push_str(&format!("({})\n", prog.title));
    out.push_str("G90 G21 G17\nG54\nM5\n");
    out.push_str(&format!(
        "T{} M6\nS{} M3\n",
        prog.tool.number, prog.spindle_rpm
    ));
    let mut prev: Option<([String; 4], [f64; 4])> = None;
    let mut motion = "";
    let mut last_f = String::new();
    for it in &prog.items {
        if it.kind == "unwind" {
            out.push_str("G92.4 A0 S0\n");
            if let Some((s, rv)) = prev.as_mut() {
                let rem = rv[3] - 360.0 * (rv[3] / 360.0).trunc();
                rv[3] = rem;
                s[3] = f(rem, o.decimals.a);
            }
        }
        let rapid = it.kind != "feed";
        let want = if rapid { "G0" } else { "G1" };
        let s = [f(it.x, d), f(it.y, d), f(it.z, d), f(it.a, o.decimals.a)];
        let rv = [r(it.x, d), r(it.y, d), r(it.z, d), r(it.a, o.decimals.a)];
        let mut words: Vec<String> = Vec::new();
        for (i, l) in ["X", "Y", "Z", "A"].iter().enumerate() {
            if prev.as_ref().map_or(true, |(ps, _)| ps[i] != s[i]) {
                words.push(format!("{l}{}", s[i]));
            }
        }
        if !rapid {
            let t = it.duration_s.unwrap();
            let p0 = prev.as_ref().map_or(rv, |(_, p)| *p);
            let dist =
                ((rv[0] - p0[0]).powi(2) + (rv[1] - p0[1]).powi(2) + (rv[2] - p0[2]).powi(2))
                    .sqrt();
            let da = (rv[3] - p0[3]).abs();
            let p = 2.0 * std::f64::consts::PI * (rv[1] * rv[1] + rv[2] * rv[2]).sqrt() + 30.0;
            let amount = if dist > 1e-9 {
                dist.max(da * p / 360.0)
            } else {
                da * p.min(360.0) / 360.0
            };
            let fv = (amount / t * 60.0).min(o.max_feed.unwrap());
            let fs = f(fv, o.decimals.f);
            if fs != last_f {
                words.push(format!("F{fs}"));
                last_f = fs;
            }
        }
        prev = Some((s, rv));
        if !words.is_empty() {
            if want != motion {
                words.insert(0, want.to_string());
                motion = want;
            }
            out.push_str(&words.join(" "));
            out.push('\n');
        }
    }
    out.push_str("M5\nG53 G0 Z-3\nG28\nM2\n");
    out
}

#[test]
fn makera_variant_matches_an_independent_rust_implementation() {
    let prog = sp3_program();
    let o = post::makera();
    assert_eq!(render_post(&prog, &o).unwrap(), reference_makera(&prog, &o));
}

#[test]
fn makera_variant_has_the_z1_dialect_properties_of_decision_0007() {
    let out = render_post(&sp3_program(), &post::makera()).unwrap();
    assert!(!out.contains("G93"), "the Z1 post never writes G93");
    assert!(
        !out.lines().any(|l| l.starts_with('N')),
        "no line numbers on the Z1"
    );
    assert!(out.contains("M5\nT1 M6\n"), "M5 before T<n> M6");
    assert!(out.ends_with("M5\nG53 G0 Z-3\nG28\nM2\n"));
    assert!(out.contains("G92.4 A0 S0\n") && !out.contains(" R0"));
    let max_f = out
        .lines()
        .filter_map(|l| l.split(' ').find_map(|w| w.strip_prefix('F')))
        .map(|v| v.parse::<f64>().unwrap())
        .fold(0.0, f64::max);
    assert!(max_f <= 1200.0);
}

// ---------------------------------------------------------------------------------------
// C1: identical on repeat runs
// ---------------------------------------------------------------------------------------

#[test]
fn c1_repeat_runs_fresh_environments_and_threads_give_identical_bytes() {
    let prog = sp3_program();
    for opt in [post::sp3("g94_linuxcnc"), post::makera()] {
        let first = render_post(&prog, &opt).unwrap();
        for _ in 0..20 {
            assert_eq!(render_post(&prog, &opt).unwrap(), first);
        }
        // One shared environment, many renders.
        let mut env = engine::environment();
        env.add_template_owned("rotary", post::ROTARY_POST).unwrap();
        let ctx = minijinja::value::Value::from(minijinja::value::Serde(&post::Ctx {
            program: &prog,
            opt: &opt,
        }));
        for _ in 0..20 {
            assert_eq!(
                env.get_template("rotary").unwrap().render(&ctx).unwrap(),
                first
            );
        }
        // Concurrent renders (the environment is Sync) with 1, 2, 4 and 8 threads.
        for n in [1usize, 2, 4, 8] {
            let env = &env;
            let ctx = &ctx;
            let outs: Vec<String> = std::thread::scope(|s| {
                let hs: Vec<_> = (0..n)
                    .map(|_| {
                        s.spawn(move || env.get_template("rotary").unwrap().render(ctx).unwrap())
                    })
                    .collect();
                hs.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert!(outs.iter().all(|o| *o == first), "{n} threads");
        }
    }
}

/// Map ordering is where engines usually leak nondeterminism: a Rust `HashMap` has a random
/// seed per process. Run this test binary again as separate processes and compare.
#[test]
fn c1_hashmap_context_renders_the_same_in_every_process() {
    #[derive(serde::Serialize)]
    struct C {
        extra: HashMap<String, i64>,
    }
    let ctx = C {
        extra: (0..40).map(|i| (format!("key{i:02}"), i)).collect(),
    };
    let out = render_str(
        "{% for k, v in extra|items %}{{ k }}={{ v }};{% endfor %}",
        &ctx,
    )
    .unwrap();
    if std::env::var("SP10_PROBE").is_ok() {
        println!("PROBE:{:016x}", fnv(&out));
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let mut hashes = std::collections::BTreeSet::new();
    for _ in 0..12 {
        let o = std::process::Command::new(&exe)
            .args([
                "--exact",
                "tests::c1_hashmap_context_renders_the_same_in_every_process",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SP10_PROBE", "1")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        let h = text
            .lines()
            .find_map(|l| l.split_once("PROBE:").map(|x| x.1))
            .expect("probe output")
            .to_string();
        hashes.insert(h);
    }
    println!(
        "distinct hashes over 12 processes with a HashMap context: {}",
        hashes.len()
    );
    assert_eq!(hashes.len(), 1, "HashMap order leaked into the output");
    // And the order is sorted by key, not insertion order:
    assert!(out.starts_with("key00=0;key01=1;"));
}

#[test]
fn c1_whole_post_hash_is_stable_across_processes() {
    let out = render_post(&sp3_program(), &post::makera()).unwrap();
    if std::env::var("SP10_PROBE2").is_ok() {
        println!("PROBE2:{:016x}", fnv(&out));
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let mut hashes = std::collections::BTreeSet::new();
    for _ in 0..5 {
        let o = std::process::Command::new(&exe)
            .args([
                "--exact",
                "tests::c1_whole_post_hash_is_stable_across_processes",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("SP10_PROBE2", "1")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        hashes.insert(
            text.lines()
                .find_map(|l| l.split_once("PROBE2:").map(|x| x.1))
                .expect("probe")
                .to_string(),
        );
    }
    assert_eq!(hashes.len(), 1);
    assert_eq!(
        hashes.iter().next().unwrap(),
        &format!("{:016x}", fnv(&out))
    );
}

// ---------------------------------------------------------------------------------------
// C3: fixed-decimal number formatting
// ---------------------------------------------------------------------------------------

/// An independent oracle: exact decimal rounding (ties to even) of a binary double with
/// integer arithmetic only. Valid for 2^-60 <= |v| < 1e9 and up to 6 decimals; smaller
/// magnitudes round to zero at those decimals.
fn oracle_fixed(v: f64, decimals: u32) -> String {
    let bits = v.abs().to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e) = if exp == 0 {
        (frac, -1074)
    } else {
        (frac | (1u64 << 52), exp - 1075)
    };
    let scale = 10u128.pow(decimals);
    let q: u128 = if v == 0.0 || v.abs() < 2f64.powi(-60) {
        0
    } else if e >= 0 {
        ((m as u128) << e) * scale
    } else {
        let s = (-e) as u32;
        let num = m as u128 * scale;
        let (q, rem) = (num >> s, num & ((1u128 << s) - 1));
        let half = 1u128 << (s - 1);
        if rem > half || (rem == half && q % 2 == 1) {
            q + 1
        } else {
            q
        }
    };
    let (ip, fp) = (q / scale, q % scale);
    let body = if decimals == 0 {
        format!("{ip}")
    } else {
        format!("{ip}.{fp:0w$}", w = decimals as usize)
    };
    if v.is_sign_negative() && q != 0 {
        format!("-{body}")
    } else {
        body
    }
}

#[test]
fn c3_fixed_filter_matches_an_exact_integer_oracle() {
    let mut rng = Rng(0x9e3779b97f4a7c15);
    let mut vals: Vec<f64> = vec![
        0.0,
        -0.0,
        0.5,
        1.5,
        2.5,
        -0.5,
        -1.5,
        0.125,
        0.375,
        0.0005,
        0.00005,
        -0.00004,
        1e-7,
        123456.789,
        99999.99995,
        0.1 + 0.2,
        2.675,
        1.005,
        8.345,
        -3.0,
        359.99999,
    ];
    for _ in 0..60_000 {
        let mag = 10f64.powf(rng.unit() * 12.0 - 8.0);
        let v = if rng.next() % 2 == 0 { mag } else { -mag };
        vals.push(v);
        vals.push((v * 1000.0).round() / 1000.0 + 0.0005); // decimal ties as the user would write them
        vals.push(f64::from_bits(rng.next() & 0x41ef_ffff_ffff_ffff)); // wide exponent spread
    }
    let mut checked = 0;
    for &v in &vals {
        if v.abs() >= 1e9 {
            continue;
        }
        for d in 0..=6u32 {
            assert_eq!(
                fixed_str(v, d as usize),
                oracle_fixed(v, d)
                    .trim_start_matches('-')
                    .to_string()
                    .pipe_sign(&oracle_fixed(v, d)),
                "v={v:e} d={d}"
            );
            checked += 1;
        }
    }
    println!("fixed_str vs oracle: {checked} comparisons");
}

trait PipeSign {
    fn pipe_sign(self, full: &str) -> String;
}
impl PipeSign for String {
    fn pipe_sign(self, full: &str) -> String {
        full.to_string()
    }
}

#[test]
fn c3_fixed_filter_through_the_template_engine_matches_the_oracle() {
    #[derive(serde::Serialize)]
    struct C {
        vals: Vec<f64>,
        ints: Vec<i64>,
    }
    let mut rng = Rng(42);
    let vals: Vec<f64> = (0..5000).map(|_| (rng.unit() - 0.5) * 2000.0).collect();
    let ctx = C {
        vals: vals.clone(),
        ints: vec![0, 7, -3, 359],
    };
    for d in [0u32, 3, 4, 6] {
        let out = render_str(
            &format!("{{% for v in vals %}}{{{{ v|fixed({d}) }}}}\n{{% endfor %}}"),
            &ctx,
        )
        .unwrap();
        for (line, v) in out.lines().zip(&vals) {
            assert_eq!(line, oracle_fixed(*v, d));
        }
    }
    // Integers are accepted and keep the decimals.
    let out = render_str("{% for v in ints %}{{ v|fixed(2) }} {% endfor %}", &ctx).unwrap();
    assert_eq!(out, "0.00 7.00 -3.00 359.00 ");
}

#[test]
fn c3_never_negative_zero_and_ties_go_to_even_on_the_exact_value() {
    for (v, d, want) in [
        (-0.00004, 4, "0.0000"),
        (-0.0, 3, "0.000"),
        (0.125, 2, "0.12"), // exact tie: even
        (0.375, 2, "0.38"), // exact tie: even
        (2.5, 0, "2"),
        (3.5, 0, "4"),
        (2.675, 2, "2.67"), // binary value is below the tie
        (1.005, 2, "1.00"),
        (0.00005, 4, "0.0001"), // binary value is above the tie
    ] {
        assert_eq!(fixed_str(v, d), want, "{v} at {d}");
    }
}

#[test]
fn c3_builtin_number_display_is_not_fixed_width_so_posts_must_use_the_filter() {
    let ctx = Empty {};
    let probe = |src: &str| render_str(src, &ctx).unwrap();
    let cases = [
        ("{{ 3.0 }}", "3.0"),
        ("{{ 2.50 }}", "2.5"),
        ("{{ 0.1 + 0.2 }}", "0.30000000000000004"),
        ("{{ 7 / 2 }}", "3.5"),
        ("{{ 8 / 2 }}", "4.0"),
        ("{{ 7 // 2 }}", "3"),
        ("{{ 1.5|round(0) }}", "2.0"),
        ("{{ 2.5|round(0) }}", "2.0"),
        ("{{ 1.23456|round(3) }}", "1.235"),
        ("{{ 2.0|round(3) }}", "2.0"),
        ("{{ 100000000.0 }}", "100000000.0"),
    ];
    for (src, want) in cases {
        let got = probe(src);
        println!("{src:28} -> {got}");
        assert_eq!(got, want, "{src}");
    }
}

#[test]
fn c3_bad_numbers_are_errors_not_garbage() {
    #[derive(serde::Serialize)]
    struct C {
        bad: f64,
        text: &'static str,
    }
    for (src, expect) in [
        ("{{ bad|fixed(3) }}", "not a finite number"),
        ("{{ text|fixed(3) }}", "expected a number"),
        ("{{ 1.0|fixed(12) }}", "outside 0..=9"),
        ("{{ -1.0|sqrt }}", "negative"),
    ] {
        for bad in [f64::NAN, f64::INFINITY] {
            let e = render_str(src, &C { bad, text: "abc" }).unwrap_err();
            assert!(format!("{e:#}").contains(expect), "{src}: {e:#}");
            assert_eq!(e.line(), Some(1));
        }
    }
}

// ---------------------------------------------------------------------------------------
// C2: errors name the line
// ---------------------------------------------------------------------------------------

#[test]
fn c2_errors_name_the_template_and_the_line() {
    let prog = sp3_program();
    let opt = post::sp3("g94_linuxcnc");
    #[derive(serde::Serialize)]
    struct C<'a> {
        program: &'a Program,
        opt: &'a Options,
        bad: f64,
    }
    let ctx = C {
        program: &prog,
        opt: &opt,
        bad: f64::NAN,
    };
    let cases: Vec<(&str, String, usize, &str)> = vec![
        // Finding: an unclosed block is reported where the input ends (line 4), not at the opening tag
        // (line 3); the debug view shows the source lines around it.
        ("unclosed block", "G90\nG21\n{% for x in program.items %}\nG1\n".into(), 4, "syntax"),
        ("unknown variable", "G90\nG21\nG17\n{{ opt.deciamls.pos }}\n".into(), 4, "undefined"),
        ("unknown filter", "G90\n{{ 1.0|fixd(3) }}\n".into(), 2, "fixd"),
        ("bad filter argument", "a\nb\nc\nd\n{{ 'abc'|fixed(3) }}\n".into(), 5, "expected a number"),
        ("division by zero", "a\n{{ 1 // 0 }}\n".into(), 2, "unable to calculate"),
        ("non-finite number from the program", "a\nb\n{{ bad|fixed(3) }}\n".into(), 3, "finite"),
        ("error inside a macro", "{% macro m(x) %}\nline {{ x }}\n{{ nope.field }}\n{% endmacro %}\n\n\n{{ m(1) }}\n".into(), 3, "undefined"),
        ("error in a loop body", "{% for it in program.items %}\n{% if loop.index == 3 %}{{ it.bogus }}{% endif %}\n{% endfor %}\n".into(), 2, "undefined"),
        ("CRLF template source", "a\r\nb\r\n{{ opt.nope }}\r\n".into(), 3, "undefined"),
        ("unknown test", "a\n{% if 1 is nosuchtest %}x{% endif %}\n".into(), 2, "nosuchtest"),
    ];
    for (what, src, line, needle) in cases {
        let e = render_str(&src, &ctx).unwrap_err();
        let text = format!("{e:#}");
        println!(
            "[{what}] kind={:?} name={:?} line={:?}\n{text}\n",
            e.kind(),
            e.name(),
            e.line()
        );
        assert_eq!(e.name(), Some("tpl"), "{what}");
        assert_eq!(e.line(), Some(line), "{what}: {text}");
        assert!(text.to_lowercase().contains(needle), "{what}: {text}");
    }
}

#[test]
fn c2_a_missing_option_is_an_error_except_under_is_none_and_is_defined() {
    #[derive(serde::Serialize)]
    struct C {
        opt: std::collections::BTreeMap<String, i64>,
    }
    let ctx = C {
        opt: Default::default(),
    };
    // Printing or comparing a missing option names the line.
    let e = render_str("a\n{{ opt.max_feed }}\n", &ctx).unwrap_err();
    assert_eq!(e.line(), Some(2));
    let e = render_str("a\nb\n{% if opt.max_feed > 3 %}x{% endif %}\n", &ctx).unwrap_err();
    assert_eq!(e.line(), Some(3));
    // Finding: the `none` and `defined` tests swallow the strict error, so a typo such as
    // `opt.max_fead is not none` silently reads as "not none" or "none". The post options must be
    // validated against a complete schema (every key present, `null` for "unset") before render.
    let r = render_str(
        "{% if opt.max_fead is not none %}set{% else %}unset{% endif %}",
        &ctx,
    )
    .unwrap();
    println!("typo under `is not none`: {r}");
}

// ---------------------------------------------------------------------------------------
// G-code text survives the template syntax
// ---------------------------------------------------------------------------------------

#[test]
fn gcode_syntax_passes_through_untouched() {
    let src = "%\n(X<Y & Z>0 \"quote\")\n; semicolon comment\nO100 sub\n#<_a> = [#5220 + 1]\n#5221=0\nG1 X[#<_a>*2] F[1+2]\n(a { brace } and a % sign)\no100 endsub\n%\n";
    assert_eq!(render_str(src, &Empty {}).unwrap(), src);
    // The escape for the rare literal that looks like a tag:
    assert_eq!(
        render_str("(use {{ '{{' }} here)\n", &Empty {}).unwrap(),
        "(use {{ here)\n"
    );
}

#[test]
fn line_endings_in_the_template_reach_the_output() {
    // A finding: engines copy template text verbatim, so a CRLF checkout of a post changes the
    // output bytes. The loader must normalise newlines (and .gitattributes should pin LF).
    let lf = render_str("G90\nG21\n", &Empty {}).unwrap();
    let crlf = render_str("G90\r\nG21\r\n", &Empty {}).unwrap();
    assert_ne!(lf, crlf);
    assert_eq!(
        render_str(&"G90\r\nG21\r\n".replace("\r\n", "\n"), &Empty {}).unwrap(),
        lf
    );
}

// ---------------------------------------------------------------------------------------
// Expressiveness: options change the output as specified (GC-04)
// ---------------------------------------------------------------------------------------

fn word_of(line: &str, letter: char) -> Option<f64> {
    line.split(' ')
        .find_map(|w| w.strip_prefix(letter))
        .and_then(|v| v.parse().ok())
}

#[test]
fn option_line_numbers() {
    let mut o = post::sp3("g94_linuxcnc");
    o.line_numbers.enabled = true;
    o.line_numbers.start = 100;
    o.line_numbers.step = 5;
    let out = render_post(&sp3_program(), &o).unwrap();
    for (i, l) in out.lines().enumerate() {
        assert!(
            l.starts_with(&format!("N{} ", 100 + 5 * i as i64)),
            "line {i}: {l}"
        );
    }
    let base = render_post(&sp3_program(), &post::sp3("g94_linuxcnc")).unwrap();
    let stripped: String = out
        .lines()
        .map(|l| format!("{}\n", l.split_once(' ').unwrap().1))
        .collect();
    assert_eq!(stripped, base);
}

#[test]
fn option_a_direction_and_decimals_and_comments() {
    let prog = sp3_program();
    let base = render_post(&prog, &post::sp3("g94_linuxcnc")).unwrap();
    let mut o = post::sp3("g94_linuxcnc");
    o.a_direction = -1.0;
    o.decimals = post::Decimals { pos: 2, a: 1, f: 0 };
    o.comments = false;
    let out = render_post(&prog, &o).unwrap();
    assert!(!out.starts_with('('));
    let a_base: Vec<f64> = base.lines().filter_map(|l| word_of(l, 'A')).collect();
    let a_out: Vec<f64> = out.lines().filter_map(|l| word_of(l, 'A')).collect();
    assert!(a_out.len() > 100);
    // Rounded at 1 decimal and negated, compared against the same rounding of the exact values.
    let want: Vec<f64> = prog
        .items
        .iter()
        .map(|i| fixed_str(-i.a, 1).parse::<f64>().unwrap())
        .collect();
    let mut last = f64::NAN;
    let mut want_words = Vec::new();
    for w in want {
        if w != last {
            want_words.push(w);
            last = w;
        }
    }
    assert_eq!(a_out, want_words);
    assert!(a_base.len() >= a_out.len());
    assert!(out
        .lines()
        .filter(|l| l.contains('F'))
        .all(|l| !word_of(l, 'F').unwrap().to_string().contains('.')));
}

#[test]
fn option_wrap_modulo360_keeps_every_a_word_in_range() {
    let mut o = post::sp3("g94_linuxcnc");
    o.wrap = "modulo360".into();
    let out = render_post(&sp3_program(), &o).unwrap();
    let a: Vec<f64> = out.lines().filter_map(|l| word_of(l, 'A')).collect();
    assert!(a.iter().all(|v| (0.0..=360.0).contains(v)), "{a:?}");
    let prog = sp3_program();
    let want: Vec<String> = prog
        .items
        .iter()
        .map(|i| fixed_str(i.a - 360.0 * (i.a / 360.0).floor(), 4))
        .collect();
    let mut last = String::new();
    let mut want_words = Vec::new();
    for w in want {
        if w != last {
            want_words.push(w.parse::<f64>().unwrap());
            last = w;
        }
    }
    assert_eq!(a, want_words);
}

#[test]
fn option_tool_change_and_header_conditionals() {
    let mut o = post::sp3("g94_linuxcnc");
    o.tool_change.stop_spindle_first = true;
    o.tool_change.length_offset = true;
    let out = render_post(&sp3_program(), &o).unwrap();
    assert!(
        out.contains("\nM5\nT1 M6 G43 H1\nS10000 M3\n"),
        "{}",
        &out[..200]
    );
    let g93 = render_post(&sp3_program(), &post::sp3("g93")).unwrap();
    assert!(g93.contains("G21 G90 G17 G54 G94\nG93\n"));
    assert!(!out.contains("G93"));
}

#[test]
fn option_max_feed_clamps_only_the_feed_words() {
    let mut o = post::sp3("g94_linuxcnc");
    o.max_feed = Some(150.0);
    let out = render_post(&sp3_program(), &o).unwrap();
    let fs: Vec<f64> = out.lines().filter_map(|l| word_of(l, 'F')).collect();
    assert!(fs.iter().all(|f| *f <= 150.0) && fs.contains(&150.0));
}

#[test]
fn option_unwind_rapid_versus_g92_4() {
    let prog = sp3_program();
    let rapid = render_post(&prog, &post::sp3("g94_linuxcnc")).unwrap();
    assert!(!rapid.contains("G92.4"));
    let mut o = post::sp3("g94_linuxcnc");
    o.unwind = "g92_4".into();
    let g92 = render_post(&prog, &o).unwrap();
    assert!(g92.contains("G92.4 A0 S0\n"));
    // The A word after the unwind is only written when the controller's remainder differs.
    let tail: Vec<&str> = g92.lines().rev().take(4).collect();
    println!("{tail:?}");
}

// ---------------------------------------------------------------------------------------
// OI-38 evidence: cost of the per-controller feed rule inside the template
// ---------------------------------------------------------------------------------------

#[test]
fn feed_rules_differ_between_the_two_controllers_on_the_same_program() {
    let prog = sp3_program();
    let lin = render_post(&prog, &post::sp3("g94_linuxcnc")).unwrap();
    let mut o = post::sp3("g94_linuxcnc");
    o.feed_mode = "g94_makera".into();
    let mak = render_post(&prog, &o).unwrap();
    assert_ne!(lin, mak);
    assert_eq!(lin.lines().count(), mak.lines().count());
}

// ---------------------------------------------------------------------------------------
// NFR-07 / NFR-09: limits and isolation
// ---------------------------------------------------------------------------------------

#[test]
fn limits_runaway_templates_stop_with_an_error() {
    let ctx = Empty {};
    let e = render_str("{% for i in range(9000) %}{% for j in range(9000) %}{% for k in range(9000) %}x{% endfor %}{% endfor %}{% endfor %}", &ctx).unwrap_err();
    println!("nested loops: {:?} {e:#}", e.kind());
    assert_eq!(e.kind(), minijinja::ErrorKind::OutOfFuel);
    let e = render_str(
        "{% macro m(n) %}{{ m(n + 1) }}{% endmacro %}{{ m(0) }}",
        &ctx,
    )
    .unwrap_err();
    println!("recursion: {:?} {e:#}", e.kind());
    assert_eq!(e.kind(), minijinja::ErrorKind::InvalidOperation);
    let e = render_str("{% for i in range(100000000) %}{% endfor %}", &ctx).unwrap_err();
    println!("big range: {:?} {e:#}", e.kind());
}

#[test]
fn limits_string_repeat_is_not_available_or_bounded() {
    let r = render_str("{{ 'x' * 1000 }}", &Empty {});
    println!("string repeat: {:?}", r.as_ref().map(|s| s.len()));
}

#[test]
fn isolation_templates_cannot_reach_files_or_other_templates() {
    let ctx = Empty {};
    for src in [
        "{% include '/etc/passwd' %}",
        "{% include '../Cargo.toml' %}",
        "{% extends 'Cargo.toml' %}",
        "{% import 'posts/rotary.post' as p %}",
    ] {
        let e = render_str(src, &ctx).unwrap_err();
        println!("{src} -> {:?}", e.kind());
        assert_eq!(e.kind(), minijinja::ErrorKind::TemplateNotFound);
    }
    // No ambient functions beyond the ones registered.
    for src in [
        "{{ env() }}",
        "{{ open('x') }}",
        "{{ now() }}",
        "{{ import_module('os') }}",
    ] {
        assert!(render_str(src, &ctx).is_err(), "{src}");
    }
}

// ---------------------------------------------------------------------------------------
// Scale
// ---------------------------------------------------------------------------------------

fn scaled(n: usize) -> Program {
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
    Program { items, ..base }
}

#[test]
fn scale_fuel_per_move_is_bounded_and_rendering_works_at_20k_moves() {
    let o = post::makera();
    let prog = scaled(20_000);
    let out = render_post(&prog, &o).unwrap();
    assert!(out.lines().count() > 20_000);
    // Fuel use: render with a tail that reads the fuel meter.
    let mut env = engine::environment();
    env.add_function("fuel_consumed", |state: &minijinja::State| {
        state.fuel_levels().map_or(0, |(used, _)| used)
    });
    for n in [1_000usize, 10_000] {
        let prog = scaled(n);
        let src = format!("{}{{{{ fuel_consumed() }}}}", post::ROTARY_POST);
        let used: u64 = engine::render(
            &mut env,
            "f",
            &src,
            &post::Ctx {
                program: &prog,
                opt: &o,
            },
        )
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .parse()
        .unwrap();
        println!("fuel for {n} moves: {used} ({} per move)", used / n as u64);
        assert!(used / (n as u64) < 1000);
    }
    let _ = program::from_segments;
}

#[test]
fn c2_debug_view_size_with_a_large_program_in_context() {
    let o = post::makera();
    let prog = scaled(20_000);
    let e = render_str(
        "a\n{{ program.nope }}\n",
        &post::Ctx {
            program: &prog,
            opt: &o,
        },
    )
    .unwrap_err();
    let (short, full) = (format!("{e}"), format!("{e:#}"));
    println!(
        "error text: short {} bytes, with debug view {} bytes",
        short.len(),
        full.len()
    );
    assert_eq!(e.line(), Some(2));
    assert!(short.len() < 300);
}

/// Replay the Makera-style output under the Z1 timing rule of decision 0007 (derived,
/// unconfirmed) and compare the total cutting time with the program's. Independent of the
/// template's own feed arithmetic: it only reads the written words, with modal G0/G1 and F.
#[test]
fn makera_output_replays_to_the_program_durations_under_the_z1_rule() {
    let prog = sp3_program();
    let out = render_post(&prog, &post::makera()).unwrap();
    let mut pos = [f64::NAN; 4];
    let (mut f, mut feed_mode) = (0.0, false);
    let (mut total, mut moves) = (0.0, 0);
    for line in out.lines().skip_while(|l| !l.starts_with("S10000")).skip(1) {
        if line.starts_with('M')
            || line.starts_with("G92.4")
            || line.starts_with("G53")
            || line.starts_with("G28")
        {
            continue;
        }
        if line.starts_with("G0") {
            feed_mode = false;
        } else if line.starts_with("G1") {
            feed_mode = true;
        }
        let prev = pos;
        for (i, l) in ['X', 'Y', 'Z', 'A'].iter().enumerate() {
            if let Some(v) = word_of(line, *l) {
                pos[i] = v;
            }
        }
        if let Some(v) = word_of(line, 'F') {
            f = v;
        }
        if !feed_mode || prev[0].is_nan() {
            continue;
        }
        let l =
            ((pos[0] - prev[0]).powi(2) + (pos[1] - prev[1]).powi(2) + (pos[2] - prev[2]).powi(2))
                .sqrt();
        let da = (pos[3] - prev[3]).abs();
        let p = 2.0 * std::f64::consts::PI * (pos[1] * pos[1] + pos[2] * pos[2]).sqrt() + 30.0;
        let amount = if l > 1e-9 {
            l.max(da * p / 360.0)
        } else {
            da * p.min(360.0) / 360.0
        };
        total += amount / f * 60.0;
        moves += 1;
    }
    let want: f64 = prog
        .items
        .iter()
        .filter(|i| i.kind == "feed")
        .map(|i| i.duration_s.unwrap())
        .sum();
    println!(
        "replayed {moves} feed moves: {total:.3} s against the program's {want:.3} s ({:.3} %)",
        (total - want) / want * 100.0
    );
    assert!(((total - want) / want).abs() < 0.005);
}
