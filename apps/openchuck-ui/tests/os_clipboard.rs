// SPDX-License-Identifier: AGPL-3.0-or-later
//! The desktop build links the operating system's clipboard (UI-12, NFR-16, NFR-07;
//! decision 0493, issue #431).
//!
//! egui hands copied text (`OutputCommand::CopyText`, from Copy all, the About window's
//! Copy, Copy paths and Ctrl+C on selected text) to eframe, which passes it to
//! `egui_winit::clipboard::Clipboard::set_text`, and turns Ctrl+V into
//! `Event::Paste(Clipboard::get())`. Without eframe's `clipboard` feature that type keeps
//! the text in a string of its own, inside the process: nothing reaches another program
//! and nothing pasted from one arrives. The unit tests of the panels check the text egui
//! is asked to copy; this test checks the step after it, through the real clipboard.
//!
//! Each `Clipboard` keeps its own fallback string, so text written by one instance can be
//! read by a second, new instance only if it went through the operating system. The
//! `egui-winit` dev-dependency has no features of its own, so the test sees the backend
//! only when the shipped `eframe` line enables it.
//!
//! Linux needs an X11 display (X11 or XWayland): without `DISPLAY` the test says it is
//! skipped, unless `OPENCHUCK_REQUIRE_DISPLAY` is set (the UI smoke job runs it
//! under Xvfb with that set), when it fails. Windows and macOS runners have a clipboard.
//! Copy, paste and the no-clipboard case run in one test, one after the other, because the clipboard is shared
//! by every process on the machine.

// `Clipboard::new` is unsafe only for the display handle it is given; it is given none.
#![allow(unsafe_code)]

use egui_winit::clipboard::Clipboard;

/// A new instance, as the application makes one at start-up.
fn clipboard() -> Clipboard {
    // SAFETY: no display handle is passed, so there is nothing that must outlive it.
    unsafe { Clipboard::new(None) }
}

/// Whether this machine has a clipboard to test against; on Linux, an X11 display.
///
/// The test passes no display handle, so on Linux it reaches only `arboard`, not the
/// `smithay-clipboard` backend the application uses on Wayland through its window. A
/// Wayland session whose compositor lacks the data-control protocol would leave `arboard`
/// on the in-process string and fail the round-trip although the application copies, so
/// only `DISPLAY` (X11 or XWayland) counts here.
fn has_clipboard() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    if set("DISPLAY") {
        return true;
    }
    assert!(
        !set("OPENCHUCK_REQUIRE_DISPLAY"),
        "OPENCHUCK_REQUIRE_DISPLAY is set but there is no X11 DISPLAY"
    );
    false
}

/// Text unlikely to be on the clipboard already, so an old value cannot pass the test.
fn marker(what: &str) -> String {
    format!("OpenChuckCAM {what} test {}", std::process::id())
}

#[test]
fn req_ui_12_req_nfr_16_req_nfr_07_copied_text_reaches_other_programs_and_pasted_text_arrives() {
    if !has_clipboard() {
        // NFR-07: with no X11 display the backend cannot connect and falls back to the
        // in-process string; neither call may panic.
        let mut clipboard = clipboard();
        clipboard.set_text(marker("no display"));
        let _ = clipboard.get();
        println!(
            "skipped: no X11 display, so no clipboard to test (set OPENCHUCK_REQUIRE_DISPLAY to fail instead)"
        );
        return;
    }

    // Copy: the application's clipboard writes, another program reads.
    let copied = marker("copy");
    let mut app = clipboard();
    app.set_text(copied.clone());
    let mut other = clipboard();
    assert_eq!(
        other.get().as_deref(),
        Some(copied.as_str()),
        "text the application copies must be on the operating system's clipboard \
         (eframe's `clipboard` feature, decision 0493)"
    );

    // Paste: another program writes, a new application clipboard reads, as Ctrl+V does.
    let pasted = marker("paste");
    other.set_text(pasted.clone());
    let mut fresh = clipboard();
    assert_eq!(
        fresh.get().as_deref(),
        Some(pasted.as_str()),
        "text another program copied must paste into the application's fields"
    );
    drop(app);
}
