# 0246: New project buttons resolve their own fields (lane BUG-NEWPROJ; issue 338)

- **Status:** accepted directly by the project owner on 26 Oct 2026: "This is an exception that I accept. The buttons are an explicit accept action that the user would not expect prompting with Keep/Discard/Cancel on."
- **Date:** 26 Oct 2026.
- **Requirements:** PRJ-01, UI-07, UI-14, NFR-07.
- **Amends:** decision 0233 item 1 and interface specification section 8, only for the New project dialog's OK and Cancel buttons.
- **Open items:** none. OI-983 remains closed; OI-584 (Unicode normalization of folder names) remains open and is independent.

## Context and decision

The New project dialog tracks its name as a pending text field. OK calls the
shared departure guard before creating the project; Cancel also enters that
guard when the field is pending. Both show "Unconfirmed changes" even though
the buttons already say whether to accept or abandon this dialog's input.

For this dialog only, enabled OK accepts the typed parent folder and project
name without a second question. Existing validation still applies: a blank
name, missing parent or unusable destination cannot create a project. Cancel
discards the dialog's input without a second question. Other actions leaving
the form, including Browse and native application close, retain decision
0233's shared warning. Esc while editing restores the last confirmed field
text, as specification section 8 says; it does not close the dialog. The
New project modal has no separate window-close control. The native window
close control closes the application and retains the shared guard.

No project file format, schema, library family or machining behavior changes.
The owner verifies the result on a desktop build and closes issue 338.
