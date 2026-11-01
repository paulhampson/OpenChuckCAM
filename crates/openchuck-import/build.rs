// SPDX-License-Identifier: AGPL-3.0-or-later
//! Builds the STEP kernel wrapper when the `kernel` feature is on (decision record 0003).
//!
//! OpenCASCADE 7.8.1 comes from `occt-sys` (build dependency only; its first
//! build takes 8 to 71 minutes, so CI caches it), or from an installation named
//! by `OCCT_ROOT`. `cpp/occ_step.cpp` is compiled with `cc`. Without the
//! feature this script does nothing, so the rest of the workspace needs no C++
//! toolchain.

#[cfg(feature = "kernel")]
fn main() {
    use std::path::PathBuf;

    // The toolkits needed to read STEP, walk topology, convert surfaces and
    // mesh, highest level first.
    const LIBS: &[&str] = &[
        "TKDESTEP",
        "TKDE",
        "TKXCAF",
        "TKVCAF",
        "TKCAF",
        "TKLCAF",
        "TKCDF",
        "TKXSBase",
        "TKMesh",
        "TKShHealing",
        "TKBool",
        "TKBO",
        "TKFillet",
        "TKOffset",
        "TKPrim",
        "TKFeat",
        "TKTopAlgo",
        "TKGeomAlgo",
        "TKBRep",
        "TKGeomBase",
        "TKG3d",
        "TKG2d",
        "TKMath",
        "TKernel",
    ];

    println!("cargo:rerun-if-changed=cpp/occ_step.cpp");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OCCT_ROOT");
    let root = match std::env::var_os("OCCT_ROOT") {
        Some(root) => PathBuf::from(root),
        None => {
            // CMake 4 (Windows and macOS runners) rejects OpenCASCADE 7.8.1's
            // cmake_minimum_required: set CMAKE_POLICY_VERSION_MINIMUM=3.5 in the
            // environment, as ci.yml does.
            occt_sys::build_occt();
            occt_sys::occt_path()
        }
    };
    let target = std::env::var("TARGET").unwrap_or_default();
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(root.join("include"))
        .define("_USE_MATH_DEFINES", None)
        .file("cpp/occ_step.cpp");
    if target.contains("msvc") {
        // The shim catches Standard_Failure, so exceptions must be on.
        build.flag("/EHsc");
    }
    build.compile("openchuck_occ_step");
    println!(
        "cargo:rustc-link-search=native={}",
        root.join("lib").display()
    );
    // `rustc-link-lib` (not `rustc-link-arg`) so crates that depend on this one with the
    // `kernel` feature (the kernel comparison tests in openchuck-app) link too.
    if target.contains("linux") {
        // GNU ld resolves a static archive's symbols from the archives listed after it, so the
        // dependency order (highest level first, TKernel last) is the right one here. It also puts
        // TKernel's static initialisers last, so the exit order is fixed in cpp/occ_step.cpp
        // instead: a priority initialiser builds the type registry first (decision 0490).
        for lib in LIBS {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-link-lib=dylib=dl");
    } else {
        // Link order decides the order of C++ static construction and destruction
        // with ld64 and MSVC. TKernel holds the type registry every toolkit's
        // statics use, so it must be linked FIRST: then it is constructed first and
        // destroyed last. The usual dependency order (TKernel last) crashed at exit
        // on macOS and Windows (decision 0003, "Exit crash and its cause").
        for lib in LIBS.iter().rev() {
            println!("cargo:rustc-link-lib=static={lib}");
        }
    }
    if target.contains("windows") {
        for lib in ["user32", "advapi32", "ws2_32"] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    }
}

#[cfg(not(feature = "kernel"))]
fn main() {}
