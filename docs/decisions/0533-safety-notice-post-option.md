# 0533: The NFR-12 safety notice as a post option, on in the built-in posts (lane SAFETYNOTICE; OI-2000)

- **Status:** accepted by the project owner on 7 Nov 2026, relayed by the coordinator (decision 0049). Proposed 6 Nov 2026. The two choices in it are the owner's, relayed by the coordinator (session_01CFw3bzew7zizFQ91L5tvT7) on 6 Nov 2026 (decision 0049): the built-in posts ship with the notice on (decision 1 of 2), and the two-line wording of item 3 (decision 2 of 2). At acceptance the owner also closed OI-2082: no per-export switch is wanted, the post option is the switch.
- **Date:** 6 Nov 2026.
- **Lane:** SAFETYNOTICE (wave 2, release blockers), branch `claude/lane-safetynotice`, from `main` at `c5aec1d` (library family 0.46.0), rebased on `main` `3c3b3f57` (0.47.0).
- **Requirements:** NFR-12 (P1), GC-03, GC-04, GC-07, NFR-06, NFR-07, GC-09, GC-10. Work breakdown slice 20 item 1 and its review box; verification plan NFR-12 row.
- **Builds on:** [0019](0019-sp-10-template-engine.md) (template engine, comment cleaning), [0020](0020-slice-6-post-and-round-trip.md) (the built-in posts), [0028](0028-slice-19-output-options-and-setup-sheet.md) (the setup sheet already carries the NFR-12 notice), [0048](0048-pre-1-0-api-changes.md) (breaking change before the first release), [0103](0103-built-in-posts-live-in-the-post-crate.md) (built-in posts inside the post crate).
- **Open items:** OI-2000 closed by this change. OI-2082 (no per-export switch for the notice) raised and closed by the owner on 7 Nov 2026: not needed.
- **Numbers:** record 0533 and OI-2082 to OI-2085 assigned by the coordinator; only OI-2082 is used. No project schema number: nothing is stored in the project. Library version 0.48.0: `main` held 0.47.0 (TABCUT #562) and no unmerged branch held 0.48.0 on 7 Nov 2026; the Merge Manager may reassign it at release.

## Context

Slice 20 item 1 asks for the NFR-12 notice ("generated G-code must be verified before use") at export in the UI, in the CLI output and "as an optional post header comment". The UI export panel and the CLI already carry it (GC-16, decisions 0038, 0060, 0071). The built-in posts carried the wording only as a TOML comment in `post.toml`, which never reaches the program (OI-2000). The program file is the one thing that reaches the machine operator, who may not be the person who exported it.

## Decision

1. **A post option, `safety_notice`** (a boolean in `[options]` of `post.toml`, GC-04). On, the template writes the notice as comment lines right after the title and generator lines (`(OpenChuckCAM <version>)`), before the revision and the header codes. The key is optional and is read as `false` when a post leaves it out, so a post written before this change loads and writes exactly what it wrote before (GC-03). It lives in the post definition, not the project, so there is no project schema change, and the post schema stays 1.
2. **On in both built-in posts** (the owner's decision 1, relayed): the LinuxCNC verification post and the Makera Z1 post set `safety_notice = true`. A user who does not want it copies the post and sets it to `false`.
3. **Fixed wording** (the owner's decision 2, relayed), the constant `openchuck_post::SAFETY_NOTICE`, written as these two comment lines:

   ```text
   (This program has not been run on your machine. Check it before you run it.)
   (Simulate it and air cut it first. You are responsible for what the machine does.)
   ```

   The text is in the post crate, not in `post.toml`, so no post can reword it into a claim that the program is safe or checked (NFR-12). It is plain printable ASCII with no parentheses, each line under 80 characters, so both controllers read it as a comment. A test checks that it says nothing about the program being safe, verified, tested or guaranteed.
4. **Never dropped silently** (NFR-07). The notice is a comment, so a post with `safety_notice = true` and `comments = false` is refused when it is loaded, with a message naming `options.safety_notice`. A copied template that does not write the notice while the option is on is refused when it posts, with a message saying so, so an old template cannot quietly lose it.
5. **Deterministic output** (GC-07, NFR-06). The text has no date, clock or commit; the version is already on the generator line. The same project, post and version give byte-identical programs at 1, 2 and N threads, as before.
6. **The setup sheet** is unchanged: its Notes section already carries the NFR-12 notice (decision 0028 item 4).
7. **Library change (breaking).** `PostOptions` gains a public field `safety_notice`, and `SAFETY_NOTICE` is new. Struct literals of `PostOptions` add the field, so the library family goes to 0.48.0 with changelog entries in the four library crates (decision 0048).
8. **Golden files.** Every golden program made with a built-in post gains the two notice lines and nothing else; they are re-blessed in one commit, listed in the pull request.

## Alternatives considered

- **An export or CLI switch stored with the project.** Would need a project schema number and UI work for a setting the post already expresses; not wanted: the owner closed OI-2082 on 7 Nov 2026, the post option is the switch.
- **Wording in `post.toml`.** Lets a post say anything, including that a program is safe; refused for NFR-12.
- **Off by default.** No golden file would move, but a notice the user has to find and switch on does little (the owner chose on).
- **One line, closer to the GC-16 sentence** ("Generated G-code: verify it before you run it on a machine."). Says that the user must, not what to do; not chosen.

## Verification

Tests in `crates/openchuck-post/tests/posts.rs`: `req_nfr_12_both_posts_write_the_safety_notice_after_the_generator`, `req_nfr_12_req_nfr_07_the_notice_is_comment_safe_and_claims_nothing`, `req_nfr_12_req_gc_04_notice_off_writes_nothing_else_changes`, `req_gc_04_req_gc_03_a_post_without_the_key_loads_with_the_notice_off`, `req_nfr_12_req_nfr_07_notice_with_comments_off_is_refused`, `req_nfr_12_req_nfr_07_a_template_that_drops_the_notice_is_refused` and `req_nfr_12_req_gc_07_notice_output_is_byte_identical_on_1_2_and_n_threads`, plus the existing golden, reader, gate 1 and gate 2 tests on the re-blessed programs. Gate 3 (the LinuxCNC replay) runs in CI on the re-blessed LinuxCNC programs; parenthesised comment lines are already accepted there.

Nothing in this change says that a program is safe to run: the notice says the opposite, and generated G-code remains the user's to verify (NFR-12).

## Open items

None open. OI-2082 (a per-export switch in the UI or CLI) was closed by the owner on 7 Nov 2026: not needed, the post option is the switch.
