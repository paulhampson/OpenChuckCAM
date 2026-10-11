# openchuck-import

Model and curve readers behind a `ModelImporter` trait: STL, STEP (the C++ kernel is wrapped here and nowhere else).

Part of [OpenChuckCAM](../../README.md). Architecture section 5.4; requirements IMP-01 to IMP-03, IMP-06, IMP-08, NFR-13.

**Status:** slice 2. The `ModelImporter` trait and the STL importer (ASCII and binary, through `stl_io`), with units read from the file or set by the user and converted to millimetres here (IMP-01, IMP-03). Decision record 0010. STEP arrives with the OpenCASCADE wrapper in slice 11 (decision record 0003); no STEP kernel is linked yet.



Licence: AGPL-3.0-or-later.
