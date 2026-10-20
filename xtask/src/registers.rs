// SPDX-License-Identifier: AGPL-3.0-or-later
//! Register checks (decision 0130, OI-231): the open-items register and the decision index.
//!
//! Both files merge as unions (decision 0054, `.gitattributes`), so a row that two branches
//! changed, or that one branch moved while another edited it, comes back as a second copy
//! with no conflict to warn about. These checks find what a union merge leaves behind:
//!
//! - open-items register: a row has the right number of cells; an ID is used once in the
//!   open table and once in the closed table; no ID is in both tables; a row in the open
//!   table does not carry a closed state (it belongs in the closed table);
//! - decision index: one row per record, in number order, each linking to a record that exists.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::Findings;

/// The open-items register.
pub const OPEN_ITEMS: &str = "docs/open-items.md";

/// The decision index.
pub const DECISION_INDEX: &str = "docs/decisions/README.md";

/// Cells in an open-table row: ID, item, source, state, decide in, needs.
const OPEN_CELLS: usize = 6;

/// Cells in a closed-table row: ID, item, source, closed, settled in.
const CLOSED_CELLS: usize = 5;

/// Which table of the register a line is in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Table {
    Open,
    Closed,
}

/// A register row: its 1-based line and its cells without the outer bars.
struct Row {
    line: usize,
    cells: Vec<String>,
}

/// The cells of a Markdown table row, trimmed, without the outer bars.
fn cells(line: &str) -> Vec<String> {
    // Take off one outer bar at each end only: a row that ends `||` has an empty last cell.
    let line = line.trim();
    let inner = line.strip_prefix('|').unwrap_or(line);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_owned()).collect()
}

/// Whether `id` is `OI-` followed by digits, or `OI-C` and digits (the numbering of the
/// first closed items, `OI-C1` to `OI-C4`).
fn is_item_id(id: &str) -> bool {
    let number = id.strip_prefix("OI-").unwrap_or("");
    let number = number.strip_prefix('C').unwrap_or(number);
    !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
}

/// Checks the open-items register text.
pub fn check_open_items(text: &str) -> Findings {
    let mut findings = Findings::new();
    let mut table = Table::Open;
    let mut seen_open: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen_closed: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows: Vec<(Table, Row)> = Vec::new();

    for (index, line) in text.lines().enumerate() {
        if line.starts_with("## ") {
            table = if line.starts_with("## Closed") {
                Table::Closed
            } else {
                Table::Open
            };
        }
        if line.starts_with("| OI-") {
            rows.push((
                table,
                Row {
                    line: index + 1,
                    cells: cells(line),
                },
            ));
        }
    }

    for (table, row) in &rows {
        let id = row.cells.first().map_or("", String::as_str);
        let (name, want) = match table {
            Table::Open => ("open", OPEN_CELLS),
            Table::Closed => ("closed", CLOSED_CELLS),
        };
        let at = |line: usize| format!("{OPEN_ITEMS}:{line}");
        if !is_item_id(id) {
            findings.push(format!("{}: {id:?} is not an OI- number", at(row.line)));
            continue;
        }
        if *table == Table::Closed && row.cells.len() == OPEN_CELLS {
            findings.push(format!(
                "{}: {id} has the six cells of an open-table row (state {:?}) but is in the closed table; an appended row can land there. Move it to the open table, or rewrite it with the five closed columns",
                at(row.line),
                row.cells.get(3).map_or("", String::as_str)
            ));
        } else if row.cells.len() != want {
            findings.push(format!(
                "{}: {id} has {} cells, the {name} table has {want} (a `|` inside a cell, or a half-merged row)",
                at(row.line),
                row.cells.len()
            ));
        }
        let slot = match table {
            Table::Open => &mut seen_open,
            Table::Closed => &mut seen_closed,
        };
        if let Some(first) = slot.get(id) {
            findings.push(format!(
                "{}: {id} is in the {name} table again (first at line {first}); a union merge keeps both versions of a row that two branches edited, so keep the newer one and delete the other",
                at(row.line)
            ));
        } else {
            slot.insert(id.to_owned(), row.line);
        }
        if *table == Table::Open
            && row
                .cells
                .get(3)
                .is_some_and(|state| state.to_ascii_lowercase().starts_with("closed"))
        {
            findings.push(format!(
                "{}: {id} has a closed state but is in the open table; move the row to the closed table",
                at(row.line)
            ));
        }
    }

    for (id, open_line) in &seen_open {
        if let Some(closed_line) = seen_closed.get(id) {
            findings.push(format!(
                "{OPEN_ITEMS}:{open_line}: {id} is in both tables (closed at line {closed_line}); delete the open row, or reopen the item and delete the closed row"
            ));
        }
    }
    findings
}

/// The record number of an index row, such as `| [0054](0054-x.md) | ...`, and its link target.
fn index_row(line: &str) -> Option<(u32, String)> {
    let rest = line.strip_prefix("| [")?;
    let (number, rest) = rest.split_once("](")?;
    let (target, _) = rest.split_once(')')?;
    if number.len() != 4 {
        return None;
    }
    Some((number.parse().ok()?, target.to_owned()))
}

/// Checks the decision index text against the record files that exist (`NNNN-title.md`).
pub fn check_decision_index(text: &str, files: &[String]) -> Findings {
    let mut findings = Findings::new();
    let mut last: Option<(u32, usize)> = None;
    let mut rows: BTreeMap<u32, usize> = BTreeMap::new();

    for (index, line) in text.lines().enumerate() {
        let Some((number, target)) = index_row(line) else {
            continue;
        };
        let line_no = index + 1;
        if let Some(first) = rows.get(&number) {
            findings.push(format!(
                "{DECISION_INDEX}:{line_no}: record {number:04} has a second row (first at line {first})"
            ));
        } else {
            rows.insert(number, line_no);
        }
        if let Some((before, before_line)) = last
            && number < before
        {
            findings.push(format!(
                "{DECISION_INDEX}:{line_no}: record {number:04} is after record {before:04} (line {before_line}); keep the rows in number order"
            ));
        }
        last = Some((number, line_no));
        if !files.contains(&target) {
            findings.push(format!(
                "{DECISION_INDEX}:{line_no}: record {number:04} links to {target}, which does not exist"
            ));
        } else if target.get(..4).and_then(|n| n.parse::<u32>().ok()) != Some(number) {
            findings.push(format!(
                "{DECISION_INDEX}:{line_no}: record {number:04} links to {target}, whose number is different"
            ));
        }
    }

    for file in files {
        let number = file.get(..4).and_then(|n| n.parse::<u32>().ok());
        if let Some(number) = number
            && !rows.contains_key(&number)
        {
            findings.push(format!(
                "{DECISION_INDEX}: record {file} has no row in the index"
            ));
        }
    }
    findings
}

/// The record files under `docs/decisions/`: `NNNN-title.md`.
fn record_files(dir: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let numbered = name
            .split_once('-')
            .is_some_and(|(n, _)| n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()));
        if numbered && name.ends_with(".md") {
            files.push(name);
        }
    }
    files.sort();
    Ok(files)
}

/// Runs both register checks under `root`.
pub fn check(root: &Path) -> Result<Findings> {
    let mut findings = Findings::new();
    let open = std::fs::read_to_string(root.join(OPEN_ITEMS))
        .with_context(|| format!("cannot read {OPEN_ITEMS}"))?;
    findings.extend(check_open_items(&open));
    let index = std::fs::read_to_string(root.join(DECISION_INDEX))
        .with_context(|| format!("cannot read {DECISION_INDEX}"))?;
    let files = record_files(&root.join("docs/decisions"))?;
    findings.extend(check_decision_index(&index, &files));
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "# Open items\n\n## Open, proposed and deferred\n\n| ID | Item | Source | State | Decide in | Needs |\n| --- | --- | --- | --- | --- | --- |\n";
    const MID: &str = "\n## Closed\n\n| ID | Item | Source | Closed | Settled in |\n| --- | --- | --- | --- | --- |\n";

    fn register(open: &[&str], closed: &[&str]) -> String {
        format!("{HEAD}{}{MID}{}\n", open.join("\n"), closed.join("\n"))
    }

    const OPEN_ROW: &str = "| OI-7 | An item | 0001 | open | Slice 3 | Owner |";
    const CLOSED_ROW: &str = "| OI-9 | Another | 0002 | 18 Oct 2026 | Decision 0003 |";

    #[test]
    fn a_clean_register_passes() {
        assert!(check_open_items(&register(&[OPEN_ROW], &[CLOSED_ROW])).is_empty());
    }

    #[test]
    fn a_row_kept_twice_by_a_union_merge_is_found() {
        let edited = "| OI-7 | An item, edited | 0001 | open | Slice 4 | Owner |";
        let findings = check_open_items(&register(&[OPEN_ROW, edited], &[]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("OI-7 is in the open table again"));
        assert!(findings[0].contains("first at line 7"));
        assert!(findings[0].starts_with("docs/open-items.md:8:"));
    }

    #[test]
    fn a_closed_row_kept_twice_is_found() {
        let findings = check_open_items(&register(&[], &[CLOSED_ROW, CLOSED_ROW]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("OI-9 is in the closed table again"));
    }

    #[test]
    fn an_item_in_both_tables_is_found() {
        let closed = "| OI-7 | An item | 0001 | 18 Oct 2026 | Done |";
        let findings = check_open_items(&register(&[OPEN_ROW], &[closed]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("OI-7 is in both tables"));
    }

    #[test]
    fn a_closed_state_in_the_open_table_is_found() {
        let row = "| OI-7 | An item | 0001 | Closed 17 Oct 2026 | Built | |";
        let findings = check_open_items(&register(&[row], &[]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("move the row to the closed table"));
    }

    #[test]
    fn states_that_only_mention_closing_are_not_closed() {
        let row = "| OI-7 | An item | 0001 | open: closes with OI-9 | Slice 3 | |";
        assert!(check_open_items(&register(&[row], &[])).is_empty());
    }

    #[test]
    fn a_row_with_the_wrong_number_of_cells_is_found() {
        let short = "| OI-7 | An item | 0001 | open |";
        let long = "| OI-8 | An `a|b` item | 0001 | open | Slice 3 | Owner |";
        let findings = check_open_items(&register(&[short, long], &[]));
        assert_eq!(findings.len(), 2);
        assert!(findings[0].contains("OI-7 has 4 cells, the open table has 6"));
        assert!(findings[1].contains("OI-8 has 7 cells"));
    }

    #[test]
    fn a_closed_row_needs_five_cells() {
        let findings =
            check_open_items(&register(&[], &["| OI-9 | Another | 0002 | 18 Oct 2026 |"]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("the closed table has 5"));
    }

    #[test]
    fn an_open_row_filed_in_the_closed_table_is_found() {
        let misfiled = "| OI-9 | Another | 0002 | open | Slice 9 | Owner |";
        let findings = check_open_items(&register(&[], &[misfiled]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("six cells of an open-table row"));
        assert!(findings[0].contains("Move it to the open table"));
    }

    #[test]
    fn the_first_closed_items_keep_their_c_numbers() {
        let row = "| OI-C1 | Adoption records | 0002 | 11 Oct 2026 | Recorded |";
        assert!(check_open_items(&register(&[], &[row])).is_empty());
        assert!(!is_item_id("OI-C"));
        assert!(!is_item_id("OI-"));
    }

    #[test]
    fn a_row_without_an_oi_number_is_found() {
        let findings = check_open_items(&register(&["| OI-x | An item | 0001 | open | | |"], &[]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("is not an OI- number"));
    }

    fn files(numbers: &[u32]) -> Vec<String> {
        numbers.iter().map(|n| format!("{n:04}-title.md")).collect()
    }

    fn index(numbers: &[u32]) -> String {
        let rows: Vec<String> = numbers
            .iter()
            .map(|n| format!("| [{n:04}]({n:04}-title.md) | Title | Accepted |"))
            .collect();
        format!(
            "## Index\n\n| Record | Title | Status |\n| --- | --- | --- |\n{}\n",
            rows.join("\n")
        )
    }

    #[test]
    fn a_complete_ordered_index_passes() {
        assert!(check_decision_index(&index(&[1, 2, 54]), &files(&[1, 2, 54])).is_empty());
    }

    #[test]
    fn a_record_without_an_index_row_is_found() {
        let findings = check_decision_index(&index(&[1, 3]), &files(&[1, 2, 3]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("0002-title.md has no row in the index"));
    }

    #[test]
    fn an_index_row_without_a_record_is_found() {
        let findings = check_decision_index(&index(&[1, 2]), &files(&[1]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("record 0002 links to 0002-title.md, which does not exist"));
    }

    #[test]
    fn index_rows_out_of_number_order_are_found() {
        let findings = check_decision_index(&index(&[1, 3, 2]), &files(&[1, 2, 3]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("record 0002 is after record 0003"));
    }

    #[test]
    fn a_second_index_row_for_a_record_is_found() {
        let findings = check_decision_index(&index(&[1, 2, 2]), &files(&[1, 2]));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("record 0002 has a second row (first at line 6)"));
    }

    #[test]
    fn a_row_whose_link_has_another_number_is_found() {
        let text = "| [0002](0003-title.md) | Title | Accepted |\n";
        let findings = check_decision_index(text, &files(&[2, 3]));
        assert!(
            findings.iter().any(
                |f| f.contains("record 0002 links to 0003-title.md, whose number is different")
            ),
            "{findings:?}"
        );
    }

    #[test]
    fn a_row_ending_in_two_bars_keeps_its_empty_last_cell() {
        assert_eq!(cells("| a | b ||"), ["a", "b", ""]);
        assert_eq!(cells("|| a | b |"), ["", "a", "b"]);
        assert_eq!(cells("| a | b |"), ["a", "b"]);
    }

    #[test]
    fn lines_that_are_not_index_rows_are_ignored() {
        let text = format!(
            "{}\n| [x](y) | not a record |\n| [12345](a.md) | five digits |\n",
            index(&[1])
        );
        assert!(check_decision_index(&text, &files(&[1])).is_empty());
    }

    #[test]
    fn the_repository_registers_pass() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let findings = check(&root).unwrap();
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
