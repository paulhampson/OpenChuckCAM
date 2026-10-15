# User interface design

This folder holds the design of the desktop application's interface.

| Document | What it settles |
| --- | --- |
| [ui-specification.md](ui-specification.md) | How the application looks and behaves: window layout, workflow steps, project tree, viewport, inspector, dock, footer, the screens of each step, supporting windows, per-user settings, visual tokens, wording, keyboard and accessibility, and how it differs from what is built |
| [mockups/](mockups/) | Rendered images of the design canvas boards (listed in specification section 20), and their sources in `mockups/source/` |

**Status.** Accepted on 15 Oct 2026 with [decision 0038](../decisions/0038-ui-specification-and-requirements.md), which also added the requirements the design needs (UI-10 to UI-24, SIM-11 to SIM-13, GC-15, GC-16, NFR-15). It ranks with the decision records; an accepted record wins over it where they disagree.

**Design canvas.** The mockups are on a claude.ai design canvas, private to the project owner: <https://claude.ai/artifact/1Qi5cpiyAiz3r5DkFnXkbu>. The specification is the reviewed copy; the canvas and the images are working drafts, like the other documents that began as artifacts (`docs/README.md`). When they disagree, the specification wins and the canvas and images are updated.

**Rendering the images again.** Each `.dc.html` source is a canvas board. To render one outside the canvas, remove its `support.js` script and its `text/x-dc` script block, replace `{{theme}}` with `dark` or `light` and `{{w}}` and `{{h}}` with the board's size, and screenshot it in headless Chromium at the board's size and 100% scale: 1440 by 900 for the workspaces, 1366 by 768 for the small-window check, and each dialog's own size (the size of its image). The canvas's own Export also works.
