# 0164: Tool windows go back inside the main window (lane BUG-IMP; #222; OI-580)

- **Status:** proposed. The owner chose option A (relayed by the coordinator session, 20 Oct 2026); the record itself is accepted only by the project owner, in this session. It would amend [decision 0029](0029-slice-8-ui-shell.md) item 17 and the matching text of the user interface specification; neither is edited until the owner decides.
- **Date:** 20 Oct 2026.
- **Open items:** OI-580.

## Context

Decision 0029 item 17 (OI-130) shows the import dialog, and later the tool library window and the export dialog, as their own operating-system windows through the `floating` helper of `openchuck-ui`, so they can be moved to a second screen. Trial feedback F-66 (issue #222, Windows nightly) found that in import step 2 the controls are squashed: the screenshot (`docs/trial-feedback-images/f-66-import-step-2-squashed.png` on `claude/upbeat-davinci-jk46yd`) shows the path row, then text running over the "Largest side after conversion" row, a large empty area, and no units or placement controls. The owner answered that the squashed part is a pane inside the dialog (the window itself is large enough), and asked that the import, tool library and export dialogs all go back inside the main window. A dialog inside the main window cannot be left behind it, which probably fixes #145; #146 (fixed size, not too tall) and #147 (the loading indicator shifting the layout) are layout rules of the same helper.

## Options

- **A. Reverse item 17 for all three dialogs (recommended).** `floating` always draws an in-window egui window, sized to its content and clamped to the main window. The fallback that item 17 already has becomes the only path. The step 2 layout fault is rechecked on Windows after the move.
- **B. Keep operating-system windows and fix only the layout.** Does not meet the owner's request, and leaves #145.
- **C. Embed the import dialog only.** The owner has said all three.

## Proposed decision (option A, if the owner accepts)

1. Item 17 of decision 0029 is replaced: tool windows (import, tool library, export) open inside the main window, never as separate operating-system windows. The close button means Cancel, as before.
2. The specification text that says tool windows open in their own window where the platform allows is replaced by: "inside the main window, sized to fit". The wording is proposed here; the specification is edited only after the owner agrees.
3. Layout rules in the helper: a dialog is never larger than the main window; the header and footer stay in view and the body scrolls (UI-16); text wraps rather than overlapping the next row; the loading indicator takes its space before the content is known (#147).

## What was built

`floating.rs` now draws every dialog as an `egui::Window` in the main window's frame: sized to fit the main window (never larger, UI-16), first placed in the centre, draggable only inside the main window, with a close box that means Cancel. Import and the tool library keep their sizes; the import dialog is not resizable. The focus-return helper (`FocusReturn`, `raise`; OI-430) is removed, since a dialog inside the main window cannot be left behind it. The import frame test reads where the body, footer text and buttons are painted and checks that none is squashed (#222).

## Proposed specification wording

Replace "tool windows open in their own operating-system window where the platform allows, and inside the main window where it does not" with "tool windows (import, tool library, export) open inside the main window, sized to fit it". The specification is not edited here.

## Consequences

- Windows-only checks (the dialog cannot be hidden when the application is in the background, #145; size, #146; no shift, #147; the squashed step 2 of F-66 does not appear) are the owner's: OI-580.
- The squashed step 2 was not reproduced in this container before the move (a footer panel with a right-to-left button row keeps its height over several frames in a unit test), so its cause is unknown; after the move the layout test passes, and the owner rechecks on Windows.
