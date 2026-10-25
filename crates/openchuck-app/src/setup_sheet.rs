// SPDX-License-Identifier: AGPL-3.0-or-later
//! The setup sheet (GC-12; decision record 0028 items 3 to 5): the tools,
//! operations and stock of a project as one printable page, written as HTML or
//! PDF.
//!
//! [`SetupSheet::new`] reads the project (and, for the times, the toolpath)
//! into a small document of facts and tables. [`SetupSheet::to_html`] and
//! [`SetupSheet::to_pdf`] write that one document, so the two formats always
//! say the same. Both are byte-for-byte repeatable (GC-07): no date, no
//! random identifier, no clock.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use openchuck_path::Registry;
use openchuck_project::{Operation, Project, Setup, ToolSnapshot};
use openchuck_types::ir::Toolpath;
use openchuck_types::notice::VERIFY_GCODE_NOTICE;
use openchuck_types::setup::PartExtent;
use openchuck_types::stock::Stock;
use openchuck_types::workholding::{HeldRegion, Tab, TabLayout, TabShape, Workholding};

/// What a section holds.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetBody {
    /// Labelled values.
    Facts(Vec<(String, String)>),
    /// A table with a header row.
    Table {
        /// Column headings.
        headers: Vec<String>,
        /// Rows of cells, each as long as `headers`.
        rows: Vec<Vec<String>>,
    },
    /// A paragraph.
    Text(String),
}

/// A headed part of the sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetSection {
    /// The heading.
    pub heading: String,
    /// The content.
    pub body: SheetBody,
}

/// A project's setup sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupSheet {
    /// The sheet's title.
    pub title: String,
    /// The sections, in reading order.
    pub sections: Vec<SheetSection>,
}

/// Where the sheet reads the time it was generated from (decision 0110 item
/// 2). The system clock is read only when a sheet is written; a test passes a
/// [`FixedClock`] so a snapshot of the sheet is the same every run.
pub trait Clock: fmt::Debug + Send + Sync {
    /// The current time.
    fn now(&self) -> SystemTime;
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// A clock that always reads the same time.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub SystemTime);

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

/// `time` as ISO 8601 in UTC to the second, for example
/// `2026-10-21T09:30:05Z`. UTC keeps the sheet the same on every computer;
/// the `Z` is its time zone. A time before 1970 reads as 1970.
pub fn iso8601_utc(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// What ties a printed sheet to the files it came from and to the moment it
/// was made (GC-12, decision 0110). Each field is the text the sheet shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Traceability {
    /// When the sheet was generated, ISO 8601 with the time zone.
    pub generated: String,
    /// When the project was last saved (its files' modification time), or
    /// "not saved yet".
    pub saved: String,
    /// The software version that made the sheet.
    pub version: String,
    /// A hash of the saved project files, or "not saved yet".
    pub fingerprint: String,
    /// The version-control line.
    pub version_control: String,
}

impl Traceability {
    /// The version-control line until `openchuck-vcs` exists.
    pub const NOT_TRACKED: &'static str = "not tracked";
    /// What the saved time and the fingerprint read when no project file has
    /// been saved.
    pub const NOT_SAVED: &'static str = "not saved yet";

    /// The sheet's Traceability section.
    pub fn section(&self) -> SheetSection {
        SheetSection {
            heading: "Traceability".into(),
            body: SheetBody::Facts(vec![
                ("Generated".into(), self.generated.clone()),
                ("Project last saved".into(), self.saved.clone()),
                ("Software version".into(), self.version.clone()),
                ("Project fingerprint".into(), self.fingerprint.clone()),
                ("Version control".into(), self.version_control.clone()),
            ]),
        }
    }
}

/// `value` in millimetres with up to three decimals and no trailing zeros.
fn mm(value: f64) -> String {
    plain(value, 3)
}

fn plain(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    };
    if text == "-0" { "0".to_string() } else { text }
}

/// Seconds as `h:mm:ss` or `m:ss`.
fn clock(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn describe_stock(stock: &Stock) -> String {
    match stock {
        Stock::Cylinder {
            diameter,
            length,
            x_start,
        } => format!(
            "cylinder, diameter {} mm, length {} mm, from X {} mm",
            mm(diameter.0),
            mm(length.0),
            mm(x_start.0)
        ),
        Stock::Box { bounds } => format!(
            "box, X {} to {}, Y {} to {}, Z {} to {} mm",
            mm(bounds.min.x),
            mm(bounds.max.x),
            mm(bounds.min.y),
            mm(bounds.max.y),
            mm(bounds.min.z),
            mm(bounds.max.z)
        ),
        Stock::OffsetBoundingBox { offset } => {
            format!(
                "the part's bounding box grown by {} mm all round",
                mm(offset.0)
            )
        }
        Stock::Imported { mesh } => format!("imported solid, {} triangles", mesh.triangles.len()),
        _ => "stock of a kind this sheet does not describe".to_string(),
    }
}

fn describe_shape(tool: &ToolSnapshot) -> String {
    use openchuck_types::tool::ToolShape;
    let name = tool.tool.shape.type_name().replace('_', " ");
    match &tool.tool.shape {
        ToolShape::BullNose { corner_radius } => format!("{name}, corner R{}", mm(corner_radius.0)),
        ToolShape::TaperedBall {
            tip_radius,
            taper_half_angle,
        } => format!(
            "{name}, tip R{}, half angle {} deg",
            mm(tip_radius.0),
            plain(taper_half_angle.0, 2)
        ),
        ToolShape::VBit {
            included_angle,
            tip_diameter,
        }
        | ToolShape::Chamfer {
            included_angle,
            tip_diameter,
        } => format!(
            "{name}, {} deg, tip {} mm",
            plain(included_angle.0, 2),
            mm(tip_diameter.0)
        ),
        _ => name,
    }
}

impl SetupSheet {
    /// Add the selected post's GC-17 report to the shared HTML/PDF document.
    pub(crate) fn with_block_time(
        mut self,
        project: &Project,
        path: Option<&Toolpath>,
        options: Result<Option<openchuck_post::PostOptions>, String>,
    ) -> Self {
        let (Some(path), Some(machine)) = (path, project.machine()) else {
            return self;
        };
        let profile = openchuck_post::BlockTimeSettings {
            min_block_ms: machine.min_block_ms,
            block_window: machine.block_window,
            mode: machine.short_block_mode,
        };
        let report = options.and_then(|options| {
            let source = if options.as_ref().is_some_and(|o| o.min_block_ms.is_some()) {
                "post"
            } else {
                "profile"
            };
            let settings = options.as_ref().map_or(profile, |o| o.block_time(profile));
            openchuck_post::block_time_reports(path, settings)
                .map(|reports| (reports, source))
                .map_err(|e| e.to_string())
        });
        let facts = match report {
            Ok((reports, source)) => reports
                .into_iter()
                .map(|r| {
                    let name = project
                        .operations()
                        .iter()
                        .find(|o| o.id.to_string() == r.operation)
                        .map_or_else(|| r.operation.clone(), |o| o.name.clone());
                    (name, r.summary(source))
                })
                .collect::<Vec<_>>(),
            Err(error) => vec![("Report unavailable".into(), error)],
        };
        if !facts.is_empty() {
            self.sections.push(SheetSection {
                heading: "Minimum block time".into(),
                body: SheetBody::Facts(facts),
            });
        }
        self
    }

    /// The sheet with the Traceability section added after the Notes, where
    /// a printed copy ends (GC-12, decision 0110).
    #[must_use]
    pub fn with_traceability(mut self, traceability: &Traceability) -> Self {
        self.sections.push(traceability.section());
        self
    }

    /// Reads the sheet from `project`. With a `toolpath` (a regeneration's),
    /// each operation shows its programmed time; without one the time column
    /// is a dash.
    ///
    /// The time is the sum of the toolpath's move durations: each move at the
    /// feed the strategy gave it, with no acceleration, no tool change and no
    /// rapid-traverse settling. It is not the SIM-08 estimate, which comes
    /// with the machine simulation, and the sheet says so (decision record
    /// 0028 item 4).
    ///
    /// Each operation's tolerance is the one it is generated with, its own
    /// value or its strategy's default from `registry`, so a user checking
    /// the program sees the limit a warning was measured against (decision
    /// record 0175).
    pub fn new(project: &Project, registry: &Registry, toolpath: Option<&Toolpath>) -> Self {
        Self::with_parts(project, registry, toolpath, &|_| None)
    }

    /// [`SetupSheet::new`], with `part` giving each setup's part as the setup
    /// places it, so the tabs placed automatically are shown where the part,
    /// the stock and the held regions put them (decision record 0202). Without
    /// a part a tab is shown where it was last stored.
    pub fn with_parts(
        project: &Project,
        registry: &Registry,
        toolpath: Option<&Toolpath>,
        part: &dyn Fn(&Setup) -> Option<PartExtent>,
    ) -> Self {
        let mut times: BTreeMap<String, f64> = BTreeMap::new();
        if let Some(toolpath) = toolpath {
            for group in &toolpath.groups {
                let seconds: f64 = group.segments.iter().map(|s| s.duration.0).sum();
                *times.entry(group.tags.operation.clone()).or_default() += seconds;
            }
        }
        let mut sections = Vec::new();

        let used: Vec<&ToolSnapshot> = used_tools(project);
        let mut job = vec![
            ("Project".to_string(), project.name().to_string()),
            (
                "Machine".to_string(),
                project
                    .machine()
                    .map_or_else(|| "none chosen".to_string(), |m| m.name.clone()),
            ),
            ("Setups".to_string(), project.setups().len().to_string()),
            (
                "Operations".to_string(),
                project
                    .operations()
                    .iter()
                    .filter(|o| o.enabled)
                    .count()
                    .to_string(),
            ),
            ("Tools used".to_string(), used.len().to_string()),
        ];
        if toolpath.is_some() {
            let total: f64 = project
                .operations()
                .iter()
                .filter(|o| o.enabled)
                .filter_map(|o| times.get(&o.id.to_string()))
                .sum();
            job.push(("Programmed time".to_string(), clock(total)));
        }
        sections.push(SheetSection {
            heading: "Job".into(),
            body: SheetBody::Facts(job),
        });

        for setup in project.setups() {
            // The held regions and tabs the toolpath was made with (SET-09,
            // OP-21; OI-918), so the stock can be loaded from the sheet.
            let workholding = setup
                .workholding
                .resolved(setup.stock.as_ref(), part(setup).as_ref());
            let mut facts = setup_facts(project, setup);
            facts.extend(workholding_facts(setup, &workholding));
            sections.push(SheetSection {
                heading: format!("Setup: {}", setup.name),
                body: SheetBody::Facts(facts),
            });
            if !workholding.tabs.is_empty() {
                sections.push(SheetSection {
                    heading: tabs_heading(setup),
                    body: SheetBody::Table {
                        headers: [
                            "Tab", "X from", "X to", "Shape", "Dia mm", "Layout", "Placed",
                        ]
                        .map(String::from)
                        .to_vec(),
                        rows: workholding
                            .tabs
                            .iter()
                            .enumerate()
                            .map(|(i, tab)| tab_row(i, tab))
                            .collect(),
                    },
                });
            }
        }

        sections.push(SheetSection {
            heading: "Tools".into(),
            body: SheetBody::Table {
                headers: [
                    "Tool",
                    "Name",
                    "Type",
                    "Dia mm",
                    "Flutes",
                    "Flute mm",
                    "Overall mm",
                    "Shank mm",
                    "Used in",
                ]
                .map(String::from)
                .to_vec(),
                rows: used
                    .iter()
                    .map(|t| {
                        let setups: Vec<&str> = project
                            .operations()
                            .iter()
                            .filter(|o| o.enabled && o.tool == Some(t.id))
                            .filter_map(|o| project.setup(o.setup))
                            .map(|s| s.name.as_str())
                            .fold(Vec::new(), |mut names, name| {
                                if !names.contains(&name) {
                                    names.push(name);
                                }
                                names
                            });
                        vec![
                            format!("T{}", t.controller.number),
                            t.name.clone(),
                            describe_shape(t),
                            mm(t.tool.diameter.0),
                            t.tool.flute_count.to_string(),
                            mm(t.tool.flute_length.0),
                            mm(t.tool.overall_length.0),
                            mm(t.tool.shank_diameter.0),
                            setups.join(", "),
                        ]
                    })
                    .collect(),
            },
        });

        sections.push(SheetSection {
            heading: "Operations".into(),
            body: SheetBody::Table {
                headers: [
                    "No",
                    "Operation",
                    "Strategy",
                    "Setup",
                    "Tool",
                    "Spindle rpm",
                    "Speed from",
                    "Tolerance",
                    "Time",
                ]
                .map(String::from)
                .to_vec(),
                rows: project
                    .operations()
                    .iter()
                    .filter(|o| o.enabled)
                    .enumerate()
                    .map(|(i, o)| {
                        operation_row(project, registry, i, o, &times, toolpath.is_some())
                    })
                    .collect(),
            },
        });

        sections.push(SheetSection {
            heading: "Notes".into(),
            body: SheetBody::Text(format!(
                "Time is the sum of the toolpath's move durations at their programmed feeds. \
                 It leaves out acceleration, tool changes and fixturing, and is not the \
                 simulated estimate. {VERIFY_GCODE_NOTICE}"
            )),
        });

        Self {
            title: format!("Setup sheet: {}", project.name()),
            sections,
        }
    }

    /// The sheet as one self-contained HTML page: no script, no external
    /// file or font, so it opens offline and prints on white paper.
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
        out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        let _ = writeln!(out, "<title>{}</title>", escape(&self.title));
        out.push_str(STYLE);
        out.push_str("</head>\n<body>\n<main>\n");
        let _ = writeln!(out, "<h1>{}</h1>", escape(&self.title));
        for section in &self.sections {
            out.push_str("<section>\n");
            let _ = writeln!(out, "<h2>{}</h2>", escape(&section.heading));
            match &section.body {
                SheetBody::Facts(facts) => {
                    out.push_str("<dl>\n");
                    for (label, value) in facts {
                        let _ =
                            writeln!(out, "<dt>{}</dt><dd>{}</dd>", escape(label), escape(value));
                    }
                    out.push_str("</dl>\n");
                }
                SheetBody::Table { headers, rows } => {
                    out.push_str("<table>\n<thead><tr>");
                    for h in headers {
                        let _ = write!(out, "<th>{}</th>", escape(h));
                    }
                    out.push_str("</tr></thead>\n<tbody>\n");
                    for row in rows {
                        out.push_str("<tr>");
                        for cell in row {
                            let _ = write!(out, "<td>{}</td>", escape(cell));
                        }
                        out.push_str("</tr>\n");
                    }
                    out.push_str("</tbody>\n</table>\n");
                }
                SheetBody::Text(text) => {
                    let _ = writeln!(out, "<p>{}</p>", escape(text));
                }
            }
            out.push_str("</section>\n");
        }
        out.push_str("</main>\n</body>\n</html>\n");
        out
    }

    /// The sheet as a PDF on A4 pages in a fixed-width font, so a column
    /// always fits the width worked out for it. Characters outside ASCII are
    /// written as `?` (the PDF's built-in font has no others).
    pub fn to_pdf(&self) -> Vec<u8> {
        pdf::render(&self.title, &self.pdf_lines())
    }

    fn pdf_lines(&self) -> Vec<pdf::Line> {
        let mut lines = pdf::Line::title(&self.title);
        lines.push(pdf::Line::blank());
        for section in &self.sections {
            lines.extend(pdf::Line::heading(&section.heading));
            match &section.body {
                SheetBody::Facts(facts) => {
                    let width = facts
                        .iter()
                        .map(|(l, _)| l.chars().count())
                        .max()
                        .unwrap_or(0);
                    let room = pdf::COLUMNS.saturating_sub(width + 2).max(10);
                    for (label, value) in facts {
                        for (i, part) in pdf::wrap(value, room).iter().enumerate() {
                            let label = if i == 0 { label.as_str() } else { "" };
                            lines.push(pdf::Line::body(&format!("{label:<width$}  {part}")));
                        }
                    }
                }
                SheetBody::Table { headers, rows } => {
                    lines.extend(pdf::table(headers, rows));
                }
                SheetBody::Text(text) => {
                    lines.extend(
                        pdf::wrap(text, pdf::COLUMNS)
                            .iter()
                            .map(|l| pdf::Line::body(l)),
                    );
                }
            }
            lines.push(pdf::Line::blank());
        }
        lines
    }
}

/// Tools that at least one enabled operation cuts with (a disabled operation
/// is not in the program, UI-06), in the project's tool order.
fn used_tools(project: &Project) -> Vec<&ToolSnapshot> {
    project
        .tools()
        .iter()
        .filter(|t| {
            project
                .operations()
                .iter()
                .any(|o| o.enabled && o.tool == Some(t.id))
        })
        .collect()
}

fn setup_facts(project: &Project, setup: &Setup) -> Vec<(String, String)> {
    let part = setup
        .part
        .and_then(|id| project.models().iter().find(|m| m.id == id))
        .map_or_else(
            || "none chosen".to_string(),
            |m| m.path.as_str().to_string(),
        );
    let stock = setup
        .stock
        .as_ref()
        .map_or_else(|| "none chosen".to_string(), describe_stock);
    let mut facts = vec![
        ("Part".to_string(), part),
        ("Stock".to_string(), stock),
        (
            "Stock material".to_string(),
            setup
                .material
                .clone()
                .unwrap_or_else(|| "none chosen".to_string()),
        ),
        (
            "Work offset".to_string(),
            format!(
                "{:?}, work zero at model X {} mm on the rotary axis",
                setup.frame.work_offset.register,
                mm(setup.frame.work_offset.x.0)
            ),
        ),
        (
            "Part offset".to_string(),
            format!(
                "part moved {} mm along the rotary axis from where its placement puts it",
                mm(setup.frame.part_offset.0)
            ),
        ),
    ];
    if let Some(c) = setup.clearance {
        facts.push((
            "Clearance".to_string(),
            format!("radius {} mm, safe Z {} mm", mm(c.radius.0), mm(c.safe_z.0)),
        ));
    }
    facts
}

fn tabs_heading(setup: &Setup) -> String {
    format!("Tabs: {}", setup.name)
}

/// The setup's grips and a count of its tabs, in plain words.
fn workholding_facts(setup: &Setup, workholding: &Workholding) -> Vec<(String, String)> {
    let stock_x = setup
        .stock
        .as_ref()
        .and_then(|s| s.bounds(None))
        .map(|b| (b.min.x, b.max.x));
    let (start, end) = stock_x.map_or((None, None), |x| workholding.held_intervals(x));
    let held = |region: Option<HeldRegion>, span: Option<(f64, f64)>| match region {
        None => "none".to_string(),
        Some(region) => {
            let mut text = format!("{} mm of the stock", mm(region.length.0));
            if let Some((lo, hi)) = span {
                let _ = write!(text, ", X {} to {} mm", mm(lo), mm(hi));
            }
            match region.diameter {
                Some(d) => {
                    let _ = write!(text, ", at diameter {} mm", mm(d.0));
                }
                None => text.push_str(", at the stock's own section"),
            }
            text
        }
    };
    let tabs = match workholding.tabs.len() {
        0 => "none".to_string(),
        n => format!("{n}, listed under {}", tabs_heading(setup)),
    };
    vec![
        (
            "Held at the start".to_string(),
            held(workholding.start, start),
        ),
        ("Held at the end".to_string(), held(workholding.end, end)),
        ("Tabs".to_string(), tabs),
    ]
}

/// One row of a setup's tab table.
fn tab_row(index: usize, tab: &Tab) -> Vec<String> {
    let (shape, diameter) = match tab.shape {
        TabShape::Cylinder { diameter } => ("cylinder", mm(diameter.0)),
        TabShape::Frustum {
            start_diameter,
            end_diameter,
        } => (
            "frustum",
            format!("{} to {}", mm(start_diameter.0), mm(end_diameter.0)),
        ),
        _ => ("of a kind this sheet does not describe", "-".to_string()),
    };
    let layout = match tab.layout {
        TabLayout::OnAxis => "on the axis".to_string(),
        TabLayout::AroundAxis {
            count,
            start_angle,
            radial_offset,
        } => format!(
            "{count} around the axis, {} mm from it, the first at {} deg",
            mm(radial_offset.0),
            plain(start_angle.0, 3)
        ),
        _ => "of a kind this sheet does not describe".to_string(),
    };
    let placed = match tab.auto {
        Some(end) => format!("automatically, at the {} of the part", end.name()),
        None => "by hand".to_string(),
    };
    vec![
        (index + 1).to_string(),
        mm(tab.x_start.0),
        mm(tab.x_start.0 + tab.length.0),
        shape.to_string(),
        diameter,
        layout,
        placed,
    ]
}

fn operation_row(
    project: &Project,
    registry: &Registry,
    index: usize,
    operation: &Operation,
    times: &BTreeMap<String, f64>,
    with_times: bool,
) -> Vec<String> {
    let setup = project
        .setup(operation.setup)
        .map_or_else(String::new, |s| s.name.clone());
    let snapshot = operation
        .tool
        .and_then(|id| project.tools().iter().find(|t| t.id == id));
    let tool = snapshot.map_or_else(
        || "none".to_string(),
        |t| format!("T{} {}", t.controller.number, t.name),
    );
    let (rpm, speed_from) = match crate::requirements::spindle_speed(project, operation) {
        Some((speed, source)) => (plain(speed.0, 3), speed_source(project, operation, source)),
        None => ("-".to_string(), "none".to_string()),
    };
    let time = if with_times {
        times
            .get(&operation.id.to_string())
            .map_or_else(|| "-".to_string(), |s| clock(*s))
    } else {
        "-".to_string()
    };
    let tolerance = crate::form::parameter_form(project, registry, operation.id)
        .ok()
        .and_then(|form| {
            form.fields
                .into_iter()
                .find(|f| f.name == openchuck_path::params::names::TOLERANCE)
        })
        .and_then(|f| match f.value {
            // As set, not rounded: the sheet records the tolerance used.
            Some(openchuck_project::ParameterValue::Number(n)) => Some(format!("{n} mm")),
            // A whole number is stored without a decimal point (decision 0008).
            Some(openchuck_project::ParameterValue::Integer(i)) => Some(format!("{i} mm")),
            _ => None,
        })
        .unwrap_or_else(|| "-".to_string());
    vec![
        (index + 1).to_string(),
        operation.name.clone(),
        registry
            .get(&operation.strategy)
            .map_or_else(|_| operation.strategy.clone(), |s| s.label().to_string()),
        setup,
        tool,
        rpm,
        speed_from,
        tolerance,
        time,
    ]
}

/// Where a spindle speed came from, in plain words (decision 0196, OI-755).
fn speed_source(
    project: &Project,
    operation: &Operation,
    source: crate::requirements::SpindleSource,
) -> String {
    match source {
        crate::requirements::SpindleSource::Operation => "operation".to_string(),
        crate::requirements::SpindleSource::ToolData => {
            let material = project
                .setup(operation.setup)
                .and_then(|s| s.material.as_deref())
                .unwrap_or_default();
            format!("tool data: {material}")
        }
    }
}

/// HTML text escaping for element content.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

const STYLE: &str = "<style>
:root { color-scheme: light; }
body { margin: 0; background: #fff; color: #111; font: 14px/1.45 system-ui, -apple-system, \"Segoe UI\", Helvetica, Arial, sans-serif; }
main { max-width: 960px; margin: 0 auto; padding: 24px 16px; }
h1 { font-size: 22px; margin: 0 0 16px; }
h2 { font-size: 16px; margin: 24px 0 8px; border-bottom: 1px solid #888; padding-bottom: 2px; }
dl { display: grid; grid-template-columns: max-content 1fr; gap: 2px 16px; margin: 0; }
dt { font-weight: 600; }
dd { margin: 0; overflow-wrap: anywhere; }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid #888; padding: 3px 6px; text-align: left; vertical-align: top; overflow-wrap: anywhere; }
th { background: #eee; }
section { break-inside: avoid; }
@media print { main { max-width: none; padding: 0; } body { font-size: 11px; } }
@page { size: A4; margin: 15mm; }
</style>
";

/// A minimal PDF writer for the sheet: A4 pages of fixed-width text in the
/// built-in Courier fonts. No fonts are embedded, nothing is compressed, and
/// the file holds no date or identifier, so the same sheet is the same bytes.
mod pdf {
    use std::fmt::Write as _;

    /// Characters per line: the printable width over the width of one glyph.
    pub const COLUMNS: usize = 100;
    const PAGE_WIDTH: f64 = 595.0;
    const PAGE_HEIGHT: f64 = 842.0;
    const MARGIN: f64 = 40.0;
    const BODY_SIZE: f64 = 8.5;
    const LEADING: f64 = 11.0;
    const LINES_PER_PAGE: usize = ((PAGE_HEIGHT - 2.0 * MARGIN - LEADING) / LEADING) as usize;

    #[derive(Debug, Clone, PartialEq)]
    pub struct Line {
        text: String,
        bold: bool,
        size: f64,
    }

    impl Line {
        pub fn blank() -> Self {
            Self::body("")
        }
        #[cfg(test)]
        pub fn text(&self) -> &str {
            &self.text
        }
        pub fn body(text: &str) -> Self {
            Self {
                text: text.to_string(),
                bold: false,
                size: BODY_SIZE,
            }
        }
        /// A heading, wrapped over as many lines as its width needs.
        pub fn heading(text: &str) -> Vec<Self> {
            Self::wrapped(text, COLUMNS * 3 / 4, 11.0)
        }
        /// The title, wrapped over as many lines as its width needs.
        pub fn title(text: &str) -> Vec<Self> {
            Self::wrapped(text, COLUMNS / 2, 14.0)
        }
        fn wrapped(text: &str, width: usize, size: f64) -> Vec<Self> {
            wrap(text, width)
                .into_iter()
                .map(|text| Self {
                    text,
                    bold: true,
                    size,
                })
                .collect()
        }
    }

    /// Splits `text` into lines of at most `width` characters at spaces,
    /// breaking a word that is longer than a line.
    pub fn wrap(text: &str, width: usize) -> Vec<String> {
        let width = width.max(1);
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let mut word: Vec<char> = word.chars().collect();
            loop {
                let used = current.chars().count();
                let gap = usize::from(used > 0);
                if used + gap + word.len() <= width {
                    if gap == 1 {
                        current.push(' ');
                    }
                    current.extend(word.iter());
                    break;
                }
                if used > 0 {
                    lines.push(std::mem::take(&mut current));
                    continue;
                }
                current.extend(word.drain(..width));
                lines.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() || lines.is_empty() {
            lines.push(current);
        }
        lines
    }

    /// A table as text lines: columns as wide as their longest cell, the
    /// widest shrunk (and their cells wrapped) until the table fits.
    pub fn table(headers: &[String], rows: &[Vec<String>]) -> Vec<Line> {
        const GAP: usize = 2;
        let columns = headers.len();
        let mut widths: Vec<usize> = (0..columns)
            .map(|c| {
                std::iter::once(headers.get(c))
                    .chain(rows.iter().map(|r| r.get(c)))
                    .map(|cell| cell.map_or(0, |t| t.chars().count()))
                    .max()
                    .unwrap_or(0)
                    .max(1)
            })
            .collect();
        let total = |w: &[usize]| w.iter().sum::<usize>() + GAP * columns.saturating_sub(1);
        // Shrink the widest column first. A column keeps the width of its
        // longest word while any other can still shrink, so a cell breaks
        // between words, not inside one (OI-842); only a table that cannot
        // otherwise fit cuts words, down to a few characters.
        let longest_word = |c: usize| {
            std::iter::once(headers.get(c))
                .chain(rows.iter().map(|r| r.get(c)))
                .flatten()
                .flat_map(|t| t.split_whitespace())
                .map(|w| w.chars().count())
                .max()
                .unwrap_or(1)
        };
        let floors: Vec<usize> = (0..columns).map(longest_word).collect();
        for use_words in [true, false] {
            while total(&widths) > COLUMNS {
                let Some((widest, _)) = widths
                    .iter()
                    .enumerate()
                    .filter(|(i, w)| {
                        **w > if use_words {
                            floors.get(*i).copied().unwrap_or(6)
                        } else {
                            6
                        }
                    })
                    .max_by_key(|(i, w)| (**w, std::cmp::Reverse(*i)))
                else {
                    break;
                };
                if let Some(w) = widths.get_mut(widest) {
                    *w -= 1;
                }
            }
        }
        let render = |cells: &[String]| -> Vec<Line> {
            let wrapped: Vec<Vec<String>> = (0..columns)
                .zip(&widths)
                .map(|(c, w)| wrap(cells.get(c).map_or("", String::as_str), *w))
                .collect();
            let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
            (0..height)
                .map(|i| {
                    let mut text = String::new();
                    for (c, cell) in wrapped.iter().enumerate() {
                        let part = cell.get(i).map_or("", String::as_str);
                        if c + 1 < columns {
                            let w = widths.get(c).copied().unwrap_or(0);
                            let _ = write!(text, "{part:<w$}{:GAP$}", "");
                        } else {
                            text.push_str(part);
                        }
                    }
                    Line::body(text.trim_end())
                })
                .collect()
        };
        let mut lines = render(headers);
        for line in &mut lines {
            line.bold = true;
        }
        let rule: usize = total(&widths).min(COLUMNS);
        lines.push(Line::body(&"-".repeat(rule)));
        for row in rows {
            lines.extend(render(row));
        }
        lines
    }

    fn ascii(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            match c {
                '\\' | '(' | ')' => {
                    out.push('\\');
                    out.push(c);
                }
                c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
                c if c.is_control() => out.push(' '),
                _ => out.push('?'),
            }
        }
        out
    }

    /// Writes `lines` as pages of a PDF 1.4 file.
    pub fn render(title: &str, lines: &[Line]) -> Vec<u8> {
        let pages: Vec<&[Line]> = if lines.is_empty() {
            vec![&[]]
        } else {
            lines.chunks(LINES_PER_PAGE).collect()
        };
        // Objects: 1 catalog, 2 pages, 3 and 4 fonts, 5 info, then a page and
        // its content stream for each page.
        let first_page = 6;
        let mut objects: Vec<String> = Vec::new();
        objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
        let kids: Vec<String> = (0..pages.len())
            .map(|i| format!("{} 0 R", first_page + 2 * i))
            .collect();
        objects.push(format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        ));
        objects.push(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>"
                .to_string(),
        );
        objects.push(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold /Encoding /WinAnsiEncoding >>"
                .to_string(),
        );
        objects.push(format!(
            "<< /Title ({}) /Producer (OpenChuckCAM) >>",
            ascii(title)
        ));
        let count = pages.len();
        for (n, page) in pages.iter().enumerate() {
            let content_id = first_page + 2 * n + 1;
            objects.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_id} 0 R >>"
            ));
            let mut stream = String::new();
            let mut y = PAGE_HEIGHT - MARGIN - BODY_SIZE;
            for line in *page {
                if !line.text.is_empty() {
                    let font = if line.bold { "F2" } else { "F1" };
                    let _ = writeln!(
                        stream,
                        "BT /{font} {} Tf {MARGIN} {y:.2} Td ({}) Tj ET",
                        line.size,
                        ascii(&line.text)
                    );
                }
                y -= LEADING;
            }
            let _ = writeln!(
                stream,
                "BT /F1 {BODY_SIZE} Tf {MARGIN} {:.2} Td (Page {} of {count}) Tj ET",
                MARGIN / 2.0,
                n + 1
            );
            objects.push(format!(
                "<< /Length {} >>\nstream\n{stream}endstream",
                stream.len()
            ));
        }
        let mut out = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            let _ = write!(out, "{} 0 obj\n{body}\nendobj\n", i + 1);
        }
        let xref = out.len();
        let _ = writeln!(out, "xref\n0 {}", objects.len() + 1);
        out.push_str("0000000000 65535 f \n");
        for offset in &offsets {
            let _ = writeln!(out, "{offset:010} 00000 n ");
        }
        let _ = write!(
            out,
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        );
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn req_gc_12_req_op_21_tabs_around_the_axis_say_how_many_where_and_how_far() {
        let tab = Tab {
            x_start: openchuck_types::units::Millimetres(2.0),
            length: openchuck_types::units::Millimetres(1.5),
            shape: TabShape::Cylinder {
                diameter: openchuck_types::units::Millimetres(3.0),
            },
            layout: TabLayout::AroundAxis {
                count: 3,
                start_angle: openchuck_types::units::Degrees(30.0),
                radial_offset: openchuck_types::units::Millimetres(6.0),
            },
            auto: None,
        };
        assert_eq!(
            tab_row(1, &tab),
            [
                "2",
                "2",
                "3.5",
                "cylinder",
                "3",
                "3 around the axis, 6 mm from it, the first at 30 deg",
                "by hand"
            ]
        );
    }

    #[test]
    fn req_gc_12_the_generation_time_is_iso_8601_in_utc() {
        assert_eq!(iso8601_utc(at(0)), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_utc(at(951_827_696)), "2000-02-29T12:34:56Z");
        assert_eq!(iso8601_utc(at(1_792_575_005)), "2026-10-21T09:30:05Z");
        assert_eq!(iso8601_utc(at(4_102_444_799)), "2099-12-31T23:59:59Z");
        // A time before 1970 reads as 1970 rather than failing (NFR-07).
        assert_eq!(
            iso8601_utc(UNIX_EPOCH - Duration::from_secs(5)),
            "1970-01-01T00:00:00Z"
        );
    }

    /// A narrow table wraps a cell at its spaces only: no column shrinks below
    /// its longest word, so a strategy name is never cut inside a word
    /// (OI-842; GC-12).
    #[test]
    fn req_gc_12_a_table_breaks_lines_only_between_words() {
        let mut headers: Vec<String> = vec!["Operation".into(), "Strategy".into()];
        let mut row: Vec<String> = vec![
            "Finish the blade surfaces".into(),
            "Helical finishing".into(),
        ];
        // Many wide cells, so the table is over the line width.
        for i in 0..8 {
            headers.push(format!("Column {i}"));
            row.push("a b c d e f g h".into());
        }
        let rows = vec![row];
        let lines = pdf::table(&headers, &rows);
        let text: Vec<&str> = lines.iter().map(pdf::Line::text).collect();
        let all = text.join("\n");
        for word in ["Helical", "finishing", "Operation", "surfaces"] {
            assert!(all.contains(word), "{word} was cut:\n{all}");
        }
        assert!(text.iter().all(|l| l.chars().count() <= pdf::COLUMNS));
        // The same table gives the same lines (GC-07).
        assert_eq!(lines, pdf::table(&headers, &rows));
    }

    #[test]
    fn req_gc_12_the_clocks_read_the_time_they_are_given() {
        assert_eq!(FixedClock(at(7)).now(), at(7));
        assert!(SystemClock.now() > at(1_700_000_000));
    }
}
