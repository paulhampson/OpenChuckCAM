// SPDX-License-Identifier: AGPL-3.0-or-later
//! Graphics start-up (decision record 0059, accepted): which wgpu backends the
//! window may use, a log on standard error that `RUST_LOG` controls, the
//! adapter chosen, and a clear message when start-up fails or stalls instead
//! of a window that never appears (NFR-02, UI-01, NFR-07).
//!
//! eframe's default lets wgpu choose among Vulkan, DX12 and OpenGL, and wgpu
//! takes Vulkan first when the adapters are of the same kind. On Windows that
//! put an Intel UHD 620 laptop on Intel's Vulkan driver, where the window
//! never showed and nothing was printed, while `WGPU_BACKEND=dx12` worked. On
//! Windows the window now uses DX12 alone when DX12 offers a hardware
//! adapter, and the old choice otherwise. `WGPU_BACKEND` still overrides.

use std::io::Write as _;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use eframe::egui_wgpu;
use openchuck_viewport::wgpu;

/// The backends eframe offers wgpu when nothing else applies.
pub const DEFAULT_BACKENDS: wgpu::Backends = wgpu::Backends::PRIMARY.union(wgpu::Backends::GL);

/// How long the window may take to draw its first frame before the user is
/// told why nothing has appeared.
pub const STALL_LIMIT: Duration = Duration::from_secs(30);

/// Why a set of backends was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// `WGPU_BACKEND` names them.
    Override,
    /// Windows, and DX12 offers a hardware adapter.
    Dx12Hardware,
    /// Anything else: eframe's own choice.
    Default,
}

/// The backends the window may use, and why. `dx12_hardware` is asked only on
/// Windows without an override, because it creates a DX12 instance.
pub fn choose(
    from_env: Option<wgpu::Backends>,
    windows: bool,
    dx12_hardware: impl FnOnce() -> bool,
) -> (wgpu::Backends, Reason) {
    match from_env {
        Some(backends) => (backends, Reason::Override),
        None if windows && dx12_hardware() => (wgpu::Backends::DX12, Reason::Dx12Hardware),
        None => (DEFAULT_BACKENDS, Reason::Default),
    }
}

/// Whether DX12 offers an adapter that is not a software renderer. Creates a
/// DX12-only instance, so no other driver is loaded. Always false where wgpu
/// is built without DX12.
fn dx12_hardware_adapter() -> bool {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
    });
    let adapters = ready(instance.enumerate_adapters(wgpu::Backends::DX12)).unwrap_or_default();
    for adapter in &adapters {
        log::info!("DX12 offers {}", describe(&adapter.get_info()));
    }
    adapters
        .iter()
        .any(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
}

/// The value of a future that is ready when first polled, as wgpu's native
/// futures are; `None` if it is not.
fn ready<F: Future>(future: F) -> Option<F::Output> {
    let future = std::pin::pin!(future);
    match future.poll(&mut std::task::Context::from_waker(std::task::Waker::noop())) {
        std::task::Poll::Ready(value) => Some(value),
        std::task::Poll::Pending => None,
    }
}

/// The wgpu setup for the window: the chosen backends with eframe's other
/// defaults (the high-performance adapter, its device limits).
pub fn setup() -> egui_wgpu::WgpuSetup {
    let (backends, reason) = choose(
        wgpu::Backends::from_env(),
        cfg!(windows),
        dx12_hardware_adapter,
    );
    log::info!("graphics backends {backends:?} ({})", explain(reason));
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = backends;
    egui_wgpu::WgpuSetup::CreateNew(setup)
}

fn explain(reason: Reason) -> &'static str {
    match reason {
        Reason::Override => "set by WGPU_BACKEND",
        Reason::Dx12Hardware => "DX12 on Windows; set WGPU_BACKEND to choose another",
        Reason::Default => "wgpu's choice; set WGPU_BACKEND to choose one",
    }
}

/// One line naming an adapter, its backend and its driver.
pub fn describe(info: &wgpu::AdapterInfo) -> String {
    let driver = [info.driver.as_str(), info.driver_info.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut line = format!("{} ({:?}, {:?}", info.name, info.backend, info.device_type);
    if !driver.is_empty() {
        line.push_str(", ");
        line.push_str(&driver);
    }
    line.push(')');
    line
}

/// The message shown when the window cannot start.
pub fn failure_message(error: &str) -> String {
    format!(
        "OpenChuckCAM could not open its window: {error}\n\n\
         The graphics driver may not support the backend that was tried. \
         Start the program from a command prompt with RUST_LOG=info to see \
         which adapter and backend were chosen, and try another backend with \
         WGPU_BACKEND set to dx12, vulkan or gl."
    )
}

/// The message printed when the first frame has not been drawn in time.
pub fn stall_message(stage: Stage, waited: Duration) -> String {
    let at = match stage {
        Stage::Starting => "the graphics adapter, device or window surface was not ready",
        Stage::DeviceReady => "the graphics device is ready but no frame has been drawn",
        Stage::Drawing => "the first frame was drawn",
    };
    format!(
        "warning: after {} s {at}. The graphics driver may have stopped responding. \
         Close the program and try another backend with WGPU_BACKEND set to dx12, \
         vulkan or gl; RUST_LOG=info shows which adapter was chosen.",
        waited.as_secs()
    )
}

/// How far the window has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Before the device exists.
    Starting,
    /// The adapter, device and surface exist.
    DeviceReady,
    /// The interface has been laid out at least once.
    Drawing,
}

static STAGE: AtomicU8 = AtomicU8::new(0);

/// Records that the window has reached `stage`.
pub fn reached(stage: Stage) {
    STAGE.fetch_max(stage as u8, Ordering::Relaxed);
}

fn stage() -> Stage {
    match STAGE.load(Ordering::Relaxed) {
        0 => Stage::Starting,
        1 => Stage::DeviceReady,
        _ => Stage::Drawing,
    }
}

/// Starts a thread that prints [`stall_message`] if the first frame has not
/// been laid out within [`STALL_LIMIT`]. A driver that hangs cannot be
/// stopped from inside the program, but the user is no longer left with
/// nothing.
pub fn watch_for_stall() {
    let started = Instant::now();
    let _ = std::thread::Builder::new()
        .name("startup watch".into())
        .spawn(move || {
            while started.elapsed() < STALL_LIMIT {
                if stage() == Stage::Drawing {
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            if stage() != Stage::Drawing {
                let message = stall_message(stage(), STALL_LIMIT);
                log::warn!("{message}");
                eprintln!("{message}");
            }
        });
}

/// Logs the adapter eframe chose.
pub fn log_adapter(render_state: Option<&egui_wgpu::RenderState>) {
    match render_state {
        Some(state) => log::info!("graphics adapter {}", describe(&state.adapter.get_info())),
        None => log::warn!("no wgpu render state: the 3D viewport is off"),
    }
}

/// A log level filter from `RUST_LOG`: comma-separated directives, each a
/// level (`info`) or a module path and a level (`wgpu_core=debug`). The
/// longest matching path wins. Without `RUST_LOG` this program logs `info`
/// and other crates `warn`, so the adapter line and every warning appear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    default: log::LevelFilter,
    targets: Vec<(String, log::LevelFilter)>,
}

impl Filter {
    /// The filter `RUST_LOG` describes, or the default when it is unset.
    pub fn parse(spec: Option<&str>) -> Self {
        let Some(spec) = spec else {
            return Self {
                default: log::LevelFilter::Warn,
                targets: vec![(env!("CARGO_CRATE_NAME").to_owned(), log::LevelFilter::Info)],
            };
        };
        let mut filter = Self {
            default: log::LevelFilter::Error,
            targets: Vec::new(),
        };
        for directive in spec.split(',').map(str::trim).filter(|d| !d.is_empty()) {
            match directive.split_once('=') {
                Some((target, level)) => {
                    if let Ok(level) = level.trim().parse() {
                        filter.targets.push((target.trim().to_owned(), level));
                    }
                }
                None => match directive.parse() {
                    Ok(level) => filter.default = level,
                    // A bare path turns everything on for it, as env_logger does.
                    Err(_) => filter
                        .targets
                        .push((directive.to_owned(), log::LevelFilter::Trace)),
                },
            }
        }
        filter
    }

    /// The most detailed level logged for `target`.
    pub fn level(&self, target: &str) -> log::LevelFilter {
        self.targets
            .iter()
            .filter(|(path, _)| {
                target == path
                    || target
                        .strip_prefix(path.as_str())
                        .is_some_and(|rest| rest.starts_with("::"))
            })
            .max_by_key(|(path, _)| path.len())
            .map_or(self.default, |(_, level)| *level)
    }

    fn max(&self) -> log::LevelFilter {
        self.targets
            .iter()
            .map(|(_, level)| *level)
            .fold(self.default, Ord::max)
    }
}

struct Logger {
    filter: Filter,
    started: Instant,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.filter.level(metadata.target())
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            let at = self.started.elapsed();
            let _ = writeln!(
                std::io::stderr().lock(),
                "[{:>4}.{:03} {:<5} {}] {}",
                at.as_secs(),
                at.subsec_millis(),
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

/// Sends `log` records to standard error under `RUST_LOG`, so that wgpu's
/// and eframe's own messages reach the user.
pub fn init_logging() {
    let filter = Filter::parse(std::env::var("RUST_LOG").ok().as_deref());
    let max = filter.max();
    if log::set_boxed_logger(Box::new(Logger {
        filter,
        started: Instant::now(),
    }))
    .is_ok()
    {
        log::set_max_level(max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_nfr_02_windows_prefers_dx12_hardware() {
        let (backends, reason) = choose(None, true, || true);
        assert_eq!(backends, wgpu::Backends::DX12);
        assert_eq!(reason, Reason::Dx12Hardware);
    }

    #[test]
    fn req_nfr_02_windows_without_dx12_hardware_keeps_default() {
        // A runner with only WARP, or a machine without DX12: as before.
        let (backends, reason) = choose(None, true, || false);
        assert_eq!(backends, DEFAULT_BACKENDS);
        assert_eq!(reason, Reason::Default);
        assert!(
            backends.contains(wgpu::Backends::VULKAN | wgpu::Backends::DX12 | wgpu::Backends::GL)
        );
    }

    #[test]
    fn req_nfr_02_other_systems_keep_default_without_probing() {
        let (backends, reason) = choose(None, false, || panic!("probed off Windows"));
        assert_eq!(backends, DEFAULT_BACKENDS);
        assert_eq!(reason, Reason::Default);
    }

    #[test]
    fn req_nfr_02_wgpu_backend_overrides_without_probing() {
        let (backends, reason) = choose(Some(wgpu::Backends::VULKAN), true, || panic!("probed"));
        assert_eq!(backends, wgpu::Backends::VULKAN);
        assert_eq!(reason, Reason::Override);
    }

    #[test]
    fn req_nfr_07_failure_message_names_the_error_and_the_override() {
        let message = failure_message("no suitable adapter");
        assert!(message.contains("no suitable adapter"));
        assert!(message.contains("WGPU_BACKEND"));
        assert!(message.contains("RUST_LOG=info"));
    }

    #[test]
    fn req_nfr_07_stall_message_names_the_stage() {
        let message = stall_message(Stage::Starting, Duration::from_secs(30));
        assert!(message.contains("after 30 s"));
        assert!(message.contains("surface was not ready"));
        assert!(message.contains("WGPU_BACKEND"));
        assert!(stall_message(Stage::DeviceReady, STALL_LIMIT).contains("no frame"));
    }

    #[test]
    fn req_nfr_07_describe_names_backend_and_driver() {
        let mut info = wgpu::AdapterInfo::new(wgpu::DeviceType::IntegratedGpu, wgpu::Backend::Dx12);
        info.name = "Intel(R) UHD Graphics 620".into();
        info.driver_info = "31.0.101.2111".into();
        assert_eq!(
            describe(&info),
            "Intel(R) UHD Graphics 620 (Dx12, IntegratedGpu, 31.0.101.2111)"
        );
    }

    #[test]
    fn req_nfr_07_log_filter_default_shows_own_info_and_others_warnings() {
        let filter = Filter::parse(None);
        assert_eq!(
            filter.level("openchuck_ui::graphics"),
            log::LevelFilter::Info
        );
        assert_eq!(filter.level("wgpu_hal::vulkan"), log::LevelFilter::Warn);
    }

    #[test]
    fn req_nfr_07_log_filter_reads_rust_log() {
        let filter = Filter::parse(Some("info, wgpu_core=debug,wgpu_hal::vulkan=trace,eframe"));
        assert_eq!(filter.level("openchuck_ui"), log::LevelFilter::Info);
        assert_eq!(filter.level("wgpu_core::instance"), log::LevelFilter::Debug);
        assert_eq!(
            filter.level("wgpu_hal::vulkan::instance"),
            log::LevelFilter::Trace
        );
        assert_eq!(filter.level("wgpu_hal::dx12"), log::LevelFilter::Info);
        assert_eq!(filter.level("eframe::native"), log::LevelFilter::Trace);
        assert_eq!(filter.level("wgpu_core_x"), log::LevelFilter::Info);
        assert_eq!(filter.max(), log::LevelFilter::Trace);
        // A bad level is ignored, not fatal.
        assert_eq!(
            Filter::parse(Some("wgpu=loud")).level("wgpu"),
            log::LevelFilter::Error
        );
    }
}
