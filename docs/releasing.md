# Releasing

Two kinds of release, each started by a tag: the library family on crates.io (`lib-v<version>`) and the application (`app-v<version>`), whose release also publishes that version's user guide. The release workflow is `.github/workflows/release.yml` ([decision 0091](decisions/0091-release-workflow-and-gate.md)).

- [The library family on crates.io](#the-library-family-on-cratesio)
- [Each application release](#each-application-release)
- [What the application archives hold](#what-the-application-archives-hold)
- [Protect the release tags](#protect-the-release-tags)
- [One-time setup for the documentation site](#one-time-setup-for-the-documentation-site)
- [Up-to-date rule relaxed](#up-to-date-rule-relaxed)

## The library family on crates.io

What the project owner does to publish `openchuck-types`, `openchuck-gcode`, `openchuck-path` and `openchuck-post` to crates.io ([decision 0138](decisions/0138-library-family-publish-step.md), OI-303). The release workflow (`.github/workflows/release.yml`, [decision 0091](decisions/0091-release-workflow-and-gate.md)) does the publishing, and only when the owner asks for it. A tag push alone never publishes a crate.

Publishing cannot be undone. A published version can be yanked (new projects stop resolving it), but it cannot be deleted or replaced, and its source stays downloadable.

### What is published, and what is not

- **Published:** the library family only, the four crates above, all at the workspace version (lockstep, decision 0031 item 2). Each package holds `src/`, `README.md` and `CHANGELOG.md`, plus `examples/` (`openchuck-types`, `openchuck-path`) and `posts/` (`openchuck-post`: the built-in posts, [decision 0103](decisions/0103-built-in-posts-live-in-the-post-crate.md)). The tests stay out: they read the repository's `testdata/` and the test-only `openchuck-testkit`.
- **Never published:** `openchuck-testkit`, `openchuck-buildinfo`, `openchuck-cli`, `openchuck-ui`, `xtask` and `openchuck-benches` say `publish = false`. The library family does not depend on any of them, nor on the application crates: `cargo xtask deps` and `cargo xtask publish` check this on every pull request. The other application crates (`openchuck-geom`, `-import`, `-sim`, `-project`, `-app`, `-viewport`) are not published by the workflow either.
- **Names:** the owner reserved the four library names on 11 Oct 2026 with `0.0.0` placeholder versions (and `openchuck-geom`, `openchuck-import`, `openchuck` and `openchuckcam` the same day). The first real publish becomes each crate's latest version; the placeholder stays in the version list.

### Once: set up the credential

1. Sign in to [crates.io](https://crates.io) with the GitHub account that owns the crates (`paulhampson`).
2. **Account Settings, API Tokens, New Token.** Name it for this repository (for example `OpenChuckCAM release workflow`), give it an expiry, tick the scope **publish-update** only, and restrict it to the crate pattern `openchuck-*`. Copy the token: crates.io shows it once.
3. In the GitHub repository: **Settings, Environments, `crates-io`, Environment secrets, Add secret**, named `CARGO_REGISTRY_TOKEN`, holding the token, so only the publish job can read it. (A repository secret of the same name also works.)
4. **Settings, Environments, `crates-io`** (create it, or the first run creates it), add yourself as a **required reviewer**. The publish job then waits for your approval before it runs (the owner's decision, 20 Oct 2026).

Renew the token before it expires and replace the secret. Revoke it on crates.io if it may have leaked.

### Each library release

1. On `main`, with the release work merged: the workspace version in `Cargo.toml` is the version to publish, and each crate's `CHANGELOG.md` has its entry. (`cargo-release` may bump and tag locally, without publishing.)
2. Push the tag `lib-v<version>`, for example `lib-v0.11.0`. The tag's version must be the workspace version, or the release gate fails. The tag push runs the gate and the publish dry run; it publishes nothing.
3. **Actions, Release, Run workflow:** choose the tag `lib-v<version>` (not a branch), **untick** `dry_run`, **tick** `publish_crates`. A run on a branch, with `dry_run` ticked or on an `app-v` tag, stops before the gate and publishes nothing.
4. The run passes the release gate and the publish dry run, waits for your approval if `crates-io` has a reviewer, and then publishes `openchuck-types`, `openchuck-gcode`, `openchuck-path` and `openchuck-post` in that order (the last three depend on `openchuck-types` and nothing else in the family). Cargo waits for each crate to reach the index before the next.
5. If the run stops part way (a network failure, an expired token), fix the cause and run it again the same way: a crate already on crates.io at that version is skipped.
6. Check the crates' pages on crates.io and their documentation on docs.rs.

Generated G-code is the user's to verify before running it on a machine (NFR-12). Publishing a crate does not change that, and nothing in a crate's page or README may say otherwise.

## Each application release

**The release notes come first.** The section for the new version on `user-docs/src/release-notes.md` (a `## <version>` heading, newest first, above *Development version*) is written in a pull request to `main` and merged before the tag is pushed. A lane drafts the release notes for each application release by default (the owner's choice, 1 Nov 2026), from the crate changelogs, the merged pull requests and the closed issues since the previous release. The draft is in plain language for users, grouped by what changed, names breaking changes and new settings, and never implies that generated G-code is verified (NFR-12). The owner reviews and edits the draft before the release is published; there is no placeholder section, because the check below exists to stop a release whose notes were never written. A dry run before that pull request merges reports the section as missing, which is expected.

Before pushing the tag `app-v<version>`, the project owner ticks these items and records them in the release's verification report (verification plan section 9, Records). The release workflow checks the items marked *(checked)* and refuses to publish if they fail ([decision 0400](decisions/0400-user-documentation-site.md), section R).

- [ ] An active tag ruleset protects `lib-v*` and `app-v*` ([Protect the release tags](#protect-the-release-tags), OI-305). *(reported: the run's plan job warns when it cannot see one, or when the run was not started by the repository owner; it never fails the run)*
- [ ] The unsigned first run works as the README says, on a downloaded copy of the dry run's archives (decision 0541, OI-2107): on a Mac, open `OpenChuckCAM.app` from the macOS archive as downloaded and follow the README's steps (the "Not Opened" dialog, then **Open Anyway**, or the `xattr` command); on Windows, start `openchuck-ui.exe` from the zip as downloaded and check the SmartScreen steps. Record what macOS and Windows showed in the verification report.
- [ ] The Makera Z1 air cut has been run, or the release notes say which Z1 values are unconfirmed ([decision 0106](decisions/0106-air-cut-is-a-manual-release-step.md)).
- [ ] The tutorial screenshots match this release: re-captured with `user-docs/tools/capture.py` at the release commit, or checked against the release build and marked reviewed in `user-docs/src/tutorial/images/CAPTURED.toml`. *(checked: the capture or review commit is not older than the last change to the screens)*
- [ ] Every setting, operation, strategy, machine-profile field and command-line command added or changed since the previous release is described in the reference pages of the user guide.
- [ ] The release-notes page (`user-docs/src/release-notes.md`) has a section for this version: what changed, the known limitations, the unconfirmed Z1 values and the notice that generated G-code must be verified before it is run on a machine. *(checked: a section headed with the version exists)*
- [ ] The safety pages of the user guide still agree with `docs/z1-air-cut-procedure.md` and `docs/z1-safe-practice-checklist.md`.
- [ ] The user guide builds with no broken link or anchor, valid HTML and no request to another site. *(checked)*
- [ ] The footer of every page of the guide names this version and tag. *(checked)*
- [ ] The OpenCASCADE notice and source offer are in each archive ([decision 0003](decisions/0003-step-kernel-route.md), NFR-10, OI-29). *(checked: each archive holds `openchuck-step-worker`, `OPENCASCADE-NOTICE.txt`, `LICENSE_LGPL_21.txt` and `OCCT_LGPL_EXCEPTION.txt`, and the notice names `occt-sys` and the OCCT version in `Cargo.lock`)* In the first release run (OI-304), also check by hand, on the dry run's archives for the three platforms: open `OPENCASCADE-NOTICE.txt` and read it through; confirm that the OCCT version matches `occt-sys` in `Cargo.lock`, that the crates.io address and the build command in the source offer still work, and that the "Source:" line of the archive's `README.txt` names the release commit; confirm that `openchuck-step-worker` sits beside `openchuck-ui` and that a STEP file imports. The notice says what a user can do (rebuild the worker against their own OpenCASCADE and replace the file); it gives no legal conclusion, and the legal review of its wording is the owner's item OI-23. Record the result in the verification report. When the version of `occt-sys` changes, update the version in `LICENSES/OpenCASCADE-NOTICE.txt` and the path in `build-archives.yml`.

Then push the tag. The run builds the archives and the user guide, checks both, publishes the release, and then adds the guide to the `docs-releases` branch as `v<version>/` (job `publish-docs`), which redeploys the site with the new version in its version list. A dry run (`workflow_dispatch` with `dry_run` ticked) does all of this except publishing. It runs every check of the guide, the release checks included, and reports their failures without stopping the run; a real release refuses to publish on any of them.

### If the documentation did not publish

The program release is published before its guide, so a failure in `publish-docs` leaves the release published but its guide missing from the site. The job says why. Fix the cause (usually the one-time setup below), then open the failed run and use **Re-run failed jobs**. The job adds the guide only if `v<version>/` is not on `docs-releases` yet; it never rebuilds or replaces a version that is there.

### Withdrawing a released version

A released version of the guide is never changed. If one holds an error that matters for safety, the owner, and only the owner:

1. records the reason in a decision record;
2. under the ruleset's bypass, replaces the files of `v<version>/` on `docs-releases` with a single page saying why the version was withdrawn and linking to the newest release, and rewrites its `MANIFEST.sha256` with `python3 user-docs/tools/manifest.py write v<version>`;
3. adds a line to that release's notes giving the new manifest's SHA-256.

The next deploy publishes the change. Nothing else ever writes that branch.

## What the application archives hold

[Decision 0541](decisions/0541-macos-app-bundle-linux-glibc-floor-and-release-tags.md) (OI-300, OI-301, OI-135). `build-archives.yml` builds one archive per platform for the nightly build and the release alike.

| Archive | Built | Holds | Runs on |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu.tar.gz` | in an `ubuntu:22.04` container, pinned by digest, on `ubuntu-latest` | `openchuck-ui`, `openchuck-cli`, `openchuck-step-worker` | glibc 2.35 or newer with the libstdc++ of GCC 12 or newer: Ubuntu 22.04, Debian 12, Fedora 36 or later (x86-64) |
| `x86_64-pc-windows-msvc.zip` | on `windows-latest` | `openchuck-ui.exe`, `openchuck-cli.exe`, `openchuck-step-worker.exe` | Windows x64 |
| `aarch64-apple-darwin.tar.gz` | on `macos-latest` | `OpenChuckCAM.app` (with `openchuck-ui` and its own `openchuck-step-worker` in `Contents/MacOS`), `openchuck-cli`, `openchuck-step-worker` beside it | macOS 11 or later on Apple silicon |

Each also holds `README.txt`, the licences and notices, and `TOOL-DATA-CREDITS.txt` (the credit for the Makera Z1 tool data, OI-269).

**Not signed.** The programs and the `.app` are not signed with a certificate and not notarised: signing is out of scope for 2026.1-alpha (the owner's decision of 7 Nov 2026, OI-135). What a user sees, and what the README says to do:

- **macOS** (15 and later): opening the app shows "OpenChuckCAM" Not Opened ("Apple could not verify..."), with **Done** and **Move to Trash**. The user clicks **Done**, then **System Settings, Privacy & Security**, **Open Anyway** under Security, and confirms. Or, in Terminal: `xattr -dr com.apple.quarantine OpenChuckCAM.app openchuck-cli openchuck-step-worker`. The quarantine flag comes from the browser download; a copy made with `curl` has none.
- **Windows**: SmartScreen may show "Windows protected your PC"; **More info**, then **Run anyway**. Or unblock the zip first (**Properties, Unblock**).
- **Linux**: nothing; the archive has no signature to check.

**The glibc floor.** A Linux program needs at least the glibc it was linked against; built on `ubuntu-latest` (now 26.04) the nightly `openchuck-ui` needed `GLIBC_2.43` on 7 Nov 2026, which only the newest distributions have. The Linux leg therefore builds in Ubuntu 22.04, and `cargo xtask glibc-floor` reads each binary's version needs and fails the build if one needs a newer `GLIBC_`, `GLIBCXX_`, `CXXABI_` or `GCC_` version than the floor. The floor is stated once, as `GLIBC_FLOOR` and `LINUX_FLOOR_TEXT` in `build-archives.yml`; the archive README takes its wording from there. The nightly UI smoke test (`ui-smoke.yml`) also launches the Linux archive in the same Ubuntu 22.04 image, which shows that the floor holds in practice. The OpenCASCADE install of the container build is cached under its own variant (`ubuntu2204`), so it is never mixed with the runner image's (decisions 0039, 0251).

To move the floor: change the image and its digest in `build-archives.yml` and `ui-smoke.yml`, the variant name, `GLIBC_FLOOR` and `LINUX_FLOOR_TEXT`, the Linux row above, the user guide's install page, and record why in a decision record. The first build on a new variant builds OpenCASCADE once (about 21 minutes).

**What a dry run proves, and what it does not.** A dry run (Actions, Release, Run workflow, `dry_run` ticked) runs the same build: it proves the container build, the glibc check, the STEP reader's tests on the shipped Linux build, the bundle, `Check the archives` (the bundle's files, the version in its `Info.plist`, the floor in the Linux README, the tool data credit), and whether the plan job can read the rulesets. Only a real `app-v` run proves the published release with these archives and the tag check on a real tag push; the first-run dialogs on a downloaded copy are checked by hand (the checklist above).

## Protect the release tags

What the owner does once, so that only the maintainer can create, move or delete the tags that start a publishing release run (`lib-v*`, `app-v*`; OI-305). The release workflow cannot change repository settings. It only reports, in its first job: a warning when no active tag ruleset covers `lib-v*` and `app-v*`, or when the run was not started by the repository owner. The warning never fails the run, so a token that cannot read the rulesets does not block the first release; it can become a failure once a run has shown that the check works (decision 0541).

1. In the repository, **Settings**; under "Code, planning, and automation" in the left sidebar, **Rules**, then **Rulesets**.
2. **New ruleset**, then **New tag ruleset**.
3. **Ruleset name:** `Release tags are created by the maintainer only`. **Enforcement status:** **Active**.
4. Under **Bypass list**, click **Add bypass**, choose the role **Repository admin**, and click **Add selected**; leave it as **Always allow**. Add nobody else, and not the docs release App: it writes the `docs-releases` branch, never a tag.
5. Under **Target tags**, click **Add target**, **Include by pattern**, and enter `lib-v*`; then again for `app-v*`.
6. Under **Tag rules** (*unchecked*: GitHub's documentation may call this section "Rules" or "Tag protections"), tick **Restrict creations**, **Restrict updates**, **Restrict deletions** and **Block force pushes**.
7. Click **Create**.

To check it: run the Release workflow by hand on `main` with `dry_run` ticked. Its summary should say "An active tag ruleset covers lib-v*" and the same for `app-v*`. If it warns that it could not read the rulesets, check the ruleset by hand in **Settings, Rules, Rulesets** before each release.

## One-time setup for the documentation site

What the repository owner does once, so that the user guide is published to GitHub Pages and each application release can add its own copy ([decision 0400](decisions/0400-user-documentation-site.md)). These are owner actions: nothing in the repository does them. Until they are done, the guide is still built and checked; the nightly deploy skips with a notice (step 6), and a release's `publish-docs` job fails with a message naming what is missing (steps 1 to 5), after the program release has been published.

The menu names below were checked on 31 Oct 2026 against the source of GitHub's documentation (`github/docs`). Where GitHub's screens may differ, the step says so (*unchecked*).

**Two environments, two different jobs.**

| Environment | Used by | May run from | Can do |
| --- | --- | --- | --- |
| `github-pages` | `docs-site.yml`, job "Publish to GitHub Pages" | `main` only (a run after a Nightly build or Release run, or by hand, runs as `main`) | Deploy the Pages site. It only reads `docs-releases`. |
| `docs-release` | `release.yml`, job `publish-docs` | `app-v*` tags only | Write `docs-releases`, with the App below. It holds the App's client ID and private key. |

So limiting `docs-release` to release tags does not stop the nightly "latest" from being published: that is the other environment.

### 1. Create the GitHub App that writes `docs-releases`

1. Click your profile picture (top right), then **Settings**.
2. In the left sidebar, click **Developer settings**, then **GitHub Apps**, then **New GitHub App**.
3. **GitHub App name:** `OpenChuckCAM docs release` (it must be unique on GitHub and at most 34 characters; add your name if it is taken).
4. **Description:** "Writes the released user guide to the docs-releases branch of paulhampson/OpenChuckCAM. Used only by the release workflow."
5. **Homepage URL:** `https://github.com/paulhampson/OpenChuckCAM`.
6. Leave **Callback URL** and **Setup URL** empty; leave **Request user authorization (OAuth) during installation** and **Enable Device Flow** unticked.
7. Under **Webhook**, untick **Active**. The App receives no events.
8. Under **Permissions**, **Repository permissions**: set **Contents** to **Read & write**. Leave everything else at **No access** (GitHub adds **Metadata: Read-only** itself).
9. Under **Where can this GitHub App be installed?**, choose **Only on this account**.
10. Click **Create GitHub App**.
11. On the App's settings page, note the **Client ID** (shown in the "About" section near the top; *unchecked*: the exact place on the page).
12. Generate a private key: in the left sidebar click **Credentials**, then **New key**; or, on the older page layout, under **Private keys** click **Generate a private key**. Your browser downloads a `.pem` file. GitHub keeps only the public half.

### 2. Install it on this repository only

1. Back in **Developer settings, GitHub Apps**, click **Edit** next to the App, then **Install App**.
2. Click **Install** next to your account.
3. Choose **Only select repositories**, select `OpenChuckCAM`, and click **Install**.

### 3. Create the `docs-release` environment

1. In the repository, **Settings**, then **Environments** in the left sidebar, then **New environment**. Name it `docs-release` and click **Configure environment**.
2. Optionally, tick **Required reviewers** and add yourself: the job then waits for your approval at each release.
3. Under **Deployment branches and tags**, choose **Selected branches and tags**, click **Add deployment branch or tag rule**, choose the type **Tag**, and enter `app-v*`. Add no branch rule.
4. Under **Environment variables**, click **Add Variable**: name `DOCS_RELEASE_APP_CLIENT_ID`, value the App's client ID (step 1.11).
5. Under **Environment secrets**, click **Add Secret**: name `DOCS_RELEASE_APP_PRIVATE_KEY`, value the whole content of the `.pem` file, including the `-----BEGIN RSA PRIVATE KEY-----` and `-----END RSA PRIVATE KEY-----` lines.
6. Delete the downloaded `.pem` file from your computer. If the key is ever lost or may have leaked, generate a new one on the App's page, replace the secret, and delete the old key there.

### 4. Make the App the only writer of `docs-releases`

1. In the repository, **Settings**; under "Code, planning, and automation" in the left sidebar, **Rules**, then **Rulesets**.
2. **New ruleset**, then **New branch ruleset**.
3. **Ruleset name:** `docs-releases is written by the release workflow only`. **Enforcement status:** **Active**.
4. Under **Bypass list**, click **Add bypass**, search for the App (`OpenChuckCAM docs release`), select it and click **Add selected**. Leave it as **Always allow**. Add nobody else.
5. Under **Target branches**, click **Add target**, choose **Include by pattern**, and enter `docs-releases`. The branch does not have to exist yet: the first application release creates it.
6. Under **Branch rules** (*unchecked*: GitHub's documentation calls this section "Branch protections"), tick **Restrict creations**, **Restrict updates**, **Restrict deletions**, **Require linear history** and **Block force pushes**. Do not tick **Require a pull request before merging**: the branch is never merged.
7. Click **Create**.

GitHub's documentation lists deploy keys as bypass actors only on GitHub Enterprise Server, which is why the writer is an App and not a deploy key.

### 5. Check the `github-pages` environment

GitHub creates `github-pages` when Pages is switched on (step 6). In **Settings, Environments, github-pages**, check that **Deployment branches and tags** allows `main` only (*unchecked*: GitHub's default for this environment). Add no secrets: the deploy job needs none.

### 6. Switch on GitHub Pages

**Settings**, then **Pages** (under "Code, planning, and automation"). Under **Build and deployment**, **Source**, choose **GitHub Actions**. No custom domain.

The guide appears after the next Nightly build that publishes the `nightly` pre-release, or at once if you run the **Docs site** workflow by hand (**Actions, Docs site, Run workflow**, on `main`). Its address is `https://paulhampson.github.io/OpenChuckCAM/`.

### 7. Check it

1. Run the **Docs site** workflow by hand on `main`. It should build, check and deploy, and the page should show the nightly build's date and commit in its footer.
2. Run the **Release** workflow by hand on `main` with **dry_run** ticked. The job "User guide for the release" should build and check the guide (on a dry run, problems are reported and the run goes on); `publish-docs` does not run.
3. Try to push to `docs-releases` yourself, for example `git push origin HEAD:docs-releases` from any clone. GitHub must refuse it, naming the ruleset. If it is accepted, the ruleset is not working: check that it is **Active** and that only the App is in its bypass list, then remove the branch you created (set the ruleset to **Disabled** for that, and back to **Active** after).

The first application release then creates `docs-releases` with its own folder, and the site lists the version.

## Up-to-date rule relaxed

How pull requests merge ([decision 0499](decisions/0499-up-to-date-rule-relaxed.md), accepted; amends [0055](decisions/0055-push-discipline-and-draft-pull-requests.md) item 4; it supersedes in part [0498](decisions/0498-merge-queue.md), accepted). GitHub's merge queue is not available on a personal-account repository, so it is not used. With "Require branches to be up to date" on, every merge turned the other ready pull requests behind, and each had to merge `main`, push and wait for a full CI run.

**What the owner changes** (Settings, Rules, Rulesets, "Main branch protection"): clear **Require branches to be up to date before merging**. Keep **Require status checks** with `CI required checks`; change nothing else.

**How merges are serialised.** The Merge Manager merges one pull request at a time, in the order it sets. A pull request that is behind `main` merges without being updated or re-run, once `CI required checks` is green on its own head and it has a CodeRabbit review of that head (a process rule; GitHub does not enforce it). A lane merges `main` only for a real conflict or a register check (`cargo xtask records`).

**CI on `main`.** Each merge triggers the push run of `ci.yml`. The Merge Manager looks at it before the next merge, because a pull request merged while behind has not been tested with the ones merged after its last run. The push run is normally a full run: the light run (decision 0358, Option 4) applies only when the merged tree equals a pull request's tested tree (OI-1999).

**If `main` goes red:** the Merge Manager stops merging, finds the pull request that broke it, and has its lane push a fix pull request or has the owner revert it. Merging resumes when `main` is green.

**Merge queue, for a later organisation-owned repository.** `ci.yml` and `cla.yml` already have `merge_group` triggers (decision 0498): a queue run is a ready-pull-request run with the path filters taken against the group's base sha, the fuzz smoke, the user guide check and `cargo semver-checks` included, and `CLA signatures` passing at once. If the repository moves to an organisation, enabling the queue is a ruleset change (**Require merge queue**; squash; build concurrency 1; group size 1; ALLGREEN; a 60-minute timeout), `CodeRabbit` would have to come off the required list because it does not report on a queue commit (not verified), and the first-run checks of OI-2001 apply. Nothing about the queue has been run.
