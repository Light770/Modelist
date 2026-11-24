use crate::curve::{eval_edge_point, eval_edge_tangent, split_edge_at_t};
use crate::model::{
    det_id, ClosedPolyCurve, CornerRule, CurveSegment, DartCapStyle, DartEntity, DartFoldDir,
    DrillHole, EaseSpec, EdgeRef, HingeMode, NotchAnchor, ParametricOp, PatternPiece, Point2,
    PolyEdge, SeamBinding, SeamDriver, SeamInterface, SeamType, StitchingGraph,
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

/// Applies piece-local ParametricOps sequentially without destroying parametric origins.
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
                if let Some(edge) = piece.seam_loop.edges.get(*edge_idx) {
                    piece.notches.push(NotchAnchor {
                        id: *id,
                        edge_idx: *edge_idx,
                        edge_id: Some(edge.id),
                        param_t: param_t.clamp(0.02, 0.98),
                        style: *style,
                        count: (*count).clamp(1, 3),
                        depth_mm: (*depth_mm).clamp(2.0, 12.0),
                    });
                }
            }
            ParametricOp::AddDrillHole {
                id,
                piece_id,
                x,
                y,
                label,
            } if *piece_id == piece.id => {
                piece.drill_holes.push(DrillHole {
                    id: *id,
                    pos: Point2::new(*x, *y),
                    radius_mm: 2.0,
                    label: label.clone(),
                });
            }
            ParametricOp::SetGrainRotation { piece_id, mode, .. } if *piece_id == piece.id => {
                piece.grainline.rotation_mode = *mode;
            }
            ParametricOp::TrueCornerG1 {
                piece_id,
                edge_idx,
                square_90,
                ..
            } if *piece_id == piece.id => {
                apply_true_corner_g1(piece, *edge_idx, *square_90);
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
            ParametricOp::MovePiece {
                piece_id, dx, dy, ..
            } if *piece_id == piece.id => {
                apply_move_piece(piece, *dx, *dy);
            }
            ParametricOp::ConvertEdgeCurve {
                piece_id,
                edge_idx,
                to_cubic,
                ..
            } if *piece_id == piece.id => {
                apply_convert_edge_curve(piece, *edge_idx, *to_cubic);
            }
            ParametricOp::SplitEdgePoint {
                piece_id,
                edge_idx,
                param_t,
                ..
            } if *piece_id == piece.id => {
                apply_split_edge_point(piece, *edge_idx, *param_t);
            }
            ParametricOp::SetEdgeLength {
                piece_id,
                edge_idx,
                target_len_mm,
                ..
            } if *piece_id == piece.id => {
                apply_set_edge_length(piece, *edge_idx, *target_len_mm);
            }
            ParametricOp::MirrorPiece { piece_id, .. } if *piece_id == piece.id => {
                apply_mirror_piece(piece);
            }
            _ => {}
        }
    }
    piece.seam_loop.sync_continuity();
    piece.seam_loop.ensure_ccw();
}

pub fn apply_move_piece(piece: &mut PatternPiece, dx: f64, dy: f64) {
    let delta = Point2::new(dx, dy);
    for e in &mut piece.seam_loop.edges {
        e.start = e.start.add(delta);
        e.segment.transform(|p| p.add(delta));
    }
    for hole in &mut piece.cutouts {
        for e in &mut hole.edges {
            e.start = e.start.add(delta);
            e.segment.transform(|p| p.add(delta));
        }
    }
    piece.grainline.start = piece.grainline.start.add(delta);
    piece.grainline.end = piece.grainline.end.add(delta);
    for d in &mut piece.darts {
        d.apex = d.apex.add(delta);
    }
    for dh in &mut piece.drill_holes {
        dh.pos = dh.pos.add(delta);
    }
    for il in &mut piece.internal_lines {
        for pt in &mut il.pts {
            *pt = pt.add(delta);
        }
    }
}

pub fn apply_convert_edge_curve(piece: &mut PatternPiece, edge_idx: usize, to_cubic: bool) {
    let Some(edge) = piece.seam_loop.edges.get_mut(edge_idx) else {
        return;
    };
    let s = edge.start;
    let e = edge.end();
    if to_cubic {
        if matches!(edge.segment, CurveSegment::Line { .. }) {
            let dir = e.sub(s);
            let perp = Point2::new(-dir.y, dir.x).normalized();
            let bow = (dir.len() * 0.06).min(14.0);
            edge.segment = CurveSegment::Cubic {
                c1: s.lerp(e, 1.0 / 3.0).add(perp.scale(bow)),
                c2: s.lerp(e, 2.0 / 3.0).add(perp.scale(bow)),
                end: e,
            };
        }
    } else if matches!(edge.segment, CurveSegment::Cubic { .. }) {
        edge.segment = CurveSegment::Line { end: e };
    }
}

pub fn apply_split_edge_point(piece: &mut PatternPiece, edge_idx: usize, param_t: f64) {
    if edge_idx >= piece.seam_loop.edges.len() {
        return;
    }
    let t = param_t.clamp(0.08, 0.92);
    let orig = piece.seam_loop.edges[edge_idx].clone();
    let (first, second) = split_edge_at_t(&orig, t);
    piece.seam_loop.edges[edge_idx] = first;
    piece.seam_loop.edges.insert(edge_idx + 1, second);
    if let Some(mf) = piece.mirror_fold_edge {
        if mf > edge_idx {
            piece.mirror_fold_edge = Some(mf + 1);
        }
    }
}

pub fn apply_set_edge_length(piece: &mut PatternPiece, edge_idx: usize, target_len_mm: f64) {
    let n = piece.seam_loop.edges.len();
    if edge_idx >= n || target_len_mm < 5.0 {
        return;
    }
    let cur_len = crate::curve::edge_total_length(&piece.seam_loop.edges[edge_idx]);
    if cur_len < 1e-4 {
        return;
    }
    let ratio = (target_len_mm / cur_len).clamp(0.25, 3.5);
    let anchor = piece.seam_loop.edges[edge_idx].start;
    let old_end = piece.seam_loop.edges[edge_idx].end();
    let new_end = anchor.add(old_end.sub(anchor).scale(ratio));
    let delta = new_end.sub(old_end);

    match &mut piece.seam_loop.edges[edge_idx].segment {
        CurveSegment::Line { end } => *end = new_end,
        CurveSegment::Cubic { c1, c2, end } => {
            *c1 = anchor.add(c1.sub(anchor).scale(ratio));
            *c2 = anchor.add(c2.sub(anchor).scale(ratio));
            *end = new_end;
        }
    }
    let next_idx = (edge_idx + 1) % n;
    piece.seam_loop.edges[next_idx].start = new_end;
    if let CurveSegment::Cubic { c1, .. } = &mut piece.seam_loop.edges[next_idx].segment {
        *c1 = c1.add(delta);
    }
}

pub fn apply_mirror_piece(piece: &mut PatternPiece) {
    let pts = piece.seam_loop.flatten(1.0);
    if pts.is_empty() {
        return;
    }
    let min_x = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let cx = 0.5 * (min_x + max_x);
    let flip = |p: Point2| Point2::new(2.0 * cx - p.x, p.y);

    for e in &mut piece.seam_loop.edges {
        e.start = flip(e.start);
        e.segment.transform(flip);
    }
    piece.seam_loop.reverse();
    piece.seam_loop.sync_continuity();
    piece.seam_loop.ensure_ccw();

    piece.grainline.start = flip(piece.grainline.start);
    piece.grainline.end = flip(piece.grainline.end);
    for d in &mut piece.darts {
        d.apex = flip(d.apex);
    }
    for dh in &mut piece.drill_holes {
        dh.pos = flip(dh.pos);
    }
    for il in &mut piece.internal_lines {
        for pt in &mut il.pts {
            *pt = flip(*pt);
        }
    }
}

/// Creates a new custom drafted rectangular pattern piece (e.g., waistband, pocket, collar, facing).
pub fn make_rect_pattern_piece(
    name: &str,
    code: &str,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> PatternPiece {
    let lx = x0.min(x1);
    let rx = x0.max(x1).max(lx + 20.0);
    let by = y0.min(y1);
    let ty = y0.max(y1).max(by + 20.0);
    let pid = det_id(&format!("custom:{code}:{lx:.1}:{by:.1}:{rx:.1}:{ty:.1}"));

    let p0 = Point2::new(lx, by);
    let p1 = Point2::new(rx, by);
    let p2 = Point2::new(rx, ty);
    let p3 = Point2::new(lx, ty);

    let edges = vec![
        PolyEdge {
            id: det_id(&format!("{pid}:e0")),
            start: p0,
            segment: CurveSegment::Line { end: p1 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Bottom Edge".into(),
        },
        PolyEdge {
            id: det_id(&format!("{pid}:e1")),
            start: p1,
            segment: CurveSegment::Line { end: p2 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Right Edge".into(),
        },
        PolyEdge {
            id: det_id(&format!("{pid}:e2")),
            start: p2,
            segment: CurveSegment::Line { end: p3 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Top Edge".into(),
        },
        PolyEdge {
            id: det_id(&format!("{pid}:e3")),
            start: p3,
            segment: CurveSegment::Line { end: p0 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Left Edge".into(),
        },
    ];
    let mut seam_loop = ClosedPolyCurve::from_edges(edges);
    seam_loop.ensure_ccw();

    let mx = 0.5 * (lx + rx);
    let pad_y = ((ty - by) * 0.18).min(35.0);
    PatternPiece {
        id: pid,
        name: name.into(),
        code: code.into(),
        cut_quantity: 1,
        mirror_fold_edge: None,
        grainline: crate::model::GrainlineSpec {
            start: Point2::new(mx, by + pad_y),
            end: Point2::new(mx, ty - pad_y),
            rotation_mode: crate::model::GrainRotationMode::TwoWay,
        },
        seam_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![],
        drill_holes: vec![],
        notches: vec![],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: crate::model::ArrangementZone::FlatCustom,
    }
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

/// Stage 4 Corner Truing: squares off an incoming/outgoing curved corner into an exact 90° intersection
/// (e.g., CF/CB neckline or armscye-to-side-seam join) or aligns handles for G1 smoothness.
pub fn apply_true_corner_g1(piece: &mut PatternPiece, edge_idx: usize, square_90: bool) {
    let n = piece.seam_loop.edges.len();
    if edge_idx >= n || n < 3 {
        return;
    }
    let prev_idx = (edge_idx + n - 1) % n;
    let v = piece.seam_loop.edges[edge_idx].start;
    let prev_tang = eval_edge_tangent(&piece.seam_loop.edges[prev_idx], 1.0);
    let next_tang = eval_edge_tangent(&piece.seam_loop.edges[edge_idx], 0.0);

    if square_90 {
        // Make cubic edge perpendicular to adjacent line/edge tangent at vertex v
        let curr_is_cubic = matches!(piece.seam_loop.edges[edge_idx].segment, CurveSegment::Cubic { .. });
        let prev_is_cubic = matches!(piece.seam_loop.edges[prev_idx].segment, CurveSegment::Cubic { .. });
        if curr_is_cubic {
            let perp = Point2::new(-prev_tang.y, prev_tang.x).normalized();
            let dir = if perp.dot(next_tang) >= 0.0 {
                perp
            } else {
                perp.scale(-1.0)
            };
            if let CurveSegment::Cubic { c1, .. } = &mut piece.seam_loop.edges[edge_idx].segment {
                let len = c1.dist(v).max(15.0);
                *c1 = v.add(dir.scale(len));
            }
        } else if prev_is_cubic {
            let perp = Point2::new(-next_tang.y, next_tang.x).normalized();
            let dir = if perp.dot(prev_tang.scale(-1.0)) >= 0.0 {
                perp
            } else {
                perp.scale(-1.0)
            };
            if let CurveSegment::Cubic { c2, .. } = &mut piece.seam_loop.edges[prev_idx].segment {
                let len = c2.dist(v).max(15.0);
                *c2 = v.add(dir.scale(len));
            }
        }
    } else {
        // G1 smooth average tangent across vertex v
        let avg = prev_tang.add(next_tang).normalized();
        if let CurveSegment::Cubic { c2, .. } = &mut piece.seam_loop.edges[prev_idx].segment {
            let len = c2.dist(v).max(12.0);
            *c2 = v.sub(avg.scale(len));
        }
        if let CurveSegment::Cubic { c1, .. } = &mut piece.seam_loop.edges[edge_idx].segment {
            let len = c1.dist(v).max(12.0);
            *c1 = v.add(avg.scale(len));
        }
    }
}

/// Stage 3 Internal Style-Line Cut & Trace: splits a piece cleanly along a cut between
/// (edge_a_idx @ t_a) and (edge_b_idx @ t_b) into two valid CCW pieces automatically linked by a SeamBinding!
#[allow(clippy::too_many_arguments)]
pub fn apply_style_line_split(
    pieces: &mut Vec<PatternPiece>,
    stitching: &mut StitchingGraph,
    piece_id: uuid::Uuid,
    edge_a_idx: usize,
    t_a: f64,
    edge_b_idx: usize,
    t_b: f64,
    label_a: &str,
    label_b: &str,
) {
    let Some(pos) = pieces.iter().position(|p| p.id == piece_id) else {
        return;
    };
    let n = pieces[pos].seam_loop.edges.len();
    if n < 3 || edge_a_idx >= n || edge_b_idx >= n || edge_a_idx == edge_b_idx {
        return;
    }
    let (ea, ta, eb, tb) = if edge_a_idx < edge_b_idx {
        (edge_a_idx, t_a.clamp(0.1, 0.9), edge_b_idx, t_b.clamp(0.1, 0.9))
    } else {
        (edge_b_idx, t_b.clamp(0.1, 0.9), edge_a_idx, t_a.clamp(0.1, 0.9))
    };

    let orig = pieces[pos].clone();
    let orig_ea = &orig.seam_loop.edges[ea];
    let orig_eb = &orig.seam_loop.edges[eb];
    let (mut ea_first, mut ea_second) = split_edge_at_t(orig_ea, ta);
    let (mut eb_first, mut eb_second) = split_edge_at_t(orig_eb, tb);
    ea_first.id = det_id(&format!("{}:split_a1:{ea}:{eb}", orig.id));
    ea_second.id = det_id(&format!("{}:split_a2:{ea}:{eb}", orig.id));
    eb_first.id = det_id(&format!("{}:split_b1:{ea}:{eb}", orig.id));
    eb_second.id = det_id(&format!("{}:split_b2:{ea}:{eb}", orig.id));
    let ea_first_id = ea_first.id;
    let ea_second_id = ea_second.id;
    let eb_first_id = eb_first.id;
    let eb_second_id = eb_second.id;
    let pt_a = ea_second.start;
    let pt_b = eb_second.start;

    let seam_id_a = det_id(&format!("{}:split_seam_a:{ea}:{eb}", orig.id));
    let seam_id_b = det_id(&format!("{}:split_seam_b:{ea}:{eb}", orig.id));

    // Piece A: ea_second -> edges (ea+1..eb) -> eb_first -> cut edge (pt_b -> pt_a)
    let mut edges_a = vec![ea_second];
    for idx in (ea + 1)..eb {
        edges_a.push(orig.seam_loop.edges[idx].clone());
    }
    edges_a.push(eb_first);
    edges_a.push(PolyEdge {
        id: seam_id_a,
        start: pt_b,
        segment: CurveSegment::Line { end: pt_a },
        sa_width_mm: 10.0,
        corner_rule: CornerRule::Miter { limit: 2.5 },
        label: "Style Seam".into(),
    });
    let mut loop_a = ClosedPolyCurve::from_edges(edges_a);
    loop_a.sync_continuity();
    loop_a.ensure_ccw();

    // Piece B: eb_second -> edges (eb+1..n, 0..ea) -> ea_first -> cut edge (pt_a -> pt_b)
    let mut edges_b = vec![eb_second];
    for idx in (eb + 1)..n {
        edges_b.push(orig.seam_loop.edges[idx].clone());
    }
    for idx in 0..ea {
        edges_b.push(orig.seam_loop.edges[idx].clone());
    }
    edges_b.push(ea_first);
    edges_b.push(PolyEdge {
        id: seam_id_b,
        start: pt_a,
        segment: CurveSegment::Line { end: pt_b },
        sa_width_mm: 10.0,
        corner_rule: CornerRule::Miter { limit: 2.5 },
        label: "Style Seam".into(),
    });
    let mut loop_b = ClosedPolyCurve::from_edges(edges_b);
    loop_b.sync_continuity();
    loop_b.ensure_ccw();

    // Slightly separate Piece B perpendicular to the cut line so both panels are immediately clear
    let cut_dir = pt_b.sub(pt_a).normalized();
    let sep = Point2::new(-cut_dir.y, cut_dir.x).scale(26.0);
    for e in &mut loop_b.edges {
        e.start = e.start.add(sep);
        e.segment.transform(|p| p.add(sep));
    }

    let id_b = det_id(&format!("{}:split_panel_b:{ea}:{eb}", orig.id));
    let moved_edge_ids: Vec<uuid::Uuid> = ((eb + 1)..n)
        .chain(0..ea)
        .map(|idx| orig.seam_loop.edges[idx].id)
        .collect();
    let orig_ea_id = orig.seam_loop.edges[ea].id;
    let orig_eb_id = orig.seam_loop.edges[eb].id;

    // Remap existing SeamBindings so split boundary chains & moved edges remain cleanly stitched
    let remap_refs = |refs: &mut Vec<EdgeRef>| {
        let old = std::mem::take(refs);
        for mut er in old {
            if er.piece_id != orig.id {
                refs.push(er);
                continue;
            }
            if er.edge_id == orig_ea_id {
                if er.dir >= 0 {
                    refs.push(EdgeRef { piece_id: id_b, edge_id: ea_first_id, dir: 1 });
                    refs.push(EdgeRef { piece_id: orig.id, edge_id: ea_second_id, dir: 1 });
                } else {
                    refs.push(EdgeRef { piece_id: orig.id, edge_id: ea_second_id, dir: -1 });
                    refs.push(EdgeRef { piece_id: id_b, edge_id: ea_first_id, dir: -1 });
                }
            } else if er.edge_id == orig_eb_id {
                if er.dir >= 0 {
                    refs.push(EdgeRef { piece_id: orig.id, edge_id: eb_first_id, dir: 1 });
                    refs.push(EdgeRef { piece_id: id_b, edge_id: eb_second_id, dir: 1 });
                } else {
                    refs.push(EdgeRef { piece_id: id_b, edge_id: eb_second_id, dir: -1 });
                    refs.push(EdgeRef { piece_id: orig.id, edge_id: eb_first_id, dir: -1 });
                }
            } else {
                if moved_edge_ids.contains(&er.edge_id) {
                    er.piece_id = id_b;
                }
                refs.push(er);
            }
        }
    };
    for binding in &mut stitching.bindings {
        remap_refs(&mut binding.interface_a.edges);
        remap_refs(&mut binding.interface_b.edges);
    }

    let mut piece_a = orig.clone();
    piece_a.name = if label_a.is_empty() {
        format!("{} (Upper)", orig.name)
    } else {
        label_a.to_string()
    };
    piece_a.seam_loop = loop_a;
    piece_a.mirror_fold_edge = None;
    piece_a.darts.clear();
    piece_a.notches.clear();

    let mut piece_b = orig.clone();
    piece_b.id = id_b;
    piece_b.name = if label_b.is_empty() {
        format!("{} (Lower)", orig.name)
    } else {
        label_b.to_string()
    };
    piece_b.code = format!("{}-B", orig.code);
    piece_b.seam_loop = loop_b;
    piece_b.grainline.start = piece_b.grainline.start.add(sep);
    piece_b.grainline.end = piece_b.grainline.end.add(sep);
    piece_b.mirror_fold_edge = None;
    piece_b.darts.clear();
    piece_b.notches.clear();

    let binding_id = det_id(&format!("{}:split_binding:{ea}:{eb}", orig.id));
    stitching.bindings.push(SeamBinding {
        id: binding_id,
        label: format!("Style Seam ({} <-> {})", piece_a.name, piece_b.name),
        interface_a: SeamInterface {
            id: det_id(&format!("{binding_id}:ia")),
            name: format!("{} Style Seam", piece_a.name),
            edges: vec![EdgeRef {
                piece_id: piece_a.id,
                edge_id: seam_id_a,
                dir: 1,
            }],
        },
        interface_b: SeamInterface {
            id: det_id(&format!("{binding_id}:ib")),
            name: format!("{} Style Seam", piece_b.name),
            edges: vec![EdgeRef {
                piece_id: piece_b.id,
                edge_id: seam_id_b,
                dir: -1,
            }],
        },
        ease: EaseSpec::Exact,
        driver: SeamDriver::DriverA,
        seam_type: SeamType::Plain,
        ease_min_mm: -1.5,
        ease_max_mm: 1.5,
        auto_partner_notches: true,
    });

    pieces[pos] = piece_a;
    pieces.insert(pos + 1, piece_b);
}

/// Rotates a pattern boundary chain around `dart.apex` by `angle_deg`, preserving rotated edge lengths.
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
    let src_idx = dart.resolve_edge_idx(&piece.seam_loop).unwrap_or(0);
    if src_idx == target_edge_idx {
        return;
    }

    let leg_a = eval_edge_point(&piece.seam_loop.edges[src_idx], dart.leg_a_t);
    let leg_b = eval_edge_point(&piece.seam_loop.edges[src_idx], dart.leg_b_t);
    let va = leg_a.sub(dart.apex);
    let vb = leg_b.sub(dart.apex);
    let max_theta = (va.x * vb.y - va.y * vb.x)
        .atan2(va.dot(vb))
        .abs()
        .max(0.05);

    let req_rad = angle_deg.to_radians().abs().clamp(0.02, max_theta);
    let steps_ccw = (target_edge_idx + n - src_idx) % n;
    let steps_cw = (src_idx + n - target_edge_idx) % n;
    let rotate_ccw_sector = steps_ccw <= steps_cw;
    let theta = if rotate_ccw_sector { -req_rad } else { req_rad };

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

    for &idx in &rotated_edges {
        let e = &mut piece.seam_loop.edges[idx];
        e.start = e.start.rotate_around(dart.apex, theta);
        e.segment.transform(|p| p.rotate_around(dart.apex, theta));
    }

    let t_split = target_t.clamp(0.15, 0.85);
    let split_pt = eval_edge_point(&piece.seam_loop.edges[target_edge_idx], t_split);
    let split_pt_rot = split_pt.rotate_around(dart.apex, theta);

    if rotate_ccw_sector {
        let t_edge = &mut piece.seam_loop.edges[target_edge_idx];
        t_edge.start = t_edge.start.rotate_around(dart.apex, theta);
        let next_start = piece.seam_loop.edges[(src_idx + 1) % n].start;
        piece.seam_loop.edges[src_idx].segment.set_end_point(next_start);
    } else {
        let t_edge = &mut piece.seam_loop.edges[target_edge_idx];
        let new_end = t_edge.end().rotate_around(dart.apex, theta);
        t_edge.segment.set_end_point(new_end);
        let prev_end = piece.seam_loop.edges[(src_idx + n - 1) % n].end();
        piece.seam_loop.edges[src_idx].start = prev_end;
    }

    let intake_new = split_pt.dist(split_pt_rot).max(8.0);
    let target_edge_id = piece.seam_loop.edges[target_edge_idx].id;
    piece.darts[dart_pos] = DartEntity {
        id: det_id(&format!("{}:pivoted:{target_edge_idx}", dart.id)),
        name: format!("{} (Pivoted)", dart.name),
        apex: dart.apex,
        edge_idx: target_edge_idx,
        edge_id: Some(target_edge_id),
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
        let Some(ei) = dart.resolve_edge_idx(&piece.seam_loop) else {
            continue;
        };
        let edge = &piece.seam_loop.edges[ei];
        let a = eval_edge_point(edge, dart.leg_a_t);
        let b = eval_edge_point(edge, dart.leg_b_t);
        let mid = a.lerp(b, 0.5);
        let apex_to_mid = mid.sub(dart.apex);
        let fold_leg = match dart.fold_dir {
            DartFoldDir::TowardCf | DartFoldDir::TowardHem => a,
            DartFoldDir::TowardCb | DartFoldDir::TowardSide => b,
        };
        let dir = apex_to_mid.normalized();
        let half_w = 0.5 * a.dist(b);
        let peak_lift = (half_w * 0.22).max(2.5);
        let peak = mid.add(dir.scale(peak_lift));
        caps.push(vec![a, dart.apex, b, peak, fold_leg.lerp(peak, 0.5), a]);
    }
    caps
}

/// Parametric Slash-and-Spread: opens controlled fullness via hinge rotation or parallel shift.
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

    let hinge_edge = &piece.seam_loop.edges[start_edge_idx];
    let hinge_pt = eval_edge_point(hinge_edge, 0.5);
    let spread_edge = &piece.seam_loop.edges[end_edge_idx];
    let spread_mid = eval_edge_point(spread_edge, 0.5);
    let outward = spread_mid.sub(hinge_pt).normalized();
    let tangent = eval_edge_tangent(spread_edge, 0.5);

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
