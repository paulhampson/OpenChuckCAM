// SPDX-License-Identifier: AGPL-3.0-or-later
//! STL import, ASCII and binary (IMP-01), through `stl_io` (architecture
//! section 7). `stl_io` types stay inside this module.
//!
//! **Telling ASCII from binary.** A file is binary when its size is exactly
//! what the triangle count in its header says (84 + 50 bytes per triangle),
//! even if the header starts with `solid`, as some exporters write it.
//! Otherwise a file that starts with `solid` and is valid text is ASCII, and
//! anything else is binary, which must be at least as long as its count says.
//!
//! **Numbering.** Triangle `i` is the `i`th facet in the file, across every
//! `solid` of an ASCII file. Vertices are numbered in order of first use and
//! joined only when their coordinates are identical in the file (`-0` and
//! `0` are the same); nothing is merged by distance, so the checks in
//! `openchuck-geom` see the file as it is. The same part written as ASCII and
//! as binary gives the same mesh, because both hold single-precision values.
//!
//! **Units (IMP-03).** STL has no unit field, so a file is read in
//! millimetres unless the user chose a unit for it. Some exporters may write
//! one into the header (binary) or the `solid` line (ASCII) as `units=in` or
//! `UNITS: mm`; that is read as a hint and, if it is not millimetres, raises
//! a [`UnitQuestion`](crate::UnitQuestion), as does a part whose size is
//! implausible in millimetres. OrcaSlicer, Cura and FreeCAD all read STL as
//! millimetres; OrcaSlicer also asks when a part looks too small.

use std::collections::BTreeMap;
use std::io::Cursor;

use openchuck_types::geometry::{Point3, Vec3};
use openchuck_types::job::JobContext;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::model::{ModelFormat, ModelGeometry};
use openchuck_types::units::LengthUnit;

use crate::{
    ImportError, ImportOptions, ImportedModel, ModelImporter, STL_TESSELLATION_NOTICE,
    UnitQuestion, Units,
};

/// Size of a binary STL header plus its triangle count.
const BINARY_HEADER: usize = 84;
/// Size of one binary STL triangle record.
const BINARY_TRIANGLE: u64 = 50;
/// Facets read between cancellation checks and progress reports.
const CHECK_EVERY: usize = 1 << 16;

/// The STL importer.
#[derive(Debug, Clone, Copy, Default)]
pub struct StlImporter;

/// How an STL file is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StlEncoding {
    /// Text.
    Ascii,
    /// Binary.
    Binary,
}

impl ModelImporter for StlImporter {
    fn format(&self) -> ModelFormat {
        ModelFormat::Stl
    }

    fn import(
        &self,
        bytes: &[u8],
        options: &ImportOptions,
        context: &JobContext<'_>,
    ) -> Result<ImportedModel, ImportError> {
        read(bytes, options, context).map(|(model, _)| model)
    }
}

/// One facet as stored: normal and three corners in the file's unit.
#[derive(Debug, Clone, Copy)]
struct Facet {
    normal: [f32; 3],
    vertices: [[f32; 3]; 3],
}

impl From<stl_io::Triangle> for Facet {
    fn from(t: stl_io::Triangle) -> Self {
        Self {
            normal: t.normal.0,
            vertices: t.vertices.map(|v| v.0),
        }
    }
}

fn invalid(reason: impl Into<String>) -> ImportError {
    ImportError::Invalid {
        format: "STL",
        reason: reason.into(),
    }
}

/// Reads an STL file and says how it was encoded.
pub fn read(
    bytes: &[u8],
    options: &ImportOptions,
    context: &JobContext<'_>,
) -> Result<(ImportedModel, StlEncoding), ImportError> {
    let mut notes = vec![STL_TESSELLATION_NOTICE.to_owned()];
    let encoding = detect(bytes)?;
    let (header, facets) = match encoding {
        Detected::Ascii => read_ascii(bytes, context, &mut notes)?,
        Detected::Binary { count, extra } => {
            if extra > 0 {
                notes.push(format!(
                    "the file has {extra} bytes after its last triangle; they were ignored"
                ));
            }
            read_binary(bytes, count, context)?
        }
    };
    if facets.is_empty() {
        return Err(invalid("the file holds no triangles"));
    }

    let (file_unit, unknown) = unit_in_header(&header);
    if let Some(word) = unknown {
        notes.push(format!(
            "the file names the unit '{word}', which is not one OpenChuckCAM knows (um, mm, cm, m, in)"
        ));
    }
    let units = Units::choose(file_unit, options.units);
    let (mesh, stated_normals) = build_mesh(&facets, units.applied.millimetres(), context)?;
    let unit_question = match options.units {
        Some(_) => None,
        None => UnitQuestion::ask(file_unit, largest_size(&mesh)),
    };
    let model = ImportedModel {
        format: ModelFormat::Stl,
        geometry: ModelGeometry::Mesh(mesh),
        units,
        unit_question,
        stated_normals: Some(stated_normals),
        notes,
    };
    let encoding = match encoding {
        Detected::Ascii => StlEncoding::Ascii,
        Detected::Binary { .. } => StlEncoding::Binary,
    };
    Ok((model, encoding))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detected {
    Ascii,
    Binary { count: u32, extra: u64 },
}

fn detect(bytes: &[u8]) -> Result<Detected, ImportError> {
    if bytes.is_empty() {
        return Err(invalid("the file is empty"));
    }
    let len = bytes.len() as u64;
    let binary = bytes
        .get(80..BINARY_HEADER)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(|b| {
            let count = u32::from_le_bytes(b);
            (
                count,
                BINARY_HEADER as u64 + BINARY_TRIANGLE * u64::from(count),
            )
        });
    if let Some((count, size)) = binary
        && size == len
    {
        return Ok(Detected::Binary { count, extra: 0 });
    }
    let text = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let start = text
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(0);
    let rest = text.get(start..).unwrap_or_default();
    let starts_solid =
        rest.starts_with(b"solid") && rest.get(5).is_none_or(|b| b.is_ascii_whitespace());
    if starts_solid && (std::str::from_utf8(bytes).is_ok() || binary.is_none()) {
        return Ok(Detected::Ascii);
    }
    match binary {
        None => Err(invalid(format!(
            "the file is {len} bytes long, too short for a binary STL (at least 84 bytes), and \
             does not start with 'solid' as an ASCII STL does"
        ))),
        Some((count, size)) if len < size => Err(invalid(format!(
            "the binary header declares {count} triangles ({size} bytes), but the file has only \
             {len} bytes: it is truncated"
        ))),
        Some((count, size)) => Ok(Detected::Binary {
            count,
            extra: len - size,
        }),
    }
}

/// Reads a binary STL whose length has been checked against `count`.
fn read_binary(
    bytes: &[u8],
    count: u32,
    context: &JobContext<'_>,
) -> Result<(String, Vec<Facet>), ImportError> {
    let header = bytes.get(..80).unwrap_or_default();
    let header_text: String = header
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| if b.is_ascii() { char::from(b) } else { ' ' })
        .collect();
    // stl_io decides ASCII or binary by a leading "solid", which some binary
    // headers start with. The header is not data, so hide the word from it.
    let masked;
    let data = if bytes.starts_with(b"solid") {
        let mut copy = bytes.to_vec();
        if let Some(word) = copy.get_mut(..5) {
            word.fill(0);
        }
        masked = copy;
        masked.as_slice()
    } else {
        bytes
    };
    let mut cursor = Cursor::new(data);
    let reader = stl_io::create_stl_reader(&mut cursor)
        .map_err(|e| invalid(format!("cannot read the binary header: {e}")))?;
    let total = count as usize;
    let mut facets = Vec::with_capacity(total);
    for (i, triangle) in reader.enumerate().take(total) {
        if i % CHECK_EVERY == 0 {
            check_in(context, "read STL", i, Some(total))?;
        }
        let triangle = triangle.map_err(|e| invalid(format!("triangle {}: {e}", i + 1)))?;
        facets.push(Facet::from(triangle));
    }
    if facets.len() != total {
        return Err(invalid(format!(
            "the binary header declares {total} triangles, but only {} could be read",
            facets.len()
        )));
    }
    Ok((header_text, facets))
}

/// Reads an ASCII STL, one `solid` at a time.
fn read_ascii(
    bytes: &[u8],
    context: &JobContext<'_>,
    notes: &mut Vec<String>,
) -> Result<(String, Vec<Facet>), ImportError> {
    let text = std::str::from_utf8(bytes).map_err(|e| {
        invalid(format!(
            "the file is not valid text (at byte {}), and too short or the wrong size to be binary",
            e.valid_up_to()
        ))
    })?;
    let text = text.trim_start_matches('\u{feff}');

    // Split into solids at each line whose first word is "solid".
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.split_whitespace().next() == Some("solid") {
            starts.push(offset + (line.len() - line.trim_start().len()));
        }
        offset += line.len();
    }
    if starts.len() > 1 {
        notes.push(format!(
            "the file holds {} solids; they were read as one model",
            starts.len()
        ));
    }

    let mut header = String::new();
    let mut facets = Vec::new();
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(text.len());
        let chunk = text.get(start..end).unwrap_or_default();
        let first_line = chunk.lines().next().unwrap_or_default();
        header.push_str(
            first_line
                .trim_start()
                .strip_prefix("solid")
                .unwrap_or_default(),
        );
        header.push(' ');
        // stl_io wants "solid " with a space; a nameless "solid" line is valid STL.
        let normalised;
        let chunk = if chunk.starts_with("solid ") {
            chunk
        } else {
            normalised = format!("solid {}", chunk.get(5..).unwrap_or_default());
            normalised.as_str()
        };
        // A solid with no "endsolid" line was cut short, whatever the parser
        // trips over first.
        let ended = chunk
            .lines()
            .any(|line| line.trim_start().starts_with("endsolid"));
        let mut cursor = Cursor::new(chunk.as_bytes());
        let reader = stl_io::create_stl_reader(&mut cursor)
            .map_err(|e| invalid(format!("cannot read the 'solid' line: {e}")))?;
        for triangle in reader {
            let n = facets.len();
            if n % CHECK_EVERY == 0 {
                check_in(context, "read STL", n, None)?;
            }
            let triangle = triangle.map_err(|e| {
                if e.kind() == std::io::ErrorKind::UnexpectedEof || !ended {
                    invalid(format!(
                        "the file ends inside facet {} or before 'endsolid': it is truncated ({e})",
                        n + 1
                    ))
                } else {
                    invalid(format!("facet {}: {e}", n + 1))
                }
            })?;
            facets.push(Facet::from(triangle));
        }
    }
    Ok((header, facets))
}

/// Joins identical vertices and converts to millimetres.
fn build_mesh(
    facets: &[Facet],
    to_mm: f64,
    context: &JobContext<'_>,
) -> Result<(TriangleMesh, Vec<Vec3>), ImportError> {
    let mut index: BTreeMap<[u32; 3], u32> = BTreeMap::new();
    let mut vertices = Vec::new();
    let mut triangles = Vec::with_capacity(facets.len());
    let mut normals = Vec::with_capacity(facets.len());
    for (i, facet) in facets.iter().enumerate() {
        if i % CHECK_EVERY == 0 {
            check_in(context, "join vertices", i, Some(facets.len()))?;
        }
        let mut corners = [0u32; 3];
        for (corner, v) in corners.iter_mut().zip(facet.vertices) {
            if !v.iter().all(|c| c.is_finite()) {
                return Err(invalid(format!(
                    "triangle {} has a coordinate that is not a finite number",
                    i + 1
                )));
            }
            // -0.0 and 0.0 are the same place.
            let key = v.map(|c| if c == 0.0 { 0.0f32 } else { c }.to_bits());
            let next = u32::try_from(vertices.len())
                .map_err(|_| invalid("the file has more vertices than OpenChuckCAM can hold"))?;
            *corner = *index.entry(key).or_insert_with(|| {
                let [x, y, z] = v.map(|c| f64::from(c) * to_mm);
                vertices.push(Point3::new(x, y, z));
                next
            });
        }
        triangles.push(corners);
        let [x, y, z] = facet.normal.map(f64::from);
        normals.push(Vec3::new(x, y, z));
    }
    Ok((
        TriangleMesh {
            vertices,
            triangles,
        },
        normals,
    ))
}

/// The largest side of the mesh's bounding box, in millimetres.
fn largest_size(mesh: &TriangleMesh) -> f64 {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for p in &mesh.vertices {
        for (k, c) in [p.x, p.y, p.z].into_iter().enumerate() {
            if let (Some(lo), Some(hi)) = (min.get_mut(k), max.get_mut(k)) {
                *lo = lo.min(c);
                *hi = hi.max(c);
            }
        }
    }
    min.iter()
        .zip(&max)
        .map(|(lo, hi)| hi - lo)
        .fold(0.0, f64::max)
}

fn check_in(
    context: &JobContext<'_>,
    stage: &str,
    done: usize,
    total: Option<usize>,
) -> Result<(), ImportError> {
    context
        .check_cancelled()
        .map_err(|_| ImportError::Cancelled)?;
    context.report(stage, done as u64, total.map(|t| t as u64));
    Ok(())
}

/// The unit named in an STL header as `unit=…`, `units: …` and the like
/// (ignoring case), and the word itself if it names no unit we know.
pub fn unit_in_header(header: &str) -> (Option<LengthUnit>, Option<String>) {
    let lower = header.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(at) = rest.find("unit") {
        let after = rest.get(at + 4..).unwrap_or_default();
        rest = after;
        let after = after.strip_prefix('s').unwrap_or(after).trim_start();
        let Some(value) = after
            .strip_prefix('=')
            .or_else(|| after.strip_prefix(':'))
            .map(str::trim_start)
        else {
            continue;
        };
        let word: String = value
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect();
        if word.is_empty() {
            continue;
        }
        return match LengthUnit::parse(&word) {
            Some(unit) => (Some(unit), None),
            None => (None, Some(word)),
        };
    }
    (None, None)
}
