---
name: parametric-grading-solver
description: >-
  Guides implementation of Modelist's parametric dependency DAG, safe expression
  evaluator, 2D geometric constraint solver, visual-to-parametric recording, and
  RTW/MTM/hybrid grading engine. Use when building formula propagation,
  constraint solving, grading rules, or cross-size balance checks.
---

# Parametric & Grading Solver

Powers Modelist's hybrid "Silent Math" drafting and dual RTW/MTM grading system in Rust/WASM. Reference [docs/ALGORITHMS_SPEC.md](../../../docs/ALGORITHMS_SPEC.md) Sections 3 and 6.

## Core Architecture

Maintain strict separation between two solver layers:
1. **Layer A — Parametric DAG:** Scalar/expression dependency graph driven by `BodyMeasurements`, `EaseTable`, variables, and geometric queries.
2. **Layer B — 2D Geometric Constraint Solver (GCS):** Iterative numerical solver satisfying geometric relations (`Coincident`, `Perpendicular`, `TangentG1`, `EqualLength`, `Distance`, `Symmetric`).

## Implementation Steps

### 1. Parametric Dependency DAG & Expression VM
- Parse user and auto-generated formulas into a sandboxed AST (`+`, `-`, `*`, `/`, `min`, `max`, `clamp`, `sin`, `cos`, `tan`, `atan2`, `sqrt`, `hypot`, `if`, `@edge.len`).
- Maintain topological ordering via Kahn's algorithm; reject cycles immediately with the exact dependency cycle path.
- On parameter or slider updates, mark changed root nodes dirty and re-evaluate only the dirty reachable subgraph (`< 2 ms` budget).

### 2. 2D Geometric Constraint Solver (GCS)
- Partition active constraints into connected clusters; solve only clusters affected by dirty parameters or active cursor drags.
- Formulate scalar residual vector $\mathbf{r}(\mathbf{x}) = 0$ over free point coordinates and Bézier handle variables, solving via damped least-squares (**Levenberg-Marquardt / Dogleg**).
- Apply **soft-priority weights**:
  1. Hard anchors (`Coincident`, `Perpendicular` square-offs, `TangentG1`).
  2. Structural & seam-length constraints (`FormulaDistance`, `EqualLength`).
  3. Soft cursor drag targets `(mouse_x, mouse_y)` so direct manipulation never explodes.
- **Failure Isolation:** On over-constrained or conflicting inputs, keep the last valid geometry for that cluster and emit diagnostic markers (`Underconstrained`, `Overconstrained`, `Conflict`).

### 3. Visual-to-Parametric Recording
- Convert direct canvas edits (dragging edges, splitting curves, pivoting darts, slash-and-spread) into live `ParametricOp` nodes using ratios, offsets, or bound variables so edits survive upstream measurement changes.

### 4. Dual Grading Engine (RTW + MTM + Hybrid)
- **RTW Rule Grading:** Apply $(\Delta x, \Delta y)$ vectors at grade vertices and run **proportional Bézier handle scaling** (scale handle length by graded chord ratio $\rho = \|\mathbf{d}'\|/\|\mathbf{d}\|$ and rotate by chord delta $\Delta\phi$) to prevent curve kinks across extreme sizes.
- **MTM Multi-Size Grading:** Batch-evaluate the full Parametric DAG + GCS across every size column in `SizeTable` or client measurement profile.
- **Hybrid Differential:** Evaluate Parametric DAG first, then layer sparse per-vertex/per-size manual overrides $(\delta x, \delta y)$ on top.
- **Cross-Size QA:** Evaluate every `StitchingGraph` pair across all sizes simultaneously and flag out-of-tolerance ease drift.

## Validation

1. Test DAG determinism (dirty-subtree result matches full cold rebuild bit-for-bit) and cycle rejection.
2. Verify GCS convergence across square-offs, `EqualLength` armhole/sleeve pairs, and conflicting constraint graceful fallback.
3. Verify graded Bézier tangents across XXS–5XL nests show no inflection inversions or handle collapse, and verify cross-size ease matrix reports exact millimeter deltas.
