# Trial feedback example projects

The owner's example projects from the nightly trial (build `50fd04ed0`, Windows), committed so the lane sessions can fetch them: cloud sessions cannot download GitHub issue attachments. They are the owner's own files, shared at the owner's instruction on 20 Oct 2026. Not part of the product; do not build tests that depend on them without copying what a test needs into `testdata/`.

## Layout

Each project folder refers to the model as `../blisk.stl`, so the projects sit beside the shared `blisk.stl`:

| Path | What it is |
| --- | --- |
| `blisk.stl` | The blisk mesh (1.2 MB), shared by the three projects. SHA-256 `48351f42635f05eea09aaf77dd685e7b2bd564cd087b4a336fb29554a7e33f37`. **It differs from `testdata/stl/blisk.stl`** (SHA-256 `190ab4b4...`), so do not assume they are the same model. |
| `blisk.step` | The blisk as a STEP file, for swarf testing (#206, #226, #227) and the STEP issues (#220, #221). Written by Open CASCADE 7.9 on 19 Oct 2026. 257 KB. |
| `blisk_1_overcut_mismatch/` | From the #217 attachment `blisk_1_overcut_mismatch.zip`. Three operations: Roughing (rotary roughing, plunge entry, stock to leave 0.5), Helical Finishing (ramp entry) and Operation 3 (pencil finishing with the 2 mm tool). Stock material Softwood. |
| `blisk_bad_roughing/` | From `blisk_bad_roughing.zip` (the owner did not say which issue it belongs to; the name suggests #202, the path in the held region, or #199, the roughing ramp error). One operation: Roughing (plunge entry, stepover 0.5, stock to leave 0.5). |
| `blisk_slow_toothpath_rendering/` | From the #200 attachment `blisk_slow_toothpath_rendering.zip`. Identical to `blisk_bad_roughing` except that the roughing stepover is 2.0. |

The issue each project belongs to is inferred from the attachment names and the issue comments; confirm with the owner where it matters.

## What was checked in the files (20 Oct 2026)

- The project TOML files and the STEP file contain no user names, home-directory paths, e-mail addresses or OneDrive paths (searched for `users`, `paul`, `onedrive`, `appdata`, `@` and drive letters). The project refers to the model by the relative path `../blisk.stl` and a BLAKE3 hash.
- All three projects use schema 13, the Makera Z1 machine, a box stock (X 0 to 60, Y and Z about +/-21 and +/-20 mm) and a model translation of 9.175 mm in X.

## Observations for the lanes (verified in the files, not interpreted)

- **Held region (#202, #187, #188).** The setup has one held region, `workholding.start`, 5 mm long with a 40 mm holding diameter (the same as the stock's Z extent), and two retaining tabs (a 3.175 mm long cylinder of 10 mm diameter from X 6.0, and a 0.825 mm long cylinder of 14.5 mm diameter from X 59.175). There is **no `workholding.end`**. The owner said the held regions were at both ends and the path goes through all of it; the tail tab may be what the owner saw at the end. Confirm with the owner, and check it against the simulated stock.
- **Offset (#175).** The model carries a translation of 9.175 mm in X, and the setup carries `part_offset = 0.0`. These are the two places the owner says overlap.
- **Roughing (#199).** The roughing operation in these projects uses plunge entry. The ramp error the owner reported (Z1 O-flute metal 3.175 x 12, ramp at 3 degrees) is not saved in these projects: the owner can supply the version with the ramp.

## Not checked

The projects have not been loaded or run by this session. The overcut figures, the path in the held region and the rendering speed are the lanes' to reproduce.
