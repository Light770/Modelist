# Modelist Engineering Rules

Modelist is a local-first, web-native hybrid parametric fashion patternmaking CAD application. The authoritative architecture and algorithm blueprint lives in [docs/ALGORITHMS_SPEC.md](docs/ALGORITHMS_SPEC.md).

## 1. Architectural Boundaries
- **Astro + React Separation:** Astro renders static/SSR routes, docs, and catalog pages, mounting the CAD Studio as an isolated `client:only="react"` island. React manages UI chrome only (toolbars, inspectors, formula bar, tables, export dialogs).
- **Imperative Viewport Rule:** Never render live pattern geometry, Bézier handles, graded nests, or simulation meshes as React DOM/SVG nodes during interactive dragging. Render imperatively via Canvas2D/WebGL2/WebGPU consuming typed-array buffers from the WASM kernel inside `requestAnimationFrame`.
- **Single Computational Kernel:** All 2D geometry, constraint solving, seam allowance offsetting, grading, DXF/PDF generation, and meshing live in Rust (compiled to WASM via `wasm-bindgen` for web and native for optional Tauri desktop). Do not duplicate geometric math in TypeScript.
- **Strict 2D / 3D Decoupling:** Keep the 3D virtual-fit engine completely lazy-loaded and isolated from 2D drafting state. No 3D physics allocations or GPU pipelines may run while drafting in 2D.
- **Local-First Persistence:** All user projects serialize to an open, documented local-first `.modelist` format. Never lock reading or exporting user files behind cloud authentication.

## 2. Geometric & Numerical Conventions
- **Units & Precision:** Compute internally in `f64` millimeters (`mm`). Display units (`mm`, `cm`, `in`) convert strictly at the UI/export boundary (`1 in = 25.4 mm`).
- **Coordinate System & Winding:** Right-handed 2D drafting coordinates (`+X` right, `+Y` up). Outer seam loops must be **CCW**; internal cutouts/holes must be **CW**.
- **Single Source of Truth:** The **seamline (net pattern)** is canonical truth; cutlines (seam allowances), 3D meshes, and marker polygons are derived artifacts.
- **Deterministic IDs:** Assign stable UUIDs deterministically across undo/redo and parametric rebuilds so `StitchingGraph` links, notches, and grade rules remain intact.
- **Tolerances:** Geometry coincidence `1e-6 mm`; arc-length & seam-walking accuracy `< 0.01 mm`; flattening chord bound `< 0.05 mm`.

## 3. Coding, Dependencies & Testing
- **Crate & License Hygiene:** Verify open-source license compatibility (prefer `MIT` / `Apache-2.0`; explicitly flag `GPL` / `AGPL` or `MPL-2.0` crates like `jagua-rs`/`sparrow` before adding them to Cargo/npm manifests).
- **WASM / TS Contract:** Keep Rust-to-TypeScript serialized schemas and typed-array buffer layouts strictly typed and synchronized across worker boundaries.
- **Algorithmic Verification:** Every geometric or solver PR must include unit tests and `proptest` invariants verifying: no cutline self-intersections, arc-length inversion `< 0.01 mm`, dart pivot length conservation `< 1e-4 mm`, DAG acyclicity/determinism, and DXF round-trip fidelity.

## 4. Workspace Skills Index (`.agent/skills/`)
Load the relevant skill before working in its subsystem:
- `cad-geometry-kernel` — 2D B-Rep, Bézier arc-length math, seam walking, truing, darts, slash-and-spread, seam allowances, and apparel corners.
- `parametric-grading-solver` — Dependency DAG, safe expression evaluator, 2D constraint solver (LM/Dogleg), "silent math" capture, and RTW/MTM/hybrid grading.
- `apparel-interop-export` — DXF-AAMA / ASTM D6673-10 R12 export/import, Schneider curve fitting, OCG layered PDFs (tiled/A0/projector), and WASM marker nesting.
- `astro-react-cad-ui` — Astro/React island boundaries, imperative GPU canvas, WASM worker typed-array protocol, R-tree snapping, and CAD UX state machines.
- `cloth-sim-3d` — On-demand `spade` CDT meshing, WebGPU anisotropic XPBD cloth solver, avatar SDF/BVH collisions, and fit heatmaps.
