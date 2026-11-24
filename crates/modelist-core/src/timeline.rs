use crate::model::{
    CornerRule, EvaluatedPiece, HingeMode, NotchStyle, ParametricOp, PatternProject,
    GrainRotationMode, StitchingGraph,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpSeverity {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpTimelineEntry {
    pub id: String,
    pub index: usize,
    pub kind: String,
    pub summary: String,
    pub piece_ids: Vec<String>,
    pub binding_id: Option<String>,
    pub suppressed: bool,
    pub after_rollback: bool,
    pub reorder_min: usize,
    pub reorder_max: usize,
    pub severity: OpSeverity,
    pub diagnostic: Option<String>,
    pub editable: bool,
    pub payload: serde_json::Value,
}

fn op_edge_idx(op: &ParametricOp) -> Option<(Uuid, usize)> {
    match op {
        ParametricOp::MoveVertex {
            piece_id, edge_idx, ..
        }
        | ParametricOp::MoveHandle {
            piece_id, edge_idx, ..
        }
        | ParametricOp::SetSeamAllowance {
            piece_id, edge_idx, ..
        }
        | ParametricOp::AddNotch {
            piece_id, edge_idx, ..
        }
        | ParametricOp::TrueCornerG1 {
            piece_id, edge_idx, ..
        }
        | ParametricOp::ConvertEdgeCurve {
            piece_id, edge_idx, ..
        }
        | ParametricOp::SplitEdgePoint {
            piece_id, edge_idx, ..
        }
        | ParametricOp::SetEdgeLength {
            piece_id, edge_idx, ..
        } => Some((*piece_id, *edge_idx)),
        _ => None,
    }
}

fn is_edge_shape_mutator(op: &ParametricOp) -> bool {
    matches!(
        op,
        ParametricOp::SetSeamAllowance { .. }
            | ParametricOp::SetEdgeLength { .. }
            | ParametricOp::ConvertEdgeCurve { .. }
            | ParametricOp::TrueCornerG1 { .. }
            | ParametricOp::MoveVertex { .. }
            | ParametricOp::MoveHandle { .. }
    )
}

fn is_topology_barrier(op: &ParametricOp) -> bool {
    matches!(
        op,
        ParametricOp::SplitStyleLine { .. }
            | ParametricOp::SplitEdgePoint { .. }
            | ParametricOp::SlashSpread { .. }
            | ParametricOp::PivotDart { .. }
            | ParametricOp::MirrorPiece { .. }
    )
}

/// Returns whether `later` must remain strictly after `earlier` in the construction timeline.
fn ops_depend(earlier: &ParametricOp, later: &ParametricOp) -> bool {
    // Same seam binding operations must keep chronological order.
    if let (Some(b1), Some(b2)) = (earlier.target_binding_id(), later.target_binding_id()) {
        if b1 == b2 {
            return true;
        }
    }

    // Style-line splits replace/fork panel topology and create new panel IDs + seam bindings,
    // so act as a conservative barrier for subsequent ops on the same parent or newly created panels.
    if let ParametricOp::SplitStyleLine { piece_id, .. } = earlier {
        let later_pieces = later.target_piece_ids();
        if later_pieces.contains(piece_id) || later.target_binding_id().is_some() {
            return true;
        }
        if matches!(later, ParametricOp::SplitStyleLine { .. }) {
            return true;
        }
    }
    if let ParametricOp::SplitStyleLine { piece_id, .. } = later {
        let earlier_pieces = earlier.target_piece_ids();
        if earlier_pieces.contains(piece_id) || earlier.target_binding_id().is_some() {
            return true;
        }
    }

    // Shared piece checks
    let p_a = earlier.target_piece_ids();
    let p_b = later.target_piece_ids();
    let shares_piece = p_a.iter().any(|id| p_b.contains(id));

    if shares_piece && (is_topology_barrier(earlier) || is_topology_barrier(later)) {
        return true;
    }

    // Two shape/allowance edits on the exact same (piece, edge_idx) keep order.
    if let (Some((pa, ea)), Some((pb, eb))) = (op_edge_idx(earlier), op_edge_idx(later)) {
        if pa == pb && ea == eb && (is_edge_shape_mutator(earlier) || is_edge_shape_mutator(later)) {
            return true;
        }
    }

    false
}

/// Computes the inclusive `[min_idx, max_idx]` valid reorder bounds for `ops[idx]`
/// using transitive ancestor/descendant dependency closure (Fission `plageDeplacement` model).
pub fn compute_reorder_range(ops: &[ParametricOp], idx: usize) -> (usize, usize) {
    let n = ops.len();
    if n == 0 || idx >= n {
        return (0, 0);
    }

    // Ancestors: every j < idx that idx directly or transitively depends on.
    let mut ancestors = HashSet::new();
    ancestors.insert(idx);
    let mut min_idx = 0usize;
    for j in (0..idx).rev() {
        let blocks = ancestors.iter().any(|&a| ops_depend(&ops[j], &ops[a]));
        if blocks {
            ancestors.insert(j);
            min_idx = min_idx.max(j + 1);
        }
    }

    // Descendants: every k > idx that directly or transitively depends on idx.
    let mut descendants = HashSet::new();
    descendants.insert(idx);
    let mut max_idx = n - 1;
    for k in (idx + 1)..n {
        let blocks = descendants.iter().any(|&d| ops_depend(&ops[d], &ops[k]));
        if blocks {
            descendants.insert(k);
            if k > 0 {
                max_idx = max_idx.min(k - 1);
            }
        }
    }

    (min_idx.min(idx), max_idx.max(idx))
}

pub fn build_timeline_entries(
    project: &PatternProject,
    evaluated: &[EvaluatedPiece],
    stitching: &StitchingGraph,
) -> Vec<OpTimelineEntry> {
    let effective_rb = project.effective_rollback();
    let split_parents: HashSet<Uuid> = project
        .active_ops()
        .iter()
        .filter_map(|op| match op {
            ParametricOp::SplitStyleLine { piece_id, .. } => Some(*piece_id),
            _ => None,
        })
        .collect();

    project
        .ops
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let id = op.id();
            let suppressed = project.is_op_suppressed(id);
            let after_rollback = i >= effective_rb;
            let (reorder_min, reorder_max) = compute_reorder_range(&project.ops, i);
            let editable = !matches!(
                op,
                ParametricOp::DeleteSeamBinding { .. } | ParametricOp::MirrorPiece { .. }
            );
            let piece_ids: Vec<String> = op
                .target_piece_ids()
                .into_iter()
                .map(|u| u.to_string())
                .collect();
            let binding_id = op.target_binding_id().map(|u| u.to_string());
            let payload = serde_json::to_value(op).unwrap_or(serde_json::Value::Null);

            let (severity, diagnostic) = if suppressed || after_rollback {
                (OpSeverity::Ok, None)
            } else {
                diagnose_active_op(op, evaluated, stitching, &split_parents)
            };

            OpTimelineEntry {
                id: id.to_string(),
                index: i,
                kind: op.kind_str().to_string(),
                summary: op.summary(),
                piece_ids,
                binding_id,
                suppressed,
                after_rollback,
                reorder_min,
                reorder_max,
                severity,
                diagnostic,
                editable,
                payload,
            }
        })
        .collect()
}

fn diagnose_active_op(
    op: &ParametricOp,
    evaluated: &[EvaluatedPiece],
    stitching: &StitchingGraph,
    split_parents: &HashSet<Uuid>,
) -> (OpSeverity, Option<String>) {
    match op {
        ParametricOp::SplitStyleLine {
            piece_id,
            label_a,
            label_b,
            ..
        } => {
            let split_exists = stitching
                .bindings
                .iter()
                .any(|b| b.label.contains("Style Seam") && (b.label.contains(label_a) || b.label.contains(label_b)));
            let orig_exists = evaluated.iter().any(|e| e.piece.id == *piece_id);
            if orig_exists || split_exists {
                (OpSeverity::Ok, None)
            } else {
                (
                    OpSeverity::Error,
                    Some("Style-line split target piece no longer exists".into()),
                )
            }
        }
        ParametricOp::AddSeamBinding { binding, .. }
        | ParametricOp::ReplaceSeamBinding { binding, .. } => {
            if let Some(live) = stitching.bindings.iter().find(|b| b.id == binding.id) {
                diagnose_binding(live, evaluated)
            } else {
                (
                    OpSeverity::Warning,
                    Some("Seam binding was superseded or removed downstream".into()),
                )
            }
        }
        ParametricOp::UpdateSeamBinding { binding_id, .. } => {
            if let Some(live) = stitching.bindings.iter().find(|b| b.id == *binding_id) {
                diagnose_binding(live, evaluated)
            } else {
                (
                    OpSeverity::Error,
                    Some("Target seam binding not found".into()),
                )
            }
        }
        _ => {
            for pid in op.target_piece_ids() {
                if let Some(ep) = evaluated.iter().find(|e| e.piece.id == pid) {
                    if let Some((_, eidx)) = op_edge_idx(op) {
                        if eidx >= ep.piece.seam_loop.edges.len() {
                            return (
                                OpSeverity::Error,
                                Some(format!(
                                    "Edge #{} out of range on {}",
                                    eidx + 1,
                                    ep.piece.code
                                )),
                            );
                        }
                    }
                    if matches!(
                        op,
                        ParametricOp::SetSeamAllowance { .. }
                            | ParametricOp::SlashSpread { .. }
                            | ParametricOp::MoveVertex { .. }
                    ) {
                        if let Some(w) = ep.warnings.first() {
                            return (OpSeverity::Warning, Some(w.clone()));
                        }
                    }
                } else if !split_parents.contains(&pid) {
                    return (
                        OpSeverity::Error,
                        Some("Target pattern piece no longer exists".into()),
                    );
                }
            }
            (OpSeverity::Ok, None)
        }
    }
}

fn diagnose_binding(
    binding: &crate::model::SeamBinding,
    evaluated: &[EvaluatedPiece],
) -> (OpSeverity, Option<String>) {
    let raw: Vec<_> = evaluated.iter().map(|e| e.piece.clone()).collect();
    let la = crate::seam::interface_total_length(&binding.interface_a, &raw);
    let lb = crate::seam::interface_total_length(&binding.interface_b, &raw);
    if la < 1.0 || lb < 1.0 {
        return (
            OpSeverity::Error,
            Some("One side of seam binding resolved to < 1 mm or missing edges".into()),
        );
    }
    let ease = lb - la;
    if ease < binding.ease_min_mm || ease > binding.ease_max_mm {
        return (
            OpSeverity::Warning,
            Some(format!(
                "Seam ease out of tolerance ({ease:+.1} mm; range {:.1}..{:.1} mm)",
                binding.ease_min_mm, binding.ease_max_mm
            )),
        );
    }
    (OpSeverity::Ok, None)
}

pub fn patch_op_from_json(op: &mut ParametricOp, patch: &serde_json::Value) -> Result<(), String> {
    let Some(obj) = patch.as_object() else {
        return Err("Patch payload must be a JSON object".into());
    };
    let f_opt = |k: &str| obj.get(k).and_then(|v| v.as_f64()).filter(|v| v.is_finite());
    let u_opt = |k: &str| obj.get(k).and_then(|v| v.as_u64());
    let b_opt = |k: &str| obj.get(k).and_then(|v| v.as_bool());
    let s_opt = |k: &str| obj.get(k).and_then(|v| v.as_str());

    match op {
        ParametricOp::MoveVertex { dx, dy, .. } | ParametricOp::MovePiece { dx, dy, .. } => {
            if let Some(v) = f_opt("dx") {
                *dx = v;
            }
            if let Some(v) = f_opt("dy") {
                *dy = v;
            }
            Ok(())
        }
        ParametricOp::MoveHandle {
            dx, dy, lock_g1, ..
        } => {
            if let Some(v) = f_opt("dx") {
                *dx = v;
            }
            if let Some(v) = f_opt("dy") {
                *dy = v;
            }
            if let Some(v) = b_opt("lock_g1") {
                *lock_g1 = v;
            }
            Ok(())
        }
        ParametricOp::SetSeamAllowance {
            width_mm,
            corner_rule,
            ..
        } => {
            if let Some(v) = f_opt("width_mm") {
                *width_mm = v.max(0.0);
            }
            if let Some(cr_val) = obj.get("corner_rule") {
                if let Ok(cr) = serde_json::from_value::<CornerRule>(cr_val.clone()) {
                    *corner_rule = cr;
                }
            }
            Ok(())
        }
        ParametricOp::SetEdgeLength { target_len_mm, .. } => {
            if let Some(v) = f_opt("target_len_mm") {
                *target_len_mm = v.max(5.0);
            }
            Ok(())
        }
        ParametricOp::AddNotch {
            param_t,
            style,
            count,
            depth_mm,
            ..
        } => {
            if let Some(v) = f_opt("param_t") {
                *param_t = v.clamp(0.02, 0.98);
            }
            if let Some(v) = u_opt("count") {
                *count = (v as u8).clamp(1, 3);
            }
            if let Some(v) = f_opt("depth_mm") {
                *depth_mm = v.clamp(2.0, 12.0);
            }
            if let Some(st) = s_opt("style") {
                *style = match st {
                    "slit" => NotchStyle::Slit,
                    "v" => NotchStyle::V,
                    "u" => NotchStyle::U,
                    "t" => NotchStyle::T,
                    "castle" => NotchStyle::Castle,
                    other => return Err(format!("Unknown notch style `{other}`")),
                };
            }
            Ok(())
        }
        ParametricOp::AddDrillHole { x, y, label, .. } => {
            if let Some(v) = f_opt("x") {
                *x = v;
            }
            if let Some(v) = f_opt("y") {
                *y = v;
            }
            if let Some(s) = s_opt("label") {
                *label = s.to_string();
            }
            Ok(())
        }
        ParametricOp::SetGrainRotation { mode, .. } => {
            if let Some(m) = s_opt("mode") {
                *mode = match m {
                    "one_way" => GrainRotationMode::OneWay,
                    "two_way" => GrainRotationMode::TwoWay,
                    "bias" => GrainRotationMode::Bias,
                    "free" => GrainRotationMode::Free,
                    other => return Err(format!("Unknown grain mode `{other}`")),
                };
            }
            Ok(())
        }
        ParametricOp::TrueCornerG1 { square_90, .. } => {
            if let Some(v) = b_opt("square_90") {
                *square_90 = v;
            }
            Ok(())
        }
        ParametricOp::PivotDart {
            target_edge_idx,
            target_t,
            angle_deg,
            ..
        } => {
            if let Some(v) = u_opt("target_edge_idx") {
                *target_edge_idx = v as usize;
            }
            if let Some(v) = f_opt("target_t") {
                *target_t = v.clamp(0.05, 0.95);
            }
            if let Some(v) = f_opt("angle_deg") {
                *angle_deg = v.clamp(1.0, 60.0);
            }
            Ok(())
        }
        ParametricOp::SlashSpread {
            slash_count,
            spread_mm,
            hinge_mode,
            ..
        } => {
            if let Some(v) = u_opt("slash_count") {
                *slash_count = (v as u8).clamp(1, 6);
            }
            if let Some(v) = f_opt("spread_mm") {
                *spread_mm = v.clamp(-120.0, 180.0);
            }
            if let Some(hm) = s_opt("hinge_mode") {
                *hinge_mode = match hm {
                    "parallel_shift" => HingeMode::ParallelShift,
                    _ => HingeMode::EdgeHinge,
                };
            }
            Ok(())
        }
        ParametricOp::SplitEdgePoint { param_t, .. } => {
            if let Some(v) = f_opt("param_t") {
                *param_t = v.clamp(0.08, 0.92);
            }
            Ok(())
        }
        ParametricOp::ConvertEdgeCurve { to_cubic, .. } => {
            if let Some(v) = b_opt("to_cubic") {
                *to_cubic = v;
            }
            Ok(())
        }
        ParametricOp::SplitStyleLine {
            t_a,
            t_b,
            label_a,
            label_b,
            ..
        } => {
            if let Some(v) = f_opt("t_a") {
                *t_a = v.clamp(0.1, 0.9);
            }
            if let Some(v) = f_opt("t_b") {
                *t_b = v.clamp(0.1, 0.9);
            }
            if let Some(s) = s_opt("label_a") {
                *label_a = s.to_string();
            }
            if let Some(s) = s_opt("label_b") {
                *label_b = s.to_string();
            }
            Ok(())
        }
        _ => Err("This operation type cannot be patched directly via JSON".into()),
    }
}
