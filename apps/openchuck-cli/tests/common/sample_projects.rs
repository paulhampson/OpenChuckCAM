// SPDX-License-Identifier: AGPL-3.0-or-later
//! The reference jobs as project folders (OI-913, OI-920; lane GATES).
//!
//! `testdata/projects/<job>/` holds each reference job's project files
//! without its model: the parts are generated meshes, and the exact ASCII
//! STL the golden programs were made from is 8.7 to 70 MB, beyond the
//! test-data size policy (decision record 0004). [`write_projects`] copies
//! the folders out and writes each part beside them, from the same parts
//! and the same writer the end-to-end suite imports. Native floating-point
//! math can vary the STL bytes between platforms, so the newly exported copy
//! records the hash of the model actually written. Committed fixtures stay
//! unchanged. It is shared by `tests/e2e.rs` and the
//! `sample_projects` example, which the owner runs to get folders to open.

// Shared by a test and an example: each uses a different part of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use openchuck_app::project::{ContentHash, Item, file};
use openchuck_testkit::parts::{Cone, HelicalGroove, LobedVase, RadialPart, Sphere};

/// Where the committed project folders are.
pub fn projects_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/projects")
}

/// The model file every sample project imports, relative to its folder.
pub const PART_FILE: &str = "models/part.stl";

/// The cone of the `cone*` jobs.
pub fn cone() -> Cone {
    Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 30.0,
    }
}

/// The groove of the `groove` job.
pub fn groove() -> HelicalGroove {
    HelicalGroove {
        radius: 12.0,
        length: 30.0,
        pitch: 12.0,
        depth: 1.5,
    }
}

/// The vase of the `lobed` job.
pub fn lobed() -> LobedVase {
    LobedVase {
        base: 10.0,
        wave: 1.5,
        wave_length: 40.0,
        lobe: 1.0,
        lobes: 3,
        length: 20.0,
    }
}

/// The name of the gate 5 first-cut project (decision record 0221, OI-914).
pub const FIRST_CUT: &str = "first_cut";

/// The first-cut project's part: a cone 25 mm long from 20 mm diameter at
/// its start (the head, towards the chuck) to 10 mm at its end, where it
/// meets the 10 mm tailstock tab flush (the owner's answers on OI-914, 22 Oct
/// 2026; the small diameter and the direction are lane GATES's reading).
pub fn first_cut_part() -> Cone {
    Cone {
        start_radius: 10.0,
        end_radius: 5.0,
        length: 25.0,
    }
}

/// The generated part of reference job `job` and its mesh (stations along
/// X, vertices round the axis), or `None` for a job read from a file.
pub fn generated_part(job: &str) -> Option<(Box<dyn RadialPart>, (u32, u32))> {
    let part: Box<dyn RadialPart> = match job {
        // A cone is straight along X, so two stations are exact; 360 round
        // the axis keep the facets within 0.0004 mm of the surface.
        FIRST_CUT => return Some((Box::new(first_cut_part()), (2, 360))),
        "groove" => Box::new(groove()),
        "sphere" => Box::new(Sphere { radius: 10.0 }),
        "lobed" => return Some((Box::new(lobed()), (201, 720))),
        "cone" | "cone_axial" | "cone_indexed" | "cone_axial_lead" | "cone_held_tabs" => {
            Box::new(cone())
        }
        _ => return None,
    };
    Some((part, (121, 144)))
}

/// The part as ASCII STL. Shortest round-trip formatting keeps every vertex
/// exact, so the imported mesh is the generated one.
pub fn write_stl(part: &dyn RadialPart, mesh: (u32, u32), path: &Path) -> std::io::Result<()> {
    use std::fmt::Write as _;
    let mesh = part.mesh(mesh.0, mesh.1);
    let mut text = String::from("solid part\n");
    for t in &mesh.triangles {
        let corner = |i: usize| {
            t.get(i)
                .and_then(|v| mesh.vertices.get(*v as usize))
                .copied()
        };
        let (Some(a), Some(b), Some(c)) = (corner(0), corner(1), corner(2)) else {
            return Err(std::io::Error::other(
                "a triangle names a vertex the mesh lacks",
            ));
        };
        let cross = (b - a).cross(c - a);
        let len = cross.length();
        let n = if len > 0.0 {
            cross * (1.0 / len)
        } else {
            cross
        };
        // Writing to a String cannot fail.
        let _ = writeln!(
            text,
            "facet normal {:e} {:e} {:e}\n outer loop",
            n.x, n.y, n.z
        );
        for p in [a, b, c] {
            let _ = writeln!(text, "  vertex {:e} {:e} {:e}", p.x, p.y, p.z);
        }
        text.push_str(" endloop\nendfacet\n");
    }
    text.push_str("endsolid part\n");
    std::fs::write(path, text)
}

/// The jobs with a committed folder, in name order.
pub fn committed_jobs() -> std::io::Result<Vec<String>> {
    let mut jobs = Vec::new();
    for entry in std::fs::read_dir(projects_dir())? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            jobs.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    jobs.sort();
    Ok(jobs)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Copies the committed project of each job in `jobs` into `out/<job>` and
/// writes its generated part there. Refuses a folder that already exists, so
/// nothing of the user's is overwritten. Returns the folders written.
pub fn write_projects(out: &Path, jobs: &[String]) -> std::io::Result<Vec<PathBuf>> {
    write_projects_with(out, jobs, write_stl)
}

fn write_projects_with(
    out: &Path,
    jobs: &[String],
    mut write_part: impl FnMut(&dyn RadialPart, (u32, u32), &Path) -> std::io::Result<()>,
) -> std::io::Result<Vec<PathBuf>> {
    // Check every name first: a job must be a committed project, so a name
    // such as "../x" or an absolute path cannot copy outside `out`, and a typo
    // leaves no empty folder behind (NFR-07).
    let committed = committed_jobs()?;
    if let Some(bad) = jobs.iter().find(|job| !committed.contains(job)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "{bad:?} is not a committed project; choose one of: {}",
                committed.join(", ")
            ),
        ));
    }
    let mut written = Vec::new();
    for job in jobs {
        let target = out.join(job);
        if target.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "{} already exists; choose an empty folder",
                    target.display()
                ),
            ));
        }
        copy_tree(&projects_dir().join(job), &target)?;
        if let Some((part, mesh)) = generated_part(job) {
            let stl = target.join(PART_FILE);
            std::fs::create_dir_all(stl.parent().unwrap_or(&target))?;
            write_part(part.as_ref(), mesh, &stl)?;
            // We created this model for a fresh export. Record its actual
            // identity rather than a different platform's generated bytes;
            // ordinary changed-model detection remains in force afterward.
            let mut project = file::load(&target).map_err(std::io::Error::other)?;
            let mut model = project
                .models()
                .iter()
                .find(|model| model.path.as_str() == PART_FILE)
                .cloned()
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "the generated part has no model reference in the sample project",
                    )
                })?;
            model.hash = ContentHash::of_file(&stl).map_err(std::io::Error::other)?;
            project
                .replace(Item::Model(model))
                .map_err(std::io::Error::other)?;
            file::save(&project, &target).map_err(std::io::Error::other)?;
        }
        written.push(target);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use openchuck_app::project::{ContentHash, Item, ModelCheck, file};

    /// NFR-07: a job name that is not a committed project, including one that
    /// climbs out of the output folder, is refused with a clear error before
    /// anything is created or copied.
    #[test]
    fn req_nfr_07_uncommitted_or_escaping_job_names_are_refused() {
        let base = std::env::temp_dir().join(format!("openchuck-jobname-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        for bad in ["../escaped", "..", "a/b", "/abs", "no_such_job", ""] {
            let err = write_projects(&out, &[bad.to_string()]).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "{bad:?}");
            assert!(err.to_string().contains("not a committed project"), "{err}");
        }
        assert_eq!(
            std::fs::read_dir(&out).unwrap().count(),
            0,
            "no stray folder"
        );
        assert!(!base.join("escaped").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A generated model belongs to the new export, so its recorded identity
    /// must describe its actual bytes even when platform math changes them.
    /// Subsequent edits must still trigger PRJ-02's ordinary changed-file check.
    #[test]
    fn req_prj_02_req_gc_07_generated_exports_hash_the_written_model_and_detect_later_changes() {
        use std::io::Write as _;

        let out =
            std::env::temp_dir().join(format!("openchuck-export-hash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        let source = projects_dir().join(FIRST_CUT);
        let original_text = std::fs::read(source.join("project.toml")).unwrap();
        let mut expected = file::load(&source).unwrap();
        let original_model = expected.models().first().unwrap().clone();
        let folders = write_projects_with(&out, &[FIRST_CUT.into()], |part, mesh, path| {
            write_stl(part, mesh, path)?;
            // A harmless byte variant stands in for platform-dependent STL
            // formatting without changing any vertex or machining input.
            std::fs::OpenOptions::new()
                .append(true)
                .open(path)?
                .write_all(b"\n")
        })
        .unwrap();
        let folder = folders.first().unwrap();
        let model_path = folder.join(PART_FILE);
        let written_hash = ContentHash::of_file(&model_path).unwrap();
        assert_ne!(written_hash, original_model.hash);
        let exported = file::load(folder).unwrap();
        let model = exported.models().first().unwrap();
        assert_eq!(model.check(folder).unwrap(), ModelCheck::Unchanged);
        let mut expected_model = original_model;
        expected_model.hash = written_hash;
        expected.replace(Item::Model(expected_model)).unwrap();
        assert_eq!(
            exported, expected,
            "retain every other project value and ID"
        );
        assert_eq!(
            std::fs::read(source.join("project.toml")).unwrap(),
            original_text
        );
        std::fs::OpenOptions::new()
            .append(true)
            .open(model_path)
            .unwrap()
            .write_all(b"\n")
            .unwrap();
        assert!(matches!(
            model.check(folder).unwrap(),
            ModelCheck::Changed { .. }
        ));
        std::fs::remove_dir_all(out).unwrap();
    }
}
