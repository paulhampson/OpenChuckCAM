// SPDX-License-Identifier: AGPL-3.0-or-later
//! Embeds the application icon in the Windows program file (decision 0542). The compiled
//! resource `assets/icon/icon.res` is made by `cargo xtask icon` and committed; the MSVC
//! linker takes it as an input, so no resource compiler is needed.

fn main() {
    let res = "assets/icon/icon.res";
    println!("cargo:rerun-if-changed={res}");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" {
        return;
    }
    if env == "msvc" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        println!("cargo:rustc-link-arg-bins={dir}/{res}");
    } else {
        println!(
            "cargo:warning=the program file gets no embedded icon on this Windows target ({env}); only MSVC links the committed icon.res"
        );
    }
}
