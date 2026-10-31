# Lane ENDFACE stage 3: adaptive pass spacing (#303), plan

Base: main 5ece3ce (#412 merged; library 0.28.0, project schema 23). Branch `claude/lane-endface-adaptive` (the coordinator, 31 Oct 2026). Accepted by the coordinator 31 Oct 2026: schema 24 assigned, the product-side bound sentence added to 0239, library 0.29.0, one pull request (a split put to the owner only if the diff is too large to review). One draft PR, one push (decision 0055). Refs #303.

## Authority
- R1 (requirements, after the OP-22 notes) and A1 (architecture 5.2) as the owner approved them, inserted word for word.
- Decision 0239 stays proposed; its 13 owner choices are the specification. The FINSTEP handoff is guidance only: no FINSTEP source survives, so nothing from it is relied on.

## Points needing a call before code
1. **Schema number.** 0239 item 6 says "schema 23, shared with 0344", but 23 merged with #412, and a file saved at 23 from now on would skip a migration added to the 22→23 step. Proposal: claim schema 24 (free on main and every branch), with a 23→24 step writing an explicit `adaptive_spacing = false` into existing helical, axial, indexed, simultaneous, roughing and facing operations. 0239 item 6 gets reworded to "schema 24" (wording only). The alternative (absent means off, new operations write on, no bump) would contradict 0239's "migrated to an explicit off".
2. **The 0255 checker is test-only.** It is `openchuck_testkit::contact`, and its record forbids `openchuck-path` from using it (independence, item 6). 0239's "used as it stands on main" therefore holds for the tests only.
   - In the product, the excess measure needs its own bound in `openchuck-path`: the operation's cutter model on exact drops (0260, 0277), with a place marked "unverified" wherever no bound justifies it (item 11).
   - The tests then grade the result with the testkit checker and the tool outline, independently.
   - Proposed: one sentence added to 0239 saying this. It is no change to R1 or A1, which already say "never with the strategy's own sampler as proof".
3. **Library version.** Yes, a minor bump, 0.28.0 → 0.29.0:
   - 0239 item 13: the new-operation default changes the output.
   - `PathWarning` gains a public field (`requires_acknowledgement`), which breaks struct literals.
   - So `cargo-semver-checks` needs the bump anyway. CHANGELOG entry (decision 0048).
4. **Size.** This is the largest stage. If the owner prefers, it splits cleanly into two PRs: (A) path, app and CLI; (B) UI, the legacy-zero dialog and the UI spec. Default is one PR, as instructed.

## Sequence (each step test-first, the tests named by requirement ID and watched failing)
- **3a, `openchuck-path::adaptive`:**
  - Limits: 1,000,000 queries; 1,000,000 points; 10,000 added passes; 20 levels; 65,536 pending intervals. Counted per operation across all layers, with checked arithmetic, charged before the work.
  - A deterministic left-first interval scheduler, with a smallest spacing of 0.01 mm.
  - An excess measure above the operation's own target (stock to leave plus the layer, decision 0345).
  - A query refused for lack of budget is never read as no surface.
- **3b, helical and axial finishing:**
  - Parameters `adaptive_spacing` (on for a new operation) and `min_spacing`.
  - The existing `scallop` key is shared as Allowed excess with adaptive on, through the `schema_for` and `resolve_parameters` hooks: default 0 off, 0.01 on, applied only when absent.
  - Off is byte-identical (golden tests unchanged).
- **3c, indexed finishing** (fixed A kept, no added angle), **the simultaneous axial pattern, both axial roughing forms** (ROUGHPATH's layer engine, groups and resets kept), and **flat Facing** (its defaults and equal layers, and its strict completion checks untouched).
- **3d, partial results:**
  - `PathWarning::requires_acknowledgement`, carried through ordinary and leave-zone paths.
  - Notes: passes added, smallest spacing, limits met, unverified places.
  - `openchuck-app` asks for an acknowledgement of the current result where it plans and writes the output; a stale acknowledgement is refused.
  - CLI `--acknowledge-partial-path`.
  - Hard failures (bad input, cancel, gouge, protection, entry, clearance, replay) stay errors.
- **3e, UI:**
  - The inspector shows Scallop or Allowed excess by mode, plus Smallest spacing.
  - Turning adaptive on from a saved Scallop 0 asks for the height: it suggests 0.01, allows 0, Cancel changes nothing, and confirming is one undo step.
  - The export dialog asks for the acknowledgement.
  - UI spec 11.4 proposal.
- **3f, records:**
  - Schema 24 and its migration, with persistence tests.
  - The R1 and A1 insertions; a verification plan proposal (from 0239's Verification section).
  - CHANGELOG, the 0.29.0 bump, the work-breakdown row and schema claim.
  - Open items, from the next free numbers at the time.
  - OI-1373 stays open (engagement is not promised).

## Tests (from R1 and 0239)
- **Excess, measured independently** on the emitted and the posted motion: the tool outline and the testkit contact checker.
- **Coverage across the profile:** steep and vertical walls; the stock to leave and the layers kept (never refined below the target); protections, held regions and tabs; entries and links; flute reach.
- **Off and determinism:** off byte-identical; 1, 2 and N threads identical, including a partial result at a limit.
- **Limits and errors:** each limit reached; bad settings refused; progress and cancelling.
- **Acknowledgement:** a stale acknowledgement refused, and an ordinary warning never counts as one.
- **Migration and UI:** the schema 24 migration; the legacy-zero dialog (cancel, confirm, one undo).
- **Timing:** uninstrumented, per the long-test rule. Coverage per changed crate (path, app, project, cli, ui), with the uncovered lines named.

## Risks
- **Run time:** limits of a million queries per operation could take minutes on the corpus. The timings will be measured and long tests listed; anything over 10 minutes gets flagged.
- **Output size and engagement:** closer passes mean more output and possibly more engagement (OI-1373).
- **Goldens:** off is unchanged. The goldens' saved projects migrate to off, so no golden should change; any change goes to the owner.
- **NFR-12:** generated G-code remains the user's to verify. No acknowledgement or added pass is called proof.
