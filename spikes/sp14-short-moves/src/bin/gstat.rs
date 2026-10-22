// SPDX-License-Identifier: AGPL-3.0-or-later
//! Block-time table for G-code files: `gstat <file>...`. `.nc` files are timed
//! by the Makera rule, others by the LinuxCNC rule.
use sp14_short_moves::*;

fn main() {
    println!("file | {}", header());
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).unwrap();
        let rule = if path.ends_with(".nc") {
            Rule::Makera
        } else {
            Rule::Linuxcnc
        };
        let b = blocks(&text, rule);
        let name = std::path::Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        println!("{name} | {}", row(&stats(&b)));
    }
}
