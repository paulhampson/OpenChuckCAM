// SPDX-License-Identifier: AGPL-3.0-or-later
//! STL corpus tests (IMP-01, IMP-03, IMP-04, NFR-07; verification plan
//! section 5, "Import"). The corpus is `testdata/stl/`, written by its
//! `generate.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use std::path::{Path, PathBuf};

use openchuck_geom::check::check_mesh;
use openchuck_geom::summary;
use openchuck_import::stl::{StlEncoding, read, unit_in_header};
use openchuck_import::{
    ImportError, ImportOptions, ImportedModel, STL_TESSELLATION_NOTICE, UnitSource, import_bytes,
    import_file,
};
use openchuck_types::job::{CancelToken, JobContext, NoProgress};
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::model::{ModelFormat, ModelGeometry};
use openchuck_types::units::LengthUnit;
use proptest::prelude::*;

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/stl")
        .join(name)
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(corpus(name)).unwrap()
}

fn import_with(name: &str, units: Option<LengthUnit>) -> Result<ImportedModel, ImportError> {
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    import_file(
        &corpus(name),
        &ImportOptions { units },
        &JobContext::new(&cancel, &progress),
    )
}

fn import(name: &str) -> ImportedModel {
    import_with(name, None).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn mesh(model: &ImportedModel) -> &TriangleMesh {
    match &model.geometry {
        ModelGeometry::Mesh(mesh) => mesh,
        other => panic!("not a mesh: {other:?}"),
    }
}

fn encoding(name: &str) -> StlEncoding {
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    read(
        &bytes(name),
        &ImportOptions::default(),
        &JobContext::new(&cancel, &progress),
    )
    .unwrap()
    .1
}

#[test]
fn req_imp_01_ascii_and_binary_give_identical_meshes() {
    let reference = import("cube_ascii.stl");
    assert_eq!(encoding("cube_ascii.stl"), StlEncoding::Ascii);
    for name in [
        "cube_binary.stl",
        "cube_binary_solid_header.stl",
        "cube_nameless_crlf.stl",
    ] {
        let other = import(name);
        assert_eq!(mesh(&other), mesh(&reference), "{name}");
        assert_eq!(other.stated_normals, reference.stated_normals, "{name}");
    }
    assert_eq!(encoding("cube_binary.stl"), StlEncoding::Binary);
    // A binary file whose header starts with "solid" is still binary.
    assert_eq!(
        encoding("cube_binary_solid_header.stl"),
        StlEncoding::Binary
    );
    assert_eq!(encoding("cube_nameless_crlf.stl"), StlEncoding::Ascii);
}

#[test]
fn req_imp_01_corpus_counts_and_bounds_match_references() {
    // (file, triangles, vertices, bounds min, bounds max) in millimetres.
    type Case = (&'static str, usize, usize, [f64; 3], [f64; 3]);
    let cases: [Case; 6] = [
        ("cube_ascii.stl", 12, 8, [0.0; 3], [10.0; 3]),
        ("cube_binary.stl", 12, 8, [0.0; 3], [10.0; 3]),
        ("cube_inch.stl", 12, 8, [0.0; 3], [25.4; 3]),
        ("cube_metre_binary.stl", 12, 8, [0.0; 3], [500.0; 3]),
        ("two_cubes.stl", 24, 16, [0.0; 3], [30.0, 10.0, 10.0]),
        (
            "defect_non_manifold.stl",
            13,
            9,
            [0.0, -10.0, -10.0],
            [10.0; 3],
        ),
    ];
    for (name, triangles, vertices, min, max) in cases {
        let model = import(name);
        assert_eq!(model.format, ModelFormat::Stl);
        let s = summary(mesh(&model));
        assert_eq!((s.triangles, s.vertices), (triangles, vertices), "{name}");
        let b = s.bounds.unwrap();
        assert_eq!([b.min.x, b.min.y, b.min.z], min, "{name}");
        assert_eq!([b.max.x, b.max.y, b.max.z], max, "{name}");
    }
    // Triangle i is the i-th facet of the file, across solids.
    let two = import("two_cubes.stl");
    let m = mesh(&two);
    assert!(m.triangle(11).unwrap().iter().all(|p| p.x <= 10.0));
    assert!(m.triangle(12).unwrap().iter().all(|p| p.x >= 20.0));
    assert!(two.notes.iter().any(|n| n.contains("2 solids")));
}

#[test]
fn req_imp_01_every_stl_says_its_tessellation_is_fixed() {
    for name in ["cube_ascii.stl", "cube_binary.stl"] {
        assert!(
            import(name)
                .notes
                .iter()
                .any(|n| n == STL_TESSELLATION_NOTICE),
            "{name}"
        );
    }
    assert!(STL_TESSELLATION_NOTICE.contains("gouge tolerance"));
    assert!(STL_TESSELLATION_NOTICE.contains("measured against this mesh"));
}

#[test]
fn req_imp_03_units_from_the_file_and_override() {
    let inch = import("cube_inch.stl");
    assert_eq!(inch.units.file, Some(LengthUnit::Inch));
    assert_eq!(inch.units.applied, LengthUnit::Inch);
    assert_eq!(inch.units.source, UnitSource::File);
    assert_eq!(mesh(&inch).vertices[1].x, 25.4);

    let metre = import("cube_metre_binary.stl");
    assert_eq!(metre.units.applied, LengthUnit::Metre);
    assert_eq!(summary(mesh(&metre)).bounds.unwrap().max.z, 500.0);

    // No unit in the file: millimetres are assumed, and the report says so.
    let mm = import("cube_ascii.stl");
    assert_eq!(mm.units.file, None);
    assert_eq!(mm.units.applied, LengthUnit::Millimetre);
    assert_eq!(mm.units.source, UnitSource::Assumed);
    assert!(mm.units.describe().contains("assumed"));

    // The user's choice wins over the file's.
    let overridden = import_with("cube_inch.stl", Some(LengthUnit::Millimetre)).unwrap();
    assert_eq!(overridden.units.source, UnitSource::Override);
    assert_eq!(summary(mesh(&overridden)).bounds.unwrap().max.x, 1.0);
    assert!(overridden.units.describe().contains("the file states in"));
    let scaled = import_with("cube_ascii.stl", Some(LengthUnit::Inch)).unwrap();
    assert_eq!(summary(mesh(&scaled)).bounds.unwrap().max.x, 254.0);
    let cm = import_with("cube_ascii.stl", Some(LengthUnit::Centimetre)).unwrap();
    assert_eq!(summary(mesh(&cm)).bounds.unwrap().max.x, 100.0);

    // An unknown unit word is reported and millimetres are assumed.
    let unknown = import("cube_unknown_unit.stl");
    assert_eq!(unknown.units.source, UnitSource::Assumed);
    assert!(unknown.notes.iter().any(|n| n.contains("'furlong'")));
}

#[test]
fn req_imp_03_unit_words_in_headers() {
    for (header, unit) in [
        ("part units=in", Some(LengthUnit::Inch)),
        ("UNITS = mm", Some(LengthUnit::Millimetre)),
        ("COLOR=1 Unit: Inches", Some(LengthUnit::Inch)),
        ("units=metres", Some(LengthUnit::Metre)),
        ("unit:cm", Some(LengthUnit::Centimetre)),
        ("community part", None),
        ("units in mm", None),
        ("units=", None),
        ("", None),
    ] {
        assert_eq!(unit_in_header(header).0, unit, "{header:?}");
    }
    assert_eq!(
        unit_in_header("units=parsec"),
        (None, Some("parsec".to_owned()))
    );
}

#[test]
fn req_imp_04_known_bad_samples_report_the_right_counts() {
    let report = |name: &str| {
        let model = import(name);
        check_mesh(mesh(&model), model.stated_normals.as_deref()).unwrap()
    };
    for name in [
        "cube_ascii.stl",
        "cube_binary.stl",
        "cube_inch.stl",
        "two_cubes.stl",
    ] {
        let r = report(name);
        assert!(r.is_clean(), "{name}: {:?}", r.problems());
    }

    let hole = report("defect_hole.stl");
    assert_eq!((hole.holes.len(), hole.boundary_edges()), (1, 3));
    assert_eq!(hole.problems().len(), 1);

    let flipped = report("defect_flipped.stl");
    assert_eq!(flipped.flipped_triangles, vec![5]);
    assert!(flipped.normal_mismatches.is_empty());
    assert_eq!(flipped.problems().len(), 1);

    let normal = report("defect_normal.stl");
    assert_eq!(normal.normal_mismatches, vec![3]);
    assert!(normal.flipped_triangles.is_empty());

    let inside_out = report("defect_inside_out.stl");
    assert_eq!(inside_out.flipped_triangles.len(), 12);
    assert_eq!(inside_out.inside_out_shells, 1);
    assert!(inside_out.normal_mismatches.is_empty());

    let fin = report("defect_non_manifold.stl");
    // Vertices are numbered in order of first use: (10, 0, 0) is vertex 2.
    assert_eq!(fin.non_manifold_edges, vec![[0, 2]]);
    assert_eq!((fin.holes.len(), fin.boundary_edges()), (1, 2));

    let degenerate = report("defect_degenerate.stl");
    assert_eq!(degenerate.degenerate_triangles, vec![12]);
    assert_eq!(degenerate.problems().len(), 1);
}

#[test]
fn req_nfr_07_damaged_files_give_clear_errors() {
    for (name, expected) in [
        ("broken_empty.stl", "the file is empty"),
        ("broken_garbage.stl", "too short for a binary STL"),
        ("broken_truncated_binary.stl", "it is truncated"),
        ("broken_truncated_ascii.stl", "it is truncated"),
        ("broken_bad_number.stl", "facet 1"),
        ("broken_bad_keyword.stl", "facet 1"),
        (
            "broken_nan_binary.stl",
            "triangle 1 has a coordinate that is not a finite number",
        ),
        ("broken_no_triangles.stl", "holds no triangles"),
    ] {
        let err = import_with(name, None).unwrap_err();
        let text = err.to_string();
        assert!(text.starts_with("not a valid STL file: "), "{name}: {text}");
        assert!(text.contains(expected), "{name}: {text}");
    }
}

#[test]
fn req_nfr_07_wrong_formats_and_missing_files_give_clear_errors() {
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    let context = JobContext::new(&cancel, &progress);
    let options = ImportOptions::default();
    let err = import_bytes("part.obj", b"v 0 0 0", &options, &context).unwrap_err();
    assert!(matches!(err, ImportError::UnknownFormat(_)), "{err}");
    let err = import_bytes("part.STEP", b"ISO-10303-21;", &options, &context).unwrap_err();
    assert_eq!(err.to_string(), "STEP import is not available yet");
    let err = import_file(&corpus("missing.stl"), &options, &context).unwrap_err();
    assert!(err.to_string().starts_with("cannot read"), "{err}");
    // Upper-case extensions are STL too.
    let text = bytes("cube_ascii.stl");
    assert!(import_bytes("CUBE.STL", &text, &options, &context).is_ok());
}

#[test]
fn import_can_be_cancelled() {
    let (cancel, progress) = (CancelToken::new(), NoProgress);
    cancel.cancel();
    let err = import_bytes(
        "cube.stl",
        &bytes("cube_binary.stl"),
        &ImportOptions::default(),
        &JobContext::new(&cancel, &progress),
    )
    .unwrap_err();
    assert!(matches!(err, ImportError::Cancelled), "{err}");
}

/// A binary STL of `facets` (normal, then three corners) written here,
/// independently of the importer.
fn write_binary(facets: &[[[f32; 3]; 4]]) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    out.extend((facets.len() as u32).to_le_bytes());
    for facet in facets {
        for v in facet {
            for c in v {
                out.extend(c.to_le_bytes());
            }
        }
        out.extend([0, 0]);
    }
    out
}

/// The same facets as ASCII, printing each value so it reads back exactly.
fn write_ascii(facets: &[[[f32; 3]; 4]]) -> Vec<u8> {
    let mut out = String::from("solid generated\n");
    for [n, a, b, c] in facets {
        out += &format!(
            "facet normal {:?} {:?} {:?}\n outer loop\n",
            n[0], n[1], n[2]
        );
        for v in [a, b, c] {
            out += &format!("  vertex {:?} {:?} {:?}\n", v[0], v[1], v[2]);
        }
        out += " endloop\nendfacet\n";
    }
    out += "endsolid generated\n";
    out.into_bytes()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Any facets written as ASCII and as binary import to the same mesh,
    /// in double precision without loss (IMP-01, NFR-05).
    #[test]
    fn req_imp_01_ascii_and_binary_agree_for_any_facets(
        facets in proptest::collection::vec(
            proptest::array::uniform4(proptest::array::uniform3(-1.0e6f32..1.0e6)),
            1..20,
        )
    ) {
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        let context = JobContext::new(&cancel, &progress);
        let options = ImportOptions::default();
        let a = import_bytes("a.stl", &write_ascii(&facets), &options, &context).unwrap();
        let b = import_bytes("b.stl", &write_binary(&facets), &options, &context).unwrap();
        prop_assert_eq!(&a.geometry, &b.geometry);
        let m = mesh(&a);
        prop_assert_eq!(m.triangles.len(), facets.len());
        for (t, facet) in facets.iter().enumerate() {
            let corners = m.triangle(t).unwrap();
            for (p, v) in corners.iter().zip(&facet[1..]) {
                prop_assert_eq!([p.x, p.y, p.z], v.map(f64::from));
            }
        }
    }

    /// Any bytes give a model or an error, never a panic (NFR-07).
    #[test]
    fn req_nfr_07_any_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..400)) {
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        let _ = import_bytes("x.stl", &data, &ImportOptions::default(), &JobContext::new(&cancel, &progress));
    }

    /// Damaged copies of good files give a model or an error, never a panic.
    #[test]
    fn req_nfr_07_damaged_corpus_files_never_panic(
        which in 0usize..4,
        edits in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8),
        cut in any::<prop::sample::Index>(),
    ) {
        let name = ["cube_ascii.stl", "cube_binary.stl", "two_cubes.stl", "cube_binary_solid_header.stl"][which];
        let mut data = bytes(name);
        for (at, byte) in edits {
            let i = at.index(data.len());
            data[i] = byte;
        }
        data.truncate(cut.index(data.len() + 1));
        let (cancel, progress) = (CancelToken::new(), NoProgress);
        if let Ok(model) = import_bytes(name, &data, &ImportOptions::default(), &JobContext::new(&cancel, &progress)) {
            // Whatever reads is a valid mesh the checks accept.
            let m = mesh(&model);
            prop_assert!(m.validate().is_ok());
            prop_assert!(check_mesh(m, model.stated_normals.as_deref()).is_ok());
        }
    }
}
