# SP-4 report

Release build, 4 threads available. Units mm and degrees.

## 1. Mesh error against the chord tolerance

Lobed part, 2000 through-axis rays. Error = mesh hit radius minus exact radius (inscribed mesh, so never positive).

| Chord tol | Triangles | Build (ms) | Mesh error min | Max ray time (us) |
| --- | --- | --- | --- | --- |
| 0.004 | 37392 | 26 | -0.01045 | 0.94 |
| 0.002 | 74472 | 53 | -0.00540 | 1.24 |
| 0.001 | 148470 | 87 | -0.00246 | 1.72 |
| 0.0005 | 296176 | 189 | -0.00129 | 1.93 |
| 0.00015 | 984060 | 673 | -0.00040 | 2.53 |

## 2. Accuracy against the exact surface (mesh chord tol 0.001)

Error = tip height from the mesh method minus the exact drop on the analytic surface along the same line (negative = the tool sits deeper than the exact surface, a gouge). Stencil rings at 0.75 mm; refinement is a pattern search down to 0.02 mm.

| Part | Tool | Mode | Method | Poses | Err min | Err max | Mean rays | us per pose (1 thread) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| vase | ball | through-axis | StencilCoarse | 1000 | -0.05368 | -0.00104 | 66 | 53 |
| vase | ball | through-axis | StencilRefined | 1000 | -0.00195 | 0.00077 | 130 | 97 |
| vase | ball | through-axis | Climb | 1000 | -0.00195 | 0.00077 | 91 | 68 |
| vase | ball | through-axis | Distance | 1000 | -0.00195 | 0.00088 | 53 | 155 |
| vase | ball | normal-following | NormalOffset | 1000 | -0.60034 | -0.01285 | 2 | 4 |
| vase | ball | normal-following | StencilCoarse | 1000 | -0.05322 | -0.00116 | 67 | 67 |
| vase | ball | normal-following | StencilRefined | 1000 | -0.00196 | 0.00066 | 129 | 88 |
| vase | ball | normal-following | Climb | 1000 | -0.00196 | 0.00066 | 90 | 58 |
| vase | ball | normal-following | Distance | 1000 | -0.00195 | 0.00067 | 54 | 119 |
| vase | flat | through-axis | StencilCoarse | 1000 | -0.00931 | 0.00088 | 66 | 51 |
| vase | flat | through-axis | StencilRefined | 1000 | -0.00178 | 0.00102 | 214 | 173 |
| vase | flat | through-axis | Climb | 1000 | -0.00178 | 0.00102 | 180 | 102 |
| vase | flat | normal-following | StencilCoarse | 1000 | -0.00881 | 0.00040 | 67 | 54 |
| vase | flat | normal-following | StencilRefined | 1000 | -0.00199 | 0.00040 | 109 | 84 |
| vase | flat | normal-following | Climb | 1000 | -0.00199 | 0.00040 | 75 | 56 |
| lobed | ball | through-axis | StencilCoarse | 1000 | -0.05501 | -0.00089 | 66 | 53 |
| lobed | ball | through-axis | StencilRefined | 1000 | -0.00267 | 0.00107 | 136 | 87 |
| lobed | ball | through-axis | Climb | 1000 | -0.00267 | 0.00107 | 100 | 81 |
| lobed | ball | through-axis | Distance | 1000 | -0.00267 | 0.00108 | 53 | 127 |
| lobed | ball | normal-following | NormalOffset | 1000 | -0.60079 | -0.01184 | 2 | 3 |
| lobed | ball | normal-following | StencilCoarse | 1000 | -0.05397 | -0.00083 | 67 | 49 |
| lobed | ball | normal-following | StencilRefined | 1000 | -0.00250 | 0.00109 | 133 | 120 |
| lobed | ball | normal-following | Climb | 1000 | -0.00250 | 0.00109 | 91 | 59 |
| lobed | ball | normal-following | Distance | 1000 | -0.00250 | 0.00109 | 54 | 128 |
| lobed | flat | through-axis | StencilCoarse | 1000 | -0.01826 | 0.00079 | 66 | 58 |
| lobed | flat | through-axis | StencilRefined | 1000 | -0.00256 | 0.00112 | 137 | 93 |
| lobed | flat | through-axis | Climb | 1000 | -0.00256 | 0.00115 | 109 | 74 |
| lobed | flat | normal-following | StencilCoarse | 1000 | -0.01025 | 0.00136 | 67 | 52 |
| lobed | flat | normal-following | StencilRefined | 1000 | -0.00271 | 0.00136 | 189 | 115 |
| lobed | flat | normal-following | Climb | 1000 | -0.00271 | 0.00136 | 155 | 108 |

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
| 0.001 | 148470 | ball | through-axis | StencilRefined | 576000 | 47.1 | 12.1 | 136 |
| 0.001 | 148470 | ball | through-axis | Climb | 573075 | 34.4 | 9.8 | 99 |
| 0.001 | 148470 | ball | through-axis | Distance | 573075 | 68.1 | 20.0 | 53 |
| 0.001 | 148470 | flat | through-axis | Climb | 573075 | 36.0 | 10.1 | 108 |
| 0.001 | 148470 | flat | normal-following | StencilRefined | 573075 | 64.5 | 18.5 | 196 |
| 0.00015 | 984060 | ball | through-axis | Climb | 573068 | 39.8 | 11.9 | 99 |
| 0.00015 | 984060 | flat | through-axis | Climb | 573068 | 44.1 | 11.7 | 110 |

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
