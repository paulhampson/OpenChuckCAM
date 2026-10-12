// SPDX-License-Identifier: AGPL-3.0-only
// SP-1 option B prototype build: OpenCASCADE from occt-sys (or OCCT_ROOT), plus our own C++ shim.
// No dependency on opencascade-sys, so its bridge headers (which do not compile under MSVC) are not used.
use std::path::PathBuf;

// Toolkits needed for STEP read, topology, adaptors, properties and meshing, highest level first.
const LIBS: &[&str] = &[
    "TKDESTEP", "TKDE", "TKXCAF", "TKVCAF", "TKCAF", "TKLCAF", "TKCDF", "TKXSBase", "TKMesh",
    "TKShHealing", "TKBool", "TKBO", "TKFillet", "TKOffset", "TKPrim", "TKFeat", "TKTopAlgo",
    "TKGeomAlgo", "TKBRep", "TKGeomBase", "TKG3d", "TKG2d", "TKMath", "TKernel",
];

fn main() {
    println!("cargo:rerun-if-changed=cpp/shim.cpp");
    println!("cargo:rerun-if-env-changed=OCCT_ROOT");
    println!("cargo:rerun-if-env-changed=SP1_LIB_ORDER");
    let root = match std::env::var_os("OCCT_ROOT") {
        Some(r) => PathBuf::from(r),
        None => {
            // GitHub's Windows and macOS runners ship CMake 4, which rejects OCCT 7.8.1's
            // cmake_minimum_required. Set the documented escape hatch here so the build is self-contained.
            if std::env::var_os("CMAKE_POLICY_VERSION_MINIMUM").is_none() {
                std::env::set_var("CMAKE_POLICY_VERSION_MINIMUM", "3.5");
            }
            occt_sys::build_occt();
            occt_sys::occt_path()
        }
    };
    let target = std::env::var("TARGET").unwrap();
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include(root.join("include"))
        .define("_USE_MATH_DEFINES", None)
        .file("cpp/shim.cpp")
        .compile("sp9shim");
    println!("cargo:rustc-link-search=native={}", root.join("lib").display());
    if target.contains("linux") {
        // One group so link order inside OCCT does not matter.
        println!("cargo:rustc-link-arg=-Wl,--start-group");
        for l in LIBS {
            println!("cargo:rustc-link-arg=-l{l}");
        }
        println!("cargo:rustc-link-arg=-Wl,--end-group");
        println!("cargo:rustc-link-arg=-lstdc++");
        println!("cargo:rustc-link-arg=-lpthread");
        println!("cargo:rustc-link-arg=-ldl");
    } else {
        // Link order decides C++ static initialisation order with ld64 and MSVC. With the toolkits
        // first, macOS segfaulted at exit inside OCCT's Standard_Type registry (a static destroyed
        // before something that still uses it). TKernel first, as opencascade-sys does, keeps the
        // kernel's statics initialised first and destroyed last. SP1_LIB_ORDER=toolkit-first
        // restores the old order so CI can show the difference.
        let toolkit_first = std::env::var("SP1_LIB_ORDER").as_deref() == Ok("toolkit-first");
        let libs: Vec<&str> = if toolkit_first { LIBS.to_vec() } else { LIBS.iter().rev().copied().collect() };
        for l in libs {
            println!("cargo:rustc-link-lib=static={l}");
        }
    }
    if target.contains("windows") {
        for l in ["user32", "advapi32", "ws2_32"] {
            println!("cargo:rustc-link-lib=dylib={l}");
        }
    }
}
