use crate::curve::eval_edge_point;
use crate::model::{CurveSegment, EvaluatedPiece, Point2};
use crate::ops::SeamWalkFrame;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasLabel {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub sub: String,
    pub piece_id: String,
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

pub fn build_render_buffers(
    evaluated: &[EvaluatedPiece],
    graded_nest: &HashMap<String, Vec<EvaluatedPiece>>,
    active_size: &str,
    size_order: &[String],
    selected_piece: Option<uuid::Uuid>,
    selected_edge: Option<(uuid::Uuid, usize)>,
    walk_frame: Option<(&SeamWalkFrame, &EvaluatedPiece)>,
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

        // Canonical seamline (edge by edge so selected edge highlights crisply)
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
            if let Some(edge) = p.seam_loop.edges.get(d.edge_idx) {
                let a = eval_edge_point(edge, d.leg_a_t);
                let b = eval_edge_point(edge, d.leg_b_t);
                rb.push_seg(a, d.apex, ROLE_DART_INTERNAL, p_idx, 0);
                rb.push_seg(d.apex, b, ROLE_DART_INTERNAL, p_idx, 0);
                if is_selected_piece {
                    rb.push_handle(d.apex, 3, pi, d.edge_idx);
                }
            }
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

        // Projected Notches
        for n in &ep.projected_notches {
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
                        rb.push_seg(base.add(tang.scale(2.5)), t, ROLE_NOTCH, p_idx, 0);
                        rb.push_seg(base.sub(tang.scale(2.5)), t, ROLE_NOTCH, p_idx, 0);
                    }
                    crate::model::NotchStyle::T => {
                        rb.push_seg(base, t, ROLE_NOTCH, p_idx, 0);
                        rb.push_seg(t.add(tang.scale(3.0)), t.sub(tang.scale(3.0)), ROLE_NOTCH, p_idx, 0);
                    }
                    crate::model::NotchStyle::Castle | crate::model::NotchStyle::U => {
                        let a = base.add(tang.scale(2.2));
                        let b = base.sub(tang.scale(2.2));
                        let c = t.add(tang.scale(2.2));
                        let d = t.sub(tang.scale(2.2));
                        rb.push_seg(a, c, ROLE_NOTCH, p_idx, 0);
                        rb.push_seg(c, d, ROLE_NOTCH, p_idx, 0);
                        rb.push_seg(d, b, ROLE_NOTCH, p_idx, 0);
                    }
                    crate::model::NotchStyle::Slit => {
                        rb.push_seg(base, t, ROLE_NOTCH, p_idx, 0);
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
                "Cut {} · {:.0} cm² · SA on",
                p.cut_quantity,
                ep.area_mm2 / 100.0
            ),
            piece_id: p.id.to_string(),
        });
    }

    // 3. Walk-Seam Ghost Piece Overlay
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
