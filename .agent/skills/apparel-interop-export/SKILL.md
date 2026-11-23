---
name: apparel-interop-export
description: >-
  Guides implementation of industrial DXF-AAMA and ASTM D6673-10 import/export,
  Schneider Bezier curve refitting, multi-layer OCG PDF generation (A4/Letter
  tiled, A0 copyshop, and projector mode), exact-mm SVG, POM spec sheets, and
  WASM 2D irregular marker nesting. Use when working on file interchange,
  printing/projector exports, or fabric nesting.
---

# Apparel Interop, Export & Marker Nesting

Handles Modelist's industrial CAD exchange, publication-ready indie PDFs, and automated fabric marker nesting. Reference [docs/ALGORITHMS_SPEC.md](../../../docs/ALGORITHMS_SPEC.md) Sections 7 and 8, plus [astm-d6673-layers.md](./references/astm-d6673-layers.md).

## Implementation Steps

### 1. Industrial DXF-AAMA / ASTM D6673-10
- **Consult [astm-d6673-layers.md](./references/astm-d6673-layers.md)** for exact layer indices, block structures, point codes, and legacy Gerber/Lectra parser quirks before editing DXF logic.
- **Strict DXF R12 Writer:** Emit one `BLOCK` per piece, omit `$MODEL_SPACE` and post-R12 entities (`LWPOLYLINE`, `SPLINE`), map boundaries/internals/notches/grainlines/drill holes/sew lines strictly to ASTM layer numbers, and generate `.rul` sidecars for graded nests.
- **Importer & Curve Refitting:** Separate turn points (`Layer 2`) from curve points (`Layer 3`) and run **Philip Schneider's cubic Bézier fitting** (`max error < 0.1 mm`) so imported factory polylines become smooth, editable splines.

### 2. Indie & Atelier PDF Engine (PDF 1.7 + OCG)
- Emit toggleable **Optional Content Groups (OCG)** per size plus `Seamlines`, `Cutlines`, `Annotations`, and `Calibration Grid`, using distinct monochrome stroke dash styles per size.
- **Tiled Home Print (A4 / US Letter):** Search grid origin $(x_0, y_0)$ to minimize non-empty sheets, cull blank tiles, clip paths with overlap margins, draw registration crosses, row/column IDs, a `100 mm` / `4 in` calibration square, and an assembly map page.
- **A0 & Projector Mode:** For projector exports, auto-unfold `cut-on-fold` pieces around symmetry axes, orient horizontally along grainlines, add `>= 300 mm` outer pan margins, use high-contrast `2–3 pt` strokes with optional dark-mode inversion, and embed a calibration grid OCG.

### 3. Exact-mm SVG & POM Spec Sheets
- Emit SVG with physical `mm` dimensions (`width`/`height` + matching `viewBox`) and grouped `<g>` layers per piece/size.
- Compute Points of Measurement (POM) dynamically across all sizes to export JSON/CSV/PDF spec tables.

### 4. Automated Marker Nesting (`jagua-rs` + `sparrow` Worker)
- Verify `MPL-2.0` / `Apache-2.0` license compatibility before bundling nesting crates.
- Discretize curved cutlines into strictly outward-bounding polygons (`<= 0.1 mm` error + `kerf / 2` padding) and enforce fabric grain rotation sets (`{0°}`, `{0°, 180°}`, `{±45°}`, or free), selvedge/fold locking, bundle counts, and flaw exclusion polygons.
- Run fast Bottom-Left-Fill (BLF) for immediate preview, then stream progressive exploration/compression layouts and utilization `%` from a dedicated **Web Worker**.

## Validation

1. Round-trip test DXF exports through an independent parser (e.g., `ezdxf` / `astm-parser`), verifying piece count, polygon area (`< 0.1%` delta), grainline angle (`< 0.01°`), and notch preservation.
2. Verify PDF OCG toggles, exact `100 mm` calibration scale at `100%` print zoom, and zero dropped segments across tile margins.
3. Assert nested marker outputs have zero polygon-polygon intersections and zero boundary or flaw-zone violations.
