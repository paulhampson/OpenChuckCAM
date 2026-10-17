# 0081: Slice 23 design refinement: the default tool library (lane TL)

- **Status:** proposed (17 Oct 2026). Items 2, 5 and 6 wait for the owner (OI-268, OI-269, OI-271); the orchestrator told the lane on 17 Oct 2026 to build the standard tools only and leave the Z1 part unstarted.
- **Date:** 17 Oct 2026.
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-268 to OI-271 (from [decision 0060](0060-gc16-command-line-reading-and-tl12-default-tool-library.md)) are updated here.

## Context

[Decision 0060](0060-gc16-command-line-reading-and-tl12-default-tool-library.md) adds TL-12 (P1) and asks slice 23 to start with a record on the source and licence of the Makera data, the standard tools, cutting data, delivery and the update policy. Slice 4 ([decision 0014](0014-slice-4-tool-library.md)) built the library folder, tool sets and the sample corpus.

## Decision

1. **Source and licence of the Makera data (OI-269).** The owner named `MakeraInc/CarveraProfiles`. Read at commit `de08a657bdc9c230a094dbdba792acd70bfcdf32` (5 Aug 2026, shallow clone). It holds post-processors and tool files for many CAM programs (FreeCAD `.fctb` bit files, Fusion 360 `.tools` files, VCarve, Kiri:Moto and DeskProto files), a `Makera_Z1.fcm` machine file and Z1 design files. **It has no licence file** (no `LICENSE`, `COPYING` or `NOTICE`) and its README states no licence. The only licence text in it is on the post-processor code: Autodesk's copyright line on `Carvera.cps`, and sliptonic's LGPL-2.1 notice on the FreeCAD posts. **None of the tool data files carries a licence.** With no grant the data is under its owner's default copyright, so it is not clear that it may be embedded in an AGPL project (NFR-10). **Nothing was copied and no Makera value was re-keyed.** The Z1 reference tools and the Z1 default tool set (TL-11) stay unbuilt. Routes for the owner: (a) ask Makera for a written grant; (b) write the Z1 tools from Makera's public product specifications with a recorded source per tool, citing the repository as a reference only (OI-269's own fallback); (c) re-key the facts by hand, which the lane will not do without the owner's say-so.
2. **Draft issue text for route (a), not sent.** "OpenChuckCAM is an AGPL-3.0 CAM program for 4-axis machining. We would like to ship the Makera Z1 tool list (types, dimensions and tool numbers) from `CarveraProfiles` as the default tool library. The repository has no licence file. Could you add one, or confirm in writing that the tool files may be copied and redistributed under the AGPL-3.0, with attribution if you wish?"
3. **Standard tools (OI-268).** 23 tools, all metric, defined for this project: nominal dimensions of a generic tool of each type, not taken from any vendor catalogue. Flat 1, 2, 3, 3.175, 4, 6 mm; ball 1, 2, 3, 3.175, 6 mm; bull-nose 3 mm r0.5 and 6 mm r1; tapered ball r0.25 on a 3.175 mm tool at 5 and 10 degree half angle; V-bit 30, 60 and 90 degree (6 mm, 0.1 mm tip); 90 degree chamfer (6 mm, 1 mm tip); drill 1, 2, 3 and 5 mm at 118 degrees. Shank equals diameter from 3 mm up, 3.175 mm below that; two flutes except the V-bits (one). The tool's own material is left unset: no source. The list awaits the owner's approval.
4. **Cutting data (OI-270).** None ships: nothing has a source, and invented speeds and feeds would break NFR-12. The tool files carry no `[cutting_data]`; a test checks it.
5. **Delivery.** The files are in `crates/openchuck-project/defaults/tools/` and embedded with `include_str!` (NFR-09, no network). `ToolLibrary::install_defaults` writes them into the user's library on first run in the existing file format (no schema change, no schema number claimed); `openchuck_app::library::install_default_tools_in_default_folder` runs it, and the UI calls it at start-up, reporting a failure without stopping. The command line does not install.
6. **Update policy (OI-271), built as the safe minimum.** A ledger file `.installed-defaults` in the library folder lists the IDs of the defaults installed. A listed ID is never installed again (a deleted tool stays deleted); an installed tool is never rewritten (an edited tool stays as edited); an ID the ledger does not know is installed (a later release adds a default); a default already in the library, for example by import, is recorded and not rewritten; a damaged ledger can only cause a missing default to be reinstalled, never an overwrite. Not built, for the owner: silent update of an unedited default to a new version (needs a content hash in the ledger), and an explicit "restore default tools" action. The ledger does not end in `.toml`, so the library does not read it as a tool.
7. **Room for the Z1 set.** `EMBEDDED` is a list of files and the ledger holds only IDs, so the Z1 tools and a `tool-sets/makera-z1.toml` can be added later without a format change. A tool set file is not ledgered yet; that is decided with the Z1 part.

## Alternatives considered

- **Copying the Makera files now.** Rejected: no licence (item 1).
- **Marking defaults with a flag inside the tool file.** Would change the tool file format and need a schema number; a ledger beside the library does not.
- **Reinstalling a deleted default on update.** Rejected by TL-12.

## Verification

Tests named after the requirement IDs in `crates/openchuck-project/tests/default_tools.rs` and `crates/openchuck-app/tests/library.rs`: every default validates, has a unique ID and no cutting data; every type in TL-02 and TL-03 is present; each shape equals its closed-form silhouette at 301 radii within 1e-9 mm; a first run installs 23 and a second installs none; an edited default is kept; a deleted default is not restored; a new default is installed; a damaged ledger overwrites nothing; installing needs no source tree or network.

## Consequences

- TL-12 is only partly met: the Z1 reference tools and tool set are missing until OI-269 is settled. Slice 23 stays open in the work breakdown.
- No dependency, no schema number, no change to `requirements.md` or `architecture.md`. `openchuck-project` gains a public API (`InstallReport`, `LEDGER_FILE`, `default_tools`, `ToolLibrary::install_defaults`), a minor addition.

## Open items

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | OI-268: approve the standard-tool list in item 3. | Owner | |
| 2 | OI-269: choose the route in item 1 for the Makera data. | Owner | |
| 3 | OI-270: cutting data ships with a recorded source, or not at all (item 4). | Owner | |
| 4 | OI-271: silent update of unedited defaults, and a restore action (item 6). | Owner | |
