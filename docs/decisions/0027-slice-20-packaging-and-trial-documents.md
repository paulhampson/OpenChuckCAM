# 0027: Slice 20 preparation: packaging route, release tags, and the trial documents

- **Status:** accepted by the project owner (14 Oct 2026): the packaging route (item 1), the tag schemes (item 2) and the drafting of the trial documents (item 3) as written. Architecture section 12 is edited to match. The Z1 operator (the owner) reviewed and accepted the air-cut procedure and the safe-practice checklist without changes on 14 Oct 2026 (OI-120, closed), and signing and extra platforms stay the owner's later call (OI-122).
- **Date:** 14 Oct 2026.
- **Work breakdown:** slice 20 items 8, 10 and 12 (preparation only; the slice is not started).
- **Open items** are tracked in [open-items.md](../open-items.md): OI-120 to OI-122 are new, and OI-103 and OI-104 are the items this record proposes answers to.

Numbering: 0026 is claimed on two unmerged branches (`claude/practical-hamilton-svzzan` and `claude/slice-8-viewport-shell`) and OI-117 to OI-119 on `claude/slice-8-viewport-shell`, so this record takes 0027 and OI-120 to OI-122. Renumber if those land differently.

## Context

Slice 20 asks for three things that can be prepared before the slice and that need other people's review:

- Item 8: assess the release tooling under NFR-14 and decide the packaging route, with separate tag schemes for the library family and the application.
- Item 10: document the Z1 air-cut procedure and package for three platforms.
- Item 12: a safe-practice checklist for first machine trials, written and reviewed by whoever operates the Z1.

Requirements touched: NFR-02 (Windows, macOS, Linux), NFR-10 (licence notices with binaries), NFR-12 (the user verifies generated G-code), NFR-14 (dependency health). Verification plan section 6 (gates 4 and 5 and their rules) and section 8. Decisions 0007 (Z1 air cut), 0013 (Z1 profile), 0020 item 8 (reference jobs) and 0023 (nightly builds, OI-103, OI-104).

## Decision (accepted)

### 1. Packaging route (item 8): a plain build matrix, extended from the nightly workflow

1. **Do not adopt `cargo-dist` for the first release.** The survey scores it 70 against 79 for `cargo-release`, its crates.io release (0.32.0, May 2026) trails its repository changelog (0.33.0, Sep 2026) by almost four months, and it does not produce the macOS app bundle, which stays ours either way ([crate-selection.md](../crate-selection.md), release tools). A plain matrix needs no new tool, so NFR-14 needs no justification beyond this. Revisit `cargo-dist` if installers for the app become wanted (item 4 below).
2. **Reuse the nightly build.** [Decision 0023](0023-nightly-builds.md) already builds `openchuck-cli` and `openchuck-ui` in release mode on the three platforms with licence notices and the G-code notice. The release workflow is the same matrix, triggered by an application tag, whose archives are attached to a GitHub release (not a pre-release) after the release gate (slice 20 item 9) passes.
3. **Archives** are the first deliverable on every platform: `.tar.gz` on Linux and macOS, `.zip` on Windows, each carrying the executables, `LICENSE.txt`, `THIRD-PARTY-NOTICES.html` and a README with the G-code notice (NFR-10, NFR-12). OpenCASCADE's LGPL notice and source offer are added to the same set (OI-29).
4. **Per-platform packaging beyond the archive:**
   - **macOS:** an `.app` bundle for `openchuck-ui` (a directory with `Info.plist` and the executable), built by a small `cargo xtask` command from a template, so no dependency is added. The CLI stays a plain executable in the archive. Without the bundle, opening the UI starts a terminal window and the system treats it as a document, so this is proposed as needed.
   - **Windows:** the `.zip` with a single `openchuck-ui.exe`. An installer (MSI or similar) is **not proposed** for the first release: the egui app is one executable and needs no registry entries or file associations yet. Add one only on a user request, and assess the tool under NFR-14 then.
   - **Linux:** the `.tar.gz`. No package format (deb, rpm, AppImage, Flatpak) for the first release.
5. **Platform coverage (OI-103).** Build the Linux archive on the oldest runner image that GitHub still offers (or in a container with an older glibc) and state the glibc floor in the README. Intel macOS and Linux arm64 are not built for the first release; each can be added as a matrix row later. The owner decides whether either is wanted.
6. **Signing (OI-104).** Not decided here: signing and notarisation need certificates the project does not have (an Apple Developer ID for notarisation; a code-signing certificate for Windows). The first release can ship unsigned with README instructions for the macOS quarantine flag and the Windows SmartScreen warning, or the owner can obtain certificates first. This is the owner's call.

### 2. Tag schemes (item 8)

Proposed, to be confirmed against `cargo-release` configuration when the library crates are first published:

| Release | Tag | Made by |
| --- | --- | --- |
| Library family (lockstep versions) | `lib-v<semver>`, for example `lib-v0.1.0` | `cargo-release` |
| Application binaries | `app-v<semver>`, for example `app-v0.1.0`; the workflow that builds archives triggers on this pattern | The maintainer, after the release gate |
| Development builds | `nightly` (moves every night; decision 0023) | The nightly workflow |

The two schemes cannot match each other's trigger pattern, and neither matches `nightly`. The release gate runs before either tag is cut.

### 3. The trial documents (items 10 and 12)

Two documents are drafted with this record:

- [z1-air-cut-procedure.md](../z1-air-cut-procedure.md): the gate 4 procedure for generated programs, which turns the plan in [reference-jobs.md](../reference-jobs.md) into a sheet the operator fills in. It uses only facts already in the documents (decisions 0007, 0013, 0020 and the reference job set). Where a value is still unmeasured (axis height, OI-60) it says so and makes the measurement a precondition.
- [z1-safe-practice-checklist.md](../z1-safe-practice-checklist.md): the checklist for gates 4 and 5. **It is a draft written by the agent from the documents, not from experience of the machine.** It is not reviewed. The slice 20 review item "reviewed by the Z1 operator" stays unticked until the operator has read it, corrected it and said so (OI-120).

Gate 5, the 3-axis repeatability cut, the G93 recheck and the tool change test remain hands-on steps for slice 20 (items 11, OI-41).

## Alternatives considered

- **`cargo-dist` for binaries and installers.** One tool for the workflow and installers, but a lagging crates.io release, a score below the plain route, no app bundle, and a tool whose generated workflow would replace one we already run.
- **A Windows MSI and Linux packages in the first release.** More reach, more maintenance, and signing questions on each; nothing in the requirements asks for them.
- **A single tag scheme.** Simpler, but a library release and an application release then collide on the same version number.
- **Waiting for the Z1 operator before drafting.** The operator's review is the point of the checklist; a draft gives them something to correct.

## Consequences

- Architecture section 12 ("Application binaries" and "Names and tags") is edited to the agreed route (done with the acceptance). Work breakdown slice 20 items 8 and 10 now note the route; what remains is to build the release workflow, the bundle command and the gate. OI-103 and OI-104 stay deferred to slice 20 (signing and extra platforms, with OI-122).
- No dependency changes and no code. `.github/workflows` is untouched.
- Nothing in the trial documents implies the generated G-code is safe to run unchecked (NFR-12).

## Open items for later decisions

| # | Item | Decide in | Needs |
| --- | --- | --- | --- |
| 1 | Accept or amend the packaging route and tag schemes above (OI-121) | Closed 14 Oct 2026: accepted | Project owner |
| 2 | macOS bundle: is a bundle without notarisation acceptable for the first release, and are Intel macOS or Linux arm64 wanted? (OI-103, OI-104, OI-122) | Slice 20 | Project owner; signing certificates if wanted |
| 3 | The Z1 operator reviews the safe-practice checklist and the air-cut procedure and corrects them (OI-120) | Closed 14 Oct 2026: reviewed and accepted without changes | The Z1 operator |
