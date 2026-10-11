// SPDX-License-Identifier: AGPL-3.0-or-later
//! Order keys (PRJ-05, decision record 0006 item 1).
//!
//! Each ordered item stores its own key, so moving or adding an item writes
//! one file, and two branches that change different items merge cleanly. A key
//! is a non-empty string of base-62 digits (`0-9`, `A-Z`, `a-z`) compared
//! bytewise, which never ends in `0`; so there is always a key before any key
//! and between any two different keys. Items sort by `(key, id)`, so two
//! branches that pick the same key for different items still merge to one
//! deterministic order.
//!
//! Keys grow when items are inserted again and again at one spot (about one
//! character per six inserts). [`renumber`] gives a list short, evenly spaced
//! keys; it touches every item, so it is an explicit command the user commits
//! on purpose, never automatic (OI-35).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::ProjectError;

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: usize = 62;
const MID: usize = BASE / 2;

/// Keys longer than this are reported, so the user can renumber.
pub const LONG_KEY: usize = 32;

/// An order key. See the module documentation.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OrderKey(String);

impl OrderKey {
    /// Checks and wraps `text`.
    pub fn new(text: impl Into<String>) -> Result<Self, ProjectError> {
        let text = text.into();
        let invalid = |reason| ProjectError::InvalidOrderKey {
            key: text.clone(),
            reason,
        };
        if text.is_empty() {
            return Err(invalid("it is empty"));
        }
        if !text.bytes().all(|b| value(b).is_some()) {
            return Err(invalid("it may only contain 0-9, A-Z and a-z"));
        }
        if text.ends_with('0') {
            return Err(invalid("it must not end in 0"));
        }
        Ok(Self(text))
    }

    /// The key as written in the project.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the key is long enough that the list should be renumbered.
    pub fn is_long(&self) -> bool {
        self.0.len() > LONG_KEY
    }

    /// A key strictly between `lo` and `hi`; either may be absent (the start or
    /// end of the list). Fails when `lo` is not less than `hi`.
    pub fn between(lo: Option<&Self>, hi: Option<&Self>) -> Result<Self, ProjectError> {
        let text = match (lo, hi) {
            (None, None) => digit(MID).to_string(),
            (Some(a), None) => after(a.0.as_bytes()),
            (None, Some(b)) => before(b.0.as_bytes()),
            (Some(a), Some(b)) => {
                if a >= b {
                    return Err(ProjectError::InvalidOrderKey {
                        key: b.0.clone(),
                        reason: "it does not come after the key before it",
                    });
                }
                between(a.0.as_bytes(), b.0.as_bytes())
            }
        };
        Self::new(text)
    }
}

impl Default for OrderKey {
    /// The middle key. Items built with it get a real key when they are added
    /// at a position.
    fn default() -> Self {
        Self(digit(MID).to_string())
    }
}

impl fmt::Display for OrderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for OrderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrderKey({})", self.0)
    }
}

impl TryFrom<String> for OrderKey {
    type Error = ProjectError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl From<OrderKey> for String {
    fn from(key: OrderKey) -> Self {
        key.0
    }
}

/// `count` short keys in increasing order, evenly spaced.
pub fn renumber(count: usize) -> Vec<OrderKey> {
    // The shortest length whose key space leaves room for every key.
    let mut length = 1;
    let mut space: u128 = BASE as u128;
    while space <= count as u128 {
        length += 1;
        space = space.saturating_mul(BASE as u128);
    }
    let step = space / (count as u128 + 1);
    (1..=count as u128)
        .map(|i| {
            let mut n = i * step;
            let mut text = vec![b'0'; length];
            for slot in text.iter_mut().rev() {
                *slot = DIGITS
                    .get((n % BASE as u128) as usize)
                    .copied()
                    .unwrap_or(b'0');
                n /= BASE as u128;
            }
            // Dropping trailing zeros keeps the order: zero is the lowest digit.
            while text.last() == Some(&b'0') {
                text.pop();
            }
            OrderKey(String::from_utf8_lossy(&text).into_owned())
        })
        .collect()
}

fn value(byte: u8) -> Option<usize> {
    DIGITS.iter().position(|&d| d == byte)
}

fn digit(v: usize) -> char {
    DIGITS.get(v).map_or('0', |&d| d as char)
}

/// A key strictly after `a`.
fn after(a: &[u8]) -> String {
    match a.split_first() {
        None => digit(MID).to_string(),
        Some((&c, rest)) => {
            let v = value(c).unwrap_or(0);
            if v + 1 < BASE {
                digit(v + 1).to_string()
            } else {
                format!("{}{}", c as char, after(rest))
            }
        }
    }
}

/// A key strictly before `b`, a valid key.
fn before(b: &[u8]) -> String {
    match b.split_first() {
        None => digit(MID).to_string(),
        Some((&c, rest)) => {
            let v = value(c).unwrap_or(0);
            if v > 1 {
                digit(v - 1).to_string()
            } else if v == 1 && !rest.is_empty() {
                // "1" is a proper prefix of b, so it sorts first.
                "1".to_owned()
            } else if rest.is_empty() {
                // b is "1": go one level down.
                format!("0{}", digit(MID))
            } else {
                format!("0{}", before(rest))
            }
        }
    }
}

/// A key strictly between `a` and `b`, where `a < b` and both are valid keys
/// (or `a` is a suffix left after a shared prefix).
fn between(a: &[u8], b: &[u8]) -> String {
    let shared = b
        .iter()
        .enumerate()
        .take_while(|&(i, &d)| a.get(i).copied().unwrap_or(b'0') == d)
        .count();
    if shared > 0 {
        let prefix = String::from_utf8_lossy(b.get(..shared).unwrap_or_default());
        let a_rest = a.get(shared..).unwrap_or_default();
        let b_rest = b.get(shared..).unwrap_or_default();
        return format!("{prefix}{}", between(a_rest, b_rest));
    }
    let da = a.first().and_then(|&c| value(c)).unwrap_or(0);
    let db = b.first().and_then(|&c| value(c)).unwrap_or(0);
    if db > da + 1 {
        return digit((da + db) / 2).to_string();
    }
    if b.len() > 1 {
        // b's first digit alone sorts before b and after a.
        return digit(db).to_string();
    }
    // Consecutive single digits: keep a's digit and go one level down.
    let a_rest = a.get(1..).unwrap_or_default();
    let below = if a_rest.is_empty() {
        digit(MID).to_string()
    } else {
        after(a_rest)
    };
    format!("{}{below}", digit(da))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use proptest::prelude::*;

    fn check_sorted(keys: &[OrderKey]) {
        for pair in keys.windows(2) {
            assert!(pair[0] < pair[1], "{:?} !< {:?}", pair[0], pair[1]);
        }
        for key in keys {
            OrderKey::new(key.as_str()).unwrap();
        }
    }

    fn insert(keys: &mut Vec<OrderKey>, i: usize) {
        let lo = i.checked_sub(1).and_then(|j| keys.get(j));
        let key = OrderKey::between(lo, keys.get(i)).unwrap();
        keys.insert(i, key);
    }

    proptest! {
        #[test]
        fn req_prj_05_random_inserts_stay_ordered(spots in proptest::collection::vec(any::<usize>(), 0..400)) {
            let mut keys = Vec::new();
            for spot in spots {
                let at = spot % (keys.len() + 1);
                insert(&mut keys, at);
            }
            check_sorted(&keys);
        }
    }

    #[test]
    fn req_prj_05_adversarial_inserts_stay_ordered() {
        // Always append, always prepend, always insert after the first item.
        for pattern in 0..3 {
            let mut keys = vec![OrderKey::default()];
            for _ in 0..2000 {
                let i = match pattern {
                    0 => keys.len(),
                    1 => 0,
                    _ => 1,
                };
                insert(&mut keys, i);
            }
            check_sorted(&keys);
        }
    }

    #[test]
    fn req_prj_05_renumber_gives_short_ordered_keys() {
        for count in [0, 1, 2, 61, 62, 63, 1000, 5000] {
            let keys = renumber(count);
            assert_eq!(keys.len(), count);
            check_sorted(&keys);
            let longest = keys.iter().map(|k| k.as_str().len()).max().unwrap_or(0);
            assert!(longest <= 3, "{count} keys need {longest} characters");
            // There is room around every renumbered key.
            if let (Some(first), Some(last)) = (keys.first(), keys.last()) {
                OrderKey::between(None, Some(first)).unwrap();
                OrderKey::between(Some(last), None).unwrap();
            }
        }
    }

    #[test]
    fn req_nfr_07_bad_order_keys_are_errors() {
        for bad in ["", "V0", "a-b", "é"] {
            assert!(OrderKey::new(bad).is_err(), "{bad:?} accepted");
        }
        let v = OrderKey::new("V").unwrap();
        assert!(OrderKey::between(Some(&v), Some(&v)).is_err());
        assert!(OrderKey::between(Some(&OrderKey::new("W").unwrap()), Some(&v)).is_err());
    }
}
