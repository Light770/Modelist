# ASTM D6673-10 / DXF-AAMA Reference

Specification summary for reading and writing industrial apparel CAD files compatible with Gerber AccuMark, Lectra Modaris, Optitex, Tukatech, and CLO 3D.

## 1. Strict DXF R12 Structural Rules

Many factory CAD importers (especially legacy Gerber parsers) reject modern AutoCAD DXF extensions. Enforce these rules on export:

- **DXF Version:** Emit strict **DXF R12 (`AC1009`)** ASCII syntax.
- **No Modern Entities:** Never emit `LWPOLYLINE`, `SPLINE`, `MTEXT`, or `HATCH` in AAMA/ASTM mode. Use `POLYLINE` + `VERTEX` + `SEQEND`, `LINE`, `POINT`, and `TEXT`.
- **Header & Tables Hygiene:** Keep `HEADER` minimal (`$ACADVER = AC1009`, `$INSUNITS` or explicit piece unit metadata); omit `$MODEL_SPACE` / `$PAPER_SPACE` blocks if targeting strict Gerber D6673 compatibility.
- **One Block Per Pattern Piece:** Define each pattern piece inside a dedicated `BLOCK` entity in the `BLOCKS` section, and place an `INSERT` reference for each block in `ENTITIES`.

## 2. Canonical Layer Table

| Layer | Purpose | Typical Entities | Notes |
| :--- | :--- | :--- | :--- |
| **`1`** | Piece Boundary (Cut Line) | Closed `POLYLINE` | Primary outer perimeter (cutline when SA exists, or net line if no SA). |
| **`2`** | Turn Points (Corner Vertices) | `POINT` | Marks sharp/graded corner vertices along boundary/sew lines. |
| **`3`** | Curve Points | `POINT` | Marks intermediate smooth-curve points between turn points (used during Schneider refitting). |
| **`4`** | Notches | `LINE` / `POINT` | Placed on boundary (`Layer 1`) or sew line (`Layer 14`); supports Slit, V, T, U conventions via geometry/metadata. |
| **`5`** | Stripe / Plaid Reference | `LINE` / `POINT` | Optional fabric alignment markers. |
| **`6`** | Mirror / Fold Line | `LINE` | Symmetry axis for cut-on-fold pieces. |
| **`7`** | Grain Line | `LINE` | Defines warp/grain orientation inside piece block. |
| **`8`** | Internal Lines (Draw / Placement) | `POLYLINE` / `LINE` | Dart stitch lines, pocket placement, fold guides, trim markings. |
| **`11`** | Internal Cutouts | Closed `POLYLINE` | Interior holes/slits cut by CNC knife. |
| **`13`** | Drill Holes / Punch Marks | `POINT` | Dart apex punch holes (`10–15 mm` inset convention configurable), button/pocket corners. |
| **`14`** | Sew Lines (Net Seamline) | Closed `POLYLINE` | Explicit net stitching contour when distinct from cut boundary (`Layer 1`). |
| **`15`** | Piece Metadata / Annotations | `TEXT` | Piece name, cut quantity, size label, style code, fabric code. |

*(Note: Graded multi-size exports may place size-specific boundary variants on numbered offset layers or ship an accompanying ANSI/AAMA `.rul` rule table matching vertex indices on `Layer 1`/`14`.)*

## 3. Importer Recovery Rules

1. **Fragment Chaining:** Legacy exporters frequently emit boundary segments as disjoint `LINE` or open `POLYLINE` fragments instead of a single closed loop. Chain endpoints within `1e-3 mm` tolerance into a closed loop and normalize winding (`CCW` outer, `CW` cutouts).
2. **Turn vs. Curve Classification:** Partition boundary vertices using `Layer 2` (hard corners / grade points) and `Layer 3` (smooth curve samples). Fit cubic Béziers via Schneider's algorithm strictly across spans bounded by turn points so corners stay crisp.
3. **Unit Auto-Detection:** Factory DXFs often lack `$INSUNITS` or rely on `TEXT` comments (`UNITS: METRIC` vs `UNITS: ENGLISH`). Inspect bounding-box heuristics + header metadata and let users override `mm` vs `in` on import if ambiguous.
