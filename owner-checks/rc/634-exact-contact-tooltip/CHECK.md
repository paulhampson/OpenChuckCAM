# Check for issue #634: the tooltip of "Exact STEP surface contact" is short

> [!NOTE]
> **Draft, to run on the release candidate.** The fix for #634 is not merged yet (8 Nov 2026). This file gives today's tooltips, read from `main` at `1f3675d52`, and what the issue asks for. When the fix merges, the new tooltips are copied here from that build, its pull request is named, and this note is removed.

**What is to be fixed:** the tooltip of **Exact STEP surface contact** carries a measurement, a decision record number and more. The issue asks for everything after "More accurate and much slower." to go. The setting has two tooltips today: one for axial finishing and pencil clean-up, one for helical finishing, rest machining and simultaneous finishing; the project shows both.

**Time:** about 2 minutes.

> [!WARNING]
> Nothing in this check shows that a program is safe to run. It looks at tooltips only. Generated G-code must be verified by you before it is run on a machine.

## The project

The project (in the zip, and as files in [`project/`](project/)), written by [`generate.py`](generate.py) from public material only: the project of the #351/#257 check (the public STEP fixture `testdata/step/valid/shouldered_shaft.step`, with **Axial, tapered ball** and **Pencil, flat**), and a third operation, **Helical, ball** (helical finishing with a 3.175 mm ball). Nothing needs generating.

## Steps

Use the release candidate, or a build from `main` that holds the fix (**Help > About OpenChuckCAM** shows the commit).

1. Download [`634-exact-contact-tooltip.zip`](634-exact-contact-tooltip.zip) (or take it from the [raw link](https://github.com/paulhampson/OpenChuckCAM/raw/owner-checks/2026.1-alpha/owner-checks/rc/634-exact-contact-tooltip/634-exact-contact-tooltip.zip)), unzip it anywhere, and open the folder `634-exact-contact-tooltip/project` with **File > Open project**.
2. Click **Operations** and **1 Axial, tapered ball**. Hover over **Exact STEP surface contact** in the Inspector and read the tooltip.
3. Do the same on **2 Pencil, flat** and on **3 Helical, ball**.

## Expected result

- Each tooltip is short and plain: what the setting does, ending at "More accurate and much slower." (or the helical wording's "More accurate and can be slower."), with no measurement, no decision record number and no test-part figures. The exact texts will be copied here from the merged build.

**Old behaviour (today, `main` at `1f3675d52`, `crates/openchuck-path/src/contact.rs`):**

- Axial and pencil: "On a STEP part, drop the tool (a ball, tapered ball or flat end) on the part's exact surfaces instead of its tessellation. More accurate and much slower. Measured once on one test part (decision 0338): pencil clean-up took about 31 times as long and cut at most 0.0066 mm into the surfaces instead of 0.034 mm; axial finishing about 7 times as long, 0.0041 mm instead of 0.041 mm. Times differ by part. No effect on a mesh part. Check the program before running it."
- Helical (also rest machining and simultaneous finishing): "On a STEP part, drop a ball-end tool on the part's exact surfaces instead of its tessellation. More accurate and can be slower. Measured on one test part (decision 0277): the deepest cut into the surfaces fell from about 0.03-0.04 mm to about 0.004 mm, and the whole job took about the same time; other parts may take longer. Off drops on the tessellation. No effect on a mesh part. Check the program before running it."

## Report back

Reply on issue #634 with: **PASS**, or **FAIL and what you saw** (the tooltip as it reads, for each of the three operations). Numbers and messages only; no project files from your own work.

## What we saw when preparing this

The tooltips are read from the code on `main` at `1f3675d52`; the interface cannot be run without a display here. `openchuck-cli validate` reports the project valid on that commit, with the STEP reader built from it.
