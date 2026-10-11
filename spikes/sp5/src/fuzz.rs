// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deterministic mutation fuzz over the fixtures (no cargo-fuzz in a 1-day spike).

use crate::model::Reader;

pub struct FuzzResult {
    pub cases: usize,
    pub panics: usize,
    pub errors: usize,
    pub example: Option<String>,
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const ALPHABET: &[u8] = b"GMTXYZAFNIJKRPSHO#[]()%;/+-.0123456789 \t\n\r";

pub fn run(r: &dyn Reader, cases: usize, seed: u64) -> FuzzResult {
    let mut rng = Rng(seed);
    let seeds: Vec<&str> = crate::FIXTURES.iter().map(|(_, s)| *s).collect();
    let mut res = FuzzResult { cases, panics: 0, errors: 0, example: None };
    for _ in 0..cases {
        let base = seeds[rng.below(seeds.len())];
        // Take a few lines, then mutate bytes.
        let lines: Vec<&str> = base.lines().collect();
        let start = rng.below(lines.len());
        let take = 1 + rng.below(4);
        let mut bytes: Vec<u8> = lines[start..(start + take).min(lines.len())].join("\n").into_bytes();
        for _ in 0..1 + rng.below(4) {
            let i = rng.below(bytes.len() + 1);
            match rng.below(3) {
                0 if i < bytes.len() => bytes[i] = ALPHABET[rng.below(ALPHABET.len())],
                1 => bytes.insert(i, ALPHABET[rng.below(ALPHABET.len())]),
                _ if i < bytes.len() => {
                    bytes.remove(i);
                }
                _ => {}
            }
        }
        let src = String::from_utf8_lossy(&bytes).into_owned();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.read(&src))) {
            Err(_) => {
                res.panics += 1;
                if res.example.is_none() {
                    res.example = Some(src.replace('\n', "\\n"));
                }
            }
            Ok(Err(_)) => res.errors += 1,
            Ok(Ok(_)) => {}
        }
    }
    res
}
