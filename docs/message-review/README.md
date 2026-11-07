# Message review: the inventory (issue #567)

The inventory behind [decision 0536](../decisions/0536-message-levels-operation-reference-and-setting-links.md) (proposed). It lists every message the application, the command line and the library crates show to a user, one row per message or per family. It was read from `main` at `e869d932` on 7 Nov 2026, and file and line references are to that commit. The record holds the proposed policy; these files hold the evidence.

## Files

| File | Area | Id prefix |
| --- | --- | --- |
| [path.md](path.md) | Path strategies: warnings, notes and refusals (`openchuck-path`) | P |
| [setup.md](setup.md) | Setup, workholding, stock, machine, tool, model, import and project messages (`openchuck-app`, `openchuck-types`, `openchuck-import`, `openchuck-project`) | S |
| [export.md](export.md) | Export checks, posting, G-code checks and replay, simulation, Results levels | E |
| [ui.md](ui.md) | Interface: the Messages panel, marks, inspector lines, dialogs, the Results tab, the links to settings, and the specification comparison | U |
| [ui-other.md](ui-other.md) | Interface: confirmations, status and progress, empty states, field validation, error dialogs, tooltips that state a rule, recovery | V |
| [errors.md](errors.md) | Error and refusal texts of the library crates and the application's errors | R |
| [cli.md](cli.md) | The command line, the setup sheet, and (listed only) help, About, log and crash texts | C |

## Columns

Each row gives:

- the text or template, and where it is made (file and line);
- its level today;
- whether it needs acknowledging (GC-16);
- whether it names the operation, and the tool;
- the setting it concerns, and the link it has today;
- who sees it;
- the proposed class, the reason against the owner's rule, and the proposed change of wording or link.

The proposed classes are:

| Class | Meaning |
| --- | --- |
| KEEP-ERROR, KEEP-WARNING, KEEP-INFO | The level stays. |
| TO-INFO, TO-WARNING, TO-ERROR | The level changes. |
| REMOVE-OR-MERGE | The message is a duplicate. |
| KEEP-STATUS, KEEP-CONFIRM, KEEP-EMPTY, KEEP-VALIDATION | The message is not one of the three levels. |
| FLAG | Listed only, outside the record unless the owner says otherwise (OI-2099). |

A "?" marks a judgement for the owner.

## Reading the counts

The rows were gathered by area. A message shown in two places (for example a path warning, in the path rows and in the interface rows) has a row in each. The record's table gives the counts per area and says where they overlap.

The key claims were checked in the code before the record was written:

- every note is a warning in the Results tab;
- "Show setting" marks the first named setting, whichever warning was clicked;
- the status line is never drawn;
- workholding Apply refuses on every zone finding but one;
- the command line writes before it checks;
- the "verified path geometry" line is shown.

The other rows are a reading of the code, not a test. A row found wrong is corrected when its pull request is built.
