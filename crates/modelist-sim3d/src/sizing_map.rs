//! Physics-inspired optimal non-uniform cloth mesh resolution & sizing map
//! (Zhang et al., Style3D Research, SIGGRAPH 2025).
//!
//! Computes an optimal base mesh spacing per fabric from the Cerda-Mahadevan / Style3D
//! bending-to-stretching scaling law (`c1 * r^6 + c2 * r^4 = B / E`), refines locally around
//! gathered/eased seams, darts, curved boundaries, and internal fold lines, and transitions
//! smoothly via the Vandeparre wrinklon power law.

use crate::material::SolverMaterial;
use modelist_core::curve::{edge_total_length, eval_edge_point};
use modelist_core::model::{
    CurveSegment, EaseSpec, PatternPiece, Point2, SeamType, StitchingGraph,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Designer bounds for the optimal resolution mapping `c1 * r^6 + c2 * r^4 = B / E`.
#[derive(Debug, Clone, Copy)]
pub struct SizingBounds {
    pub r_min_m: f64,
    pub r_max_m: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl Default for SizingBounds {
    fn default() -> Self {
        Self {
            r_min_m: 0.006,
            r_max_m: 0.016,
            y_min: 5.0e-14,
            y_max: 1.0e-11,
        }
    }
}

impl SizingBounds {
    /// Closed-form `(c1, c2)` coefficients (Paper §4.1).
    pub fn c1_c2(self) -> (f64, f64) {
        let rmin2 = self.r_min_m * self.r_min_m;
        let rmax2 = self.r_max_m * self.r_max_m;
        let rmin4 = rmin2 * rmin2;
        let rmax4 = rmax2 * rmax2;
        let rmin6 = rmin4 * rmin2;
        let rmax6 = rmax4 * rmax2;
        let det = rmax4 * rmin4 * (self.r_max_m + self.r_min_m) * (self.r_max_m - self.r_min_m);
        if det.abs() < 1e-30 {
            return (0.0, 0.0);
        }
        let c1 = (rmin4 * self.y_max - rmax4 * self.y_min) / det;
        let c2 = (-rmin6 * self.y_max + rmax6 * self.y_min) / det;
        (c1, c2)
    }
}

/// Solves `c1 * r^6 + c2 * r^4 = B / E` for `r` in `[bounds.r_min_m, bounds.r_max_m]`.
pub fn solve_optimal_spacing_m(b_nm: f64, e_pa: f64, bounds: SizingBounds) -> f64 {
    let y = (b_nm / e_pa.max(1e3)).clamp(bounds.y_min, bounds.y_max);
    let (c1, c2) = bounds.c1_c2();
    let u_lo_init = bounds.r_min_m * bounds.r_min_m;
    let u_hi_init = bounds.r_max_m * bounds.r_max_m;
    let f = |u: f64| c1 * u * u * u + c2 * u * u - y;
    let df = |u: f64| 3.0 * c1 * u * u + 2.0 * c2 * u;

    if f(u_lo_init) >= 0.0 {
        return bounds.r_min_m;
    }
    if f(u_hi_init) <= 0.0 {
        return bounds.r_max_m;
    }

    let mut lo = u_lo_init;
    let mut hi = u_hi_init;
    let mut u = 0.5 * (lo + hi);
    for _ in 0..24 {
        let fu = f(u);
        if fu.abs() < 1e-22 || (hi - lo) < 1e-12 {
            break;
        }
        if fu < 0.0 {
            lo = u;
        } else {
            hi = u;
        }
        let dfu = df(u);
        let next = if dfu.abs() > 1e-24 { u - fu / dfu } else { 0.5 * (lo + hi) };
        u = if next > lo && next < hi {
            next
        } else {
            0.5 * (lo + hi)
        };
    }
    u.sqrt().clamp(bounds.r_min_m, bounds.r_max_m)
}

#[derive(Debug, Clone, Copy)]
pub struct MaterialSizing {
    pub r_warp_mm: f64,
    pub r_weft_mm: f64,
    pub r_base_mm: f64,
    pub aniso: f64,
}

impl MaterialSizing {
    pub(crate) fn from_material(m: &SolverMaterial, bounds: SizingBounds) -> Self {
        let thick = (m.thickness as f64).max(1e-4);
        let e_warp_pa = (m.e_warp as f64) / thick;
        let e_weft_pa = (m.e_weft as f64) / thick;
        let r_warp_mm = solve_optimal_spacing_m(m.b_warp as f64, e_warp_pa, bounds) * 1000.0;
        let r_weft_mm = solve_optimal_spacing_m(m.b_weft as f64, e_weft_pa, bounds) * 1000.0;
        let r_base_mm = (r_warp_mm * r_weft_mm).sqrt();
        let aniso = (r_warp_mm / r_weft_mm.max(1e-6)).clamp(0.6, 1.65);
        Self {
            r_warp_mm,
            r_weft_mm,
            r_base_mm,
            aniso,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RefinementSource {
    Segment {
        a: Point2,
        b: Point2,
        r_src_mm: f64,
        lw_mm: f64,
        power: f64,
    },
    Point {
        p: Point2,
        r_src_mm: f64,
        lw_mm: f64,
        power: f64,
    },
}

impl RefinementSource {
    #[inline]
    fn eval_mm(&self, q: Point2) -> f64 {
        let (d, r_src, lw, power) = match *self {
            Self::Segment {
                a,
                b,
                r_src_mm,
                lw_mm,
                power,
            } => (dist_point_to_segment(q, a, b), r_src_mm, lw_mm, power),
            Self::Point {
                p,
                r_src_mm,
                lw_mm,
                power,
            } => (q.dist(p), r_src_mm, lw_mm, power),
        };
        if d <= 1e-6 {
            return r_src;
        }
        let ratio = (d / lw.max(1.0)).min(16.0);
        r_src * (1.0 + ratio.powf(power))
    }

    fn reflect_x(&self, cx: f64) -> Self {
        match *self {
            Self::Segment {
                a,
                b,
                r_src_mm,
                lw_mm,
                power,
            } => Self::Segment {
                a: Point2::new(2.0 * cx - a.x, a.y),
                b: Point2::new(2.0 * cx - b.x, b.y),
                r_src_mm,
                lw_mm,
                power,
            },
            Self::Point {
                p,
                r_src_mm,
                lw_mm,
                power,
            } => Self::Point {
                p: Point2::new(2.0 * cx - p.x, p.y),
                r_src_mm,
                lw_mm,
                power,
            },
        }
    }

    fn reflect_line(&self, line_a: Point2, line_dir: Point2) -> Self {
        let ref_pt = |p: Point2| -> Point2 {
            let v = p.sub(line_a);
            let proj = line_dir.scale(v.dot(line_dir));
            line_a.add(proj).sub(v.sub(proj))
        };
        match *self {
            Self::Segment {
                a,
                b,
                r_src_mm,
                lw_mm,
                power,
            } => Self::Segment {
                a: ref_pt(a),
                b: ref_pt(b),
                r_src_mm,
                lw_mm,
                power,
            },
            Self::Point {
                p,
                r_src_mm,
                lw_mm,
                power,
            } => Self::Point {
                p: ref_pt(p),
                r_src_mm,
                lw_mm,
                power,
            },
        }
    }
}

/// Spatial sizing map `r(x, y)` in mm for a pattern piece.
#[derive(Debug, Clone)]
pub struct SizingMap {
    pub r_min_mm: f64,
    pub r_max_mm: f64,
    pub r_base_mm: f64,
    pub aniso: f64,
    pub grain: Point2,
    pub sources: Vec<RefinementSource>,
}

impl SizingMap {
    pub(crate) fn from_piece(
        piece: &PatternPiece,
        stitching: &StitchingGraph,
        all_pieces: &[PatternPiece],
        mat: &SolverMaterial,
        bounds: SizingBounds,
    ) -> Self {
        let ms = MaterialSizing::from_material(mat, bounds);
        let r_min_mm = bounds.r_min_m * 1000.0;
        let r_max_mm = bounds.r_max_m * 1000.0;
        let mut grain = piece.grainline.end.sub(piece.grainline.start);
        if grain.len() < 1e-9 {
            grain = Point2::new(0.0, 1.0);
        } else {
            grain = grain.normalized();
        }

        let mut edge_lens: HashMap<(Uuid, Uuid), f64> = HashMap::new();
        let mut edge_elastic_ratio: HashMap<(Uuid, Uuid), f64> = HashMap::new();
        for p in all_pieces {
            for e in &p.seam_loop.edges {
                edge_lens.insert((p.id, e.id), edge_total_length(e).max(1.0));
                if let Some(el) = e.elastic {
                    if el.target_ratio < 0.98 {
                        edge_elastic_ratio.insert((p.id, e.id), el.target_ratio);
                    }
                }
            }
        }

        let mut edge_rho: HashMap<Uuid, f64> = HashMap::new();
        let mut elastic_edges: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        for b in &stitching.bindings {
            let len_a: f64 = b
                .interface_a
                .edges
                .iter()
                .map(|er| *edge_lens.get(&(er.piece_id, er.edge_id)).unwrap_or(&0.0))
                .sum();
            let len_b: f64 = b
                .interface_b
                .edges
                .iter()
                .map(|er| *edge_lens.get(&(er.piece_id, er.edge_id)).unwrap_or(&0.0))
                .sum();
            let is_gather = matches!(
                b.ease,
                EaseSpec::Gather { .. } | EaseSpec::Pleats { .. }
            ) || b.seam_type == SeamType::Gathered;
            let is_structured_seam =
                matches!(b.seam_type, SeamType::French | SeamType::FlatFelled);
            let binding_el_ratio = b
                .interface_a
                .edges
                .iter()
                .chain(&b.interface_b.edges)
                .filter_map(|er| edge_elastic_ratio.get(&(er.piece_id, er.edge_id)).copied())
                .fold(1.0_f64, f64::min);

            for er in b.interface_a.edges.iter().chain(&b.interface_b.edges) {
                if er.piece_id != piece.id {
                    continue;
                }
                if binding_el_ratio < 0.98 {
                    let rho_el = binding_el_ratio.clamp(0.45, 0.92);
                    elastic_edges.insert(er.edge_id);
                    edge_rho
                        .entry(er.edge_id)
                        .and_modify(|prev| *prev = prev.min(rho_el))
                        .or_insert(rho_el);
                }
                let in_a = b.interface_a.edges.iter().any(|e| e.edge_id == er.edge_id && e.piece_id == piece.id);
                let (my_len, other_len) = if in_a { (len_a, len_b) } else { (len_b, len_a) };
                let rho = if my_len > other_len * 1.02 && other_len > 1.0 {
                    (other_len / my_len).clamp(0.38, 0.96)
                } else if is_gather {
                    match b.ease {
                        EaseSpec::Gather { ratio } => (1.0 / ratio.max(1.1)).clamp(0.38, 0.9),
                        _ => 0.65,
                    }
                } else if is_structured_seam {
                    0.78
                } else {
                    continue;
                };
                edge_rho
                    .entry(er.edge_id)
                    .and_modify(|prev| *prev = prev.min(rho))
                    .or_insert(rho);
            }
        }

        let mut sources = Vec::new();
        let thick_m = (mat.thickness as f64).max(3e-4);

        for edge in &piece.seam_loop.edges {
            if let Some(el) = edge.elastic {
                if el.target_ratio < 0.98 {
                    let rho_el = el.target_ratio.clamp(0.45, 0.96);
                    elastic_edges.insert(edge.id);
                    edge_rho
                        .entry(edge.id)
                        .and_modify(|prev| *prev = prev.min(rho_el))
                        .or_insert(rho_el);
                }
            } else if let Some(hem) = edge.hem {
                let r_hem = (0.75 * ms.r_base_mm).clamp(r_min_mm, r_max_mm);
                let lw_mm = hem.depth_mm.clamp(15.0, 50.0);
                let segs = if matches!(edge.segment, CurveSegment::Cubic { .. }) { 4 } else { 1 };
                for k in 0..segs {
                    let a = eval_edge_point(edge, k as f64 / segs as f64);
                    let b = eval_edge_point(edge, (k + 1) as f64 / segs as f64);
                    sources.push(RefinementSource::Segment {
                        a,
                        b,
                        r_src_mm: r_hem,
                        lw_mm,
                        power: 0.7,
                    });
                }
            }
            if let Some(&rho) = edge_rho.get(&edge.id) {
                let mut r_src_mm = rho * ms.r_base_mm;
                if elastic_edges.contains(&edge.id) {
                    r_src_mm = r_src_mm.min(9.0);
                }
                let r_src_mm = r_src_mm.clamp(r_min_mm, r_max_mm);
                let r0_m = r_src_mm * 0.001;
                let delta = (1.0 - rho).max(0.05);
                // Vandeparre wrinklon transition length (Paper §4.2.1), clamped for pattern scale.
                let lw_mm = (r0_m.powf(1.5) * (delta / (thick_m * thick_m)).powf(0.25) * 0.18 * 1000.0)
                    .clamp(28.0, 140.0);
                let power = 0.65;
                let segs = if matches!(edge.segment, CurveSegment::Cubic { .. }) {
                    6
                } else {
                    1
                };
                for k in 0..segs {
                    let a = eval_edge_point(edge, k as f64 / segs as f64);
                    let b = eval_edge_point(edge, (k + 1) as f64 / segs as f64);
                    sources.push(RefinementSource::Segment {
                        a,
                        b,
                        r_src_mm,
                        lw_mm,
                        power,
                    });
                }
            } else if matches!(edge.segment, CurveSegment::Cubic { .. }) {
                let r_src_mm = (0.82 * ms.r_base_mm).clamp(r_min_mm, r_max_mm);
                for k in 0..4 {
                    let a = eval_edge_point(edge, k as f64 / 4.0);
                    let b = eval_edge_point(edge, (k + 1) as f64 / 4.0);
                    sources.push(RefinementSource::Segment {
                        a,
                        b,
                        r_src_mm,
                        lw_mm: 36.0,
                        power: 0.7,
                    });
                }
            }
        }

        for d in &piece.darts {
            let r_apex = (0.65 * ms.r_base_mm).clamp(r_min_mm, r_max_mm);
            sources.push(RefinementSource::Point {
                p: d.apex,
                r_src_mm: r_apex,
                lw_mm: 42.0,
                power: 0.7,
            });
            if let Some(ei) = d.resolve_edge_idx(&piece.seam_loop) {
                let edge = &piece.seam_loop.edges[ei];
                let pa = eval_edge_point(edge, d.leg_a_t.clamp(0.0, 1.0));
                let pb = eval_edge_point(edge, d.leg_b_t.clamp(0.0, 1.0));
                let r_leg = (0.75 * ms.r_base_mm).clamp(r_min_mm, r_max_mm);
                sources.push(RefinementSource::Segment {
                    a: pa,
                    b: d.apex,
                    r_src_mm: r_leg,
                    lw_mm: 32.0,
                    power: 0.7,
                });
                sources.push(RefinementSource::Segment {
                    a: pb,
                    b: d.apex,
                    r_src_mm: r_leg,
                    lw_mm: 32.0,
                    power: 0.7,
                });
            }
        }

        for il in &piece.internal_lines {
            let r_line = (0.68 * ms.r_base_mm).clamp(r_min_mm, r_max_mm);
            for w in il.pts.windows(2) {
                sources.push(RefinementSource::Segment {
                    a: w[0],
                    b: w[1],
                    r_src_mm: r_line,
                    lw_mm: 38.0,
                    power: 0.7,
                });
            }
        }

        Self {
            r_min_mm,
            r_max_mm,
            r_base_mm: ms.r_base_mm.clamp(r_min_mm, r_max_mm),
            aniso: ms.aniso,
            grain,
            sources,
        }
    }

    pub fn eval_mm(&self, p: Point2) -> f64 {
        let mut r = self.r_base_mm;
        for s in &self.sources {
            let v = s.eval_mm(p);
            if v < r {
                r = v;
            }
        }
        r.clamp(self.r_min_mm, self.r_max_mm)
    }

    pub fn mirrored_x(&self, cx: f64) -> Self {
        Self {
            r_min_mm: self.r_min_mm,
            r_max_mm: self.r_max_mm,
            r_base_mm: self.r_base_mm,
            aniso: self.aniso,
            grain: Point2::new(-self.grain.x, self.grain.y),
            sources: self.sources.iter().map(|s| s.reflect_x(cx)).collect(),
        }
    }

    pub fn with_fold_reflection(&self, fold_a: Point2, fold_dir: Point2) -> Self {
        let mut sources = self.sources.clone();
        sources.extend(self.sources.iter().map(|s| s.reflect_line(fold_a, fold_dir)));
        Self {
            r_min_mm: self.r_min_mm,
            r_max_mm: self.r_max_mm,
            r_base_mm: self.r_base_mm,
            aniso: self.aniso,
            grain: self.grain,
            sources,
        }
    }
}

#[inline]
fn dist_point_to_segment(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b.sub(a);
    let l2 = ab.len_sq();
    if l2 < 1e-12 {
        return p.dist(a);
    }
    let u = (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
    p.dist(a.add(ab.scale(u)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material_tests::rect_piece;
    use modelist_core::model::{det_id, EdgeRef, FabricSpec, SeamBinding, SeamDriver, SeamInterface};

    #[test]
    fn test_optimal_spacing_monotone_in_stiffness() {
        let bounds = SizingBounds::default();
        let silk = SolverMaterial::from_spec(&FabricSpec::from_preset("silk_charmeuse"), false);
        let poplin = SolverMaterial::from_spec(&FabricSpec::from_preset("cotton_poplin"), false);
        let denim = SolverMaterial::from_spec(&FabricSpec::from_preset("denim_12oz"), false);

        let r_silk = MaterialSizing::from_material(&silk, bounds).r_base_mm;
        let r_poplin = MaterialSizing::from_material(&poplin, bounds).r_base_mm;
        let r_denim = MaterialSizing::from_material(&denim, bounds).r_base_mm;

        assert!(
            r_silk < r_poplin && r_poplin < r_denim,
            "Expected silk ({r_silk:.2}) < poplin ({r_poplin:.2}) < denim ({r_denim:.2}) mm"
        );
    }

    #[test]
    fn test_gathered_seam_refines_local_sizing_map() {
        let piece = rect_piece(300.0, 400.0, 0.0);
        let top_edge_id = piece.seam_loop.edges[2].id;
        let stitching = StitchingGraph {
            bindings: vec![SeamBinding {
                id: det_id("test:gather"),
                label: "Gathered top".into(),
                interface_a: SeamInterface {
                    id: det_id("test:ia"),
                    name: "Top".into(),
                    edges: vec![EdgeRef {
                        piece_id: piece.id,
                        edge_id: top_edge_id,
                        dir: 1,
                    }],
                },
                interface_b: SeamInterface {
                    id: det_id("test:ib"),
                    name: "Waist".into(),
                    edges: vec![],
                },
                ease: EaseSpec::Gather { ratio: 2.0 },
                driver: SeamDriver::DriverA,
                seam_type: SeamType::Gathered,
                ease_min_mm: -5.0,
                ease_max_mm: 5.0,
                auto_partner_notches: false,
            }],
        };
        let mat = SolverMaterial::from_spec(&FabricSpec::from_preset("cotton_poplin"), false);
        let map = SizingMap::from_piece(
            &piece,
            &stitching,
            &[piece.clone()],
            &mat,
            SizingBounds::default(),
        );
        let r_near = map.eval_mm(Point2::new(150.0, 398.0));
        let r_far = map.eval_mm(Point2::new(150.0, 100.0));
        assert!(
            r_near < r_far - 1.0,
            "Expected gathered seam ({r_near:.2} mm) finer than interior ({r_far:.2} mm)"
        );
    }
}

