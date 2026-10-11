// SPDX-License-Identifier: AGPL-3.0-or-later
//! In-house order keys (PRJ-05): variable-length base-62 strings compared
//! bytewise. A key never ends in the lowest digit, so there is always room
//! before it, and between any two distinct keys there is always another.
//!
//! Items sort by `(key, id)`, so two branches that insert at the same spot and
//! pick the same key still give one deterministic order after a merge.

use anyhow::{Result, bail};

const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: usize = 62;

fn val(c: u8) -> Result<usize> {
    match DIGITS.iter().position(|&d| d == c) {
        Some(v) => Ok(v),
        None => bail!("order key digit {:?} is not base 62", c as char),
    }
}

fn digit(v: usize) -> char {
    DIGITS.get(v).map_or('?', |&d| d as char)
}

/// Checks a stored key: non-empty, base 62, not ending in `0`.
pub fn validate(key: &str) -> Result<()> {
    if key.is_empty() {
        bail!("order key is empty");
    }
    for b in key.bytes() {
        val(b)?;
    }
    if key.ends_with('0') {
        bail!("order key {key:?} ends in 0");
    }
    Ok(())
}

/// A key strictly after `a` (append).
fn after(a: &[u8]) -> Result<String> {
    match a.split_first() {
        None => Ok(digit(BASE / 2).to_string()),
        Some((&c, rest)) => {
            let v = val(c)?;
            if v + 1 < BASE {
                Ok(digit(v + 1).to_string())
            } else {
                Ok(format!("{}{}", c as char, after(rest)?))
            }
        }
    }
}

/// A key strictly before `b` (prepend). `b` is a valid key.
fn before(b: &[u8]) -> Result<String> {
    match b.split_first() {
        None => bail!("no key before the empty key"),
        Some((&c, rest)) => {
            let v = val(c)?;
            if v > 1 {
                Ok(digit(v - 1).to_string())
            } else if !rest.is_empty() && v == 1 {
                // "1" is a proper prefix of b, so it sorts first.
                Ok("1".to_string())
            } else if rest.is_empty() {
                // b is "1": go one level down.
                Ok(format!("0{}", digit(BASE / 2)))
            } else {
                Ok(format!("0{}", before(rest)?))
            }
        }
    }
}

/// A key strictly between `a` and `b`, where `a < b`.
fn between(a: &[u8], b: &[u8]) -> Result<String> {
    // Shared prefix (a padded with zeros).
    let mut n = 0;
    while n < b.len() && a.get(n).copied().unwrap_or(b'0') == b[n] {
        n += 1;
    }
    if n > 0 {
        let prefix = std::str::from_utf8(&b[..n])?;
        let a_rest = a.get(n..).unwrap_or(&[]);
        return Ok(format!("{prefix}{}", between(a_rest, &b[n..])?));
    }
    let da = match a.first() {
        Some(&c) => val(c)?,
        None => 0,
    };
    let db = match b.first() {
        Some(&c) => val(c)?,
        None => bail!("upper key is exhausted"),
    };
    if db > da + 1 {
        return Ok(digit((da + db) / 2).to_string());
    }
    if b.len() > 1 {
        // b's first digit alone sorts before b and after a.
        return Ok(digit(db).to_string());
    }
    // Consecutive single digits: keep a's digit and go one level down.
    let a_rest = a.get(1..).unwrap_or(&[]);
    Ok(format!("{}{}", digit(da), after_or_mid(a_rest)?))
}

fn after_or_mid(a: &[u8]) -> Result<String> {
    if a.is_empty() { Ok(digit(BASE / 2).to_string()) } else { after(a) }
}

/// The key for an item placed between neighbours `lo` and `hi` (either may be
/// absent). Fails when `lo >= hi`; the caller then rekeys a neighbour.
pub fn key_between(lo: Option<&str>, hi: Option<&str>) -> Result<String> {
    let k = match (lo, hi) {
        (None, None) => digit(BASE / 2).to_string(),
        (Some(a), None) => after(a.as_bytes())?,
        (None, Some(b)) => before(b.as_bytes())?,
        (Some(a), Some(b)) => {
            if a >= b {
                bail!("order keys {a:?} and {b:?} are not increasing");
            }
            between(a.as_bytes(), b.as_bytes())?
        }
    };
    validate(&k)?;
    debug_assert!(lo.is_none_or(|a| a < k.as_str()) && hi.is_none_or(|b| k.as_str() < b));
    Ok(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny deterministic generator so the spike needs no rand crate.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: usize) -> usize {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n
        }
    }

    fn check_sorted(keys: &[String]) {
        for w in keys.windows(2) {
            assert!(w[0] < w[1], "{:?} !< {:?}", w[0], w[1]);
        }
        for k in keys {
            validate(k).unwrap();
        }
    }

    #[test]
    fn req_prj_05_random_inserts_stay_ordered() {
        let mut rng = Lcg(7);
        let mut keys: Vec<String> = Vec::new();
        for _ in 0..5000 {
            let i = rng.next(keys.len() + 1);
            let lo = if i == 0 { None } else { Some(keys[i - 1].as_str()) };
            let hi = keys.get(i).map(String::as_str);
            let k = key_between(lo, hi).unwrap();
            keys.insert(i, k);
        }
        check_sorted(&keys);
    }

    #[test]
    fn req_prj_05_adversarial_patterns_stay_ordered() {
        // Always append, always prepend, always insert after the first item.
        for pattern in 0..3 {
            let mut keys = vec![key_between(None, None).unwrap()];
            for _ in 0..2000 {
                match pattern {
                    0 => keys.push(key_between(keys.last().map(String::as_str), None).unwrap()),
                    1 => keys.insert(0, key_between(None, Some(&keys[0])).unwrap()),
                    _ => {
                        let hi = keys.get(1).map(String::as_str);
                        let k = key_between(Some(&keys[0]), hi).unwrap();
                        keys.insert(1, k);
                    }
                }
            }
            check_sorted(&keys);
        }
    }

    #[test]
    fn equal_keys_refuse() {
        assert!(key_between(Some("V"), Some("V")).is_err());
        assert!(key_between(Some("W"), Some("V")).is_err());
    }
}
