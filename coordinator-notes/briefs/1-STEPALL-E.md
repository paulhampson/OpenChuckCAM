# Brief: Lane STEPALL-E (edge following on the exact edge curve)  [Sonnet]
START WHEN: any time (no dependency); lowest priority of the STEPALL family, small (S).
ORIGIN: #257 / decision 0209 option A, step 4.
Branch claude/lane-stepall-e. Numbers: decision 0227, OI-960 to OI-969 (verify). Existing OI owned: OI-229.
SCOPE: engraving depth exact on curved B-rep edges: edge following uses the exact edge curve (decision 0047 for the mesh form; lane P built B-rep edge following). Keep the mesh path unchanged. Deterministic; kernel stays in openchuck-import; CI job 'STEP kernel (os)' runs the kernel tests (install OpenCASCADE with the repo tooling if your container lacks it).
