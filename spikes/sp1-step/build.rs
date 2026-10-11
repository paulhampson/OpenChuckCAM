// Spike only: compile the shim against the OCCT that opencascade-sys / occt-sys built.
//
// Cargo gives no ordering between this build script and opencascade-sys's, so on a clean machine the
// OCCT headers may not exist yet when this runs. The shim is compiled only when they exist, and the
// build is run twice in CI.
//
// Why the second pass previously did nothing: `rerun-if-changed` on the header compared its mtime
// with this script's last run, and CMake's install keeps the header's original (old) source mtime, so
// Cargo saw nothing newer and skipped this script. Watching the installed TKernel library instead
// fixes it: that file is produced by the OCCT build itself, so its mtime is newer than the first run.
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let occt = out.join("../../../../OCCT");
    let probe = occt.join("include/BRepAdaptor_Surface.hxx");
    let lib = if std::env::var("TARGET").unwrap().contains("windows") { "lib/TKernel.lib" } else { "lib/libTKernel.a" };
    println!("cargo:rerun-if-changed={}", probe.display());
    println!("cargo:rerun-if-changed={}", occt.join(lib).display());
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
