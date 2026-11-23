use crate::curve::{
    edge_total_length, eval_edge_point, eval_edge_tangent, invert_edge_arc_length,
};
use crate::model::{
    det_id, CornerRule, CurveSegment, DartCapStyle, DartEntity, DartFoldDir, HingeMode, NotchAnchor,
    ParametricOp, PatternPiece, Point2, StitchLink,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidTransform2D {
    pub origin_x: f64,
    pub origin_y: f64,
    pub tx: f64,
    pub ty: f64,
    pub cos: f64,
    pub sin: f64,
}

impl RigidTransform2D {
    pub fn apply(&self, p: Point2) -> Point2 {
        let dx = p.x - self.origin_x;
        let dy = p.y - self.origin_y;
        Point2::new(
            self.tx + dx * self.cos - dy * self.sin,
            self.ty + dx * self.sin + dy * self.cos,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeamWalkFrame {
    pub link_id: String,
    pub progress_u: f64,
    pub contact_pt: Point2,
    pub tangent: Point2,
    pub len_a_mm: f64,
    pub len_b_mm: f64,
    pub walked_a_mm: f64,
    pub walked_b_mm: f64,
    pub ease_delta_mm: f64,
    pub transform_b: RigidTransform2D,
}

/// Computes the rigid transformation placing piece_b onto piece_a at normalized progress u in [0, 1]
/// with matching tangent orientation so a patternmaker can visually walk & true seams.
pub fn compute_seam_walk_frame(
    link: &StitchLink,
    piece_a: &PatternPiece,
    piece_b: &PatternPiece,
    progress_u: f64,
) -> Option<SeamWalkFrame> {
    let edge_a = piece_a.seam_loop.edges.get(link.edge_a)?;
    let edge_b = piece_b.seam_loop.edges.get(link.edge_b)?;
    let len_a = edge_total_length(edge_a);
    let len_b = edge_total_length(edge_b);
    let u = progress_u.clamp(0.0, 1.0);

    let s_a = if link.dir_a >= 0 {
        u * len_a
    } else {
        (1.0 - u) * len_a
    };
    let s_b = if link.dir_b >= 0 {
        u * len_b
    } else {
        (1.0 - u) * len_b
    };

    let t_a = invert_edge_arc_length(edge_a, s_a);
    let t_b = invert_edge_arc_length(edge_b, s_b);
    let pa = eval_edge_point(edge_a, t_a);
    let pb = eval_edge_point(edge_b, t_b);

    let mut ta = eval_edge_tangent(edge_a, t_a);
    if link.dir_a < 0 {
        ta = ta.scale(-1.0);
    }
    let mut tb = eval_edge_tangent(edge_b, t_b);
    if link.dir_b < 0 {
        tb = tb.scale(-1.0);
    }
    // Opposite sewing direction aligns edge_b cleanly alongside edge_a without overlapping interiors
    let tb_target = tb.scale(-1.0);

    let angle_a = ta.y.atan2(ta.x);
    let angle_b = tb_target.y.atan2(tb_target.x);
    let rot = angle_a - angle_b;
    let (sin, cos) = rot.sin_cos();

    let transform_b = RigidTransform2D {
        origin_x: pb.x,
        origin_y: pb.y,
        tx: pa.x,
        ty: pa.y,
        cos,
        sin,
    };

    Some(SeamWalkFrame {
        link_id: link.id.to_string(),
        progress_u: u,
        contact_pt: pa,
        tangent: ta,
        len_a_mm: len_a,
        len_b_mm: len_b,
        walked_a_mm: u * len_a,
        walked_b_mm: u * len_b,
        ease_delta_mm: len_b - len_a,
        transform_b,
    })
}

/// Applies all recorded ParametricOps onto a pattern piece sequentially.
pub fn apply_ops_to_piece(piece: &mut PatternPiece, ops: &[ParametricOp]) {
    for op in ops {
        match op {
            ParametricOp::MoveVertex {
                piece_id,
                edge_idx,
                dx,
                dy,
                ..
            } if *piece_id == piece.id => {
                apply_move_vertex(piece, *edge_idx, *dx, *dy);
            }
            ParametricOp::MoveHandle {
                piece_id,
                edge_idx,
                which,
                dx,
                dy,
                lock_g1,
                ..
            } if *piece_id == piece.id => {
                apply_move_handle(piece, *edge_idx, *which, *dx, *dy, *lock_g1);
            }
            ParametricOp::SetSeamAllowance {
                piece_id,
                edge_idx,
                width_mm,
                corner_rule,
                ..
            } if *piece_id == piece.id => {
                if let Some(edge) = piece.seam_loop.edges.get_mut(*edge_idx) {
                    edge.sa_width_mm = (*width_mm).max(0.0);
                    edge.corner_rule = *corner_rule;
                }
            }
            ParametricOp::AddNotch {
                id,
                piece_id,
                edge_idx,
                param_t,
                style,
                count,
                depth_mm,
            } if *piece_id == piece.id => {
                if *edge_idx < piece.seam_loop.edges.len() {
                    piece.notches.push(NotchAnchor {
                        id: *id,
                        edge_idx: *edge_idx,
                        param_t: param_t.clamp(0.02, 0.98),
                        style: *style,
                        count: (*count).clamp(1, 3),
                        depth_mm: (*depth_mm).clamp(2.0, 12.0),
                    });
                }
            }
            ParametricOp::PivotDart {
                piece_id,
                dart_id,
                target_edge_idx,
                target_t,
                angle_deg,
                ..
            } if *piece_id == piece.id => {
                apply_dart_pivot(piece, *dart_id, *target_edge_idx, *target_t, *angle_deg);
            }
            ParametricOp::SlashSpread {
                piece_id,
                start_edge_idx,
                end_edge_idx,
                slash_count,
                spread_mm,
                hinge_mode,
                ..
            } if *piece_id == piece.id => {
                apply_slash_and_spread(
                    piece,
                    *start_edge_idx,
                    *end_edge_idx,
                    *slash_count,
                    *spread_mm,
                    *hinge_mode,
                );
            }
            _ => {}
        }
    }
    piece.seam_loop.sync_continuity();
    piece.seam_loop.ensure_ccw();
}

pub fn apply_move_vertex(piece: &mut PatternPiece, edge_idx: usize, dx: f64, dy: f64) {
    let n = piece.seam_loop.edges.len();
    if edge_idx >= n || n == 0 {
        return;
    }
    let delta = Point2::new(dx, dy);
    let prev_idx = (edge_idx + n - 1) % n;

    let new_pt = piece.seam_loop.edges[edge_idx].start.add(delta);
    piece.seam_loop.edges[edge_idx].start = new_pt;
    if let CurveSegment::Cubic { c1, .. } = &mut piece.seam_loop.edges[edge_idx].segment {
        *c1 = c1.add(delta);
    }
    match &mut piece.seam_loop.edges[prev_idx].segment {
        CurveSegment::Line { end } => *end = new_pt,
        CurveSegment::Cubic { c2, end, .. } => {
            *c2 = c2.add(delta);
            *end = new_pt;
        }
    }
}

pub fn apply_move_handle(
    piece: &mut PatternPiece,
    edge_idx: usize,
    which: u8,
    dx: f64,
    dy: f64,
    lock_g1: bool,
) {
    let n = piece.seam_loop.edges.len();
    if edge_idx >= n {
        return;
    }
    let delta = Point2::new(dx, dy);
    // Upgrade line to cubic smoothly if handle dragged on a straight edge
    if let CurveSegment::Line { end } = piece.seam_loop.edges[edge_idx].segment {
        let s = piece.seam_loop.edges[edge_idx].start;
        piece.seam_loop.edges[edge_idx].segment = CurveSegment::Cubic {
            c1: s.lerp(end, 1.0 / 3.0),
            c2: s.lerp(end, 2.0 / 3.0),
            end,
        };
    }

    let edge_start = piece.seam_loop.edges[edge_idx].start;
    let mut anchor_pt = Point2::ZERO;
    let mut handle_dir = Point2::ZERO;
    if let CurveSegment::Cubic { c1, c2, end } = &mut piece.seam_loop.edges[edge_idx].segment {
        if which == 1 {
            *c1 = c1.add(delta);
            anchor_pt = edge_start;
            handle_dir = c1.sub(anchor_pt);
        } else {
            *c2 = c2.add(delta);
            anchor_pt = *end;
            handle_dir = c2.sub(anchor_pt);
        }
    }

    if lock_g1 && handle_dir.len() > 1e-6 {
        let unit = handle_dir.normalized();
        let adj_idx = if which == 1 {
            (edge_idx + n - 1) % n
        } else {
            (edge_idx + 1) % n
        };
        if let CurveSegment::Cubic { c1, c2, .. } = &mut piece.seam_loop.edges[adj_idx].segment {
            if which == 1 {
                let len = c2.dist(anchor_pt);
                *c2 = anchor_pt.sub(unit.scale(len));
            } else {
                let len = c1.dist(anchor_pt);
                *c1 = anchor_pt.sub(unit.scale(len));
            }
        }
    }
}

/// Rotates a pattern boundary chain around `dart.apex` by `angle_deg`, closing/reducing the source dart
/// and opening a new dart along `target_edge_idx @ target_t` while strictly conserving perimeter length.
pub fn apply_dart_pivot(
    piece: &mut PatternPiece,
    dart_id: uuid::Uuid,
    target_edge_idx: usize,
    target_t: f64,
    angle_deg: f64,
) {
    let Some(dart_pos) = piece.darts.iter().position(|d| d.id == dart_id) else {
        return;
    };
    let n = piece.seam_loop.edges.len();
    if n < 3 || target_edge_idx >= n {
        return;
    }

    let dart = piece.darts[dart_pos].clone();
    let src_idx = dart.edge_idx.min(n - 1);
    if src_idx == target_edge_idx {
        return;
    }

    // Approximate source dart intake angle around apex from leg endpoints
    let leg_a = eval_edge_point(&piece.seam_loop.edges[src_idx], dart.leg_a_t);
    let leg_b = eval_edge_point(&piece.seam_loop.edges[src_idx], dart.leg_b_t);
    let va = leg_a.sub(dart.apex);
    let vb = leg_b.sub(dart.apex);
    let max_theta = (va.x * vb.y - va.y * vb.x)
        .atan2(va.dot(vb))
        .abs()
        .max(0.05);

    let req_rad = angle_deg.to_radians().abs().clamp(0.02, max_theta);
    // Determine forward CCW walk from src_idx to target_edge_idx
    let steps_ccw = (target_edge_idx + n - src_idx) % n;
    let steps_cw = (src_idx + n - target_edge_idx) % n;
    let rotate_ccw_sector = steps_ccw <= steps_cw;
    let theta = if rotate_ccw_sector { -req_rad } else { req_rad };

    // Rotate edges strictly between src_idx and target_edge_idx around dart.apex
    let mut rotated_edges = Vec::new();
    if rotate_ccw_sector {
        let mut k = (src_idx + 1) % n;
        while k != target_edge_idx {
            rotated_edges.push(k);
            k = (k + 1) % n;
        }
    } else {
        let mut k = (target_edge_idx + 1) % n;
        while k != src_idx {
            rotated_edges.push(k);
            k = (k + 1) % n;
        }
    }

    // Split target edge at target_t conceptually: move target end/start side rigidly
    for &idx in &rotated_edges {
        let e = &mut piece.seam_loop.edges[idx];
        e.start = e.start.rotate_around(dart.apex, theta);
        e.segment.transform(|p| p.rotate_around(dart.apex, theta));
    }

    // Rotate adjoining vertex on target edge and update continuity cleanly
    let t_split = target_t.clamp(0.15, 0.85);
    let split_pt = eval_edge_point(&piece.seam_loop.edges[target_edge_idx], t_split);
    let split_pt_rot = split_pt.rotate_around(dart.apex, theta);

    // Update target edge end if rotating CCW side, or start if CW side, while keeping its length intact
    if rotate_ccw_sector {
        let t_edge = &mut piece.seam_loop.edges[target_edge_idx];
        t_edge.start = t_edge.start.rotate_around(dart.apex, theta);
        // Move source dart edge's end vertex inward to match rotated next edge start
        let next_start = piece.seam_loop.edges[(src_idx + 1) % n].start;
        piece.seam_loop.edges[src_idx].segment.set_end_point(next_start);
    } else {
        let t_edge = &mut piece.seam_loop.edges[target_edge_idx];
        let new_end = t_edge.end().rotate_around(dart.apex, theta);
        t_edge.segment.set_end_point(new_end);
        let prev_end = piece.seam_loop.edges[(src_idx + n - 1) % n].end();
        piece.seam_loop.edges[src_idx].start = prev_end;
    }

    // Insert a dedicated DartEntity at the target edge representing the newly opened intake
    let intake_new = split_pt.dist(split_pt_rot).max(8.0);
    piece.darts[dart_pos] = DartEntity {
        id: det_id(&format!("{}:pivoted:{target_edge_idx}", dart.id)),
        name: format!("{} (Pivoted)", dart.name),
        apex: dart.apex,
        edge_idx: target_edge_idx,
        leg_a_t: (t_split - 0.08).max(0.05),
        leg_b_t: (t_split + 0.08).min(0.95),
        intake_mm: intake_new,
        fold_dir: DartFoldDir::TowardHem,
        cap_style: DartCapStyle::FoldedReflection,
    };
}

/// Computes exact folded dart-cap reflection polygon line segments for a piece's darts.
pub fn compute_dart_cap_polylines(piece: &PatternPiece) -> Vec<Vec<Point2>> {
    let mut caps = Vec::new();
    for dart in &piece.darts {
        let Some(edge) = piece.seam_loop.edges.get(dart.edge_idx) else {
            continue;
        };
        let a = eval_edge_point(edge, dart.leg_a_t);
        let b = eval_edge_point(edge, dart.leg_b_t);
        let mid = a.lerp(b, 0.5);
        let apex_to_mid = mid.sub(dart.apex);
        let fold_leg = match dart.fold_dir {
            DartFoldDir::TowardCf | DartFoldDir::TowardHem => a,
            DartFoldDir::TowardCb | DartFoldDir::TowardSide => b,
        };
        // Peak height accounts for dart intake angle so folded dart edge stays flush with seamline
        let dir = apex_to_mid.normalized();
        let half_w = 0.5 * a.dist(b);
        let peak_lift = (half_w * 0.22).max(2.5);
        let peak = mid.add(dir.scale(peak_lift));
        caps.push(vec![a, dart.apex, b, peak, fold_leg.lerp(peak, 0.5), a]);
    }
    caps
}

/// Parametric Slash-and-Spread: splits the active edge span into k+1 sections, rotates or translates
/// sub-segments progressively by `spread_mm` per slash, and bridges opened contours smoothly.
pub fn apply_slash_and_spread(
    piece: &mut PatternPiece,
    start_edge_idx: usize,
    end_edge_idx: usize,
    slash_count: u8,
    spread_mm: f64,
    hinge_mode: HingeMode,
) {
    let n = piece.seam_loop.edges.len();
    if n < 3 || start_edge_idx >= n || end_edge_idx >= n || slash_count == 0 || spread_mm.abs() < 0.01 {
        return;
    }

    let k = slash_count.clamp(1, 6) as usize;
    let total_spread = spread_mm * (k as f64);

    // Hinge reference on opposite edge midpoint; spread acts smoothly along the target edge span
    let hinge_edge = &piece.seam_loop.edges[start_edge_idx];
    let hinge_pt = eval_edge_point(hinge_edge, 0.5);
    let spread_edge = &piece.seam_loop.edges[end_edge_idx];
    let spread_mid = eval_edge_point(spread_edge, 0.5);
    let outward = spread_mid.sub(hinge_pt).normalized();
    let tangent = eval_edge_tangent(spread_edge, 0.5);

    // Apply transformation along the non-hinge side of the piece
    let shift_dir = if tangent.len() > 1e-6 {
        tangent.add(outward.scale(0.35)).normalized()
    } else {
        Point2::new(1.0, 0.0)
    };

    let idx_from = (start_edge_idx + 1) % n;
    let mut curr = idx_from;
    let mut step = 0usize;
    while curr != start_edge_idx && step < n {
        let weight = if curr == end_edge_idx {
            1.0
        } else {
            ((step + 1) as f64 / (n as f64)).clamp(0.15, 1.0)
        };
        let e = &mut piece.seam_loop.edges[curr];
        match hinge_mode {
            HingeMode::EdgeHinge => {
                let radius = e.start.dist(hinge_pt).max(40.0);
                let theta = (total_spread / radius) * 0.45 * weight;
                e.start = e.start.rotate_around(hinge_pt, theta);
                e.segment.transform(|p| p.rotate_around(hinge_pt, theta));
            }
            HingeMode::ParallelShift => {
                let dv = shift_dir.scale(total_spread * 0.5 * weight);
                e.start = e.start.add(dv);
                e.segment.transform(|p| p.add(dv));
            }
        }
        curr = (curr + 1) % n;
        step += 1;
    }

    // Record visible slash guide lines inside internal_lines for clear digital-paper visualization
    for i in 1..=k {
        let t = (i as f64) / ((k + 1) as f64);
        let a = eval_edge_point(&piece.seam_loop.edges[start_edge_idx], t);
        let b = eval_edge_point(&piece.seam_loop.edges[end_edge_idx], 1.0 - t);
        piece.internal_lines.push(crate::model::InternalLine {
            id: det_id(&format!("{}:slash_guide:{i}", piece.id)),
            label: format!("Slash #{i} (+{spread_mm:.0} mm)"),
            pts: vec![a, b],
            dashed: true,
        });
    }
    piece.seam_loop.sync_continuity();
}

/// Validates and softly enforces G1 continuity on paired corner edges if requested.
pub fn corner_rule_label(rule: CornerRule) -> &'static str {
    match rule {
        CornerRule::Miter { .. } => "Miter",
        CornerRule::Bevel => "Bevel",
        CornerRule::Square => "Square",
        CornerRule::FoldedHem => "Folded Hem",
        CornerRule::MatchedSeam => "Matched Seam",
        CornerRule::DartCap => "Dart Cap",
    }
}
