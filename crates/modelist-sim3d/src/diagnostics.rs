//! Drape-quality diagnostics for the 3D fit room.
//!
//! Seam gaps and strain alone cannot tell a clean drape from a crumpled one, so these metrics
//! measure what a designer sees: body features poking through the garment, crumpled/creased
//! cloth, and cloth passing through itself. They work on plain buffers so tests, benches and
//! the WASM stats can share them.

use crate::avatar::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct DrapeQuality {
    /// Avatar vertices that stick out through the cloth (cloth found behind the skin surface).
    pub poke_through_count: usize,
    /// Deepest poke-through, in millimetres.
    pub max_poke_through_mm: f32,
    /// Share of interior hinges whose dihedral angle exceeds 60 degrees (crumpling), in percent.
    pub sharp_crease_pct: f32,
    /// Mean dihedral angle between neighbouring triangles, in degrees.
    pub mean_dihedral_deg: f32,
    /// Non-adjacent cloth triangle pairs that intersect each other.
    pub self_intersections: usize,
}

type Cell = (i32, i32, i32);

#[inline]
fn v3(buf: &[f32], i: usize) -> Vec3 {
    Vec3::new(buf[i * 3], buf[i * 3 + 1], buf[i * 3 + 2])
}

#[inline]
fn cell_of(p: Vec3, h: f32) -> Cell {
    (
        (p.x / h).floor() as i32,
        (p.y / h).floor() as i32,
        (p.z / h).floor() as i32,
    )
}

fn tri_hash(pos: &[f32], tris: &[u32], h: f32) -> HashMap<Cell, Vec<u32>> {
    let mut grid: HashMap<Cell, Vec<u32>> = HashMap::new();
    for (t, tri) in tris.chunks_exact(3).enumerate() {
        let a = v3(pos, tri[0] as usize);
        let b = v3(pos, tri[1] as usize);
        let c = v3(pos, tri[2] as usize);
        let lo = cell_of(
            Vec3::new(a.x.min(b.x).min(c.x), a.y.min(b.y).min(c.y), a.z.min(b.z).min(c.z)),
            h,
        );
        let hi = cell_of(
            Vec3::new(a.x.max(b.x).max(c.x), a.y.max(b.y).max(c.y), a.z.max(b.z).max(c.z)),
            h,
        );
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    grid.entry((x, y, z)).or_default().push(t as u32);
                }
            }
        }
    }
    grid
}

/// Moller-Trumbore segment/triangle test. Returns the segment parameter in `[0, 1]` on a hit.
pub(crate) fn segment_triangle(p0: Vec3, p1: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let dir = p1.sub(p0);
    let e1 = b.sub(a);
    let e2 = c.sub(a);
    let pv = dir.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = p0.sub(a);
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = dir.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(qv) * inv;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// Computes drape-quality metrics from flat render buffers (`xyz` triples, metres).
pub fn drape_quality(
    cloth_pos: &[f32],
    cloth_tris: &[u32],
    avatar_pos: &[f32],
    avatar_nrm: &[f32],
) -> DrapeQuality {
    let mut q = DrapeQuality::default();
    if cloth_tris.len() < 3 {
        return q;
    }
    let grid_h = 0.025_f32;
    let grid = tri_hash(cloth_pos, cloth_tris, grid_h);

    // 1. Poke-through: march a short ray from each skin vertex inwards. Any cloth found there lies
    //    inside the body, so that skin point is visible through the garment.
    let probe_depth = 0.020_f32;
    let n_av = avatar_pos.len() / 3;
    for vi in 0..n_av {
        let p = v3(avatar_pos, vi);
        let n = v3(avatar_nrm, vi);
        if n.len_sq() < 0.5 {
            continue;
        }
        let inner = p.sub(n.scale(probe_depth));
        let lo = cell_of(
            Vec3::new(p.x.min(inner.x), p.y.min(inner.y), p.z.min(inner.z)),
            grid_h,
        );
        let hi = cell_of(
            Vec3::new(p.x.max(inner.x), p.y.max(inner.y), p.z.max(inner.z)),
            grid_h,
        );
        let mut deepest = 0.0_f32;
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    let Some(list) = grid.get(&(x, y, z)) else {
                        continue;
                    };
                    for &t in list {
                        let tri = &cloth_tris[t as usize * 3..t as usize * 3 + 3];
                        let a = v3(cloth_pos, tri[0] as usize);
                        let b = v3(cloth_pos, tri[1] as usize);
                        let c = v3(cloth_pos, tri[2] as usize);
                        if let Some(s) = segment_triangle(p, inner, a, b, c) {
                            let depth = s * probe_depth;
                            if depth > 0.0005 && depth > deepest {
                                deepest = depth;
                            }
                        }
                    }
                }
            }
        }
        if deepest > 0.0 {
            q.poke_through_count += 1;
            q.max_poke_through_mm = q.max_poke_through_mm.max(deepest * 1000.0);
        }
    }

    // 2. Crease statistics over interior hinges (same-panel triangles sharing an edge).
    let tri_count = cloth_tris.len() / 3;
    let mut normals = Vec::with_capacity(tri_count);
    for tri in cloth_tris.chunks_exact(3) {
        let a = v3(cloth_pos, tri[0] as usize);
        let b = v3(cloth_pos, tri[1] as usize);
        let c = v3(cloth_pos, tri[2] as usize);
        normals.push(b.sub(a).cross(c.sub(a)).normalized());
    }
    let mut edge_tris: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for (t, tri) in cloth_tris.chunks_exact(3).enumerate() {
        for (u, v) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let key = if u < v { (u, v) } else { (v, u) };
            edge_tris.entry(key).or_default().push(t as u32);
        }
    }
    let mut hinge_count = 0usize;
    let mut sharp = 0usize;
    let mut angle_sum = 0.0_f64;
    for tris in edge_tris.values() {
        if tris.len() != 2 {
            continue;
        }
        let n0 = normals[tris[0] as usize];
        let n1 = normals[tris[1] as usize];
        if n0.len_sq() < 0.5 || n1.len_sq() < 0.5 {
            continue;
        }
        let ang = n0.dot(n1).clamp(-1.0, 1.0).acos().to_degrees();
        hinge_count += 1;
        angle_sum += ang as f64;
        if ang > 60.0 {
            sharp += 1;
        }
    }
    if hinge_count > 0 {
        q.sharp_crease_pct = 100.0 * sharp as f32 / hinge_count as f32;
        q.mean_dihedral_deg = (angle_sum / hinge_count as f64) as f32;
    }

    // 3. Self-intersections between non-adjacent triangles.
    q.self_intersections = intersecting_pairs_in(cloth_pos, cloth_tris, &grid).len();
    q
}

/// Pairs of non-adjacent cloth triangles that intersect. Pairs whose corners nearly coincide are
/// skipped: those are sewn seam neighbours from different panels.
pub fn intersecting_pairs(cloth_pos: &[f32], cloth_tris: &[u32]) -> Vec<(u32, u32)> {
    if cloth_tris.len() < 3 {
        return Vec::new();
    }
    let grid = tri_hash(cloth_pos, cloth_tris, 0.025);
    intersecting_pairs_in(cloth_pos, cloth_tris, &grid)
}

fn intersecting_pairs_in(cloth_pos: &[f32], cloth_tris: &[u32], grid: &HashMap<Cell, Vec<u32>>) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    for list in grid.values() {
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                let (ta, tb) = if list[i] < list[j] { (list[i], list[j]) } else { (list[j], list[i]) };
                if !seen.insert((ta, tb)) {
                    continue;
                }
                let ia = &cloth_tris[ta as usize * 3..ta as usize * 3 + 3];
                let ib = &cloth_tris[tb as usize * 3..tb as usize * 3 + 3];
                if ia.iter().any(|v| ib.contains(v)) {
                    continue;
                }
                let pa = [v3(cloth_pos, ia[0] as usize), v3(cloth_pos, ia[1] as usize), v3(cloth_pos, ia[2] as usize)];
                let pb = [v3(cloth_pos, ib[0] as usize), v3(cloth_pos, ib[1] as usize), v3(cloth_pos, ib[2] as usize)];
                let near_seam = pa.iter().any(|x| pb.iter().any(|y| x.sub(*y).len_sq() < 0.003 * 0.003));
                if near_seam {
                    continue;
                }
                if triangles_intersect(&pa, &pb) {
                    out.push((ta, tb));
                }
            }
        }
    }
    out
}

fn triangles_intersect(a: &[Vec3; 3], b: &[Vec3; 3]) -> bool {
    for k in 0..3 {
        if segment_triangle(a[k], a[(k + 1) % 3], b[0], b[1], b[2]).is_some() {
            return true;
        }
        if segment_triangle(b[k], b[(k + 1) % 3], a[0], a[1], a[2]).is_some() {
            return true;
        }
    }
    false
}
