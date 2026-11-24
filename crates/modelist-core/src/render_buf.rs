use crate::curve::{eval_edge_point, eval_edge_tangent};
use crate::model::{CurveSegment, EdgeRef, EvaluatedPiece, Point2, StitchingGraph};
use crate::ops::SeamWalkFrame;
use crate::seam::interface_total_length;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const ROLE_SEAMLINE: u32 = 0;
pub const ROLE_CUTLINE: u32 = 1;
pub const ROLE_DART_INTERNAL: u32 = 2;
pub const ROLE_GRAINLINE: u32 = 3;
pub const ROLE_NOTCH: u32 = 4;
pub const ROLE_GRADED_SIZE: u32 = 5;
pub const ROLE_HANDLE_GUIDE: u32 = 6;
pub const ROLE_WALK_GHOST: u32 = 7;
pub const ROLE_DART_CAP: u32 = 8;
pub const ROLE_SELECTED_EDGE: u32 = 9;
pub const ROLE_MIRROR_FOLD: u32 = 10;
pub const ROLE_DRILL_HOLE: u32 = 11;
pub const ROLE_MIRROR_GHOST: u32 = 12;
pub const ROLE_PARTNER_NOTCH: u32 = 13;
pub const ROLE_SEAM_BINDING: u32 = 14;
pub const ROLE_SEAM_ERROR: u32 = 15;
pub const ROLE_SEAM_DRAFT_A: u32 = 16;
pub const ROLE_SEAM_DRAFT_B: u32 = 17;
pub const ROLE_SELECTED_SEAM: u32 = 18;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasLabel {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub sub: String,
    pub piece_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDimLabel {
    pub piece_id: String,
    pub edge_idx: usize,
    pub x: f32,
    pub y: f32,
    pub length_mm: f32,
    pub is_selected_piece: bool,
    pub is_selected_edge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieceFill {
    pub piece_id: String,
    pub selected: bool,
    /// Flat [x0, y0, x1, y1, ...] world mm coordinates for Canvas2D paper fill
    pub coords: Vec<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct RenderBuffers {
    /// Flat line segments: [x0, y0, x1, y1, ...] (4 floats per segment)
    pub stroke_coords: Vec<f32>,
    /// Flat segment metadata: [role, piece_idx, sub_idx, ...] (3 u32s per segment)
    pub stroke_meta: Vec<u32>,
    /// Flat interactive control points: [x, y, kind(0=vertex,1=c1,2=c2,3=apex), piece_idx, edge_idx] (5 floats per point)
    pub handle_points: Vec<f32>,
    pub labels: Vec<CanvasLabel>,
    pub edge_dims: Vec<EdgeDimLabel>,
    pub piece_fills: Vec<PieceFill>,
}

impl RenderBuffers {
    fn push_seg(&mut self, a: Point2, b: Point2, role: u32, piece_idx: u32, sub_idx: u32) {
        self.stroke_coords
            .extend_from_slice(&[a.x as f32, a.y as f32, b.x as f32, b.y as f32]);
        self.stroke_meta
            .extend_from_slice(&[role, piece_idx, sub_idx]);
    }

    fn push_polyline(
        &mut self,
        pts: &[Point2],
        closed: bool,
        role: u32,
        piece_idx: u32,
        sub_idx: u32,
    ) {
        if pts.len() < 2 {
            return;
        }
        for w in pts.windows(2) {
            self.push_seg(w[0], w[1], role, piece_idx, sub_idx);
        }
        if closed {
            self.push_seg(*pts.last().unwrap(), pts[0], role, piece_idx, sub_idx);
        }
    }

    fn push_handle(&mut self, p: Point2, kind: u8, piece_idx: usize, edge_idx: usize) {
        self.handle_points.extend_from_slice(&[
            p.x as f32,
            p.y as f32,
            kind as f32,
            piece_idx as f32,
            edge_idx as f32,
        ]);
    }
}

fn reflect_point(p: Point2, origin: Point2, dir: Point2) -> Point2 {
    let d = dir.normalized();
    let v = p.sub(origin);
    let proj = d.scale(v.dot(d));
    let perp = v.sub(proj);
    origin.add(proj).sub(perp)
}

#[allow(clippy::too_many_arguments)]
pub fn build_render_buffers(
    evaluated: &[EvaluatedPiece],
    stitching: &StitchingGraph,
    graded_nest: &HashMap<String, Vec<EvaluatedPiece>>,
    active_size: &str,
    size_order: &[String],
    selected_piece: Option<uuid::Uuid>,
    selected_edge: Option<(uuid::Uuid, usize)>,
    selected_binding: Option<uuid::Uuid>,
    draft_seam_a: &[EdgeRef],
    draft_seam_b: &[EdgeRef],
    walk_frame: Option<(&SeamWalkFrame, &EvaluatedPiece)>,
    show_mirror_unfold: bool,
) -> RenderBuffers {
    let mut rb = RenderBuffers::default();

    // 1. Graded size nest overlays
    for (s_idx, s_name) in size_order.iter().enumerate() {
        if s_name == active_size {
            continue;
        }
        if let Some(list) = graded_nest.get(s_name) {
            for (pi, ep) in list.iter().enumerate() {
                if !ep.piece.visible {
                    continue;
                }
                rb.push_polyline(&ep.seam_polyline, true, ROLE_GRADED_SIZE, pi as u32, s_idx as u32);
            }
        }
    }

    // 2. Active pieces
    for (pi, ep) in evaluated.iter().enumerate() {
        let p = &ep.piece;
        if !p.visible {
            continue;
        }
        let p_idx = pi as u32;
        let is_selected_piece = selected_piece == Some(p.id);

        // Paper-cardstock polygon fill buffer
        let mut fill_coords = Vec::with_capacity(ep.seam_polyline.len() * 2);
        for pt in &ep.seam_polyline {
            fill_coords.push(pt.x as f32);
            fill_coords.push(pt.y as f32);
        }
        rb.piece_fills.push(PieceFill {
            piece_id: p.id.to_string(),
            selected: is_selected_piece,
            coords: fill_coords,
        });

        // Live symmetry unfolded mirror preview for cut-on-fold pieces
        if show_mirror_unfold {
            if let Some(fold_idx) = p.mirror_fold_edge {
                if let Some(fe) = p.seam_loop.edges.get(fold_idx) {
                    let axis_origin = fe.start;
                    let axis_dir = fe.end().sub(fe.start).normalized();
                    let mirrored_seam: Vec<Point2> = ep
                        .seam_polyline
                        .iter()
                        .map(|&pt| reflect_point(pt, axis_origin, axis_dir))
                        .collect();
                    rb.push_polyline(&mirrored_seam, true, ROLE_MIRROR_GHOST, p_idx, 0);
                }
            }
        }

        // Cutline (Seam Allowance)
        if ep.cut_polyline.len() >= 3 {
            rb.push_polyline(&ep.cut_polyline, true, ROLE_CUTLINE, p_idx, 0);
        }
        for hole_cut in &ep.cutout_cutlines {
            rb.push_polyline(hole_cut, true, ROLE_CUTLINE, p_idx, 1);
        }

        // Dart caps
        for cap in &ep.dart_cap_lines {
            rb.push_polyline(cap, false, ROLE_DART_CAP, p_idx, 0);
        }

        // Canonical seamline + edge dimension callouts
        for (ei, edge) in p.seam_loop.edges.iter().enumerate() {
            let pts = crate::curve::flatten_edge(edge, 0.15);
            let is_sel_edge = selected_edge == Some((p.id, ei));
            let role = if is_sel_edge {
                ROLE_SELECTED_EDGE
            } else if p.mirror_fold_edge == Some(ei) {
                ROLE_MIRROR_FOLD
            } else {
                ROLE_SEAMLINE
            };
            rb.push_polyline(&pts, false, role, p_idx, ei as u32);

            // Inline edge dimension midpoint offset slightly outward from seamline
            let mid = eval_edge_point(edge, 0.5);
            let tang = eval_edge_tangent(edge, 0.5).normalized();
            let out_n = Point2::new(tang.y, -tang.x); // right normal = outward for CCW
            let offset_d = (edge.sa_width_mm * 0.5 + 14.0).clamp(14.0, 28.0);
            let dim_pt = mid.add(out_n.scale(offset_d));
            let len_mm = ep.edge_lengths_mm.get(ei).copied().unwrap_or(0.0);
            rb.edge_dims.push(EdgeDimLabel {
                piece_id: p.id.to_string(),
                edge_idx: ei,
                x: dim_pt.x as f32,
                y: dim_pt.y as f32,
                length_mm: len_mm as f32,
                is_selected_piece,
                is_selected_edge: is_sel_edge,
            });

            if is_selected_piece || is_sel_edge {
                rb.push_handle(edge.start, 0, pi, ei);
                if let CurveSegment::Cubic { c1, c2, end } = edge.segment {
                    rb.push_seg(edge.start, c1, ROLE_HANDLE_GUIDE, p_idx, ei as u32);
                    rb.push_seg(end, c2, ROLE_HANDLE_GUIDE, p_idx, ei as u32);
                    rb.push_handle(c1, 1, pi, ei);
                    rb.push_handle(c2, 2, pi, ei);
                }
            }
        }

        // Darts
        for d in &p.darts {
            if let Some(ei) = d.resolve_edge_idx(&p.seam_loop) {
                let edge = &p.seam_loop.edges[ei];
                let a = eval_edge_point(edge, d.leg_a_t);
                let b = eval_edge_point(edge, d.leg_b_t);
                rb.push_seg(a, d.apex, ROLE_DART_INTERNAL, p_idx, 0);
                rb.push_seg(d.apex, b, ROLE_DART_INTERNAL, p_idx, 0);
                if is_selected_piece {
                    rb.push_handle(d.apex, 3, pi, ei);
                }
            }
        }

        // Drill holes (crosshair + octagon ring)
        for dh in &p.drill_holes {
            let r = dh.radius_mm.max(3.2);
            rb.push_seg(
                dh.pos.sub(Point2::new(r * 1.4, 0.0)),
                dh.pos.add(Point2::new(r * 1.4, 0.0)),
                ROLE_DRILL_HOLE,
                p_idx,
                0,
            );
            rb.push_seg(
                dh.pos.sub(Point2::new(0.0, r * 1.4)),
                dh.pos.add(Point2::new(0.0, r * 1.4)),
                ROLE_DRILL_HOLE,
                p_idx,
                0,
            );
            let mut ring = Vec::with_capacity(8);
            for k in 0..8 {
                let ang = (k as f64) * std::f64::consts::TAU / 8.0;
                ring.push(dh.pos.add(Point2::new(ang.cos() * r, ang.sin() * r)));
            }
            rb.push_polyline(&ring, true, ROLE_DRILL_HOLE, p_idx, 1);
        }

        // Internal lines
        for il in &p.internal_lines {
            rb.push_polyline(&il.pts, false, ROLE_DART_INTERNAL, p_idx, 1);
        }

        // Grainline + arrowheads
        let g0 = p.grainline.start;
        let g1 = p.grainline.end;
        rb.push_seg(g0, g1, ROLE_GRAINLINE, p_idx, 0);
        let dir = g1.sub(g0).normalized();
        let perp = Point2::new(-dir.y, dir.x);
        rb.push_seg(g1, g1.sub(dir.scale(9.0)).add(perp.scale(4.5)), ROLE_GRAINLINE, p_idx, 0);
        rb.push_seg(g1, g1.sub(dir.scale(9.0)).sub(perp.scale(4.5)), ROLE_GRAINLINE, p_idx, 0);
        rb.push_seg(g0, g0.add(dir.scale(9.0)).add(perp.scale(4.5)), ROLE_GRAINLINE, p_idx, 0);
        rb.push_seg(g0, g0.add(dir.scale(9.0)).sub(perp.scale(4.5)), ROLE_GRAINLINE, p_idx, 0);

        // Projected Notches (user vs auto-projected partner notches)
        for n in &ep.projected_notches {
            let role = if n.is_auto_partner {
                ROLE_PARTNER_NOTCH
            } else {
                ROLE_NOTCH
            };
            let tip = n.cut_pt.add(n.inward_dir.scale(n.depth_mm));
            let tang = Point2::new(-n.inward_dir.y, n.inward_dir.x);
            let offsets: &[f64] = match n.count {
                2 => &[-2.5, 2.5],
                3 => &[-4.5, 0.0, 4.5],
                _ => &[0.0],
            };
            for &off in offsets {
                let base = n.cut_pt.add(tang.scale(off));
                let t = tip.add(tang.scale(off));
                match n.style {
                    crate::model::NotchStyle::V => {
                        rb.push_seg(base.add(tang.scale(2.5)), t, role, p_idx, 0);
                        rb.push_seg(base.sub(tang.scale(2.5)), t, role, p_idx, 0);
                    }
                    crate::model::NotchStyle::T => {
                        rb.push_seg(base, t, role, p_idx, 0);
                        rb.push_seg(t.add(tang.scale(3.0)), t.sub(tang.scale(3.0)), role, p_idx, 0);
                    }
                    crate::model::NotchStyle::Castle | crate::model::NotchStyle::U => {
                        let a = base.add(tang.scale(2.2));
                        let b = base.sub(tang.scale(2.2));
                        let c = t.add(tang.scale(2.2));
                        let d = t.sub(tang.scale(2.2));
                        rb.push_seg(a, c, role, p_idx, 0);
                        rb.push_seg(c, d, role, p_idx, 0);
                        rb.push_seg(d, b, role, p_idx, 0);
                    }
                    crate::model::NotchStyle::Slit => {
                        rb.push_seg(base, t, role, p_idx, 0);
                    }
                }
            }
        }

        // Piece label
        let mid = g0.lerp(g1, 0.55);
        rb.labels.push(CanvasLabel {
            x: mid.x as f32,
            y: mid.y as f32,
            text: format!("{} ({})", p.name, p.code),
            sub: format!(
                "Cut {} · {:.0} cm² · {:?}",
                p.cut_quantity,
                ep.area_mm2 / 100.0,
                p.grainline.rotation_mode
            ),
            piece_id: p.id.to_string(),
        });
    }

    // 3. CLO3D/Fusion-style Color-Coded Seam Binding Indicators & Directional Match Chevrons
    let raw_pieces: Vec<_> = evaluated.iter().map(|e| e.piece.clone()).collect();
    for (bi, binding) in stitching.bindings.iter().enumerate() {
        let la = interface_total_length(&binding.interface_a, &raw_pieces);
        let lb = interface_total_length(&binding.interface_b, &raw_pieces);
        let ease_mm = lb - la;
        let out_of_tol = ease_mm < binding.ease_min_mm || ease_mm > binding.ease_max_mm;
        let is_selected = selected_binding == Some(binding.id);
        let role = if is_selected {
            ROLE_SELECTED_SEAM
        } else if out_of_tol {
            ROLE_SEAM_ERROR
        } else {
            ROLE_SEAM_BINDING
        };

        let mut mid_a: Option<Point2> = None;
        let mut mid_b: Option<Point2> = None;

        for (side_idx, er) in binding
            .interface_a
            .edges
            .iter()
            .map(|e| (0u8, e))
            .chain(binding.interface_b.edges.iter().map(|e| (1u8, e)))
        {
            let Some((pi, ep)) = evaluated
                .iter()
                .enumerate()
                .find(|(_, e)| e.piece.id == er.piece_id && e.piece.visible)
            else {
                continue;
            };
            let Some(edge) = ep.piece.seam_loop.edge_by_id(er.edge_id) else {
                continue;
            };
            let pts = crate::curve::flatten_edge(edge, 0.35);
            if pts.len() < 2 {
                continue;
            }
            let inset_d = if is_selected { 5.2 } else { 4.5 };
            let mut inset = Vec::with_capacity(pts.len());
            for k in 0..pts.len() {
                let prev = pts[k.saturating_sub(1)];
                let next = pts[(k + 1).min(pts.len() - 1)];
                let tang = next.sub(prev).normalized();
                let in_n = Point2::new(-tang.y, tang.x);
                inset.push(pts[k].add(in_n.scale(inset_d)));
            }
            rb.push_polyline(&inset, false, role, pi as u32, bi as u32);

            let mid_pt = eval_edge_point(edge, 0.5);
            if side_idx == 0 && mid_a.is_none() {
                mid_a = Some(mid_pt);
            } else if side_idx == 1 && mid_b.is_none() {
                mid_b = Some(mid_pt);
            }
            let tang = eval_edge_tangent(edge, 0.5).normalized();
            let in_n = Point2::new(-tang.y, tang.x);
            let dir_vec = if er.dir >= 0 { tang } else { tang.scale(-1.0) };
            let chev = if is_selected { 8.5 } else { 6.5 };
            let tip = mid_pt.add(in_n.scale(inset_d)).add(dir_vec.scale(chev * 0.65));
            let base_l = tip.sub(dir_vec.scale(chev)).add(in_n.scale(chev * 0.52));
            let base_r = tip.sub(dir_vec.scale(chev)).sub(in_n.scale(chev * 0.52));
            rb.push_seg(base_l, tip, role, pi as u32, bi as u32);
            rb.push_seg(base_r, tip, role, pi as u32, bi as u32);
        }

        // Visual dashed leader connecting both halves when a Seam Binding is selected
        if is_selected {
            if let (Some(ma), Some(mb)) = (mid_a, mid_b) {
                rb.push_seg(ma, mb, ROLE_HANDLE_GUIDE, 998, bi as u32);
            }
        }
    }

    // 4. Fusion 360 Interactive Seam Collector Draft Preview (Interface A = Blue, Interface B = Teal)
    let mut emit_draft_chain = |refs: &[EdgeRef], role: u32| {
        for (idx, er) in refs.iter().enumerate() {
            let Some((pi, ep)) = evaluated
                .iter()
                .enumerate()
                .find(|(_, e)| e.piece.id == er.piece_id && e.piece.visible)
            else {
                continue;
            };
            let Some(edge) = ep.piece.seam_loop.edge_by_id(er.edge_id) else {
                continue;
            };
            let pts = crate::curve::flatten_edge(edge, 0.25);
            rb.push_polyline(&pts, false, role, pi as u32, idx as u32);

            // Bold directional arrows at 30% and 70% along edge so direction flips read immediately!
            for &frac in &[0.32, 0.68] {
                let p = eval_edge_point(edge, frac);
                let tang = eval_edge_tangent(edge, frac).normalized();
                let in_n = Point2::new(-tang.y, tang.x);
                let dir_vec = if er.dir >= 0 { tang } else { tang.scale(-1.0) };
                let tip = p.add(dir_vec.scale(5.5));
                let bl = tip.sub(dir_vec.scale(9.0)).add(in_n.scale(5.0));
                let br = tip.sub(dir_vec.scale(9.0)).sub(in_n.scale(5.0));
                rb.push_seg(bl, tip, role, pi as u32, idx as u32);
                rb.push_seg(br, tip, role, pi as u32, idx as u32);
            }
        }
    };
    emit_draft_chain(draft_seam_a, ROLE_SEAM_DRAFT_A);
    emit_draft_chain(draft_seam_b, ROLE_SEAM_DRAFT_B);

    // 5. Walk-Seam Ghost Piece Overlay
    if let Some((frame, ghost_ep)) = walk_frame {
        let transformed: Vec<Point2> = ghost_ep
            .seam_polyline
            .iter()
            .map(|&pt| frame.transform_b.apply(pt))
            .collect();
        rb.push_polyline(&transformed, true, ROLE_WALK_GHOST, 999, 0);
        let g0 = frame.transform_b.apply(ghost_ep.piece.grainline.start);
        let g1 = frame.transform_b.apply(ghost_ep.piece.grainline.end);
        rb.push_seg(g0, g1, ROLE_WALK_GHOST, 999, 1);
    }

    rb
}
