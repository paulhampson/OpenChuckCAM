# Minimal post: the example post-processing template

The smallest useful post-processor folder, for the user guide's worked example "Create a post-processing template". `post.toml` names the post and sets its options; `minimal.post` is the template. It writes the LinuxCNC dialect, with every axis word on every move, and keeps the safety notice on.

To use it, copy this folder, change it, and choose it in the export as **A post folder**, or pass the folder to `openchuck-cli regenerate --post`. The guide's page walks through it line by line.

It is an example for learning the format, not a post set up for any machine. A program written by it, as by any post, must be verified by you before it is run on a machine.

Written for this repository by lane DOCSUSER-2, under AGPL-3.0-or-later like the rest of it. `crates/openchuck-post/tests/posts.rs` loads and posts it, so the example keeps working with the program.
