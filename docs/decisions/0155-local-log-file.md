# 0155: A local log file for the desktop application (lane BUG-LOG; issue 155; OI-453 to OI-457)

- **Status:** proposed. Not accepted; only the project owner accepts a record. The choices below follow the owner's answers relayed by the coordinator session (location and fall-backs, rotation, verbosity left as is, the About button, CLI unchanged, requirement NFR-17).
- **Date:** 19 Oct 2026.
- **Open items:** OI-453 (requirement, closed: NFR-17), OI-454 (About button), OI-455 (Windows checks), OI-456 (closed), OI-457 (issue 156 seam), OI-458 and OI-459 (a log-level flag and a Settings option, not built).

Numbering: 0155 and 0156 were assigned to lane BUG-LOG by the coordinator session.

## Context

Trial finding F-14 (issue 155): on Windows `openchuck-ui` opens a console window behind the application. The owner wants no console window and the text it prints kept in a log file instead. Decision [0156](0156-windows-gui-subsystem-and-console-attach.md) removes the window; this record is what replaces it.

What the program emits today: a `log` facade logger on standard error under `RUST_LOG` ([decision 0059](0059-graphics-backend-on-windows.md): the program's own `info` lines and every crate's warnings), a few `eprintln!` calls (start-up errors, the 30 s stall warning, the benchmark's errors), and native message boxes. Panics reach only the panic hook's default output, which is invisible without a console.

**Requirement.** None existed: UI-01, NFR-07, NFR-09 and NFR-12 do not say that no console window opens or that a log is kept. The owner agreed the wording and the ID: **NFR-17** (P1), added to `docs/requirements.md` and the verification plan in this change (OI-453, closed). Tests are named `req_nfr_17_*`.

## Decision

1. **The log is a second sink of the existing logger.** The logger in `graphics.rs` keeps writing to standard error and also appends each line to the file. No new logging crate and no new level system (owner: verbosity is left as is): `RUST_LOG` and the defaults of decision 0059 are unchanged. A `--log-level` flag and a Settings option were considered and are not built (OI-458, OI-459). `tracing` (crate selection) is still the choice for the job manager's spans; this needs only the `log` facade already in `openchuck-ui`. Libraries log nothing (architecture section 7).
2. **Location (owner's answers).** The first of these that can be created and written, in order: `OPENCHUCK_LOG_DIR` when set to an absolute path; the folder of the executable; the per-user log folder (`%LOCALAPPDATA%\OpenChuckCAM\logs` on Windows, `~/Library/Logs/OpenChuckCAM` on macOS, `$XDG_STATE_HOME/openchuckcam`, default `~/.local/state/openchuckcam`, on Linux); `openchuckcam` in the system temp folder. If none can be written nothing is logged and the program runs on (NFR-07). The file is `openchuckcam.log`, appended, one line per record in the format standard error uses.
3. **Rotation (owner's answer).** A log over 1 MiB at start is renamed `openchuckcam.1.log` and the older ones move up; 3 old files are kept (about 4 MiB at most). The limits are constants (`MAX_BYTES`, `KEEP`). Size is checked at start only, so no thread or lock is added to the hot path. `logging::log_path()` returns the file actually in use, so the About window's button finds it whichever folder was used. A build under `Program Files` thus logs to the per-user folder.
4. **Panics are logged.** A panic hook writes the message, the thread, the place and a backtrace at `error` level, then calls the previous hook. It records the panic's own text only: no project contents, no file contents. Project paths and error messages are logged as before; nothing is logged that is not already printed.
5. **Finding the log (owner's answer): a button in the About window, and no menu item.** Lane BI (pull request 150) owns that window and is not edited here; this lane adds the button after pull request 150 has merged, or lane BI does if it merges later. `logging::log_path() -> Option<PathBuf>` and `logging::open_log_folder() -> Result<(), String>` are the calls it makes (OI-454). The error boxes this lane owns (start-up failure) end with the log path (`with_log_hint`).
6. **Room for issue 156.** `logging::line` is the one place a record becomes text and `Sink::write_line` the one place a line reaches the file, so a structured user-action log (issue 156, P2) can write through them without changing the folder choice or the rotation. Nothing of it is built (OI-457).
7. **The command-line tool is unchanged (owner's answer)**: it keeps printing to the console and writes no log.
8. **Local only.** The file is written to the computer and never sent anywhere; no network client or telemetry is added (NFR-09, AGENTS.md).

## Alternatives considered

- **Per-user folder only.** Works when installed, but the owner chose the executable's folder for portable use; kept as the first fallback.
- **`tracing` with a rolling file appender.** Adds `tracing`, `tracing-subscriber` and `tracing-appender` for what is a dozen lines of file handling, and a background writer thread that can lose the last lines in a crash. The file here is flushed per line.
- **Rotation by a background thread, or by size on every write.** Needless for sessions of a few KB; a start-time check is enough and cannot fail a write.

## Consequences

- Tests (Linux), named `req_nfr_17_*`: file names, rotation (newest files kept, small files appended, keep 0), the folder order and the per-platform user folders, fall-back when the first folder is unwritable, no writable folder, line format, panic text, the panic hook writing to a file through the logger, and the error box text without a log.
- Windows behaviour is checked by the owner (OI-455). Coverage of `logging.rs` on Linux: everything except `open` (reads the real executable path), `open_log_folder` (starts a process) and the `cfg(windows)` console call.
- `docs/dependencies.md`, the work breakdown (lane BUG-LOG) and the open-items register are updated in the same pull request.
