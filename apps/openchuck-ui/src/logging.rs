// SPDX-License-Identifier: AGPL-3.0-or-later
//! The local log file (issue 155; decision record 0155; requirement NFR-17).
//!
//! On Windows a release build has no console window, so everything the
//! program says goes to a file as well as to standard error. The file is
//! `openchuckcam.log` in `OPENCHUCK_LOG_DIR` when that is set, else in the
//! folder of the executable; when that folder cannot be written (an installed
//! build under `Program Files`) it is in the per-user log folder instead, then
//! in the system temp folder, and when none works nothing is written and the
//! program runs on (NFR-07). The file never leaves the computer (NFR-09).
//!
//! At start a log over [`MAX_BYTES`] becomes `openchuckcam.1.log`, and the
//! older ones move up, keeping [`KEEP`] old files. Panics are written too,
//! because without a console nobody would see them.
//!
//! [`line`] is the one place a record becomes text. The user's actions
//! (requirement NFR-18; decision record 0157) are written through it by
//! [`action`], to the file only, when the user's switch is on.

use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use openchuck_app::library::Platform;

/// The size above which a log file is rotated at start.
pub const MAX_BYTES: u64 = 1024 * 1024;

/// How many rotated files are kept beside the current one.
pub const KEEP: usize = 3;

const STEM: &str = "openchuckcam";

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// The open log file and when logging started, for [`to_file`] and
/// [`action`], which write outside the `log` facade.
static SINK: OnceLock<(Arc<Sink>, Instant)> = OnceLock::new();

/// Whether the user's actions are recorded (NFR-18): on until the settings
/// say otherwise.
static RECORDING: AtomicBool = AtomicBool::new(true);

/// Held by the tests that change the switch, which is process-wide.
#[cfg(test)]
pub(crate) static SWITCH_LOCK: Mutex<()> = Mutex::new(());

/// Turns the recording of the user's actions on or off (NFR-18).
pub fn set_recording(on: bool) {
    RECORDING.store(on, Ordering::Relaxed);
}

/// Whether the user's actions are recorded.
pub fn recording() -> bool {
    RECORDING.load(Ordering::Relaxed)
}

/// The text of one action line: the same layout as every record, level
/// `INFO`, target `action`.
pub fn action_line(at: Duration, text: &str) -> String {
    line(at, log::Level::Info, "action", text)
}

/// Records one user action in the log file, when the switch is on and a log
/// file is open. The text names the action and the kind and name of the
/// item acted on, and parameter names: never values or file contents.
pub fn action(text: &str) {
    if !recording() {
        return;
    }
    if let Some((sink, started)) = SINK.get() {
        sink.write_line(&action_line(started.elapsed(), text));
    }
}

/// Writes one record to the log file only, not to standard error. For the
/// messages `main` prints itself with `eprintln!`, so that a terminal shows
/// each once (OI-496).
pub fn to_file(level: log::Level, message: &str) {
    if let Some((sink, started)) = SINK.get() {
        sink.write_line(&line(started.elapsed(), level, "openchuck_ui", message));
    }
}

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

/// The environment variable that names the log folder, tried first.
pub const FOLDER_VARIABLE: &str = "OPENCHUCK_LOG_DIR";

/// The per-user log folder on `platform`, reading variables with `var`:
/// `%LOCALAPPDATA%\OpenChuckCAM\logs`, `~/Library/Logs/OpenChuckCAM` or
/// `$XDG_STATE_HOME/openchuckcam` (`~/.local/state/openchuckcam`). `None`
/// when the variables it needs are unset, empty or relative.
pub fn user_folder(
    platform: Platform,
    var: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let absolute = |name: &str| var(name).map(PathBuf::from).filter(|p| p.is_absolute());
    match platform {
        Platform::Windows => absolute("LOCALAPPDATA").map(|p| p.join("OpenChuckCAM").join("logs")),
        Platform::MacOs => absolute("HOME").map(|h| h.join("Library/Logs/OpenChuckCAM")),
        Platform::Unix => absolute("XDG_STATE_HOME")
            .or_else(|| absolute("HOME").map(|h| h.join(".local/state")))
            .map(|p| p.join("openchuckcam")),
    }
}

/// The folders to try in order: the override, beside the executable, the
/// per-user folder, and last the private folder in the system temp folder
/// (see [`private_temp_folder`]).
pub fn candidate_folders(
    over: Option<PathBuf>,
    exe: Option<&Path>,
    user: Option<PathBuf>,
    temp: Option<PathBuf>,
) -> Vec<PathBuf> {
    over.into_iter()
        .chain(exe.and_then(Path::parent).map(Path::to_path_buf))
        .chain(user)
        .chain(temp)
        .collect()
}

/// The name of the log folder in the system temp folder: one per user, so
/// that on a shared machine another user's folder is never reused (OI-497).
pub fn temp_folder_name(user: &str) -> String {
    let safe: String = user
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    if safe.is_empty() || safe.chars().all(|c| c == '.') {
        format!("{STEM}-user")
    } else {
        format!("{STEM}-{safe}")
    }
}

/// Whether `meta` describes a folder that belongs to the current user and
/// that nobody else can change: a real directory (not a symbolic link), owned
/// by this user, with no group or other permission at all (Unix: a log made
/// in it is readable by others otherwise). On Windows
/// the temp folder is already inside the user's profile, so a real directory
/// is enough.
#[cfg(unix)]
fn is_private_folder(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    meta.is_dir() && Some(meta.uid()) == current_uid() && meta.mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn is_private_folder(meta: &fs::Metadata) -> bool {
    meta.is_dir()
}

/// The numeric ID of the current user, read from the owner of `/proc/self`
/// (Linux) or of the home folder; `None` when neither is known, in which case
/// the temp folder is not used.
#[cfg(unix)]
fn current_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;
    fs::metadata("/proc/self")
        .or_else(|_| fs::metadata(std::env::var_os("HOME").unwrap_or_default()))
        .ok()
        .map(|m| m.uid())
}

/// Creates `base/name` for the log, private to the current user (mode 0700 on
/// Unix), or accepts one that already is. `None` when the name exists and is
/// not a real directory owned by this user, or is a symbolic link, or cannot
/// be created: the last-resort folder must not be one another user prepared
/// (OI-497).
pub fn private_folder(base: &Path, name: &str) -> Option<PathBuf> {
    let path = base.join(name);
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt as _;
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder
    };
    #[cfg(not(unix))]
    let builder = fs::DirBuilder::new();
    match builder.create(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // `symlink_metadata` does not follow a link, so a link is not a
            // directory here.
            let meta = fs::symlink_metadata(&path).ok()?;
            if !is_private_folder(&meta) {
                return None;
            }
        }
        Err(_) => return None,
    }
    Some(path)
}

/// The last-resort log folder in `temp`, named for `user`.
pub fn private_temp_folder(temp: &Path, user: &str) -> Option<PathBuf> {
    private_folder(temp, &temp_folder_name(user))
}

fn user_name() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default()
}

fn default_folders() -> Vec<PathBuf> {
    let over = std::env::var_os(FOLDER_VARIABLE)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    candidate_folders(
        over,
        std::env::current_exe().ok().as_deref(),
        user_folder(Platform::current(), |name| std::env::var_os(name)),
        private_temp_folder(&std::env::temp_dir(), &user_name()),
    )
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

/// Opens the log file in the default places and remembers its path and the
/// start time.
pub fn open() -> Option<Arc<Sink>> {
    let (file, path) = open_in(&default_folders(), MAX_BYTES, KEEP)?;
    let _ = PATH.set(path);
    let sink = Arc::new(Sink::new(file));
    let _ = SINK.set((Arc::clone(&sink), Instant::now()));
    Some(sink)
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

/// Makes a panic reach the log file, and still standard error through the
/// hook that was there (so a terminal shows it once, even with `RUST_LOG=off`).
/// The message is the panic's own text; no file contents.
pub fn install_panic_hook() {
    install_panic_hook_to(|text| to_file(log::Level::Error, text));
}

/// [`install_panic_hook`], writing the panic text with `write`.
pub fn install_panic_hook_to(write: impl Fn(&str) + Send + Sync + 'static) {
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
        write(&text);
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
    fn req_nfr_17_file_names() {
        let dir = Path::new("d");
        assert_eq!(file_name(dir, 0), dir.join("openchuckcam.log"));
        assert_eq!(file_name(dir, 2), dir.join("openchuckcam.2.log"));
    }

    #[test]
    fn req_nfr_17_rotation_keeps_the_newest_files() {
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
    fn req_nfr_17_small_file_is_appended_not_rotated() {
        let dir = scratch("small");
        fs::write(file_name(&dir, 0), b"short").unwrap();
        rotate(&dir, 10, 2);
        assert_eq!(fs::read(file_name(&dir, 0)).unwrap(), b"short");
        assert!(!file_name(&dir, 1).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_17_keep_zero_deletes_the_full_file() {
        let dir = scratch("keep0");
        fs::write(file_name(&dir, 0), vec![0u8; 20]).unwrap();
        rotate(&dir, 10, 0);
        assert!(!file_name(&dir, 0).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_17_falls_back_when_the_first_folder_is_unwritable() {
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
    fn req_nfr_17_no_writable_folder_gives_nothing() {
        let dir = scratch("none");
        let blocked = dir.join("blocked");
        fs::write(&blocked, b"x").unwrap();
        assert!(open_in(&[blocked.join("a")], 100, 2).is_none());
        assert!(open_in(&[], 100, 2).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_17_folders_are_tried_in_order() {
        let folders = candidate_folders(
            Some(PathBuf::from("/o")),
            Some(Path::new("/opt/oc/openchuck-ui")),
            Some(PathBuf::from("/home/u/logs")),
            Some(PathBuf::from("/tmp/openchuckcam-u")),
        );
        assert_eq!(
            folders,
            ["/o", "/opt/oc", "/home/u/logs", "/tmp/openchuckcam-u"].map(PathBuf::from)
        );
        let bare = candidate_folders(None, None, None, None);
        assert!(bare.is_empty());
    }

    #[test]
    fn req_nfr_17_temp_folder_is_named_for_the_user() {
        assert_eq!(temp_folder_name("alice"), "openchuckcam-alice");
        // Path separators and dots alone cannot make a path out of the name.
        assert_eq!(temp_folder_name("a/b\\c"), "openchuckcam-abc");
        assert_eq!(temp_folder_name(".."), "openchuckcam-user");
        assert_eq!(temp_folder_name(""), "openchuckcam-user");
    }

    #[test]
    fn req_nfr_17_private_folder_is_created_and_reused() {
        let base = scratch("private");
        let made = private_folder(&base, "openchuckcam-t").unwrap();
        assert!(made.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&made).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        // A second run finds its own folder again.
        assert_eq!(private_folder(&base, "openchuckcam-t"), Some(made));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn req_nfr_17_private_folder_refuses_a_file_in_its_place() {
        let base = scratch("private-file");
        fs::write(base.join("openchuckcam-t"), b"x").unwrap();
        assert_eq!(private_folder(&base, "openchuckcam-t"), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn req_nfr_17_private_folder_cannot_be_created_under_a_file() {
        let base = scratch("private-blocked");
        fs::write(base.join("f"), b"x").unwrap();
        assert_eq!(private_folder(&base.join("f"), "openchuckcam-t"), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn req_nfr_17_private_folder_refuses_a_symbolic_link() {
        let base = scratch("private-link");
        let elsewhere = base.join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, base.join("openchuckcam-t")).unwrap();
        assert_eq!(private_folder(&base, "openchuckcam-t"), None);
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn req_nfr_17_private_folder_refuses_one_others_can_use() {
        use std::os::unix::fs::PermissionsExt as _;
        let base = scratch("private-open");
        // Writable by others, and readable by them (a log made in it would
        // be too under the usual umask): both are refused.
        for mode in [0o777, 0o755, 0o750, 0o705] {
            let open = base.join(format!("openchuckcam-{mode:o}"));
            fs::create_dir(&open).unwrap();
            fs::set_permissions(&open, fs::Permissions::from_mode(mode)).unwrap();
            assert_eq!(
                private_folder(&base, &format!("openchuckcam-{mode:o}")),
                None
            );
        }
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn req_nfr_17_user_folder_per_platform() {
        // An absolute path on every host: "/home/u" is relative on Windows.
        let root = std::env::temp_dir();
        let env = |pairs: Vec<(&'static str, PathBuf)>| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| v.clone().into_os_string())
            }
        };
        assert_eq!(
            user_folder(Platform::Windows, env(vec![("LOCALAPPDATA", root.clone())])),
            Some(root.join("OpenChuckCAM").join("logs"))
        );
        assert_eq!(
            user_folder(Platform::MacOs, env(vec![("HOME", root.clone())])),
            Some(root.join("Library").join("Logs").join("OpenChuckCAM"))
        );
        assert_eq!(
            user_folder(Platform::Unix, env(vec![("HOME", root.clone())])),
            Some(root.join(".local").join("state").join("openchuckcam"))
        );
        let state = root.join("state");
        assert_eq!(
            user_folder(
                Platform::Unix,
                env(vec![
                    ("HOME", root.clone()),
                    ("XDG_STATE_HOME", state.clone())
                ])
            ),
            Some(state.join("openchuckcam"))
        );
        // Unset or relative variables give nothing.
        assert_eq!(user_folder(Platform::Windows, env(vec![])), None);
        assert_eq!(
            user_folder(Platform::Unix, env(vec![("HOME", PathBuf::from("rel"))])),
            None
        );
    }

    #[test]
    fn req_nfr_17_sink_writes_lines_to_the_file() {
        let dir = scratch("sink");
        let (file, path) = open_in(std::slice::from_ref(&dir), 100, 2).unwrap();
        let sink = Sink::new(file);
        sink.write_line("one");
        sink.write_line("two");
        assert_eq!(fs::read_to_string(path).unwrap(), "one\ntwo\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_17_line_format() {
        let text = line(
            Duration::from_millis(1234),
            log::Level::Warn,
            "openchuck_ui",
            "hi",
        );
        assert_eq!(text, "[   1.234 WARN  openchuck_ui] hi");
    }

    #[test]
    fn req_nfr_17_panic_text_names_message_place_and_thread() {
        let text = panic_text("boom", Some(("a.rs", 7)), "main", "trace");
        assert_eq!(text, "panic in thread 'main' at a.rs:7: boom\ntrace");
        assert!(panic_text("x", None, "t", "").starts_with("panic in thread 't': x"));
    }

    #[test]
    fn req_nfr_17_panic_hook_writes_to_the_file() {
        // The hook is process-wide, so this is the one test that installs it.
        let dir = scratch("hook");
        let (file, path) = open_in(std::slice::from_ref(&dir), 100, 2).unwrap();
        let sink = Sink::new(file);
        install_panic_hook_to(move |text| sink.write_line(text));
        let outcome = std::panic::catch_unwind(|| panic!("test panic text"));
        let _ = std::panic::take_hook();
        assert!(outcome.is_err());
        let written = fs::read_to_string(&path).unwrap();
        assert!(written.contains("test panic text"), "{written}");
        assert!(written.contains("logging.rs"), "{written}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn req_nfr_18_action_lines_use_the_log_layout() {
        let text = action_line(Duration::from_millis(2500), "command: Remove setup 'S'");
        assert_eq!(text, "[   2.500 INFO  action] command: Remove setup 'S'");
    }

    #[test]
    fn req_nfr_18_recording_is_on_by_default_and_can_be_turned_off() {
        // With no log file open (tests do not call `open`) nothing is written
        // either way.
        let _held = SWITCH_LOCK.lock().unwrap();
        assert!(recording());
        set_recording(false);
        assert!(!recording());
        action("ignored");
        to_file(log::Level::Warn, "ignored");
        set_recording(true);
        assert!(recording());
    }

    #[test]
    fn req_nfr_17_error_box_without_a_log_is_the_message() {
        // PATH is set only by `open`, which tests do not call.
        assert_eq!(with_log_hint("boom"), "boom");
    }

    #[test]
    fn req_nfr_17_folder_functions_agree_with_the_path() {
        // PATH is set only by `open`, which tests do not call.
        assert_eq!(log_path().is_some(), log_folder().is_some());
    }
}
