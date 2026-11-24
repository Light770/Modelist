//! SIGGRAPH 2024 Vertex Block Descent (VBD) cloth solver core (Chen et al., 2024).
//!
//! Solves each vertex's local 3-DOF implicit Euler variational minimization problem in parallel
//! graph-colored Gauss-Seidel passes with Chebyshev acceleration, supporting:
//! - Anisotropic StVK membrane energy (warp, weft, shear, buckling, yarn-jamming lock)
//! - Quadratic Bergou bending + non-zero dihedral rest-angle pleats/folds
//! - Barycentric sewing & dart springs
//! - Avatar SDF contact barrier penalties
//! - Interactive 3D vertex grab springs & user-placed pins
//! - Aerodynamic wind & drag forces
//! - Flat storage-buffer export for WebGPU WGSL compute acceleration

use crate::avatar::Vec3;
use crate::material::LOCK_STIFFENING;
use crate::xpbd::{ClothCore, Contact, MAX_STEP};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Number of VBD substeps per 60 Hz frame.
pub const VBD_SUBSTEPS: u32 = 10;
/// Number of Gauss-Seidel VBD iterations per substep.
const VBD_ITERS: u32 = 4;
/// Bending gain calibrated to match Peirce cantilever (ASTM D1388) at 6-8 substeps.
const VBD_BEND_GAIN: f32 = 2.0;
/// Spectral radius estimate for Chebyshev acceleration.
const CHEBYSHEV_RHO: f32 = 0.85;
/// Penalty stiffness for sewing / dart springs, N/m.
const SEW_STIFFNESS: f32 = 400.0;
/// Penalty stiffness for user-placed pins, N/m.
const PIN_STIFFNESS: f32 = 30000.0;
/// Penalty stiffness for interactive 3D grab spring, N/m.
const GRAB_STIFFNESS: f32 = 1800.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct VbdHinge {
    pub rest_angle_rad: f32,
    pub seam_stiff_mult: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GrabState {
    pub vert: usize,
    pub target: Vec3,
    pub radius_m: f32,
}

/// Graph coloring of mesh vertices so no two vertices in the same color share a triangle,
/// hinge, or sewing constraint.
#[derive(Debug, Clone)]
pub(crate) struct VertexColoring {
    pub color_offsets: Vec<u32>,
    pub color_verts: Vec<u32>,
}

impl VertexColoring {
    pub fn build(n_verts: usize, core: &ClothCore) -> Self {
        let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); n_verts];
        let mut connect = |a: usize, b: usize| {
            if a != b && a < n_verts && b < n_verts {
                adj[a].insert(b);
                adj[b].insert(a);
            }
        };
        for t in &core.tris {
            connect(t.v[0], t.v[1]);
            connect(t.v[1], t.v[2]);
            connect(t.v[2], t.v[0]);
        }
        for h in &core.hinges {
            for i in 0..4 {
                for j in (i + 1)..4 {
                    connect(h.v[i], h.v[j]);
                }
            }
        }
        for s in &core.sew {
            connect(s.a, s.b0);
            connect(s.a, s.b1);
            connect(s.b0, s.b1);
        }

        // Welsh-Powell: sort vertices by descending degree.
        let mut order: Vec<usize> = (0..n_verts).collect();
        order.sort_by(|&a, &b| adj[b].len().cmp(&adj[a].len()));

        let mut colors = vec![usize::MAX; n_verts];
        let mut max_color = 0usize;
        let mut used = Vec::new();
        for &v in &order {
            used.clear();
            for &nb in &adj[v] {
                if colors[nb] != usize::MAX {
                    used.push(colors[nb]);
                }
            }
            used.sort_unstable();
            used.dedup();
            let mut c = 0usize;
            for &u in &used {
                if u == c {
                    c += 1;
                } else if u > c {
                    break;
                }
            }
            colors[v] = c;
            if c + 1 > max_color {
                max_color = c + 1;
            }
        }

        let mut groups: Vec<Vec<u32>> = vec![Vec::new(); max_color.max(1)];
        for (v, &c) in colors.iter().enumerate() {
            groups[c].push(v as u32);
        }
        let mut color_offsets = Vec::with_capacity(groups.len() + 1);
        let mut color_verts = Vec::with_capacity(n_verts);
        color_offsets.push(0);
        for g in groups {
            color_verts.extend(g);
            color_offsets.push(color_verts.len() as u32);
        }
        Self {
            color_offsets,
            color_verts,
        }
    }
}

/// CSR adjacency from each vertex to its incident triangles, hinges, and sew constraints.
struct VertexAdjacency {
    tri_offsets: Vec<u32>,
    /// Packed `(tri_idx << 2) | local_slot (0..3)`
    tri_entries: Vec<u32>,
    hinge_offsets: Vec<u32>,
    /// Packed `(hinge_idx << 2) | local_slot (0..4)`
    hinge_entries: Vec<u32>,
    sew_offsets: Vec<u32>,
    /// Packed `(sew_idx << 2) | role (0=a, 1=b0, 2=b1)`
    sew_entries: Vec<u32>,
}

impl VertexAdjacency {
    fn build(n_verts: usize, core: &ClothCore) -> Self {
        let mut tri_lists: Vec<Vec<u32>> = vec![Vec::new(); n_verts];
        for (ti, t) in core.tris.iter().enumerate() {
            for (slot, &v) in t.v.iter().enumerate() {
                if v < n_verts {
                    tri_lists[v].push(((ti as u32) << 2) | (slot as u32));
                }
            }
        }
        let mut hinge_lists: Vec<Vec<u32>> = vec![Vec::new(); n_verts];
        for (hi, h) in core.hinges.iter().enumerate() {
            for (slot, &v) in h.v.iter().enumerate() {
                if v < n_verts {
                    hinge_lists[v].push(((hi as u32) << 2) | (slot as u32));
                }
            }
        }
        let mut sew_lists: Vec<Vec<u32>> = vec![Vec::new(); n_verts];
        for (si, s) in core.sew.iter().enumerate() {
            if s.a < n_verts {
                sew_lists[s.a].push(((si as u32) << 2) | 0);
            }
            if s.b0 < n_verts && s.b0 != s.a {
                sew_lists[s.b0].push(((si as u32) << 2) | 1);
            }
            if s.b1 < n_verts && s.b1 != s.a && s.b1 != s.b0 {
                sew_lists[s.b1].push(((si as u32) << 2) | 2);
            }
        }

        let flatten = |lists: Vec<Vec<u32>>| -> (Vec<u32>, Vec<u32>) {
            let mut offsets = Vec::with_capacity(n_verts + 1);
            let mut entries = Vec::new();
            offsets.push(0);
            for l in lists {
                entries.extend(l);
                offsets.push(entries.len() as u32);
            }
            (offsets, entries)
        };
        let (tri_offsets, tri_entries) = flatten(tri_lists);
        let (hinge_offsets, hinge_entries) = flatten(hinge_lists);
        let (sew_offsets, sew_entries) = flatten(sew_lists);

        Self {
            tri_offsets,
            tri_entries,
            hinge_offsets,
            hinge_entries,
            sew_offsets,
            sew_entries,
        }
    }
}

/// Closed-form solver for `H * dx = -g` where `H` is a symmetric 3x3 matrix stored as
/// `[h00, h01, h02, h11, h12, h22]`.
#[inline]
pub(crate) fn solve_3x3_spd(h: [f32; 6], g: Vec3) -> Vec3 {
    let h00 = h[0] + 1e-6;
    let h01 = h[1];
    let h02 = h[2];
    let h11 = h[3] + 1e-6;
    let h12 = h[4];
    let h22 = h[5] + 1e-6;

    let c00 = h11 * h22 - h12 * h12;
    let c01 = h02 * h12 - h01 * h22;
    let c02 = h01 * h12 - h02 * h11;
    let c11 = h00 * h22 - h02 * h02;
    let c12 = h01 * h02 - h00 * h12;
    let c22 = h00 * h11 - h01 * h01;

    let det = h00 * c00 + h01 * c01 + h02 * c02;
    if det <= 1e-18 || !det.is_finite() {
        return Vec3::new(-g.x / h00, -g.y / h11, -g.z / h22);
    }
    let inv = -1.0 / det;
    Vec3::new(
        (c00 * g.x + c01 * g.y + c02 * g.z) * inv,
        (c01 * g.x + c11 * g.y + c12 * g.z) * inv,
        (c02 * g.x + c12 * g.y + c22 * g.z) * inv,
    )
}

#[inline]
fn add_outer(h: &mut [f32; 6], v: Vec3, scale: f32) {
    h[0] += scale * v.x * v.x;
    h[1] += scale * v.x * v.y;
    h[2] += scale * v.x * v.z;
    h[3] += scale * v.y * v.y;
    h[4] += scale * v.y * v.z;
    h[5] += scale * v.z * v.z;
}

#[inline]
fn add_iso(h: &mut [f32; 6], scale: f32) {
    h[0] += scale;
    h[3] += scale;
    h[5] += scale;
}

/// Computes dihedral angle `theta` and gradients `dtheta/dx[0..4]` for hinge `[a, b, c, d]`,
/// where `(a, b)` is the shared edge and `c, d` are the opposite vertices.
fn dihedral_angle_and_grads(p: [Vec3; 4]) -> Option<(f32, [Vec3; 4])> {
    let e = p[1].sub(p[0]);
    let elen = e.len();
    if elen < 1e-7 {
        return None;
    }
    let e_hat = e.scale(1.0 / elen);
    let n1_raw = p[2].sub(p[0]).cross(e);
    let n2_raw = e.cross(p[3].sub(p[0]));
    let l1_sq = n1_raw.len_sq();
    let l2_sq = n2_raw.len_sq();
    if l1_sq < 1e-14 || l2_sq < 1e-14 {
        return None;
    }
    let l1 = l1_sq.sqrt();
    let l2 = l2_sq.sqrt();
    let n1 = n1_raw.scale(1.0 / l1);
    let n2 = n2_raw.scale(1.0 / l2);
    let cos_t = n1.dot(n2).clamp(-1.0, 1.0);
    let sin_t = n1.cross(n2).dot(e_hat);
    let theta = sin_t.atan2(cos_t);

    // Bridson / Grinspun analytical dihedral gradients (sum to zero for translation invariance).
    let inv_h1 = elen / l1;
    let inv_h2 = elen / l2;
    let g2 = n1.scale(inv_h1);
    let g3 = n2.scale(inv_h2);
    let w1_a = p[2].sub(p[1]).dot(e_hat) / elen;
    let w2_a = p[3].sub(p[1]).dot(e_hat) / elen;
    let w1_b = p[0].sub(p[2]).dot(e_hat) / elen;
    let w2_b = p[0].sub(p[3]).dot(e_hat) / elen;
    let g0 = n1.scale(w1_a * inv_h1).add(n2.scale(w2_a * inv_h2));
    let g1 = n1.scale(w1_b * inv_h1).add(n2.scale(w2_b * inv_h2));

    Some((theta, [g0, g1, g2, g3]))
}

#[derive(Debug, Clone, Serialize)]
pub struct VbdGpuMeta {
    pub vertex_count: usize,
    pub tri_count: usize,
    pub color_count: usize,
    pub color_offsets: Vec<u32>,
}

/// VBD solver state attached to `ClothSimSession`.
pub(crate) struct VbdSolverState {
    pub coloring: VertexColoring,
    adj: VertexAdjacency,
    pub hinge_params: Vec<VbdHinge>,
    pub user_pins: HashMap<usize, Vec3>,
    pub grab: Option<GrabState>,
    y_target: Vec<Vec3>,
    x_prev_iter: Vec<Vec3>,
    pub aero_acc: Vec<Vec3>,
}

impl VbdSolverState {
    pub fn new(core: &ClothCore) -> Self {
        let n = core.pos.len();
        let coloring = VertexColoring::build(n, core);
        let adj = VertexAdjacency::build(n, core);
        let hinge_params = vec![
            VbdHinge {
                rest_angle_rad: 0.0,
                seam_stiff_mult: 1.0,
            };
            core.hinges.len()
        ];
        Self {
            coloring,
            adj,
            hinge_params,
            user_pins: HashMap::new(),
            grab: None,
            y_target: vec![Vec3::ZERO; n],
            x_prev_iter: vec![Vec3::ZERO; n],
            aero_acc: vec![Vec3::ZERO; n],
        }
    }

    /// Recomputes aerodynamic drag + wind acceleration on every vertex.
    pub fn compute_aero(&mut self, core: &ClothCore, dt: f32, wind: Vec3) {
        for a in &mut self.aero_acc {
            *a = Vec3::ZERO;
        }
        if wind.len_sq() < 1e-6 || dt <= 0.0 {
            return;
        }
        let drag_coeff = 0.35_f32;
        for t in &core.tris {
            let [i0, i1, i2] = t.v;
            let p0 = core.pos[i0];
            let p1 = core.pos[i1];
            let p2 = core.pos[i2];
            let cross = p1.sub(p0).cross(p2.sub(p0));
            let area = 0.5 * cross.len();
            if area < 1e-8 {
                continue;
            }
            let n = cross.scale(0.5 / area);
            let v_tri = core.pos[i0]
                .sub(core.prev[i0])
                .add(core.pos[i1].sub(core.prev[i1]))
                .add(core.pos[i2].sub(core.prev[i2]))
                .scale(1.0 / (3.0 * dt.max(1e-5)));
            let v_rel = wind.sub(v_tri);
            let vn = v_rel.dot(n);
            let f_normal = n.scale(drag_coeff * area * vn * vn.abs().max(0.25));
            let f_tang = v_rel.sub(n.scale(vn)).scale(0.05 * drag_coeff * area);
            let f_vert = f_normal.add(f_tang).scale(1.0 / 3.0);
            for &vi in &t.v {
                if core.inv_mass[vi] > 0.0 {
                    self.aero_acc[vi] = self.aero_acc[vi].add(f_vert.scale(core.inv_mass[vi]));
                }
            }
        }
        for a in &mut self.aero_acc {
            let l = a.len();
            if l > 25.0 {
                *a = a.scale(25.0 / l);
            }
        }
    }

    /// Executes one VBD substep (prediction -> graph-colored Gauss-Seidel + Chebyshev -> velocity).
    pub fn step_substep(
        &mut self,
        core: &mut ClothCore,
        contacts: &[Contact],
        vert_offset: &[f32],
        dt: f32,
        gravity: Vec3,
        extra_damp: f32,
        sew_scale: f32,
        wind: Vec3,
        use_chebyshev: bool,
    ) {
        let n = core.pos.len();
        if n == 0 {
            return;
        }
        let inv_dt2 = 1.0 / (dt * dt);
        self.compute_aero(core, dt, wind);

        // 1. Inertia target y_i = x_i + dt * v_i + dt^2 * (g + a_aero)
        for i in 0..n {
            if core.inv_mass[i] <= 0.0 {
                self.y_target[i] = core.pos[i];
                core.prev[i] = core.pos[i];
                continue;
            }
            let decay = (-core.vert_damp[i] * dt).exp() * extra_damp;
            let mut v_dt = core.pos[i].sub(core.prev[i]).scale(decay);
            let vl = v_dt.len();
            if vl > MAX_STEP {
                v_dt = v_dt.scale(MAX_STEP / vl);
            }
            let ext = gravity.add(self.aero_acc[i]).scale(dt * dt);
            self.y_target[i] = core.pos[i].add(v_dt).add(ext);
            core.prev[i] = core.pos[i];
            core.pos[i] = self.y_target[i];
        }

        // 2. Graph-colored Gauss-Seidel VBD iterations with Chebyshev acceleration.
        let mut omega = 1.0_f32;
        let rho2 = CHEBYSHEV_RHO * CHEBYSHEV_RHO;
        for iter in 0..VBD_ITERS {
            if use_chebyshev {
                if iter == 0 {
                    self.x_prev_iter.copy_from_slice(&core.pos);
                    omega = 1.0;
                } else if iter == 1 {
                    omega = 2.0 / (2.0 - rho2);
                } else {
                    omega = 4.0 / (4.0 - rho2 * omega);
                }
            }

            for c in 0..(self.coloring.color_offsets.len().saturating_sub(1)) {
                let start = self.coloring.color_offsets[c] as usize;
                let end = self.coloring.color_offsets[c + 1] as usize;
                for &vi_u32 in &self.coloring.color_verts[start..end] {
                    let i = vi_u32 as usize;
                    if core.inv_mass[i] <= 0.0 {
                        continue;
                    }
                    let mut dx = self.solve_vertex(core, contacts, vert_offset, i, inv_dt2, sew_scale);
                    let dl = dx.len();
                    if dl > MAX_STEP {
                        dx = dx.scale(MAX_STEP / dl);
                    }
                    core.pos[i] = core.pos[i].add(dx);
                }
            }

            if use_chebyshev && iter >= 1 {
                for i in 0..n {
                    if core.inv_mass[i] <= 0.0 || self.user_pins.contains_key(&i) {
                        continue;
                    }
                    let blended = self.x_prev_iter[i]
                        .add(core.pos[i].sub(self.x_prev_iter[i]).scale(omega));
                    self.x_prev_iter[i] = core.pos[i];
                    let step = blended.sub(core.prev[i]);
                    if step.len() <= MAX_STEP * 1.5 {
                        core.pos[i] = blended;
                    }
                }
            }
        }
    }

    fn solve_vertex(
        &self,
        core: &ClothCore,
        contacts: &[Contact],
        vert_offset: &[f32],
        i: usize,
        inv_dt2: f32,
        sew_scale: f32,
    ) -> Vec3 {
        let xi = core.pos[i];
        if let Some(&pin_pos) = self.user_pins.get(&i) {
            return pin_pos.sub(xi);
        }
        let mass = 1.0 / core.inv_mass[i];
        let m_dt2 = mass * inv_dt2;

        // Inertia term
        let mut grad = xi.sub(self.y_target[i]).scale(m_dt2);
        let mut hess = [m_dt2, 0.0, 0.0, m_dt2, 0.0, m_dt2];

        // 1. Incident anisotropic StVK membrane triangles
        let ts = self.adj.tri_offsets[i] as usize;
        let te = self.adj.tri_offsets[i + 1] as usize;
        for &entry in &self.adj.tri_entries[ts..te] {
            let ti = (entry >> 2) as usize;
            let slot_idx = (entry & 3) as usize;
            let t = &core.tris[ti];
            let mat = &core.mats[t.slot as usize];
            let area = t.area.max(1e-8);
            let [i0, i1, i2] = t.v;
            let cu_i = t.cu[slot_idx];
            let cv_i = t.cv[slot_idx];

            let fu = core.pos[i0]
                .scale(t.cu[0])
                .add(core.pos[i1].scale(t.cu[1]))
                .add(core.pos[i2].scale(t.cu[2]));
            let fv = core.pos[i0]
                .scale(t.cv[0])
                .add(core.pos[i1].scale(t.cv[1]))
                .add(core.pos[i2].scale(t.cv[2]));

            // Warp stretch: 0.5 * k_u * (|fu| - rest_warp)^2
            let lu = fu.len();
            if lu > 1e-7 {
                let nu = fu.scale(1.0 / lu);
                let c_u = lu - mat.rest_warp;
                let strain_u = c_u / mat.rest_warp;
                let k_u = area
                    * if strain_u > mat.lock_warp {
                        mat.e_warp * LOCK_STIFFENING
                    } else if c_u < 0.0 {
                        mat.e_warp * mat.buckling
                    } else {
                        mat.e_warp
                    };
                grad = grad.add(nu.scale(k_u * c_u * cu_i));
                add_outer(&mut hess, nu, k_u * cu_i * cu_i);
                if c_u > 0.0 {
                    let k_geo = (k_u * c_u / lu) * cu_i * cu_i;
                    add_iso(&mut hess, k_geo);
                    add_outer(&mut hess, nu, -k_geo);
                }
            }

            // Weft stretch: 0.5 * k_v * (|fv| - rest_weft)^2
            let lv = fv.len();
            if lv > 1e-7 {
                let nv = fv.scale(1.0 / lv);
                let c_v = lv - mat.rest_weft;
                let strain_v = c_v / mat.rest_weft;
                let k_v = area
                    * if strain_v > mat.lock_weft {
                        mat.e_weft * LOCK_STIFFENING
                    } else if c_v < 0.0 {
                        mat.e_weft * mat.buckling
                    } else {
                        mat.e_weft
                    };
                grad = grad.add(nv.scale(k_v * c_v * cv_i));
                add_outer(&mut hess, nv, k_v * cv_i * cv_i);
                if c_v > 0.0 {
                    let k_geo = (k_v * c_v / lv) * cv_i * cv_i;
                    add_iso(&mut hess, k_geo);
                    add_outer(&mut hess, nv, -k_geo);
                }
            }

            // In-plane shear: e_uv = fu . fv
            let euv = fu.dot(fv);
            let k_s = area * mat.g_shear;
            let gs_dir = fv.scale(cu_i).add(fu.scale(cv_i));
            grad = grad.add(gs_dir.scale(k_s * euv));
            add_outer(&mut hess, gs_dir, k_s);
        }

        // 2. Incident bending hinges (quadratic Bergou or dihedral pleat/fold angle)
        let hs = self.adj.hinge_offsets[i] as usize;
        let he = self.adj.hinge_offsets[i + 1] as usize;
        for &entry in &self.adj.hinge_entries[hs..he] {
            let hi = (entry >> 2) as usize;
            let local = (entry & 3) as usize;
            let h = &core.hinges[hi];
            let hp = self.hinge_params[hi];
            let mat = &core.mats[h.slot as usize];
            let b = (mat.b_warp * h.warp_w + mat.b_weft * (1.0 - h.warp_w)).max(1e-9)
                * hp.seam_stiff_mult
                * VBD_BEND_GAIN;
            let kb = b / h.area2.max(1e-8);

            if hp.rest_angle_rad.abs() < 1e-4 {
                let sum_kx = core.pos[h.v[0]]
                    .scale(h.k[0])
                    .add(core.pos[h.v[1]].scale(h.k[1]))
                    .add(core.pos[h.v[2]].scale(h.k[2]))
                    .add(core.pos[h.v[3]].scale(h.k[3]));
                let w_i = h.k[local];
                grad = grad.add(sum_kx.scale(kb * w_i));
                add_iso(&mut hess, kb * w_i * w_i);
            } else if let Some((theta, grads)) = dihedral_angle_and_grads([
                core.pos[h.v[0]],
                core.pos[h.v[1]],
                core.pos[h.v[2]],
                core.pos[h.v[3]],
            ]) {
                let diff = theta - hp.rest_angle_rad;
                let g_loc = grads[local];
                let k_ang = b * 16.0;
                grad = grad.add(g_loc.scale(k_ang * diff));
                add_outer(&mut hess, g_loc, k_ang);
            }
        }

        // 3. Sewing & dart springs (active once initial gentle sew-closing completes)
        let ss = self.adj.sew_offsets[i] as usize;
        let se = self.adj.sew_offsets[i + 1] as usize;
        if sew_scale >= 0.99 {
            let k_sew = SEW_STIFFNESS;
            for &entry in &self.adj.sew_entries[ss..se] {
                let si = (entry >> 2) as usize;
                let role = entry & 3;
                let s = &core.sew[si];
                let pb = core.pos[s.b0]
                    .scale(1.0 - s.w)
                    .add(core.pos[s.b1].scale(s.w));
                let res = core.pos[s.a].sub(pb);
                let coeff = match role {
                    0 => 1.0,
                    1 => -(1.0 - s.w),
                    _ => -s.w,
                };
                grad = grad.add(res.scale(k_sew * coeff));
                add_iso(&mut hess, k_sew * coeff * coeff);
            }
        }

        // 4. Avatar SDF contact barrier is handled by `collide_vertices` + `collide_body_points`
        //    after each substep so friction and fresh SDF planes stay consistent.
        let _ = (contacts, vert_offset);

        // 5. User pins
        if let Some(&pin_pos) = self.user_pins.get(&i) {
            grad = grad.add(xi.sub(pin_pos).scale(PIN_STIFFNESS));
            add_iso(&mut hess, PIN_STIFFNESS);
        }

        // 6. Interactive 3D grab spring (Gaussian falloff over grab radius)
        if let Some(grab) = self.grab {
            let d_grab = xi.sub(core.pos[grab.vert]).len();
            if d_grab <= grab.radius_m {
                let r = grab.radius_m.max(1e-4);
                let w = (-2.0 * (d_grab / r) * (d_grab / r)).exp();
                let target_i = grab.target.add(xi.sub(core.pos[grab.vert]));
                let k = GRAB_STIFFNESS * w;
                grad = grad.add(xi.sub(target_i).scale(k));
                add_iso(&mut hess, k);
            }
        }

        solve_3x3_spd(hess, grad)
    }

    pub fn gpu_meta(&self, core: &ClothCore) -> VbdGpuMeta {
        VbdGpuMeta {
            vertex_count: core.pos.len(),
            tri_count: core.tris.len(),
            color_count: self.coloring.color_offsets.len().saturating_sub(1),
            color_offsets: self.coloring.color_offsets.clone(),
        }
    }

    pub fn gpu_u32_buffer(&self, core: &ClothCore, kind: &str) -> Vec<u32> {
        match kind {
            "color_verts" => self.coloring.color_verts.clone(),
            "vert_tri_offsets" => self.adj.tri_offsets.clone(),
            "vert_tri_entries" => self.adj.tri_entries.clone(),
            "tri_verts" => core
                .tris
                .iter()
                .flat_map(|t| [t.v[0] as u32, t.v[1] as u32, t.v[2] as u32, t.slot as u32])
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn gpu_f32_buffer(&self, core: &ClothCore, kind: &str) -> Vec<f32> {
        match kind {
            "inv_mass" => core.inv_mass.clone(),
            "tri_cuv" => core
                .tris
                .iter()
                .flat_map(|t| {
                    let m = &core.mats[t.slot as usize];
                    [
                        t.cu[0], t.cu[1], t.cu[2], t.area, t.cv[0], t.cv[1], t.cv[2], m.e_warp,
                        m.e_weft, m.g_shear, m.rest_warp, m.rest_weft,
                    ]
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::SolverMaterial;
    use crate::xpbd::{make_hinge, make_tri};
    use modelist_core::model::{FabricSpec, Point2};

    #[test]
    fn test_vbd_graph_coloring_has_no_conflicts() {
        let pos = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(0.0, 0.1, 0.0),
            Vec3::new(0.1, 0.1, 0.0),
        ];
        let rest = [
            Point2::new(0.0, 0.0),
            Point2::new(0.1, 0.0),
            Point2::new(0.0, 0.1),
            Point2::new(0.1, 0.1),
        ];
        let g = Point2::new(0.0, 1.0);
        let tris = vec![
            make_tri([0, 1, 2], [rest[0], rest[1], rest[2]], g, 0).unwrap(),
            make_tri([1, 3, 2], [rest[1], rest[3], rest[2]], g, 0).unwrap(),
        ];
        let hinges = vec![make_hinge([1, 2, 0, 3], [rest[1], rest[2], rest[0], rest[3]], g, 0).unwrap()];
        let mat = SolverMaterial::from_spec(&FabricSpec::from_preset("cotton_poplin"), false);
        let core = ClothCore::new(pos, tris, hinges, vec![mat]);
        let coloring = VertexColoring::build(4, &core);

        let mut vert_color = vec![0usize; 4];
        for c in 0..(coloring.color_offsets.len() - 1) {
            let s = coloring.color_offsets[c] as usize;
            let e = coloring.color_offsets[c + 1] as usize;
            for &v in &coloring.color_verts[s..e] {
                vert_color[v as usize] = c;
            }
        }
        for t in &core.tris {
            assert_ne!(vert_color[t.v[0]], vert_color[t.v[1]]);
            assert_ne!(vert_color[t.v[1]], vert_color[t.v[2]]);
            assert_ne!(vert_color[t.v[0]], vert_color[t.v[2]]);
        }
        for h in &core.hinges {
            for a in 0..4 {
                for b in (a + 1)..4 {
                    assert_ne!(vert_color[h.v[a]], vert_color[h.v[b]]);
                }
            }
        }
    }

    #[test]
    fn test_vbd_3x3_spd_solver_exact() {
        let h = [4.0, 1.2, -0.5, 5.0, 0.8, 3.5];
        let g = Vec3::new(2.0, -3.5, 1.25);
        let dx = solve_3x3_spd(h, g);
        let r0 = h[0] * dx.x + h[1] * dx.y + h[2] * dx.z + g.x;
        let r1 = h[1] * dx.x + h[3] * dx.y + h[4] * dx.z + g.y;
        let r2 = h[2] * dx.x + h[4] * dx.y + h[5] * dx.z + g.z;
        assert!(r0.abs() < 1e-4 && r1.abs() < 1e-4 && r2.abs() < 1e-4);
    }

    #[test]
    fn test_vbd_pleat_fold_angle_holds_crease() {
        // Two triangles sharing edge (0,1) along Y, initially flat; target crease = 60 deg.
        let pos = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.1, 0.0),
            Vec3::new(-0.08, 0.05, 0.02),
            Vec3::new(0.08, 0.05, 0.02),
        ];
        let rest = [
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 0.1),
            Point2::new(-0.08, 0.05),
            Point2::new(0.08, 0.05),
        ];
        let g = Point2::new(0.0, 1.0);
        let tris = vec![
            make_tri([0, 2, 1], [rest[0], rest[2], rest[1]], g, 0).unwrap(),
            make_tri([0, 1, 3], [rest[0], rest[1], rest[3]], g, 0).unwrap(),
        ];
        let hinges = vec![make_hinge([0, 1, 2, 3], [rest[0], rest[1], rest[2], rest[3]], g, 0).unwrap()];
        let mat = SolverMaterial::from_spec(&FabricSpec::from_preset("wool_suiting"), false);
        let mut core = ClothCore::new(pos, tris, hinges, vec![mat]);
        core.pinned[0] = true;
        core.pinned[1] = true;
        core.refresh_materials();

        let mut vbd = VbdSolverState::new(&core);
        let target_rad = 60.0_f32.to_radians();
        vbd.hinge_params[0].rest_angle_rad = target_rad;
        let dt = 1.0 / (60.0 * VBD_SUBSTEPS as f32);
        for _ in 0..240 {
            vbd.step_substep(
                &mut core,
                &[],
                &[],
                dt,
                Vec3::ZERO,
                0.88,
                1.0,
                Vec3::ZERO,
                false,
            );
        }
        let (theta, _) = dihedral_angle_and_grads([
            core.pos[0],
            core.pos[1],
            core.pos[2],
            core.pos[3],
        ])
        .unwrap();
        assert!(
            (theta - target_rad).abs() < 0.08,
            "Pleat crease angle {theta:.3} rad did not converge near target {target_rad:.3} rad"
        );
    }
}

