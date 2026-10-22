// SPDX-License-Identifier: AGPL-3.0-or-later
//! The reference jobs as project folders (OI-913, OI-920; lane GATES).
//!
//! `testdata/projects/<job>/` holds each reference job's project files
//! without its model: the parts are generated meshes, and the exact ASCII
//! STL the golden programs were made from is 8.7 to 70 MB, beyond the
//! test-data size policy (decision record 0004). [`write_projects`] copies
//! the folders out and writes each part beside them, from the same parts
//! and the same writer the end-to-end suite imports, so the model hash each
//! project records matches. It is shared by `tests/e2e.rs` and the
//! `sample_projects` example, which the owner runs to get folders to open.

// Shared by a test and an example: each uses a different part of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

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

/// The generated part of reference job `job` and its mesh (stations along
/// X, vertices round the axis), or `None` for a job read from a file.
pub fn generated_part(job: &str) -> Option<(Box<dyn RadialPart>, (u32, u32))> {
    let part: Box<dyn RadialPart> = match job {
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
            write_stl(part.as_ref(), mesh, &stl)?;
        }
        written.push(target);
    }
    Ok(written)
}
