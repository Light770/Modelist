//! Panel meshing for the 3D fit room.
//!
//! Each piece's canonical seam loop (never the cutline) is sampled at the target spacing, darts are
//! cut out as wedges whose legs are sewn back together in 3D, cut-on-fold pieces are unfolded,
//! and the interior is filled with hex-staggered Steiner points before a spade constrained
//! Delaunay triangulation. The mesh carries everything the solver needs: per-triangle grainline
//! directions for anisotropy, bending hinges, and seam/dart vertex memberships for sewing.

use modelist_core::curve::{
    edge_arc_length_to, edge_total_length, eval_edge_point, invert_edge_arc_length,
};
use modelist_core::model::{ArrangementZone, PatternPiece, Point2, PolyEdge};
use spade::{ConstrainedDelaunayTriangulation, Point2 as SpadePt, Triangulation};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BoundaryVertexRef {
    pub vertex_idx: usize,
    pub edge_id: Uuid,
    /// Arc-length fraction along the full edge, `0..=1`.
    pub edge_frac: f64,
    /// Arc-length fraction along the sewn length of the edge, i.e. with its dart caps folded
    /// away. Both legs of a dart share the same value, so partners never bunch at a closed dart.
    pub sewn_frac: f64,
    /// +1 for the original (wearer's left) side, -1 for the mirrored side.
    pub side_sign: i8,
}

#[derive(Debug, Clone)]
pub struct DartLegRef {
    pub vertex_idx: usize,
    pub dart_id: Uuid,
    /// 0 = leg nearer the edge start, 1 = the other leg.
    pub leg: u8,
    /// 0 at the seam edge, 1 at the apex.
    pub frac: f64,
    pub side_sign: i8,
}

#[derive(Debug, Clone)]
pub struct PanelMesh {
    pub piece_id: Uuid,
    /// +1.0 = original / left, -1.0 = mirrored right
    pub side_sign: f32,
    pub arrangement_zone: ArrangementZone,
    /// 2D rest coordinates in metres relative to the panel's bounding-box centre, X right, Y up.
    pub rest_uv: Vec<(f32, f32)>,
    pub raw_2d_mm: Vec<Point2>,
    pub triangles: Vec<[usize; 3]>,
    /// Unit warp (grainline) direction of each triangle in the panel's 2D frame.
    pub tri_grain: Vec<Point2>,
    /// Interior edges as `[a, b, opposite_1, opposite_2]` for bending.
    pub hinges: Vec<[usize; 4]>,
    pub boundary_refs: Vec<BoundaryVertexRef>,
    pub dart_refs: Vec<DartLegRef>,
    /// Boundary vertices on waist edges (used to report waist placement).
    pub waist_verts: Vec<usize>,
    /// Fold line `(point, unit direction)` for unfolded cut-on-fold pieces, in mm.
    pub fold_line: Option<(Point2, Point2)>,
    pub total_area_mm2: f64,
    pub min_angle_deg: f64,
    /// Darts cut out of this panel (their legs are sewn shut by the solver).
    pub darts_closed: usize,
    /// Sewn length of each meshed edge (total length minus closed dart caps), mm.
    pub edge_sewn_len_mm: Vec<(Uuid, f64)>,
}

#[derive(Clone)]
struct BoundarySample {
    pt: Point2,
    /// `(edge_id, edge_frac, sewn_frac, side)`
    memberships: Vec<(Uuid, f64, f64, i8)>,
    /// `(dart_id, leg, frac, side)`
    dart_marks: Vec<(Uuid, u8, f64, i8)>,
    is_waist: bool,
}

impl BoundarySample {
    fn new(pt: Point2, is_waist: bool) -> Self {
        Self {
            pt,
            memberships: Vec::new(),
            dart_marks: Vec::new(),
            is_waist,
        }
    }

    fn merge(&mut self, other: BoundarySample) {
        self.is_waist |= other.is_waist;
        for m in other.memberships {
            push_membership(&mut self.memberships, m);
        }
        for d in other.dart_marks {
            push_dart_mark(&mut self.dart_marks, d);
        }
    }
}

fn push_membership(list: &mut Vec<(Uuid, f64, f64, i8)>, item: (Uuid, f64, f64, i8)) {
    if !list
        .iter()
        .any(|m| m.0 == item.0 && m.3 == item.3 && (m.1 - item.1).abs() < 1e-6)
    {
        list.push(item);
    }
}

fn push_dart_mark(list: &mut Vec<(Uuid, u8, f64, i8)>, item: (Uuid, u8, f64, i8)) {
    if !list
        .iter()
        .any(|m| m.0 == item.0 && m.1 == item.1 && m.3 == item.3 && (m.2 - item.2).abs() < 1e-6)
    {
        list.push(item);
    }
}

#[derive(Debug, Clone)]
struct DartCut {
    id: Uuid,
    s_a: f64,
    s_b: f64,
    pa: Point2,
    pb: Point2,
    apex: Point2,
}

/// Valid darts on edge `ei`, sorted along the edge, as arc-length cuts.
fn dart_cuts_for_edge(
    piece: &PatternPiece,
    ei: usize,
    edge: &PolyEdge,
    inside_poly: Option<&[Point2]>,
) -> Vec<DartCut> {
    let Some(poly) = inside_poly else {
        return Vec::new();
    };
    let len = edge_total_length(edge);
    let mut cuts: Vec<DartCut> = piece
        .darts
        .iter()
        .filter(|d| d.resolve_edge_idx(&piece.seam_loop) == Some(ei))
        .filter_map(|d| {
            let t_lo = d.leg_a_t.min(d.leg_b_t).clamp(0.002, 0.998);
            let t_hi = d.leg_a_t.max(d.leg_b_t).clamp(0.002, 0.998);
            if !(t_lo.is_finite() && t_hi.is_finite()) {
                return None;
            }
            let s_a = edge_arc_length_to(edge, t_lo);
            let s_b = edge_arc_length_to(edge, t_hi);
            if s_b - s_a < 2.0 || s_a < 1.0 || s_b > len - 1.0 {
                return None;
            }
            let pa = eval_edge_point(edge, t_lo);
            let pb = eval_edge_point(edge, t_hi);
            let mid = Point2::new(0.5 * (pa.x + pb.x), 0.5 * (pa.y + pb.y));
            let apex = d.apex;
            if !(apex.x.is_finite() && apex.y.is_finite()) || apex.dist(mid) < 8.0 {
                return None;
            }
            if !point_in_poly(apex, poly) || dist_to_poly_boundary(apex, poly) < 2.0 {
                return None;
            }
            Some(DartCut {
                id: d.id,
                s_a,
                s_b,
                pa,
                pb,
                apex,
            })
        })
        .collect();
    cuts.sort_by(|a, b| a.s_a.partial_cmp(&b.s_a).unwrap_or(std::cmp::Ordering::Equal));
    let mut kept: Vec<DartCut> = Vec::with_capacity(cuts.len());
    for c in cuts {
        if kept.last().is_none_or(|k| c.s_a > k.s_b + 1.0) {
            kept.push(c);
        }
    }
    kept
}

/// Samples one edge from its start (inclusive) to its end (exclusive), inserting dart wedges.
fn push_edge_samples(
    out: &mut Vec<BoundarySample>,
    edge: &PolyEdge,
    side: i8,
    spacing: f64,
    cuts: &[DartCut],
) -> (usize, f64) {
    let is_waist = edge.label.to_lowercase().contains("waist");
    let len = edge_total_length(edge).max(1e-9);
    let cap_total: f64 = cuts.iter().map(|c| c.s_b - c.s_a).sum();
    let len_eff = (len - cap_total).max(1e-6);
    let sewn = |s: f64| -> f64 {
        let folded: f64 = cuts
            .iter()
            .filter(|c| c.s_b <= s + 1e-9)
            .map(|c| c.s_b - c.s_a)
            .sum();
        ((s - folded) / len_eff).clamp(0.0, 1.0)
    };
    let on_edge = |s: f64| -> BoundarySample {
        let t = invert_edge_arc_length(edge, s);
        let mut b = BoundarySample::new(eval_edge_point(edge, t), is_waist);
        b.memberships.push((edge.id, (s / len).clamp(0.0, 1.0), sewn(s), side));
        b
    };

    let mut segments: Vec<(f64, f64, Option<&DartCut>)> = Vec::new();
    let mut s0 = 0.0;
    for c in cuts {
        segments.push((s0, c.s_a, Some(c)));
        s0 = c.s_b;
    }
    segments.push((s0, len, None));

    let mut closed = 0usize;
    for (si, (s_lo, s_hi, dart)) in segments.into_iter().enumerate() {
        let steps = (((s_hi - s_lo) / spacing).ceil() as usize).clamp(1, 64);
        for k in 0..steps {
            if k == 0 && si > 0 {
                continue; // the segment start was emitted as the previous dart's second leg point
            }
            out.push(on_edge(s_lo + (s_hi - s_lo) * (k as f64) / (steps as f64)));
        }
        if let Some(c) = dart {
            let mut a = on_edge(c.s_a);
            a.pt = c.pa;
            a.dart_marks.push((c.id, 0, 0.0, side));
            out.push(a);
            let n_a = ((c.apex.dist(c.pa) / spacing).ceil() as usize).clamp(1, 32);
            for k in 1..n_a {
                let f = k as f64 / n_a as f64;
                let mut b = BoundarySample::new(lerp_pt(c.pa, c.apex, f), false);
                b.dart_marks.push((c.id, 0, f, side));
                out.push(b);
            }
            let mut apex = BoundarySample::new(c.apex, false);
            apex.dart_marks.push((c.id, 0, 1.0, side));
            apex.dart_marks.push((c.id, 1, 1.0, side));
            out.push(apex);
            let n_b = ((c.apex.dist(c.pb) / spacing).ceil() as usize).clamp(1, 32);
            for k in (1..n_b).rev() {
                let f = k as f64 / n_b as f64;
                let mut b = BoundarySample::new(lerp_pt(c.pb, c.apex, f), false);
                b.dart_marks.push((c.id, 1, f, side));
                out.push(b);
            }
            let mut b = on_edge(c.s_b);
            b.pt = c.pb;
            b.dart_marks.push((c.id, 1, 0.0, side));
            out.push(b);
            closed += 1;
        }
    }
    (closed, len_eff)
}

/// Builds the boundary samples of the (optionally unfolded) piece in its own 2D frame.
fn build_boundary(
    piece: &PatternPiece,
    unfold: bool,
    base_side: i8,
    spacing: f64,
    inside_poly: Option<&[Point2]>,
) -> Boundary {
    let edges = &piece.seam_loop.edges;
    let n_edges = edges.len();
    let mut samples: Vec<BoundarySample> = Vec::new();
    let mut darts = 0usize;
    let mut sewn_lens: Vec<(Uuid, f64)> = Vec::with_capacity(n_edges);
    let fold = if unfold { piece.mirror_fold_edge } else { None }.filter(|&f| f < n_edges);

    if let Some(fold_idx) = fold {
        let active = n_edges - 1;
        for offset in 0..active {
            let ei = (fold_idx + 1 + offset) % n_edges;
            let edge = &edges[ei];
            let first = samples.len();
            let cuts = dart_cuts_for_edge(piece, ei, edge, inside_poly);
            let (closed, sewn) = push_edge_samples(&mut samples, edge, 1, spacing, &cuts);
            darts += closed;
            sewn_lens.push((edge.id, sewn));
            if offset > 0 {
                let prev = &edges[(fold_idx + offset) % n_edges];
                if let Some(s) = samples.get_mut(first) {
                    push_membership(&mut s.memberships, (prev.id, 1.0, 1.0, 1));
                }
            }
            if offset == active - 1 {
                let mut end = BoundarySample::new(
                    eval_edge_point(edge, 1.0),
                    edge.label.to_lowercase().contains("waist"),
                );
                end.memberships.push((edge.id, 1.0, 1.0, 1));
                samples.push(end);
            }
        }

        // Reflect across the fold line to build the full panel. The two fold-axis vertices are
        // shared and receive mirrored (-1) memberships too.
        let fe = &edges[fold_idx];
        let a = fe.start;
        let dir = fe.end().sub(fe.start).normalized();
        let orig_len = samples.len();
        if orig_len >= 3 {
            for idx in [0, orig_len - 1] {
                let s = &mut samples[idx];
                let mirrored: Vec<_> = s.memberships.iter().map(|m| (m.0, m.1, m.2, -1)).collect();
                for m in mirrored {
                    push_membership(&mut s.memberships, m);
                }
                let mirrored_d: Vec<_> = s.dart_marks.iter().map(|d| (d.0, d.1, d.2, -1)).collect();
                for d in mirrored_d {
                    push_dart_mark(&mut s.dart_marks, d);
                }
            }
            let reflect = |p: Point2| -> Point2 {
                let v = p.sub(a);
                let proj = dir.scale(v.dot(dir));
                a.add(proj).sub(v.sub(proj))
            };
            for idx in (1..orig_len - 1).rev() {
                let src = &samples[idx];
                let mut b = BoundarySample::new(reflect(src.pt), src.is_waist);
                b.memberships = src.memberships.iter().map(|m| (m.0, m.1, m.2, -1)).collect();
                b.dart_marks = src.dart_marks.iter().map(|d| (d.0, d.1, d.2, -1)).collect();
                samples.push(b);
            }
        }
        Boundary {
            samples,
            darts,
            fold_line: Some((a, dir)),
            sewn_lens,
        }
    } else {
        let mut starts = Vec::with_capacity(n_edges);
        for (ei, edge) in edges.iter().enumerate() {
            starts.push(samples.len());
            let cuts = dart_cuts_for_edge(piece, ei, edge, inside_poly);
            let (closed, sewn) = push_edge_samples(&mut samples, edge, base_side, spacing, &cuts);
            darts += closed;
            sewn_lens.push((edge.id, sewn));
        }
        for (ei, edge) in edges.iter().enumerate() {
            if let Some(s) = samples.get_mut(starts[(ei + 1) % n_edges]) {
                push_membership(&mut s.memberships, (edge.id, 1.0, 1.0, base_side));
            }
        }
        Boundary {
            samples,
            darts,
            fold_line: None,
            sewn_lens,
        }
    }
}

struct Boundary {
    samples: Vec<BoundarySample>,
    darts: usize,
    fold_line: Option<(Point2, Point2)>,
    sewn_lens: Vec<(Uuid, f64)>,
}

fn dedup_boundary(samples: Vec<BoundarySample>) -> Vec<BoundarySample> {
    let mut clean: Vec<BoundarySample> = Vec::with_capacity(samples.len());
    for item in samples {
        if let Some(last) = clean.last_mut() {
            if last.pt.dist(item.pt) <= 1.5 {
                last.merge(item);
                continue;
            }
        }
        clean.push(item);
    }
    if clean.len() > 3 && clean[0].pt.dist(clean[clean.len() - 1].pt) < 1.5 {
        if let Some(last) = clean.pop() {
            clean[0].merge(last);
        }
    }
    clean
}

/// Discretizes a piece's canonical `seam_loop` into a solver-ready panel mesh.
pub fn mesh_pattern_piece(
    piece: &PatternPiece,
    mirror_x: bool,
    unfold_mirror_fold: bool,
    target_spacing_mm: f64,
) -> Option<PanelMesh> {
    let spacing = target_spacing_mm.clamp(8.0, 35.0);
    if piece.seam_loop.edges.len() < 3 {
        return None;
    }
    let base_side: i8 = if mirror_x { -1 } else { 1 };
    let unfold = unfold_mirror_fold && piece.mirror_fold_edge.is_some();

    // Pass 1 without darts gives the outline used to validate dart apexes.
    let plain = build_boundary(piece, unfold, base_side, spacing, None).samples;
    let plain_poly: Vec<Point2> = dedup_boundary(plain).iter().map(|b| b.pt).collect();
    if plain_poly.len() < 3 {
        return None;
    }
    let with_darts = !piece.darts.is_empty();
    if with_darts {
        if let Some(m) = triangulate(piece, mirror_x, unfold, base_side, spacing, Some(&plain_poly)) {
            return Some(m);
        }
    }
    triangulate(piece, mirror_x, unfold, base_side, spacing, None)
}

fn triangulate(
    piece: &PatternPiece,
    mirror_x: bool,
    unfold: bool,
    base_side: i8,
    spacing: f64,
    dart_poly: Option<&[Point2]>,
) -> Option<PanelMesh> {
    let Boundary {
        mut samples,
        darts: darts_closed,
        fold_line,
        sewn_lens: edge_sewn_len_mm,
    } = build_boundary(piece, unfold, base_side, spacing, dart_poly);

    // Mirror X for the right-hand copy of a cut-2 pair (keeps the loop CCW).
    let mut grain = piece.grainline.end.sub(piece.grainline.start);
    if grain.len() < 1e-9 {
        grain = Point2::new(0.0, 1.0);
    }
    let mut grain = grain.normalized();
    if mirror_x {
        let cx = piece.grainline.start.x;
        for s in &mut samples {
            s.pt.x = 2.0 * cx - s.pt.x;
        }
        samples.reverse();
        grain = Point2::new(-grain.x, grain.y);
    }

    let clean = dedup_boundary(samples);
    if clean.len() < 3 {
        return None;
    }
    let poly: Vec<Point2> = clean.iter().map(|b| b.pt).collect();

    let mut cdt: ConstrainedDelaunayTriangulation<SpadePt<f64>> =
        ConstrainedDelaunayTriangulation::new();
    let mut handles = Vec::with_capacity(clean.len());
    for b in &clean {
        match cdt.insert(SpadePt::new(b.pt.x, b.pt.y)) {
            Ok(h) => handles.push(h),
            Err(_) => return None,
        }
    }
    for i in 0..handles.len() {
        let a = handles[i];
        let b = handles[(i + 1) % handles.len()];
        if a == b {
            continue;
        }
        if cdt.can_add_constraint(a, b) {
            cdt.add_constraint(a, b);
        } else if dart_poly.is_some() {
            return None; // dart wedges made the outline self-intersect: retry without darts
        }
    }

    // Interior hex-staggered Steiner points, clear of the boundary.
    let (min_x, min_y, max_x, max_y) = bbox(&poly);
    let dy = spacing * 0.866;
    let margin = spacing * 0.62;
    let mut row = 0usize;
    let mut y = min_y + margin;
    while y < max_y - margin {
        let x_off = if row % 2 == 1 { spacing * 0.5 } else { 0.0 };
        let mut x = min_x + margin + x_off;
        while x < max_x - margin {
            let pt = Point2::new(x, y);
            if point_in_poly(pt, &poly) && dist_to_poly_boundary(pt, &poly) >= margin {
                let _ = cdt.insert(SpadePt::new(x, y));
            }
            x += spacing;
        }
        y += dy;
        row += 1;
    }

    let mut handle_to_idx: HashMap<spade::handles::FixedVertexHandle, usize> = HashMap::new();
    let mut raw_2d_mm: Vec<Point2> = Vec::new();
    for v in cdt.vertices() {
        handle_to_idx.insert(v.fix(), raw_2d_mm.len());
        let p = v.position();
        raw_2d_mm.push(Point2::new(p.x, p.y));
    }

    // Reflected half of an unfolded panel: grain mirrored across the fold line.
    let fold_side = |p: Point2| -> f64 {
        match fold_line {
            Some((a, d)) => d.perp_dot(p.sub(a)),
            None => 0.0,
        }
    };
    let original_sign = clean
        .iter()
        .find(|b| b.memberships.iter().any(|m| m.3 == 1) && fold_side(b.pt).abs() > 1.0)
        .map(|b| fold_side(b.pt).signum())
        .unwrap_or(1.0);
    let reflect_dir = |g: Point2| -> Point2 {
        match fold_line {
            Some((_, d)) => {
                let proj = d.scale(g.dot(d));
                proj.sub(g.sub(proj)).normalized()
            }
            None => g,
        }
    };

    let mut triangles: Vec<[usize; 3]> = Vec::new();
    let mut tri_grain: Vec<Point2> = Vec::new();
    let mut total_area_mm2 = 0.0_f64;
    let mut min_angle_deg = 180.0_f64;
    for face in cdt.inner_faces() {
        let vs = face.vertices();
        let p0 = Point2::new(vs[0].position().x, vs[0].position().y);
        let p1 = Point2::new(vs[1].position().x, vs[1].position().y);
        let p2 = Point2::new(vs[2].position().x, vs[2].position().y);
        let c = Point2::new((p0.x + p1.x + p2.x) / 3.0, (p0.y + p1.y + p2.y) / 3.0);
        if !point_in_poly(c, &poly) {
            continue;
        }
        let area = 0.5 * p1.sub(p0).perp_dot(p2.sub(p0)).abs();
        if area < 1e-3 {
            continue;
        }
        total_area_mm2 += area;
        min_angle_deg = min_angle_deg.min(triangle_min_angle_deg(p0, p1, p2));
        triangles.push([
            handle_to_idx[&vs[0].fix()],
            handle_to_idx[&vs[1].fix()],
            handle_to_idx[&vs[2].fix()],
        ]);
        let reflected = fold_line.is_some() && fold_side(c) * original_sign < -1e-6;
        tri_grain.push(if reflected { reflect_dir(grain) } else { grain });
    }
    if triangles.is_empty() {
        return None;
    }

    let center = Point2::new(0.5 * (min_x + max_x), 0.5 * (min_y + max_y));
    let rest_uv: Vec<(f32, f32)> = raw_2d_mm
        .iter()
        .map(|p| (((p.x - center.x) * 0.001) as f32, ((p.y - center.y) * 0.001) as f32))
        .collect();

    let mut edge_to_opp: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for &[a, b, c] in &triangles {
        for (u, v, opp) in [(a, b, c), (b, c, a), (c, a, b)] {
            let key = if u < v { (u, v) } else { (v, u) };
            edge_to_opp.entry(key).or_default().push(opp);
        }
    }
    let hinges: Vec<[usize; 4]> = edge_to_opp
        .into_iter()
        .filter(|(_, opps)| opps.len() == 2 && opps[0] != opps[1])
        .map(|((i, j), opps)| [i, j, opps[0], opps[1]])
        .collect();

    let mut boundary_refs = Vec::with_capacity(clean.len() * 2);
    let mut dart_refs = Vec::new();
    let mut waist_verts = Vec::new();
    for (bi, sample) in clean.iter().enumerate() {
        let Some(&v_idx) = handles.get(bi).and_then(|h| handle_to_idx.get(h)) else {
            continue;
        };
        for &(edge_id, edge_frac, sewn_frac, side_sign) in &sample.memberships {
            boundary_refs.push(BoundaryVertexRef {
                vertex_idx: v_idx,
                edge_id,
                edge_frac,
                sewn_frac,
                side_sign,
            });
        }
        for &(dart_id, leg, frac, side_sign) in &sample.dart_marks {
            dart_refs.push(DartLegRef {
                vertex_idx: v_idx,
                dart_id,
                leg,
                frac,
                side_sign,
            });
        }
        if sample.is_waist {
            waist_verts.push(v_idx);
        }
    }

    Some(PanelMesh {
        piece_id: piece.id,
        side_sign: if mirror_x { -1.0 } else { 1.0 },
        arrangement_zone: piece.arrangement_zone,
        rest_uv,
        raw_2d_mm,
        triangles,
        tri_grain,
        hinges,
        boundary_refs,
        dart_refs,
        waist_verts,
        fold_line,
        total_area_mm2,
        min_angle_deg,
        darts_closed,
        edge_sewn_len_mm,
    })
}

#[inline]
fn lerp_pt(a: Point2, b: Point2, t: f64) -> Point2 {
    Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn bbox(pts: &[Point2]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in pts {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    (min_x, min_y, max_x, max_y)
}

pub(crate) fn point_in_poly(p: Point2, poly: &[Point2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let a = poly[i];
        let b = poly[j];
        if ((a.y > p.y) != (b.y > p.y))
            && (p.x < (b.x - a.x) * (p.y - a.y) / ((b.y - a.y) + 1e-18) + a.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_to_poly_boundary(p: Point2, poly: &[Point2]) -> f64 {
    let mut min_d = f64::INFINITY;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let ab = b.sub(a);
        let l2 = ab.len_sq();
        let d = if l2 < 1e-12 {
            p.dist(a)
        } else {
            let u = (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
            p.dist(a.add(ab.scale(u)))
        };
        min_d = min_d.min(d);
    }
    min_d
}

fn triangle_min_angle_deg(a: Point2, b: Point2, c: Point2) -> f64 {
    let ang = |p: Point2, q: Point2, r: Point2| -> f64 {
        let u = q.sub(p).normalized();
        let v = r.sub(p).normalized();
        u.dot(v).clamp(-1.0, 1.0).acos().to_degrees()
    };
    ang(a, b, c).min(ang(b, c, a)).min(ang(c, a, b))
}
