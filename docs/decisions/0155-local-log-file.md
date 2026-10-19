# 0155: A local log file for the desktop application (lane BUG-LOG; issue 155; OI-453 to OI-457)

- **Status:** proposed. Not accepted; only the project owner accepts a record. Several choices below follow the owner's answers relayed by the coordinator session (location, verbosity, the About button, CLI unchanged); the rotation and the read-only fallback were not answered and are built as proposed (OI-456).
- **Date:** 19 Oct 2026.
- **Open items:** OI-453 (requirement wording), OI-454 (About button), OI-455 (Windows checks), OI-456 (rotation, fallback), OI-457 (issue 156 seam).

Numbering: 0155 and 0156 were assigned to lane BUG-LOG by the coordinator session.

## Context

Trial finding F-14 (issue 155): on Windows `openchuck-ui` opens a console window behind the application. The owner wants no console window and the text it prints kept in a log file instead. Decision [0156](0156-windows-gui-subsystem-and-console-attach.md) removes the window; this record is what replaces it.

What the program emits today: a `log` facade logger on standard error under `RUST_LOG` ([decision 0059](0059-graphics-backend-on-windows.md): the program's own `info` lines and every crate's warnings), a few `eprintln!` calls (start-up errors, the 30 s stall warning, the benchmark's errors), and native message boxes. Panics reach only the panic hook's default output, which is invisible without a console.

**Requirement.** None exists: UI-01 (cross-platform application), NFR-07 (clear error, never a crash), NFR-09 (offline, no telemetry) and NFR-12 do not say that no console window opens or that a log is kept. The wording is proposed in OI-453 and tests are named `req_nfr_log_*` until the owner agrees an ID (NFR-16 is lane BI's).

## Decision

1. **The log is a second sink of the existing logger.** The logger in `graphics.rs` keeps writing to standard error and also appends each line to the file. No new logging crate and no new level system (owner's answer): `RUST_LOG` and the defaults of decision 0059 are unchanged. `tracing` (crate selection) is still the choice for the job manager's spans; this needs only the `log` facade already in `openchuck-ui`. Libraries log nothing (architecture section 7).
2. **Location (owner's answer): the folder of the executable**, file `openchuckcam.log`, appended, one line per record in the format standard error uses. A log over 1 MiB at start is renamed `openchuckcam.1.log` and the older ones move up; 2 old files are kept (about 3 MiB at most). Size is checked at start only, so no thread or lock is added to the hot path (OI-456).
3. **Read-only install folder.** If the executable's folder cannot be created or written (an installed build under `Program Files`), the file goes to the per-user configuration folder the settings and tool libraries already use, in `OpenChuckCAM/logs` (`%APPDATA%\OpenChuckCAM\logs` on Windows). If that also fails, nothing is written and the program runs on; a log must never stop it (NFR-07). `logging::log_path()` returns the file actually in use, so the About window's button finds it either way. The alternative (log to nothing when the install folder is read-only) was rejected: the installed build is where the feature is needed.
4. **Panics are logged.** A panic hook writes the message, the thread, the place and a backtrace at `error` level, then calls the previous hook. It records the panic's own text only: no project contents, no file contents. Project paths and error messages are logged as before; nothing is logged that is not already printed.
5. **Finding the log (owner's answer): a button in the About window.** Lane BI (pull request 150) owns that window and is not edited here. `logging::log_path() -> Option<PathBuf>` and `logging::open_log_folder() -> Result<(), String>` are the calls it makes (OI-454). The error boxes this lane owns (start-up failure) end with the log path (`with_log_hint`).
6. **Room for issue 156.** `logging::line` is the one place a record becomes text and `Sink::write_line` the one place a line reaches the file, so a structured user-action log (issue 156, P2) can write through them without changing the folder choice or the rotation. Nothing of it is built (OI-457).
7. **The command-line tool is unchanged (owner's answer)**: it keeps printing to the console and writes no log.
8. **Local only.** The file is written to the computer and never sent anywhere; no network client or telemetry is added (NFR-09, AGENTS.md).

## Alternatives considered

- **Per-user folder only.** Works when installed, but the owner chose the executable's folder for portable use; kept as the fallback.
- **`tracing` with a rolling file appender.** Adds `tracing`, `tracing-subscriber` and `tracing-appender` for what is a dozen lines of file handling, and a background writer thread that can lose the last lines in a crash. The file here is flushed per line.
- **Rotation by a background thread, or by size on every write.** Needless for sessions of a few KB; a start-time check is enough and cannot fail a write.

## Consequences

- Tests (Linux): file names, rotation (newest files kept, small files appended, keep 0), fall-back when the first folder is unwritable, no writable folder, the executable's folder tried first, line format, panic text, the panic hook writing to a file through the logger, and the error box text without a log. They are named `req_nfr_log_*`; the trace report does not list them until the ID exists.
- Windows behaviour is checked by the owner (OI-455). Coverage of `logging.rs` on Linux: everything except `open` (reads the real executable path), `open_log_folder` (starts a process) and the `cfg(windows)` console call.
- `docs/dependencies.md`, the work breakdown (lane BUG-LOG) and the open-items register are updated in the same pull request.
