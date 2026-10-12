| Surface | Operation | Method | Samples | Max | Mean | Failures |
| --- | --- | --- | --- | --- | --- | --- |
| sphere as NURBS | conversion: distance of NURBS point to exact surface (mm) | curvo | 1296 | 1.300e-14 | 1.868e-15 | 0 |
| sphere as NURBS | conversion: distance of NURBS point to exact surface (mm) | rust | 1296 | 3.056e-14 | 2.326e-15 | 0 |
| sphere as NURBS | evaluator agreement, rust vs curvo (mm) | curvo | 1296 | 3.117e-14 | 5.042e-15 | 0 |
| sphere as NURBS | project point (mm) | curvo | 2592 | 1.378e-7 | 3.792e-8 | 0 |
| sphere as NURBS | project point (mm) | rust | 2592 | 4.750e-1 | 1.161e-2 | 0 |
| sphere as NURBS | ray first hit (mm) | rust | 1296 | 9.774e-13 | 3.084e-14 | 0 |
| torus as NURBS | conversion: distance of NURBS point to exact surface (mm) | curvo | 144 | 1.507e-14 | 4.921e-15 | 0 |
| torus as NURBS | conversion: distance of NURBS point to exact surface (mm) | rust | 144 | 1.831e-14 | 5.303e-15 | 0 |
| torus as NURBS | evaluator agreement, rust vs curvo (mm) | curvo | 144 | 1.443e-14 | 3.379e-15 | 0 |
| torus as NURBS | project point (mm) | curvo | 288 | 1.256e-7 | 3.539e-8 | 0 |
| torus as NURBS | project point (mm) | rust | 288 | 2.296e-14 | 6.993e-15 | 0 |
| torus as NURBS | ray first hit (mm) | rust | 144 | 5.360e-13 | 1.937e-14 | 0 |

| Surface | Operation | Method | ns per call |
| --- | --- | --- | --- |

