# SP-13 report

Threads available: 4. Tool D = 6 mm, flute 12 mm. Mesh chord 0.0005 mm.

## 1. Drop-cutter against closed form (mesh is inscribed, so error = exact - mesh >= 0)

Error along the surface normal is the vertical error times the normal's Z at the contact.

| Part | Tool | Positions | Min error (mm) | Max error (mm) | Max error along the normal (mm) | Mesh edge sagitta (mm) |
| --- | --- | --- | --- | --- | --- | --- |
| sphere r15 | flat | 1681 | 0.00e0 | 1.09e-3 | 9.93e-4 | 4.99e-4 |
| sphere r15 | ball | 1681 | 0.00e0 | 1.21e-3 | 9.95e-4 | 4.99e-4 |
| sphere r15 | V 90 | 1681 | 0.00e0 | 1.30e-3 | 9.66e-4 | 4.99e-4 |
| cylinder r20 | flat | 1681 | 0.00e0 | 5.20e-4 | 4.95e-4 | 4.98e-4 |
| cylinder r20 | ball | 1681 | 0.00e0 | 5.40e-4 | 4.97e-4 | 4.98e-4 |
| cylinder r20 | V 90 | 1681 | 0.00e0 | 5.20e-4 | 4.95e-4 | 4.98e-4 |
| cone r10 20deg (y=0) | flat | 41 | -1.78e-15 | 1.78e-15 | 1.78e-15 | 4.99e-4 |
| cone r10 20deg (y=0) | ball | 41 | -1.78e-15 | 1.78e-15 | 1.78e-15 | 4.99e-4 |
| cone r10 20deg (y=0) | V 90 | 41 | -1.78e-15 | 1.78e-15 | 1.78e-15 | 4.99e-4 |

## 1b. Custom profile against the equivalent standard tool (169 positions per part)

| Custom profile | Sphere worst difference (mm) | Cylinder | Cone 20deg |
| --- | --- | --- | --- |
| ball as two arcs | 0.0e0 | 0.0e0 | 0.0e0 |
| flat, split bottom and side | 0.0e0 | 0.0e0 | 0.0e0 |
| V 90, split flank | 0.0e0 | 0.0e0 | 0.0e0 |
| bull-nose typed in | 0.0e0 | 0.0e0 | 0.0e0 |
| tapered ball typed in | 0.0e0 | 0.0e0 | 0.0e0 |

## 2. Independent gouge checker (exact surface, 0.01 mm sampling)

| Part | Tool | Worst gouge at dropped pose (mm) | Smallest gouge with tip lowered 0.05 mm (mm) |
| --- | --- | --- | --- |
| sphere r15 | flat D6 | 9.13e-5 | 0.0343 |
| sphere r15 | ball D6 | 9.76e-4 | 0.0429 |
| sphere r15 | bull-nose D6 rc1 | 6.18e-4 | 0.0447 |
| sphere r15 | tapered ball D6 r0.5 15deg | 6.01e-4 | 0.0399 |
| sphere r15 | V-bit D6 90deg | 6.04e-4 | 0.0354 |
| sphere r15 | chamfer D6 90deg tip1 | 7.33e-4 | 0.0376 |
| sphere r15 | drill D6 118deg | 7.32e-4 | 0.0429 |
| sphere r15 | custom bell (concave arc) | 5.93e-4 | 0.0298 |
| cylinder r20 | flat D6 | 4.44e-16 | 0.0367 |
| cylinder r20 | ball D6 | 4.12e-4 | 0.0458 |
| cylinder r20 | bull-nose D6 rc1 | 4.87e-4 | 0.0469 |
| cylinder r20 | tapered ball D6 r0.5 15deg | 4.58e-4 | 0.0445 |
| cylinder r20 | V-bit D6 90deg | 3.60e-4 | 0.0354 |
| cylinder r20 | chamfer D6 90deg tip1 | 2.88e-4 | 0.0400 |
| cylinder r20 | drill D6 118deg | 4.36e-4 | 0.0429 |
| cylinder r20 | custom bell (concave arc) | 3.54e-8 | 0.0313 |

## 3. Timings (release build, this container)

Sphere mesh: 147070 triangles (385 divisions around).

| Tool | Profile query h(r) (ns) | Drop per position, 1 thread (us) | Normal-offset tip (ns) |
| --- | --- | --- | --- |
| flat D6 | 8.0 | 351.6 | 90 |
| ball D6 | 6.7 | 2456.6 | 122 |
| bull-nose D6 rc1 | 8.3 | 793.0 | 102 |
| tapered ball D6 r0.5 15deg | 5.6 | 3240.0 | 109 |
| V-bit D6 90deg | 6.0 | 2730.7 | 82 |
| chamfer D6 90deg tip1 | 6.8 | 2275.0 | 91 |
| drill D6 118deg | 5.2 | 2782.3 | 91 |
| custom bell (concave arc) | 6.3 | 2205.1 | n/a (not convex) |

| Sweep | Tool | Cells x poses | 1 thread (s) | 4 threads (s) | ns per cell-pose (1 thread) | Identical bits |
| --- | --- | --- | --- | --- | --- | --- |
| dexel 401x201, 0.05 mm, line move | flat D6 | 19018659 | 0.308 | 0.141 | 16.2 | true |
| dexel 401x201, 0.05 mm, line move | ball D6 | 19018659 | 0.334 | 0.147 | 17.6 | true |
| dexel 401x201, 0.05 mm, line move | bull-nose D6 rc1 | 19018659 | 0.397 | 0.217 | 20.9 | true |
| dexel 401x201, 0.05 mm, line move | tapered ball D6 r0.5 15deg | 19018659 | 0.326 | 0.176 | 17.1 | true |
| dexel 401x201, 0.05 mm, line move | V-bit D6 90deg | 19018659 | 0.313 | 0.184 | 16.5 | true |
| dexel 401x201, 0.05 mm, line move | chamfer D6 90deg tip1 | 19018659 | 0.336 | 0.142 | 17.7 | true |
| dexel 401x201, 0.05 mm, line move | drill D6 118deg | 19018659 | 0.364 | 0.139 | 19.2 | true |
| dexel 401x201, 0.05 mm, line move | custom bell (concave arc) | 19018659 | 0.375 | 0.215 | 19.7 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | flat D6 | 31104000 | 0.724 | 0.287 | 23.3 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | ball D6 | 31104000 | 3.129 | 1.216 | 100.6 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | bull-nose D6 rc1 | 31104000 | 1.974 | 0.589 | 63.5 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | tapered ball D6 r0.5 15deg | 31104000 | 2.435 | 1.112 | 78.3 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | V-bit D6 90deg | 31104000 | 2.594 | 1.006 | 83.4 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | chamfer D6 90deg tip1 | 31104000 | 2.561 | 1.047 | 82.3 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | drill D6 118deg | 31104000 | 3.070 | 1.226 | 98.7 | true |
| rotary grid 161x360, wrapped move, 360 deg of A | custom bell (concave arc) | 31104000 | 2.199 | 0.720 | 70.7 | true |
