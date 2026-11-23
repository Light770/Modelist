use crate::curve::{eval_edge_point, flatten_edge};
use crate::model::{CurveSegment, EvaluatedPiece, Point2};
use rstar::{primitives::GeomWithData, RTree, AABB};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapKind {
    Endpoint,
    DartApex,
    Midpoint,
    OnCurve,
    Grid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapHit {
    pub kind: SnapKind,
    pub point: Point2,
    pub piece_id: Option<String>,
    pub edge_idx: Option<usize>,
    pub param_t: Option<f64>,
    pub dist_mm: f64,
}

#[derive(Debug, Clone)]
struct SnapEntry {
    kind: SnapKind,
    priority: u8,
    piece_id: String,
    edge_idx: usize,
    param_t: f64,
}

type RTreePoint = GeomWithData<[f64; 2], SnapEntry>;

pub struct SpatialSnapIndex {
    tree: RTree<RTreePoint>,
}

impl Default for SpatialSnapIndex {
    fn default() -> Self {
        Self { tree: RTree::new() }
    }
}

impl SpatialSnapIndex {
    pub fn build(evaluated: &[EvaluatedPiece]) -> Self {
        let mut items = Vec::new();
        for ep in evaluated {
            if !ep.piece.visible {
                continue;
            }
            let pid = ep.piece.id.to_string();
            for (ei, edge) in ep.piece.seam_loop.edges.iter().enumerate() {
                items.push(GeomWithData::new(
                    [edge.start.x, edge.start.y],
                    SnapEntry {
                        kind: SnapKind::Endpoint,
                        priority: 4,
                        piece_id: pid.clone(),
                        edge_idx: ei,
                        param_t: 0.0,
                    },
                ));
                let mid = eval_edge_point(edge, 0.5);
                items.push(GeomWithData::new(
                    [mid.x, mid.y],
                    SnapEntry {
                        kind: SnapKind::Midpoint,
                        priority: 3,
                        piece_id: pid.clone(),
                        edge_idx: ei,
                        param_t: 0.5,
                    },
                ));
                let poly = flatten_edge(edge, 1.5);
                let denom = (poly.len().saturating_sub(1)).max(1) as f64;
                for (si, p) in poly.iter().enumerate().skip(1) {
                    if si + 1 < poly.len() {
                        items.push(GeomWithData::new(
                            [p.x, p.y],
                            SnapEntry {
                                kind: SnapKind::OnCurve,
                                priority: 1,
                                piece_id: pid.clone(),
                                edge_idx: ei,
                                param_t: (si as f64) / denom,
                            },
                        ));
                    }
                }
            }
            for d in &ep.piece.darts {
                items.push(GeomWithData::new(
                    [d.apex.x, d.apex.y],
                    SnapEntry {
                        kind: SnapKind::DartApex,
                        priority: 4,
                        piece_id: pid.clone(),
                        edge_idx: d.edge_idx,
                        param_t: 0.5,
                    },
                ));
            }
        }
        Self {
            tree: RTree::bulk_load(items),
        }
    }

    pub fn query_snap(&self, wx: f64, wy: f64, tol_mm: f64) -> Option<SnapHit> {
        let env = AABB::from_corners([wx - tol_mm, wy - tol_mm], [wx + tol_mm, wy + tol_mm]);
        let mut best: Option<(u8, f64, SnapHit)> = None;

        for item in self.tree.locate_in_envelope(env) {
            let pt = *item.geom();
            let d = (pt[0] - wx).hypot(pt[1] - wy);
            if d > tol_mm {
                continue;
            }
            let e = &item.data;
            let replace = match &best {
                None => true,
                Some((bp, bd, _)) => e.priority > *bp || (e.priority == *bp && d < *bd),
            };
            if replace {
                best = Some((
                    e.priority,
                    d,
                    SnapHit {
                        kind: e.kind.clone(),
                        point: Point2::new(pt[0], pt[1]),
                        piece_id: Some(e.piece_id.clone()),
                        edge_idx: Some(e.edge_idx),
                        param_t: Some(e.param_t),
                        dist_mm: d,
                    },
                ));
            }
        }
        best.map(|(_, _, h)| h)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HitResult {
    None,
    Handle {
        piece_id: String,
        edge_idx: usize,
        which: u8,
        point: Point2,
    },
    Vertex {
        piece_id: String,
        edge_idx: usize,
        point: Point2,
    },
    Edge {
        piece_id: String,
        edge_idx: usize,
        param_t: f64,
        point: Point2,
    },
    Piece {
        piece_id: String,
    },
}

pub fn hit_test_pieces(
    evaluated: &[EvaluatedPiece],
    selected_piece: Option<uuid::Uuid>,
    wx: f64,
    wy: f64,
    tol_mm: f64,
) -> HitResult {
    let cursor = Point2::new(wx, wy);

    // 1. Bezier handles on selected piece first
    if let Some(sel_id) = selected_piece {
        if let Some(ep) = evaluated.iter().find(|e| e.piece.id == sel_id && e.piece.visible) {
            for (ei, edge) in ep.piece.seam_loop.edges.iter().enumerate() {
                if let CurveSegment::Cubic { c1, c2, .. } = edge.segment {
                    if c1.dist(cursor) <= tol_mm * 1.15 {
                        return HitResult::Handle {
                            piece_id: sel_id.to_string(),
                            edge_idx: ei,
                            which: 1,
                            point: c1,
                        };
                    }
                    if c2.dist(cursor) <= tol_mm * 1.15 {
                        return HitResult::Handle {
                            piece_id: sel_id.to_string(),
                            edge_idx: ei,
                            which: 2,
                            point: c2,
                        };
                    }
                }
            }
        }
    }

    // 2. Vertices across visible pieces
    let mut best_v: Option<(f64, HitResult)> = None;
    for ep in evaluated {
        if !ep.piece.visible {
            continue;
        }
        for (ei, edge) in ep.piece.seam_loop.edges.iter().enumerate() {
            let d = edge.start.dist(cursor);
            if d <= tol_mm * 1.2 && best_v.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                best_v = Some((
                    d,
                    HitResult::Vertex {
                        piece_id: ep.piece.id.to_string(),
                        edge_idx: ei,
                        point: edge.start,
                    },
                ));
            }
        }
    }
    if let Some((_, hit)) = best_v {
        return hit;
    }

    // 3. Edges across visible pieces
    let mut best_e: Option<(f64, HitResult)> = None;
    for ep in evaluated {
        if !ep.piece.visible {
            continue;
        }
        for (ei, edge) in ep.piece.seam_loop.edges.iter().enumerate() {
            let poly = flatten_edge(edge, 1.0);
            let n_segs = poly.len().saturating_sub(1).max(1) as f64;
            for (si, w) in poly.windows(2).enumerate() {
                let (d, proj_u) = dist_to_segment(cursor, w[0], w[1]);
                if d <= tol_mm && best_e.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                    let param_t = ((si as f64 + proj_u) / n_segs).clamp(0.0, 1.0);
                    best_e = Some((
                        d,
                        HitResult::Edge {
                            piece_id: ep.piece.id.to_string(),
                            edge_idx: ei,
                            param_t,
                            point: w[0].lerp(w[1], proj_u),
                        },
                    ));
                }
            }
        }
    }
    if let Some((_, hit)) = best_e {
        return hit;
    }

    // 4. Inside piece polygon
    for ep in evaluated {
        if ep.piece.visible && point_in_polygon(cursor, &ep.seam_polyline) {
            return HitResult::Piece {
                piece_id: ep.piece.id.to_string(),
            };
        }
    }

    HitResult::None
}

fn dist_to_segment(p: Point2, a: Point2, b: Point2) -> (f64, f64) {
    let ab = b.sub(a);
    let len_sq = ab.len_sq();
    if len_sq < 1e-12 {
        return (p.dist(a), 0.0);
    }
    let u = (p.sub(a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (p.dist(a.add(ab.scale(u))), u)
}

fn point_in_polygon(p: Point2, poly: &[Point2]) -> bool {
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
