---
name: cloth-sim-3d
description: >-
  Guides implementation of Modelist's decoupled on-demand 3D virtual fit engine,
  including Constrained Delaunay Triangulation (spade), parametric avatar SDFs,
  virtual sewing from StitchingGraph, anisotropic XPBD WebGPU compute shaders,
  and fit diagnostic heatmaps. Use when implementing or debugging 3D meshing or
  cloth simulation.
---

# On-Demand 3D Cloth Simulation

Implements Modelist's isolated Phase 3 virtual fitting room without adding runtime overhead to 2D drafting. Reference [docs/ALGORITHMS_SPEC.md](../../../docs/ALGORITHMS_SPEC.md) Section 9.

## Core Isolation Rule

- Lazy-load the 3D module, WebGPU pipelines, and avatar assets only when the user opens the 3D viewport.
- Never let 3D simulation meshes mutate 2D `seam_loop` geometry directly; surface fit issues via diagnostic maps and explicit 2D edit suggestions.

## Implementation Steps

### 1. Panel Meshing (`spade` CDT)
- Input each piece's **canonical `seam_loop`** (not cutline), plus internal fold/dart lines and `StitchingGraph` seam endpoints.
- Build Constrained Delaunay Triangulation (`spade`) and run Ruppert refinement targeting `5–15 mm` edge length and minimum triangle angle `>= 25°`.
- Compute per-triangle 2D UV coordinates aligned strictly to the piece's **grainline axes** $(\hat{\mathbf{g}}_{\text{warp}}, \hat{\mathbf{g}}_{\text{weft}})$ so anisotropic fabric stiffness rotates accurately with grainline orientation.

### 2. Avatar SDF & Virtual Sewing
- Drive the parametric mannequin from `BodyMeasurements` and build a fast Signed Distance Field (SDF) + BVH for collision queries.
- Place 2D panels onto body-zone bounding cylinders (`TorsoFront`, `TorsoBack`, `SleeveL/R`, etc.) around the avatar.
- Generate 1:1 sewing constraints from `StitchingGraph` matched by eased arc length, ramping sewing stiffness smoothly over the initial 30–60 substeps to prevent explosive forces.

### 3. Anisotropic XPBD Solver (WebGPU Compute + WASM Fallback)
- Run `8–16` substeps per 60 Hz frame with substep-invariant compliance $\alpha = \tilde{\alpha} / \Delta t^2$.
- Partition constraints via **graph coloring** so WebGPU compute shaders run parallel Gauss-Seidel passes without race conditions:
  - **Anisotropic Triangle Strain:** Penalize warp ($\epsilon_{uu}$), weft ($\epsilon_{vv}$), and shear ($\epsilon_{uv}$) separately via StVK/Green strain.
  - **Anisotropic Dihedral Bending:** Enforce rest fold angles weighted by edge orientation relative to warp/weft.
  - **Seam & Interfacing Overrides:** Increase bending stiffness along sewn seams and interfacing zones; apply shrinkage factors (`< 1.0`) on gathered/elastic seams.
- Resolve **Cloth–Avatar** collisions via SDF + Coulomb friction and **Cloth–Cloth** self-collisions via a GPU uniform spatial hash grid respecting garment layer order.
- Maintain a clean Rust/WASM CPU XPBD fallback sharing identical constraint math when WebGPU is unavailable.

### 4. Fit Diagnostic Shaders
- Compute and visualize three per-vertex/per-triangle heatmaps: **Principal Strain (%)**, **Tension Stress (kPa)**, and **Ease / Avatar Clearance (mm)** (`Red < 0 mm`, `Green 10–40 mm`, `Blue > 60 mm`).

## Validation

1. Verify CDT meshes contain zero degenerate triangles (`min angle >= 25°`) and conserve total 2D surface area (`< 0.5%` delta).
2. Assert XPBD equilibrium rest-shape convergence matches between WebGPU compute and WASM fallback on a standard woven bodice block test scene.
3. Verify closing the 3D panel releases all WebGPU buffers/pipelines and restores idle memory usage.
