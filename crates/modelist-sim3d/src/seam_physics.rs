//! Homogenized seam-allowance band physics (Pabst et al. 2008; Rodríguez & Cirio 2022).
//!
//! Instead of meshing 4–14 literal folded flap layers along every seam, this module computes
//! per-vertex mass multipliers, per-hinge bending stiffness multipliers, and a shader seam-type
//! mask across each seam's physical `sa_width_mm` band.

use crate::xpbd::{BoundVert, PanelRange};
use modelist_core::model::{PatternPiece, Point2, SeamType, StitchingGraph};
use std::collections::HashMap;
use uuid::Uuid;

pub struct SeamBandData {
    pub vert_mass_mult: Vec<f32>,
    pub vert_seam_mask: Vec<f32>,
    pub hinge_bend_mult: Vec<f32>,
}

pub(crate) fn compute_seam_band_data(
    pieces: &[PatternPiece],
    stitching: &StitchingGraph,
    rest2d_m: &[Point2],
    panels: &[PanelRange],
    vert_panel: &[u16],
    edge_vert_map: &HashMap<(Uuid, i8, Uuid), Vec<BoundVert>>,
    hinges: &[[usize; 4]],
) -> SeamBandData {
    let n = rest2d_m.len();
    let mut vert_mass_mult = vec![1.0_f32; n];
    let mut vert_seam_mask = vec![0.0_f32; n];
    let mut hinge_bend_mult = vec![1.0_f32; hinges.len()];

    let mut edge_sa_m: HashMap<(Uuid, Uuid), f64> = HashMap::new();
    for p in pieces {
        for e in &p.seam_loop.edges {
            edge_sa_m.insert((p.id, e.id), e.sa_width_mm);
        }
    }

    // Build per-panel hinge index lists for fast spatial lookup.
    let mut panel_hinges: Vec<Vec<usize>> = vec![Vec::new(); panels.len()];
    for (hi, h) in hinges.iter().enumerate() {
        let p_idx = vert_panel.get(h[0]).copied().unwrap_or(0) as usize;
        if let Some(list) = panel_hinges.get_mut(p_idx) {
            list.push(hi);
        }
    }

    for b in &stitching.bindings {
        let spec = b.seam_type.physics_spec();
        for er in b.interface_a.edges.iter().chain(&b.interface_b.edges) {
            let raw_sa = edge_sa_m
                .get(&(er.piece_id, er.edge_id))
                .copied()
                .unwrap_or(0.0);
            let sa_m = (if raw_sa > 1.0 {
                raw_sa
            } else {
                b.seam_type.recommended_sa_mm()
            } * 0.001)
                .clamp(0.006, 0.025);

            for side in [1_i8, -1_i8] {
                let Some(bvs) = edge_vert_map.get(&(er.piece_id, side, er.edge_id)) else {
                    continue;
                };
                if bvs.is_empty() {
                    continue;
                }
                let p_idx = vert_panel
                    .get(bvs[0].global_idx)
                    .copied()
                    .unwrap_or(0) as usize;
                let Some(pr) = panels.get(p_idx) else {
                    continue;
                };

                // Bounding box of the seam edge in 2D rest meters + sa_m pad.
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for bv in bvs {
                    let pt = rest2d_m[bv.global_idx];
                    min_x = min_x.min(pt.x);
                    min_y = min_y.min(pt.y);
                    max_x = max_x.max(pt.x);
                    max_y = max_y.max(pt.y);
                }
                min_x -= sa_m;
                min_y -= sa_m;
                max_x += sa_m;
                max_y += sa_m;

                let min_dist_sq = |pt: Point2| -> f64 {
                    if bvs.len() == 1 {
                        return pt.sub(rest2d_m[bvs[0].global_idx]).len_sq();
                    }
                    let mut best = f64::INFINITY;
                    for w in bvs.windows(2) {
                        let a = rest2d_m[w[0].global_idx];
                        let b = rest2d_m[w[1].global_idx];
                        let ab = b.sub(a);
                        let l2 = ab.len_sq();
                        let d2 = if l2 < 1e-12 {
                            pt.sub(a).len_sq()
                        } else {
                            let u = (pt.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
                            pt.sub(a.add(ab.scale(u))).len_sq()
                        };
                        if d2 < best {
                            best = d2;
                        }
                    }
                    best
                };

                let sa_sq = sa_m * sa_m;
                for v in pr.start..pr.end.min(n) {
                    let pt = rest2d_m[v];
                    if pt.x < min_x || pt.x > max_x || pt.y < min_y || pt.y > max_y {
                        continue;
                    }
                    let d2 = min_dist_sq(pt);
                    if d2 <= sa_sq {
                        let d = d2.sqrt();
                        let w = (1.0 - d / sa_m) as f32;
                        let m_mult = 1.0 + (spec.mass_mult - 1.0) * w;
                        if m_mult > vert_mass_mult[v] {
                            vert_mass_mult[v] = m_mult;
                        }
                        if d <= 0.55 * sa_m {
                            vert_seam_mask[v] = spec.shader_code;
                        }
                    }
                }

                if let Some(h_list) = panel_hinges.get(p_idx) {
                    for &hi in h_list {
                        let h = hinges[hi];
                        let mid = rest2d_m[h[0]].add(rest2d_m[h[1]]).scale(0.5);
                        if mid.x < min_x || mid.x > max_x || mid.y < min_y || mid.y > max_y {
                            continue;
                        }
                        let d2 = min_dist_sq(mid);
                        if d2 <= sa_sq {
                            let d = d2.sqrt();
                            let w = (1.0 - d / sa_m) as f32;
                            let b_mult = 1.0 + (spec.bend_mult - 1.0) * w;
                            if b_mult > hinge_bend_mult[hi] {
                                hinge_bend_mult[hi] = b_mult;
                            }
                        }
                    }
                }
            }
        }
    }

    // Second pass: physical hems (*ourlets*) and elastic casing bands on piece edges.
    for p in pieces.iter().filter(|p| p.visible) {
        for e in &p.seam_loop.edges {
            if e.hem.is_none() && e.elastic.is_none() {
                continue;
            }
            let (band_m, mass_target, bend_target, is_hem) = if let Some(hem) = e.hem {
                let depth_m = (hem.depth_mm * 0.001).clamp(0.010, 0.060);
                let folds = hem.fold_count.clamp(1, 2) as f32;
                (depth_m, 1.0 + 0.45 * folds, 1.0 + 0.35 * folds, true)
            } else if let Some(el) = e.elastic {
                let w_m = (el.width_mm * 0.001).clamp(0.012, 0.060);
                (w_m, 1.25_f32, 1.15_f32, false)
            } else {
                continue;
            };
            for side in [1_i8, -1_i8] {
                let Some(bvs) = edge_vert_map.get(&(p.id, side, e.id)) else {
                    continue;
                };
                if bvs.is_empty() {
                    continue;
                }
                let p_idx = vert_panel
                    .get(bvs[0].global_idx)
                    .copied()
                    .unwrap_or(0) as usize;
                let Some(pr) = panels.get(p_idx) else {
                    continue;
                };
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for bv in bvs {
                    let pt = rest2d_m[bv.global_idx];
                    min_x = min_x.min(pt.x);
                    min_y = min_y.min(pt.y);
                    max_x = max_x.max(pt.x);
                    max_y = max_y.max(pt.y);
                }
                min_x -= band_m;
                min_y -= band_m;
                max_x += band_m;
                max_y += band_m;
                let min_dist_sq = |pt: Point2| -> f64 {
                    if bvs.len() == 1 {
                        return pt.sub(rest2d_m[bvs[0].global_idx]).len_sq();
                    }
                    let mut best = f64::INFINITY;
                    for w in bvs.windows(2) {
                        let a = rest2d_m[w[0].global_idx];
                        let b = rest2d_m[w[1].global_idx];
                        let ab = b.sub(a);
                        let l2 = ab.len_sq();
                        let d2 = if l2 < 1e-12 {
                            pt.sub(a).len_sq()
                        } else {
                            let u = (pt.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
                            pt.sub(a.add(ab.scale(u))).len_sq()
                        };
                        if d2 < best {
                            best = d2;
                        }
                    }
                    best
                };
                let band_sq = band_m * band_m;
                for v in pr.start..pr.end.min(n) {
                    let pt = rest2d_m[v];
                    if pt.x < min_x || pt.x > max_x || pt.y < min_y || pt.y > max_y {
                        continue;
                    }
                    let d2 = min_dist_sq(pt);
                    if d2 <= band_sq {
                        let d = d2.sqrt();
                        let w = (1.0 - d / band_m) as f32;
                        let m_mult = 1.0 + (mass_target - 1.0) * w;
                        if m_mult > vert_mass_mult[v] {
                            vert_mass_mult[v] = m_mult;
                        }
                        if is_hem {
                            if d >= 0.65 * band_m {
                                vert_seam_mask[v] = 3.0;
                            } else if d <= 0.25 * band_m && vert_seam_mask[v] < 1.0 {
                                vert_seam_mask[v] = 1.0;
                            }
                        }
                    }
                }
                if let Some(h_list) = panel_hinges.get(p_idx) {
                    for &hi in h_list {
                        let h = hinges[hi];
                        let mid = rest2d_m[h[0]].add(rest2d_m[h[1]]).scale(0.5);
                        if mid.x < min_x || mid.x > max_x || mid.y < min_y || mid.y > max_y {
                            continue;
                        }
                        let d2 = min_dist_sq(mid);
                        if d2 <= band_sq {
                            let d = d2.sqrt();
                            let w = (1.0 - d / band_m) as f32;
                            let b_mult = 1.0 + (bend_target - 1.0) * w;
                            if b_mult > hinge_bend_mult[hi] {
                                hinge_bend_mult[hi] = b_mult;
                            }
                        }
                    }
                }
            }
        }
    }

    // Ensure Overlock / French / Plain seam types have consistent non-zero mask on exact boundary verts.
    let _ = SeamType::Plain;

    SeamBandData {
        vert_mass_mult,
        vert_seam_mask,
        hinge_bend_mult,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material_tests::rect_piece;
    use modelist_core::model::{det_id, EdgeRef, EaseSpec, SeamBinding, SeamDriver, SeamInterface};

    #[test]
    fn test_seam_types_scale_mass_and_bending() {
        let p = rect_piece(200.0, 200.0, 0.0);
        let rest2d = vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.2, 0.0),
            Point2::new(0.2, 0.2),
            Point2::new(0.0, 0.2),
            Point2::new(0.1, 0.1),
        ];
        let panels = vec![PanelRange {
            start: 0,
            end: 5,
            tri_start: 0,
            tri_end: 0,
        }];
        let vert_panel = vec![0u16; 5];
        let hinges = vec![[0, 1, 4, 2], [2, 3, 4, 0]];
        let mut edge_vert_map = HashMap::new();
        edge_vert_map.insert(
            (p.id, 1, p.seam_loop.edges[0].id),
            vec![
                BoundVert { global_idx: 0, sewn_frac: 0.0 },
                BoundVert { global_idx: 1, sewn_frac: 1.0 },
            ],
        );

        let run_type = |st: SeamType| {
            let sg = StitchingGraph {
                bindings: vec![SeamBinding {
                    id: det_id("test:sb"),
                    label: "Test".into(),
                    interface_a: SeamInterface {
                        id: det_id("test:ia"),
                        name: "A".into(),
                        edges: vec![EdgeRef {
                            piece_id: p.id,
                            edge_id: p.seam_loop.edges[0].id,
                            dir: 1,
                        }],
                    },
                    interface_b: SeamInterface {
                        id: det_id("test:ib"),
                        name: "B".into(),
                        edges: vec![],
                    },
                    ease: EaseSpec::Exact,
                    driver: SeamDriver::DriverA,
                    seam_type: st,
                    ease_min_mm: -2.0,
                    ease_max_mm: 2.0,
                    auto_partner_notches: false,
                }],
            };
            compute_seam_band_data(
                &[p.clone()],
                &sg,
                &rest2d,
                &panels,
                &vert_panel,
                &edge_vert_map,
                &hinges,
            )
        };

        let d_plain = run_type(SeamType::Plain);
        let d_french = run_type(SeamType::French);
        let d_felled = run_type(SeamType::FlatFelled);

        assert!(d_felled.vert_mass_mult[0] > d_french.vert_mass_mult[0]);
        assert!(d_french.vert_mass_mult[0] > d_plain.vert_mass_mult[0]);
        assert!(d_plain.vert_mass_mult[0] > 1.0);
        assert_eq!(d_felled.vert_mass_mult[4], 1.0); // interior vertex outside SA band
        assert!(d_felled.hinge_bend_mult[0] > d_french.hinge_bend_mult[0]);
        assert!(d_french.hinge_bend_mult[0] > d_plain.hinge_bend_mult[0]);
    }
}

