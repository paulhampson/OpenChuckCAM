// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simple arithmetic in number boxes (UI-26 as proposed in decision record
//! 0206; issue 183, trial feedback F-27).
//!
//! An entry may be a sum: numbers, `+ - * /`, brackets and a sign in front
//! of a number or bracket. Nothing else is read: no units (the box shows its
//! own), no percent, no functions, no variables. The sum is worked out in
//! double precision when the entry is confirmed, so the same entry always
//! gives the same number (GC-07, NFR-06). An entry that cannot be read or
//! worked out gives a short plain message that quotes it, never a panic
//! (NFR-07).

use std::fmt;

/// The longest entry read, in characters.
pub const MAX_LEN: usize = 200;

/// The deepest nesting of brackets and signs read.
const MAX_DEPTH: usize = 32;

/// Why an entry could not be turned into a number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// The entry is longer than [`MAX_LEN`].
    TooLong,
    /// A character that is not part of a sum.
    Character(char),
    /// A comma, which could be a decimal comma.
    Comma,
    /// A word or letter, such as a unit typed after the number.
    Word,
    /// The entry ends where a number or a bracket is expected ("12*").
    Incomplete,
    /// Something other than a number where one is expected ("*3").
    ExpectedNumber,
    /// Two numbers or a bracket and a number side by side ("1 2").
    MissingOperator,
    /// A bracket is opened and not closed.
    Unclosed,
    /// A bracket is closed that was not opened.
    Unopened,
    /// Brackets or signs are nested too deeply.
    TooDeep,
    /// A number that is not a valid number ("1.2.3").
    BadNumber,
    /// A division by zero.
    DivideByZero,
    /// The result is too large to be a number.
    TooLarge,
}

/// A fault with the entry it came from, shown as it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArithError {
    /// What went wrong.
    pub fault: Fault,
    /// The entry, as typed.
    pub entry: String,
}

impl ArithError {
    /// The entry, cut to about thirty characters for a short message.
    fn quoted(&self) -> String {
        let mut shown: String = self.entry.trim().chars().take(30).collect();
        if self.entry.trim().chars().count() > 30 {
            shown.push('\u{2026}');
        }
        shown
    }
}

impl fmt::Display for ArithError {
    /// A short plain message that quotes the entry and says what to type.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let q = self.quoted();
        match &self.fault {
            Fault::TooLong => write!(
                f,
                "Cannot read the entry; it is longer than {MAX_LEN} characters."
            ),
            Fault::Character(c) => write!(
                f,
                "Cannot read '{q}'; '{c}' is not allowed, use numbers with + - * / and brackets."
            ),
            Fault::Comma => write!(f, "Cannot read '{q}'; use a point for decimals."),
            Fault::Word => write!(
                f,
                "Cannot read '{q}'; type a number or a sum, the unit is shown beside the box."
            ),
            Fault::Incomplete | Fault::ExpectedNumber => {
                write!(f, "Cannot read '{q}'; expected a number or a sum.")
            }
            Fault::MissingOperator => {
                write!(f, "Cannot read '{q}'; put + - * or / between the numbers.")
            }
            Fault::Unclosed => write!(f, "Cannot read '{q}'; a bracket is not closed."),
            Fault::Unopened => write!(f, "Cannot read '{q}'; a bracket is not opened."),
            Fault::TooDeep => write!(f, "Cannot read '{q}'; the brackets are nested too deeply."),
            Fault::BadNumber => write!(f, "Cannot read '{q}'; a number in it is not valid."),
            Fault::DivideByZero => write!(f, "Cannot work out '{q}'; it divides by zero."),
            Fault::TooLarge => write!(f, "Cannot work out '{q}'; the result is too large."),
        }
    }
}

impl std::error::Error for ArithError {}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Open,
    Close,
}

fn tokens(entry: &str) -> Result<Vec<Token>, Fault> {
    if entry.chars().count() > MAX_LEN {
        return Err(Fault::TooLong);
    }
    let chars: Vec<char> = entry.chars().collect();
    let at = |k: usize| chars.get(k).copied();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(c) = at(i) {
        let simple = match c {
            '+' => Some(Token::Plus),
            '-' | '\u{2212}' => Some(Token::Minus),
            '*' | '\u{d7}' => Some(Token::Star),
            '/' | '\u{f7}' => Some(Token::Slash),
            '(' => Some(Token::Open),
            ')' => Some(Token::Close),
            _ => None,
        };
        if let Some(token) = simple {
            out.push(token);
            i += 1;
            continue;
        }
        match c {
            ' ' | '\t' | '\u{a0}' => i += 1,
            ',' => return Err(Fault::Comma),
            '0'..='9' | '.' => {
                let start = i;
                while at(i).is_some_and(|d| d.is_ascii_digit() || d == '.') {
                    i += 1;
                }
                // An exponent counts only when digits follow it, so "2e" and
                // "5em" read as a word, not as a number.
                if at(i).is_some_and(|e| e == 'e' || e == 'E') {
                    let mut j = i + 1;
                    if at(j).is_some_and(|s| s == '+' || s == '-') {
                        j += 1;
                    }
                    let digits = j;
                    while at(j).is_some_and(|d| d.is_ascii_digit()) {
                        j += 1;
                    }
                    if j > digits {
                        i = j;
                    }
                }
                let text: String = chars.iter().skip(start).take(i - start).collect();
                let value = text.parse::<f64>().map_err(|_| Fault::BadNumber)?;
                if !value.is_finite() {
                    return Err(Fault::TooLarge);
                }
                out.push(Token::Number(value));
            }
            c if c.is_alphabetic() || c == '\u{b0}' || c == '%' => return Err(Fault::Word),
            c => return Err(Fault::Character(c)),
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.at).copied()
    }

    /// sum = product { ("+" | "-") product }
    fn sum(&mut self, depth: usize) -> Result<f64, Fault> {
        let mut value = self.product(depth)?;
        while let Some(op @ (Token::Plus | Token::Minus)) = self.peek() {
            self.at += 1;
            let rhs = self.product(depth)?;
            value = if op == Token::Plus {
                value + rhs
            } else {
                value - rhs
            };
            check(value)?;
        }
        Ok(value)
    }

    /// product = factor { ("*" | "/") factor }
    fn product(&mut self, depth: usize) -> Result<f64, Fault> {
        let mut value = self.factor(depth)?;
        while let Some(op @ (Token::Star | Token::Slash)) = self.peek() {
            self.at += 1;
            let rhs = self.factor(depth)?;
            if op == Token::Star {
                value *= rhs;
            } else if rhs == 0.0 {
                return Err(Fault::DivideByZero);
            } else {
                value /= rhs;
            }
            check(value)?;
        }
        Ok(value)
    }

    /// factor = number | "(" sum ")" | ("+" | "-") factor
    fn factor(&mut self, depth: usize) -> Result<f64, Fault> {
        if depth > MAX_DEPTH {
            return Err(Fault::TooDeep);
        }
        let token = self.peek().ok_or(Fault::Incomplete)?;
        self.at += 1;
        match token {
            Token::Number(v) => Ok(v),
            Token::Plus => self.factor(depth + 1),
            Token::Minus => Ok(-self.factor(depth + 1)?),
            Token::Open => {
                let value = self.sum(depth + 1)?;
                match self.peek() {
                    Some(Token::Close) => {
                        self.at += 1;
                        Ok(value)
                    }
                    Some(_) => Err(Fault::MissingOperator),
                    None => Err(Fault::Unclosed),
                }
            }
            Token::Close => Err(Fault::ExpectedNumber),
            Token::Star | Token::Slash => Err(Fault::ExpectedNumber),
        }
    }
}

fn check(value: f64) -> Result<(), Fault> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Fault::TooLarge)
    }
}

/// Works out a sum typed in a number box.
///
/// # Errors
/// An [`ArithError`] naming what could not be read or worked out.
pub fn evaluate(entry: &str) -> Result<f64, ArithError> {
    let fail = |fault| ArithError {
        fault,
        entry: entry.to_owned(),
    };
    let tokens = tokens(entry).map_err(fail)?;
    let mut parser = Parser { tokens, at: 0 };
    let value = parser.sum(0).map_err(fail)?;
    match parser.peek() {
        None => Ok(value),
        Some(Token::Close) => Err(fail(Fault::Unopened)),
        Some(_) => Err(fail(Fault::MissingOperator)),
    }
}

/// `value` rounded to `places` decimals (none: as it is). A negative zero
/// reads as zero.
pub fn round_to(value: f64, places: Option<usize>) -> f64 {
    let Some(places) = places else {
        return value;
    };
    let rounded = format!("{value:.places$}").parse::<f64>().unwrap_or(value);
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// Turns what was typed into a number box into the text to apply. A plain
/// number is returned as typed, so what is stored never changes for an entry
/// that was already a number. A sum is worked out and rounded to `places`
/// decimals; only that number is returned, never the sum (issue 183).
///
/// # Errors
/// The plain message for an entry that cannot be read or worked out.
pub fn entry_text(typed: &str, places: Option<usize>) -> Result<String, String> {
    let typed = typed.trim();
    if typed.parse::<f64>().is_ok() {
        return Ok(typed.to_owned());
    }
    let value = evaluate(typed).map_err(|e| e.to_string())?;
    Ok(crate::field_row::number(round_to(value, places), places))
}

/// Reads what was typed into a number box as a number, working out a sum
/// (rounded to `places`), for the boxes that keep their own text.
///
/// # Errors
/// The plain message for an entry that cannot be read or worked out.
pub fn number(typed: &str, places: Option<usize>) -> Result<f64, String> {
    let text = entry_text(typed, places)?;
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| ArithError {
            fault: Fault::BadNumber,
            entry: typed.to_owned(),
        })
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]
    use super::*;

    fn ok(entry: &str) -> f64 {
        evaluate(entry).unwrap_or_else(|e| panic!("{entry}: {e}"))
    }

    fn fault(entry: &str) -> Fault {
        evaluate(entry).unwrap_err().fault
    }

    #[test]
    fn req_ui_07_the_examples_of_the_issue_are_worked_out() {
        assert_eq!(ok("12.5 + 3"), 15.5);
        assert_eq!(ok("60 / 2"), 30.0);
        assert_eq!(ok("20 * 0.5"), 10.0);
    }

    #[test]
    fn req_ui_07_multiplication_binds_tighter_and_brackets_override() {
        assert_eq!(ok("2 + 3 * 4"), 14.0);
        assert_eq!(ok("(2 + 3) * 4"), 20.0);
        assert_eq!(ok("10 - 4 - 3"), 3.0);
        assert_eq!(ok("100 / 5 / 2"), 10.0);
        assert_eq!(ok("2 * (3 + (4 - 1))"), 12.0);
    }

    #[test]
    fn req_ui_07_a_sign_may_stand_before_a_number_or_bracket() {
        assert_eq!(ok("-5"), -5.0);
        assert_eq!(ok("+5"), 5.0);
        assert_eq!(ok("3 * -2"), -6.0);
        assert_eq!(ok("-(1 + 2)"), -3.0);
        assert_eq!(ok("--4"), 4.0);
        assert_eq!(ok("\u{2212}4 \u{d7} 2 \u{f7} 8"), -1.0);
    }

    #[test]
    fn req_ui_07_numbers_may_have_a_leading_point_and_an_exponent() {
        assert_eq!(ok(".5 + .5"), 1.0);
        assert_eq!(ok("1e3 + 1"), 1001.0);
        assert_eq!(ok("2.5E-1"), 0.25);
    }

    #[test]
    fn req_ui_07_the_same_entry_always_gives_the_same_number() {
        let a = ok("0.1 + 0.2 * 3 / 7");
        let b = ok("0.1 + 0.2 * 3 / 7");
        assert_eq!(a.to_bits(), b.to_bits());
    }

    #[test]
    fn req_ui_07_req_nfr_07_each_unreadable_entry_has_its_own_fault() {
        assert_eq!(fault("12*"), Fault::Incomplete);
        assert_eq!(fault(""), Fault::Incomplete);
        assert_eq!(fault("*3"), Fault::ExpectedNumber);
        assert_eq!(fault("1 +* 2"), Fault::ExpectedNumber);
        assert_eq!(fault("1 2"), Fault::MissingOperator);
        assert_eq!(fault("(1 + 2"), Fault::Unclosed);
        assert_eq!(fault("1 + 2)"), Fault::Unopened);
        assert_eq!(fault("()"), Fault::ExpectedNumber);
        assert_eq!(fault("1.2.3"), Fault::BadNumber);
        assert_eq!(fault("5 mm"), Fault::Word);
        assert_eq!(fault("10%"), Fault::Word);
        assert_eq!(fault("90\u{b0}"), Fault::Word);
        assert_eq!(fault("2e"), Fault::Word);
        assert_eq!(fault("1,5"), Fault::Comma);
        assert_eq!(fault("3 ^ 2"), Fault::Character('^'));
        assert_eq!(fault("1 / 0"), Fault::DivideByZero);
        assert_eq!(fault("1 / (2 - 2)"), Fault::DivideByZero);
        assert_eq!(fault("1e308 * 10"), Fault::TooLarge);
        assert_eq!(fault("1e999"), Fault::TooLarge);
        assert_eq!(fault("1e308 + 1e308"), Fault::TooLarge);
    }

    #[test]
    fn req_ui_07_the_message_quotes_the_entry_in_plain_words() {
        let message = evaluate("12*").unwrap_err().to_string();
        assert_eq!(message, "Cannot read '12*'; expected a number or a sum.");
        let unit = evaluate("5 mm").unwrap_err().to_string();
        assert!(unit.contains("'5 mm'") && unit.contains("unit"));
        let zero = evaluate("4/0").unwrap_err().to_string();
        assert_eq!(zero, "Cannot work out '4/0'; it divides by zero.");
    }

    #[test]
    fn req_ui_07_every_message_names_the_entry_or_says_what_to_do() {
        let entries = [
            "12*", "*3", "1 2", "(1", "1)", "1.2.3", "5 mm", "1,5", "3^2", "1/0", "1e999",
        ];
        for entry in entries {
            let message = evaluate(entry).unwrap_err().to_string();
            assert!(message.starts_with("Cannot "), "{message}");
            assert!(message.ends_with('.'), "{message}");
            assert!(message.len() < 100, "{message}");
        }
        assert!(
            evaluate(&"1+".repeat(200))
                .unwrap_err()
                .to_string()
                .contains("longer than 200")
        );
        assert!(
            evaluate(&format!("{}1{}", "(".repeat(50), ")".repeat(50)))
                .unwrap_err()
                .to_string()
                .contains("nested too deeply")
        );
        assert!(
            evaluate("1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 +")
                .unwrap_err()
                .to_string()
                .contains('\u{2026}')
        );
    }

    #[test]
    fn req_ui_07_req_nfr_07_garbage_never_panics() {
        // Every string over a small alphabet up to length six, plus a few
        // hostile ones: all give a number or an error.
        let alphabet = [
            '1', '.', '+', '-', '*', '/', '(', ')', ' ', 'e', 'x', ',', '\u{0}',
        ];
        let mut count = 0usize;
        let mut stack = vec![String::new()];
        while let Some(entry) = stack.pop() {
            let _ = evaluate(&entry);
            count += 1;
            if entry.chars().count() < 5 {
                for c in alphabet {
                    stack.push(format!("{entry}{c}"));
                }
            }
        }
        assert!(count > 1000);
        for entry in [
            "\u{e9}",
            "((((((((((",
            "))))",
            &"9".repeat(5000),
            "-".repeat(100).as_str(),
        ] {
            let _ = evaluate(entry);
        }
    }

    #[test]
    fn req_ui_07_a_sum_is_rounded_to_the_places_of_the_field() {
        assert_eq!(round_to(10.0 / 3.0, Some(3)), 3.333);
        assert_eq!(round_to(2.0 / 3.0, Some(2)), 0.67);
        assert_eq!(round_to(1234.5678, Some(0)), 1235.0);
        assert_eq!(round_to(1.23456789, None), 1.23456789);
        assert!(round_to(-0.0001, Some(3)).is_sign_positive());
    }

    #[test]
    fn req_ui_07_only_the_worked_out_number_is_kept() {
        assert_eq!(entry_text("12.5 + 3", Some(3)).unwrap(), "15.500");
        assert_eq!(entry_text("10 / 3", Some(3)).unwrap(), "3.333");
        assert_eq!(entry_text("1000 / 3", Some(0)).unwrap(), "333");
        assert_eq!(entry_text("1 / 3", None).unwrap(), "0.3333333333333333");
        assert_eq!(entry_text(" (2+3)*4 ", Some(3)).unwrap(), "20.000");
    }

    #[test]
    fn req_ui_07_a_plain_number_is_kept_exactly_as_typed() {
        assert_eq!(entry_text("12.3456", Some(3)).unwrap(), "12.3456");
        assert_eq!(entry_text("-7", Some(3)).unwrap(), "-7");
        assert_eq!(entry_text("1e3", Some(0)).unwrap(), "1e3");
    }

    #[test]
    fn req_ui_07_boxes_with_their_own_text_read_a_sum_as_a_number() {
        assert_eq!(number("6 * 0.5", Some(3)).unwrap(), 3.0);
        assert_eq!(number(" 0.125 ", Some(3)).unwrap(), 0.125);
        assert!(number("inf", None).unwrap_err().starts_with("Cannot read"));
        assert!(number("6 *", None).is_err());
    }

    #[test]
    fn req_ui_07_an_unreadable_entry_gives_its_message() {
        assert_eq!(
            entry_text("12*", Some(3)).unwrap_err(),
            "Cannot read '12*'; expected a number or a sum."
        );
    }
}
