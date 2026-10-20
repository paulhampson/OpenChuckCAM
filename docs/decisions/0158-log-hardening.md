# 0158: Log messages once on the terminal, and a private temp log folder (lane UAL; OI-496; OI-497)

- **Status:** proposed. Minor findings of CodeRabbit on pull request 160, assigned to lane UAL by the owner; this record awaits the owner's acceptance. It amends [decision 0155](0155-local-log-file.md) items 2 and 4.
- **Date:** 20 Oct 2026.
- **Open items:** OI-496 and OI-497, both closed by this change.

## Context

Decision 0155 sends every `log` record to standard error and to the file. `main` also prints some messages itself with `eprintln!` and then logged them through the facade, so a terminal showed them twice (OI-496). Decision 0155 item 2 names `openchuckcam` in the system temp folder as the last fall-back; on a shared machine another user can create that folder first, or make it a link to somewhere they control (OI-497).

## Decision

1. **A message `main` prints is written to the file only.** `logging::to_file(level, message)` writes one record through the same line layout, straight to the file. It follows each `eprintln!` in `main.rs` (and the start-up stall warning in `graphics.rs`), which are the messages a terminal user already sees. The panic hook writes the panic text with `to_file` and calls the previous hook, which prints it to standard error, so a panic still shows in a terminal when `RUST_LOG=off`. The `log` facade's records are unchanged. The bench errors, which had no file line, get one.
2. **The temp fall-back is per user and private.** The folder is `openchuckcam-<user>` (the user name from `USER`, `USERNAME` or `LOGNAME`, reduced to letters, digits, `-`, `_` and `.`; `openchuckcam-user` when none). It is created with `create_dir` (mode 0700 on Unix). If it exists it is used only when it is a real directory (not a link), owned by the current user, and not writable by group or others (Unix; the owner is read from `/proc/self`, else the home folder, and the folder is refused when the owner cannot be found); otherwise the fall-back is skipped and, with none other writable, nothing is logged (NFR-07). It stays the last fall-back, after `OPENCHUCK_LOG_DIR`, the executable's folder and the per-user folder. On Windows the temp folder is inside the user's profile, so only the real-directory check applies.

## Consequences

- Tests `req_nfr_17_*`: the folder name, creation with mode 0700, reuse, refusal of a file, a link and a folder others can write, and that a folder cannot be made under a file. Not testable here: an existing folder owned by another user (the container runs as root).
- A previous `openchuckcam` folder in the temp folder is no longer used; a log there is left in place.
