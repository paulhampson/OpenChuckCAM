// Spike only: compile the shim against the OCCT that opencascade-sys / occt-sys built.
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let occt = out.join("../../../../OCCT");
    cc::Build::new()
        .cpp(true)
        .flag_if_supported("-std=c++14")
        .include(occt.join("include"))
        .file("shim/shim.cpp")
        .compile("sp1shim");
    println!("cargo:rerun-if-changed=shim/shim.cpp");
}
