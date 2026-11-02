// SPDX-License-Identifier: AGPL-3.0-or-later
//! The STEP reader process (work breakdown slice 11 item 1; decision record 0003).
//!
//! `openchuck-step-worker <input> <output> <linear mm> <angular rad> <unit override mm>`
//!
//! Reads the STEP file `input`, writes the JSON answer to `output`, and exits.
//! On a failure it writes `{"error": "..."}` and exits with code 2. A crash
//! inside the kernel writes nothing, which the parent reports as a damaged
//! file. [`openchuck_import::step::StepImporter`] is the caller; this program
//! has no other use.
//!
//! The process ends by returning from `main`, so the kernel's static
//! destructors run. That is safe because OpenCASCADE's type registry is built
//! before, and destroyed after, every other kernel static: on macOS and Windows
//! the build links `TKernel` first, on Linux the wrapper builds the registry in
//! a priority initialiser (decision 0003, exit crash; decision 0490). The tests
//! exit this way on every platform.

#![allow(clippy::print_stderr)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let [_, input, output, linear, angular, override_mm] = args.as_slice() else {
        eprintln!(
            "usage: openchuck-step-worker <input> <output> <linear mm> <angular rad> <unit override mm>"
        );
        return ExitCode::from(64);
    };
    let (Ok(linear), Ok(angular), Ok(override_mm)) = (
        linear.parse::<f64>(),
        angular.parse::<f64>(),
        override_mm.parse::<f64>(),
    ) else {
        eprintln!("the tolerances and the unit override must be numbers");
        return ExitCode::from(64);
    };
    match openchuck_import::kernel::read_step_json(input, linear, angular, override_mm) {
        Ok(json) => match std::fs::write(output, json) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("cannot write '{output}': {e}");
                ExitCode::from(1)
            }
        },
        Err(message) => {
            let json = serde_json::json!({ "error": message }).to_string();
            if std::fs::write(output, json).is_err() {
                return ExitCode::from(1);
            }
            ExitCode::from(2)
        }
    }
}
