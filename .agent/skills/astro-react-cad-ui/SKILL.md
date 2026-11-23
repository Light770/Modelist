---
name: astro-react-cad-ui
description: >-
  Guides development of Modelist's Astro outer shell, React Studio chrome,
  imperative 2D Canvas/WebGL2/WebGPU viewport, WASM Worker typed-array buffer
  bridge, R-tree snapping UX, and undo/redo command history. Use when building
  frontend components, viewport rendering, tool state machines, or worker IPC.
---

# Astro & React CAD Studio UI

Connects Modelist's Astro + React frontend cleanly to the Rust/WASM CAD kernel while maintaining 60–120 FPS interactive drafting performance. Reference [docs/ALGORITHMS_SPEC.md](../../../docs/ALGORITHMS_SPEC.md) Sections 1.4 and 2.4.

## Core Boundaries

1. **Astro Outer Shell:** Static pages, documentation, templates, and pattern catalog live in `.astro` components. Mount the CAD editor via `<StudioRoot client:only="react" />`.
2. **React Chrome Only:** React renders sidebars, dockable toolbars, inspectors, formula bars, measurement grids, and export modals—subscribing only to throttled UI state slices.
3. **Never Render CAD Geometry in React DOM/SVG:** Do not map pattern contours, Bézier handles, graded nests, or seam ticks into JSX `<svg>` or HTML nodes during interaction.

## Implementation Steps

### 1. Imperative 2D Viewport & Typed-Array Bridge
- Execute WASM kernel commands inside a Web Worker (or shared WASM instance) that packs tessellated fills, strokes, handles, guides, and highlight buffers into contiguous `Float32Array` / `Uint32Array` views.
- Transfer or share buffers cleanly into an imperative Canvas2D / WebGL2 / WebGPU viewport controller running a dirty-flagged `requestAnimationFrame` loop.
- Maintain camera pan/zoom matrices purely in the viewport controller so pan/zoom runs at full refresh rate with zero WASM or React round-trips.

### 2. R-Tree Hit-Testing & "Digital Paper" Snapping
- Send cursor world coordinates `(wx, wy)` and tolerance radius (`px_tol / zoom`) to the kernel's `rstar` R*-tree index.
- Resolve snap priority cleanly: `Endpoint` > `Intersection` > `Midpoint` > `Perpendicular` > `Tangent` > `OnCurve` > `Grid`, rendering a transient snap badge + alignment guide directly on the overlay canvas.
- Support direct-input **Precision HUD** (typing `120` or `45deg` mid-drag locks exact length/angle constraints immediately).

### 3. Tool State Machines & Command History
- Model every tool (`Select`, `DraftPen`, `WalkSeam`, `TrueCorner`, `PivotDart`, `SlashSpread`, `Notch`, `Measure`) as an explicit finite state machine (`Idle -> Hover -> Dragging/Preview -> Commit | Cancel`).
- Keep live drag previews ephemeral; commit a deterministic `Command` (with undo/redo delta or `ParametricOp`) only on pointer release/confirm.

### 4. Local-First File Workflows
- Persist open `.modelist` archives via browser File System Access API (`showSaveFilePicker` / `showOpenFilePicker`) with IndexedDB autosave recovery snapshots and zero mandatory cloud dependency.

## Validation

1. Profile a continuous Bézier handle or dart-pivot drag on a multi-piece graded project with React DevTools Profiler: verify **zero** React component re-renders per pointer-move frame (aside from throttled numeric HUD coordinates if displayed).
2. Verify `Esc` / right-click cancel cleanly aborts mid-tool state without mutating document history or leaking preview buffers.
3. Verify undo/redo across structural commands (`PivotDart`, `SlashSpread`, `SplitEdge`) restores identical UUIDs and `StitchingGraph` links.
