# Archive: decision 0543 (smooth steps between rings), raw evidence

- `engagement.py`: a 2D model of one roughing layer, unrolled (X along the part, arc length round it), that sweeps a flat end mill's footprint over a grid and reports the width of cut along a spiral, along rings with today's sharp step, and along rings with a blended step (option A, "whole", and option B, "seam") at several blend angles. Independent of the product code. Run with `python3 -I engagement.py` (needs NumPy); about a minute.
- `engagement-output.txt`: its output, 8 Nov 2026.

Cited by [decision 0543](../../decisions/0543-smooth-steps-between-rings.md), Verification. Produced on `main` `1f3675d`; no product code was run. Nothing here verifies generated G-code (NFR-12).
