// Spike only: compile the shim against the OCCT that opencascade-sys / occt-sys built.
// Cargo gives no ordering between this build script and opencascade-sys's, so on a clean
// machine the OCCT headers may not exist yet (the first CI macOS run hit exactly that).
// The shim is therefore compiled only when the headers are present: build twice on a clean tree.
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let occt = out.join("../../../../OCCT");
    let probe = occt.join("include/BRepAdaptor_Surface.hxx");
    println!("cargo:rerun-if-changed={}", probe.display());
    println!("cargo:rerun-if-changed=shim/shim.cpp");
    println!("cargo:rustc-check-cfg=cfg(has_shim)");
    if probe.exists() {
        cc::Build::new()
            .cpp(true)
            .flag_if_supported("-std=c++14")
            .include(occt.join("include"))
            .file("shim/shim.cpp")
            .compile("sp1shim");
        println!("cargo:rustc-cfg=has_shim");
    } else {
        println!("cargo:warning=OCCT headers not found yet; shim skipped. Build again.");
    }
}
