use modelist_core::blocks::build_project_pieces;
use modelist_core::grading::{compute_cross_size_qa, evaluate_graded_nest, CrossSizeQaRow};
use modelist_core::interop::{
    compute_pdf_layout_plan, compute_pom_table, export_aama_rul, export_dxf_aama, export_pom_csv,
    export_svg_mm, import_dxf_aama, import_garmentcode_json, PomRow,
};
use modelist_core::model::{
    det_id, BodyMeasurements, CornerRule, EaseSpec, EdgeRef, EvaluatedPiece, FabricSpec,
    GradeMode, GrainRotationMode, HingeMode, NotchStyle, ParametricOp, ParametricVar,
    PatternProject, SeamBinding, SeamDriver, SeamInterface, SeamType, StackAnchor,
    StitchingGraph, PatternPiece, remap_entity_ids,
};
use modelist_core::container::{is_container, read_container, write_container, ContainerPart, WriteOptions};
use modelist_core::nesting::{compute_marker_plan, export_marker_svg, MarkerConfig};
use modelist_core::offset::evaluate_piece;
use modelist_core::ops::{make_rect_pattern_piece, SeamWalkFrame};
use modelist_core::render_buf::{
    build_render_buffers, CanvasLabel, EdgeDimLabel, PieceFill, RenderBuffers,
};
use modelist_core::seam::{
    auto_detect_seam_bindings, compute_binding_walk_frame, interface_total_length,
    synthesize_partner_notches,
};
use modelist_core::snap::{hit_test_pieces, SpatialSnapIndex};
use modelist_core::timeline::{
    build_timeline_entries, compute_reorder_range, patch_op_from_json, OpTimelineEntry,
};
use modelist_core::model::{
    fabric_bending_length_cm, fabric_role_key, is_fabric_variable, FabricFinish, FABRIC_PROPERTIES,
};
use modelist_sim3d::avatar::{AvatarBodyForm, AvatarPose};
use modelist_sim3d::material::{garment_role, MaterialSet};
use modelist_sim3d::xpbd::ClothSimSession;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct MeasurementItem {
    key: String,
    label: String,
    value_mm: f64,
    min_mm: f64,
    max_mm: f64,
}

#[derive(Serialize)]
struct EdgeInfo {
    edge_idx: usize,
    edge_id: String,
    label: String,
    length_mm: f64,
    sa_width_mm: f64,
    corner_rule: CornerRule,
    is_cubic: bool,
    is_fold: bool,
}

#[derive(Serialize)]
struct DartInfo {
    id: String,
    name: String,
    edge_idx: usize,
    intake_mm: f64,
    apex_x: f64,
    apex_y: f64,
}

#[derive(Serialize)]
struct PieceSummary {
    id: String,
    /// Garment (component) the piece belongs to.
    component_id: String,
    name: String,
    code: String,
    cut_quantity: u16,
    visible: bool,
    fabric_role: String,
    grain_rotation: GrainRotationMode,
    has_mirror_fold: bool,
    perimeter_mm: f64,
    area_mm2: f64,
    cut_area_mm2: f64,
    notch_count: usize,
    drill_hole_count: usize,
    warnings: Vec<String>,
    edges: Vec<EdgeInfo>,
    darts: Vec<DartInfo>,
}

#[derive(Serialize)]
struct EdgeRefInfo {
    piece_id: String,
    piece_code: String,
    edge_id: String,
    edge_idx: usize,
    edge_label: String,
    length_mm: f64,
    dir: i8,
}

#[derive(Serialize)]
struct StitchLinkSummary {
    id: String,
    label: String,
    interface_a_name: String,
    interface_b_name: String,
    edge_count_a: usize,
    edge_count_b: usize,
    edges_a: Vec<EdgeRefInfo>,
    edges_b: Vec<EdgeRefInfo>,
    len_a_mm: f64,
    len_b_mm: f64,
    ease_mm: f64,
    ease_target_mm: f64,
    ease: EaseSpec,
    driver: SeamDriver,
    seam_type: SeamType,
    auto_partner_notches: bool,
    status: String,
    color_idx: usize,
}

#[derive(Serialize)]
struct VarSummary {
    name: String,
    expr: String,
    description: String,
    value: Option<f64>,
    error: Option<String>,
}

#[derive(Serialize)]
struct FabricMetrics {
    /// Peirce bending length (cm) with the warp and with the weft bent.
    bending_length_warp_cm: f64,
    bending_length_weft_cm: f64,
    /// `fluid` | `soft` | `medium` | `crisp` | `stiff`
    drape: &'static str,
}

#[derive(Serialize)]
struct RoleFabricInfo {
    role: String,
    label: String,
    fabric: Option<FabricSpec>,
}

fn fabric_metrics(f: &FabricSpec) -> FabricMetrics {
    let warp = fabric_bending_length_cm(f.bend_warp_un_m, f.density_gsm);
    let weft = fabric_bending_length_cm(f.bend_weft_un_m, f.density_gsm);
    let c = 0.5 * (warp + weft);
    let drape = if c < 1.2 {
        "fluid"
    } else if c < 1.8 {
        "soft"
    } else if c < 2.5 {
        "medium"
    } else if c < 3.3 {
        "crisp"
    } else {
        "stiff"
    };
    FabricMetrics {
        bending_length_warp_cm: warp,
        bending_length_weft_cm: weft,
        drape,
    }
}

fn fabric_edited(f: &FabricSpec) -> bool {
    match FabricSpec::preset(&f.id) {
        Some(p) => FABRIC_PROPERTIES.iter().any(|spec| {
            (f.property(spec.key).unwrap_or(0.0) - p.property(spec.key).unwrap_or(0.0)).abs() > 1e-9
        }),
        None => true,
    }
}

/// Editable fabric properties (keys, units, ranges, help) for the fabric editor.
#[wasm_bindgen]
pub fn fabric_property_specs_json() -> String {
    serde_json::to_string(FABRIC_PROPERTIES).unwrap_or_else(|_| "[]".into())
}

#[derive(Serialize)]
struct ComponentSummary {
    id: String,
    name: String,
    template_id: String,
    layer: u8,
    visible: bool,
    visible_3d: bool,
    piece_count: usize,
}

#[derive(Serialize)]
struct UiSnapshot {
    project_name: String,
    /// Template of the active component.
    template_id: String,
    /// Garments of the project, in layer-independent creation order.
    components: Vec<ComponentSummary>,
    active_component_id: String,
    grade_mode: GradeMode,
    stack_anchor: StackAnchor,
    active_size: String,
    sizes: Vec<String>,
    visible_sizes: Vec<String>,
    fabric: FabricSpec,
    fabric_catalog: Vec<FabricSpec>,
    /// True when the shell fabric's properties differ from its preset.
    fabric_edited: bool,
    fabric_metrics: FabricMetrics,
    /// Non-shell piece roles in the garment and the fabric each one uses (None = shell).
    role_fabrics: Vec<RoleFabricInfo>,
    measurements: Vec<MeasurementItem>,
    ease: Vec<MeasurementItem>,
    variables: Vec<VarSummary>,
    pieces: Vec<PieceSummary>,
    stitching_links: Vec<StitchLinkSummary>,
    ops: Vec<OpTimelineEntry>,
    rollback_index: Option<usize>,
    qa_matrix: Vec<CrossSizeQaRow>,
    pom_rows: Vec<PomRow>,
    labels: Vec<CanvasLabel>,
    edge_dims: Vec<EdgeDimLabel>,
    piece_fills: Vec<PieceFill>,
    selected_piece_id: Option<String>,
    selected_edge_idx: Option<usize>,
    selected_binding_id: Option<String>,
    show_mirror_unfold: bool,
    sim3d_active: bool,
    can_undo: bool,
    can_redo: bool,
    solve_ms: f64,
}

#[wasm_bindgen]
pub struct ModelistSession {
    project: PatternProject,
    undo_stack: Vec<PatternProject>,
    redo_stack: Vec<PatternProject>,
    drag_base: Option<PatternProject>,
    drag_op_id: Option<Uuid>,
    evaluated: Vec<EvaluatedPiece>,
    stitching: StitchingGraph,
    /// Component index of each evaluated piece.
    piece_component: Vec<usize>,
    graded_nest: HashMap<String, Vec<EvaluatedPiece>>,
    var_values: HashMap<String, f64>,
    var_errors: HashMap<String, String>,
    snap_index: SpatialSnapIndex,
    render_buffers: RenderBuffers,
    selected_piece: Option<Uuid>,
    selected_edge: Option<(Uuid, usize)>,
    selected_binding: Option<Uuid>,
    draft_seam_a: Vec<EdgeRef>,
    draft_seam_b: Vec<EdgeRef>,
    walk_frame: Option<(SeamWalkFrame, Uuid)>,
    show_mirror_unfold: bool,
    /// Strictly lazy-loaded on-demand 3D cloth simulator (None during 2D drafting!)
    sim3d: Option<ClothSimSession>,
    /// Per-layer fingerprints of the inputs `sim3d` was dressed from (see `sim_layer_keys`).
    sim_keys: Vec<(u8, u64)>,
    /// Parts of the opened `.modelist` file this build does not interpret (kept for saving).
    container_extras: Vec<ContainerPart>,
    solve_ms: f64,
    op_counter: u64,
}

#[wasm_bindgen]
impl ModelistSession {
    #[wasm_bindgen(constructor)]
    pub fn new(template_id: &str) -> Self {
        let project = Self::make_project(template_id);
        let mut session = Self {
            project,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            drag_base: None,
            drag_op_id: None,
            evaluated: Vec::new(),
            stitching: StitchingGraph::default(),
            piece_component: Vec::new(),
            graded_nest: HashMap::new(),
            var_values: HashMap::new(),
            var_errors: HashMap::new(),
            snap_index: SpatialSnapIndex::default(),
            render_buffers: RenderBuffers::default(),
            selected_piece: None,
            selected_edge: None,
            selected_binding: None,
            draft_seam_a: Vec::new(),
            draft_seam_b: Vec::new(),
            walk_frame: None,
            show_mirror_unfold: true,
            sim3d: None,
            sim_keys: Vec::new(),
            container_extras: Vec::new(),
            solve_ms: 0.0,
            op_counter: 1,
        };
        session.recompute();
        if let Some(first) = session.evaluated.first() {
            session.selected_piece = Some(first.piece.id);
            session.rebuild_render_buffers();
        }
        session
    }

    pub fn load_template(&mut self, template_id: &str) {
        self.push_undo();
        self.project = Self::make_project(template_id);
        self.walk_frame = None;
        self.selected_edge = None;
        self.selected_binding = None;
        self.draft_seam_a.clear();
        self.draft_seam_b.clear();
        self.recompute();
        self.selected_piece = self.evaluated.first().map(|e| e.piece.id);
        self.rebuild_render_buffers();
    }

    pub fn set_project_name(&mut self, name: &str) {
        self.project.name = name.trim().to_string();
    }

    // --- Garments (components) ---

    /// Adds a garment drafted from a template, to the right of the others on the canvas and
    /// one layer above them in the 3D fit. It becomes the active garment. `name` (the template
    /// name in the UI language) is optional. Returns its id.
    pub fn add_component(&mut self, template_id: &str, name: &str) -> String {
        self.push_undo();
        let tmpl = modelist_core::library::resolve_template_id(template_id);
        let seq = self.free_component_seq(tmpl);
        let mut comp = modelist_core::library::new_component(tmpl, seq, modelist_core::model::Point2::ZERO);
        if !name.trim().is_empty() {
            comp.name = name.trim().to_string();
        }
        self.place_component(comp)
    }

    /// Adds a copy of a garment (same template, ease, formulas, fabrics and history) beside the
    /// others, one layer up. Returns the copy's id.
    pub fn duplicate_component(&mut self, component_id: &str) -> String {
        let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) else {
            return String::new();
        };
        self.push_undo();
        let src = self.project.components[ci].clone();
        let ns = format!("component:{}:{}", src.template_id, self.free_component_seq(&src.template_id));
        let mut copy = modelist_core::library::relocate_component(&src, &ns, &self.project.measurements);
        copy.name = format!("{} (2)", src.name);
        self.place_component(copy)
    }

    /// One garment as its own `.modelist` file (for the garment library or to share), on the
    /// current body. `thumbnail_png` may be empty.
    pub fn export_component_file(&self, component_id: &str, thumbnail_png: &[u8]) -> Result<Vec<u8>, JsValue> {
        let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) else {
            return Err(JsValue::from_str("unknown garment"));
        };
        let comp = self.project.components[ci].clone();
        let mut single = self.project.clone();
        single.id = det_id(&format!("garment-file:{}", comp.id));
        single.name = comp.name.clone();
        single.components = vec![comp];
        single.active_component = 0;
        write_container(
            &single,
            &WriteOptions {
                writer_version: env!("CARGO_PKG_VERSION"),
                thumbnail_png: (!thumbnail_png.is_empty()).then_some(thumbnail_png),
                ..Default::default()
            },
        )
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Adds the garments of a `.modelist` file (a saved garment, or every garment of another
    /// project) beside the others. They are refitted to this document's body. Returns the id of
    /// the last one added (it becomes the active garment).
    pub fn insert_component_file(&mut self, bytes: &[u8]) -> Result<String, JsValue> {
        let source = if is_container(bytes) {
            read_container(bytes).map_err(|e| JsValue::from_str(&e))?.project
        } else {
            let text = std::str::from_utf8(bytes).map_err(|_| JsValue::from_str("not a .modelist file"))?;
            PatternProject::from_json(text).map_err(|e| JsValue::from_str(&e))?
        };
        self.push_undo();
        let mut last = String::new();
        for mut comp in source.components {
            comp.normalize_fabrics();
            let ns = format!("component:{}:{}", comp.template_id, self.free_component_seq(&comp.template_id));
            let moved = modelist_core::library::relocate_component(&comp, &ns, &source.measurements);
            last = self.place_component(moved);
        }
        Ok(last)
    }

    /// Removes a garment (the last one cannot be removed).
    pub fn remove_component(&mut self, component_id: &str) {
        let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) else {
            return;
        };
        if self.project.components.len() <= 1 {
            return;
        }
        self.push_undo();
        self.project.components.remove(ci);
        let active = self.project.active_component;
        self.project.active_component = if active > ci { active - 1 } else { active.min(self.project.components.len() - 1) };
        self.selected_piece = None;
        self.selected_edge = None;
        self.walk_frame = None;
        self.recompute();
    }

    /// Makes a garment the one edits, parameters, fabrics and the timeline apply to.
    pub fn set_active_component(&mut self, component_id: &str) {
        let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) else {
            return;
        };
        if ci != self.project.active_component {
            self.project.active_component = ci;
            if self.selected_piece.and_then(|p| self.component_of_piece(p)) != Some(ci) {
                self.selected_piece = None;
                self.selected_edge = None;
            }
            self.recompute();
        }
    }

    pub fn rename_component(&mut self, component_id: &str, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) {
            self.push_undo();
            self.project.components[ci].name = name.to_string();
        }
    }

    /// Garment layer in the 3D fit (0 = next to the body). Re-dresses the 3D fit when open.
    pub fn set_component_layer(&mut self, component_id: &str, layer: u8) {
        if let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) {
            if self.project.components[ci].layer != layer {
                self.push_undo();
                self.project.components[ci].layer = layer;
                self.recompute();
            }
        }
    }

    /// Moves a garment one place outward (worn over the next garment) or inward, swapping
    /// layers with its neighbour; layers are then renumbered 0, 1, 2... in wearing order.
    pub fn move_component_layer(&mut self, component_id: &str, outward: bool) {
        let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) else {
            return;
        };
        // Wearing order: by layer, then creation order.
        let mut order: Vec<usize> = (0..self.project.components.len()).collect();
        order.sort_by_key(|&i| (self.project.components[i].layer, i));
        let pos = order.iter().position(|&i| i == ci).unwrap_or(0);
        let target = if outward { pos + 1 } else { pos.wrapping_sub(1) };
        if target >= order.len() {
            return;
        }
        self.push_undo();
        order.swap(pos, target);
        for (rank, &i) in order.iter().enumerate() {
            self.project.components[i].layer = rank.min(255) as u8;
        }
        self.recompute();
    }

    /// Shows or hides a garment everywhere (canvas, exports, 3D fit).
    pub fn set_component_visible(&mut self, component_id: &str, visible: bool) {
        if let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) {
            if self.project.components[ci].visible != visible {
                self.project.components[ci].visible = visible;
                self.recompute();
            }
        }
    }

    /// Shows or hides a garment in the 3D fit only. A hidden garment is still worn, so hiding
    /// an outer layer reveals what is under it without re-draping anything.
    pub fn set_component_visible_3d(&mut self, component_id: &str, visible: bool) {
        if let Some(ci) = Uuid::parse_str(component_id).ok().and_then(|id| self.project.component_index(id)) {
            self.project.components[ci].visible_3d = visible;
        }
    }

    pub fn serialize_project_json(&self) -> String {
        serde_json::to_string_pretty(&self.project).unwrap_or_else(|_| "{}".into())
    }

    pub fn load_project_json(&mut self, json: &str) -> Result<(), JsValue> {
        let parsed = PatternProject::from_json(json)
            .map_err(|e| JsValue::from_str(&format!("Invalid .modelist file: {e}")))?;
        self.container_extras.clear();
        self.install_project(parsed);
        Ok(())
    }

    /// Writes the project as a `.modelist` container (ZIP of JSON parts, see
    /// `modelist_core::container`). `modified` is the save time (ISO 8601) and `session_json`
    /// the UI state to restore; both may be empty, like `thumbnail_png`. Parts of the opened
    /// file this build does not know are written back unchanged.
    pub fn save_project_container(
        &self,
        modified: &str,
        session_json: &str,
        thumbnail_png: &[u8],
    ) -> Result<Vec<u8>, JsValue> {
        write_container(
            &self.project,
            &WriteOptions {
                writer_version: env!("CARGO_PKG_VERSION"),
                modified: (!modified.is_empty()).then_some(modified),
                session_json: (!session_json.is_empty()).then_some(session_json),
                thumbnail_png: (!thumbnail_png.is_empty()).then_some(thumbnail_png),
                extra_parts: &self.container_extras,
            },
        )
        .map_err(|e| JsValue::from_str(&e))
    }

    /// Opens a project file: a `.modelist` container, or plain JSON (`.modelist.json`, any
    /// format version). Returns the saved UI state (`session.json`), or an empty string.
    pub fn load_project_file(&mut self, bytes: &[u8]) -> Result<String, JsValue> {
        if is_container(bytes) {
            let opened = read_container(bytes)
                .map_err(|e| JsValue::from_str(&format!("Invalid .modelist file: {e}")))?;
            self.container_extras = opened.extra_parts;
            self.install_project(opened.project);
            Ok(opened.session_json.unwrap_or_default())
        } else {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| JsValue::from_str("Invalid .modelist file: neither a container nor JSON text"))?;
            self.load_project_json(text)?;
            Ok(String::new())
        }
    }
}

impl ModelistSession {
    /// First sequence number whose component id and ID namespace are free for a template.
    fn free_component_seq(&self, tmpl: &str) -> usize {
        (1..)
            .find(|n| {
                let ns = format!("component:{tmpl}:{n}");
                let id = det_id(&format!("component:{tmpl}:{n}"));
                let ns_id = det_id(&ns);
                self.project.components.iter().all(|c| c.id != id && c.id != ns_id && c.id_ns != ns)
            })
            .unwrap_or(1)
    }

    /// Adds a garment to the right of the others on the canvas and one layer above them in the
    /// 3D fit, makes it active and selects its first piece. Returns its id.
    fn place_component(&mut self, mut comp: modelist_core::model::GarmentComponent) -> String {
        let max_x = self
            .evaluated
            .iter()
            .flat_map(|e| e.cut_polyline.iter().chain(&e.seam_polyline).map(|p| p.x))
            .fold(f64::NEG_INFINITY, f64::max);
        let (pieces, _, _, _) = modelist_core::blocks::build_component_pieces(&comp, &self.project.measurements);
        let min_x = pieces
            .iter()
            .flat_map(|p| p.seam_loop.edges.iter().map(|e| e.start.x))
            .fold(f64::INFINITY, f64::min);
        if max_x.is_finite() && min_x.is_finite() {
            modelist_core::library::shift_component(&mut comp, max_x + 250.0 - min_x, 0.0);
        }
        comp.layer = self.project.components.iter().map(|c| c.layer).max().map(|l| l.saturating_add(1)).unwrap_or(0);
        comp.visible = true;
        let id = comp.id.to_string();
        self.project.components.push(comp);
        self.project.active_component = self.project.components.len() - 1;
        self.walk_frame = None;
        self.selected_edge = None;
        self.recompute();
        self.selected_piece = self
            .evaluated
            .iter()
            .zip(&self.piece_component)
            .find(|(_, &c)| c == self.project.active_component)
            .map(|(e, _)| e.piece.id);
        self.rebuild_render_buffers();
        id
    }

    fn install_project(&mut self, mut parsed: PatternProject) {
        parsed.normalize_fabrics();
        parsed.sync_fabric_variables();
        self.push_undo();
        self.project = parsed;
        self.walk_frame = None;
        self.selected_edge = None;
        self.recompute();
        self.selected_piece = self.evaluated.first().map(|e| e.piece.id);
        self.rebuild_render_buffers();
    }
}

#[wasm_bindgen]
impl ModelistSession {

    pub fn set_measurement(&mut self, key: &str, val: f64) {
        if !self.project.measurements.set_by_key(key, val) {
            return;
        }
        self.recompute();
    }

    pub fn set_ease(&mut self, key: &str, val: f64) {
        if !self.project.comp_mut().ease.set_by_key(key, val) {
            return;
        }
        self.recompute();
    }

    pub fn set_fabric_preset(&mut self, preset_id: &str) {
        self.push_undo();
        self.project.comp_mut().fabric = FabricSpec::from_preset(preset_id);
        self.after_fabric_change();
    }

    pub fn set_fabric_color(&mut self, color_hex: &str) {
        if color_hex.starts_with('#') && color_hex.len() == 7 {
            self.project.comp_mut().fabric.color_hex = color_hex.to_string();
            self.after_fabric_change();
        }
    }

    /// Sets one physical property of the shell fabric (key from `fabric_property_specs_json`).
    /// Values are clamped to the property's valid range. Returns false for unknown keys.
    pub fn set_fabric_property(&mut self, key: &str, value: f64) -> bool {
        if !self.project.comp_mut().fabric.set_property(key, value) {
            return false;
        }
        self.after_fabric_change();
        true
    }

    /// Surface finish for the 3D render: plain, twill, satin, jersey, rib, sheer, terry.
    pub fn set_fabric_finish(&mut self, finish: &str) {
        let parsed: Result<FabricFinish, _> = serde_json::from_str(&format!("\"{finish}\""));
        if let Ok(f) = parsed {
            self.project.comp_mut().fabric.finish = f;
            self.after_fabric_change();
        }
    }

    /// Restores the shell fabric's physical properties from its preset (keeps the colour).
    pub fn reset_fabric_to_preset(&mut self) {
        if let Some(mut p) = FabricSpec::preset(&self.project.comp_mut().fabric.id) {
            self.push_undo();
            p.color_hex = self.project.comp_mut().fabric.color_hex.clone();
            self.project.comp_mut().fabric = p;
            self.after_fabric_change();
        }
    }

    /// Assigns a preset to a piece role (e.g. "rib"); an empty preset id means "same as shell".
    pub fn set_role_fabric_preset(&mut self, role: &str, preset_id: &str) {
        self.push_undo();
        let fabric = if preset_id.is_empty() {
            None
        } else {
            Some(FabricSpec::from_preset(preset_id))
        };
        self.project.set_role_fabric(role, fabric);
        self.after_fabric_change();
    }

    /// Sets one physical property of a role's fabric (the role must have its own fabric).
    pub fn set_role_fabric_property(&mut self, role: &str, key: &str, value: f64) -> bool {
        let key_role = fabric_role_key(role);
        let Some(rf) = self.project.comp_mut().role_fabrics.iter_mut().find(|r| r.role == key_role) else {
            return false;
        };
        if !rf.fabric.set_property(key, value) {
            return false;
        }
        self.after_fabric_change();
        true
    }

    pub fn set_role_fabric_color(&mut self, role: &str, color_hex: &str) {
        let key_role = fabric_role_key(role);
        if let Some(rf) = self.project.comp_mut().role_fabrics.iter_mut().find(|r| r.role == key_role) {
            if color_hex.starts_with('#') && color_hex.len() == 7 {
                rf.fabric.color_hex = color_hex.to_string();
                self.after_fabric_change();
            }
        }
    }

    pub fn move_piece(&mut self, piece_id: &str, total_dx: f64, total_dy: f64, commit: bool) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        if self.drag_base.is_none() {
            self.drag_base = Some(self.project.clone());
            self.drag_op_id = Some(self.next_op_id("move_piece"));
        }
        let op_id = self.drag_op_id.unwrap();
        let op = ParametricOp::MovePiece {
            id: op_id,
            piece_id: pid,
            dx: total_dx,
            dy: total_dy,
        };
        self.upsert_op(op);
        if commit {
            if let Some(base) = self.drag_base.take() {
                if total_dx.hypot(total_dy) > 1e-3 {
                    self.undo_stack.push(base);
                    self.redo_stack.clear();
                }
            }
            self.drag_op_id = None;
        }
        self.recompute();
    }

    pub fn convert_edge_curve(&mut self, piece_id: &str, edge_idx: usize, to_cubic: bool) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("convert_curve");
        self.insert_op(ParametricOp::ConvertEdgeCurve {
            id,
            piece_id: pid,
            edge_idx,
            to_cubic,
        });
        self.recompute();
    }

    pub fn split_edge_point(&mut self, piece_id: &str, edge_idx: usize, param_t: f64) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("split_edge_pt");
        self.insert_op(ParametricOp::SplitEdgePoint {
            id,
            piece_id: pid,
            edge_idx,
            param_t,
        });
        self.recompute();
    }

    pub fn set_edge_length(&mut self, piece_id: &str, edge_idx: usize, target_len_mm: f64) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("set_edge_len");
        self.insert_op(ParametricOp::SetEdgeLength {
            id,
            piece_id: pid,
            edge_idx,
            target_len_mm,
        });
        self.recompute();
    }

    pub fn mirror_piece(&mut self, piece_id: &str) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("mirror_piece");
        self.insert_op(ParametricOp::MirrorPiece { id, piece_id: pid });
        self.recompute();
    }

    pub fn duplicate_piece(&mut self, piece_id: &str) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        let Some(orig) = self
            .evaluated
            .iter()
            .find(|e| e.piece.id == pid)
            .map(|e| e.piece.clone())
        else {
            return;
        };
        self.push_undo();
        let mut copy = orig;
        let new_id = self.next_op_id("dup_piece");
        copy.id = new_id;
        copy.name = format!("{} (Copy)", copy.name);
        copy.code = format!("{}-C", copy.code);
        for (ei, e) in copy.seam_loop.edges.iter_mut().enumerate() {
            e.id = det_id(&format!("{new_id}:edge:{ei}"));
            e.start.x += 65.0;
            e.start.y -= 35.0;
            e.segment
                .transform(|pt| modelist_core::model::Point2::new(pt.x + 65.0, pt.y - 35.0));
        }
        copy.grainline.start.x += 65.0;
        copy.grainline.start.y -= 35.0;
        copy.grainline.end.x += 65.0;
        copy.grainline.end.y -= 35.0;
        for d in &mut copy.darts {
            d.apex.x += 65.0;
            d.apex.y -= 35.0;
            d.edge_id = copy.seam_loop.edges.get(d.edge_idx).map(|e| e.id);
        }
        for dh in &mut copy.drill_holes {
            dh.pos.x += 65.0;
            dh.pos.y -= 35.0;
        }
        for n in &mut copy.notches {
            n.edge_id = copy.seam_loop.edges.get(n.edge_idx).map(|e| e.id);
        }
        self.project.comp_mut().custom_pieces.push(copy);
        self.selected_piece = Some(new_id);
        self.selected_edge = None;
        self.recompute();
    }

    pub fn add_rect_piece(&mut self, name: &str, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.push_undo();
        let count = self.evaluated.len() + 1;
        let clean_name = if name.trim().is_empty() {
            format!("Drafted Panel {count}")
        } else {
            name.trim().to_string()
        };
        let code = format!("DP-{count:02}");
        let piece = make_rect_pattern_piece(&clean_name, &code, x0, y0, x1, y1);
        let pid = piece.id;
        self.project.comp_mut().custom_pieces.push(piece);
        self.selected_piece = Some(pid);
        self.selected_edge = None;
        self.recompute();
    }

    pub fn set_show_mirror_unfold(&mut self, show: bool) {
        self.show_mirror_unfold = show;
        self.rebuild_render_buffers();
    }

    pub fn set_variable(&mut self, name: &str, expr: &str, description: &str) {
        let clean = name.trim().to_string();
        // fabric_* variables mirror the Fabric tab and are read-only in formulas.
        if clean.is_empty() || is_fabric_variable(&clean) {
            return;
        }
        self.push_undo();
        if let Some(existing) = self.project.comp_mut().variables.iter_mut().find(|v| v.name == clean) {
            existing.expr = expr.to_string();
            if !description.is_empty() {
                existing.description = description.to_string();
            }
        } else {
            self.project.comp_mut().variables.push(ParametricVar {
                name: clean,
                expr: expr.to_string(),
                description: description.to_string(),
            });
        }
        self.recompute();
    }

    pub fn delete_variable(&mut self, name: &str) {
        if is_fabric_variable(name) {
            return;
        }
        self.push_undo();
        self.project.comp_mut().variables.retain(|v| v.name != name);
        self.recompute();
    }

    pub fn set_grade_mode(&mut self, mode: &str) {
        self.push_undo();
        self.project.grade_mode = match mode {
            "rule_based_rtw" => GradeMode::RuleBasedRtw,
            "hybrid_differential" => GradeMode::HybridDifferential,
            _ => GradeMode::ParametricMtm,
        };
        self.recompute();
    }

    pub fn set_stack_anchor(&mut self, anchor: &str) {
        self.project.stack_anchor = match anchor {
            "bust_or_waist_center" => StackAnchor::BustOrWaistCenter,
            "top_left_anchor" => StackAnchor::TopLeftAnchor,
            _ => StackAnchor::GrainlineMidpoint,
        };
        self.recompute();
    }

    pub fn set_active_size(&mut self, size: &str) {
        if !self.project.size_table.sizes.iter().any(|s| s == size) {
            return;
        }
        self.push_undo();
        self.project.measurements = BodyMeasurements::from_size_preset(size);
        self.project.size_table.active_size = size.to_string();
        if !self
            .project
            .size_table
            .visible_sizes
            .iter()
            .any(|s| s == size)
        {
            self.project.size_table.visible_sizes.push(size.to_string());
        }
        self.recompute();
    }

    pub fn set_visible_sizes(&mut self, sizes_json: &str) {
        if let Ok(mut list) = serde_json::from_str::<Vec<String>>(sizes_json) {
            if !list.iter().any(|s| s == &self.project.size_table.active_size) {
                list.push(self.project.size_table.active_size.clone());
            }
            self.project.size_table.visible_sizes = list;
            self.recompute();
        }
    }

    pub fn set_selection(&mut self, piece_id: Option<String>, edge_idx: Option<usize>) {
        self.selected_piece = piece_id.and_then(|s| Uuid::parse_str(&s).ok());
        if let Some(ci) = self.selected_piece.and_then(|pid| self.component_of_piece(pid)) {
            self.project.active_component = ci;
        }
        self.selected_edge = match (self.selected_piece, edge_idx) {
            (Some(pid), Some(ei)) => Some((pid, ei)),
            _ => None,
        };
        if self.selected_piece.is_some() {
            self.selected_binding = None;
        }
        self.rebuild_render_buffers();
    }

    pub fn set_selected_binding(&mut self, binding_id: Option<String>) {
        self.selected_binding = binding_id.and_then(|s| Uuid::parse_str(&s).ok());
        self.rebuild_render_buffers();
    }

    pub fn set_seam_draft_preview(&mut self, edges_a_json: &str, edges_b_json: &str) {
        self.draft_seam_a = serde_json::from_str::<Vec<EdgeRef>>(edges_a_json).unwrap_or_default();
        self.draft_seam_b = serde_json::from_str::<Vec<EdgeRef>>(edges_b_json).unwrap_or_default();
        self.rebuild_render_buffers();
    }

    /// Creates or replaces a full multi-edge SeamBinding (used by the Fusion 360 Seam Command Dialog).
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_seam_binding(
        &mut self,
        binding_id: Option<String>,
        label: &str,
        edges_a_json: &str,
        edges_b_json: &str,
        driver: &str,
        ease_json: &str,
        seam_type: &str,
        auto_partner_notches: bool,
        sync_recommended_sa: bool,
    ) -> Result<String, JsValue> {
        let edges_a: Vec<EdgeRef> = serde_json::from_str(edges_a_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid Interface A edges: {e}")))?;
        let edges_b: Vec<EdgeRef> = serde_json::from_str(edges_b_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid Interface B edges: {e}")))?;
        if edges_a.is_empty() || edges_b.is_empty() {
            return Err(JsValue::from_str(
                "Both Interface A and Interface B require at least one edge",
            ));
        }

        let drv = match driver {
            "driver_b" => SeamDriver::DriverB,
            "mutual" => SeamDriver::Mutual,
            "monitor_only" => SeamDriver::MonitorOnly,
            _ => SeamDriver::DriverA,
        };
        let ease: EaseSpec = serde_json::from_str(ease_json).unwrap_or_default();
        let st = match seam_type {
            "overlock" => SeamType::Overlock,
            "french" => SeamType::French,
            "flat_felled" => SeamType::FlatFelled,
            "gathered" => SeamType::Gathered,
            _ => SeamType::Plain,
        };

        self.push_undo();
        let bid = binding_id
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(|| self.next_op_id("seam_binding"));

        let clean_label = if label.trim().is_empty() {
            format!("Seam ({}e <-> {}e)", edges_a.len(), edges_b.len())
        } else {
            label.trim().to_string()
        };

        let expected = ease.expected_ease_mm(200.0);
        let (ease_min, ease_max) = match ease {
            EaseSpec::Exact => (-2.5, 2.5),
            EaseSpec::EaseMm { mm } => (mm - 6.0, mm + 12.0),
            EaseSpec::EasePct { .. } | EaseSpec::Gather { .. } | EaseSpec::Pleats { .. } => {
                ((expected - 15.0).min(-2.0), (expected + 40.0).max(25.0))
            }
        };

        if sync_recommended_sa {
            let sa_w = st.recommended_sa_mm();
            for er in edges_a.iter().chain(edges_b.iter()) {
                if let Some(ep) = self.evaluated.iter().find(|e| e.piece.id == er.piece_id) {
                    if let Some(eidx) = ep.piece.seam_loop.edge_index_by_id(er.edge_id) {
                        let oid = self.next_op_id("seam_sa");
                        self.insert_op(ParametricOp::SetSeamAllowance {
                            id: oid,
                            piece_id: er.piece_id,
                            edge_idx: eidx,
                            width_mm: sa_w,
                            corner_rule: CornerRule::Miter { limit: 2.5 },
                        });
                    }
                }
            }
        }

        let binding = SeamBinding {
            id: bid,
            label: clean_label,
            interface_a: SeamInterface {
                id: det_id(&format!("{bid}:ia")),
                name: "Interface A".into(),
                edges: edges_a,
            },
            interface_b: SeamInterface {
                id: det_id(&format!("{bid}:ib")),
                name: "Interface B".into(),
                edges: edges_b,
            },
            ease,
            driver: drv,
            seam_type: st,
            ease_min_mm: ease_min,
            ease_max_mm: ease_max,
            auto_partner_notches,
        };
        let op_id = self.next_op_id("replace_binding");
        self.insert_op(ParametricOp::ReplaceSeamBinding {
            id: op_id,
            binding,
        });
        self.selected_binding = Some(bid);
        self.draft_seam_a.clear();
        self.draft_seam_b.clear();
        self.recompute();
        Ok(bid.to_string())
    }

    pub fn delete_seam_binding(&mut self, binding_id: &str) {
        let Ok(bid) = Uuid::parse_str(binding_id) else {
            return;
        };
        self.push_undo();
        let op_id = self.next_op_id("delete_binding");
        self.insert_op(ParametricOp::DeleteSeamBinding {
            id: op_id,
            binding_id: bid,
        });
        if self.selected_binding == Some(bid) {
            self.selected_binding = None;
        }
        self.recompute();
    }

    pub fn toggle_piece_visibility(&mut self, piece_id: &str) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        if let Some(ep) = self.evaluated.iter_mut().find(|e| e.piece.id == pid) {
            ep.piece.visible = !ep.piece.visible;
        }
        if let Some(cp) = self
            .project
            .comp_mut()
            .custom_pieces
            .iter_mut()
            .find(|p| p.id == pid)
        {
            cp.visible = !cp.visible;
        }
        self.rebuild_render_buffers();
    }

    pub fn move_vertex(
        &mut self,
        piece_id: &str,
        edge_idx: usize,
        total_dx: f64,
        total_dy: f64,
        commit: bool,
    ) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        if self.drag_base.is_none() {
            self.drag_base = Some(self.project.clone());
            self.drag_op_id = Some(self.next_op_id("move_vertex"));
        }
        let op_id = self.drag_op_id.unwrap();
        let op = ParametricOp::MoveVertex {
            id: op_id,
            piece_id: pid,
            edge_idx,
            dx: total_dx,
            dy: total_dy,
        };
        self.upsert_op(op);
        if commit {
            if let Some(base) = self.drag_base.take() {
                if total_dx.hypot(total_dy) > 1e-3 {
                    self.undo_stack.push(base);
                    self.redo_stack.clear();
                }
            }
            self.drag_op_id = None;
        }
        self.recompute();
    }

    pub fn move_handle(
        &mut self,
        piece_id: &str,
        edge_idx: usize,
        which: u8,
        total_dx: f64,
        total_dy: f64,
        lock_g1: bool,
        commit: bool,
    ) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        if self.drag_base.is_none() {
            self.drag_base = Some(self.project.clone());
            self.drag_op_id = Some(self.next_op_id("move_handle"));
        }
        let op_id = self.drag_op_id.unwrap();
        let op = ParametricOp::MoveHandle {
            id: op_id,
            piece_id: pid,
            edge_idx,
            which,
            dx: total_dx,
            dy: total_dy,
            lock_g1,
        };
        self.upsert_op(op);
        if commit {
            if let Some(base) = self.drag_base.take() {
                if total_dx.hypot(total_dy) > 1e-3 {
                    self.undo_stack.push(base);
                    self.redo_stack.clear();
                }
            }
            self.drag_op_id = None;
        }
        self.recompute();
    }

    pub fn set_edge_sa(
        &mut self,
        piece_id: &str,
        edge_idx: usize,
        width_mm: f64,
        corner_rule_json: &str,
    ) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        let rule: CornerRule = serde_json::from_str(corner_rule_json).unwrap_or_default();
        self.push_undo();
        let id = self.next_op_id("set_sa");
        self.insert_op(ParametricOp::SetSeamAllowance {
            id,
            piece_id: pid,
            edge_idx,
            width_mm,
            corner_rule: rule,
        });
        self.recompute();
    }

    pub fn add_notch(
        &mut self,
        piece_id: &str,
        edge_idx: usize,
        param_t: f64,
        style: &str,
        count: u8,
        depth_mm: f64,
    ) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        let notch_style = match style {
            "v" => NotchStyle::V,
            "u" => NotchStyle::U,
            "t" => NotchStyle::T,
            "castle" => NotchStyle::Castle,
            _ => NotchStyle::Slit,
        };
        self.push_undo();
        let id = self.next_op_id("add_notch");
        self.insert_op(ParametricOp::AddNotch {
            id,
            piece_id: pid,
            edge_idx,
            param_t,
            style: notch_style,
            count,
            depth_mm,
        });
        self.recompute();
    }

    pub fn add_drill_hole(&mut self, piece_id: &str, x: f64, y: f64, label: &str) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("drill_hole");
        self.insert_op(ParametricOp::AddDrillHole {
            id,
            piece_id: pid,
            x,
            y,
            label: if label.is_empty() { "Drill" } else { label }.into(),
        });
        self.recompute();
    }

    pub fn set_grain_rotation(&mut self, piece_id: &str, mode: &str) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        let rot = match mode {
            "one_way" => GrainRotationMode::OneWay,
            "bias" => GrainRotationMode::Bias,
            "free" => GrainRotationMode::Free,
            _ => GrainRotationMode::TwoWay,
        };
        self.push_undo();
        let id = self.next_op_id("grain_rot");
        self.insert_op(ParametricOp::SetGrainRotation {
            id,
            piece_id: pid,
            mode: rot,
        });
        self.recompute();
    }

    pub fn true_corner(&mut self, piece_id: &str, edge_idx: usize, square_90: bool) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("true_corner");
        self.insert_op(ParametricOp::TrueCornerG1 {
            id,
            piece_id: pid,
            edge_idx,
            square_90,
        });
        self.recompute();
    }

    pub fn split_style_line(
        &mut self,
        piece_id: &str,
        edge_a_idx: usize,
        t_a: f64,
        edge_b_idx: usize,
        t_b: f64,
        label_a: &str,
        label_b: &str,
    ) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("split_style");
        self.insert_op(ParametricOp::SplitStyleLine {
            id,
            piece_id: pid,
            edge_a_idx,
            t_a,
            edge_b_idx,
            t_b,
            label_a: label_a.into(),
            label_b: label_b.into(),
        });
        self.recompute();
    }

    pub fn update_seam_binding(
        &mut self,
        binding_id: &str,
        driver: &str,
        ease_json: &str,
        seam_type: &str,
        auto_partner_notches: bool,
    ) {
        let Ok(bid) = Uuid::parse_str(binding_id) else {
            return;
        };
        let drv = match driver {
            "driver_b" => SeamDriver::DriverB,
            "mutual" => SeamDriver::Mutual,
            "monitor_only" => SeamDriver::MonitorOnly,
            _ => SeamDriver::DriverA,
        };
        let ease: EaseSpec = serde_json::from_str(ease_json).unwrap_or_default();
        let st = match seam_type {
            "overlock" => SeamType::Overlock,
            "french" => SeamType::French,
            "flat_felled" => SeamType::FlatFelled,
            "gathered" => SeamType::Gathered,
            _ => SeamType::Plain,
        };
        self.push_undo();
        let id = self.next_op_id("update_binding");
        self.insert_op(ParametricOp::UpdateSeamBinding {
            id,
            binding_id: bid,
            driver: drv,
            ease,
            seam_type: st,
            auto_partner_notches,
        });
        self.recompute();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_seam_binding(
        &mut self,
        label: &str,
        piece_a_id: &str,
        edge_a_id: &str,
        piece_b_id: &str,
        edge_b_id: &str,
        driver: &str,
        ease_json: &str,
    ) {
        let (Ok(pa), Ok(ea), Ok(pb), Ok(eb)) = (
            Uuid::parse_str(piece_a_id),
            Uuid::parse_str(edge_a_id),
            Uuid::parse_str(piece_b_id),
            Uuid::parse_str(edge_b_id),
        ) else {
            return;
        };
        let drv = match driver {
            "driver_b" => SeamDriver::DriverB,
            "mutual" => SeamDriver::Mutual,
            "monitor_only" => SeamDriver::MonitorOnly,
            _ => SeamDriver::DriverA,
        };
        let ease: EaseSpec = serde_json::from_str(ease_json).unwrap_or_default();
        self.push_undo();
        let bid = self.next_op_id("custom_binding");
        let binding = SeamBinding {
            id: bid,
            label: if label.trim().is_empty() {
                "Custom Seam Binding".into()
            } else {
                label.trim().into()
            },
            interface_a: SeamInterface {
                id: det_id(&format!("{bid}:ia")),
                name: "Driver Interface".into(),
                edges: vec![EdgeRef {
                    piece_id: pa,
                    edge_id: ea,
                    dir: 1,
                }],
            },
            interface_b: SeamInterface {
                id: det_id(&format!("{bid}:ib")),
                name: "Follower Interface".into(),
                edges: vec![EdgeRef {
                    piece_id: pb,
                    edge_id: eb,
                    dir: 1,
                }],
            },
            ease,
            driver: drv,
            seam_type: SeamType::Plain,
            ease_min_mm: -3.0,
            ease_max_mm: 25.0,
            auto_partner_notches: true,
        };
        let op_id = self.next_op_id("add_binding");
        self.insert_op(ParametricOp::AddSeamBinding {
            id: op_id,
            binding,
        });
        self.recompute();
    }

    pub fn auto_detect_seams(&mut self) -> usize {
        let raw_pieces: Vec<_> = self.evaluated.iter().map(|e| e.piece.clone()).collect();
        let found = auto_detect_seam_bindings(&raw_pieces, &self.stitching.bindings);
        let count = found.len();
        if count > 0 {
            self.push_undo();
            for b in found {
                let oid = self.next_op_id("auto_seam_op");
                self.insert_op(ParametricOp::AddSeamBinding {
                    id: oid,
                    binding: b,
                });
            }
            self.recompute();
        }
        count
    }

    pub fn pivot_dart(
        &mut self,
        piece_id: &str,
        dart_id: &str,
        target_edge_idx: usize,
        target_t: f64,
        angle_deg: f64,
    ) {
        let (Ok(pid), Ok(did)) = (Uuid::parse_str(piece_id), Uuid::parse_str(dart_id)) else {
            return;
        };
        self.push_undo();
        let id = self.next_op_id("pivot_dart");
        self.insert_op(ParametricOp::PivotDart {
            id,
            piece_id: pid,
            dart_id: did,
            target_edge_idx,
            target_t,
            angle_deg,
        });
        self.recompute();
    }

    pub fn slash_and_spread(
        &mut self,
        piece_id: &str,
        start_edge_idx: usize,
        end_edge_idx: usize,
        slash_count: u8,
        spread_mm: f64,
        hinge_mode: &str,
    ) {
        let Ok(pid) = Uuid::parse_str(piece_id) else {
            return;
        };
        let h_mode = if hinge_mode == "parallel_shift" {
            HingeMode::ParallelShift
        } else {
            HingeMode::EdgeHinge
        };
        self.push_undo();
        let id = self.next_op_id("slash_spread");
        self.insert_op(ParametricOp::SlashSpread {
            id,
            piece_id: pid,
            start_edge_idx,
            end_edge_idx,
            slash_count,
            spread_mm,
            hinge_mode: h_mode,
        });
        self.recompute();
    }

    pub fn remove_op(&mut self, op_id: &str) {
        let Ok(oid) = Uuid::parse_str(op_id) else {
            return;
        };
        let Some(pos) = self.project.comp_mut().ops.iter().position(|o| o.id() == oid) else {
            return;
        };
        self.push_undo();
        self.project.comp_mut().ops.remove(pos);
        self.project.comp_mut().suppressed_ops.retain(|id| *id != oid);
        if let Some(rb) = self.project.comp_mut().rollback_index {
            if pos < rb {
                self.project.comp_mut().rollback_index = Some(rb.saturating_sub(1));
            } else if rb > self.project.comp_mut().ops.len() {
                self.project.comp_mut().rollback_index = None;
            }
        }
        self.recompute();
    }

    pub fn set_rollback_index(&mut self, index: i32) {
        if index < 0 || (index as usize) >= self.project.comp_mut().ops.len() {
            self.project.comp_mut().rollback_index = None;
        } else {
            self.project.comp_mut().rollback_index = Some(index as usize);
        }
        self.recompute();
    }

    pub fn toggle_suppress_op(&mut self, op_id: &str) {
        let Ok(oid) = Uuid::parse_str(op_id) else {
            return;
        };
        self.push_undo();
        if let Some(pos) = self.project.comp_mut().suppressed_ops.iter().position(|id| *id == oid) {
            self.project.comp_mut().suppressed_ops.remove(pos);
        } else {
            self.project.comp_mut().suppressed_ops.push(oid);
        }
        self.recompute();
    }

    pub fn reorder_op(&mut self, from_idx: usize, to_idx: usize) -> bool {
        let n = self.project.comp_mut().ops.len();
        if from_idx >= n || to_idx >= n || from_idx == to_idx {
            return false;
        }
        let (min_idx, max_idx) = compute_reorder_range(&self.project.comp_mut().ops, from_idx);
        if to_idx < min_idx || to_idx > max_idx {
            return false;
        }
        self.push_undo();
        let op = self.project.comp_mut().ops.remove(from_idx);
        self.project.comp_mut().ops.insert(to_idx, op);
        if let Some(rb) = self.project.comp_mut().rollback_index {
            if from_idx < rb && to_idx >= rb {
                self.project.comp_mut().rollback_index = Some(rb.saturating_sub(1));
            } else if from_idx >= rb && to_idx < rb {
                self.project.comp_mut().rollback_index = Some(rb + 1);
            }
        }
        self.recompute();
        true
    }

    pub fn update_op_json(&mut self, op_id: &str, patch_json: &str) -> Result<(), JsValue> {
        let oid = Uuid::parse_str(op_id).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let patch: serde_json::Value =
            serde_json::from_str(patch_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let Some(pos) = self.project.comp_mut().ops.iter().position(|o| o.id() == oid) else {
            return Err(JsValue::from_str("Operation not found in timeline"));
        };
        self.push_undo();
        if let Err(err) = patch_op_from_json(&mut self.project.comp_mut().ops[pos], &patch) {
            self.undo_stack.pop();
            return Err(JsValue::from_str(&err));
        }
        self.recompute();
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.project.clone());
            self.project = prev;
            self.recompute();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.project.clone());
            self.project = next;
            self.recompute();
            true
        } else {
            false
        }
    }

    pub fn query_snap(&self, wx: f64, wy: f64, world_tol_mm: f64) -> String {
        match self.snap_index.query_snap(wx, wy, world_tol_mm) {
            Some(hit) => serde_json::to_string(&hit).unwrap_or_else(|_| "null".into()),
            None => "null".into(),
        }
    }

    pub fn hit_test(&self, wx: f64, wy: f64, world_tol_mm: f64) -> String {
        let hit = hit_test_pieces(&self.evaluated, self.selected_piece, wx, wy, world_tol_mm);
        serde_json::to_string(&hit).unwrap_or_else(|_| "{\"kind\":\"none\"}".into())
    }

    pub fn compute_seam_walk(&mut self, link_id: &str, progress_u: f64) -> String {
        let Ok(lid) = Uuid::parse_str(link_id) else {
            return "null".into();
        };
        let Some(binding) = self.stitching.bindings.iter().find(|b| b.id == lid) else {
            return "null".into();
        };
        let raw_pieces: Vec<_> = self.evaluated.iter().map(|e| e.piece.clone()).collect();
        let Some((frame, ghost_pid)) = compute_binding_walk_frame(binding, &raw_pieces, progress_u)
        else {
            return "null".into();
        };
        let json = serde_json::to_string(&frame).unwrap_or_else(|_| "null".into());
        self.walk_frame = Some((frame, ghost_pid));
        self.rebuild_render_buffers();
        json
    }

    pub fn clear_seam_walk(&mut self) {
        self.walk_frame = None;
        self.rebuild_render_buffers();
    }

    pub fn get_stroke_coords(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.render_buffers.stroke_coords.as_slice())
    }

    pub fn get_stroke_meta(&self) -> js_sys::Uint32Array {
        js_sys::Uint32Array::from(self.render_buffers.stroke_meta.as_slice())
    }

    pub fn get_handle_points(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.render_buffers.handle_points.as_slice())
    }

    // --- Strictly Decoupled On-Demand 3D Virtual Fit API ---
    pub fn init_sim3d(&mut self, pose: &str, body_form: &str) -> String {
        let avatar_pose = match pose {
            "arms_raised" => AvatarPose::ArmsRaised,
            "t_pose" => AvatarPose::TPose,
            _ => AvatarPose::APose,
        };
        let avatar_form = match body_form {
            "male" => AvatarBodyForm::Male,
            "neutral" => AvatarBodyForm::Neutral,
            _ => AvatarBodyForm::Female,
        };
        let sim = self.build_sim(avatar_pose, avatar_form, None);
        let stats_json = serde_json::to_string(&sim.stats()).unwrap_or_else(|_| "{}".into());
        self.sim3d = Some(sim);
        stats_json
    }

    pub fn step_sim3d(&mut self, steps: u32) -> String {
        let Some(ref mut sim) = self.sim3d else {
            return "null".into();
        };
        sim.step(steps.clamp(1, 16));
        serde_json::to_string(&sim.stats()).unwrap_or_else(|_| "null".into())
    }

    pub fn reset_sim3d(&mut self, pose: &str, body_form: &str) -> String {
        self.init_sim3d(pose, body_form)
    }

    pub fn dispose_sim3d(&mut self) {
        self.sim3d = None;
    }

    pub fn get_sim3d_cloth_positions(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.flat_positions.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_cloth_normals(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.flat_normals.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_cloth_strain(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.flat_strain_pct.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_cloth_tension(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.flat_tension_kpa.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_cloth_clearance(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.flat_clearance_mm.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    /// Per-vertex cloth colour (rgb 0..1) from each piece role's fabric.
    pub fn get_sim3d_cloth_colors(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.cloth_colors().as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    /// Drape-quality metrics (poke-through, creases, self-intersections) as JSON. A few ms:
    /// poll it occasionally, not every frame.
    pub fn get_sim3d_quality_json(&self) -> String {
        match &self.sim3d {
            Some(s) => serde_json::to_string(&s.drape_quality()).unwrap_or_else(|_| "null".into()),
            None => "null".into(),
        }
    }

    pub fn get_sim3d_cloth_indices(&self) -> js_sys::Uint32Array {
        match &self.sim3d {
            Some(s) => {
                // Garments hidden in 3D are still worn (simulated), just not drawn.
                let shown: Vec<bool> = (0..self.evaluated.len())
                    .map(|ei| {
                        let ci = self.piece_component.get(ei).copied().unwrap_or(0);
                        self.project.components.get(ci).map(|c| c.visible_3d).unwrap_or(true)
                    })
                    .collect();
                js_sys::Uint32Array::from(&s.triangle_indices_shown(&shown)[..])
            }
            None => js_sys::Uint32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_avatar_positions(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.avatar_mesh.positions.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_avatar_normals(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.avatar_mesh.normals.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_avatar_indices(&self) -> js_sys::Uint32Array {
        match &self.sim3d {
            Some(s) => js_sys::Uint32Array::from(s.avatar_mesh.indices.as_slice()),
            None => js_sys::Uint32Array::new_with_length(0),
        }
    }

    pub fn get_sim3d_avatar_torso_mask(&self) -> js_sys::Float32Array {
        match &self.sim3d {
            Some(s) => js_sys::Float32Array::from(s.avatar_mesh.torso_mask.as_slice()),
            None => js_sys::Float32Array::new_with_length(0),
        }
    }

    pub fn get_ui_snapshot_json(&self) -> String {
        let measurements: Vec<MeasurementItem> = self
            .project
            .measurements
            .entries()
            .into_iter()
            .map(|(k, l, v, mn, mx)| MeasurementItem {
                key: k.into(),
                label: l.into(),
                value_mm: v,
                min_mm: mn,
                max_mm: mx,
            })
            .collect();

        let ease: Vec<MeasurementItem> = self
            .project
            .comp()
            .ease
            .entries()
            .into_iter()
            .map(|(k, l, v, mn, mx)| MeasurementItem {
                key: k.into(),
                label: l.into(),
                value_mm: v,
                min_mm: mn,
                max_mm: mx,
            })
            .collect();

        let variables: Vec<VarSummary> = self
            .project
            .comp()
            .variables
            .iter()
            .map(|v| VarSummary {
                name: v.name.clone(),
                expr: v.expr.clone(),
                description: v.description.clone(),
                value: self.var_values.get(&v.name).copied(),
                error: self.var_errors.get(&v.name).cloned(),
            })
            .collect();

        let pieces: Vec<PieceSummary> = self
            .evaluated
            .iter()
            .enumerate()
            .map(|(ei, ep)| {
                let p = &ep.piece;
                let component_id = self
                    .project
                    .components
                    .get(self.piece_component.get(ei).copied().unwrap_or(0))
                    .map(|c| c.id.to_string())
                    .unwrap_or_default();
                let edges = p
                    .seam_loop
                    .edges
                    .iter()
                    .enumerate()
                    .map(|(ei, e)| EdgeInfo {
                        edge_idx: ei,
                        edge_id: e.id.to_string(),
                        label: e.label.clone(),
                        length_mm: ep.edge_lengths_mm.get(ei).copied().unwrap_or(0.0),
                        sa_width_mm: e.sa_width_mm,
                        corner_rule: e.corner_rule,
                        is_cubic: matches!(
                            e.segment,
                            modelist_core::model::CurveSegment::Cubic { .. }
                        ),
                        is_fold: p.mirror_fold_edge == Some(ei),
                    })
                    .collect();
                let darts = p
                    .darts
                    .iter()
                    .map(|d| DartInfo {
                        id: d.id.to_string(),
                        name: d.name.clone(),
                        edge_idx: d.resolve_edge_idx(&p.seam_loop).unwrap_or(d.edge_idx),
                        intake_mm: d.intake_mm,
                        apex_x: d.apex.x,
                        apex_y: d.apex.y,
                    })
                    .collect();
                PieceSummary {
                    id: p.id.to_string(),
                    component_id,
                    name: p.name.clone(),
                    code: p.code.clone(),
                    cut_quantity: p.cut_quantity,
                    visible: p.visible,
                    fabric_role: p.fabric_role.clone(),
                    grain_rotation: p.grainline.rotation_mode,
                    has_mirror_fold: p.mirror_fold_edge.is_some(),
                    perimeter_mm: ep.perimeter_mm,
                    area_mm2: ep.area_mm2,
                    cut_area_mm2: ep.cut_area_mm2,
                    notch_count: ep.projected_notches.len(),
                    drill_hole_count: p.drill_holes.len(),
                    warnings: ep.warnings.clone(),
                    edges,
                    darts,
                }
            })
            .collect();

        let resolve_edge_refs = |refs: &[EdgeRef]| -> Vec<EdgeRefInfo> {
            refs.iter()
                .filter_map(|er| {
                    let ep = self.evaluated.iter().find(|e| e.piece.id == er.piece_id)?;
                    let eidx = ep.piece.seam_loop.edge_index_by_id(er.edge_id)?;
                    let edge = ep.piece.seam_loop.edges.get(eidx)?;
                    Some(EdgeRefInfo {
                        piece_id: ep.piece.id.to_string(),
                        piece_code: ep.piece.code.clone(),
                        edge_id: edge.id.to_string(),
                        edge_idx: eidx,
                        edge_label: edge.label.clone(),
                        length_mm: ep.edge_lengths_mm.get(eidx).copied().unwrap_or(0.0),
                        dir: er.dir,
                    })
                })
                .collect()
        };

        let raw_pieces: Vec<_> = self.evaluated.iter().map(|e| e.piece.clone()).collect();
        let stitching_links: Vec<StitchLinkSummary> = self
            .stitching
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(bi, b)| {
                let la = interface_total_length(&b.interface_a, &raw_pieces);
                let lb = interface_total_length(&b.interface_b, &raw_pieces);
                if la < 1.0 || lb < 1.0 {
                    return None;
                }
                let ease_mm = lb - la;
                let expected = b.ease.expected_ease_mm(la);
                let status = if ease_mm < b.ease_min_mm {
                    "tight".into()
                } else if ease_mm > b.ease_max_mm {
                    "excess".into()
                } else {
                    "ok".into()
                };
                Some(StitchLinkSummary {
                    id: b.id.to_string(),
                    label: b.label.clone(),
                    interface_a_name: b.interface_a.name.clone(),
                    interface_b_name: b.interface_b.name.clone(),
                    edge_count_a: b.interface_a.edges.len(),
                    edge_count_b: b.interface_b.edges.len(),
                    edges_a: resolve_edge_refs(&b.interface_a.edges),
                    edges_b: resolve_edge_refs(&b.interface_b.edges),
                    len_a_mm: la,
                    len_b_mm: lb,
                    ease_mm,
                    ease_target_mm: expected,
                    ease: b.ease,
                    driver: b.driver,
                    seam_type: b.seam_type,
                    auto_partner_notches: b.auto_partner_notches,
                    status,
                    color_idx: bi,
                })
            })
            .collect();

        let ops = build_timeline_entries(&self.project, &self.evaluated, &self.stitching);

        let qa_matrix = compute_cross_size_qa(&self.project);
        let pom_rows = compute_pom_table(&self.project);

        let snap = UiSnapshot {
            project_name: self.project.name.clone(),
            template_id: self.project.comp().template_id.clone(),
            components: self
                .project
                .components
                .iter()
                .enumerate()
                .map(|(ci, c)| ComponentSummary {
                    id: c.id.to_string(),
                    name: c.name.clone(),
                    template_id: c.template_id.clone(),
                    layer: c.layer,
                    visible: c.visible,
                    visible_3d: c.visible_3d,
                    piece_count: self.piece_component.iter().filter(|&&k| k == ci).count(),
                })
                .collect(),
            active_component_id: self.project.comp().id.to_string(),
            grade_mode: self.project.grade_mode,
            stack_anchor: self.project.stack_anchor,
            active_size: self.project.size_table.active_size.clone(),
            sizes: self.project.size_table.sizes.clone(),
            visible_sizes: self.project.size_table.visible_sizes.clone(),
            fabric: self.project.comp().fabric.clone(),
            fabric_catalog: FabricSpec::catalog(),
            fabric_edited: fabric_edited(&self.project.comp().fabric),
            fabric_metrics: fabric_metrics(&self.project.comp().fabric),
            role_fabrics: self.role_fabric_infos(),
            measurements,
            ease,
            variables,
            pieces,
            stitching_links,
            ops,
            rollback_index: self.project.comp().rollback_index,
            qa_matrix,
            pom_rows,
            labels: self.render_buffers.labels.clone(),
            edge_dims: self.render_buffers.edge_dims.clone(),
            piece_fills: self.render_buffers.piece_fills.clone(),
            selected_piece_id: self.selected_piece.map(|id| id.to_string()),
            selected_edge_idx: self.selected_edge.map(|(_, ei)| ei),
            selected_binding_id: self.selected_binding.map(|id| id.to_string()),
            show_mirror_unfold: self.show_mirror_unfold,
            sim3d_active: self.sim3d.is_some(),
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            solve_ms: self.solve_ms,
        };
        serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into())
    }

    pub fn export_dxf_aama(&self) -> String {
        let shown: Vec<EvaluatedPiece> = self.evaluated.iter().filter(|e| e.piece.visible).cloned().collect();
        export_dxf_aama(&shown, &self.project.size_table.active_size)
    }

    pub fn export_aama_rul(&self) -> String {
        export_aama_rul(&self.project)
    }

    pub fn import_dxf_aama(&mut self, dxf_text: &str) -> Result<String, JsValue> {
        let mut imported = import_dxf_aama(dxf_text).map_err(|e| JsValue::from_str(&e))?;
        let count = imported.len();
        self.push_undo();
        self.dedupe_imported_ids(&mut imported, &mut []);
        let max_x = self
            .evaluated
            .iter()
            .flat_map(|e| e.seam_polyline.iter().map(|p| p.x))
            .fold(0.0_f64, f64::max);
        for (idx, mut p) in imported.into_iter().enumerate() {
            let min_x = p
                .seam_loop
                .edges
                .iter()
                .map(|e| e.start.x)
                .fold(f64::INFINITY, f64::min);
            let dx = if min_x.is_finite() {
                (max_x + 90.0 + (idx as f64) * 320.0) - min_x
            } else {
                0.0
            };
            for e in &mut p.seam_loop.edges {
                e.start.x += dx;
                e.segment
                    .transform(|pt| modelist_core::model::Point2::new(pt.x + dx, pt.y));
            }
            p.grainline.start.x += dx;
            p.grainline.end.x += dx;
            self.project.comp_mut().custom_pieces.push(p);
        }
        self.recompute();
        if let Some(last) = self.evaluated.last() {
            self.selected_piece = Some(last.piece.id);
            self.rebuild_render_buffers();
        }
        Ok(format!("Imported {count} piece(s) via Schneider Bezier refit"))
    }

    pub fn import_garmentcode_json(&mut self, json_text: &str) -> Result<String, JsValue> {
        let (mut pieces, mut bindings) =
            import_garmentcode_json(json_text).map_err(|e| JsValue::from_str(&e))?;
        let p_count = pieces.len();
        let b_count = bindings.len();
        self.push_undo();
        self.dedupe_imported_ids(&mut pieces, &mut bindings);
        let max_x = self
            .evaluated
            .iter()
            .flat_map(|e| e.seam_polyline.iter().map(|p| p.x))
            .fold(0.0_f64, f64::max);
        for mut p in pieces {
            let dx = max_x + 120.0;
            for e in &mut p.seam_loop.edges {
                e.start.x += dx;
                e.segment
                    .transform(|pt| modelist_core::model::Point2::new(pt.x + dx, pt.y));
            }
            p.grainline.start.x += dx;
            p.grainline.end.x += dx;
            self.project.comp_mut().custom_pieces.push(p);
        }
        for b in bindings {
            let oid = self.next_op_id("gc_binding");
            self.insert_op(ParametricOp::AddSeamBinding {
                id: oid,
                binding: b,
            });
        }
        self.recompute();
        Ok(format!(
            "Imported {p_count} GarmentCode panel(s) & {b_count} stitch binding(s)"
        ))
    }

    pub fn export_svg_mm(&self, include_nest: bool) -> String {
        let empty = HashMap::new();
        let nest = if include_nest {
            &self.graded_nest
        } else {
            &empty
        };
        export_svg_mm(
            &self.project.name,
            &self.project.size_table.active_size,
            &self.evaluated,
            nest,
        )
    }

    pub fn export_pom_csv(&self) -> String {
        export_pom_csv(&self.project)
    }

    pub fn get_pdf_layout_json(&self, preset: &str, unfold_on_fold: bool) -> String {
        let plan = compute_pdf_layout_plan(
            preset,
            unfold_on_fold,
            &self.project.size_table.active_size,
            &self.evaluated,
            &self.graded_nest,
        );
        serde_json::to_string(&plan).unwrap_or_else(|_| "{}".into())
    }

    pub fn compute_marker_plan_json(&self, config_json: &str) -> String {
        let cfg: MarkerConfig = serde_json::from_str(config_json).unwrap_or_default();
        let plan = compute_marker_plan(&self.project, &cfg);
        serde_json::to_string(&plan).unwrap_or_else(|_| "{}".into())
    }

    pub fn export_marker_svg(&self, config_json: &str) -> String {
        let cfg: MarkerConfig = serde_json::from_str(config_json).unwrap_or_default();
        let plan = compute_marker_plan(&self.project, &cfg);
        export_marker_svg(&plan, &self.project.name)
    }
}

/// Starter library catalog (blocks + styles) as JSON, so the UI never hard-codes template ids.
#[wasm_bindgen]
pub fn template_catalog_json() -> String {
    serde_json::to_string(modelist_core::library::template_catalog())
        .unwrap_or_else(|_| "[]".into())
}

impl ModelistSession {
    fn make_project(template_id: &str) -> PatternProject {
        modelist_core::library::new_project(template_id)
    }

    /// Applies a fabric edit: migrates/clamps, mirrors fabric values into formula variables,
    /// rebuilds the pattern if any formula depends on them, else updates the 3D drape live.
    fn after_fabric_change(&mut self) {
        self.project.normalize_fabrics();
        let vars_changed = self.project.sync_fabric_variables();
        let formulas_depend = self
            .project
            .comp()
            .variables
            .iter()
            .any(|v| !is_fabric_variable(&v.name) && v.expr.contains("fabric_"));
        if vars_changed && formulas_depend {
            self.recompute();
        } else if let Some(ref mut sim) = self.sim3d {
            sim.set_materials(MaterialSet::from_project(&self.project));
        }
    }

    fn role_fabric_infos(&self) -> Vec<RoleFabricInfo> {
        let mut out: Vec<RoleFabricInfo> = Vec::new();
        let active = self.project.active_component;
        for (ei, ep) in self.evaluated.iter().enumerate() {
            if self.piece_component.get(ei).copied().unwrap_or(0) != active {
                continue;
            }
            let key = fabric_role_key(&ep.piece.fabric_role);
            if key == "shell" || out.iter().any(|r| r.role == key) {
                continue;
            }
            out.push(RoleFabricInfo {
                fabric: self
                    .project
                    .comp()
                    .role_fabrics
                    .iter()
                    .find(|r| r.role == key)
                    .map(|r| r.fabric.clone()),
                label: ep.piece.fabric_role.clone(),
                role: key,
            });
        }
        out
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.project.clone());
        if self.undo_stack.len() > 60 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// IDs already used by the project, including template-drafted pieces, edges and seams.
    fn used_ids(&self) -> HashSet<Uuid> {
        let mut ids = self.project.stored_ids();
        for ep in &self.evaluated {
            ids.insert(ep.piece.id);
            ids.extend(ep.piece.seam_loop.edges.iter().map(|e| e.id));
        }
        for b in &self.stitching.bindings {
            ids.insert(b.id);
        }
        ids
    }

    /// A deterministic ID that nothing in the project uses yet. The counter restarts with every
    /// session, so after opening a file it skips the IDs that file already holds.
    fn next_op_id(&mut self, tag: &str) -> Uuid {
        let used = self.used_ids();
        loop {
            self.op_counter += 1;
            let id = det_id(&format!("op:{tag}:{}", self.op_counter));
            if !used.contains(&id) {
                return id;
            }
        }
    }

    /// Imported pieces carry IDs derived from the file content, so importing the same file twice
    /// would duplicate them. On any collision, move the whole import into a fresh namespace.
    fn dedupe_imported_ids(&mut self, pieces: &mut [PatternPiece], bindings: &mut [SeamBinding]) {
        let used = self.used_ids();
        let mut seen = HashSet::new();
        let clash = pieces.iter().any(|p| {
            used.contains(&p.id)
                || !seen.insert(p.id)
                || p.seam_loop.edges.iter().any(|e| used.contains(&e.id))
        }) || bindings.iter().any(|b| used.contains(&b.id));
        if clash {
            let ns = self.next_op_id("import");
            remap_entity_ids(pieces, bindings, &ns.to_string());
        }
    }

    /// Component that owns a piece (as last evaluated).
    fn component_of_piece(&self, pid: Uuid) -> Option<usize> {
        self.evaluated
            .iter()
            .position(|e| e.piece.id == pid)
            .and_then(|ei| self.piece_component.get(ei).copied())
    }

    fn insert_op(&mut self, op: ParametricOp) {
        // An op belongs to the garment of the piece it edits (else the active garment).
        if let Some(ci) = op.target_piece_ids().first().and_then(|pid| self.component_of_piece(*pid)) {
            self.project.active_component = ci;
        }
        if let Some(rb) = self.project.comp_mut().rollback_index {
            let idx = rb.min(self.project.comp_mut().ops.len());
            self.project.comp_mut().ops.insert(idx, op);
            self.project.comp_mut().rollback_index = Some(idx + 1);
        } else {
            let ops = &mut self.project.comp_mut().ops;
            ops.push(op);
        }
    }

    fn upsert_op(&mut self, op: ParametricOp) {
        if let Some(pos) = self.project.comp_mut().ops.iter().position(|o| o.id() == op.id()) {
            self.project.comp_mut().ops[pos] = op;
        } else {
            self.insert_op(op);
        }
    }

    fn recompute(&mut self) {
        let t0 = now_ms();
        let draft = build_project_pieces(&self.project, &self.project.measurements);
        let pieces = draft.pieces;
        let stitching = draft.stitching;
        let mut evaluated: Vec<EvaluatedPiece> = pieces
            .iter()
            .map(|p| evaluate_piece(p, &stitching.bindings, &pieces))
            .collect();

        // Automatically synthesize partner notches via GarmentCode fraction projection!
        synthesize_partner_notches(&pieces, &stitching.bindings, &mut evaluated);

        let graded_nest = evaluate_graded_nest(&self.project, &evaluated, &stitching.bindings);
        let snap_index = SpatialSnapIndex::build(&evaluated);

        let active = self.project.active_component.min(self.project.components.len() - 1);
        self.evaluated = evaluated;
        self.stitching = stitching;
        self.piece_component = draft.piece_component;
        self.graded_nest = graded_nest;
        self.var_values = draft.var_values.get(active).cloned().unwrap_or_default();
        self.var_errors = draft.var_errors.get(active).cloned().unwrap_or_default();
        self.snap_index = snap_index;
        self.rebuild_render_buffers();

        // Live 2D -> 3D sync ONLY if the user currently has the 3D Fit Room open!
        if let Some(prev_sim) = self.sim3d.take() {
            let pose = prev_sim.avatar.pose;
            let body_form = prev_sim.avatar.body_form;
            self.sim3d = Some(self.build_sim(pose, body_form, Some(prev_sim)));
        }

        self.solve_ms = (now_ms() - t0).max(0.01);
    }

    /// The 3D fit of every garment: each one dressed at its component's layer, with its own
    /// fabrics (roles qualified by garment). With the previous fit, only the layers whose inputs
    /// changed are dressed again (the ones under them keep their drape); when nothing the 3D fit
    /// depends on changed, the previous fit is kept as it is.
    fn build_sim(
        &mut self,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
        prev: Option<ClothSimSession>,
    ) -> ClothSimSession {
        let mut pieces = Vec::with_capacity(self.evaluated.len());
        let mut layers = Vec::with_capacity(self.evaluated.len());
        for (ei, ep) in self.evaluated.iter().enumerate() {
            let ci = self.piece_component.get(ei).copied().unwrap_or(0);
            let mut p = ep.piece.clone();
            p.fabric_role = garment_role(ci, &p.fabric_role);
            pieces.push(p);
            layers.push(self.project.components.get(ci).map(|c| c.layer).unwrap_or(0));
        }
        let keys = self.sim_layer_keys(&pieces, &layers, pose, body_form);
        let materials = MaterialSet::from_project(&self.project);
        let sim = match prev {
            Some(prev) if keys == self.sim_keys => prev,
            Some(prev) => {
                let changed = keys
                    .iter()
                    .zip(&self.sim_keys)
                    .position(|(a, b)| a != b)
                    .unwrap_or(keys.len().min(self.sim_keys.len()));
                match keys.get(changed) {
                    Some(&(level, _)) if changed > 0 => ClothSimSession::redress_from(
                        &prev,
                        level,
                        &pieces,
                        &layers,
                        &self.stitching,
                        &self.project.measurements,
                        &materials,
                        pose,
                        body_form,
                    ),
                    _ => ClothSimSession::start_dressing(
                        &pieces,
                        &layers,
                        &self.stitching,
                        &self.project.measurements,
                        &materials,
                        pose,
                        body_form,
                    ),
                }
            }
            None => ClothSimSession::start_dressing(
                &pieces,
                &layers,
                &self.stitching,
                &self.project.measurements,
                &materials,
                pose,
                body_form,
            ),
        };
        self.sim_keys = keys;
        sim
    }

    /// Fingerprint of everything the 3D fit of each layer depends on (inner layer first): its
    /// pieces, the seams touching them, its garments' fabrics, plus the body and pose.
    fn sim_layer_keys(
        &self,
        pieces: &[PatternPiece],
        layers: &[u8],
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Vec<(u8, u64)> {
        use std::hash::{Hash, Hasher};
        let mut levels: Vec<u8> = layers.to_vec();
        levels.sort_unstable();
        levels.dedup();
        let common = format!(
            "{}|{:?}|{:?}",
            serde_json::to_string(&self.project.measurements).unwrap_or_default(),
            pose,
            body_form
        );
        levels
            .into_iter()
            .map(|level| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                common.hash(&mut h);
                let ids: HashSet<Uuid> = pieces
                    .iter()
                    .zip(layers)
                    .filter(|(_, &l)| l == level)
                    .map(|(p, _)| p.id)
                    .collect();
                for (p, &l) in pieces.iter().zip(layers) {
                    if l == level {
                        serde_json::to_string(p).unwrap_or_default().hash(&mut h);
                    }
                }
                for b in &self.stitching.bindings {
                    let touches = b
                        .interface_a
                        .edges
                        .iter()
                        .chain(&b.interface_b.edges)
                        .any(|e| ids.contains(&e.piece_id));
                    if touches {
                        serde_json::to_string(b).unwrap_or_default().hash(&mut h);
                    }
                }
                for c in self.project.components.iter().filter(|c| c.layer == level) {
                    serde_json::to_string(&(&c.fabric, &c.role_fabrics)).unwrap_or_default().hash(&mut h);
                }
                (level, h.finish())
            })
            .collect()
    }

    fn rebuild_render_buffers(&mut self) {
        let walk_ref = self.walk_frame.as_ref().and_then(|(frame, gid)| {
            self.evaluated
                .iter()
                .find(|e| e.piece.id == *gid)
                .map(|ep| (frame, ep))
        });

        self.render_buffers = build_render_buffers(
            &self.evaluated,
            &self.stitching,
            &self.graded_nest,
            &self.project.size_table.active_size,
            &self.project.size_table.sizes,
            self.selected_piece,
            self.selected_edge,
            self.selected_binding,
            &self.draft_seam_a,
            &self.draft_seam_b,
            walk_ref,
            self.show_mirror_unfold,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    0.0
}

#[cfg(test)]
mod id_tests {
    use super::*;

    fn all_op_ids(s: &ModelistSession) -> Vec<Uuid> {
        s.project.comp().ops.iter().map(|o| o.id()).collect()
    }

    fn assert_unique(ids: &[Uuid]) {
        let set: HashSet<_> = ids.iter().collect();
        assert_eq!(set.len(), ids.len(), "duplicate IDs: {ids:?}");
    }

    #[test]
    fn op_ids_stay_unique_after_reopening_a_file() {
        let mut a = ModelistSession::new("boxy_tee");
        let pid = a.evaluated[0].piece.id.to_string();
        for i in 0..4 {
            a.add_notch(&pid, 0, 0.2 + 0.1 * i as f64, "v", 1, 6.0);
        }
        let json = a.serialize_project_json();

        // A fresh session restarts its counter at 1, as the browser does after opening a file.
        let mut b = ModelistSession::new("boxy_tee");
        b.load_project_json(&json).unwrap();
        let pid_b = b.evaluated[0].piece.id.to_string();
        for i in 0..4 {
            b.add_notch(&pid_b, 1, 0.2 + 0.1 * i as f64, "v", 1, 6.0);
        }
        assert_eq!(b.project.comp().ops.len(), 8);
        assert_unique(&all_op_ids(&b));
    }

    #[test]
    fn duplicated_pieces_never_reuse_a_piece_id() {
        let mut a = ModelistSession::new("boxy_tee");
        let pid = a.evaluated[0].piece.id.to_string();
        a.duplicate_piece(&pid);
        let json = a.serialize_project_json();
        let mut b = ModelistSession::new("boxy_tee");
        b.load_project_json(&json).unwrap();
        b.duplicate_piece(&pid);
        let ids: Vec<Uuid> = b.evaluated.iter().map(|e| e.piece.id).collect();
        assert_unique(&ids);
    }

    #[test]
    fn importing_the_same_dxf_twice_keeps_ids_unique() {
        let mut s = ModelistSession::new("boxy_tee");
        let dxf = s.export_dxf_aama();
        let before = s.evaluated.len();
        s.import_dxf_aama(&dxf).unwrap();
        s.import_dxf_aama(&dxf).unwrap();
        assert!(s.evaluated.len() > before);
        let ids: Vec<Uuid> = s.evaluated.iter().map(|e| e.piece.id).collect();
        assert_unique(&ids);
        let edges: Vec<Uuid> = s
            .evaluated
            .iter()
            .flat_map(|e| e.piece.seam_loop.edges.iter().map(|x| x.id))
            .collect();
        assert_unique(&edges);
    }
}

#[cfg(test)]
mod component_tests {
    use super::*;

    #[test]
    fn a_second_garment_drafts_beside_the_first_and_keeps_its_own_history() {
        let mut s = ModelistSession::new("boxy_tee");
        let first_count = s.evaluated.len();
        let first_max_x = s.evaluated.iter().flat_map(|e| e.seam_polyline.iter().map(|p| p.x)).fold(f64::MIN, f64::max);
        let id = s.add_component("wide_pleat_trouser", "");
        assert_eq!(s.project.components.len(), 2);
        assert_eq!(s.project.comp().id.to_string(), id, "the new garment is active");
        assert_eq!(s.project.comp().layer, 1, "it goes on top in the 3D fit");
        assert!(s.evaluated.len() > first_count);
        let second_min_x = s
            .evaluated
            .iter()
            .zip(&s.piece_component)
            .filter(|(_, &c)| c == 1)
            .flat_map(|(e, _)| e.seam_polyline.iter().map(|p| p.x))
            .fold(f64::MAX, f64::min);
        assert!(second_min_x > first_max_x, "drafted to the right of the first garment");

        // A notch on a trouser piece lands in the trouser timeline only.
        let trouser_piece = s.evaluated.iter().zip(&s.piece_component).find(|(_, &c)| c == 1).unwrap().0.piece.id;
        s.set_active_component(&s.project.components[0].id.to_string());
        s.add_notch(&trouser_piece.to_string(), 0, 0.5, "v", 1, 6.0);
        assert_eq!(s.project.components[0].ops.len(), 0);
        assert_eq!(s.project.components[1].ops.len(), 1);
        assert_eq!(s.project.active_component, 1, "editing a piece activates its garment");

        // Saving and reopening keeps both garments; undo removes the second one.
        let json = s.serialize_project_json();
        let mut b = ModelistSession::new("bodice_sleeve");
        b.load_project_json(&json).unwrap();
        assert_eq!(b.project.components.len(), 2);
        assert_eq!(b.evaluated.len(), s.evaluated.len());
        s.undo();
        s.undo();
        assert_eq!(s.project.components.len(), 1);
    }

    #[test]
    fn hiding_a_garment_in_3d_keeps_it_worn() {
        let mut s = ModelistSession::new("boxy_tee");
        let outer = s.add_component("boxy_tee", "Outer tee");
        s.init_sim3d("a_pose", "female");
        let all = s.sim3d.as_ref().unwrap().triangle_indices().len();
        s.set_component_visible_3d(&outer, false);
        let shown = s.sim3d.as_ref().unwrap().triangle_indices_shown(
            &(0..s.evaluated.len()).map(|ei| s.project.components[s.piece_component[ei]].visible_3d).collect::<Vec<_>>(),
        );
        assert!(shown.len() < all && !shown.is_empty());
        // Hidden everywhere: its pieces leave the canvas and the 3D fit.
        s.set_component_visible(&outer, false);
        assert!(s.evaluated.iter().zip(&s.piece_component).all(|(e, &c)| e.piece.visible == (c == 0)));
    }

    #[test]
    fn editing_the_outer_garment_keeps_the_inner_drape() {
        let mut s = ModelistSession::new("boxy_tee");
        let trousers = s.add_component("wide_pleat_trouser", "");
        s.set_component_layer(&trousers, 1);
        s.init_sim3d("a_pose", "female");
        while s.sim3d.as_ref().unwrap().dressing_progress().is_some() {
            s.step_sim3d(4);
        }
        let inner = s.sim3d.as_ref().unwrap().vertex_layers().iter().filter(|&&l| l == 0).count();
        let before: Vec<f32> = s.sim3d.as_ref().unwrap().flat_positions[..inner * 3].to_vec();
        let steps_before = s.sim3d.as_ref().unwrap().stats().step_count;

        // Renaming does not touch the 3D fit: it is kept as it is.
        s.rename_component(&trousers, "Wide trousers");
        s.set_component_layer(&trousers, 1);
        assert_eq!(s.sim3d.as_ref().unwrap().stats().step_count, steps_before);

        // An ease change on the trousers re-dresses the trousers only.
        s.set_active_component(&trousers);
        s.set_variable("wt_leg_opening", "720", "Hem circumference (mm)");
        let sim = s.sim3d.as_ref().unwrap();
        assert_eq!(sim.dressing_progress(), Some((2, 2)));
        assert_eq!(&sim.flat_positions[..inner * 3], &before[..], "the tee keeps its drape");
    }

    #[test]
    fn a_container_file_reopens_with_every_garment() {
        let mut s = ModelistSession::new("boxy_tee");
        s.add_component("wide_pleat_trouser", "Trousers");
        let bytes = s.save_project_container("2026-11-24T12:00:00Z", r#"{"view":"split"}"#, &[]).unwrap();
        let mut b = ModelistSession::new("bodice_sleeve");
        let session = b.load_project_file(&bytes).unwrap();
        assert_eq!(session, r#"{"view":"split"}"#);
        assert_eq!(b.project, s.project);
        assert_eq!(b.evaluated.len(), s.evaluated.len());
        // Plain JSON (old files, the debug export) still opens through the same call.
        let mut c = ModelistSession::new("bodice_sleeve");
        c.load_project_file(s.serialize_project_json().as_bytes()).unwrap();
        assert_eq!(c.project.components.len(), 2);
    }

    #[test]
    fn a_saved_garment_inserts_into_another_document_with_its_history() {
        let mut a = ModelistSession::new("boxy_tee");
        let piece = a.evaluated[0].piece.id.to_string();
        a.add_notch(&piece, 0, 0.5, "v", 1, 6.0);
        let tee = a.project.components[0].id.to_string();
        let file = a.export_component_file(&tee, &[]).unwrap();

        let mut b = ModelistSession::new("wide_pleat_trouser");
        let before = b.evaluated.len();
        b.insert_component_file(&file).unwrap();
        b.insert_component_file(&file).unwrap();
        assert_eq!(b.project.components.len(), 3, "the same garment can be inserted twice");
        assert_eq!(b.project.components[1].ops.len(), 1, "its history comes with it");
        let ids: HashSet<Uuid> = b.evaluated.iter().map(|e| e.piece.id).collect();
        assert_eq!(ids.len(), b.evaluated.len());
        assert!(b.evaluated.len() > before);
        let notches = |c: usize| {
            b.evaluated.iter().zip(&b.piece_component).filter(|(_, &k)| k == c).map(|(e, _)| e.projected_notches.len()).sum::<usize>()
        };
        assert_eq!(notches(1), notches(2));
        // Duplicating works the same way; undo removes the duplicate.
        let dup = b.duplicate_component(&b.project.components[1].id.to_string());
        assert!(!dup.is_empty());
        assert_eq!(b.project.components.len(), 4);
        b.undo();
        assert_eq!(b.project.components.len(), 3);
    }
}
