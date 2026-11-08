# Settings

The program's own settings belong to you and this computer, not to a project: opening a project on another computer does not bring them. A project's own choices, such as its machine, setups and operations, are in the project folder.

## The Settings dialog

Open it with **File > Settings...** (also **Help > Settings...**).

| Setting | Default | What it does |
| --- | --- | --- |
| Recovery checkpoint interval (minutes) | 3 | How often the program keeps a recovery copy of the open project, from 1 to 15 minutes. It keeps three periodic copies and one more after 15 seconds with no change. Recovery never saves over your project by itself; it does not replace **File > Save**. |
| Memory for playback checkpoints (MB) | 1536 | How much memory the simulation's playback may use, from 256 to 8192 MB in steps of 256, and at most a quarter of the computer's memory. More memory means shorter waits after a jump in playback; the cut part shown is the same either way. A change applies the next time the cut part is prepared. |
| Generate playback information with toolpath generation | on with 16 GB of memory or more | Prepares playback while the toolpaths are generated, so it is ready sooner. Generation takes about a tenth longer. **Use this computer's default** returns to the default. |
| Skip the stock simulation in new projects | off | New projects then make only their toolpaths when generating. Each project keeps its own choice, on the Simulate step. |
| Record my actions in the log | on | Adds the menu commands, buttons, dialogs and edits you use to the [log](#the-log), with item and setting names but not their values. |

Below them the dialog shows where this run's log file is.

## Remembered without a setting

The program also remembers, between runs:

- the interface scale, from 80 % to 200 %, set with **View > Scale** or <kbd>Ctrl</kbd>+<kbd>+</kbd> and <kbd>Ctrl</kbd>+<kbd>-</kbd>;
- the window size, the panel sizes and which panels are collapsed (<kbd>Ctrl</kbd>+<kbd>1</kbd>, <kbd>2</kbd> and <kbd>3</kbd>);
- the 10 most recent projects, under **File > Open recent** (**Clear recent list** empties it);
- which Inspector sections are open;
- the Simulate view choices, such as **Turn part with A** and which of **Model**, **Cut part** and **Toolpath** are shown;
- the tool library window's folded groups and size filter;
- the file name patterns for the G-code and the setup sheet.

**View > Projection** and **View > Display mode** are not remembered.

Units are millimetres and degrees. There is no setting for inches, and no light theme yet.

## Where settings are kept

All of the above is in one file, `settings.toml`:

| System | File |
| --- | --- |
| Linux | `~/.config/OpenChuckCAM/settings.toml` (or under `$XDG_CONFIG_HOME`) |
| Windows | `%APPDATA%\OpenChuckCAM\settings.toml` |
| macOS | `~/Library/Application Support/OpenChuckCAM/settings.toml` |

The same file records that you confirmed, before your first export on this computer, that you will verify the G-code before running it. The same folder holds the [tool library](tool-library.md) (`tools/`) and your [machine profiles](machine-profiles.md) (`machines/`).

Changes are saved about a second after you make them. If the file is missing, the program uses the defaults. If it cannot be read, the program says so, uses the defaults, and keeps the old file as `settings.toml.bad` when it next saves. A value outside its range is brought back into range.

Recovery copies of a project are kept in a `.openchuck-recovery` folder inside the project folder, apart from the project's own files.

## The log

The program writes a log, `openchuckcam.log`, of what happened at start-up, the graphics adapter, warnings and errors, and, if **Record my actions in the log** is on, what you did. It never leaves your computer. **Help > About** has a button that opens its folder.

The log is in the program's own folder. When that folder cannot be written, as in an installed copy, it is in:

| System | Folder |
| --- | --- |
| Linux | `~/.local/state/openchuckcam` (or under `$XDG_STATE_HOME`) |
| macOS | `~/Library/Logs/OpenChuckCAM` |
| Windows | `%LOCALAPPDATA%\OpenChuckCAM\logs` |

At start, a log over 1 MB is renamed `openchuckcam.1.log`, and three old logs are kept. The command line does not write a log. When you report a problem, you can attach the log if you choose to.
