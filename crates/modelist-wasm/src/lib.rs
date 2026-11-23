use modelist_core::blocks::{build_default_variables, build_template_pieces};
use modelist_core::curve::edge_total_length;
use modelist_core::grading::{compute_cross_size_qa, evaluate_graded_nest, CrossSizeQaRow};
use modelist_core::interop::{
    compute_pdf_layout_plan, compute_pom_table, export_dxf_aama, export_pom_csv, export_svg_mm,
    import_dxf_aama, PomRow,
};
use modelist_core::model::{
    det_id, BodyMeasurements, CornerRule, EaseTable, EvaluatedPiece, GradeMode, HingeMode,
    NotchStyle, ParametricOp, ParametricVar, PatternProject, SizeTable, StackAnchor,
    StitchingGraph,
};
use modelist_core::offset::evaluate_piece;
use modelist_core::ops::{compute_seam_walk_frame, SeamWalkFrame};
use modelist_core::render_buf::{build_render_buffers, CanvasLabel, RenderBuffers};
use modelist_core::snap::{hit_test_pieces, SpatialSnapIndex};
use serde::Serialize;
use std::collections::HashMap;
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
    name: String,
    code: String,
    cut_quantity: u16,
    visible: bool,
    fabric_role: String,
    perimeter_mm: f64,
    area_mm2: f64,
    cut_area_mm2: f64,
    notch_count: usize,
    warnings: Vec<String>,
    edges: Vec<EdgeInfo>,
    darts: Vec<DartInfo>,
}

#[derive(Serialize)]
struct StitchLinkSummary {
    id: String,
    label: String,
    piece_a_id: String,
    piece_a_name: String,
    edge_a: usize,
    piece_b_id: String,
    piece_b_name: String,
    edge_b: usize,
    len_a_mm: f64,
    len_b_mm: f64,
    ease_mm: f64,
    ease_target_mm: f64,
    status: String,
}

#[derive(Serialize)]
struct OpSummary {
    id: String,
    summary: String,
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
struct UiSnapshot {
    project_name: String,
    template_id: String,
    grade_mode: GradeMode,
    stack_anchor: StackAnchor,
    active_size: String,
    sizes: Vec<String>,
    visible_sizes: Vec<String>,
    measurements: Vec<MeasurementItem>,
    ease: Vec<MeasurementItem>,
    variables: Vec<VarSummary>,
    pieces: Vec<PieceSummary>,
    stitching_links: Vec<StitchLinkSummary>,
    ops: Vec<OpSummary>,
    qa_matrix: Vec<CrossSizeQaRow>,
    pom_rows: Vec<PomRow>,
    labels: Vec<CanvasLabel>,
    selected_piece_id: Option<String>,
    selected_edge_idx: Option<usize>,
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
    graded_nest: HashMap<String, Vec<EvaluatedPiece>>,
    var_values: HashMap<String, f64>,
    var_errors: HashMap<String, String>,
    snap_index: SpatialSnapIndex,
    render_buffers: RenderBuffers,
    selected_piece: Option<Uuid>,
    selected_edge: Option<(Uuid, usize)>,
    walk_frame: Option<(SeamWalkFrame, Uuid)>,
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
            graded_nest: HashMap::new(),
            var_values: HashMap::new(),
            var_errors: HashMap::new(),
            snap_index: SpatialSnapIndex::default(),
            render_buffers: RenderBuffers::default(),
            selected_piece: None,
            selected_edge: None,
            walk_frame: None,
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
        self.recompute();
        self.selected_piece = self.evaluated.first().map(|e| e.piece.id);
        self.rebuild_render_buffers();
    }

    pub fn set_project_name(&mut self, name: &str) {
        self.project.name = name.trim().to_string();
    }

    pub fn serialize_project_json(&self) -> String {
        serde_json::to_string_pretty(&self.project).unwrap_or_else(|_| "{}".into())
    }

    pub fn load_project_json(&mut self, json: &str) -> Result<(), JsValue> {
        let parsed: PatternProject = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Invalid .modelist JSON: {e}")))?;
        self.push_undo();
        self.project = parsed;
        self.walk_frame = None;
        self.selected_edge = None;
        self.recompute();
        self.selected_piece = self.evaluated.first().map(|e| e.piece.id);
        self.rebuild_render_buffers();
        Ok(())
    }

    pub fn set_measurement(&mut self, key: &str, val: f64) {
        if !self.project.measurements.set_by_key(key, val) {
            return;
        }
        self.recompute();
    }

    pub fn set_ease(&mut self, key: &str, val: f64) {
        if !self.project.ease.set_by_key(key, val) {
            return;
        }
        self.recompute();
    }

    pub fn set_variable(&mut self, name: &str, expr: &str, description: &str) {
        self.push_undo();
        let clean = name.trim().to_string();
        if clean.is_empty() {
            return;
        }
        if let Some(existing) = self.project.variables.iter_mut().find(|v| v.name == clean) {
            existing.expr = expr.to_string();
            if !description.is_empty() {
                existing.description = description.to_string();
            }
        } else {
            self.project.variables.push(ParametricVar {
                name: clean,
                expr: expr.to_string(),
                description: description.to_string(),
            });
        }
        self.recompute();
    }

    pub fn delete_variable(&mut self, name: &str) {
        self.push_undo();
        self.project.variables.retain(|v| v.name != name);
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
        self.selected_edge = match (self.selected_piece, edge_idx) {
            (Some(pid), Some(ei)) => Some((pid, ei)),
            _ => None,
        };
        self.rebuild_render_buffers();
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
        self.project.ops.push(ParametricOp::SetSeamAllowance {
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
        self.project.ops.push(ParametricOp::AddNotch {
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
        self.project.ops.push(ParametricOp::PivotDart {
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
        self.project.ops.push(ParametricOp::SlashSpread {
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
        self.push_undo();
        self.project.ops.retain(|o| o.id() != oid);
        self.recompute();
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
        let Some(link) = self.stitching.links.iter().find(|l| l.id == lid) else {
            return "null".into();
        };
        let Some(ep_a) = self.evaluated.iter().find(|e| e.piece.id == link.piece_a) else {
            return "null".into();
        };
        let Some(ep_b) = self.evaluated.iter().find(|e| e.piece.id == link.piece_b) else {
            return "null".into();
        };

        let Some(frame) = compute_seam_walk_frame(link, &ep_a.piece, &ep_b.piece, progress_u)
        else {
            return "null".into();
        };
        let json = serde_json::to_string(&frame).unwrap_or_else(|_| "null".into());
        self.walk_frame = Some((frame, ep_b.piece.id));
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
            .map(|ep| {
                let p = &ep.piece;
                let edges = p
                    .seam_loop
                    .edges
                    .iter()
                    .enumerate()
                    .map(|(ei, e)| EdgeInfo {
                        edge_idx: ei,
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
                        edge_idx: d.edge_idx,
                        intake_mm: d.intake_mm,
                        apex_x: d.apex.x,
                        apex_y: d.apex.y,
                    })
                    .collect();
                PieceSummary {
                    id: p.id.to_string(),
                    name: p.name.clone(),
                    code: p.code.clone(),
                    cut_quantity: p.cut_quantity,
                    visible: p.visible,
                    fabric_role: p.fabric_role.clone(),
                    perimeter_mm: ep.perimeter_mm,
                    area_mm2: ep.area_mm2,
                    cut_area_mm2: ep.cut_area_mm2,
                    notch_count: p.notches.len(),
                    warnings: ep.warnings.clone(),
                    edges,
                    darts,
                }
            })
            .collect();

        let stitching_links: Vec<StitchLinkSummary> = self
            .stitching
            .links
            .iter()
            .filter_map(|link| {
                let pa = self.evaluated.iter().find(|e| e.piece.id == link.piece_a)?;
                let pb = self.evaluated.iter().find(|e| e.piece.id == link.piece_b)?;
                let ea = pa.piece.seam_loop.edges.get(link.edge_a)?;
                let eb = pb.piece.seam_loop.edges.get(link.edge_b)?;
                let la = edge_total_length(ea);
                let lb = edge_total_length(eb);
                let ease_mm = lb - la;
                let status = if ease_mm < link.ease_min_mm {
                    "tight".into()
                } else if ease_mm > link.ease_max_mm {
                    "excess".into()
                } else {
                    "ok".into()
                };
                Some(StitchLinkSummary {
                    id: link.id.to_string(),
                    label: link.label.clone(),
                    piece_a_id: pa.piece.id.to_string(),
                    piece_a_name: pa.piece.name.clone(),
                    edge_a: link.edge_a,
                    piece_b_id: pb.piece.id.to_string(),
                    piece_b_name: pb.piece.name.clone(),
                    edge_b: link.edge_b,
                    len_a_mm: la,
                    len_b_mm: lb,
                    ease_mm,
                    ease_target_mm: link.ease_target_mm,
                    status,
                })
            })
            .collect();

        let ops: Vec<OpSummary> = self
            .project
            .ops
            .iter()
            .map(|o| OpSummary {
                id: o.id().to_string(),
                summary: o.summary(),
            })
            .collect();

        let qa_matrix = compute_cross_size_qa(&self.project);
        let pom_rows = compute_pom_table(&self.project);

        let snap = UiSnapshot {
            project_name: self.project.name.clone(),
            template_id: self.project.template_id.clone(),
            grade_mode: self.project.grade_mode,
            stack_anchor: self.project.stack_anchor,
            active_size: self.project.size_table.active_size.clone(),
            sizes: self.project.size_table.sizes.clone(),
            visible_sizes: self.project.size_table.visible_sizes.clone(),
            measurements,
            ease,
            variables,
            pieces,
            stitching_links,
            ops,
            qa_matrix,
            pom_rows,
            labels: self.render_buffers.labels.clone(),
            selected_piece_id: self.selected_piece.map(|id| id.to_string()),
            selected_edge_idx: self.selected_edge.map(|(_, ei)| ei),
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            solve_ms: self.solve_ms,
        };
        serde_json::to_string(&snap).unwrap_or_else(|_| "{}".into())
    }

    pub fn export_dxf_aama(&self) -> String {
        export_dxf_aama(&self.evaluated, &self.project.size_table.active_size)
    }

    pub fn import_dxf_aama(&mut self, dxf_text: &str) -> Result<String, JsValue> {
        let imported = import_dxf_aama(dxf_text).map_err(|e| JsValue::from_str(&e))?;
        let count = imported.len();
        self.push_undo();
        // Shift imported pieces cleanly past current project max X
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
                e.segment.transform(|pt| modelist_core::model::Point2::new(pt.x + dx, pt.y));
            }
            p.grainline.start.x += dx;
            p.grainline.end.x += dx;
            self.project.custom_pieces.push(p);
        }
        self.recompute();
        if let Some(last) = self.evaluated.last() {
            self.selected_piece = Some(last.piece.id);
            self.rebuild_render_buffers();
        }
        Ok(format!("Imported {count} piece(s) via Schneider Bezier refit"))
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
}

impl ModelistSession {
    fn make_project(template_id: &str) -> PatternProject {
        let tmpl = if template_id == "aline_skirt" {
            "aline_skirt"
        } else {
            "bodice_sleeve"
        };
        let name = if tmpl == "aline_skirt" {
            "Tailored A-Line Skirt"
        } else {
            "Fitted Bodice & Set-In Sleeve"
        };
        PatternProject {
            id: det_id(&format!("project:{tmpl}")),
            name: name.into(),
            template_id: tmpl.into(),
            measurements: BodyMeasurements::default(),
            ease: EaseTable::default(),
            variables: build_default_variables(tmpl),
            size_table: SizeTable::default(),
            grade_mode: GradeMode::ParametricMtm,
            stack_anchor: StackAnchor::GrainlineMidpoint,
            ops: Vec::new(),
            custom_pieces: Vec::new(),
        }
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.project.clone());
        if self.undo_stack.len() > 60 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn next_op_id(&mut self, tag: &str) -> Uuid {
        self.op_counter += 1;
        det_id(&format!("op:{tag}:{}", self.op_counter))
    }

    fn upsert_op(&mut self, op: ParametricOp) {
        if let Some(pos) = self.project.ops.iter().position(|o| o.id() == op.id()) {
            self.project.ops[pos] = op;
        } else {
            self.project.ops.push(op);
        }
    }

    fn recompute(&mut self) {
        let t0 = now_ms();
        let (pieces, stitching, var_values, var_errors) = build_template_pieces(
            &self.project.template_id,
            &self.project.measurements,
            &self.project.ease,
            &self.project.variables,
            &self.project.ops,
            &self.project.custom_pieces,
        );
        let evaluated: Vec<EvaluatedPiece> = pieces
            .iter()
            .map(|p| evaluate_piece(p, &stitching.links, &pieces))
            .collect();

        let graded_nest = evaluate_graded_nest(&self.project, &evaluated, &stitching.links);
        let snap_index = SpatialSnapIndex::build(&evaluated);

        self.evaluated = evaluated;
        self.stitching = stitching;
        self.graded_nest = graded_nest;
        self.var_values = var_values;
        self.var_errors = var_errors;
        self.snap_index = snap_index;
        self.rebuild_render_buffers();
        self.solve_ms = (now_ms() - t0).max(0.01);
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
            &self.graded_nest,
            &self.project.size_table.active_size,
            &self.project.size_table.sizes,
            self.selected_piece,
            self.selected_edge,
            walk_ref,
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
