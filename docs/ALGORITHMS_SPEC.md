# Modelist — Algorithmic & Technical Specification

> **Status:** Draft v1.0 Architecture & Algorithm Specification  
> **Target Product:** Local-first, web-native hybrid parametric fashion patternmaking CAD with decoupled on-demand 3D simulation, publication-grade indie PDF output, and industrial DXF-AAMA/ASTM D6673 interoperability.

---

## 1. Product Vision & Build-vs-Basis Strategy

### 1.1 Design Thesis (Solving Industry Pain Points)
Modelist eliminates the forced trade-off between **fluid "digital paper" drafting** (CLO 3D, PAD System), **non-breaking parametric math** (Grafis, Seamly2D), **publication-ready pattern layout** (Illustrator/Affinity), **industrial factory precision** (Optitex, Gerber AccuMark), and **local-first ownership**:

1. **Visual-Parametric Hybrid ("Silent Math"):** Patternmakers manipulate points, walk seams, pivot darts, and slash-and-spread directly on canvas, while the engine records proportional rules and geometric constraints underneath.
2. **Decoupled 2D & 3D:** 2D drafting is lightweight, vector-crisp, and runs smoothly on any laptop; 3D cloth simulation runs strictly on demand in an isolated WebGPU/WASM module.
3. **Zero "Illustrator Tax":** Layered multi-size PDFs (A4/Letter tiled, A0 copyshop, and projector-native), clean variable seam allowances, and strict ASTM D6673 / AAMA DXF exports come directly from one canonical pattern model.
4. **Local-First & Open File Format:** Open `.modelist` archive (JSON + binary/asset blobs in a ZIP container) owned by the user via File System Access API / IndexedDB, plus optional Tauri desktop packaging reusing the exact same Rust core.

### 1.2 Evaluation of Existing Open-Source Bases

| Candidate Basis | Verdict | Architectural Rationale |
| :--- | :--- | :--- |
| **Seamly2D / Valentina** (`C++ / Qt`, `GPLv3`) | **Do NOT fork** *(reference only)* | Monolithic desktop GUI coupled to Qt widgets; sequential dialog-driven point graph breaks on organic edits; lacks native dart pivoting & slash-and-spread; `GPLv3` copyleft restricts licensing. **Reuse:** study `SeamlyME` measurement schemas and classical tailoring formula conventions. |
| **FreeSewing Core** (`JS / Node`, `MIT`) | **Do NOT use as CAD kernel** *(reference / plugin target)* | Batch functional generator (`draft() -> SVG`), not an interactive B-Rep CAD kernel with hit-testing, constraint solving, or direct manipulation. **Reuse:** reference measurement definitions, tiled/projector PDF layout UX, and optional procedural block plugin imports. |
| **Composable Rust → WASM Kernel + Astro/React Studio** | **Recommended Architecture** | Clean separation between an exact `f64` Rust geometric/parametric kernel compiled to WebAssembly (`wasm-bindgen`), an imperative GPU-accelerated 2D/3D viewport, and an Astro + React Studio shell. Compiles identically to Browser WASM and Native Desktop (Tauri). |

### 1.3 Recommended Open-Source Rust & Web Building Blocks

* **2D Analytic Curves & Path Math:** `kurbo` (`Apache-2.0 OR MIT`) — cubic/quadratic Béziers, circular arcs, arc-length parameterization, intersections, nearest-point queries.
* **2D Offsetting & Booleans:** `cavalier_contours` (`MIT`) — exact polyline + circular-arc (bulge) parallel offsetting and boolean operations with self-intersection pruning; optionally pair with `clipper2` where strict polygonal fallbacks are needed.
* **2D Spatial Indexing:** `rstar` (`MIT OR Apache-2.0`) — R*-tree for `< 1 ms` snap, hit-testing, and viewport culling.
* **2D GPU Path Tessellation:** `lyon` (`MIT OR Apache-2.0`) — fast path-to-triangle tessellation for WebGL2/WebGPU rendering.
* **2D Constrained Meshing (for 3D):** `spade` (`MIT OR Apache-2.0`) — Constrained Delaunay Triangulation (CDT) and Ruppert mesh refinement.
* **2D Irregular Marker Nesting:** `jagua-rs` + `sparrow` *(verify `MPL-2.0` / `Apache-2.0` licensing compatibility before commercial distribution)* — high-speed Collision Detection Engine (CDE) and strip-packing heuristics compiling to WASM.
* **Interop References:** `ezdxf` (`gerber_D6673` addon) and `ksons/astm-parser` for strict DXF-AAMA / ASTM D6673 compliance; `GarmentCode` for procedural sewing graphs and XPBD cloth solver pipelines.

### 1.4 Tech Stack Architecture (Astro + React + Canvas/WebGPU + Rust/WASM)

```text
+-------------------------------------------------------------------+
| Astro Shell (Static / SSR Pages, Docs, Library, Studio Route)     |
|   └── <StudioRoot client:only="react" />                          |
+-------------------------------------------------------------------+
| React Studio Chrome (Zustand/Signals, throttled UI state)         |
|   Toolbars | Piece Tree | Formula Bar | Grade Grid | Export Modal |
+-------------------------+-----------------------------------------+
| Viewport Bridge         | WASM Worker Pool (Web Workers)          |
| (Imperative 2D Canvas / |   ├── CadKernel (kurbo, rstar, GCS)     |
|  WebGL2 / WebGPU)       |   ├── Offset & Grading Engine           |
| Reads TypedArrays       |   ├── Marker Nesting Worker (jagua-rs)  |
| directly (0 React DOM)  |   └── Export Pipeline (PDF / DXF / SVG) |
+-------------------------+-----------------------------------------+
| On-Demand 3D Viewport (Lazy-loaded WebGPU XPBD / WASM Fallback)   |
+-------------------------------------------------------------------+
```

* **Astro Role:** Marketing site, documentation, pattern marketplace/library browser, and fast static shell. Mounts the interactive CAD editor as an isolated `client:only="react"` island.
* **React Role:** UI chrome only—dockable toolbars, property inspectors, formula editor, measurement tables, grade rule grid, and export dialogs. **Never** render live pattern contours, handles, or snaps as React DOM/SVG nodes during interactive dragging.
* **Imperative 2D Canvas:** Renders directly from WASM linear memory / transferable `Float32Array` & `Uint32Array` buffers inside `requestAnimationFrame` at 60–120 FPS.
* **Web Workers:** Heavy operations (marker nesting, full size-run DXF/PDF generation, multi-size re-evaluation) run off the main thread and stream progress updates back to the UI.

---

## 2. Canonical Data Model & Coordinate System

### 2.1 Geometric & Numerical Conventions
* **Internal Unit:** `f64` millimeters (`mm`). Imperial inches (`in`) and centimeters (`cm`) exist strictly as presentation/input transforms (`1 in = 25.4 mm` exact).
* **Coordinate Frame:** Right-handed 2D drafting space (`+X` right, `+Y` up; screen rendering applies Y-flip via viewport transform matrix).
* **Contour Winding:** Outer pattern boundaries are oriented **counter-clockwise (CCW)**; internal cutouts/holes are oriented **clockwise (CW)**.
* **Tolerances:**
  * Geometric coincidence epsilon: $\varepsilon_{\text{geo}} = 10^{-6}\text{ mm}$
  * Arc-length / walking-seam solver precision: $\varepsilon_{\text{len}} = 10^{-2}\text{ mm}$
  * Curve-to-polyline flattening chord tolerance (for nesting/DXF fallback): $\varepsilon_{\text{flat}} = 0.05\text{ mm}$
* **Single Source of Truth:** The **seamline (net pattern)** is canonical truth. The **cutline (seam allowance)**, **3D simulation mesh**, **PDF layouts**, and **DXF exports** are purely derived projections.
* **Deterministic Identity:** Every piece, vertex, edge, dart, notch, and constraint carries a stable UUID (`u128`) assigned deterministically by command replay so undo/redo, grading rules, and cross-piece stitching links never break when topology updates.

### 2.2 Apparel B-Rep & Topology Schema

```rust
pub struct PatternProject {
    pub id: EntityId,
    pub units_display: DisplayUnit,
    pub measurements: MeasurementProfile,
    pub ease_table: EaseTable,
    pub variables: Vec<ParametricVar>,
    pub size_table: SizeTable,
    pub pieces: Vec<PatternPiece>,
    pub stitching: StitchingGraph,
    pub history_ops: Vec<ParametricOp>,
}

pub struct PatternPiece {
    pub id: EntityId,
    pub name: String,
    pub cut_quantity: u16,
    pub mirror_fold: Option<EdgeId>,        // Symmetry / cut-on-fold axis
    pub grainline: GrainlineSpec,           // Anchor + angle + allowed rotation set
    pub seam_loop: ClosedPolyCurve,         // Canonical CCW seamline
    pub cutouts: Vec<ClosedPolyCurve>,      // CW internal cutouts
    pub darts: Vec<DartEntity>,             // First-class structural dart objects
    pub pleats: Vec<PleatEntity>,           // First-class pleat objects
    pub internal_lines: Vec<InternalCurve>, // Placement lines, buttonholes, fold lines
    pub drill_holes: Vec<DrillHole>,        // Dart tips, pocket corners, button centers
    pub notches: Vec<NotchAnchor>,          // Parametrically bound to seam_loop edges
    pub sa_spec: SeamAllowanceSpec,         // Per-edge widths & per-vertex corner types
    pub grade_rules: GradeRuleMap,          // Optional RTW (dx, dy) deltas & overrides
    pub fabric_override: Option<FabricId>,  // Shell, lining, interfacing, rib
}
```

### 2.3 Cross-Piece `StitchingGraph`
A garment is a topological graph where edges across pieces are explicitly paired:

* `StitchLink { id, edge_a, dir_a, range_a, edge_b, dir_b, range_b, ease_mm, seam_type, sa_match_rule }`
* Drives four subsystems from one source:
  1. **Live Seam-Length Balance Checks:** Instant diff badge (`+14.2 mm ease` on sleeve cap vs. armscye).
  2. **Seam Walking & Corner Truing:** Rigidly snaps `piece_b` onto `piece_a` along matching arc-length intervals.
  3. **Matched Seam Allowance Corners:** Computes complementary corner cut angles across joined pieces.
  4. **3D Virtual Sewing:** Emits 1:1 vertex-to-edge correspondence pairs for the XPBD solver without manual re-stitching.

### 2.4 Spatial Acceleration (`rstar`)
All vertices, curve AABBs, internal lines, and intersection candidates are indexed in a per-piece and workspace **R\*-tree** (`rstar`), keeping cursor snapping (`Endpoint`, `Midpoint`, `OnCurve`, `Perpendicular`, `Tangent`, `Intersection`) and marquee selection under `1 ms` even with 10,000+ entities.

---

## 3. Module 1 — Hybrid Visual-Parametric Solver

Modelist separates **numerical/formula propagation** from **geometric constraint satisfaction** while capturing visual edits cleanly.

### 3.1 Layer A: Parametric Dependency DAG
* **Inputs:** `BodyMeasurements` (e.g., `chest_circ`, `waist_circ`, `back_waist_len`, `armscye_depth`), `EaseTable`, custom variables, and geometric measurements (e.g., `@armscye_front.len + @armscye_back.len`).
* **Safe Expression Evaluator:** Custom lexer/parser compiling formulas into a deterministic stack-VM AST supporting arithmetic, `min/max`, `clamp`, `sin/cos/tan/atan2`, `sqrt`, `hypot`, conditional `if(cond, a, b)`, and entity property queries—zero `eval()` or arbitrary code execution.
* **Incremental Dirty-Subtree Evaluation:**
  1. Maintain topological order via **Kahn's algorithm** (rejecting cycles on insertion with clear dependency-path diagnostics).
  2. On parameter or slider drag, mark changed roots dirty and traverse only reachable downstream nodes.
  3. Target latency: `< 2 ms` per interactive drag tick.

### 3.2 Layer B: 2D Geometric Constraint Solver (GCS)
* **Supported Constraints:**
  * `Coincident(p1, p2)`, `Distance(p1, p2, expr)`, `Angle(e1, e2, expr)`
  * `Horizontal(e)`, `Vertical(e)`, `Perpendicular(e1, e2)` *(critical for 90° square-offs at CF/CB/hems)*
  * `TangentG1(c1, c2)`, `CurvatureG2(c1, c2)`
  * `EqualLength(e1, e2)` or `LengthRatio(e1, e2, ratio_or_ease)` *(auto-matches side seams and armscyes)*
  * `PointOnCurve(p, curve, u_or_s)`, `Symmetric(p1, p2, axis)`
* **Solver Algorithm:**
  1. **Cluster Decomposition:** Partition the constraint graph into independent connected components; solve only components touched by dirty variables or cursor drag.
  2. **Residual Vector Formulation:** Express each constraint as scalar residuals $r_k(\mathbf{x}) = 0$ where $\mathbf{x}$ packs free point coordinates and curve handle parameters.
  3. **Damped Least-Squares (Levenberg-Marquardt / Dogleg):** Minimize $\frac{1}{2} \|\mathbf{W} \mathbf{r}(\mathbf{x})\|^2$ using sparse/dense QR or LDLT solves of $(\mathbf{J}^T \mathbf{W} \mathbf{J} + \lambda \mathbf{I}) \Delta \mathbf{x} = -\mathbf{J}^T \mathbf{W} \mathbf{r}$.
  4. **Soft-Priority Hierarchy:**
     * *Priority 1 (Hard/High weight):* Fixed body anchors, `Coincident`, `Perpendicular` square-offs, `TangentG1`.
     * *Priority 2 (Medium weight):* Formula distances, `EqualLength` seam locks.
     * *Priority 3 (Soft):* User cursor drag target `(mouse_x, mouse_y)`, ensuring smooth, predictable dragging without solver explosion.

### 3.3 Visual-to-Parametric Recording ("Silent Math")
When a user performs a direct canvas action instead of typing a formula:
* Dragging a hemline down by `-35 mm` records `OffsetEdge(hem, -35 mm)` (or binds to a variable if snapped).
* Splitting a curve creates `SplitAtRatio(edge, t = 0.375)` or `SplitAtArcLength(edge, s = 120 mm)` selectable via a lightweight pill toggle (`Ratio` vs `Fixed mm` vs `Formula`).
* Moving a dart or running Slash-and-Spread appends a live **Parametric Operator Node** in the piece's history tree so changing bust size upstream re-executes the dart pivot and spread cleanly.

### 3.4 Graceful Degradation & Diagnostics
If an extreme measurement or contradictory constraint makes a cluster unsolvable ($\|\mathbf{r}\| > \varepsilon_{\text{tol}}$ or rank-deficient conflict):
* Isolate the failing component and retain the **last valid geometry** (never vanish or explode the entire pattern).
* Highlight conflicting constraints in amber/red with degree-of-freedom indicators (`Underconstrained`, `Overconstrained`, `Inconsistent`).

---

## 4. Module 2 — Curve Math, Truing & "Digital Paper" Pattern Engineering

### 4.1 Exact Arc-Length Parameterization (`kurbo`)
Cubic Bézier curves $C(t), t \in [0, 1]$ do not have linear speed $\|C'(t)\|$. All apparel operations (seam walking, notch placement, ease checks, grading handles) require arc-length precision `< 0.01 mm`:
1. **Forward Arc Length $L(t)$:** Evaluate $\int_0^t \|C'(\tau)\|\,d\tau$ using **5-to-7 point Gauss-Legendre quadrature** (subdividing at inflection/cusp points if curvature spikes).
2. **Inverse Arc Length $t(s)$:** Given target length $s \in [0, L(1)]$, solve $L(t) - s = 0$ via **Newton-Raphson** ($t_{n+1} = t_n - \frac{L(t_n) - s}{\|C'(t_n)\|}$), bracketed with **bisection fallback** whenever a step exits the bracket or derivative approaches zero.

### 4.2 Interactive Seam Walking & Corner Truing
* **Seam Walking:** To inspect how edge $C_B$ (e.g., sleeve cap) sews onto edge $C_A$ (e.g., armscye):
  * Parameterize both curves by normalized or eased arc length: $s_B = f(s_A)$ (incorporating distributed ease or fixed ease zones).
  * At cursor arc position $s_A$, sample point $P_A = C_A(t_A(s_A))$ and unit tangent $\hat{T}_A$, and $P_B = C_B(t_B(s_B))$ with flipped tangent $-\hat{T}_B$ (matching sewing direction).
  * Compute rigid 2D transform $\mathbf{M} \in SE(2)$ mapping $(P_B, -\hat{T}_B) \mapsto (P_A, \hat{T}_A)$ and render `piece_b` ghosted directly over `piece_a`—supporting rolling without mutating `piece_b`, plus one-click notch transfer across the contact point.
* **Corner Truing:** Temporarily rigidly align partner piece at shared vertex `v_join` (matching seamline tangent at the corner). Allow simultaneous editing of both incoming Bézier handles with `G1` collinearity lock so joined hemlines, waistlines, and necklines form a continuous smooth curve without dips or peaks.

### 4.3 First-Class Dart Manipulation & Dart Cap Reflection
A dart is modeled structurally, not as three unrelated lines:
```rust
pub struct DartEntity {
    pub id: EntityId,
    pub apex: Point2D,
    pub leg_a_end: EdgeAnchor,
    pub leg_b_end: EdgeAnchor,
    pub intake_angle_rad: f64,
    pub fold_direction: DartFoldDir, // TowardCF | TowardCB | TowardHem | TowardSide
    pub cap_style: DartCapStyle,     // FoldedReflection | Pointed | Cutout
}
```
* **Dart Pivot / Rotation Algorithm:**
  1. User specifies target seamline point $Q_{\text{new}}$ and rotation angle $\theta$ (up to full closure of source dart $\theta_{\max}$).
  2. Extract the ordered boundary chain $\Gamma$ between source dart leg and $Q_{\text{new}}$.
  3. Apply affine rotation $\mathbf{R}(\theta)$ centered at `apex` to every vertex, curve control point, notch, and internal line on $\Gamma$.
  4. Merge source dart legs if fully closed ($\theta = \theta_{\max}$) and heal tangent continuity, or shrink intake by $\theta$; split target edge at $Q_{\text{new}}$ and insert rotated legs.
  5. **Dart Cap Reflection:** To ensure the dart folds flat without pinching or falling short of the seamline when sewn:
     * Compute the fold bisector ray from `apex` toward the folded side (`fold_direction`).
     * Reflect the closed seamline segment spanning the dart mouth across the fold leg, then project back onto the open dart triangle to construct the exact peaked/stepped dart cap polygon.
* **Dart Splitting & Princess Line Conversion:** Partition $\theta = \sum \theta_i$ across multiple target points, or convert dart legs into two smooth `G1` princess seam segments spanning the apex while locking total left/right seam arc lengths equal.

### 4.4 Parametric Slash-and-Spread & Pleat Underlay
* **Inputs:** Base piece, ordered array of slash rays/segments $S_1 \dots S_k$, pivot mode (`EdgeHinge` vs. `ParallelShift`), and spread amounts $(\Delta_1 \dots \Delta_k)$ (angle or linear distance at edge).
* **Algorithm:**
  1. **Topological Split:** Intersect each $S_i$ with `seam_loop` to partition the piece into $k+1$ ordered sub-polygons $P_0, P_1, \dots, P_k$ (preserving `P_0` as anchored reference).
  2. **Cumulative Rigid Transform:** For $i = 1 \dots k$, accumulate rotation $\mathbf{R}(\alpha_i)$ around hinge $H_i$ (or translation $\mathbf{T}(\mathbf{v}_i)$ for parallel spread) and apply to $P_i \dots P_k$.
  3. **Contour Bridging:**
     * *Gather / Flare Mode:* Bridge each opened gap $(E_i^-, E_i^+)$ with a minimum-energy / `G1`-matched cubic Bézier inheriting incoming/outgoing boundary tangents at gap endpoints.
     * *Pleat Mode (`Knife`, `Box`, `Inverted`):* Compute exact geometric **pleat underlay reflection**—reflect the outer seamline boundary across both fold legs so the folded paper cut edge matches the garment silhouette when the pleat lies flat.
  4. **Internal Preservation:** Re-anchor notches, drill holes, and grainline inside their parent sub-polygons, and persist the entire operation as a parametric DAG node.

---

## 5. Module 3 — Seam Allowances, Apparel Corners & Markings

### 5.1 Variable-Width Parallel Offsetting
Because garments have varying allowances (`10 mm` side seams, `6 mm` collar, `30 mm` hem, `0 mm` on cut-on-fold edges), each edge $e_i$ carries `sa_width: f64`.
1. **Curve Subdivision & Offset:** Subdivide each `CubicBezier` adaptively via `kurbo` / biarc approximation to `< 0.05 mm` error, shift control/segment normals outward by $w_i$ along unit right-hand normals $\hat{\mathbf{n}}(t)$ (since outer loops are CCW).
2. **Self-Intersection Pruning (`cavalier_contours`):** Sharp concave curves (deep armscyes, tight necklines) produce offset swallowtails/loops where offset distance $w_i$ exceeds radius of curvature $R = 1/\kappa$. Run `cavalier_contours` winding-number pruning / plane-sweep loop trimming to guarantee a simple, non-self-intersecting outer cutline.

### 5.2 Apparel Corner Join Algorithms
Standard CAD miter/bevel breaks real garments. Modelist solves corner $(e_i, e_{i+1})$ at vertex $V$ according to `CornerRule`:

| Corner Type | Mathematical Rule | Use Case |
| :--- | :--- | :--- |
| `Miter(limit)` | Intersect offset lines $L_i, L_{i+1}$; clamp/switch to `Bevel` if miter extension $> \text{limit} \cdot \max(w_i, w_{i+1})$. | Standard external corners. |
| `Bevel` | Connect offset endpoints directly with a straight chord. | Acute points (collar tips). |
| `Square` | Cap each offset orthogonally at $V$ and join. | Boxy/block allowances. |
| `FoldedHem` | Reflect the incoming seam edge $e_i$'s offset tangent across the hem fold line (`e_{i+1}` seamline) and intersect with the hem cutline. Creates the characteristic **flared/dog-ear hem corner** so tapered sleeves and A-line skirts fold up cleanly without coming short. | Trousers, flared skirts, sleeve hems. |
| `MatchedSeam` | Query partner piece width $w_{\text{partner}}$ from `StitchingGraph`; angle the corner clip so cut corners meet flush when sewn together. | Set-in waistbands, yoke corners. |
| `DartCap` | Temporarily rotate/fold closed dart legs, compute continuous seamline offset across the folded edge, and unfold back into the cutline peak. | Waist, bust, and shoulder darts. |

### 5.3 Notch Projection & Validation
* **Projection:** Anchor notch $N$ at seamline parameter $s$ (either fixed `mm` or proportional ratio `t`), locate outward unit normal $\hat{\mathbf{n}}(s)$, and intersect ray $P(s) + \lambda \hat{\mathbf{n}}(s)$ with the derived cutline to place the notch tip and base cleanly on the cut edge.
* **Notch Geometries:** `Slit` (inward cut depth `< sa_width - 2 mm`), `VNotch`, `UNotch`, `TNotch`, `Castle`.
* **Safety Diagnostics:** Warn automatically if `sa_width == 0` (cut-on-fold), if `notch_depth >= sa_width`, or if two notches/corners overlap within `< 3 mm`.

---

## 6. Module 4 — Dual Grading Engine (RTW + MTM + Hybrid)

### 6.1 Three Interoperable Grading Modes
1. **Mode A — Rule-Based RTW Grading:**
   * Each grade point $v_j$ maps to a `GradeRule` table defining $(\Delta x_k, \Delta y_k)$ relative to base size $S_0$.
   * **Proportional Bézier Handle Scaling (Anti-Kink):** Naive handle translation or unscaled handles distort armholes at extreme sizes. Given edge endpoints $A \to B$ moving to $A' \to B'$:
     * Let chord vector $\mathbf{d} = B - A$, graded chord $\mathbf{d}' = B' - A'$, scale ratio $\rho = \|\mathbf{d}'\| / \|\mathbf{d}\|$, and chord rotation $\Delta\phi = \text{angle}(\mathbf{d}') - \text{angle}(\mathbf{d})$.
     * Transform handle offset vectors $\mathbf{h}_1, \mathbf{h}_2$ by scaling lengths by $\rho$ and rotating by $\Delta\phi$ (or interpolating tangent direction toward adjacent edge constraints), keeping curves smooth from XXS to 5XL.
2. **Mode B — Parametric MTM / Multi-Size Evaluation:**
   * Re-evaluate the full Parametric Dependency DAG + GCS across every size column in `SizeTable` (or custom client profiles).
   * Handles non-linear body proportion shifts (petite/tall vertical scaling, bust cup grading, posture adjustments) automatically without writing brittle XY rule tables.
3. **Mode C — Hybrid Differential Grading:**
   * Compute the base nest via Mode B (Parametric DAG), then apply sparse manual patternmaker tweaks $(\delta x_{j,k}, \delta y_{j,k})$ layered cleanly on top as additive overrides—surviving upstream edits.

### 6.2 Automated Cross-Size QA & Alignment
* **Cross-Size Balance Matrix:** Automatically evaluate every link in `StitchingGraph` across *all* sizes simultaneously (`Armhole vs. Sleeve Cap Ease`, `Side Seam Front vs. Back`, `Collar vs. Neckline`). Highlight any size where ease leaves `[ease_min, ease_max]`.
* **Nest Stacking Anchors:** Align displayed/exported graded nests around a selectable reference: `GrainlineMidpoint`, `BustApex`, `CenterBackNeck`, or custom user datum.

---

## 7. Module 5 — Automated Marker Making & 2D Irregular Nesting

### 7.1 Apparel Fabric Constraints
* **Grainline & Nap Rotation Sets:**
  * One-way nap / directional print / velvet: $\Theta = \{0^\circ\}$
  * Standard woven / symmetric grain: $\Theta = \{0^\circ, 180^\circ\}$
  * Bias cut: $\Theta = \{\pm 45^\circ\}$ or $\{\pm 45^\circ, \pm 135^\circ\}$
  * Interfacing / felt (non-directional): continuous or fine steps (e.g., $5^\circ$ increments).
* **Fold-Edge Locking:** Cut-on-fold pieces can lock directly along the fabric roll selvedge/fold edge (`y = 0` or `y = W_roll`) without unfolding.
* **Buffer & Flaw Zones:** Configurable inter-piece kerf/gap (`0–5 mm`), usable fabric width (minus selvedges), stripe/plaid repeat pitch snapping (optional phase constraint), and polygonal fabric defect exclusion zones.
* **Bundle Multipliers:** Nest multiple sizes (`1x S, 2x M, 2x L, 1x XL`) simultaneously with mirrored L/R pair enforcement.

### 7.2 Nesting Pipeline (`jagua-rs` + `sparrow` in Web Worker)
1. **Outward-Conservative Discretization:** Flatten every piece cutline into a strictly *outer-bounding* polygon (`max error <= 0.1 mm`) plus inflation by `kerf / 2` so packed pieces can never overlap in true curved geometry.
2. **Fast Deterministic Baseline (BLF):** Run a quick **Bottom-Left-Fill** pass (`< 100 ms`) for immediate visual feedback and yield estimation.
3. **State-of-the-Art Strip Packing (`sparrow` + `jagua-rs`):**
   * `jagua-rs` Collision Detection Engine (CDE) resolves millions of polygon-polygon and polygon-container queries per second using hierarchical bounding boxes + exact edge intersection tests.
   * `sparrow` runs **exploration + compression** phases asynchronously in a dedicated **Web Worker** with a configurable time/iteration budget, streaming progressively tighter layouts and live **fabric utilization %** back to the UI canvas without blocking drafting.

---

## 8. Module 6 — Export & Interop Pipelines (Indie PDF + Factory DXF)

### 8.1 Indie & Atelier Multi-Layer PDF Engine (PDF 1.7 + OCG)
* **Optional Content Groups (OCG):** Each size gets an independent toggleable PDF layer, alongside `Seamlines`, `Cutlines`, `Annotations & Grainlines`, and `Calibration Grid` layers.
* **Monochrome Distinguishability:** Auto-assign distinct stroke dash patterns and weights per size so patterns remain legible when printed in black & white.
* **A4 / US Letter Tiling Algorithm:**
  1. Compute optimal grid origin offset $(x_0, y_0)$ via a coarse-to-fine search minimizing **total non-empty pages** (avoiding sliver overlaps across sheet borders).
  2. Cull completely blank tiles automatically.
  3. Clip piece paths cleanly to each tile's printable rect + configurable overlap margin (`10–15 mm`), adding corner registration crosses/triangles, row/column IDs (`R2-C3`), cut/tape guides, a `100 mm × 100 mm` (or `4 in × 4 in`) calibration square on Page 1, and an overview **Assembly Map** page.
* **A0 Copyshop Export:** Single-page (or minimal multi-sheet) large-format artboards (`841 × 1189 mm` / `36 in` wide continuous roll).
* **Projector-Native Export:**
  * Auto-unfold `cut-on-fold` pieces around their symmetry axis so makers cut mirrored halves directly without flipping fabric.
  * Auto-orient pieces horizontally along grainlines.
  * Add generous pan margin (`>= 300 mm` around bounding artboard) so edge pieces can center under any ceiling projector.
  * High-contrast thick strokes (`2–3 pt`), crisp large labels placed inside piece centers, optional **dark-background inverted theme**, and `50 mm` / `1 in` calibration grid OCG layer.

### 8.2 Industrial DXF-AAMA / ASTM D6673-10 Importer & Exporter
Legacy factory CAD parsers (Gerber AccuMark, Lectra Modaris, Tukatech) reject modern DXF features:
* **Strict DXF R12 Compliance:**
  * Emit minimal `HEADER`, `TABLES`, `BLOCKS`, and `ENTITIES` sections using **DXF R12** entities (`LINE`, `POLYLINE`/`VERTEX`/`SEQEND`, `POINT`, `TEXT`). Omit `$ACADVER > AC1009`, `$MODEL_SPACE`, `LWPOLYLINE`, `SPLINE`, and unsupported extended headers.
  * Encapsulate each pattern piece inside one named `BLOCK` and insert instances in `ENTITIES`.
  * Map geometries strictly to **ASTM D6673 / AAMA** numerical layers (`Layer 1` Boundary, `Layer 2/3` Turn & Curve points, `Layer 4` Notches, `Layer 6` Mirror line, `Layer 7` Grainline, `Layer 8/11` Internal/Cutouts, `Layer 13` Drill holes, `Layer 14` Sew lines) + piece/notch metadata text tags and `.rul` grade rule sidecar where graded nests are exported.
* **Importer + Schneider Curve Fitting:**
  * When importing dense polygonal factory DXFs (`Layer 1` / `14`), identify corner turn points (`Layer 2`) vs. curve points (`Layer 3`), then run **Philip Schneider's algorithm** (chord-length parameterized cubic Bézier fitting with Newton-Raphson reparameterization and max-error split threshold `< 0.1 mm`) to reconstruct clean, editable cubic Bézier splines instead of hundreds of uneditable line segments.

### 8.3 Vector & Tech-Pack Outputs
* **Exact-mm SVG:** Preserves `<g id="size-M">` groups, exact `viewBox` in millimeters (`width="...mm"`), CSS classes for seam/cut/internal lines, suitable for laser cutters, plotters, or web previews.
* **POM & Tech Pack Matrix:** Evaluates user-defined Points of Measurement (`Chest 1" below armhole`, `CB Length`, `Sleeve Length`) across every size in the table and exports structured JSON, CSV, and printable PDF spec sheets.

---

## 9. Module 7 — Decoupled On-Demand 3D Virtual Fit (Phase 3)

### 9.1 Strict Architectural Isolation
* Zero 3D physics state or GPU textures are allocated during 2D drafting.
* Clicking **"Preview in 3D"** snapshots the current `PatternProject`, meshes only dirty pieces, compiles the `StitchingGraph`, and boots the WebGPU compute pipeline (with a single-threaded/worker WASM CPU fallback for browsers without WebGPU).

### 9.2 Panel Meshing (`spade` Constrained Delaunay Triangulation)
1. Discretize each piece's `seam_loop` (note: **seamline**, not cutline—seam allowances do not hang outside sewn seams unless explicitly simulated as raw-edge styling) plus internal structural lines, dart legs, and stitch endpoints.
2. Insert constrained edges into `spade` CDT and run **Ruppert's Delaunay refinement** (`target edge length: 5–15 mm`, `minimum angle >= 25°`) to avoid ill-conditioned sliver triangles that destabilize XPBD.
3. Attach per-triangle UV coordinates `(u_2d, v_2d)` aligned to the piece's **grainline direction** $(\hat{\mathbf{g}}_{\text{warp}}, \hat{\mathbf{g}}_{\text{weft}})$ so anisotropic fabric stiffness works regardless of 2D sheet rotation.

### 9.3 Parametric Avatar & Virtual Arrangement
* **Parametric Mannequin:** Blendshape/rigged body mesh driven directly by `BodyMeasurements`, generating an analytical/voxel **Signed Distance Field (SDF)** + BVH surface mesh for fast continuous collision detection.
* **Initial 3D Arrangement:** Map each piece onto cylindrical/ellipsoidal body zones (`FrontTorso`, `BackTorso`, `LeftSleeve`, `RightLeg`, etc.) surrounding the avatar, keeping initial poses collision-free while bringing paired seam edges within pull range.
* **Virtual Sewing Constraints:** Sample paired `(edge_a, edge_b)` links from `StitchingGraph` at matching normalized/eased arc-length stations and emit dynamic XPBD distance constraints (`compliance` ramping down smoothly over the first 30–60 simulation substeps to avoid explosive pulls).

### 9.4 Anisotropic XPBD Cloth Solver (WebGPU Compute Shaders)
Extended Position-Based Dynamics (XPBD) achieves stable, substep-independent cloth behavior by converting physical material moduli into inverse stiffness **compliance** $\alpha = \tilde{\alpha} / \Delta t^2$:

* **Solver Loop (`N_substeps = 8..16` per 60 Hz frame):**
  1. Predict positions $\tilde{\mathbf{x}}_i = \mathbf{x}_i + \Delta t\,\mathbf{v}_i + \Delta t^2\,\mathbf{g}$.
  2. Reset Lagrange multipliers $\lambda_c \leftarrow 0$.
  3. Solve constraints via **Graph-Colored Parallel Gauss-Seidel** (partition mesh triangles/edges into non-overlapping color sets so WebGPU workgroups update positions race-free without atomic contention):
     * **Anisotropic Triangle Strain (St. Venant-Kirchhoff / Green Strain):** Compute deformation gradient $\mathbf{F}$ from rest 2D UV triangle to 3D triangle; penalize warp stretch ($\epsilon_{uu}$), weft stretch ($\epsilon_{vv}$), and bias shear ($\epsilon_{uv}$) independently using real fabric parameters (`E_warp`, `E_weft`, `G_shear`, Poisson ratio $\nu$).
     * **Anisotropic Dihedral Bending:** Across adjacent triangle pairs sharing an edge, enforce rest dihedral angle $\theta_0$ weighted by warp vs. weft alignment relative to hinge orientation.
     * **Seam & Interfacing Modifiers:** Apply stiffer bending compliance along sewn seams, optional elastic gathering shrinkage ratios (rest length factor `< 1.0`), and fused interfacing zones.
  4. **Collision Resolution:**
     * *Cloth–Avatar:* Query avatar SDF $\phi(\mathbf{x})$ + BVH closest-point normal + Coulomb friction.
     * *Cloth–Cloth Self-Collision:* Build uniform **spatial hash grid** on GPU each substep; resolve triangle-point / edge-edge proximity within `2 * thickness` + friction + garment layer order (`Underwear < Shirt < Jacket`).
  5. Update velocities $\mathbf{v}_i = (\mathbf{x}_i - \mathbf{x}_i^{\text{old}}) / \Delta t$ with Rayleigh/global air damping.

### 9.5 Fit Diagnostic Shaders
Compute per-triangle and per-vertex metrics in GPU buffers and render directly via fragment shaders:
* **Strain Map (`%`):** Elongation along principal warp/weft axes.
* **Stress / Tension Map (`kPa`):** Internal force magnitude highlighting tightness across bust, hip, or shoulder blades.
* **Ease / Clearance Map (`mm`):** Signed distance $\phi(\mathbf{x})$ from cloth vertex to avatar SDF (`Red < 0 mm` tight compression, `Green 10–40 mm` tailored ease, `Blue > 60 mm` loose drape).

---

## 10. Phased Roadmap & Algorithmic Verification Invariants

### 10.1 Phased Delivery Plan
* **Phase 1 — Precision 2D Hybrid CAD MVP:**
  * Rust/WASM kernel (`kurbo`, `cavalier_contours`, `rstar`), canonical `.modelist` schema, Parametric DAG + GCS solver, arc-length Béziers, seam walking & corner truing, dart pivoting/cap reflection, slash-and-spread, variable SA + apparel corners, dual grading (RTW + MTM), Astro + React Studio + Canvas2D/WebGL2 viewport, layered PDF (A4 tiled / A0 / projector) + SVG export.
* **Phase 2 — Industrial Interop & Production:**
  * Strict DXF-AAMA / ASTM D6673-10 exporter + importer (with Schneider curve refitting & `.rul` sidecars), automated cross-size QA matrix, POM spec sheets, Web Worker irregular marker nesting (`jagua-rs` + `sparrow`), optional Tauri desktop wrapper.
* **Phase 3 — Decoupled On-Demand 3D Virtual Fit:**
  * `spade` CDT panel meshing, parametric avatar + SDF/BVH, automated `StitchingGraph` sewing, WebGPU anisotropic XPBD compute solver (+ WASM fallback), strain/tension/ease diagnostic heatmaps.

### 10.2 Mandatory Verification Invariants
Every algorithmic module must ship with deterministic unit tests and property-based fuzz tests (`proptest` in Rust):

1. **Arc-Length Accuracy:** $\max |L(t(s)) - s| < 0.01\text{ mm}$ across arbitrary valid cubic Bézier curves including inflection points.
2. **Dart Pivot Conservation:** Total seamline perimeter length and corner angles outside the pivoted sector are conserved within `< 10^{-4}\text{ mm}` after arbitrary pivot rotations.
3. **Cutline Topological Validity:** Derived seam-allowance contours have zero self-intersections, CCW outer winding, and valid miter/fold caps across acute (`15°`), right (`90°`), and obtuse (`165°`) corners.
4. **Parametric DAG Determinism & Acyclicity:** Evaluating the DAG in topological order yields bit-identical geometry regardless of dirty-subtree origin; cyclic expressions are rejected cleanly before mutation.
5. **Grading Smoothness:** Cross-size curves exhibit no tangent reversals or unexpected kinks; `StitchingGraph` ease deltas compute deterministically across all sizes.
6. **DXF Round-Trip Fidelity:** Exporting a pattern piece to DXF-AAMA / ASTM D6673 and re-importing conserves polygon area within `< 0.1%`, grainline angle within `< 0.01°`, and notch count/positions identically.
