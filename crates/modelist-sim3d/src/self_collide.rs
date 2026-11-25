//! Spatial-hash cloth-to-cloth self-collision and garment layer ordering.
//!
//! Uses a reusable flat 3D uniform grid over the garment's bounding box (zero per-frame
//! allocations after warm-up) to detect vertex-vs-triangle proximities within `2 * thickness`
//! and resolve them with mass-weighted positional pushes and velocity-level Coulomb friction.

use crate::avatar::Vec3;
use crate::xpbd::BarySew;
use std::collections::HashSet;

/// Cell size for the cloth spatial hash, m.
const CELL_SIZE: f32 = 0.018;
/// Maximum positional separation per substep, m (prevents explosive impulses).
const MAX_SELF_PUSH: f32 = 0.0016;
/// Maximum grid dimension per axis so an exploded outlier vertex cannot OOM.
const MAX_GRID_DIM: usize = 160;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelfContact {
    pub vert: usize,
    pub tri: [usize; 3],
    pub bary: [f32; 3],
    pub normal: Vec3,
    pub penetration: f32,
}

/// Reusable uniform 3D grid indexing cloth triangles.
pub(crate) struct ClothSpatialHash {
    origin: Vec3,
    h: f32,
    dims: [usize; 3],
    cell_start: Vec<u32>,
    items: Vec<u32>,
    tri_lo_hi: Vec<(Vec3, Vec3)>,
    sew_pairs: HashSet<(usize, usize)>,
    sewn_panel_pairs: HashSet<(u16, u16)>,
    is_seam_vert: Vec<bool>,
    vert_panel: Vec<u16>,
    vert_layer: Vec<u8>,
}

impl ClothSpatialHash {
    pub fn new(
        vert_panel: Vec<u16>,
        vert_layer: Vec<u8>,
        sew: &[BarySew],
        seam_welds: &[Vec<usize>],
        tris: &[[usize; 3]],
    ) -> Self {
        let mut sew_pairs = HashSet::with_capacity(sew.len() * 2 + seam_welds.len() * 2);
        let mut sewn_panel_pairs = HashSet::new();
        let mut is_seam_vert = vec![false; vert_panel.len()];
        let mut add_pair = |u: usize, v: usize| {
            if u != v {
                let key = if u < v { (u, v) } else { (v, u) };
                sew_pairs.insert(key);
                let pu = vert_panel.get(u).copied().unwrap_or(0);
                let pv = vert_panel.get(v).copied().unwrap_or(0);
                if pu != pv {
                    sewn_panel_pairs.insert(if pu < pv { (pu, pv) } else { (pv, pu) });
                }
            }
        };
        for s in sew {
            add_pair(s.a, s.b0);
            add_pair(s.a, s.b1);
            if s.a < is_seam_vert.len() {
                is_seam_vert[s.a] = true;
            }
            if s.b0 < is_seam_vert.len() {
                is_seam_vert[s.b0] = true;
            }
            if s.b1 < is_seam_vert.len() {
                is_seam_vert[s.b1] = true;
            }
        }
        for cluster in seam_welds {
            for i in 0..cluster.len() {
                if cluster[i] < is_seam_vert.len() {
                    is_seam_vert[cluster[i]] = true;
                }
                for j in (i + 1)..cluster.len() {
                    add_pair(cluster[i], cluster[j]);
                }
            }
        }
        let _ = tris;
        Self {
            origin: Vec3::ZERO,
            h: CELL_SIZE,
            dims: [1, 1, 1],
            cell_start: vec![0, 0],
            items: Vec::new(),
            tri_lo_hi: Vec::new(),
            sew_pairs,
            sewn_panel_pairs,
            is_seam_vert,
            vert_panel,
            vert_layer,
        }
    }

    #[inline]
    fn clamp_cell(&self, v: f32, axis: usize) -> usize {
        if v <= 0.0 {
            0
        } else {
            (v as usize).min(self.dims[axis] - 1)
        }
    }

    /// Rebuilds the spatial grid for `tris` at `pos` with padding `pad` (m).
    pub fn rebuild(&mut self, pos: &[Vec3], tris: &[[usize; 3]], pad: f32) {
        self.tri_lo_hi.clear();
        if tris.is_empty() || pos.is_empty() {
            self.dims = [1, 1, 1];
            self.cell_start.clear();
            self.cell_start.resize(2, 0);
            self.items.clear();
            return;
        }
        let mut lo = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut hi = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        self.tri_lo_hi.reserve(tris.len());
        for t in tris {
            let (a, b, c) = (pos[t[0]], pos[t[1]], pos[t[2]]);
            let tlo = Vec3::new(
                a.x.min(b.x).min(c.x) - pad,
                a.y.min(b.y).min(c.y) - pad,
                a.z.min(b.z).min(c.z) - pad,
            );
            let thi = Vec3::new(
                a.x.max(b.x).max(c.x) + pad,
                a.y.max(b.y).max(c.y) + pad,
                a.z.max(b.z).max(c.z) + pad,
            );
            lo = Vec3::new(lo.x.min(tlo.x), lo.y.min(tlo.y), lo.z.min(tlo.z));
            hi = Vec3::new(hi.x.max(thi.x), hi.y.max(thi.y), hi.z.max(thi.z));
            self.tri_lo_hi.push((tlo, thi));
        }

        let span = hi.sub(lo);
        let min_h = (span.x.max(span.y).max(span.z) / MAX_GRID_DIM as f32).max(CELL_SIZE);
        self.h = min_h;
        self.origin = lo.sub(Vec3::new(self.h, self.h, self.h));
        self.dims = [
            (((hi.x - self.origin.x) / self.h).ceil() as usize + 2).clamp(1, MAX_GRID_DIM),
            (((hi.y - self.origin.y) / self.h).ceil() as usize + 2).clamp(1, MAX_GRID_DIM),
            (((hi.z - self.origin.z) / self.h).ceil() as usize + 2).clamp(1, MAX_GRID_DIM),
        ];
        let ncell = self.dims[0] * self.dims[1] * self.dims[2];
        self.cell_start.clear();
        self.cell_start.resize(ncell + 1, 0);

        for &(tlo, thi) in &self.tri_lo_hi {
            let x0 = self.clamp_cell((tlo.x - self.origin.x) / self.h, 0);
            let x1 = self.clamp_cell((thi.x - self.origin.x) / self.h, 0);
            let y0 = self.clamp_cell((tlo.y - self.origin.y) / self.h, 1);
            let y1 = self.clamp_cell((thi.y - self.origin.y) / self.h, 1);
            let z0 = self.clamp_cell((tlo.z - self.origin.z) / self.h, 2);
            let z1 = self.clamp_cell((thi.z - self.origin.z) / self.h, 2);
            for z in z0..=z1 {
                for y in y0..=y1 {
                    let row = (z * self.dims[1] + y) * self.dims[0];
                    for x in x0..=x1 {
                        self.cell_start[row + x + 1] += 1;
                    }
                }
            }
        }
        for c in 1..=ncell {
            self.cell_start[c] += self.cell_start[c - 1];
        }
        let total = self.cell_start[ncell] as usize;
        self.items.clear();
        self.items.resize(total, 0);
        let mut fill = self.cell_start.clone();
        for (ti, &(tlo, thi)) in self.tri_lo_hi.iter().enumerate() {
            let x0 = self.clamp_cell((tlo.x - self.origin.x) / self.h, 0);
            let x1 = self.clamp_cell((thi.x - self.origin.x) / self.h, 0);
            let y0 = self.clamp_cell((tlo.y - self.origin.y) / self.h, 1);
            let y1 = self.clamp_cell((thi.y - self.origin.y) / self.h, 1);
            let z0 = self.clamp_cell((tlo.z - self.origin.z) / self.h, 2);
            let z1 = self.clamp_cell((thi.z - self.origin.z) / self.h, 2);
            for z in z0..=z1 {
                for y in y0..=y1 {
                    let row = (z * self.dims[1] + y) * self.dims[0];
                    for x in x0..=x1 {
                        let idx = row + x;
                        self.items[fill[idx] as usize] = ti as u32;
                        fill[idx] += 1;
                    }
                }
            }
        }
    }

    #[inline]
    fn is_sew_connected(&self, a: usize, b: usize) -> bool {
        let key = if a < b { (a, b) } else { (b, a) };
        self.sew_pairs.contains(&key)
    }

    /// Detects vertex-vs-triangle self-contacts within `thickness` (m).
    pub fn detect(
        &self,
        pos: &[Vec3],
        tris: &[[usize; 3]],
        tri_layer: &[u8],
        thickness: f32,
        same_garment_only: bool,
        out: &mut Vec<SelfContact>,
    ) {
        out.clear();
        if tris.is_empty() || self.items.is_empty() {
            return;
        }
        let thick_sq = thickness * thickness;
        let mut seen_tris: Vec<u32> = Vec::with_capacity(16);

        for (v, &p) in pos.iter().enumerate() {
            let cx = self.clamp_cell((p.x - self.origin.x) / self.h, 0);
            let cy = self.clamp_cell((p.y - self.origin.y) / self.h, 1);
            let cz = self.clamp_cell((p.z - self.origin.z) / self.h, 2);
            let cell = (cz * self.dims[1] + cy) * self.dims[0] + cx;
            let a = self.cell_start[cell] as usize;
            let b = self.cell_start[cell + 1] as usize;
            if a >= b {
                continue;
            }
            seen_tris.clear();
            let v_panel = self.vert_panel.get(v).copied().unwrap_or(0);

            for &ti in &self.items[a..b] {
                if seen_tris.contains(&ti) {
                    continue;
                }
                seen_tris.push(ti);

                let (tlo, thi) = self.tri_lo_hi[ti as usize];
                if p.x < tlo.x
                    || p.x > thi.x
                    || p.y < tlo.y
                    || p.y > thi.y
                    || p.z < tlo.z
                    || p.z > thi.z
                {
                    continue;
                }
                let tri = tris[ti as usize];
                let v_layer = self.vert_layer.get(v).copied().unwrap_or(1);
                let t_layer = tri_layer.get(ti as usize).copied().unwrap_or(1);
                let t_panel = self.vert_panel.get(tri[0]).copied().unwrap_or(0);
                if same_garment_only && (t_panel == v_panel || (v_layer / 4) != (t_layer / 4)) {
                    continue;
                }
                if tri[0] == v || tri[1] == v || tri[2] == v {
                    continue;
                }
                if self.is_sew_connected(v, tri[0])
                    || self.is_sew_connected(v, tri[1])
                    || self.is_sew_connected(v, tri[2])
                {
                    continue;
                }

                let (pa, pb, pc) = (pos[tri[0]], pos[tri[1]], pos[tri[2]]);
                let (closest, bary) = closest_point_on_triangle(p, pa, pb, pc);
                let diff = p.sub(closest);
                let d2 = diff.len_sq();
                if d2 >= thick_sq {
                    continue;
                }

                let dist = d2.sqrt();
                let tri_n = pb.sub(pa).cross(pc.sub(pa)).normalized();
                // Same-panel vertices closer than 1.5*thickness in 2D rest space are neighbors; skip
                // unless they genuinely folded across opposite sides.
                // On the same panel, skip local neighborhood vertices so curved hems/darts
                // never push against their own adjacent triangles, and skip tangential grazing.
                if t_panel == v_panel {
                    let ring_thresh = (thickness * 4.0) * (thickness * 4.0);
                    if p.sub(pa).len_sq() < ring_thresh
                        || p.sub(pb).len_sq() < ring_thresh
                        || p.sub(pc).len_sq() < ring_thresh
                    {
                        continue;
                    }
                    if dist > 1e-6 && diff.dot(tri_n).abs() < 0.25 * dist {
                        continue;
                    }
                } else {
                    let pair = if v_panel < t_panel {
                        (v_panel, t_panel)
                    } else {
                        (t_panel, v_panel)
                    };
                    if self.sewn_panel_pairs.contains(&pair)
                        && self.is_seam_vert.get(v).copied().unwrap_or(false)
                        && (self.is_seam_vert.get(tri[0]).copied().unwrap_or(false)
                            || self.is_seam_vert.get(tri[1]).copied().unwrap_or(false)
                            || self.is_seam_vert.get(tri[2]).copied().unwrap_or(false))
                    {
                        let seam_thresh = (thickness * 3.5) * (thickness * 3.5);
                        if p.sub(pa).len_sq() < seam_thresh
                            || p.sub(pb).len_sq() < seam_thresh
                            || p.sub(pc).len_sq() < seam_thresh
                        {
                            continue;
                        }
                    }
                }
                let mut normal = if t_panel != v_panel && v_layer != t_layer {
                    // Enforce one-sided layer separation: outer layers stay on +tri_n, inner on -tri_n.
                    if v_layer > t_layer {
                        tri_n
                    } else {
                        tri_n.scale(-1.0)
                    }
                } else if dist > 1e-6 {
                    diff.scale(1.0 / dist)
                } else {
                    tri_n
                };
                if normal.len_sq() < 0.5 {
                    continue;
                }
                if normal.dot(tri_n) < 0.0 && dist < 1e-4 && v_layer == t_layer {
                    normal = tri_n;
                }
                let pen = if t_panel != v_panel && v_layer != t_layer {
                    let signed = p.sub(closest).dot(normal);
                    if signed >= thickness {
                        continue;
                    }
                    thickness - signed
                } else {
                    thickness - dist
                };
                out.push(SelfContact {
                    vert: v,
                    tri,
                    bary,
                    normal,
                    penetration: pen,
                });
            }
        }
    }
}

/// Resolves detected cloth-to-cloth self-contacts with mass-weighted positional pushes and
/// velocity-level Coulomb friction.
pub(crate) fn resolve_self_contacts(
    pos: &mut [Vec3],
    prev: &mut [Vec3],
    inv_mass: &[f32],
    contacts: &[SelfContact],
    friction: f32,
) {
    for c in contacts {
        let [i0, i1, i2] = c.tri;
        let wv = inv_mass[c.vert];
        let w0 = inv_mass[i0] * c.bary[0];
        let w1 = inv_mass[i1] * c.bary[1];
        let w2 = inv_mass[i2] * c.bary[2];
        let wsum = wv + c.bary[0] * w0 + c.bary[1] * w1 + c.bary[2] * w2;
        if wsum <= 1e-12 {
            continue;
        }
        let push = (c.penetration * 0.65).min(MAX_SELF_PUSH);
        let corr = c.normal.scale(push / wsum);
        let dv = corr.scale(wv);
        let d0 = corr.scale(w0);
        let d1 = corr.scale(w1);
        let d2 = corr.scale(w2);
        pos[c.vert] = pos[c.vert].add(dv);
        pos[i0] = pos[i0].sub(d0);
        pos[i1] = pos[i1].sub(d1);
        pos[i2] = pos[i2].sub(d2);
        // Inelastic resolution: shift `prev` with `pos` so resolving overlap never injects bounce velocity.
        prev[c.vert] = prev[c.vert].add(dv);
        prev[i0] = prev[i0].sub(d0);
        prev[i1] = prev[i1].sub(d1);
        prev[i2] = prev[i2].sub(d2);

        if friction > 0.0 {
            let v_vert = pos[c.vert].sub(prev[c.vert]);
            let v_tri = pos[i0]
                .sub(prev[i0])
                .scale(c.bary[0])
                .add(pos[i1].sub(prev[i1]).scale(c.bary[1]))
                .add(pos[i2].sub(prev[i2]).scale(c.bary[2]));
            let v_rel = v_vert.sub(v_tri);
            let vn = v_rel.dot(c.normal);
            let vt = v_rel.sub(c.normal.scale(vn));
            let lt = vt.len();
            if lt > 1e-7 {
                let damp = (friction * push / lt).min(0.6);
                prev[c.vert] = prev[c.vert].add(vt.scale(damp * (wv / wsum)));
            }
        }
    }
}

/// Exact Voronoi-region closest point on triangle `(a, b, c)` to `p`, returning `(point, bary)`.
pub(crate) fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (Vec3, [f32; 3]) {
    let ab = b.sub(a);
    let ac = c.sub(a);
    let ap = p.sub(a);

    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, [1.0, 0.0, 0.0]);
    }

    let bp = p.sub(b);
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, [0.0, 1.0, 0.0]);
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3).max(1e-12);
        return (a.add(ab.scale(v)), [1.0 - v, v, 0.0]);
    }

    let cp = p.sub(c);
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, [0.0, 0.0, 1.0]);
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6).max(1e-12);
        return (a.add(ac.scale(w)), [1.0 - w, 0.0, w]);
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6)).max(1e-12);
        return (b.add(c.sub(b).scale(w)), [0.0, 1.0 - w, w]);
    }

    let denom = 1.0 / (va + vb + vc).max(1e-12);
    let v = vb * denom;
    let w = vc * denom;
    let u = 1.0 - v - w;
    (a.add(ab.scale(v)).add(ac.scale(w)), [u, v, w])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_collision_separates_overlapping_sheets() {
        // Sheet 0 (panel 0) at z = 0.0005, Sheet 1 (panel 1) at z = 0.0 (within thickness 0.004 m)
        let mut pos = vec![
            Vec3::new(0.0, 0.0, 0.0005),
            Vec3::new(0.05, 0.0, 0.0005),
            Vec3::new(0.025, 0.05, 0.0005),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.05, 0.0, 0.0),
            Vec3::new(0.025, 0.05, 0.0),
        ];
        let mut prev = pos.clone();
        let inv_mass = vec![1.0; 6];
        let tris = vec![[0, 1, 2], [3, 4, 5]];
        let tri_layer = vec![1, 1];
        let mut hash = ClothSpatialHash::new(vec![0, 0, 0, 1, 1, 1], vec![1, 1, 1, 1, 1, 1], &[], &[], &tris);
        let mut contacts = Vec::new();
        let thickness = 0.004_f32;
        for _ in 0..8 {
            hash.rebuild(&pos, &tris, thickness + 0.002);
            hash.detect(&pos, &tris, &tri_layer, thickness, false, &mut contacts);
            resolve_self_contacts(&mut pos, &mut prev, &inv_mass, &contacts, 0.2);
        }
        let dz = (pos[0].z - pos[3].z).abs();
        assert!(
            dz >= thickness - 0.0005,
            "Expected overlapping sheets to separate to >= {thickness} m, got {dz:.5} m"
        );
    }

    #[test]
    fn test_layer_order_keeps_outer_outside_inner() {
        // Outer layer (layer 2) starts 1 mm *behind* inner layer (layer 0), with tri_n = +Z.
        let mut pos = vec![
            Vec3::new(0.025, 0.025, -0.001), // outer vertex behind inner tri
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.05, 0.0, 0.0),
            Vec3::new(0.025, 0.05, 0.0),
        ];
        let mut prev = pos.clone();
        let inv_mass = vec![1.0, 0.0, 0.0, 0.0];
        let tris = vec![[1, 2, 3]];
        let tri_layer = vec![0];
        let mut hash = ClothSpatialHash::new(vec![0, 1, 1, 1], vec![2, 0, 0, 0], &[], &[], &tris);
        let mut contacts = Vec::new();
        let thickness = 0.003_f32;
        for _ in 0..6 {
            hash.rebuild(&pos, &tris, thickness + 0.002);
            hash.detect(&pos, &tris, &tri_layer, thickness, false, &mut contacts);
            resolve_self_contacts(&mut pos, &mut prev, &inv_mass, &contacts, 0.1);
        }
        assert!(
            pos[0].z >= thickness - 0.0005,
            "Expected outer layer to separate onto +Z side, got z={:.5}",
            pos[0].z
        );
    }
}

