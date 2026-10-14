# SP-6 report

Release build, 4 threads available. Stock Ø50 x 40 mm.

## 1. Turned parts (ball Ø4, 0.25 mm stations, 1 degree of A)

| Part | Cell | Model | Volume error mm3 | Error bound mm3 | Worst radial error mm | Time s (4 threads) | Memory MB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| cylinder r 20 (52200 poses) | 0.2 | rotary grid | +18.3 | 479.5 | 0.0108 | 5.8 | 1.3 |
| cylinder r 20 (52200 poses) | 0.2 | dexel | +21.5 | 479.5 | 0.0946 | 3.7 | 3.7 |
| cylinder r 20 (52200 poses) | 0.2 | voxel | +33.5 | 479.5 | 0.1248 | 4.1 | 1.6 |
| cylinder r 20 (52200 poses) | 0.1 | rotary grid | +18.3 | 253.3 | 0.0106 | 23.2 | 5.0 |
| cylinder r 20 (52200 poses) | 0.1 | dexel | +19.0 | 253.3 | 0.0535 | 14.7 | 14.9 |
| cylinder r 20 (52200 poses) | 0.1 | voxel | +10.8 | 253.3 | 0.0607 | 16.1 | 12.5 |
| cylinder r 20 (52200 poses) | 0.05 | rotary grid | +19.0 | 140.2 | 0.0122 | 94.2 | 20.1 |
| cylinder r 20 (52200 poses) | 0.05 | dexel | +19.3 | 140.2 | 0.0308 | 58.7 | 59.6 |
| cylinder r 20 (52200 poses) | 0.05 | voxel | +18.8 | 140.2 | 0.0365 | 66.6 | 100.0 |
| cone 16.7 deg (52200 poses) | 0.2 | rotary grid | +9.8 | 350.5 | 0.0352 | 6.3 | 1.3 |
| cone 16.7 deg (52200 poses) | 0.2 | dexel | +9.9 | 350.5 | 0.1270 | 3.1 | 2.9 |
| cone 16.7 deg (52200 poses) | 0.2 | voxel | +9.4 | 350.5 | 0.1585 | 3.7 | 1.6 |
| cone 16.7 deg (52200 poses) | 0.1 | rotary grid | +9.8 | 185.1 | 0.0227 | 25.1 | 5.0 |
| cone 16.7 deg (52200 poses) | 0.1 | dexel | +9.8 | 185.1 | 0.0707 | 13.0 | 11.8 |
| cone 16.7 deg (52200 poses) | 0.1 | voxel | +9.6 | 185.1 | 0.0841 | 13.6 | 12.5 |
| cone 16.7 deg (52200 poses) | 0.05 | rotary grid | +10.3 | 102.5 | 0.0169 | 98.5 | 20.1 |
| cone 16.7 deg (52200 poses) | 0.05 | dexel | +10.3 | 102.5 | 0.0362 | 57.6 | 47.6 |
| cone 16.7 deg (52200 poses) | 0.05 | voxel | +10.3 | 102.5 | 0.0434 | 61.4 | 100.0 |
| sphere r 20 (23400 poses) | 0.2 | rotary grid | +8.4 | 213.1 | 0.0414 | 7.1 | 1.3 |
| sphere r 20 (23400 poses) | 0.2 | dexel | +8.8 | 213.1 | 0.1257 | 4.6 | 2.8 |
| sphere r 20 (23400 poses) | 0.2 | voxel | +8.9 | 213.1 | 0.1636 | 4.3 | 1.6 |
| sphere r 20 (23400 poses) | 0.1 | rotary grid | +8.3 | 112.6 | 0.0230 | 28.8 | 5.0 |
| sphere r 20 (23400 poses) | 0.1 | dexel | +8.4 | 112.6 | 0.0626 | 17.1 | 11.4 |
| sphere r 20 (23400 poses) | 0.1 | voxel | +8.4 | 112.6 | 0.0821 | 18.9 | 12.5 |
| sphere r 20 (23400 poses) | 0.05 | rotary grid | +8.6 | 62.3 | 0.0157 | 115.5 | 20.1 |
| sphere r 20 (23400 poses) | 0.05 | dexel | +8.6 | 62.3 | 0.0367 | 68.1 | 45.4 |
| sphere r 20 (23400 poses) | 0.05 | voxel | +8.6 | 62.3 | 0.0432 | 73.6 | 100.0 |

## 2. Straight pass by tool type (A = 0, tip 3 mm into the cylinder, 20 mm long, 0.05 mm poses)

Reference: quadrature of the plan view with the profile's `h(r)`, 0.01 mm. Removed volume in mm3, then microseconds per pose on one thread.

| Tool | Reference | Model | Removed (0.1 cell) | Error | us per pose |
| --- | --- | --- | --- | --- | --- |
| flat Ø6 | 436.3 | rotary grid | 464.1 | +27.8 | 3195 |
| flat Ø6 | 436.3 | dexel | 436.4 | +0.0 | 95 |
| flat Ø6 | 436.3 | voxel | 436.9 | +0.6 | 152 |
| ball Ø6 | 330.8 | rotary grid | 330.8 | -0.0 | 7691 |
| ball Ø6 | 330.8 | dexel | 331.0 | +0.2 | 6598 |
| ball Ø6 | 330.8 | voxel | 331.4 | +0.6 | 6004 |
| V-bit 90 | 200.5 | rotary grid | 200.5 | +0.0 | 6813 |
| V-bit 90 | 200.5 | dexel | 200.4 | -0.0 | 5965 |
| V-bit 90 | 200.5 | voxel | 196.8 | -3.7 | 6216 |
| custom ball (2 segments) | 232.6 | rotary grid | 235.2 | +2.6 | 3640 |
| custom ball (2 segments) | 232.6 | dexel | 232.8 | +0.2 | 2992 |
| custom ball (2 segments) | 232.6 | voxel | 234.1 | +1.5 | 2496 |

## 3. Reference job at the default 0.1 mm cell: cone, 52,200 poses

| Model | 1 thread s | 2 threads s | 4 threads s | Hash equal at 1, 2, 4 |
| --- | --- | --- | --- | --- |
| rotary grid | 83.1 | 48.5 | 24.3 | true |
| dexel | 45.8 | 24.1 | 13.3 | true |
| voxel | 50.8 | 26.2 | 14.1 | true |

## 4. Memory at the default cell, untouched stock

| Model | 0.2 MB | 0.1 MB | 0.05 MB |
| --- | --- | --- | --- |
| rotary grid | 1.3 | 5.0 | 20.1 |
| dexel | 2.0 | 8.0 | 32.0 |
| voxel | 1.6 | 12.5 | 100.0 |
