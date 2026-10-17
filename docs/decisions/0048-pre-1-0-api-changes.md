# 0048: Before 1.0, design to the architecture; a breaking change to the library API bumps the version

- **Status:** accepted by the project owner (16 Oct 2026, in the orchestration session, on seeing that decision 0047 had rejected an alternative because it would fail `cargo-semver-checks`).
- **Date:** 16 Oct 2026. OI-210 settled 17 Oct 2026 (item 5).
- **Open items** are tracked in [docs/open-items.md](../open-items.md): OI-210 was new and is closed by item 5.

Numbering: this record takes 0048 and OI-210, after lanes W, M and K hold 0045 to 0047 and OI-194 to OI-209.

## Context

The library family (`openchuck-types`, `-path`, `-post`, `-gcode`) is at 0.7.0 and nothing has been released (slice 20). CI runs `cargo-semver-checks` on it in every pull request (INT-07, verification plan). Since decision 0026 the project instructions told contributors never to add a field to a public struct, and to add a type or function beside it instead (`EntryParameters` and `assemble_with_entry`, then others). That rule kept the check green, but it shaped designs: decision 0047 rejected "chains in `StrategyInput`" because the field would fail the check, and lanes have written parallel types and `*_with_*` functions that a host will have to learn.

Semantic versioning allows breaking changes in a 0.x release, as long as the minor version is bumped. The check passes when the version is bumped.

## Decision

1. **Until the first release (slice 20), choose the API that best fits the architecture and the design.** A public struct gains a field, a function gains a parameter, a type is renamed or removed, whenever that is the better design. Do not add a parallel type or a second entry point only to keep `cargo-semver-checks` green. A lane that finds a workaround in its own design for this reason revisits it with the orchestrator.
2. **A breaking change bumps the version.** The pull request that makes it raises the minor version of the workspace (`[workspace.package] version`, currently 0.7.0), adds a changelog entry to each affected library crate saying what broke and how a host adapts, and keeps `cargo-semver-checks` green that way. It is not a reason to ask for approval; the record that explains the design says it breaks the API.
3. **After the first release this rule ends.** From 1.0 the API is stable (INT-07) and breaking changes follow semantic versioning with a major bump. Architecture section 5 and INT-07 are unchanged.
4. **Existing workarounds stay** unless a lane is already changing the code; they are not refactored for their own sake.
5. **Each breaking pull request bumps the minor version again (OI-210, settled by the project owner on 17 Oct 2026, option A).** CI compares a pull request with its base commit on `main`, so a second breaking change on a version a first one already raised fails the check until it bumps too. That is accepted: the numbers climb quickly before the first release and carry no meaning, the changelog entry is what tells a host what broke, and the first release takes whatever version is current. Two breaking pull requests are not open at once in practice, because the orchestrator merges one at a time; if they are, the second bumps again after it updates from `main`. Not chosen: taking the last released tag as the CI baseline (no tag exists, so the check would be off until the first release) and making the check advisory until then (it loses the failure signal and the changelog discipline).

## Alternatives considered

- **Keep the rule:** safe for hosts, but nothing is released and no host exists, so the cost is design quality now for a stability nobody depends on yet.
- **Turn the semver check off before 1.0:** loses the changelog discipline and the first-release baseline. Keeping it on with a version bump costs one line.

## Consequences

- **Documents:** `AGENTS.md`, `CLAUDE.md` and the work breakdown's "Library API" bullet say the above in place of the old prohibition.
- **Open items:** OI-210 is closed by item 5. No change to CI, `AGENTS.md`, `CLAUDE.md` or the verification plan: they already say a breaking change bumps the version.
