# Lane FACEAPPROACH screenshots (issue #499), throwaway, not for merging

Owner screenshots of the Baluster example (public model, Setup 1 Makera Z1, stock box 72.4 x 35 x 35, Operations: 1 End facing, 2 Rough, 3 Finish).
- roughing-enabled.png: with "2 Rough" enabled the End facing toolpath (orange) is a set of straight parallel passes across the end plane, ending at the stock edges.
- roughing-disabled.png: with "2 Rough" disabled the End facing toolpath is drawn as many stacked rings/helical turns up the end plane, with the entry (violet) coming down from the top.
Both: the tool (violet) sits high above the stock, and the approach/entry appears to start at the safe Z, not just above the stock top.
- cutter-sweep-pr3.png (lane FACEAPPROACH, 2 Nov 2026): the same Baluster set-up (End facing T104, stock box 72.4 x 35 x 35, start end held) generated with the build of PR 3 #506 (head c0e0d9f1, which contains PR 1 #502 and PR 2 #505), captured in the real application under Xvfb with Mesa's software Vulkan driver. The path legend lists "Cutter sweep" (lime, thin line); the sweep is drawn only along the cutting moves at the tool's tip (decision 0643), not at the flute top or on plunges, so it no longer stacks up to the safe Z. The orange lines are the cuts; the end is faced from A 0 and A 180 (decision 0642), and the white lines are the rapids and the A turn between the two sides.
