# Install and first run

OpenChuckCAM has no stable release yet. You can download a development build, or build the program from its source code.

> [!NOTE]
> A development build has passed none of the project's release checks. Use it to learn the program and to try it on your own parts, and check every program it writes before you run it on a machine (see [Checking G-code before you run it](../safety/checking-gcode.md)).

## Download a development build

When the program's code changes, the project builds it for three systems and publishes the result as the rolling [`nightly` pre-release](https://github.com/paulhampson/OpenChuckCAM/releases/tag/nightly) on GitHub.

| System | Archive | Notes |
| --- | --- | --- |
| Linux | x86-64 | Needs a Vulkan or OpenGL driver, and X11 or Wayland. |
| Windows | x64 | Not signed: Windows SmartScreen may warn before it runs. |
| macOS | Apple silicon | Not signed: macOS quarantines it. The README in the archive gives the command that clears this. |

Each archive holds:

| File | What it is |
| --- | --- |
| `openchuck-ui` | The program with a window. This is the one most people use. |
| `openchuck-cli` | The command-line program, which does the same work without a window. |
| `openchuck-step-worker` | The reader for STEP files. Keep it in the same folder as the other two. |
| Licence and README | The licence texts, the third-party notices, and notes for the system. |

To install it:

1. Download the archive for your system from the [`nightly` pre-release](https://github.com/paulhampson/OpenChuckCAM/releases/tag/nightly).
2. Unpack it into a folder of its own.
3. On macOS, run the command from the archive's README to clear the quarantine.
4. Start `openchuck-ui`.

## Build from source

The program is written in Rust. The version of Rust it needs is pinned in the repository's `rust-toolchain.toml`, and `rustup` installs it for you.

1. Install Rust with [rustup](https://rustup.rs).
2. Get the source: `git clone https://github.com/paulhampson/OpenChuckCAM.git`.
3. In the `OpenChuckCAM` folder, build and start the program:

   ```text
   cargo run --release -p openchuck-ui
   ```

4. For the command line, run `cargo run -p openchuck-cli -- --help`.

Reading STEP files needs the OpenCASCADE geometry kernel, which is not built by default and takes a long time to build the first time. The [`openchuck-import` README](https://github.com/paulhampson/OpenChuckCAM/blob/main/crates/openchuck-import/README.md) says how.

## The first time it opens

The program opens with an empty window and offers **New project...** (also **File > New project...**, <kbd>Ctrl</kbd>+<kbd>N</kbd>). A [project](../glossary.md#project) is a folder of plain text files that holds the model, the setup, the tools and the operations of one job.

The program does not save by itself. Use **File > Save** (<kbd>Ctrl</kbd>+<kbd>S</kbd>) as you work. Edits in a form take effect when you press <kbd>Enter</kbd> or leave the field, and **Edit > Undo** reverses them.

Next: [Your first project](../tutorial/index.md) takes you through one job from start to finish.

## Where the program keeps its log

The program writes a log file, `openchuckcam.log`, with what happened at start-up, the graphics adapter, warnings and errors. It never leaves your computer. **Help > About** has a button that opens the folder it is in.

| System | Folder, when the program's own folder cannot be written |
| --- | --- |
| Linux | `~/.local/state/openchuckcam` |
| macOS | `~/Library/Logs/OpenChuckCAM` |
| Windows | `%LOCALAPPDATA%\OpenChuckCAM\logs` |

Otherwise the log is in the program's own folder. When you report a problem, you can attach it if you choose to.
