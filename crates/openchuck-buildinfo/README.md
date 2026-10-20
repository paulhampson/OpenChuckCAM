# openchuck-buildinfo

Which build is running (NFR-16, decision 0151): the Git commit, whether the build tree had
uncommitted changes, the build date, the Rust compiler and target, and the version of each
workspace crate. A build script gathers the facts with `git` and `rustc` (no network); a fact it
cannot find reads `unknown`. Only `openchuck-cli` and `openchuck-ui` depend on this crate, so
nothing in the library family, the G-code or a project file carries build data (GC-07, NFR-06).
