// SPDX-License-Identifier: AGPL-3.0-or-later
//! The local log file (issue 155; decision record 0155; requirement NFR-LOG,
//! proposed, not yet numbered).
//!
//! On Windows a release build has no console window, so everything the
//! program says goes to a file as well as to standard error. The file is
//! `openchuckcam.log` in the folder of the executable; when that folder
//! cannot be written (an installed build under `Program Files`) it is in the
//! per-user configuration folder instead, and when neither works nothing is
//! written and the program runs on (NFR-07). The file never leaves the
//! computer (NFR-09).
//!
//! At start a log over [`MAX_BYTES`] becomes `openchuckcam.1.log`, and the
//! older ones move up, keeping [`KEEP`] old files. Panics are written too,
//! because without a console nobody would see them.
//!
//! [`line`] is the one place a record becomes text, so a later structured
//! event log (issue 156) can write through [`Sink::write_line`] without
//! changing the rotation or the folder choice.

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// The size above which a log file is rotated at start.
pub const MAX_BYTES: u64 = 1024 * 1024;

/// How many rotated files are kept beside the current one.
pub const KEEP: usize = 2;

const STEM: &str = "openchuckcam";

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// The log file in use, once [`open`] has found a writable place for it.
pub fn log_path() -> Option<PathBuf> {
    PATH.get().cloned()
}

/// The folder holding the log file in use.
pub fn log_folder() -> Option<PathBuf> {
    PATH.get().and_then(|p| p.parent()).map(Path::to_path_buf)
}

/// Shows the log folder in the system file browser. The About window calls
/// this from its button.
#[allow(dead_code)] // Called by the About window's button (lane BI) once it lands.
pub fn open_log_folder() -> Result<(), String> {
    let folder = log_folder().ok_or_else(|| "There is no log file in this run.".to_owned())?;
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(&folder)
        .spawn()
        .map(drop)
        .map_err(|e| format!("Could not open {}: {e}", folder.display()))
}

/// `message` with a line naming the log file, for an error box: started from
/// the desktop there is no terminal to read the same text in.
pub fn with_log_hint(message: &str) -> String {
    match log_path() {
        Some(path) => format!(
            "{message}\n\nMore detail is in the log file:\n{}",
            path.display()
        ),
        None => message.to_owned(),
    }
}

/// The path of the log file number `index` (0 is the current one) in `folder`.
pub fn file_name(folder: &Path, index: usize) -> PathBuf {
    if index == 0 {
        folder.join(format!("{STEM}.log"))
    } else {
        folder.join(format!("{STEM}.{index}.log"))
    }
}

/// Moves a log over `max_bytes` to the first rotated name, the rotated ones
/// up by one, and drops the oldest beyond `keep`. A failure leaves the files
/// as they are: the log is appended to instead.
pub fn rotate(folder: &Path, max_bytes: u64, keep: usize) {
    let current = file_name(folder, 0);
    if fs::metadata(&current).map_or(true, |m| m.len() <= max_bytes) {
        return;
    }
    let _ = fs::remove_file(file_name(folder, keep));
    for index in (1..keep).rev() {
        let _ = fs::rename(file_name(folder, index), file_name(folder, index + 1));
    }
    if keep == 0 {
        let _ = fs::remove_file(&current);
    } else {
        let _ = fs::rename(&current, file_name(folder, 1));
    }
}

/// Opens the log file in the first of `folders` that can be written,
/// rotating it first. `None` when none can.
pub fn open_in(folders: &[PathBuf], max_bytes: u64, keep: usize) -> Option<(File, PathBuf)> {
    folders.iter().find_map(|folder| {
        fs::create_dir_all(folder).ok()?;
        rotate(folder, max_bytes, keep);
        let path = file_name(folder, 0);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
        Some((file, path))
    })
}

/// The folders to try: beside the executable, then the per-user folder.
pub fn candidate_folders(exe: Option<&Path>, user: Option<PathBuf>) -> Vec<PathBuf> {
    exe.and_then(Path::parent)
        .map(Path::to_path_buf)
        .into_iter()
        .chain(user)
        .collect()
}

fn default_folders() -> Vec<PathBuf> {
    let user = openchuck_app::library::config_folder_from(
        openchuck_app::library::Platform::current(),
        |name| std::env::var_os(name),
    )
    .map(|base| base.join(openchuck_app::library::APP_FOLDER).join("logs"));
    candidate_folders(std::env::current_exe().ok().as_deref(), user)
}

/// Where log lines go besides standard error.
#[derive(Debug)]
pub struct Sink {
    file: Mutex<File>,
}

impl Sink {
    fn new(file: File) -> Self {
        Self {
            file: Mutex::new(file),
        }
    }

    /// Appends `text` and a newline, flushing so that a crash loses nothing.
    /// Errors are ignored: a full disk must not stop the program (NFR-07).
    pub fn write_line(&self, text: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{text}");
            let _ = file.flush();
        }
    }
}

/// Opens the log file in the default places and remembers its path.
pub fn open() -> Option<Sink> {
    let (file, path) = open_in(&default_folders(), MAX_BYTES, KEEP)?;
    let _ = PATH.set(path);
    Some(Sink::new(file))
}

/// One log line: seconds since start, level, target and message.
pub fn line(at: Duration, level: log::Level, target: &str, message: &str) -> String {
    format!(
        "[{:>4}.{:03} {:<5} {}] {}",
        at.as_secs(),
        at.subsec_millis(),
        level,
        target,
        message
    )
}

/// The text logged for a panic: message, place, thread and backtrace.
pub fn panic_text(
    message: &str,
    location: Option<(&str, u32)>,
    thread: &str,
    backtrace: &str,
) -> String {
    let place = location.map_or_else(String::new, |(file, line)| format!(" at {file}:{line}"));
    format!("panic in thread '{thread}'{place}: {message}\n{backtrace}")
}

/// Makes a panic reach the log (and still standard error, through the hook
/// that was there). The message is the panic's own text; no file contents.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(no message)");
        let thread = std::thread::current();
        let text = panic_text(
            message,
            info.location().map(|l| (l.file(), l.line())),
            thread.name().unwrap_or("unnamed"),
            &std::backtrace::Backtrace::force_capture().to_string(),
        );
        log::error!("{text}");
        log::logger().flush();
        previous(info);
    }));
}

/// Gives a Windows release build the terminal it was started from, so that
/// `--help`, `--bench` and error text still print there. A double click has
/// no parent console and the call fails, which is the wanted result. Handles
/// the caller redirected (a pipe, a file) are left as they are.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: AttachConsole takes a process id and has no pointer arguments.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// Other platforms keep the terminal they were started in.
#[cfg(not(windows))]
pub fn attach_parent_console() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openchuck-log-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn req_nfr_log_file_names() {
        let dir = Path::new("d");
        assert_eq!(file_name(dir, 0), dir.join("openchuckcam.log"));
        assert_eq!(file_name(dir, 2), dir.join("openchuckcam.2.log"));
    }

    #[test]
    fn req_nfr_log_rotation_keeps_the_newest_files() {
        let dir = scratch("rotate");
        for round in 0..4u8 {
            fs::write(file_name(&dir, 0), vec![b'a' + round; 20]).unwrap();
            rotate(&dir, 10, 2);
            assert!(!file_name(&dir, 0).exists(), "round {round}");
        }
        assert_eq!(fs::read(file_name(&dir, 1)).unwrap(), vec![b'd'; 20]);
        assert_eq!(fs::read(file_name(&dir, 2)).unwrap(), vec![b'c'; 20]);
        assert!(!file_name(&dir, 3).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_log_small_file_is_appended_not_rotated() {
        let dir = scratch("small");
        fs::write(file_name(&dir, 0), b"short").unwrap();
        rotate(&dir, 10, 2);
        assert_eq!(fs::read(file_name(&dir, 0)).unwrap(), b"short");
        assert!(!file_name(&dir, 1).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_log_keep_zero_deletes_the_full_file() {
        let dir = scratch("keep0");
        fs::write(file_name(&dir, 0), vec![0u8; 20]).unwrap();
        rotate(&dir, 10, 0);
        assert!(!file_name(&dir, 0).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_log_falls_back_when_the_first_folder_is_unwritable() {
        let dir = scratch("fallback");
        // A file where the first folder should be cannot be a folder.
        let blocked = dir.join("blocked");
        fs::write(&blocked, b"x").unwrap();
        let good = dir.join("user").join("logs");
        let (mut file, path) = open_in(&[blocked.join("sub"), good.clone()], 100, 2).unwrap();
        assert_eq!(path, good.join("openchuckcam.log"));
        writeln!(file, "hello").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_log_no_writable_folder_gives_nothing() {
        let dir = scratch("none");
        let blocked = dir.join("blocked");
        fs::write(&blocked, b"x").unwrap();
        assert!(open_in(&[blocked.join("a")], 100, 2).is_none());
        assert!(open_in(&[], 100, 2).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_log_prefers_the_executable_folder() {
        let folders = candidate_folders(
            Some(Path::new("/opt/oc/openchuck-ui")),
            Some(PathBuf::from("/home/u/logs")),
        );
        assert_eq!(
            folders,
            vec![PathBuf::from("/opt/oc"), PathBuf::from("/home/u/logs")]
        );
        assert_eq!(candidate_folders(None, None), Vec::<PathBuf>::new());
    }

    #[test]
    fn req_nfr_log_sink_writes_lines_to_the_file() {
        let dir = scratch("sink");
        let (file, path) = open_in(std::slice::from_ref(&dir), 100, 2).unwrap();
        let sink = Sink::new(file);
        sink.write_line("one");
        sink.write_line("two");
        assert_eq!(fs::read_to_string(path).unwrap(), "one\ntwo\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_log_line_format() {
        let text = line(
            Duration::from_millis(1234),
            log::Level::Warn,
            "openchuck_ui",
            "hi",
        );
        assert_eq!(text, "[   1.234 WARN  openchuck_ui] hi");
    }

    #[test]
    fn req_nfr_log_panic_text_names_message_place_and_thread() {
        let text = panic_text("boom", Some(("a.rs", 7)), "main", "trace");
        assert_eq!(text, "panic in thread 'main' at a.rs:7: boom\ntrace");
        assert!(panic_text("x", None, "t", "").starts_with("panic in thread 't': x"));
    }

    #[test]
    fn req_nfr_log_panic_hook_writes_to_the_file() {
        // The hook and the logger are process-wide, so this is the one test
        // that installs them; it logs through a private Logger to a file.
        struct ToFile(Sink);
        impl log::Log for ToFile {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool {
                true
            }
            fn log(&self, record: &log::Record<'_>) {
                self.0.write_line(&record.args().to_string());
            }
            fn flush(&self) {}
        }
        let dir = scratch("hook");
        let (file, path) = open_in(std::slice::from_ref(&dir), 100, 2).unwrap();
        if log::set_boxed_logger(Box::new(ToFile(Sink::new(file)))).is_err() {
            return; // another test in this process owns the logger
        }
        log::set_max_level(log::LevelFilter::Error);
        install_panic_hook();
        let outcome = std::panic::catch_unwind(|| panic!("test panic text"));
        let _ = std::panic::take_hook();
        assert!(outcome.is_err());
        let written = fs::read_to_string(&path).unwrap();
        assert!(written.contains("test panic text"), "{written}");
        assert!(written.contains("logging.rs"), "{written}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_log_error_box_without_a_log_is_the_message() {
        // PATH is set only by `open`, which tests do not call.
        assert_eq!(with_log_hint("boom"), "boom");
    }

    #[test]
    fn req_nfr_log_folder_functions_agree_with_the_path() {
        // PATH is set only by `open`, which tests do not call.
        assert_eq!(log_path().is_some(), log_folder().is_some());
    }
}
