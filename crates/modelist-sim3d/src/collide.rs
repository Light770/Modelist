//! Cloth-versus-body contact helpers.
//!
//! Vertex-vs-SDF collision keeps every cloth vertex outside the body, but a 20 mm cloth triangle
//! can still be pierced *between* its vertices by small features (nipples, knuckles, the acromion,
//! hip bones). [`BodyPointGrid`] stores the static avatar surface points in a dense grid so each
//! near-body cloth triangle can be pushed off any skin point that pokes through it.

use crate::avatar::{AvatarMesh, Vec3};

/// Deepest a body point may sit behind a cloth triangle and still be resolved, m. Deeper hits
/// belong to cloth on the far side of a limb and are left to the SDF test.
const MAX_DEPTH: f32 = 0.012;
/// Largest push applied to a triangle per call, m (deep overlaps resolve over a few substeps).
const MAX_PUSH: f32 = 0.002;

pub(crate) struct BodyPointGrid {
    origin: Vec3,
    h: f32,
    dims: [usize; 3],
    cell_start: Vec<u32>,
    items: Vec<u32>,
    pos: Vec<Vec3>,
    nrm: Vec<Vec3>,
}

impl BodyPointGrid {
    pub fn build(mesh: &AvatarMesh, h: f32) -> Self {
        let n = mesh.positions.len() / 3;
        let mut pos = Vec::with_capacity(n);
        let mut nrm = Vec::with_capacity(n);
        let mut lo = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut hi = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for i in 0..n {
            let p = Vec3::new(
                mesh.positions[i * 3],
                mesh.positions[i * 3 + 1],
                mesh.positions[i * 3 + 2],
            );
            let q = Vec3::new(mesh.normals[i * 3], mesh.normals[i * 3 + 1], mesh.normals[i * 3 + 2])
                .normalized();
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
            pos.push(p);
            nrm.push(q);
        }
        if n == 0 {
            return Self {
                origin: Vec3::ZERO,
                h,
                dims: [1, 1, 1],
                cell_start: vec![0, 0],
                items: Vec::new(),
                pos,
                nrm,
            };
        }
        let origin = lo.sub(Vec3::new(h, h, h));
        let dims = [
            (((hi.x - origin.x) / h).ceil() as usize + 2).max(1),
            (((hi.y - origin.y) / h).ceil() as usize + 2).max(1),
            (((hi.z - origin.z) / h).ceil() as usize + 2).max(1),
        ];
        let ncell = dims[0] * dims[1] * dims[2];
        let cell_of = |p: Vec3| -> usize {
            let cx = (((p.x - origin.x) / h) as usize).min(dims[0] - 1);
            let cy = (((p.y - origin.y) / h) as usize).min(dims[1] - 1);
            let cz = (((p.z - origin.z) / h) as usize).min(dims[2] - 1);
            (cz * dims[1] + cy) * dims[0] + cx
        };
        let mut counts = vec![0u32; ncell + 1];
        for p in &pos {
            counts[cell_of(*p) + 1] += 1;
        }
        for c in 1..=ncell {
            counts[c] += counts[c - 1];
        }
        let mut fill = counts.clone();
        let mut items = vec![0u32; n];
        for (i, p) in pos.iter().enumerate() {
            let c = cell_of(*p);
            items[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        Self {
            origin,
            h,
            dims,
            cell_start: counts,
            items,
            pos,
            nrm,
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

    /// Collects the body points inside the axis-aligned box `[lo, hi]` into `out`.
    pub fn query(&self, lo: Vec3, hi: Vec3, out: &mut Vec<u32>) {
        out.clear();
        if self.items.is_empty() {
            return;
        }
        let x0 = self.clamp_cell((lo.x - self.origin.x) / self.h, 0);
        let x1 = self.clamp_cell((hi.x - self.origin.x) / self.h, 0);
        let y0 = self.clamp_cell((lo.y - self.origin.y) / self.h, 1);
        let y1 = self.clamp_cell((hi.y - self.origin.y) / self.h, 1);
        let z0 = self.clamp_cell((lo.z - self.origin.z) / self.h, 2);
        let z1 = self.clamp_cell((hi.z - self.origin.z) / self.h, 2);
        for z in z0..=z1 {
            for y in y0..=y1 {
                let row = (z * self.dims[1] + y) * self.dims[0];
                let a = self.cell_start[row + x0] as usize;
                let b = self.cell_start[row + x1 + 1] as usize;
                for &pi in &self.items[a..b] {
                    let p = self.pos[pi as usize];
                    if p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y && p.z >= lo.z && p.z <= hi.z {
                        out.push(pi);
                    }
                }
            }
        }
    }

    #[inline]
    pub fn point(&self, i: u32) -> (Vec3, Vec3) {
        (self.pos[i as usize], self.nrm[i as usize])
    }
}

/// Pushes one cloth triangle off every body point that pokes through it (or sits closer than
/// `offset` in front of it). The correction moves the triangle along the skin normal, weighted
/// by barycentric coordinates and inverse masses. Returns the number of contacts resolved.
pub(crate) fn push_triangle_off_points(
    grid: &BodyPointGrid,
    pos: &mut [Vec3],
    prev: &mut [Vec3],
    inv_mass: &[f32],
    tri: [usize; 3],
    offset: f32,
    scratch: &mut Vec<u32>,
) -> usize {
    let [i0, i1, i2] = tri;
    let (a, b, c) = (pos[i0], pos[i1], pos[i2]);
    let pad = offset + 0.001;
    let lo = Vec3::new(
        a.x.min(b.x).min(c.x) - pad,
        a.y.min(b.y).min(c.y) - pad,
        a.z.min(b.z).min(c.z) - pad,
    );
    let hi = Vec3::new(
        a.x.max(b.x).max(c.x) + pad,
        a.y.max(b.y).max(c.y) + pad,
        a.z.max(b.z).max(c.z) + pad,
    );
    grid.query(lo, hi, scratch);
    let mut hits = 0usize;
    for &pt in scratch.iter() {
        let (p, n) = grid.point(pt);
        let (x0, x1, x2) = (pos[i0], pos[i1], pos[i2]);
        let e1 = x1.sub(x0);
        let e2 = x2.sub(x0);
        let nt = e1.cross(e2);
        let nl = nt.len();
        if nl < 1e-12 {
            break;
        }
        let denom = nt.dot(n) / nl;
        if denom.abs() < 0.3 {
            continue; // grazing: the skin point is beside the triangle, not under it
        }
        let t = nt.dot(x0.sub(p)) / (nl * denom);
        if t >= offset || t < -MAX_DEPTH {
            continue;
        }
        let q = p.add(n.scale(t));
        let v2 = q.sub(x0);
        let d00 = e1.dot(e1);
        let d01 = e1.dot(e2);
        let d11 = e2.dot(e2);
        let d20 = v2.dot(e1);
        let d21 = v2.dot(e2);
        let den = d00 * d11 - d01 * d01;
        if den < 1e-16 {
            continue;
        }
        let b1 = (d11 * d20 - d01 * d21) / den;
        let b2 = (d00 * d21 - d01 * d20) / den;
        let b0 = 1.0 - b1 - b2;
        if b0 < -0.02 || b1 < -0.02 || b2 < -0.02 {
            continue;
        }
        let (b0, b1, b2) = (b0.max(0.0), b1.max(0.0), b2.max(0.0));
        let (w0, w1, w2) = (inv_mass[i0], inv_mass[i1], inv_mass[i2]);
        let wsum = w0 * b0 * b0 + w1 * b1 * b1 + w2 * b2 * b2;
        if wsum < 1e-12 {
            continue;
        }
        // Resolve gradually and without adding velocity (prev moves with pos).
        let s = (offset - t).min(MAX_PUSH) / wsum;
        for (vi, w, b) in [(i0, w0, b0), (i1, w1, b1), (i2, w2, b2)] {
            let delta = n.scale(w * b * s);
            pos[vi] = pos[vi].add(delta);
            prev[vi] = prev[vi].add(delta);
        }
        hits += 1;
    }
    hits
}
