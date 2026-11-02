# 0493: The desktop build links the operating system's clipboard (UI-12, NFR-16, NFR-07; issue #431; lane RESULTS-COPY)

- **Status:** accepted by the project owner on 1 Nov 2026, relayed by the coordinator (decision 0049). The owner chose the fix, eframe's `clipboard` feature rather than calling `arboard` from `openchuck-ui` for the Copy buttons only, the same day, also relayed.
- **Date:** 1 Nov 2026.
- **Issues:** #431 (the Results text cannot be copied), and its comment on the About window (OI-1580).
- **Requirements:** UI-12 (the Results window's Copy all and copyable lines), NFR-16 (the About window's Copy), NFR-07 (no crash where there is no clipboard).
- **Amends:** [0430](0430-results-feed-limit-summary-and-copyable-results.md) item 7 (the About window was "not verified here"): the cause is found and is not in the About window.
- **Open items:** OI-1580 (cause found and fixed here; closes when the owner has checked a Windows build); new OI-1920 (the native check on Windows, Linux and macOS). Related: OI-1224 (Wayland copy and paste after the egui 0.37 migration). No project schema change; no library version change.

## Context

After lane RESULTS-MSGS (decision 0430, pull request #443) made the Results lines selectable and added Copy all and a plain-text box, the owner tested a Windows build on 1 Nov 2026: copy and paste still did not work, and the About window's Copy had never worked (OI-1580). The tests passed because they check that egui is asked to copy the right text (`OutputCommand::CopyText`), not that the text reaches the operating system.

The workspace declared `eframe` with `default-features = false` and the features `wgpu`, `default_fonts`, `wayland` and `x11`. eframe's `clipboard` feature, one of its defaults, switches on `egui-winit/clipboard`, which links `arboard` (and `smithay-clipboard` on Linux). Without it, `egui_winit::clipboard::Clipboard` is, in its own words, "a fallback clipboard that just works within the same app": copied text is kept in a string inside the process, and Ctrl+V pastes from that string. So, on every operating system and in every build since eframe was first added:

- nothing copied in OpenChuckCAM (Copy all, the About window's Copy, the export result's Copy paths, Ctrl+C on selected text or in a text field) reached another program;
- nothing copied in another program pasted into an OpenChuckCAM text field;
- copy and paste inside OpenChuckCAM worked, which hid the fault.

`cargo tree -p openchuck-ui --target x86_64-pc-windows-msvc -f '{p} {f}'` showed `egui-winit v0.37.0 bytemuck,wayland,x11`. The archives (`build-archives.yml`, called by `release.yml` and the nightly build) build `-p openchuck-ui` with no extra features, so the workspace line decides. `arboard` and `clipboard-win` were in `Cargo.lock` because the lock file records optional dependencies, but they were not compiled in.

Ruled out: egui's keyboard handling (egui-winit turns Ctrl+C into `Event::Copy`, and label selection answers it), input methods, and anything specific to Windows.

## Decision

1. **Enable eframe's `clipboard` feature** in the workspace `[workspace.dependencies]` line, beside the existing features (`default-features = false` stays, so nothing else of eframe's defaults is added). This is upstream's default behaviour, one line, and fixes every copy and paste path at once.
2. **Not chosen:** calling `arboard` from `openchuck-ui` for the Copy buttons only. It would leave Ctrl+C on selected text, Ctrl+V and every text field unable to reach other programs, and add a second clipboard path beside egui's.
3. **A test through the real clipboard** (`apps/openchuck-ui/tests/os_clipboard.rs`): text set through `egui_winit::clipboard::Clipboard`, the type eframe hands copied text to, must be read back by a second, new instance, which can see it only through the operating system (each instance keeps its own fallback string); then the reverse, as Ctrl+V does. `egui-winit` is a dev-dependency of `openchuck-ui` with no features of its own, so the test gets the backend only from the shipped `eframe` line: a dev-dependency that switched on `clipboard` would make the test pass on its own through Cargo's feature unification.
4. **Where it runs.** On every pull request on the Windows and macOS legs of "Build and test", which have a clipboard. On Linux it needs a display server: the "UI smoke test" job runs it under Xvfb with `OPENCHUCK_REQUIRE_DISPLAY=1`, so it fails there rather than skip; in a Linux job with no display it prints that it is skipped and checks only that copy and paste do not crash (NFR-07). No new job or matrix name: one step is added to the existing UI smoke job.
5. **Dependencies.** The build now includes `arboard` 3.6.1 (MIT OR Apache-2.0) on every system, `clipboard-win` 5.4.3 (BSL-1.0) on Windows, and on Linux `wl-clipboard-rs` 0.9.5 (MIT OR Apache-2.0), `smithay-clipboard` 0.7.3, `smithay-client-toolkit` 0.20.0, `calloop` 0.14.5, `calloop-wayland-source` 0.4.1, `wayland-protocols-experimental` and `wayland-protocols-misc` (all MIT); the last six are new to `Cargo.lock` (`smithay-client-toolkit` 0.20 and `calloop` 0.14 come back beside the 0.19 and 0.13 versions winit uses; `deny.toml` warns on duplicates and does not fail). All are clipboard and Wayland client crates with no network client (NFR-09); the licences are in `deny.toml`'s allowed list (NFR-10). `egui-winit` 0.37.0 is declared directly, as a dev-dependency only. Recorded in `docs/dependencies.md` (NFR-14).
6. **NFR-12.** Nothing here changes what is generated or says anything about its safety.

## Documents

No change to the requirements or the architecture: `egui-winit` is one of the egui crates that `cargo xtask deps` already allows only in the UI crates. `docs/dependencies.md` (eframe row and a new `egui-winit` row), `docs/open-items.md` (OI-1580, OI-1920) and `docs/work-breakdown.md` (lane RESULTS-COPY) are updated in the same change.

## Verification

- **Reproduction (Linux, Xvfb, 1 Nov 2026).** A probe calling `egui_winit::clipboard::Clipboard::set_text` and reading the operating system's clipboard through a separate `arboard` client: without the `clipboard` feature, `Err(ContentNotAvailable)`; with it, the text.
- **Test first.** `req_ui_12_req_nfr_16_req_nfr_07_copied_text_reaches_other_programs_and_pasted_text_arrives` failed under `xvfb-run` with `OPENCHUCK_REQUIRE_DISPLAY=1` before the fix (`left: Some("")`, the new instance's empty fallback string) and passes after it (0.11 s uninstrumented); without a display it reports that it is skipped and passes the NFR-07 check.
- **Existing tests.** The panel tests that check the copied text (Copy all, About Copy, Copy paths) are unchanged and still pass.
- **Not verified here (OI-1920).** The click-to-Notepad path on a native Windows build, Wayland and X11 desktops, and macOS: the owner's check.
