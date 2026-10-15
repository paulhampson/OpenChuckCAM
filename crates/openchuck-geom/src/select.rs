// SPDX-License-Identifier: AGPL-3.0-or-later
//! Triangle region selection on meshes (UI-04; work breakdown slice 10
//! item 2; architecture section 5.4).
//!
//! A selection is a [`TriangleSet`]: triangle indices into the mesh, in
//! ascending order, which is also how a selection set stores them (PRJ-06).
//! The algorithms are:
//!
//! - [`flood_fill`]: every triangle connected to the seeds through shared
//!   edges (a whole shell);
//! - [`flood_fill_by_angle`]: the same, but a step across an edge is taken
//!   only when the two triangles' normals differ by at most the given angle,
//!   which selects a smooth patch such as one face of a block or a fillet;
//! - [`select_by_direction`]: every triangle whose normal lies within an
//!   angle of a direction, wherever it is on the part (a cone test, not
//!   connected).
//!
//! A triangle that shares an edge is a neighbour whatever its winding, so a
//! non-manifold edge joins every triangle that uses it. A degenerate triangle
//! (zero area) has no normal: the angle-limited fills can be seeded on one
//! but never spread through one, and [`select_by_direction`] never picks
//! one. Results are sorted sets, so they are the same on every run (NFR-06).

use std::collections::BTreeSet;

use openchuck_types::geometry::Vec3;
use openchuck_types::mesh::TriangleMesh;
use openchuck_types::{Error, Result};

use crate::edges::EdgeMap;

/// Selected triangles by index, ascending.
pub type TriangleSet = BTreeSet<u32>;

/// The unit normal of every triangle (right-hand rule on its winding), `None`
/// for a degenerate triangle or one that refers to a missing vertex.
pub fn triangle_normals(mesh: &TriangleMesh) -> Vec<Option<Vec3>> {
    (0..mesh.triangles.len())
        .map(|t| {
            let [a, b, c] = mesh.triangle(t)?;
            let n = (b - a).cross(c - a);
            let length = n.length();
            // A sliver whose normal is lost to rounding is degenerate too.
            (length.is_finite() && length > 1e-300).then(|| n * (1.0 / length))
        })
        .collect()
}

/// The part of `mesh` that `selected` names: the same vertices and only the
/// selected triangles, in ascending order. Fails if a triangle is not in the
/// mesh.
pub fn submesh(mesh: &TriangleMesh, selected: &TriangleSet) -> Result<TriangleMesh> {
    let triangles = selected
        .iter()
        .map(|&t| {
            mesh.triangles.get(t as usize).copied().ok_or_else(|| {
                Error::invalid(
                    "selection",
                    format!(
                        "triangle {t} does not exist; the mesh has {} triangles",
                        mesh.triangles.len()
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(TriangleMesh {
        vertices: mesh.vertices.clone(),
        triangles,
    })
}

fn check_seeds(mesh: &TriangleMesh, seeds: &[u32]) -> Result<()> {
    match seeds.iter().find(|&&s| s as usize >= mesh.triangles.len()) {
        Some(bad) => Err(Error::invalid(
            "seed",
            format!(
                "triangle {bad} does not exist; the mesh has {} triangles",
                mesh.triangles.len()
            ),
        )),
        None => Ok(()),
    }
}

fn check_angle(field: &str, degrees: f64) -> Result<()> {
    if degrees.is_finite() && (0.0..=180.0).contains(&degrees) {
        Ok(())
    } else {
        Err(Error::invalid(
            field,
            format!("{degrees} must be a number of degrees from 0 to 180"),
        ))
    }
}

/// Every triangle connected to `seeds` through shared edges. Fails if a seed
/// is not a triangle of the mesh.
pub fn flood_fill(mesh: &TriangleMesh, edges: &EdgeMap, seeds: &[u32]) -> Result<TriangleSet> {
    check_seeds(mesh, seeds)?;
    Ok(fill(mesh, edges, seeds, |_, _| true))
}

/// Every triangle reachable from `seeds` by steps across shared edges where
/// the two normals differ by at most `max_edge_angle` degrees. Fails if a seed
/// is not a triangle of the mesh or the angle is not from 0 to 180.
pub fn flood_fill_by_angle(
    mesh: &TriangleMesh,
    edges: &EdgeMap,
    seeds: &[u32],
    max_edge_angle: f64,
) -> Result<TriangleSet> {
    check_seeds(mesh, seeds)?;
    check_angle("max_edge_angle", max_edge_angle)?;
    let normals = triangle_normals(mesh);
    let least_cosine = max_edge_angle.to_radians().cos();
    Ok(fill(mesh, edges, seeds, |from, to| {
        match (
            normals.get(from as usize).copied().flatten(),
            normals.get(to as usize).copied().flatten(),
        ) {
            // A hair of slack, so an angle of exactly the limit passes.
            (Some(a), Some(b)) => a.dot(b) >= least_cosine - 1e-12,
            _ => false,
        }
    }))
}

/// Every triangle whose normal is within `max_angle` degrees of `direction`,
/// wherever it lies. Fails if the direction is zero or not finite, or the
/// angle is not from 0 to 180.
pub fn select_by_direction(
    mesh: &TriangleMesh,
    direction: Vec3,
    max_angle: f64,
) -> Result<TriangleSet> {
    check_angle("max_angle", max_angle)?;
    let length = direction.length();
    if !direction.is_finite() || length <= 0.0 {
        return Err(Error::invalid(
            "direction",
            "the direction must be a finite, non-zero vector",
        ));
    }
    let unit = direction * (1.0 / length);
    let least_cosine = max_angle.to_radians().cos();
    Ok(triangle_normals(mesh)
        .into_iter()
        .enumerate()
        .filter(|(_, n)| n.is_some_and(|n| n.dot(unit) >= least_cosine - 1e-12))
        .filter_map(|(t, _)| u32::try_from(t).ok())
        .collect())
}

/// Breadth-first fill from `seeds`, stepping from triangle `from` to its
/// neighbour `to` when `allow(from, to)`.
fn fill(
    mesh: &TriangleMesh,
    edges: &EdgeMap,
    seeds: &[u32],
    allow: impl Fn(u32, u32) -> bool,
) -> TriangleSet {
    let mut selected = TriangleSet::new();
    let mut queue: Vec<u32> = Vec::new();
    for &seed in seeds {
        if selected.insert(seed) {
            queue.push(seed);
        }
    }
    let mut next = 0;
    while let Some(&from) = queue.get(next) {
        next += 1;
        for edge in edges.sides(from as usize).into_iter().flatten() {
            for used in edges.uses(edge as usize) {
                let to = used.triangle;
                if (to as usize) < mesh.triangles.len()
                    && !selected.contains(&to)
                    && allow(from, to)
                {
                    selected.insert(to);
                    queue.push(to);
                }
            }
        }
    }
    selected
}
