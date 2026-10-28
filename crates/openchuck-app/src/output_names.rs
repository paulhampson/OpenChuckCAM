// SPDX-License-Identifier: AGPL-3.0-or-later
//! File name templates for the program files and the setup sheet (issues
//! 308 and 310; GC-05, GC-07, NFR-07; decision record 0267).
//!
//! A template is text with fields in braces, such as `{n}-{contents}`. `{{`
//! and `}}` stand for a brace. There is nothing else: no code, no
//! conditions and no path separators, so a template can only ever name one
//! file in the chosen folder. The extension is not part of the template: the
//! post gives the program's, and the sheet is always `.html` and `.pdf`.
//!
//! Each field's value is cleaned the way the names were before (lower-case
//! ASCII letters and digits joined by hyphens), and the whole name is cleaned
//! again with the project folder rules of issue 219, so every operating
//! system accepts it. The default templates give exactly the names written
//! before templates existed. The templates are a per-user setting
//! ([`crate::settings::UserSettings::file_names`]), never in the project.

use serde::{Deserialize, Serialize};

use crate::posting::slug;

/// The program file template that gives the names written before templates
/// existed: the running number, then the setup (or the project for one file
/// per project) and what the file holds.
pub const DEFAULT_GCODE_TEMPLATE: &str = "{n}-{contents}";
/// The setup sheet template that gives the name written before templates
/// existed.
pub const DEFAULT_SHEET_TEMPLATE: &str = "{project}-setup-sheet";
/// The longest template kept, in characters.
pub const MAX_TEMPLATE_CHARS: usize = 200;

/// One user's file name templates, kept on this computer (issues 308, 310).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileNameTemplates {
    /// The program (NC) file name, without its extension.
    pub gcode: String,
    /// The setup sheet's name, without `.html` or `.pdf`.
    pub setup_sheet: String,
}

impl Default for FileNameTemplates {
    fn default() -> Self {
        Self {
            gcode: DEFAULT_GCODE_TEMPLATE.to_owned(),
            setup_sheet: DEFAULT_SHEET_TEMPLATE.to_owned(),
        }
    }
}

impl FileNameTemplates {
    /// The templates with each one cut to [`MAX_TEMPLATE_CHARS`]; an empty one
    /// becomes its default.
    #[must_use]
    pub fn clamped(self) -> Self {
        let keep = |text: String, default: &str| {
            let text: String = text.chars().take(MAX_TEMPLATE_CHARS).collect();
            if text.trim().is_empty() {
                default.to_owned()
            } else {
                text
            }
        };
        Self {
            gcode: keep(self.gcode, DEFAULT_GCODE_TEMPLATE),
            setup_sheet: keep(self.setup_sheet, DEFAULT_SHEET_TEMPLATE),
        }
    }
}

/// A field a template can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameField {
    /// The file's place in the write order, two digits (`01`).
    Number,
    /// What the name said after the number before templates existed.
    Contents,
    /// The project's name.
    Project,
    /// The machine's name.
    Machine,
    /// The post's id, or its folder's name.
    Post,
    /// The setup's name.
    Setup,
    /// The setup's number in the project, from 1.
    SetupNumber,
    /// The operation's name.
    Operation,
    /// The operation's number among the enabled operations, from 1.
    OperationNumber,
    /// The tool's name.
    Tool,
    /// The tool's number on the machine.
    ToolNumber,
    /// The export's date, `YYYY-MM-DD` in UTC.
    Date,
}

impl NameField {
    /// Every field, in the order the dialog lists them.
    pub const ALL: [Self; 12] = [
        Self::Number,
        Self::Contents,
        Self::Project,
        Self::Machine,
        Self::Post,
        Self::Setup,
        Self::SetupNumber,
        Self::Operation,
        Self::OperationNumber,
        Self::Tool,
        Self::ToolNumber,
        Self::Date,
    ];

    /// The name written between the braces.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Number => "n",
            Self::Contents => "contents",
            Self::Project => "project",
            Self::Machine => "machine",
            Self::Post => "post",
            Self::Setup => "setup",
            Self::SetupNumber => "setup_no",
            Self::Operation => "operation",
            Self::OperationNumber => "operation_no",
            Self::Tool => "tool",
            Self::ToolNumber => "tool_no",
            Self::Date => "date",
        }
    }

    /// What the field gives, for the dialog's list of fields.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Number => "the file's number in the write order (01, 02, ...)",
            Self::Contents => "the setup, or the project, and what the file holds",
            Self::Project => "the project's name",
            Self::Machine => "the machine's name",
            Self::Post => "the post",
            Self::Setup => "the setup's name",
            Self::SetupNumber => "the setup's number",
            Self::Operation => "the operation's name (one operation per file)",
            Self::OperationNumber => "the operation's number (one operation per file)",
            Self::Tool => "the tool's name (one tool per file)",
            Self::ToolNumber => "the tool's number (one tool per file)",
            Self::Date => "the export date, YYYY-MM-DD (UTC)",
        }
    }

    /// Whether the setup sheet's template may use the field: one sheet
    /// covers every file, so the per-file fields have no value there.
    pub const fn for_sheet(self) -> bool {
        matches!(
            self,
            Self::Project | Self::Machine | Self::Post | Self::Date
        )
    }

    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.as_str() == name)
    }
}

/// A piece of a parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    Field(NameField),
}

/// Reads `template`, or says what is wrong with it in a sentence.
fn parse(template: &str, sheet: bool) -> Result<Vec<Piece>, String> {
    if template.trim().is_empty() {
        return Err("The template is empty. Enter a name, or reset it to the default.".into());
    }
    if template.chars().count() > MAX_TEMPLATE_CHARS {
        return Err(format!(
            "The template is longer than {MAX_TEMPLATE_CHARS} characters."
        ));
    }
    if template.contains(['/', '\\']) {
        return Err(
            "A template names one file in the output folder, so it cannot hold / or \\.".into(),
        );
    }
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                text.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                text.push('}');
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') | None => {
                            return Err(
                                "A { has no matching }. Write {{ for a brace in the name.".into()
                            );
                        }
                        Some(c) => name.push(c),
                    }
                }
                let field = NameField::parse(name.trim()).ok_or_else(|| {
                    format!(
                        "There is no field {{{name}}}. The fields are {}.",
                        fields_list(sheet)
                    )
                })?;
                if sheet && !field.for_sheet() {
                    return Err(format!(
                        "The setup sheet covers every file, so it cannot use {{{}}}. Its fields are {}.",
                        field.as_str(),
                        fields_list(true)
                    ));
                }
                if !text.is_empty() {
                    pieces.push(Piece::Text(std::mem::take(&mut text)));
                }
                pieces.push(Piece::Field(field));
            }
            '}' => {
                return Err("A } has no matching {. Write }} for a brace in the name.".into());
            }
            c => text.push(c),
        }
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    Ok(pieces)
}

/// The fields a template may use, as `{n}, {contents}, ...`.
pub fn fields_list(sheet: bool) -> String {
    NameField::ALL
        .into_iter()
        .filter(|f| !sheet || f.for_sheet())
        .map(|f| format!("{{{}}}", f.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Checks `template` without naming any file: `None` when it reads, else
/// the sentence the dialog shows under the field.
pub fn template_problem(template: &str, sheet: bool) -> Option<String> {
    parse(template, sheet).err()
}

/// One operation in a program file, in the order the program runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStep {
    /// The operation's number among the enabled operations, from 1, as the
    /// setup sheet's Operations table numbers it.
    pub operation_number: usize,
    /// The operation's name.
    pub operation: String,
    /// The tool's machine number, when the operation has a tool.
    pub tool_number: Option<u32>,
    /// The tool's name.
    pub tool: Option<String>,
    /// Whether the program changes tool before this operation: the first
    /// tool of a file and each later different tool number, as the post
    /// writes them.
    pub tool_change: bool,
}

/// One program file as it is written: its final name and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFile {
    /// The name the posting gave the file, the key in
    /// [`crate::Regeneration::gcode`].
    pub posted_name: String,
    /// The name it is written as, from the template.
    pub name: String,
    /// The setup's number in the project, from 1.
    pub setup_number: usize,
    /// The setup's name.
    pub setup: String,
    /// The operations, in the order they run.
    pub steps: Vec<ProgramStep>,
}

/// Every file one write makes, with its final name (issues 308 to 310).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFiles {
    /// The program files, in write order.
    pub programs: Vec<ProgramFile>,
    /// The setup sheet's names, HTML then PDF, written after the programs.
    pub setup_sheet: [String; 2],
}

/// What the whole job gives the fields that do not depend on the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobFacts {
    pub project: String,
    pub machine: String,
    pub post: String,
    pub date: String,
}

/// What one file gives the per-file fields.
#[derive(Debug)]
pub(crate) struct FileFacts<'a> {
    pub index: usize,
    pub contents: &'a str,
    pub file: &'a ProgramFile,
}

/// The single value of a per-file field, or why it has none.
fn single<T: PartialEq + Clone>(
    values: impl Iterator<Item = T>,
    what: &str,
    field: NameField,
    file: &ProgramFile,
    fix: &str,
) -> Result<T, String> {
    let mut distinct: Vec<T> = Vec::new();
    for value in values {
        if !distinct.contains(&value) {
            distinct.push(value);
        }
    }
    match distinct.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!(
            "File '{}' has no {what}, so {{{}}} has no value. Remove {{{}}} from the template.",
            file.posted_name,
            field.as_str(),
            field.as_str()
        )),
        many => Err(format!(
            "File '{}' holds {} {what}s, so {{{}}} has no single value. {fix}",
            file.posted_name,
            many.len(),
            field.as_str()
        )),
    }
}

fn value(field: NameField, job: &JobFacts, file: Option<&FileFacts<'_>>) -> Result<String, String> {
    let file = match (field, file) {
        (NameField::Project, _) => return Ok(slug(&job.project, "project")),
        (NameField::Machine, _) => return Ok(slug(&job.machine, "machine")),
        (NameField::Post, _) => return Ok(slug(&job.post, "post")),
        (NameField::Date, _) => return Ok(job.date.clone()),
        (_, Some(file)) => file,
        // A sheet template with a per-file field is refused when parsed.
        (_, None) => return Err(format!("{{{}}} has no value here.", field.as_str())),
    };
    let one_op = format!(
        "Choose one file per operation, or remove {{{}}} from the template.",
        field.as_str()
    );
    let one_tool = format!(
        "Choose one file per tool or per operation, or remove {{{}}} from the template.",
        field.as_str()
    );
    let steps = &file.file.steps;
    Ok(match field {
        NameField::Number => format!("{:02}", file.index + 1),
        NameField::Contents => file.contents.to_owned(),
        NameField::Setup => slug(&file.file.setup, "setup"),
        NameField::SetupNumber => file.file.setup_number.to_string(),
        NameField::Operation => slug(
            &single(
                steps.iter().map(|s| s.operation.clone()),
                "operation",
                field,
                file.file,
                &one_op,
            )?,
            "operation",
        ),
        NameField::OperationNumber => single(
            steps.iter().map(|s| s.operation_number),
            "operation",
            field,
            file.file,
            &one_op,
        )?
        .to_string(),
        NameField::Tool => slug(
            &single(
                steps.iter().filter_map(|s| s.tool.clone()),
                "tool",
                field,
                file.file,
                &one_tool,
            )?,
            "tool",
        ),
        NameField::ToolNumber => single(
            steps.iter().filter_map(|s| s.tool_number),
            "tool",
            field,
            file.file,
            &one_tool,
        )?
        .to_string(),
        NameField::Project | NameField::Machine | NameField::Post | NameField::Date => {
            return value(field, job, None);
        }
    })
}

/// The cleaned file name stem `template` gives, without an extension.
fn render(
    template: &str,
    sheet: bool,
    job: &JobFacts,
    file: Option<&FileFacts<'_>>,
) -> Result<String, String> {
    let mut stem = String::new();
    for piece in parse(template, sheet)? {
        match piece {
            Piece::Text(text) => stem.push_str(&text),
            Piece::Field(field) => stem.push_str(&value(field, job, file)?),
        }
    }
    Ok(clean_stem(&stem))
}

/// `stem` made acceptable to every operating system with the project folder
/// rules (issue 219): forbidden and control characters become `_`, leading
/// and trailing dots and spaces go, a Windows device name gets a `_` in
/// front, and the length is limited. A stem that is already clean, such as
/// every default name, is unchanged.
pub fn clean_stem(stem: &str) -> String {
    openchuck_project::new_folder::folder_name(stem)
}

/// The program file's name for `template`.
pub(crate) fn program_name(
    template: &str,
    job: &JobFacts,
    file: &FileFacts<'_>,
    extension: &str,
) -> Result<String, String> {
    render(template, false, job, Some(file)).map(|stem| with_extension(&stem, extension))
}

/// The setup sheet's two names for `template`, HTML then PDF.
pub(crate) fn sheet_names(template: &str, job: &JobFacts) -> Result<[String; 2], String> {
    let stem = render(template, true, job, None)?;
    Ok([format!("{stem}.html"), format!("{stem}.pdf")])
}

fn with_extension(stem: &str, extension: &str) -> String {
    if extension.is_empty() {
        stem.to_owned()
    } else {
        format!("{stem}.{extension}")
    }
}

/// The first pair of names that would be the same file on a computer that
/// ignores letter case (Windows and macOS by default), as a sentence that
/// says how to tell them apart; `None` when every name is distinct.
pub fn collision(names: &[String]) -> Option<String> {
    for (i, name) in names.iter().enumerate() {
        let folded = name.to_lowercase();
        if let Some((j, other)) = names
            .iter()
            .take(i)
            .enumerate()
            .find(|(_, other)| other.to_lowercase() == folded)
        {
            let same = if other == name {
                format!(
                    "Files {} and {} would both be called '{name}'.",
                    j + 1,
                    i + 1
                )
            } else {
                format!(
                    "Files {} and {} would be called '{}' and '{name}', which are the same file where letter case is ignored (Windows and macOS).",
                    j + 1,
                    i + 1,
                    other
                )
            };
            return Some(format!(
                "{same} Nothing is written until every name is different: add {{n}} or {{operation_no}} to the program file template, or change the setup sheet template."
            ));
        }
    }
    None
}

/// The hash the setup sheet gives each program file, of the file's exact
/// bytes as written (issue 309; decision record 0267): SHA-256, which
/// Windows, macOS and Linux can each compute with a tool they ship.
pub mod file_hash {
    use sha2::{Digest, Sha256};

    /// The label of the hash on the sheet.
    pub const LABEL: &str = "SHA-256";

    /// How the operator checks a file against the sheet.
    pub const CHECK: &str = "To check that a file on the machine is the one this sheet \
        describes, compute its SHA-256 and compare all 64 characters with the sheet: \
        on Windows `certutil -hashfile <file> SHA256`, on macOS `shasum -a 256 <file>`, \
        on Linux `sha256sum <file>`. A file whose hash differs is not the file listed here.";

    /// The SHA-256 of `bytes` as 64 lower-case hexadecimal digits.
    pub fn hex(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn job() -> JobFacts {
        JobFacts {
            project: "Bracket A".into(),
            machine: "Makera Z1".into(),
            post: "makera-z1".into(),
            date: "2026-10-28".into(),
        }
    }

    fn step(no: usize, op: &str, tool: u32, change: bool) -> ProgramStep {
        ProgramStep {
            operation_number: no,
            operation: op.into(),
            tool_number: Some(tool),
            tool: Some(format!("{tool} mm flat")),
            tool_change: change,
        }
    }

    fn file(steps: Vec<ProgramStep>) -> ProgramFile {
        ProgramFile {
            posted_name: "01-setup-1.nc".into(),
            name: String::new(),
            setup_number: 1,
            setup: "Setup 1".into(),
            steps,
        }
    }

    fn name(template: &str, file: &ProgramFile) -> Result<String, String> {
        program_name(
            template,
            &job(),
            &FileFacts {
                index: 0,
                contents: "setup-1",
                file,
            },
            "nc",
        )
    }

    #[test]
    fn req_gc_05_req_gc_07_the_default_templates_give_the_names_written_before() {
        let one = file(vec![step(1, "Rough", 3, true)]);
        assert_eq!(name(DEFAULT_GCODE_TEMPLATE, &one).unwrap(), "01-setup-1.nc");
        assert_eq!(
            sheet_names(DEFAULT_SHEET_TEMPLATE, &job()).unwrap(),
            ["bracket-a-setup-sheet.html", "bracket-a-setup-sheet.pdf"]
        );
        assert_eq!(FileNameTemplates::default().gcode, DEFAULT_GCODE_TEMPLATE);
        assert_eq!(
            FileNameTemplates::default().setup_sheet,
            DEFAULT_SHEET_TEMPLATE
        );
    }

    #[test]
    fn req_gc_05_every_field_gives_its_cleaned_value() {
        let one = file(vec![step(4, "Finish Top!", 7, true)]);
        assert_eq!(
            name(
                "{project}_{machine}_{post}_{setup}_{setup_no}_{operation}_{operation_no}_{tool}_T{tool_no}_{date}_{n}_{contents}",
                &one
            )
            .unwrap(),
            "bracket-a_makera-z1_makera-z1_setup-1_1_finish-top_4_7-mm-flat_T7_2026-10-28_01_setup-1.nc"
        );
        assert_eq!(
            sheet_names("{project} sheet {date}", &job()).unwrap()[0],
            "bracket-a sheet 2026-10-28.html"
        );
    }

    #[test]
    fn req_nfr_07_a_template_that_cannot_name_one_file_is_refused_with_a_reason() {
        let one = file(vec![step(1, "Rough", 3, true)]);
        for (template, says) in [
            ("", "empty"),
            ("   ", "empty"),
            ("{n}/{contents}", "cannot hold"),
            ("..\\{n}", "cannot hold"),
            ("{n", "no matching }"),
            ("{n{x}}", "no matching }"),
            ("n}", "no matching {"),
            ("{name}", "no field {name}"),
        ] {
            let error = name(template, &one).unwrap_err();
            assert!(error.contains(says), "{template}: {error}");
            assert_eq!(template_problem(template, false), Some(error));
        }
        let long = "x".repeat(MAX_TEMPLATE_CHARS + 1);
        assert!(name(&long, &one).unwrap_err().contains("longer"));
        assert_eq!(template_problem(DEFAULT_GCODE_TEMPLATE, false), None);
    }

    #[test]
    fn req_gc_05_doubled_braces_are_literal_braces() {
        let one = file(vec![step(1, "Rough", 3, true)]);
        assert_eq!(name("{{x}}-{n}", &one).unwrap(), "{x}-01.nc");
    }

    #[test]
    fn req_gc_05_the_sheet_template_refuses_per_file_fields() {
        for field in NameField::ALL {
            let template = format!("{{{}}}", field.as_str());
            assert_eq!(
                template_problem(&template, true).is_none(),
                field.for_sheet(),
                "{template}"
            );
        }
        assert!(
            sheet_names("{operation}", &job())
                .unwrap_err()
                .contains("covers every file")
        );
        assert_eq!(fields_list(true), "{project}, {machine}, {post}, {date}");
    }

    #[test]
    fn req_nfr_07_a_field_with_no_single_value_says_how_to_fix_it() {
        let two_ops = file(vec![step(1, "Rough", 3, true), step(2, "Finish", 3, false)]);
        let error = name("{operation}", &two_ops).unwrap_err();
        assert!(
            error.contains("holds 2 operations") && error.contains("one file per operation"),
            "{error}"
        );
        assert!(name("{operation_no}", &two_ops).is_err());
        // One tool across two operations has a single tool value.
        assert_eq!(
            name("T{tool_no}-{tool}", &two_ops).unwrap(),
            "T3-3-mm-flat.nc"
        );
        let two_tools = file(vec![step(1, "Rough", 3, true), step(2, "Finish", 5, true)]);
        assert!(
            name("{tool}", &two_tools)
                .unwrap_err()
                .contains("one file per tool")
        );
        assert!(name("{tool_no}", &two_tools).is_err());
        let no_tool = file(vec![ProgramStep {
            tool_number: None,
            tool: None,
            ..step(1, "Probe", 1, false)
        }]);
        assert!(
            name("{tool}", &no_tool)
                .unwrap_err()
                .contains("has no tool")
        );
    }

    #[test]
    fn req_nfr_07_names_are_cleaned_for_every_operating_system() {
        let one = file(vec![step(1, "Rough", 3, true)]);
        assert_eq!(name("CON", &one).unwrap(), "_CON.nc");
        assert_eq!(name("a:b*c?", &one).unwrap(), "a_b_c_.nc");
        assert_eq!(name(" {n}. ", &one).unwrap(), "01.nc");
        assert_eq!(name("...", &one).unwrap(), "Untitled.nc");
        let long = name(&"y".repeat(150), &one).unwrap();
        assert_eq!(long.chars().count(), 100 + ".nc".len());
        assert_eq!(clean_stem("01-setup-1"), "01-setup-1");
    }

    #[test]
    fn req_nfr_07_names_that_differ_only_in_case_collide() {
        let names = ["01-a.nc", "A.nc", "a.nc"].map(String::from);
        let said = collision(&names).unwrap();
        assert!(
            said.contains("Files 2 and 3") && said.contains("letter case"),
            "{said}"
        );
        let same = ["x.nc", "x.nc"].map(String::from);
        assert!(collision(&same).unwrap().contains("both be called 'x.nc'"));
        assert!(collision(&["a.nc".into(), "b.nc".into()]).is_none());
    }

    #[test]
    fn req_gc_12_the_file_hash_is_sha_256_of_the_exact_bytes() {
        // FIPS 180-2 test vectors.
        assert_eq!(
            file_hash::hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            file_hash::hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_ne!(file_hash::hex(b"G0 X0\n"), file_hash::hex(b"G0 X0\r\n"));
    }

    #[test]
    fn req_ui_15_templates_are_clamped_when_read() {
        let kept = FileNameTemplates {
            gcode: " ".into(),
            setup_sheet: "z".repeat(MAX_TEMPLATE_CHARS + 5),
        }
        .clamped();
        assert_eq!(kept.gcode, DEFAULT_GCODE_TEMPLATE);
        assert_eq!(kept.setup_sheet.chars().count(), MAX_TEMPLATE_CHARS);
    }
}
