// SPDX-License-Identifier: AGPL-3.0-only
// SP-1 throwaway: the same questions put to cadrum (MIT; OCCT 8 prebuilt, statically linked).
use cadrum::{Solid, Tessellation};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let mut f = match std::fs::File::open(&path) { Ok(f) => f, Err(e) => { println!("ERR io {e}"); std::process::exit(2) } };
    let solids = match Solid::read_step(&mut f) { Ok(s) => s, Err(e) => { println!("ERR {e:?}"); std::process::exit(2) } };
    println!("solids {}", solids.len());
    for s in &solids {
        let faces: Vec<_> = s.iter_face().collect();
        let mut line = String::new();
        for (i, fc) in faces.iter().enumerate().take(40) {
            line += &format!("[{i} id={} {:?} area={:.4}] ", fc.id(), fc.surface().map(|k| format!("{:?}", k.kind)), fc.area());
        }
        println!("faces {} {}", faces.len(), line);
    }
    for (lin, ang) in [(0.1, 0.5), (0.01, 0.5), (0.01, 0.1)] {
        let o = Tessellation { deflection_linear: lin, deflection_angular: ang, relative_linear: false };
        match Solid::mesh(&solids, o) {
            Ok(m) => println!("tess lin={lin} ang={ang} verts={} tris={} face_ids={}", m.vertices.len(), m.indices.len() / 3, m.face_ids.len()),
            Err(e) => println!("tess ERR {e:?}"),
        }
    }
}
