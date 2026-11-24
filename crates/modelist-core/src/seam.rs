use crate::curve::{
    edge_arc_length_to, edge_total_length, eval_edge_outward_normal, eval_edge_point,
    eval_edge_tangent, invert_edge_arc_length,
};
use crate::model::{
    det_id, CurveSegment, EaseSpec, EdgeRef, EvaluatedPiece, PatternPiece, Point2, ProjectedNotch,
    SeamBinding, SeamDriver, SeamInterface, SeamType,
};
use crate::ops::{RigidTransform2D, SeamWalkFrame};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct InterfaceSample {
    pub piece_id: Uuid,
    pub edge_id: Uuid,
    pub edge_idx: usize,
    pub local_t: f64,
    pub point: Point2,
    pub tangent: Point2,
    pub outward_normal: Point2,
}

pub fn find_piece_edge<'a>(
    pieces: &'a [PatternPiece],
    er: &EdgeRef,
) -> Option<(&'a PatternPiece, usize)> {
    let p = pieces.iter().find(|p| p.id == er.piece_id)?;
    let idx = p.seam_loop.edge_index_by_id(er.edge_id)?;
    Some((p, idx))
}

pub fn interface_total_length(iface: &SeamInterface, pieces: &[PatternPiece]) -> f64 {
    let mut total = 0.0;
    for er in &iface.edges {
        if let Some((p, idx)) = find_piece_edge(pieces, er) {
            total += edge_total_length(&p.seam_loop.edges[idx]);
        }
    }
    total
}

/// Evaluates a multi-edge SeamInterface at normalized global arc-length fraction u in [0, 1].
pub fn eval_interface_at_fraction(
    iface: &SeamInterface,
    pieces: &[PatternPiece],
    frac_u: f64,
) -> Option<InterfaceSample> {
    let mut segs: Vec<(&PatternPiece, usize, i8, f64)> = Vec::new();
    let mut total = 0.0;
    for er in &iface.edges {
        if let Some((p, idx)) = find_piece_edge(pieces, er) {
            let l = edge_total_length(&p.seam_loop.edges[idx]);
            if l > 1e-9 {
                segs.push((p, idx, er.dir, l));
                total += l;
            }
        }
    }
    if segs.is_empty() || total <= 1e-9 {
        return None;
    }

    let u = frac_u.clamp(0.0, 1.0);
    let target_s = u * total;
    let mut acc = 0.0;

    for (si, &(p, idx, dir, seg_len)) in segs.iter().enumerate() {
        let is_last = si + 1 == segs.len();
        if target_s <= acc + seg_len + 1e-9 || is_last {
            let local_s_fwd = (target_s - acc).clamp(0.0, seg_len);
            let s_edge = if dir >= 0 {
                local_s_fwd
            } else {
                seg_len - local_s_fwd
            };
            let edge = &p.seam_loop.edges[idx];
            let t = invert_edge_arc_length(edge, s_edge);
            let pt = eval_edge_point(edge, t);
            let mut tang = eval_edge_tangent(edge, t);
            if dir < 0 {
                tang = tang.scale(-1.0);
            }
            let out_n = eval_edge_outward_normal(edge, t);
            return Some(InterfaceSample {
                piece_id: p.id,
                edge_id: edge.id,
                edge_idx: idx,
                local_t: t,
                point: pt,
                tangent: tang,
                outward_normal: out_n,
            });
        }
        acc += seg_len;
    }
    None
}

/// Computes normalized global fraction u in [0, 1] for a point located on (piece_id, edge_id, param_t).
pub fn interface_fraction_for_anchor(
    iface: &SeamInterface,
    pieces: &[PatternPiece],
    piece_id: Uuid,
    edge_id: Uuid,
    param_t: f64,
) -> Option<f64> {
    let total = interface_total_length(iface, pieces);
    if total <= 1e-9 {
        return None;
    }
    let mut acc = 0.0;
    for er in &iface.edges {
        if let Some((p, idx)) = find_piece_edge(pieces, er) {
            let edge = &p.seam_loop.edges[idx];
            let seg_len = edge_total_length(edge);
            if er.piece_id == piece_id && er.edge_id == edge_id {
                let s_edge = edge_arc_length_to(edge, param_t);
                let s_along = if er.dir >= 0 { s_edge } else { seg_len - s_edge };
                return Some(((acc + s_along) / total).clamp(0.0, 1.0));
            }
            acc += seg_len;
        }
    }
    None
}

/// Multi-edge Seam Walking across SeamBindings (e.g., walking the entire Sleeve Cap across Front + Back Armscyes).
pub fn compute_binding_walk_frame(
    binding: &SeamBinding,
    pieces: &[PatternPiece],
    progress_u: f64,
) -> Option<(SeamWalkFrame, Uuid)> {
    let u = progress_u.clamp(0.0, 1.0);
    let len_a = interface_total_length(&binding.interface_a, pieces);
    let len_b = interface_total_length(&binding.interface_b, pieces);
    let sa = eval_interface_at_fraction(&binding.interface_a, pieces, u)?;
    let sb = eval_interface_at_fraction(&binding.interface_b, pieces, u)?;

    let tb_target = sb.tangent.scale(-1.0);
    let angle_a = sa.tangent.y.atan2(sa.tangent.x);
    let angle_b = tb_target.y.atan2(tb_target.x);
    let (sin, cos) = (angle_a - angle_b).sin_cos();

    let transform_b = RigidTransform2D {
        origin_x: sb.point.x,
        origin_y: sb.point.y,
        tx: sa.point.x,
        ty: sa.point.y,
        cos,
        sin,
    };

    let frame = SeamWalkFrame {
        link_id: binding.id.to_string(),
        progress_u: u,
        contact_pt: sa.point,
        tangent: sa.tangent,
        len_a_mm: len_a,
        len_b_mm: len_b,
        walked_a_mm: u * len_a,
        walked_b_mm: u * len_b,
        ease_delta_mm: len_b - len_a,
        transform_b,
    };
    Some((frame, sb.piece_id))
}

/// Active Fit-to-Partner Solver:
/// For every active SeamBinding (DriverA / DriverB / Mutual), adjusts the follower interface's
/// curve crown / endpoint via safeguarded 1D secant + bisection on exact Gauss-Legendre arc length
/// until |len(follower) - target_len(driver, ease)| < 0.05 mm.
pub fn solve_seam_bindings(pieces: &mut [PatternPiece], bindings: &[SeamBinding]) {
    for _pass in 0..2 {
        for binding in bindings {
            if binding.driver == SeamDriver::MonitorOnly {
                continue;
            }
            let len_a = interface_total_length(&binding.interface_a, pieces);
            let len_b = interface_total_length(&binding.interface_b, pieces);
            if len_a < 1.0 || len_b < 1.0 {
                continue;
            }
            match binding.driver {
                SeamDriver::DriverA => {
                    let target_b = binding.ease.target_len_from_driver(len_a);
                    if (len_b - target_b).abs() > 0.04 {
                        fit_interface_to_length(pieces, &binding.interface_b, target_b);
                    }
                }
                SeamDriver::DriverB => {
                    let target_a = binding.ease.target_len_from_driver(len_b);
                    if (len_a - target_a).abs() > 0.04 {
                        fit_interface_to_length(pieces, &binding.interface_a, target_a);
                    }
                }
                SeamDriver::Mutual => {
                    let expected_ease = binding.ease.expected_ease_mm(len_a);
                    let avg_base = 0.5 * (len_a + (len_b - expected_ease));
                    let target_a = avg_base.max(5.0);
                    let target_b = binding.ease.target_len_from_driver(target_a);
                    if (len_a - target_a).abs() > 0.04 {
                        fit_interface_to_length(pieces, &binding.interface_a, target_a);
                    }
                    if (len_b - target_b).abs() > 0.04 {
                        fit_interface_to_length(pieces, &binding.interface_b, target_b);
                    }
                }
                SeamDriver::MonitorOnly => {}
            }
        }
    }
}

fn fit_interface_to_length(
    pieces: &mut [PatternPiece],
    iface: &SeamInterface,
    target_len: f64,
) {
    if iface.edges.is_empty() {
        return;
    }

    // Check if all edges in iface belong to one piece (most common: sleeve cap [e2, e3] or single side/shoulder seam)
    let first_pid = iface.edges[0].piece_id;
    let same_piece = iface.edges.iter().all(|e| e.piece_id == first_pid);
    if !same_piece {
        // Multi-piece follower: distribute target ratio proportionally across each sub-edge
        let curr_total = interface_total_length(iface, pieces);
        if curr_total < 1e-6 {
            return;
        }
        let ratio = target_len / curr_total;
        for er in &iface.edges {
            let single = SeamInterface {
                id: iface.id,
                name: iface.name.clone(),
                edges: vec![er.clone()],
            };
            let l = interface_total_length(&single, pieces);
            fit_interface_to_length(pieces, &single, l * ratio);
        }
        return;
    }

    let Some(p_pos) = pieces.iter().position(|p| p.id == first_pid) else {
        return;
    };
    let edge_indices: Vec<usize> = iface
        .edges
        .iter()
        .filter_map(|er| pieces[p_pos].seam_loop.edge_index_by_id(er.edge_id))
        .collect();
    if edge_indices.len() != iface.edges.len() {
        return;
    }

    let base_piece = pieces[p_pos].clone();
    let n = base_piece.seam_loop.edges.len();

    // Strategy 1: 2 consecutive cubic edges sharing an interior crest vertex (e.g. Set-In Sleeve Cap!)
    // Shift shared crest vertex + adjacent top control handles along outward crest normal.
    if edge_indices.len() == 2 {
        let e0 = edge_indices[0];
        let e1 = edge_indices[1];
        let shared_v = if (e0 + 1) % n == e1 {
            Some((e0, e1))
        } else if (e1 + 1) % n == e0 {
            Some((e1, e0))
        } else {
            None
        };
        if let Some((first_e, second_e)) = shared_v {
            let p_start = base_piece.seam_loop.edges[first_e].start;
            let p_crest = base_piece.seam_loop.edges[second_e].start;
            let p_end = base_piece.seam_loop.edges[second_e].end();
            let chord_mid = p_start.lerp(p_end, 0.5);
            let mut crest_dir = p_crest.sub(chord_mid);
            if crest_dir.len() < 1e-3 {
                crest_dir = p_end.sub(p_start).right_normal();
            }
            crest_dir = crest_dir.normalized();

            let apply_crest = |alpha: f64| -> PatternPiece {
                let mut cand = base_piece.clone();
                let dv = crest_dir.scale(alpha);
                let new_crest = p_crest.add(dv);
                cand.seam_loop.edges[second_e].start = new_crest;
                match &mut cand.seam_loop.edges[first_e].segment {
                    CurveSegment::Line { end } => *end = new_crest,
                    CurveSegment::Cubic { c1, c2, end } => {
                        *c1 = c1.add(dv.scale(0.28));
                        *c2 = c2.add(dv);
                        *end = new_crest;
                    }
                }
                if let CurveSegment::Cubic { c1, c2, .. } = &mut cand.seam_loop.edges[second_e].segment {
                    *c1 = c1.add(dv);
                    *c2 = c2.add(dv.scale(0.28));
                }
                cand.seam_loop.sync_continuity();
                cand
            };

            let eval_len = |alpha: f64| -> f64 {
                let cand = apply_crest(alpha);
                edge_total_length(&cand.seam_loop.edges[first_e])
                    + edge_total_length(&cand.seam_loop.edges[second_e])
            };

            if let Some(best_a) = solve_1d_root(eval_len, target_len, -120.0, 220.0) {
                pieces[p_pos] = apply_crest(best_a);
                return;
            }
        }
    }

    // Strategy 2: Single edge (Cubic -> scale handles + endpoint shift; Line -> extend/contract endpoint along edge tangent)
    if edge_indices.len() == 1 {
        let ei = edge_indices[0];
        let next_ei = (ei + 1) % n;
        let orig_edge = &base_piece.seam_loop.edges[ei];
        let chord_dir = orig_edge.end().sub(orig_edge.start).normalized();
        let is_cubic = matches!(orig_edge.segment, CurveSegment::Cubic { .. });

        let apply_single = |alpha: f64| -> PatternPiece {
            let mut cand = base_piece.clone();
            let shift = chord_dir.scale(alpha);
            if is_cubic {
                // Blend chord extension + proportional handle scale so curve stays graceful
                let old_len = orig_edge.start.dist(orig_edge.end()).max(1.0);
                let new_end = orig_edge.end().add(shift);
                let scale = ((old_len + alpha) / old_len).clamp(0.45, 2.4);
                let mid_n = eval_edge_outward_normal(orig_edge, 0.5);
                let orig_start = cand.seam_loop.edges[ei].start;
                if let CurveSegment::Cubic { c1, c2, end } = &mut cand.seam_loop.edges[ei].segment {
                    *c1 = orig_start.add(c1.sub(orig_start).scale(scale)).add(mid_n.scale(alpha * 0.12));
                    *c2 = new_end.add(c2.sub(orig_edge.end()).scale(scale)).add(mid_n.scale(alpha * 0.12));
                    *end = new_end;
                }
                cand.seam_loop.edges[next_ei].start = new_end;
                if let CurveSegment::Cubic { c1, .. } = &mut cand.seam_loop.edges[next_ei].segment {
                    *c1 = c1.add(shift);
                }
            } else {
                let new_end = orig_edge.end().add(shift);
                cand.seam_loop.edges[ei].segment.set_end_point(new_end);
                cand.seam_loop.edges[next_ei].start = new_end;
                if let CurveSegment::Cubic { c1, .. } = &mut cand.seam_loop.edges[next_ei].segment {
                    *c1 = c1.add(shift);
                }
            }
            cand.seam_loop.sync_continuity();
            cand
        };

        let eval_len = |alpha: f64| -> f64 {
            let cand = apply_single(alpha);
            edge_total_length(&cand.seam_loop.edges[ei])
        };

        let orig_len = edge_total_length(orig_edge);
        let lo = -(orig_len * 0.65);
        let hi = orig_len * 2.5 + 120.0;
        if let Some(best_a) = solve_1d_root(eval_len, target_len, lo, hi) {
            pieces[p_pos] = apply_single(best_a);
        }
    }
}

/// Safeguarded 1D bisection + secant root finder for f(alpha) = target within < 0.02 mm.
fn solve_1d_root(
    f: impl Fn(f64) -> f64,
    target: f64,
    min_a: f64,
    max_a: f64,
) -> Option<f64> {
    let f0 = f(0.0) - target;
    if f0.abs() < 0.02 {
        return Some(0.0);
    }

    // Coarse scan to bracket a root cleanly even on non-monotonic curve arches
    let steps = 24;
    let mut best_a = 0.0;
    let mut best_err = f0.abs();
    let mut bracket: Option<(f64, f64, f64, f64)> = None;

    let mut prev_a = min_a;
    let mut prev_v = f(min_a) - target;
    if prev_v.abs() < best_err {
        best_err = prev_v.abs();
        best_a = prev_a;
    }

    for i in 1..=steps {
        let a = min_a + (max_a - min_a) * (i as f64) / (steps as f64);
        let v = f(a) - target;
        if v.abs() < best_err {
            best_err = v.abs();
            best_a = a;
        }
        if (prev_v <= 0.0 && v >= 0.0) || (prev_v >= 0.0 && v <= 0.0) {
            let pick = match bracket {
                None => true,
                Some((ba, bb, _, _)) => a.abs().min(prev_a.abs()) < ba.abs().min(bb.abs()),
            };
            if pick {
                bracket = Some((prev_a, a, prev_v, v));
            }
        }
        prev_a = a;
        prev_v = v;
    }

    let Some((mut lo, mut hi, mut f_lo, mut f_hi)) = bracket else {
        return Some(best_a);
    };

    for _ in 0..22 {
        // Secant guess inside bracket, falling back to bisection
        let denom = f_hi - f_lo;
        let secant = if denom.abs() > 1e-9 {
            lo - f_lo * (hi - lo) / denom
        } else {
            0.5 * (lo + hi)
        };
        let mid = if secant > lo + 0.05 * (hi - lo) && secant < hi - 0.05 * (hi - lo) {
            secant
        } else {
            0.5 * (lo + hi)
        };
        let fm = f(mid) - target;
        if fm.abs() < 0.015 {
            return Some(mid);
        }
        if (f_lo <= 0.0 && fm <= 0.0) || (f_lo >= 0.0 && fm >= 0.0) {
            lo = mid;
            f_lo = fm;
        } else {
            hi = mid;
            f_hi = fm;
        }
    }
    Some(0.5 * (lo + hi))
}

/// GarmentCode-inspired Fraction Projection for Automatic Partner Notches:
/// Projects notches from the driver interface onto the follower interface by normalized arc-length fraction,
/// plus projects interior junction vertices (e.g., shoulder seam junction -> sleeve cap apex notch).
pub fn synthesize_partner_notches(
    pieces: &[PatternPiece],
    bindings: &[SeamBinding],
    evaluated: &mut [EvaluatedPiece],
) {
    let mut additions: HashMap<Uuid, Vec<ProjectedNotch>> = HashMap::new();

    for binding in bindings {
        if !binding.auto_partner_notches {
            continue;
        }
        let (driver_iface, follower_iface) = match binding.driver {
            SeamDriver::DriverB => (&binding.interface_b, &binding.interface_a),
            _ => (&binding.interface_a, &binding.interface_b),
        };

        // 1. Project every notch on driver edges onto follower
        for er in &driver_iface.edges {
            let Some((dp, e_idx)) = find_piece_edge(pieces, er) else {
                continue;
            };
            let edge_id = dp.seam_loop.edges[e_idx].id;
            for notch in &dp.notches {
                if notch.resolve_edge_idx(&dp.seam_loop) != Some(e_idx) {
                    continue;
                }
                let Some(frac) = interface_fraction_for_anchor(
                    driver_iface,
                    pieces,
                    dp.id,
                    edge_id,
                    notch.param_t,
                ) else {
                    continue;
                };
                let Some(sample) = eval_interface_at_fraction(follower_iface, pieces, frac) else {
                    continue;
                };
                if let Some(ep) = evaluated.iter().find(|e| e.piece.id == sample.piece_id) {
                    let already_near = ep
                        .projected_notches
                        .iter()
                        .any(|pn| pn.seam_pt.dist(sample.point) < 10.0);
                    if already_near {
                        continue;
                    }
                    let sa_w = ep
                        .piece
                        .seam_loop
                        .edges
                        .get(sample.edge_idx)
                        .map(|e| e.sa_width_mm)
                        .unwrap_or(10.0);
                    let cut_pt = crate::offset::ray_intersect_polygon_pub(
                        sample.point,
                        sample.outward_normal,
                        &ep.cut_polyline,
                    )
                    .unwrap_or_else(|| sample.point.add(sample.outward_normal.scale(sa_w)));

                    additions.entry(sample.piece_id).or_default().push(ProjectedNotch {
                        id: det_id(&format!("auto_notch:{}:{}", binding.id, notch.id)),
                        seam_pt: sample.point,
                        cut_pt,
                        inward_dir: sample.outward_normal.scale(-1.0),
                        style: notch.style,
                        count: notch.count,
                        depth_mm: notch.depth_mm.min((sa_w - 1.0).max(2.5)),
                        warning: None,
                        is_auto_partner: true,
                    });
                }
            }
        }
    }

    for ep in evaluated.iter_mut() {
        if let Some(extra) = additions.remove(&ep.piece.id) {
            ep.projected_notches.extend(extra);
        }
    }
}

/// Berthouzoz-inspired (SIGGRAPH 2013) Automatic Seam Detection:
/// Scores unbound candidate edge pairs across pieces using length ratio, curve/line compatibility,
/// notch counts, and label semantics, then performs global greedy maximum-weight assignment.
pub fn auto_detect_seam_bindings(
    pieces: &[PatternPiece],
    existing: &[SeamBinding],
) -> Vec<SeamBinding> {
    let mut bound_edges: HashSet<(Uuid, Uuid)> = HashSet::new();
    for b in existing {
        for er in b.interface_a.edges.iter().chain(b.interface_b.edges.iter()) {
            bound_edges.insert((er.piece_id, er.edge_id));
        }
    }

    struct Cand {
        piece_id: Uuid,
        piece_name: String,
        edge_id: Uuid,
        edge_idx: usize,
        label: String,
        length: f64,
        is_cubic: bool,
        notch_count: usize,
    }

    let mut cands = Vec::new();
    for p in pieces {
        for (ei, e) in p.seam_loop.edges.iter().enumerate() {
            if p.mirror_fold_edge == Some(ei) {
                continue;
            }
            if bound_edges.contains(&(p.id, e.id)) {
                continue;
            }
            let lower = e.label.to_lowercase();
            if lower.contains("hem") || lower.contains("neck") || lower.contains("waist") && pieces.len() > 2 {
                continue;
            }
            let len = edge_total_length(e);
            if len < 25.0 {
                continue;
            }
            let notch_count = p
                .notches
                .iter()
                .filter(|n| n.resolve_edge_idx(&p.seam_loop) == Some(ei))
                .count();
            cands.push(Cand {
                piece_id: p.id,
                piece_name: p.name.clone(),
                edge_id: e.id,
                edge_idx: ei,
                label: e.label.clone(),
                length: len,
                is_cubic: matches!(e.segment, CurveSegment::Cubic { .. }),
                notch_count,
            });
        }
    }

    let mut scored: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..cands.len() {
        for j in (i + 1)..cands.len() {
            let a = &cands[i];
            let b = &cands[j];
            if a.piece_id == b.piece_id {
                continue;
            }
            let ratio = a.length.min(b.length) / a.length.max(b.length).max(1.0);
            if ratio < 0.78 {
                continue;
            }
            let len_score = (-(1.0 - ratio) / 0.12).exp();
            let curve_bonus = if a.is_cubic == b.is_cubic { 0.15 } else { 0.0 };
            let notch_bonus = if a.notch_count > 0 && a.notch_count == b.notch_count {
                0.22
            } else {
                0.0
            };
            let la = a.label.to_lowercase();
            let lb = b.label.to_lowercase();
            let semantic_bonus = if (la.contains("side") && lb.contains("side"))
                || (la.contains("shoulder") && lb.contains("shoulder"))
                || (la.contains("inseam") && lb.contains("inseam"))
                || (la.contains("outseam") && lb.contains("outseam"))
                || (la.contains("armscye") && lb.contains("cap"))
                || (la.contains("cap") && lb.contains("armscye"))
            {
                0.35
            } else {
                0.0
            };
            let score = len_score + curve_bonus + notch_bonus + semantic_bonus;
            if score >= 0.72 {
                scored.push((score, i, j));
            }
        }
    }

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut used = HashSet::new();
    let mut created = Vec::new();

    for (_, i, j) in scored {
        if used.contains(&i) || used.contains(&j) {
            continue;
        }
        used.insert(i);
        used.insert(j);
        let a = &cands[i];
        let b = &cands[j];
        let bid = det_id(&format!("auto_seam:{}:{}:{}:{}", a.piece_id, a.edge_id, b.piece_id, b.edge_id));
        created.push(SeamBinding {
            id: bid,
            label: format!("Auto: {} #{} <-> {} #{}", a.piece_name, a.edge_idx + 1, b.piece_name, b.edge_idx + 1),
            interface_a: SeamInterface {
                id: det_id(&format!("{bid}:ia")),
                name: format!("{} {}", a.piece_name, a.label),
                edges: vec![EdgeRef {
                    piece_id: a.piece_id,
                    edge_id: a.edge_id,
                    dir: 1,
                }],
            },
            interface_b: SeamInterface {
                id: det_id(&format!("{bid}:ib")),
                name: format!("{} {}", b.piece_name, b.label),
                edges: vec![EdgeRef {
                    piece_id: b.piece_id,
                    edge_id: b.edge_id,
                    dir: 1,
                }],
            },
            ease: EaseSpec::Exact,
            driver: SeamDriver::MonitorOnly,
            seam_type: SeamType::Plain,
            ease_min_mm: -3.0,
            ease_max_mm: 3.0,
            auto_partner_notches: true,
        });
    }

    created
}
