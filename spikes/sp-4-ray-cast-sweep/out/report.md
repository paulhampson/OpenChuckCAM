# SP-4 report

Release build, 4 threads available. Units mm and degrees.

## 1. Mesh error against the chord tolerance

Lobed part, 2000 through-axis rays. Error = mesh hit radius minus exact radius (inscribed mesh, so never positive).

| Chord tol | Triangles | Build (ms) | Mesh error min | Max ray time (us) |
| --- | --- | --- | --- | --- |
| 0.004 | 37392 | 24 | -0.01045 | 1.19 |
| 0.002 | 74472 | 48 | -0.00540 | 1.54 |
| 0.001 | 148470 | 100 | -0.00246 | 1.78 |
| 0.0005 | 296176 | 205 | -0.00129 | 2.27 |
| 0.00015 | 984060 | 727 | -0.00040 | 2.73 |

## 2. Accuracy against the exact surface (mesh chord tol 0.001)

For `NormalOffset` the error is instead the distance from the ball centre to the exact surface minus the radius (negative = gouge), because that centre is not on the tool-axis line. Otherwise error = tip height from the mesh method minus the exact drop on the analytic surface along the same line (negative = the tool sits deeper than the exact surface, a gouge). Stencil rings at 0.75 mm; refinement is a pattern search down to 0.02 mm.

| Part | Tool | Mode | Method | Poses | Err min | Err max | Mean rays | us per pose (1 thread) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| vase | ball | through-axis | StencilCoarse | 1000 | -0.05368 | -0.00104 | 66 | 80 |
| vase | ball | through-axis | StencilRefined | 1000 | -0.00195 | 0.00077 | 130 | 82 |
| vase | ball | through-axis | Climb | 1000 | -0.00195 | 0.00077 | 91 | 63 |
| vase | ball | through-axis | Distance | 1000 | -0.00195 | 0.00088 | 53 | 138 |
| vase | ball | normal-following | NormalOffset | 1000 | -0.00168 | 0.00071 | 2 | 3 |
| vase | ball | normal-following | StencilCoarse | 1000 | -0.05322 | -0.00116 | 67 | 56 |
| vase | ball | normal-following | StencilRefined | 1000 | -0.00196 | 0.00066 | 129 | 84 |
| vase | ball | normal-following | Climb | 1000 | -0.00196 | 0.00066 | 90 | 61 |
| vase | ball | normal-following | Distance | 1000 | -0.00195 | 0.00067 | 54 | 120 |
| vase | flat | through-axis | StencilCoarse | 1000 | -0.00931 | 0.00088 | 66 | 85 |
| vase | flat | through-axis | StencilRefined | 1000 | -0.00178 | 0.00102 | 214 | 130 |
| vase | flat | through-axis | Climb | 1000 | -0.00178 | 0.00102 | 180 | 104 |
| vase | flat | normal-following | StencilCoarse | 1000 | -0.00881 | 0.00040 | 67 | 52 |
| vase | flat | normal-following | StencilRefined | 1000 | -0.00199 | 0.00040 | 109 | 99 |
| vase | flat | normal-following | Climb | 1000 | -0.00199 | 0.00040 | 75 | 49 |
| lobed | ball | through-axis | StencilCoarse | 1000 | -0.05501 | -0.00089 | 66 | 51 |
| lobed | ball | through-axis | StencilRefined | 1000 | -0.00267 | 0.00107 | 136 | 90 |
| lobed | ball | through-axis | Climb | 1000 | -0.00267 | 0.00107 | 100 | 82 |
| lobed | ball | through-axis | Distance | 1000 | -0.00267 | 0.00108 | 53 | 168 |
| lobed | ball | normal-following | NormalOffset | 1000 | -0.00220 | 0.00113 | 2 | 4 |
| lobed | ball | normal-following | StencilCoarse | 1000 | -0.05397 | -0.00083 | 67 | 51 |
| lobed | ball | normal-following | StencilRefined | 1000 | -0.00250 | 0.00109 | 133 | 131 |
| lobed | ball | normal-following | Climb | 1000 | -0.00250 | 0.00109 | 91 | 77 |
| lobed | ball | normal-following | Distance | 1000 | -0.00250 | 0.00109 | 54 | 127 |
| lobed | flat | through-axis | StencilCoarse | 1000 | -0.01826 | 0.00079 | 66 | 81 |
| lobed | flat | through-axis | StencilRefined | 1000 | -0.00256 | 0.00112 | 137 | 152 |
| lobed | flat | through-axis | Climb | 1000 | -0.00256 | 0.00115 | 109 | 85 |
| lobed | flat | normal-following | StencilCoarse | 1000 | -0.01025 | 0.00136 | 67 | 61 |
| lobed | flat | normal-following | StencilRefined | 1000 | -0.00271 | 0.00136 | 189 | 125 |
| lobed | flat | normal-following | Climb | 1000 | -0.00271 | 0.00136 | 155 | 101 |

## 3. Tool angle (normal-following, lobed part)

| Quantity | Min | Max |
| --- | --- | --- |
| Mesh tool angle minus exact in-plane normal angle (deg) | -1.178 | 1.082 |
| Normal-following angle minus through-axis angle (deg) | -22.42 | 22.37 |
| Angle left between exact normal and tool axis, from the X slope (deg) | 0.27 | 25.17 |

Machine frame check at one pose: tool direction after A = psi is (0.0e0, 0.0e0, 1.000000); tip at machine X 22.666, Y -3.891, Z 14.123.

## 4. Runtime on the reference part (80 mm long, 0.1 mm X stations, 0.5 degree A steps = 720 x 800)

| Mesh tol | Triangles | Tool | Mode | Method | Poses | 1 thread (s) | 4 threads (s) | Mean rays |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.001 | 148470 | ball | through-axis | StencilRefined | 576000 | 47.9 | 13.8 | 136 |
| 0.001 | 148470 | ball | through-axis | Climb | 573075 | 33.6 | 9.5 | 99 |
| 0.001 | 148470 | ball | through-axis | Distance | 573075 | 67.0 | 20.0 | 53 |
| 0.001 | 148470 | flat | through-axis | Climb | 573075 | 36.7 | 10.0 | 108 |
| 0.001 | 148470 | flat | normal-following | StencilRefined | 573075 | 66.2 | 18.6 | 196 |
| 0.00015 | 984060 | ball | through-axis | Climb | 573068 | 39.4 | 10.8 | 99 |
| 0.00015 | 984060 | flat | through-axis | Climb | 573068 | 42.9 | 12.3 | 110 |

## 5. Determinism

FNV-1a hash of every pose's bits, lobed part, ball tool:

| Mode | Method | 1 thread | 2 threads | 3 threads | 4 threads | 7 threads |
| --- | --- | --- | --- | --- | --- | --- |
| through-axis | StencilRefined | 024e407a507e0843 | 024e407a507e0843 | 024e407a507e0843 | 024e407a507e0843 | 024e407a507e0843 |
| through-axis | Climb | 4d524a294b31102f | 4d524a294b31102f | 4d524a294b31102f | 4d524a294b31102f | 4d524a294b31102f |
| through-axis | Distance | fbfeacd0f910d2f9 | fbfeacd0f910d2f9 | fbfeacd0f910d2f9 | fbfeacd0f910d2f9 | fbfeacd0f910d2f9 |
| normal-following | StencilRefined | b71674df50e344cd | b71674df50e344cd | b71674df50e344cd | b71674df50e344cd | b71674df50e344cd |
| normal-following | Climb | 13fecc366e4064d0 | 13fecc366e4064d0 | 13fecc366e4064d0 | 13fecc366e4064d0 | 13fecc366e4064d0 |
| normal-following | Distance | 1f88ac3d2625cd90 | 1f88ac3d2625cd90 | 1f88ac3d2625cd90 | 1f88ac3d2625cd90 | 1f88ac3d2625cd90 |
