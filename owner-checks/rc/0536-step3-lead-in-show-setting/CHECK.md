# Check for decision 0536 build step 3: every message has its lead-in, and "Show setting" goes to the field

> [!NOTE]
> **To run on the release candidate.** Build step 3 is merged (pull request #648, `main` at `645256c3`); the texts below are those of `main` at `81bade8a3`. This check stays in `rc/` until the coordinator opens the tracking issue for decision 0536 steps 3 to 5, where it is posted.

**What changed:** every operation message starts with a topic and its context, `<Topic> (operation '<name>'[, tool '<name>']): ...`, as the flute-length and tab-length messages already do, and ends with what to change, named by the label the Inspector shows. **Show setting** goes to that setting, opens its section, scrolls to it and marks it, for every kind of field: numbers, tick boxes, drop-down lists, and a tool's own fields in the tool library.

**Time:** about 10 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at messages and links only. Do not run this project's G-code on a machine. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the user guide's baluster in 74 mm of 24 mm stock from X -13 to 61, the chuck holding 8 mm, nothing at the far end. Three operations:

- **Short flutes** (first): helical finishing of the whole part with a ball whose flutes are 5 mm long. It warns about the flute length (a tool's field).
- **Rough**: rotary roughing with a 3.175 mm flat end mill.
- **Free ends**: helical finishing with a 3.175 mm ball, **Face free ends** on and a **Stepdown** of 2 mm. It warns that the ring at X 0 can part the part (a tick box and a number).

## Steps

Use the release candidate, or a build from `main` at commit `645256c3` or later (**Help > About OpenChuckCAM** shows the commit).

1. Download [`0536-step3-lead-in-show-setting.zip`](0536-step3-lead-in-show-setting.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/0536-step3-lead-in-show-setting/0536-step3-lead-in-show-setting.zip)), unzip it anywhere, and open the folder `0536-step3-lead-in-show-setting/project` with **File > Open project**.
2. Click **Operations**, then **Generate**.
3. Read every warning in the **Messages** list. For each, check that it starts with a short topic and the operation in brackets, and ends with what to change.
4. For each warning that has **Show setting**, click it. Check that the right operation opens, the Inspector scrolls to the named setting, and the setting is marked:
   - the **Flute length** warning of **Short flutes**: it names **Finishing layers** (a tick box on **Short flutes**);
   - the **Part cut free** warning of **Free ends**: **Face free ends** (a tick box on **Free ends**);
   - the **Held region** warnings name no setting: they have only the link to their operation.
5. Read the information lines in the **Results** tab the same way (they need no Show setting unless they name a setting).

## Expected result

- Every line in Messages and Results starts with a topic and its context. As the command-line tool prints the warnings:

  > Flute length (operation 'Short flutes', tool 'Short-flute ball 3.175x5 (check 0536)'): the stock stands up to 9.335 mm above the tool's tip along its axis (at X 61.000, A 0.0), more than the tool's 5.000 mm of flute, so the shank may meet the stock; the stock is a 24.000 mm diameter cylinder on the rotary axis. The path was made: check it before running it. Use a tool with longer flutes, or rough the part first in this setup, or turn on Finishing layers

  > Held region (operation 'Short flutes'): Nothing within one tool radius of the held region at the start of the stock is cut: no part of the tool reaches over the gripped length of the stock.

  > Part cut free (operation 'Free ends'): the end face at X 0.000 is faced down to the rotary axis: the tool cuts a ring 3.175 mm wide from X -3.176 to -0.001 down to the axis, and nothing in this setup holds the part on the other side of the ring (no held region at its other end, and no tab joining it to one), so the part can be cut free there and thrown while the rotary axis turns. Before running this program, check the stock and the workholding on the machine

  > Held region (operation 'Free ends'): Nothing within one tool radius of the held region at the start of the stock is cut: ...

  and the notes: "Feed limit (operation '...'): ...", "Shoulders (operation 'Rough'): ...", "Free ends (operation 'Free ends'): ..." (twice), "Rotary axis (operation 'Free ends'): moves 9888 to 9959 (72 moves): ...", and "Cutting data (operation '...'): stepover 1 set on the operation overrides ...".
- No line has an internal name, a requirement number or a decision number.
- Each **Show setting** opens the operation and marks the setting named in step 4, whether it is a number or a tick box.

**Old behaviour (before step 3, `main` at `1f3675d52`):** only the flute-length line had the topic form. The others began "Operation 'Free ends': ..." or "operation 'Rough': ...".

## Report back

Reply with: **PASS**, or **FAIL and what you saw** (the message as it reads, and where Show setting went). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

Run with the command-line tool (`openchuck-cli regenerate project`), Linux x86_64, release builds, 8 November 2026:

| Build | Result |
| --- | --- |
| Before step 3: `1f3675d52` (0.50.0) | "Flute length (operation 'Short flutes', ...)" in the topic form; "Operation 'Free ends': the end face at X 0.000 is faced down ...", "Operation 'Short flutes': Nothing within one tool radius ...", "operation 'Rough': the part has 2 steps ...": no topic. |
| `main` at `81bade8a3` (0.51.0), after steps 3 and 5 | Every line with its topic, as quoted under "Expected result". |

The settings in step 4 are read from the code at `81bade8a3` (the flute check names Finishing layers; the free-end checks name Face free ends; the held-region note names none). The interface (Messages, Show setting) cannot be run without a display here. `openchuck-cli validate` reports the project valid.
