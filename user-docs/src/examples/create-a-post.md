# Create a post-processing template

This example makes a small post-processor of your own, loads it, writes a program with it and checks the result. It uses the example post in the repository, [`testdata/examples/minimal-post`](https://github.com/paulhampson/OpenChuckCAM/tree/main/testdata/examples/minimal-post), and the tutorial's finished project, [`testdata/examples/baluster/baluster-project`](https://github.com/paulhampson/OpenChuckCAM/tree/main/testdata/examples/baluster/baluster-project). [Writing a post-processor](../reference/writing-a-post.md) is the reference for every option, variable and filter used here.

> [!WARNING]
> A post you write is yours to check. The program cannot know your controller: its checks read the program back in the LinuxCNC or Makera Z1 dialect only, and nothing they report makes a post right for your machine. Keep the safety notice on, check every program, and run an [air cut](../glossary.md#air-cut) before anything else; the [Makera Z1 air cut](../safety/air-cut.md) and [safe-practice checklist](../safety/checklist.md) show the kind of steps to follow on any machine. Generated G-code must be verified by you before it is run on a machine.

## What you need

- OpenChuckCAM, and a copy of the repository's `testdata/examples` folder (download the source, or clone it as in [Install and first run](../start/install.md#build-from-source)).
- A text editor.

## 1. Look at the example post

The folder has two files.

`post.toml` names the post and sets its options:

```toml
schema = 1
name = "Minimal example"
template = "minimal.post"
feed_rule = "linuxcnc"

[options]
units = "mm"
a_letter = "A"
a_direction = "same"
unwind = "rotate"
header = ["G90 G21 G17"]
footer = ["M5", "M30"]
work_offset = "G54"
coolant = "off"
feed_mode = "g94"
comments = true
safety_notice = true
file_extension = "ngc"

[options.tool_change]
stop_spindle_first = true
length_offset = false

[options.line_numbers]
enabled = false
start = 10
step = 10

[options.decimals]
pos = 3
a = 3
f = 1
```

Every key is required unless [the reference](../reference/writing-a-post.md#options) says it is optional, and a key the program does not know is refused. Three choices matter most:

- `feed_rule` is how the F word is worked out on a move that turns A. Choose the rule of your controller: `linuxcnc` applies F to the X, Y and Z distance, `makera` is the Makera Z1's rule.
- `comments = true` and `safety_notice = true` write the two lines of the safety notice at the top of every program. The notice needs comments, and a post with the notice on and comments off is refused.
- `unwind = "rotate"` writes no command to reset the A count, so A keeps its full value to the end of the program.

`minimal.post` is the template. Text outside `{% %}` and `{{ }}` is written as it is; `{{ ... }}` writes a value; `{% ... %}` is logic:

```jinja
{% set st = namespace(at="") %}
({{ title }})
{% if generator is not none %}
({{ generator }})
{% endif %}
{% for notice in safety_notice %}
({{ notice }})
{% endfor %}
{% for h in opt.header %}
{{ h }}
{% endfor %}
{{ opt.work_offset }}
{% for it in steps %}
{% if it.step == "comment" %}
({{ it.text }})
{% elif it.step == "tool_change" %}
M5
T{{ it.number }} M6
{% elif it.step == "spindle_on" %}
S{{ it.rpm|fixed(0) }} M3
{% elif it.step == "rapid" or it.step == "feed" %}
{% set at = "X" ~ it.target.x|fixed(opt.decimals.pos) ~ " Y" ~ it.target.y|fixed(opt.decimals.pos) ~ " Z" ~ it.target.z|fixed(opt.decimals.pos) ~ " A" ~ it.target.a|fixed(opt.decimals.a) %}
{% if at != st.at %}
{% if it.step == "rapid" %}
G0 {{ at }}
{% else %}
G1 {{ at }} F{{ it.feed.value|feed(opt.decimals.f) }}
{% endif %}
{% set st.at = at %}
{% endif %}
{% endif %}
{% endfor %}
{% for f in opt.footer %}
{{ f }}
{% endfor %}
```

It writes the opening comments, the header and the work offset, then one line or more for each step of the program, then the footer. Unlike the built-in posts it writes every axis word on every move, which makes a longer file that is easier to read. `fixed(3)` writes a number with three decimal places, and `feed(1)` writes the feed with at least three significant figures. The `st.at` variable remembers the last position, so a move that would go nowhere is left out (see [Common errors](#common-errors)).

## 2. Make your own copy

1. Copy the `minimal-post` folder to a folder of your own, for example `my-post`.
2. In `my-post/post.toml`, change `name` to `My post`.
3. Make one change you can see: set `[options.line_numbers]` `enabled = true`. The template above does not write line numbers yet, so also change each line it writes; the built-in template `crates/openchuck-post/posts/makera-z1/main.post` shows how, with its `line` macro. Or leave line numbers off for this first try.

## 3. Write a program with it

In the program:

1. Copy `testdata/examples/baluster/baluster-project` to a folder of your own, and open the copy with **File > Open project**.
2. Generate the toolpaths, and simulate them, as in the [tutorial](../tutorial/08-generate.md).
3. On the Simulate step, press **Post and export...**. Under **Post-processor**, set **Post** to **A post folder**.
4. In **Post folder**, type the folder of your post, for example `/home/you/my-post`.
5. Read the **Checks before writing**, tick **I will verify this G-code before running it on a machine** if asked, and press **Write G-code**.

If the folder cannot be read, the panel says why, in red, before you export.

On the command line, the same is one command:

```text
openchuck-cli regenerate baluster --post my-post --tool-numbers keep --out baluster-out
```

## 4. Read the result

The program starts like this:

```text
(Baluster - Setup 1)
(OpenChuckCAM 0.49.0)
(This program has not been run on your machine. Check it before you run it.)
(Simulate it and air cut it first. You are responsible for what the machine does.)
G90 G21 G17
G54
(operation Rough, tool T104 Z1 O-flute 3.175x25, about 805 s)
M5
T104 M6
S10000 M3
G0 X9.088 Y0.000 Z40.000 A0.000
G0 X9.088 Y0.000 Z13.000 A0.000
G1 X9.088 Y0.000 Z11.001 A0.000 F300.0
```

and ends with the footer, `M5` and `M30`. The version in the second line is the version of your program.

After writing, the program reads the file back and replays it, and compares the replay with the toolpath. The command line prints, for example:

```text
01-setup-1.ngc: 29099 moves; replay matches the toolpath (tip 0.0007 mm, A 0.0005 deg; cut 1564.6 s, toolpath 1564.5 s)
```

This says the file holds the same moves as the toolpath, within the numbers shown, and takes the same time. It does not say that your controller reads the file as the program does. Read the start, a tool change and the end of the file yourself, and compare one with a program from the built-in post you started from.

## 5. Test it in a simulator, then on the machine

1. If your controller is LinuxCNC, or close to it, run the program in the LinuxCNC simulator first, as the project does with its own posts.
2. Run an air cut on the machine: no tool, no material, reduced feed, with your hand on the stop.
3. Only then cut, in a soft material first.

## Common errors

These are the messages the command line gives for each mistake; the export panel shows the same text.

`options.safety_notice: is written as comment lines, so it needs comments = true; set comments = true, or safety_notice = false`
: The notice is on and `comments = false`. Turn comments on. If you turn the notice off instead, your programs carry no reminder to check them.

``post definition ...: unknown field `colour`, expected one of ...``
: A key in `post.toml` is misspelt, or in the wrong table. The message names it and lists the keys that are allowed there.

`feed_rule "linuxcnc" cannot be written in feed_mode G93: inverse_time goes with g93, the others with g94`
: `feed_mode` and `feed_rule` do not agree.

An error with the template's name and a line number
: A mistake in the template, such as a misspelt variable (`it.taget`): the template stops at the first unknown name instead of writing empty text.

`replay does not match the toolpath: the G-code has moves beyond the end of the IR`
: The template wrote a move that goes nowhere, such as two lines with the same position. The replay counts the moves, so they no longer match. The `st.at` check in the example leaves such moves out.

The export is refused because the notice is missing
: `safety_notice = true`, but the template does not write the `safety_notice` lines. Keep the `{% for notice in safety_notice %}` loop.

## What the example does not do

The example post is for learning the format. It is written for the LinuxCNC dialect and is not set up for any machine. It has no line numbers, no tool length offset, and no coolant. It keeps A at its full value, so a long program reaches large A values; a controller that wraps A, or needs the count reset, needs its own `unwind` choice.
