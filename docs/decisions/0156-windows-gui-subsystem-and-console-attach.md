# 0156: `openchuck-ui` is a Windows GUI-subsystem release binary that attaches to the parent console (lane BUG-LOG; issue 155; OI-455)

- **Status:** proposed. Not accepted; only the project owner accepts a record. The coordinator relayed the owner's answer that the CLI keeps printing to the console; the plan for the UI binary below was sent to the coordinator and has not been answered (OI-455 lists what the owner checks).
- **Date:** 19 Oct 2026.
- **Open items:** OI-455.

## Context

`openchuck-ui.exe` is built for the Windows console subsystem, so Windows opens a console window behind the application when it is started from the desktop (trial finding F-14, issue 155). The program is also used from a terminal and from CI:

- The workflow "UI smoke test of the archives" (`ui-smoke.yml`, [decision 0101](0101-ui-smoke-of-the-built-archives.md)) runs the release `openchuck-ui.exe --bench MODEL --seconds 3 --out FILE` from bash on Windows, needs exit code 0 or 2, and shows the redirected output on failure.
- `--bench` ([verification plan](../verification-plan.md) 4.2) reports its result on the terminal and in the `--out` file; start-up errors go to standard error ([decision 0059](0059-graphics-backend-on-windows.md)).
- The packaging ([decision 0031](0031-slice-20-packaging-and-trial-documents.md)) and the nightly build ([decision 0023](0023-nightly-builds.md)) build the same release binary for the archives.

## Decision

1. **Release builds on Windows use the GUI subsystem**: `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]` in `main.rs`. No console window opens from the desktop or a shortcut. Debug builds, tests, `cargo run` and every other platform keep the console. The build and release workflows need no change: they build the release profile.
2. **At the start of `main` the program attaches to its parent's console** (`AttachConsole(ATTACH_PARENT_PROCESS)`). Started from cmd or PowerShell, `--bench`, error text and the warnings print there as before. A double click has no parent console and the call fails, which is the wanted result. A handle the caller redirected (a pipe or a file, as in the smoke workflow) is kept by the process, so redirected output and the exit code work as they do now; the owner confirms this on a nightly build (OI-455 item 6). A known limit of this approach: cmd does not wait for a GUI-subsystem program, so an interactive `openchuck-ui.exe --bench` returns the prompt before it finishes (bash, `start /wait` and CI wait); the report is in the `--out` file and the log.
3. **Everything also goes to the log file** ([decision 0155](0155-local-log-file.md)), so nothing is lost when no terminal exists.
4. **The call is one `unsafe` FFI call in `logging.rs`**, behind `#[allow(unsafe_code)]` on that function alone (the workspace denies `unsafe_code`; the exception is local, commented and Windows-only). It uses `windows-sys` 0.60.2, Windows-only target dependency, one feature (`Win32_System_Console`); the row is in `docs/dependencies.md` (NFR-14; MIT OR Apache-2.0, NFR-10).
5. **`openchuck-cli` is unchanged** (owner's answer): a console program that prints to the console.

## Alternatives considered

- **Keep a console only in `--bench` and `--help` modes**: needs a second binary or a re-launch, which breaks the exit code and the redirected output the smoke workflow reads. Rejected.
- **Hide the window at start (`FreeConsole`)**: the window flashes on every start. Rejected.
- **GUI subsystem without attaching**: the terminal would show nothing, and `--bench` output and errors would be lost to anyone starting it from a terminal. Rejected.
- **A hand-written `extern "system"` declaration instead of `windows-sys`**: no dependency, but the same `unsafe` and an ABI the project would own. `windows-sys` is already in the lock file.

## Consequences

- Not testable in the cloud: the Windows target does not build there (`ring` needs `ml64.exe`), so the `cfg(windows)` function was checked only against the `windows-sys` 0.60.2 source (`AttachConsole(u32) -> BOOL`, `ATTACH_PARENT_PROCESS`). The owner's checks are OI-455.
- No project format change; no change to the library family.
