# Writing a post-processor

A post is a folder with two files: `post.toml`, which holds the post's name and options, and a template, which says how each line of G-code is written. You can copy a built-in post, change it, and load it from its folder without rebuilding the program.

> [!WARNING]
> Every program written by a post you made is yours to check on your machine before you run it, as for any program. The program's checks know only the LinuxCNC and Makera Z1 dialects. See [Checking G-code before you run it](../safety/checking-gcode.md).

## Start from a built-in post

The built-in posts are in the source repository, in [`crates/openchuck-post/posts/`](https://github.com/paulhampson/OpenChuckCAM/tree/main/crates/openchuck-post/posts): `makera-z1/` and `linuxcnc/`. The program carries its own copy of them, so take them from the repository, from the version that matches your program.

1. Copy the folder of the post closest to your controller to a folder of your own.
2. Change the `name` in `post.toml`, so you can tell your post from the built-in one.
3. Change the options you need (below).
4. Export with your post: in the export panel, set **Post** to **A post folder** and type the folder in **Post folder**. On the command line, use `openchuck-cli regenerate PROJECT --post FOLDER --out OUT`.
5. Read the program it writes, and compare it with one from the post you started from.

If the folder cannot be read, the export panel shows why, in red.

## `post.toml`

Every key is checked when the post is loaded. An unknown or misspelt key is refused, and the message names it, for example `options.decimals.pos`.

| Key | Values | What it does |
| --- | --- | --- |
| `schema` | `1` | The file format. This version of the program reads 1 only. |
| `name` | text | The name shown in the export panel. |
| `template` | a file name | The template file, in the same folder. |
| `feed_rule` | `linuxcnc`, `makera`, `inverse_time`, `contact` | How the F word is computed for a move that turns A. `contact` is the plain feed at the cutter and is wrong on rotary moves; it is only for tests. |

Each file can be at most 1 MiB.

### `[options]`

| Option | Values | What it does |
| --- | --- | --- |
| `units` | `"mm"` | Millimetres (`G21`). There is no other choice. |
| `a_letter` | `"A"`, `"B"`, `"C"` | The letter of the rotary axis. |
| `a_direction` | `"same"`, `"reversed"` | `reversed` turns every A value round, for a controller whose A runs the other way to the machine profile. |
| `unwind` | `"rotate"`, `"drop_turns"` | `drop_turns` writes `unwind_command` between operations to drop whole turns from the controller's count. With `rotate`, A keeps its full value and nothing is written. |
| `unwind_command` | text | The command for `drop_turns`, for example `"G92.4 A0 S0"`. |
| `header` | list of lines | Lines written after the opening comments. |
| `footer` | list of lines | The last lines of the program. |
| `work_offset` | `"G54"` to `"G59"` | Optional. The setup's own work offset replaces it. |
| `tool_change.stop_spindle_first` | `true`, `false` | Write `M5` before `T<n> M6`. |
| `tool_change.length_offset` | `true`, `false` | Add `G43 H<n>` to the tool change line. |
| `coolant` | `"off"`, `"flood"`, `"mist"` | `flood` writes `M8` and `mist` writes `M7` after the spindle starts. Nothing turns it off: put `M9` in the `footer` if your controller needs it. |
| `line_numbers.enabled`, `.start`, `.step` | `true` or `false`, then whole numbers | Write `N` numbers. |
| `decimals.pos`, `.a`, `.f` | 0 to 9 | Decimal places for X, Y and Z, for A, and for F. |
| `max_feed` | a number above 0 | Optional. The highest F to write; a higher feed is lowered, and the export says how many were. |
| `feed_mode` | `"g94"`, `"g93"` | Must agree with `feed_rule`: `g93` with `inverse_time`, `g94` with the others. |
| `combine_rotary_runs` | `true`, `false` | Optional, off when left out. Joins moves in a row that turn only A, in the same direction at the same feed, into one line. |
| `comments` | `true`, `false` | Write comments. |
| `safety_notice` | `true`, `false` | Optional, off when left out. Write the two lines of the safety notice after the version line. It needs `comments = true`. Both built-in posts have it on. |
| `file_extension` | 1 to 8 lower-case letters or digits | The file's extension, without the dot. |
| `min_block_ms`, `block_window`, `short_block_mode` | as in the machine profile | Optional. Override the machine's [short move settings](../how-to/short-block-settings.md) for this post. |

Each `header`, `footer` and `unwind_command` line is plain ASCII, at most 200 characters.

## The template

The template uses the [minijinja](https://docs.rs/minijinja) language, which is like Jinja2. It cannot read files or include other templates, and a misspelt name is an error rather than empty text. The template only arranges the output: the A direction, the turn dropping and each F value are worked out before it runs, so it cannot change them.

It receives:

`title`, `generator`, `revision`
: The opening comments: the project and setup, the program's version, and a revision when there is one.

`safety_notice`
: The two notice lines when the option is on; otherwise empty. If the option is on and the template does not write them, the export is refused.

`opt`
: Every option from `post.toml`, for example `opt.decimals.pos`.

`steps`
: The program, one step at a time. `step.step` says what kind it is:

  - `comment`, with `text`;
  - `tool_change`, with `number` and `name`;
  - `spindle_on`, with `rpm`;
  - `rapid`, with `target` (`x`, `y`, `z` in millimetres and `a` in degrees);
  - `feed`, with `kind` (`linking`, `cutting`, `plunge`, `lead_in`, `lead_out`), `target` and `feed` (`mode` and `value`);
  - `rotary_boundary`, with `at`, where whole turns can be dropped (only for `drop_turns` posts).

Besides the usual filters, the template can use `fixed(d)` (a number with `d` decimal places, never `-0`), `feed(d)` (at least 3 significant figures), `rounded(d)`, `sqrt`, `floor` and `trunc`, and the value `pi`. The built-in `main.post` shows how to write an axis word, the F word and the G code only when they change.

## Checks

When the post is loaded, the program checks every key and option and compiles the template, so a mistake is reported with its line. When it writes a program, an error in the template is reported with the template's name and line. Every program is then read back and checked, as for the built-in posts (see [G-code and posts](gcode-and-posts.md#checks-when-you-export)). The reader knows the LinuxCNC and Makera Z1 dialects: a post for another controller can write codes it does not understand, and those parts are not checked.
