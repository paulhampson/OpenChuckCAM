# 0510: Axial and indexed finishing can keep the links between passes low (OP-14, SET-06, SET-09, NFR-07, NFR-12, GC-07, NFR-06; issue #436; lane PATH-TIDY)

- **Status:** proposed. The owner's choice of the option (decision 1: name, default and wording) was relayed by the coordinator on 1 Nov 2026; the record is not yet accepted (decision 0049).
- **Date:** 1 Nov 2026.
- **Issue:** #436, "Axial strategies should have an option to reduce retraction".
- **Builds on:** [0033](0033-slice-14-rotary-roughing.md) item 6 (links against the stock the pass starts with), [0034](0034-slice-12-baseline-finishing.md) (finishing links), [0035](0035-workholding-and-retaining-tabs.md) (held regions and tabs are part of the target and stay in the stock).
- **Open items:** new OI-1740 (a height from the part's surface where the setup is known to be roughed) and OI-1741 (the same option for the other strategies that retract between passes). No project schema change.

## Context

Axial finishing cuts passes along the part, alternating direction (zigzag). Between two passes the tool leaves the cut, goes up to the retract height (the clearance cylinder or the safe Z, OP-14, SET-06), turns A and comes down. On a long carve most of the non-cutting time is this lift and drop at each end. Rotary roughing already links passes at a lower height, checked against the stock the layer starts with (0033 item 6); finishing passes carry no stock of their own, so they always retract fully.

## Decision

1. **The option (the owner's choice, relayed).** Axial and indexed finishing gain `link_height`, label **Links between passes**, a choice of `retract` (the default, for new and saved operations) or `low`. With `retract` the operation is made by exactly the code it was, so its output and every golden file are byte-identical. With `low`, a link stays the approach distance above the stock where the whole link is clear, and otherwise retracts as before.
2. **Where it is decided.** `Pass` gains `low_link: bool`; the finishing strategies set it on every pass when the option is `low`. The shared assembly (`assemble.rs`) then uses the same link rule roughing uses (`local_link_height`) for such a pass, against the setup's stock instead of a pass stock. Entries and leaves are not changed.
3. **How clearance is guaranteed (NFR-12, SET-09, OP-14).**
   - The link height is the approach distance above the highest stock under the tool's footprint (nine probes across it) at any sample of the straight move, samples at most half a tool radius apart with A included, and never below either end of the link.
   - The stock model contains the part, the held regions and the tabs (0035 items 1 and 8), so the link cannot be lower than them.
   - A link whose X range lies over a held region's keep-out interval (the region widened by the tool's radius, SET-09) is not lowered.
   - A height that is not below the retract height is not used. Every move is still put to the collision oracle.
4. **Where it cannot be honoured (NFR-07).** A link that cannot be lowered retracts as before. When any link did, a note against the first segment says "N of M links between passes could not be lowered". An unroughed blank, whose top is close to the clearance, will often lower few links; the parameter text says the saving depends on the stock.
5. **The generated G-code is still the user's to verify (NFR-12).** The parameter text says so; it does not claim the lower links were verified on a machine.
6. **Library version.** `Pass` is built by literal by hosts, so this is a breaking change under 0048: minor bump (provisional 0.35.0, assigned by the coordinator) and a changelog entry.

## Alternatives considered

- **A third value on the common `retract` parameter:** changes every strategy's schema and goldens. Rejected by the owner.
- **A boolean:** a choice names both behaviours in the interface and leaves room for a third.
- **A height from the part's surface plus the stock to leave:** lower, but not provably clear unless the stock is known to be roughed. Not chosen by the owner for now (OI-1740).
- **Giving finishing passes a `PassStock`:** would also change their entry and leave probing, and so the goldens.

## Consequences

- **Code:** `openchuck-path` (`strategy.rs`, `assemble.rs`, `axial.rs`, `indexed.rs`, every `Pass` literal), `openchuck-app` and `openchuck-ui` test literals; library version and changelog.
- **Tests:** `crates/openchuck-path/tests/axial_links.rs`, named `req_op_14_...`, `req_set_09_...`, `req_gc_07_...`.
- **Documents:** the verification plan row OP-14, the work breakdown (lane PATH-TIDY), the open-items register and the decision index.
- **Open items:** OI-1740, OI-1741.
