# Brief: Lane STEPALL-P (pencil from concave B-rep edges)  [Opus]
START WHEN: STEPALL-F (#274) has MERGED.
ORIGIN: #257 / decision 0209 option A, step 3.
Branch claude/lane-stepall-p. Numbers: decision 0226, OI-950 to OI-959 (verify). Existing OI owned: OI-234 (b).
SCOPE: pencil finishing from the exact concave B-rep edges of a STEP part (no false creases between tessellation facets), with face sets honoured as on an STL (OP-23 from 0210). Keep the mesh path unchanged for STL parts. Pencil lessons: lane BUG-RESULTS (#231, decision 0162 item 6) fixed pencil cutting through blade tips: keep that behaviour and its tests. Measure with the independent checker against the exact faces; deterministic at any thread count; kernel stays in openchuck-import (surface-source interface). Example data: testdata/step/blisk.step.
