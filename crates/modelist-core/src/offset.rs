use crate::curve::{
    edge_total_length, eval_edge_outward_normal, eval_edge_point, flatten_edge,
};
use crate::model::{
    polygon_perimeter, polygon_signed_area, CornerRule, EvaluatedPiece, PatternPiece, Point2,
    ProjectedNotch, SeamBinding, FLATTEN_TOL_MM,
};
use crate::ops::compute_dart_cap_polylines;
use cavalier_contours::polyline::{PlineSource, PlineSourceMut, PlineVertex, Polyline};

pub fn evaluate_piece(
    piece: &PatternPiece,
    bindings: &[SeamBinding],
    all_pieces: &[PatternPiece],
) -> EvaluatedPiece {
    let mut warnings = Vec::new();
    let seam_polyline = piece.seam_loop.flatten(FLATTEN_TOL_MM);
    let edge_lengths_mm: Vec<f64> = piece
        .seam_loop
        .edges
        .iter()
        .map(edge_total_length)
        .collect();
    let perimeter_mm = polygon_perimeter(&seam_polyline);
    let area_mm2 = polygon_signed_area(&seam_polyline).abs();

    let cut_polyline = compute_variable_sa_cutline(piece, bindings, all_pieces);
    let cut_area_mm2 = if cut_polyline.len() >= 3 {
        polygon_signed_area(&cut_polyline).abs()
    } else {
        area_mm2
    };

    let cutout_cutlines = piece
        .cutouts
        .iter()
        .map(|hole| {
            let mut h_pts = hole.flatten(FLATTEN_TOL_MM);
            if polygon_signed_area(&h_pts) > 0.0 {
                h_pts.reverse();
            }
            h_pts
        })
        .collect();

    let dart_cap_lines = compute_dart_cap_polylines(piece);
    let projected_notches = project_notches(piece, &cut_polyline, &mut warnings);

    if has_self_intersection(&cut_polyline) {
        warnings.push("Cutline self-intersection detected after corner resolution".into());
    }

    EvaluatedPiece {
        piece: piece.clone(),
        seam_polyline,
        cut_polyline,
        cutout_cutlines,
        dart_cap_lines,
        projected_notches,
        edge_lengths_mm,
        perimeter_mm,
        area_mm2,
        cut_area_mm2,
        warnings,
    }
}

pub fn compute_variable_sa_cutline(
    piece: &PatternPiece,
    bindings: &[SeamBinding],
    all_pieces: &[PatternPiece],
) -> Vec<Point2> {
    let edges = &piece.seam_loop.edges;
    if edges.is_empty() {
        return Vec::new();
    }

    let n = edges.len();
    let mut per_edge_offsets: Vec<(Vec<Point2>, f64)> = Vec::with_capacity(n);

    for e in edges {
        let w = e.sa_width_mm.max(0.0);
        let pts = flatten_edge(e, FLATTEN_TOL_MM);
        if w <= 1e-6 || pts.len() < 2 {
            per_edge_offsets.push((pts, w));
            continue;
        }
        let offset_pts = offset_open_polyline_outward(&pts, w);
        per_edge_offsets.push((offset_pts, w));
    }

    let mut cut_pts: Vec<Point2> = Vec::with_capacity(n * 8);
    for i in 0..n {
        let next_i = (i + 1) % n;
        let (ref curr_pts, w_i) = per_edge_offsets[i];
        let (ref next_pts, w_next) = per_edge_offsets[next_i];
        if curr_pts.len() < 2 || next_pts.len() < 2 {
            continue;
        }

        for p in &curr_pts[..curr_pts.len() - 1] {
            push_if_distinct(&mut cut_pts, *p);
        }

        let v_seam = edges[i].end();
        let a_end = *curr_pts.last().unwrap();
        let a_prev = curr_pts[curr_pts.len() - 2];
        let b_start = next_pts[0];
        let b_next = next_pts[1];

        let rule = resolve_corner_rule(edges[i].corner_rule, edges[next_i].corner_rule);
        match rule {
            CornerRule::Bevel => {
                push_if_distinct(&mut cut_pts, a_end);
                push_if_distinct(&mut cut_pts, b_start);
            }
            CornerRule::Square => {
                let ta = a_end.sub(a_prev).normalized();
                let tb = b_next.sub(b_start).normalized();
                if let Some(ix) = intersect_lines(a_end, ta, b_start, tb) {
                    if ix.dist(v_seam) <= (w_i + w_next + 40.0) {
                        push_if_distinct(&mut cut_pts, ix);
                    } else {
                        push_if_distinct(&mut cut_pts, a_end.add(ta.scale(w_next)));
                        push_if_distinct(&mut cut_pts, b_start.sub(tb.scale(w_i)));
                    }
                } else {
                    push_if_distinct(&mut cut_pts, a_end);
                    push_if_distinct(&mut cut_pts, b_start);
                }
            }
            CornerRule::FoldedHem => {
                let corner_pt = solve_folded_hem_corner(
                    v_seam,
                    edges[i].corner_rule == CornerRule::FoldedHem,
                    edges[next_i].corner_rule == CornerRule::FoldedHem,
                    &edges[i],
                    &edges[next_i],
                    a_prev,
                    a_end,
                    b_start,
                    b_next,
                    w_i,
                    w_next,
                );
                for p in corner_pt {
                    push_if_distinct(&mut cut_pts, p);
                }
            }
            CornerRule::MatchedSeam => {
                let partner_w = lookup_partner_sa(piece.id, edges[i].id, bindings, all_pieces)
                    .unwrap_or(w_next);
                let ta = a_end.sub(a_prev).normalized();
                let tb = b_next.sub(b_start).normalized();
                if let Some(ix) = intersect_lines(a_end, ta, b_start, tb) {
                    let max_ext = 2.8 * w_i.max(partner_w).max(8.0);
                    if ix.dist(v_seam) <= max_ext {
                        push_if_distinct(&mut cut_pts, ix);
                    } else {
                        push_if_distinct(&mut cut_pts, a_end);
                        push_if_distinct(&mut cut_pts, b_start);
                    }
                } else {
                    push_if_distinct(&mut cut_pts, a_end);
                    push_if_distinct(&mut cut_pts, b_start);
                }
            }
            CornerRule::DartCap | CornerRule::Miter { .. } => {
                let limit = match rule {
                    CornerRule::Miter { limit } => limit.max(1.0),
                    _ => 2.5,
                };
                let ta = a_end.sub(a_prev).normalized();
                let tb = b_next.sub(b_start).normalized();
                if let Some(ix) = intersect_lines(a_end, ta, b_start, tb) {
                    let max_ext = limit * w_i.max(w_next).max(1.0) + 1.0;
                    if ix.dist(v_seam) <= max_ext {
                        push_if_distinct(&mut cut_pts, ix);
                    } else {
                        push_if_distinct(&mut cut_pts, a_end);
                        push_if_distinct(&mut cut_pts, b_start);
                    }
                } else {
                    push_if_distinct(&mut cut_pts, a_end);
                    push_if_distinct(&mut cut_pts, b_start);
                }
            }
        }
    }

    if cut_pts.len() >= 3 && cut_pts.first().unwrap().dist(*cut_pts.last().unwrap()) < 1e-4 {
        cut_pts.pop();
    }

    let pruned = prune_self_intersections(cut_pts);
    let mut cleaned = cleanup_via_cavalier(&pruned);
    if polygon_signed_area(&cleaned) < 0.0 {
        cleaned.reverse();
    }
    cleaned
}

fn cleanup_via_cavalier(pts: &[Point2]) -> Vec<Point2> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut pline = Polyline::new_closed();
    for &p in pts {
        pline.add_vertex(PlineVertex::new(p.x, p.y, 0.0));
    }
    let dedup = pline.remove_redundant(1e-4).unwrap_or(pline);
    dedup
        .iter_vertexes()
        .map(|v| Point2::new(v.x, v.y))
        .collect()
}

fn offset_open_polyline_outward(pts: &[Point2], width_mm: f64) -> Vec<Point2> {
    if pts.len() < 2 {
        return pts.to_vec();
    }
    let mut seg_lines: Vec<(Point2, Point2)> = Vec::with_capacity(pts.len() - 1);
    for w in pts.windows(2) {
        let dir = w[1].sub(w[0]);
        if dir.len() < 1e-9 {
            continue;
        }
        let n = dir.right_normal();
        seg_lines.push((w[0].add(n.scale(width_mm)), w[1].add(n.scale(width_mm))));
    }
    if seg_lines.is_empty() {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(seg_lines.len() + 1);
    out.push(seg_lines[0].0);
    for k in 0..(seg_lines.len() - 1) {
        let (a0, a1) = seg_lines[k];
        let (b0, b1) = seg_lines[k + 1];
        let ta = a1.sub(a0).normalized();
        let tb = b1.sub(b0).normalized();
        if let Some(ix) = intersect_lines(a0, ta, b0, tb) {
            if ix.dist(a1) <= width_mm * 3.0 {
                out.push(ix);
            } else {
                out.push(a1.lerp(b0, 0.5));
            }
        } else {
            out.push(a1.lerp(b0, 0.5));
        }
    }
    out.push(seg_lines.last().unwrap().1);
    out
}

fn resolve_corner_rule(prev: CornerRule, next: CornerRule) -> CornerRule {
    if prev == CornerRule::FoldedHem || next == CornerRule::FoldedHem {
        CornerRule::FoldedHem
    } else if prev == CornerRule::MatchedSeam || next == CornerRule::MatchedSeam {
        CornerRule::MatchedSeam
    } else if prev == CornerRule::Square || next == CornerRule::Square {
        CornerRule::Square
    } else if prev == CornerRule::Bevel || next == CornerRule::Bevel {
        CornerRule::Bevel
    } else {
        prev
    }
}

#[allow(clippy::too_many_arguments)]
fn solve_folded_hem_corner(
    v_seam: Point2,
    curr_is_hem: bool,
    next_is_hem: bool,
    edge_i: &crate::model::PolyEdge,
    edge_next: &crate::model::PolyEdge,
    a_prev: Point2,
    a_end: Point2,
    b_start: Point2,
    b_next: Point2,
    w_i: f64,
    w_next: f64,
) -> Vec<Point2> {
    let ta = a_end.sub(a_prev).normalized();
    let tb = b_next.sub(b_start).normalized();

    if curr_is_hem && !next_is_hem {
        let fold_dir = eval_edge_point(edge_i, 1.0)
            .sub(eval_edge_point(edge_i, 0.75))
            .normalized();
        let refl_b_start = reflect_pt(b_start, v_seam, fold_dir);
        let refl_b_next = reflect_pt(b_next, v_seam, fold_dir);
        let refl_tb = refl_b_next.sub(refl_b_start).normalized();
        if let Some(ix) = intersect_lines(a_end, ta, refl_b_start, refl_tb) {
            if ix.dist(v_seam) <= (w_i + w_next) * 4.0 {
                return vec![ix, b_start];
            }
        }
    } else if next_is_hem && !curr_is_hem {
        let fold_dir = eval_edge_point(edge_next, 0.25)
            .sub(eval_edge_point(edge_next, 0.0))
            .normalized();
        let refl_a_prev = reflect_pt(a_prev, v_seam, fold_dir);
        let refl_a_end = reflect_pt(a_end, v_seam, fold_dir);
        let refl_ta = refl_a_end.sub(refl_a_prev).normalized();
        if let Some(ix) = intersect_lines(refl_a_end, refl_ta, b_start, tb) {
            if ix.dist(v_seam) <= (w_i + w_next) * 4.0 {
                return vec![a_end, ix];
            }
        }
    }

    if let Some(ix) = intersect_lines(a_end, ta, b_start, tb) {
        if ix.dist(v_seam) <= (w_i + w_next) * 3.0 {
            return vec![ix];
        }
    }
    vec![a_end, b_start]
}

fn reflect_pt(p: Point2, origin: Point2, axis_dir: Point2) -> Point2 {
    let d = axis_dir.normalized();
    let v = p.sub(origin);
    let proj = d.scale(v.dot(d));
    let perp = v.sub(proj);
    origin.add(proj).sub(perp)
}

fn lookup_partner_sa(
    piece_id: uuid::Uuid,
    edge_id: uuid::Uuid,
    bindings: &[SeamBinding],
    all_pieces: &[PatternPiece],
) -> Option<f64> {
    for b in bindings {
        if b.interface_a
            .edges
            .iter()
            .any(|e| e.piece_id == piece_id && e.edge_id == edge_id)
        {
            let first = b.interface_b.edges.first()?;
            let pb = all_pieces.iter().find(|p| p.id == first.piece_id)?;
            return pb.seam_loop.edge_by_id(first.edge_id).map(|e| e.sa_width_mm);
        }
        if b.interface_b
            .edges
            .iter()
            .any(|e| e.piece_id == piece_id && e.edge_id == edge_id)
        {
            let first = b.interface_a.edges.first()?;
            let pa = all_pieces.iter().find(|p| p.id == first.piece_id)?;
            return pa.seam_loop.edge_by_id(first.edge_id).map(|e| e.sa_width_mm);
        }
    }
    None
}

fn intersect_lines(p0: Point2, d0: Point2, p1: Point2, d1: Point2) -> Option<Point2> {
    let denom = d0.perp_dot(d1);
    if denom.abs() < 1e-8 {
        return None;
    }
    let t = p1.sub(p0).perp_dot(d1) / denom;
    Some(p0.add(d0.scale(t)))
}

fn push_if_distinct(out: &mut Vec<Point2>, p: Point2) {
    if out.last().map(|last| last.dist(p) > 1e-4).unwrap_or(true) {
        out.push(p);
    }
}

fn segment_intersection(a0: Point2, a1: Point2, b0: Point2, b1: Point2) -> Option<Point2> {
    let da = a1.sub(a0);
    let db = b1.sub(b0);
    let denom = da.perp_dot(db);
    if denom.abs() < 1e-9 {
        return None;
    }
    let diff = b0.sub(a0);
    let u = diff.perp_dot(db) / denom;
    let v = diff.perp_dot(da) / denom;
    if u > 1e-5 && u < 1.0 - 1e-5 && v > 1e-5 && v < 1.0 - 1e-5 {
        Some(a0.add(da.scale(u)))
    } else {
        None
    }
}

pub fn has_self_intersection(pts: &[Point2]) -> bool {
    let n = pts.len();
    if n < 4 {
        return false;
    }
    for i in 0..n {
        let a0 = pts[i];
        let a1 = pts[(i + 1) % n];
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let b0 = pts[j];
            let b1 = pts[(j + 1) % n];
            if segment_intersection(a0, a1, b0, b1).is_some() {
                return true;
            }
        }
    }
    false
}

fn prune_self_intersections(mut pts: Vec<Point2>) -> Vec<Point2> {
    for _ in 0..64 {
        let n = pts.len();
        if n < 4 {
            break;
        }
        let mut found = None;
        'outer: for i in 0..n {
            let a0 = pts[i];
            let a1 = pts[(i + 1) % n];
            for j in (i + 2)..n {
                if i == 0 && j == n - 1 {
                    continue;
                }
                let b0 = pts[j];
                let b1 = pts[(j + 1) % n];
                if let Some(ix) = segment_intersection(a0, a1, b0, b1) {
                    found = Some((i, j, ix));
                    break 'outer;
                }
            }
        }
        let Some((i, j, ix)) = found else {
            break;
        };

        let mut loop_a = Vec::with_capacity(n);
        loop_a.extend_from_slice(&pts[..=i]);
        push_if_distinct(&mut loop_a, ix);
        if j + 1 < n {
            for &p in &pts[(j + 1)..] {
                push_if_distinct(&mut loop_a, p);
            }
        }

        let mut loop_b = Vec::with_capacity(j - i + 2);
        loop_b.push(ix);
        for &p in &pts[(i + 1)..=j] {
            push_if_distinct(&mut loop_b, p);
        }

        let area_a = polygon_signed_area(&loop_a).abs();
        let area_b = polygon_signed_area(&loop_b).abs();
        pts = if area_a >= area_b { loop_a } else { loop_b };
    }
    pts
}

fn project_notches(
    piece: &PatternPiece,
    cut_poly: &[Point2],
    warnings: &mut Vec<String>,
) -> Vec<ProjectedNotch> {
    let mut out = Vec::with_capacity(piece.notches.len());
    for n in &piece.notches {
        let Some(ei) = n.resolve_edge_idx(&piece.seam_loop) else {
            continue;
        };
        let edge = &piece.seam_loop.edges[ei];
        let seam_pt = eval_edge_point(edge, n.param_t);
        let out_n = eval_edge_outward_normal(edge, n.param_t);
        let mut warn = None;

        if edge.sa_width_mm <= 1e-3 {
            warn = Some(format!(
                "Notch on edge `{}` sits on a 0 mm fold/raw edge",
                edge.label
            ));
        } else if n.depth_mm >= edge.sa_width_mm - 1.0 {
            warn = Some(format!(
                "Notch depth ({:.1} mm) approaches or exceeds SA ({:.1} mm) on `{}`",
                n.depth_mm, edge.sa_width_mm, edge.label
            ));
        }

        let cut_pt = ray_intersect_polygon_pub(seam_pt, out_n, cut_poly)
            .unwrap_or_else(|| seam_pt.add(out_n.scale(edge.sa_width_mm)));

        if let Some(ref w) = warn {
            warnings.push(w.clone());
        }

        out.push(ProjectedNotch {
            id: n.id,
            seam_pt,
            cut_pt,
            inward_dir: out_n.scale(-1.0),
            style: n.style,
            count: n.count,
            depth_mm: n.depth_mm.min((edge.sa_width_mm - 1.0).max(2.0)),
            warning: warn,
            is_auto_partner: false,
        });
    }
    out
}

pub fn ray_intersect_polygon_pub(origin: Point2, dir: Point2, poly: &[Point2]) -> Option<Point2> {
    if poly.len() < 3 {
        return None;
    }
    let mut best_t = f64::INFINITY;
    let mut best_pt = None;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let seg = b.sub(a);
        let denom = dir.perp_dot(seg);
        if denom.abs() < 1e-9 {
            continue;
        }
        let diff = a.sub(origin);
        let t = diff.perp_dot(seg) / denom;
        let u = diff.perp_dot(dir) / denom;
        if t >= -0.5 && (-1e-4..=1.0 + 1e-4).contains(&u) && t < best_t {
            best_t = t;
            best_pt = Some(origin.add(dir.scale(t)));
        }
    }
    best_pt
}
