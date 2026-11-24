---
name: cad-geometry-kernel
description: >-
  Guides implementation and modification of Modelist's Rust/WASM 2D apparel CAD
  geometry kernel, including half-edge topology, Bezier arc-length math, seam
  walking, corner truing, dart pivoting, slash-and-spread, and seam allowance
  offsetting. Use when building or debugging 2D pattern geometry primitives or
  pattern engineering operators.
---

# CAD Geometry Kernel

Implements exact `f64` millimeter 2D apparel geometry in Rust compiled to WebAssembly (`kurbo`, `cavalier_contours`, `rstar`). Reference [docs/ALGORITHMS_SPEC.md](../../../docs/ALGORITHMS_SPEC.md) Sections 2, 4, and 5 for full specifications.

## Core Principles

1. **Seamline Is Canonical:** Store `seam_loop` (CCW) and `cutouts` (CW) as primary geometry; compute cutlines (seam allowances) purely as derived outputs.
2. **Preserve Parametric Identity:** Never discard deterministic UUIDs on edge splits, dart pivots, or slash-and-spread steps; map child IDs deterministically from parent IDs and operator indices.
3. **Analytic Over Polygonal:** Keep lines, circular arcs, and cubic Béziers analytical (`kurbo`) throughout drafting and truing; flatten conservatively only for GPU tessellation, nesting, or DXF fallback.

## Implementation Steps

### 1. Exact Arc-Length Parameterization
- Compute forward arc length $L(t)$ via 5–7 point Gauss-Legendre quadrature (subdividing at curvature extrema/inflections).
- Invert $t(s)$ using Newton-Raphson ($t_{n+1} = t_n - (L(t_n) - s)/\|C'(t_n)\|$) guarded by bisection bracketing until $|L(t) - s| < 0.01\text{ mm}$.

### 2. Interdependent Seam Interfaces, Walking & Corner Truing
- Always reference seams via deterministic edge UUIDs (`EdgeRef { piece_id, edge_id, dir }`) grouped into `SeamInterface`s and connected by `SeamBinding`s — never brittle `edge_idx: usize` links.
- Run `solve_seam_bindings` after parametric rebuilds and manual edits so driver interfaces propagate ease/gather targets (`Exact`, `EaseMm`, `EasePct`, `Gather`) onto followers via safeguarded 1D secant/Newton arc-length iteration (`< 0.05 mm`).
- Project driver notches and multi-edge junctions onto followers by arc-length fraction (GarmentCode fraction projection) to auto-generate partner notches and walk multi-edge seams continuously.
- In corner truing mode, snap partner piece across the shared seam corner and constrain paired incoming Bézier handles to `G1` collinearity or `90°` square-offs.

### 3. Dart Manipulation & Slash-and-Spread
- Model darts as first-class `DartEntity` records (`apex`, legs, `intake_angle_rad`, `fold_direction`, `cap_style`).
- **Dart Pivot:** Rotate the boundary chain between old leg and target slash point by $\mathbf{R}(\theta)$ around `apex`, merge/heal old legs, insert split legs, and compute **dart cap reflection** across the fold bisector.
- **Slash-and-Spread:** Partition piece polygon along ordered slash segments $S_1 \dots S_k$, apply cumulative transforms (`EdgeHinge` rotation or `ParallelShift`), bridge gaps with `G1` cubic splines (or pleat underlay reflections for knife/box/inverted pleats), and re-anchor notches/grainlines.

### 4. Variable Seam Allowances & Apparel Corners
- Subdivide curves (`< 0.05 mm` chord bound), offset per-edge by `sa_width` along outward right-hand normals, and prune raw self-intersections via `cavalier_contours`.
- Resolve corners explicitly by `CornerRule`: `Miter(limit)`, `Bevel`, `Square`, `FoldedHem` (reflect incoming side-seam edge across hem fold line), `MatchedSeam` (query partner SA width via `StitchingGraph`), and `DartCap`.
- Project seamline notches along outward normals to cutline and flag `sa_width == 0`, `notch_depth >= sa_width`, or `< 3 mm` crowding.

## Validation

Run Rust unit + `proptest` geometry checks after any kernel change:
1. Assert $|L(t(s)) - s| < 0.01\text{ mm}$ across randomly generated cubic Béziers.
2. Assert dart pivot conserves external perimeter length within `< 1e-4 mm`.
3. Assert derived cutline contours have zero self-intersections, CCW outer winding, and valid corner caps for angles from `15°` to `165°`.
4. Benchmark R*-tree (`rstar`) snap queries under `< 1 ms` on 10,000+ entities.
