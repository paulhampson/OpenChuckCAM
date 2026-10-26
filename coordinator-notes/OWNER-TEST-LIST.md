# Issues for the owner to verify and close on a new build

## BUG-SIMEND — pending draft review and a build containing this lane

| Issue | What to check | Lane |
| --- | --- | --- |
| #334 | On the affected project, drag the playback point to the far right. The final cut part must appear without stepping back or another interaction. Also check Go to the final move, stepping and playing to the end, with deviation colours on/off. Retest after a new simulation and while an earlier stock surface is still being prepared. | BUG-SIMEND; cause and automated evidence in [verification](../docs/handoffs/bug-simend-verification.md) |
| #330 | When checkpoint preparation hits its memory bound or a cut-surface job fails, Deviation colours is disabled with the reason beside it and in its tooltip. Playback retains the error and guidance; preparation stops without automatic retry. Retained stale results remain marked stale and export stays blocked. After a successful fresh run, stock colouring and picking work normally. | BUG-SIMEND; automated tests inject the errors at the UI job boundaries rather than allocating over 1.5 GiB |

The owner tests and closes these issues; the lane uses Refs, not Closes.
Existing reference-hardware rendering follow-up remains OI-987. These display
checks do not verify generated G-code for machining; NFR-12 remains unchanged.
