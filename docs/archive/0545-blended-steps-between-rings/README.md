# Decision 0545: simulation of Sharp against Blended steps between rings

Raw output cited by [decision 0545](../../decisions/0545-blended-steps-between-rings.md), As built ("Simulated both ways"). Made by lane RINGSTEP on 8 Nov 2026 from branch `claude/lane-ringstep` at commit `eb33ebf` (pull request #655), with the debug build of `openchuck-cli`.

The project is the public baluster example (`testdata/examples/baluster/baluster-project`), copied twice to a scratch folder. Its two operations, Rough (continuous rotary roughing) and Finish (helical finishing), were each given `pass_shape = "rings"`, one copy with `ring_step = "sharp"` and the other with `ring_step = "blended"` (Blend over at its default, 45 degrees). Nothing else was changed. The scratch folder's path is removed from the output.

| File | What it is |
| --- | --- |
| `simulate-sharp.txt`, `simulate-blended.txt` | `openchuck-cli simulate <copy>`: the notes, the material removed by each operation, the most material left on the part and the deepest overcut (cell 0.1 mm) |
| `regenerate-sharp.txt`, `regenerate-blended.txt` | `openchuck-cli regenerate <copy> --out <folder> --setup-sheet` |
| `setup-sheet-times-sharp.txt`, `setup-sheet-times-blended.txt` | The programmed times read from the setup sheet each run wrote (the sum of the move durations at their programmed feeds, with the machine's feed limits) |

Nothing was run on a machine. Generated G-code remains the user's to verify before running it on a machine.
