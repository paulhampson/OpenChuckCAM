# SP-11 report

Tolerance 0.01 mm. Detection: 9 lines x 17 samples per family. 91 poses, 15 ruling samples, 60 x 30 check grid.

## Detection

| Case | Ruling family | Residual along v (mm) | along u (mm) | Twist (deg) | Verdict |
| --- | --- | --- | --- | --- | --- |
| cylinder | AlongV | 0.000000 | 2.890188 | 0.000 | ruled |
| cone, matching taper | AlongV | 0.000000 | 1.107498 | 0.000 | ruled |
| cone, taper 4 deg | AlongV | 0.000000 | 1.107498 | 0.000 | ruled |
| cone, straight tool | AlongV | 0.000000 | 1.107498 | 0.000 | ruled |
| helicoid p=1.5 | AlongV | 0.000000 | 13.704624 | 3.029 | ruled |
| helicoid p=0.3 | AlongV | 0.000000 | 13.702694 | 0.611 | ruled |
| spline bump 0.004 | AlongV | 0.001875 | 1.082809 | 1.334 | ruled |
| spline bump 0.05 | - | 0.023438 | 1.082809 | 1.412 | refused: not a ruled surface within tolerance: best straight-line fit misses by 0.0234 mm (limit 0.0100 mm) |
| coaxial cylinder | AlongV | 0.000000 | 3.612735 | 0.000 | ruled |
| plane | AlongV | 0.000000 | 0.000000 | 0.000 | ruled |
| sphere | - | 1.142913 | 5.914922 | 28.591 | refused: not a ruled surface within tolerance: best straight-line fit misses by 1.1429 mm (limit 0.0100 mm) |
| torus | - | 1.399539 | 6.002163 | 51.463 | refused: not a ruled surface within tolerance: best straight-line fit misses by 1.3995 mm (limit 0.0100 mm) |

## Placement and deviation (poses from fitted rulings)

| Case | Mode | Max tilt (deg) | A travel (deg) | Max A step (deg) | Gouge (mm) | Left (mm) |
| --- | --- | --- | --- | --- | --- | --- |
| cylinder | Ideal | 0.000 | 0.0 | 0.00 | 0.0000 | 0.0022 |
| cylinder | Axis4 | 0.000 | 0.0 | 0.00 | 0.0000 | 0.0022 |
| cone, matching taper | Ideal | 0.000 | 0.0 | 0.00 | 0.0000 | 0.0006 |
| cone, matching taper | Axis4 | 0.000 | 0.0 | 0.00 | 0.0000 | 0.0006 |
| cone, taper 4 deg | Ideal | 0.000 | 2.3 | 0.05 | 0.0000 | 0.0005 |
| cone, taper 4 deg | Axis4 | 1.832 | 2.3 | 0.05 | 0.2009 | 0.1858 |
| cone, straight tool | Ideal | 0.000 | 6.9 | 0.14 | 0.0000 | 0.0005 |
| cone, straight tool | Axis4 | 5.495 | 6.9 | 0.14 | 0.6564 | 0.4226 |
| helicoid p=1.5 | Ideal | 0.000 | 153.0 | 1.70 | 0.0042 | 0.0318 |
| helicoid p=1.5 | Axis4 | 0.000 | 153.0 | 1.70 | 0.0042 | 0.0318 |
| helicoid p=0.3 | Ideal | 0.000 | 153.0 | 1.70 | 0.0002 | 0.0326 |
| helicoid p=0.3 | Axis4 | 0.000 | 153.0 | 1.70 | 0.0002 | 0.0326 |
| spline bump 0.004 | Ideal | 0.000 | 1.7 | 0.04 | 0.0000 | 0.0039 |
| spline bump 0.004 | Axis4 | 0.000 | 1.7 | 0.04 | 0.0000 | 0.0039 |
| spline bump 0.05 | Ideal | 0.000 | 1.7 | 0.04 | 0.0000 | 0.0383 |
| spline bump 0.05 | Axis4 | 0.000 | 1.7 | 0.04 | 0.0000 | 0.0383 |
| coaxial cylinder | Ideal | 0.000 | 0.0 | 0.00 | 0.0000 | 0.0017 |
| coaxial cylinder | Axis4 | - | - | - | AxisAlongA | |
| plane | Ideal | 0.000 | 0.0 | 0.00 | 0.0000 | 0.0005 |
| plane | Axis4 | 0.000 | 0.0 | 0.00 | 0.0000 | 0.0005 |
| sphere | Ideal | 0.000 | 53.0 | 1.32 | 1.8363 | 0.0006 |
| sphere | Axis4 | 41.506 | 53.0 | 1.32 | 2.9910 | 2.2901 |
| torus | Ideal | 0.000 | 42.9 | 1.84 | 0.0000 | 2.2767 |
| torus | Axis4 | 60.651 | 42.9 | 1.84 | 2.9489 | 4.0608 |

## Convergence with pose count (Axis4)

| Case | Poses | Gouge (mm) | Left (mm) |
| --- | --- | --- | --- |
| cylinder | 11 | 0.00000 | 0.12519 |
| cylinder | 21 | 0.00000 | 0.04177 |
| cylinder | 41 | 0.00000 | 0.00933 |
| cylinder | 81 | 0.00000 | 0.00292 |
| cylinder | 161 | 0.00000 | 0.00073 |
| cylinder | 321 | 0.00000 | 0.00018 |
| cylinder | 641 | 0.00000 | 0.00004 |
| cone, matching taper | 11 | 0.00000 | 0.03289 |
| cone, matching taper | 21 | 0.00000 | 0.01088 |
| cone, matching taper | 41 | 0.00000 | 0.00242 |
| cone, matching taper | 81 | 0.00000 | 0.00076 |
| cone, matching taper | 161 | 0.00000 | 0.00019 |
| cone, matching taper | 321 | 0.00000 | 0.00005 |
| cone, matching taper | 641 | 0.00000 | 0.00001 |
| helicoid p=1.5 | 11 | 0.00421 | 1.49646 |
| helicoid p=1.5 | 21 | 0.00418 | 0.56040 |
| helicoid p=1.5 | 41 | 0.00422 | 0.14739 |
| helicoid p=1.5 | 81 | 0.00422 | 0.04059 |
| helicoid p=1.5 | 161 | 0.00422 | 0.00984 |
| helicoid p=1.5 | 321 | 0.00422 | 0.00192 |
| helicoid p=1.5 | 641 | 0.00422 | 0.00033 |
| spline bump 0.004 | 11 | 0.00000 | 0.08263 |
| spline bump 0.004 | 21 | 0.00000 | 0.02384 |
| spline bump 0.004 | 41 | 0.00000 | 0.00766 |
| spline bump 0.004 | 81 | 0.00000 | 0.00398 |
| spline bump 0.004 | 161 | 0.00000 | 0.00329 |
| spline bump 0.004 | 321 | 0.00000 | 0.00305 |
| spline bump 0.004 | 641 | 0.00000 | 0.00302 |

## Detection residual against sampling density (mm)

| Case | 5 x 9 | 9 x 17 | 17 x 33 | 33 x 65 |
| --- | --- | --- | --- | --- |
| cylinder | 0.000000 | 0.000000 | 0.000000 | 0.000000 |
| helicoid p=1.5 | 0.000000 | 0.000000 | 0.000000 | 0.000000 |
| spline bump 0.004 | 0.001750 | 0.001875 | 0.001937 | 0.001969 |
| spline bump 0.05 | 0.021875 | 0.023438 | 0.024219 | 0.024609 |
| sphere | 1.066929 | 1.142913 | 1.180952 | 1.199985 |
| torus | 1.307084 | 1.399539 | 1.445956 | 1.469219 |

## Helicoid pitch sweep (straight tool r 3, 641 poses, Axis4)

| Pitch (mm/rad) | Twist (deg) | Gouge (mm) | Left (mm) |
| --- | --- | --- | --- |
| 0.3 | 0.611 | 0.00017 | 0.00053 |
| 1.5 | 3.029 | 0.00422 | 0.00033 |
| 3 | 5.886 | 0.01593 | 0.00033 |
| 6 | 10.585 | 0.05145 | 0.00029 |
| 12 | 15.212 | 0.10585 | 0.00036 |
