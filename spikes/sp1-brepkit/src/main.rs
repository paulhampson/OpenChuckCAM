// SPDX-License-Identifier: AGPL-3.0-only
// SP-1 throwaway: same questions put to brepkit-io (pure Rust fallback candidate).
use brepkit_io::step::reader::read_step;
use brepkit_operations::tessellate::tessellate_solid_with_tolerance;
use brepkit_topology::Topology;
use brepkit_topology::explorer::solid_faces;
use std::collections::BTreeMap;

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let text = match std::fs::read(&path) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => { println!("ERR io {e}"); std::process::exit(2) }
    };
    let mut topo = Topology::new();
    let solids = match read_step(&text, &mut topo) {
        Ok(s) => s,
        Err(e) => { println!("ERR {e}"); std::process::exit(2) }
    };
    println!("solids {}", solids.len());
    for &sid in &solids {
        let faces = solid_faces(&topo, sid).unwrap();
        let mut mix: BTreeMap<&str, usize> = BTreeMap::new();
        for &f in &faces { *mix.entry(topo.face(f).unwrap().surface().type_tag()).or_default() += 1; }
        println!("faces {} {:?}", faces.len(), mix);
        for tol in [0.1, 0.01] {
            match tessellate_solid_with_tolerance(&topo, sid, tol, 0.5) {
                Ok(m) => println!("tess tol={tol} verts={} tris={}", m.positions.len(), m.indices.len() / 3),
                Err(e) => println!("tess tol={tol} ERR {e}"),
            }
        }
    }
}
