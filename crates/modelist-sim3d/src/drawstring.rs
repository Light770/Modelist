//! Phase 2: Waist & hoodie drawstrings (*cordons*), eyelets (*œillets*), and tunnel casings
//! (*coulisses*) powered by [`CosseratRod`] 1D elastic rod physics.

use crate::avatar::{ParametricAvatar, Vec3};
use crate::cosserat::CosseratRod;
use crate::xpbd::TriElem;

const TAIL_SEGMENTS: usize = 12;
const EYELET_SEGS: usize = 12;
const EYELET_RING_SIDES: usize = 6;
const TUBE_SIDES: usize = 10;

#[derive(Debug, Clone, Copy)]
pub struct EyeletAnchor {
    pub top_vert: usize,
    pub bottom_vert: usize,
    pub side_sign: f32,
}

impl EyeletAnchor {
    /// Computes the live 3D eyelet center and outward surface normal parallel to the waistband.
    pub fn frame(
        &self,
        cloth_pos: &[Vec3],
        cloth_normals: &[f32],
        vert_thick: &[f32],
        avatar: &ParametricAvatar,
        extra_out_m: f32,
    ) -> (Vec3, Vec3, Vec3) {
        let pt = cloth_pos[self.top_vert];
        let pb = cloth_pos[self.bottom_vert];
        let mid = pt.add(pb).scale(0.5);
        let mut up_vec = pt.sub(pb);
        if up_vec.len_sq() < 1e-8 {
            up_vec = Vec3::new(0.0, 1.0, 0.0);
        } else {
            up_vec = up_vec.normalized();
        }
        let nt = Vec3::new(
            cloth_normals[self.top_vert * 3],
            cloth_normals[self.top_vert * 3 + 1],
            cloth_normals[self.top_vert * 3 + 2],
        );
        let nb = Vec3::new(
            cloth_normals[self.bottom_vert * 3],
            cloth_normals[self.bottom_vert * 3 + 1],
            cloth_normals[self.bottom_vert * 3 + 2],
        );
        let mut nrm = nt.add(nb);
        if let Some((_, sdf_n)) = avatar.eval_sdf_within(mid, 0.08) {
            if nrm.len_sq() < 1e-6 {
                nrm = sdf_n;
            } else if nrm.dot(sdf_n) < 0.0 {
                nrm = nrm.scale(-1.0);
            }
        } else if nrm.len_sq() < 1e-6 {
            nrm = Vec3::new(0.0, 0.0, 1.0);
        }
        nrm = nrm.sub(up_vec.scale(nrm.dot(up_vec)));
        if nrm.len_sq() < 1e-8 {
            nrm = Vec3::new(0.0, 0.0, 1.0);
        } else {
            nrm = nrm.normalized();
        }
        let half_t = 0.25
            * (vert_thick.get(self.top_vert).copied().unwrap_or(0.001)
                + vert_thick.get(self.bottom_vert).copied().unwrap_or(0.001));
        (mid.add(nrm.scale(half_t + extra_out_m)), nrm, up_vec)
    }
}

pub struct DrawstringSim {
    pub piece_idx: u32,
    pub ring_verts: Vec<usize>,
    pub ring_weld_copies: Vec<(usize, usize)>,
    pub total_orig: f32,
    pub tighten_ratio: f32,
    pub cord_radius_m: f32,
    pub color_rgb: [f32; 3],
    pub eyelets: [EyeletAnchor; 2],
    pub tails: [CosseratRod; 2],
}

impl DrawstringSim {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        piece_idx: u32,
        ring_verts: Vec<usize>,
        ring_weld_copies: Vec<(usize, usize)>,
        total_orig: f32,
        eyelets: [EyeletAnchor; 2],
        cloth_pos: &[Vec3],
        cloth_normals: &[f32],
        avatar: &ParametricAvatar,
        tail_length_m: f32,
        cord_radius_m: f32,
        tighten_ratio: f32,
        color_rgb: [f32; 3],
    ) -> Self {
        let seg_len = (tail_length_m.max(0.04)) / TAIL_SEGMENTS as f32;
        let build_tail = |anchor: EyeletAnchor| -> CosseratRod {
            let (origin, nrm, _) =
                anchor.frame(cloth_pos, cloth_normals, &[], avatar, cord_radius_m + 0.0025);
            let mut pts = Vec::with_capacity(TAIL_SEGMENTS + 1);
            for k in 0..=TAIL_SEGMENTS {
                let s = k as f32 / TAIL_SEGMENTS as f32;
                let out = (1.0 - s * 0.5) * 0.012 * (k.min(2) as f32 * 0.5);
                let lat = anchor.side_sign * 0.004 * s;
                pts.push(
                    origin
                        .add(nrm.scale(out))
                        .add(Vec3::new(lat, -(k as f32) * seg_len, 0.0)),
                );
            }
            let mut rod = CosseratRod::new(pts, 0.0015, 80.0, 0.0012);
            rod.inv_mass[0] = 0.0;
            rod
        };
        let tails = [build_tail(eyelets[0]), build_tail(eyelets[1])];
        Self {
            piece_idx,
            ring_verts,
            ring_weld_copies,
            total_orig,
            tighten_ratio: tighten_ratio.clamp(0.65, 1.0),
            cord_radius_m: cord_radius_m.clamp(0.0015, 0.006),
            color_rgb,
            eyelets,
            tails,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step(
        &mut self,
        cloth_pos: &mut [Vec3],
        cloth_prev: &mut [Vec3],
        cloth_inv_mass: &[f32],
        cloth_normals: &[f32],
        cloth_tris: &[TriElem],
        avatar: &ParametricAvatar,
        dt: f32,
        gravity: Vec3,
        engage: f32,
    ) {
        // 1. Optional casing channel cinch when tighten_ratio < 0.995.
        let n = self.ring_verts.len();
        if n >= 4 && self.tighten_ratio < 0.995 && engage > 1e-4 {
            let eff_ratio = 1.0 - (1.0 - self.tighten_ratio) * engage.clamp(0.0, 1.0);
            let target_len = self.total_orig * eff_ratio;
            let mut grad = vec![Vec3::ZERO; n];
            let mut len = 0.0_f32;
            for k in 0..n {
                let k1 = (k + 1) % n;
                let d = cloth_pos[self.ring_verts[k1]].sub(cloth_pos[self.ring_verts[k]]);
                let l = d.len();
                len += l;
                if l > 1e-9 {
                    let u = d.scale(1.0 / l);
                    grad[k] = grad[k].sub(u);
                    grad[k1] = grad[k1].add(u);
                }
            }
            let cval = len - target_len;
            if cval > 0.0 {
                let mut wsum = 0.0_f32;
                for k in 0..n {
                    wsum += cloth_inv_mass[self.ring_verts[k]] * grad[k].len_sq();
                }
                if wsum > 1e-12 {
                    let inv_dt2 = 1.0 / (dt * dt);
                    let alpha = self.total_orig / (220.0 * 60.0);
                    let dl = -cval / (wsum + alpha * inv_dt2);
                    let mut max_disp = 0.0_f32;
                    for k in 0..n {
                        max_disp = max_disp
                            .max(grad[k].len() * cloth_inv_mass[self.ring_verts[k]] * dl.abs());
                    }
                    let scale = if max_disp > 0.0005 { 0.0005 / max_disp } else { 1.0 };
                    let eff_dl = dl * scale;
                    for k in 0..n {
                        let v = self.ring_verts[k];
                        let dp = grad[k].scale(cloth_inv_mass[v] * eff_dl);
                        cloth_pos[v] = cloth_pos[v].add(dp);
                        cloth_prev[v] = cloth_prev[v].add(dp.scale(0.95));
                    }
                    for &(copy_v, ring_k) in &self.ring_weld_copies {
                        let v = self.ring_verts[ring_k];
                        let dp = grad[ring_k].scale(cloth_inv_mass[v] * eff_dl);
                        cloth_pos[copy_v] = cloth_pos[copy_v].add(dp);
                        cloth_prev[copy_v] = cloth_prev[copy_v].add(dp.scale(0.95));
                    }
                }
            }
        }

        // 2. Step both hanging CosseratRod cord tails from their live eyelet anchors.
        let min_clear = self.cord_radius_m + 0.010;
        for (anchor, rod) in self.eyelets.iter().zip(self.tails.iter_mut()) {
            let (origin, nrm, _) =
                anchor.frame(cloth_pos, cloth_normals, &[], avatar, self.cord_radius_m + 0.0025);
            rod.pos[0] = origin;
            rod.prev[0] = origin;
            if rod.pos.len() > 1 {
                let exit_target = origin.add(nrm.scale(self.cord_radius_m * 1.6));
                let diff = exit_target.sub(rod.pos[1]);
                let d_nrm = diff.dot(nrm);
                if d_nrm > 0.0 {
                    rod.pos[1] = rod.pos[1].add(nrm.scale(d_nrm * 0.35));
                }
            }
            rod.step(dt, gravity, 4, false);
            let cloth_pad = self.cord_radius_m + 0.005;
            for k in 1..rod.pos.len() {
                let mut p = rod.pos[k];
                if let Some((d, n_sdf)) = avatar.eval_sdf_within(p, min_clear + 0.015) {
                    if d < min_clear {
                        p = p.add(n_sdf.scale(min_clear - d));
                    }
                }
                if !cloth_tris.is_empty() {
                    let out_hint = avatar
                        .eval_sdf_within(p, 0.15)
                        .map(|(_, n)| n)
                        .unwrap_or_else(|| Vec3::new(0.0, 0.0, 1.0));
                    let search = 0.025_f32;
                    for tri in cloth_tris {
                        let a = cloth_pos[tri.v[0]];
                        let b = cloth_pos[tri.v[1]];
                        let c = cloth_pos[tri.v[2]];
                        if p.x < a.x.min(b.x).min(c.x) - search
                            || p.x > a.x.max(b.x).max(c.x) + search
                            || p.y < a.y.min(b.y).min(c.y) - search
                            || p.y > a.y.max(b.y).max(c.y) + search
                            || p.z < a.z.min(b.z).min(c.z) - search
                            || p.z > a.z.max(b.z).max(c.z) + search
                        {
                            continue;
                        }
                        let ab = b.sub(a);
                        let ac = c.sub(a);
                        let n_raw = ab.cross(ac);
                        let n2 = n_raw.len_sq();
                        if n2 < 1e-12 {
                            continue;
                        }
                        let mut n_hat = n_raw.scale(1.0 / n2.sqrt());
                        if n_hat.dot(out_hint) < 0.0 {
                            n_hat = n_hat.scale(-1.0);
                        }
                        let ap = p.sub(a);
                        let w_b = ap.cross(ac).dot(n_raw) / n2;
                        let w_c = ab.cross(ap).dot(n_raw) / n2;
                        let w_a = 1.0 - w_b - w_c;
                        if w_a < -0.15 || w_b < -0.15 || w_c < -0.15 {
                            continue;
                        }
                        let d = ap.dot(n_hat);
                        if d > -0.025 && d < cloth_pad {
                            let push = cloth_pad - d;
                            p = p.add(n_hat.scale(push));
                            let v = p.sub(rod.prev[k]);
                            let vn = v.dot(n_hat);
                            if vn < 0.0 {
                                rod.prev[k] = rod.prev[k].add(n_hat.scale(vn));
                            }
                        }
                    }
                }
                rod.pos[k] = p;
            }
        }
    }

    pub fn render_vert_count(&self) -> usize {
        2 * (EYELET_SEGS * EYELET_RING_SIDES) + 2 * ((TAIL_SEGMENTS + 1) * TUBE_SIDES)
    }

    pub fn append_indices(&self, base_vert: u32, out: &mut Vec<u32>) {
        let mut cursor = base_vert;
        let segs = EYELET_SEGS as u32;
        let rsides = EYELET_RING_SIDES as u32;
        for _ in 0..2 {
            for k in 0..segs {
                let k1 = (k + 1) % segs;
                let r0 = cursor + k * rsides;
                let r1 = cursor + k1 * rsides;
                for s in 0..rsides {
                    let s1 = (s + 1) % rsides;
                    out.extend_from_slice(&[r0 + s, r1 + s, r1 + s1, r0 + s, r1 + s1, r0 + s1]);
                }
            }
            cursor += segs * rsides;
        }
        let sides = TUBE_SIDES as u32;
        for _ in 0..2 {
            for r in 0..TAIL_SEGMENTS as u32 {
                let r0 = cursor + r * sides;
                let r1 = cursor + (r + 1) * sides;
                for s in 0..sides {
                    let s1 = (s + 1) % sides;
                    out.extend_from_slice(&[r0 + s, r0 + s1, r1 + s1, r0 + s, r1 + s1, r1 + s]);
                }
            }
            cursor += (TAIL_SEGMENTS as u32 + 1) * sides;
        }
    }

    pub fn append_colors(&self, out: &mut Vec<f32>) {
        let grommet = [0.72_f32, 0.74, 0.78];
        let aglet = [0.30_f32, 0.32, 0.36];
        for _ in 0..2 * (EYELET_SEGS * EYELET_RING_SIDES) {
            out.extend_from_slice(&grommet);
        }
        for _ in 0..2 {
            for r in 0..=TAIL_SEGMENTS {
                let col = if r >= TAIL_SEGMENTS - 1 { aglet } else { self.color_rgb };
                for _ in 0..TUBE_SIDES {
                    out.extend_from_slice(&col);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn write_render_geometry(
        &self,
        cloth_pos: &[Vec3],
        cloth_normals: &[f32],
        vert_thick: &[f32],
        avatar: &ParametricAvatar,
        base_vert: usize,
        positions: &mut [f32],
        normals: &mut [f32],
    ) {
        let mut cursor = base_vert;
        let r_major = (self.cord_radius_m * 1.45).max(0.0038);
        let r_minor = 0.0011_f32;

        // 2 3D torus metallic eyelet grommets flush with the outer waistband surface
        for anchor in &self.eyelets {
            let (center, nrm, up_vec) =
                anchor.frame(cloth_pos, cloth_normals, vert_thick, avatar, 0.0012);
            let mut t1 = up_vec.cross(nrm);
            if t1.len_sq() < 1e-6 {
                t1 = Vec3::new(1.0, 0.0, 0.0);
            } else {
                t1 = t1.normalized();
            }
            let t2 = nrm.cross(t1).normalized();
            for k in 0..EYELET_SEGS {
                let phi = std::f32::consts::TAU * (k as f32 / EYELET_SEGS as f32);
                let radial = t1.scale(phi.cos()).add(t2.scale(phi.sin()));
                let ring_c = center.add(radial.scale(r_major));
                for s in 0..EYELET_RING_SIDES {
                    let theta = std::f32::consts::TAU * (s as f32 / EYELET_RING_SIDES as f32);
                    let out_n = radial.scale(theta.cos()).add(nrm.scale(theta.sin()));
                    let p = ring_c.add(out_n.scale(r_minor));
                    let dst = cursor + k * EYELET_RING_SIDES + s;
                    positions[dst * 3..dst * 3 + 3].copy_from_slice(&[p.x, p.y, p.z]);
                    normals[dst * 3..dst * 3 + 3].copy_from_slice(&[out_n.x, out_n.y, out_n.z]);
                }
            }
            cursor += EYELET_SEGS * EYELET_RING_SIDES;
        }

        // 2 cylindrical Cosserat cord tails
        for rod in &self.tails {
            let n_pts = rod.pos.len();
            let mut prev_t = Vec3::new(0.0, -1.0, 0.0);
            let mut u_axis = Vec3::new(1.0, 0.0, 0.0);
            for r in 0..n_pts {
                let tang = if r + 1 < n_pts {
                    rod.pos[r + 1].sub(rod.pos[r]).normalized()
                } else {
                    rod.pos[r].sub(rod.pos[r - 1]).normalized()
                };
                if r == 0 {
                    let ref_v = if tang.x.abs() < 0.85 {
                        Vec3::new(1.0, 0.0, 0.0)
                    } else {
                        Vec3::new(0.0, 0.0, 1.0)
                    };
                    u_axis = ref_v.cross(tang).normalized();
                } else {
                    let rot_axis = prev_t.cross(tang);
                    if rot_axis.len_sq() > 1e-8 {
                        u_axis = u_axis.sub(tang.scale(u_axis.dot(tang))).normalized();
                    }
                }
                prev_t = tang;
                let v_axis = tang.cross(u_axis).normalized();
                let rad = if r + 1 == n_pts {
                    self.cord_radius_m * 0.78
                } else {
                    self.cord_radius_m
                };
                for s in 0..TUBE_SIDES {
                    let ang = std::f32::consts::TAU * (s as f32 / TUBE_SIDES as f32);
                    let radial = u_axis.scale(ang.cos()).add(v_axis.scale(ang.sin()));
                    let p = rod.pos[r].add(radial.scale(rad));
                    let dst = cursor + r * TUBE_SIDES + s;
                    positions[dst * 3..dst * 3 + 3].copy_from_slice(&[p.x, p.y, p.z]);
                    normals[dst * 3..dst * 3 + 3].copy_from_slice(&[radial.x, radial.y, radial.z]);
                }
            }
            cursor += n_pts * TUBE_SIDES;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avatar::AvatarPose;
    use modelist_core::model::BodyMeasurements;

    #[test]
    fn test_drawstring_drapes_under_gravity_without_nan_or_penetration() {
        let avatar = ParametricAvatar::from_measurements(
            &BodyMeasurements::default(),
            AvatarPose::APose,
        );
        let top_y = avatar.waist_y + 0.02;
        let bot_y = avatar.waist_y - 0.025;
        let z_front = avatar
            .eval_sdf_within(Vec3::new(0.0, top_y, 0.25), 0.30)
            .map(|(d, _)| 0.25 - d + 0.008)
            .unwrap_or(0.14);

        let mut cloth_pos = vec![
            Vec3::new(0.02, top_y, z_front),
            Vec3::new(0.02, bot_y, z_front),
            Vec3::new(-0.02, top_y, z_front),
            Vec3::new(-0.02, bot_y, z_front),
        ];
        let mut cloth_prev = cloth_pos.clone();
        let cloth_inv_mass = vec![1.0; 4];
        let cloth_normals = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let vert_thick = vec![0.002; 4];
        let eyelets = [
            EyeletAnchor { top_vert: 0, bottom_vert: 1, side_sign: 1.0 },
            EyeletAnchor { top_vert: 2, bottom_vert: 3, side_sign: -1.0 },
        ];
        let mut ds = DrawstringSim::new(
            0,
            vec![],
            vec![],
            0.74,
            eyelets,
            &cloth_pos,
            &cloth_normals,
            &avatar,
            0.19,
            0.003,
            0.92,
            [0.92, 0.90, 0.87],
        );

        let dt = 1.0 / 240.0;
        let gravity = Vec3::new(0.0, -9.81, 0.0);
        for _ in 0..120 {
            ds.step(
                &mut cloth_pos,
                &mut cloth_prev,
                &cloth_inv_mass,
                &cloth_normals,
                &[],
                &avatar,
                dt,
                gravity,
                1.0,
            );
        }

        for rod in &ds.tails {
            assert!(rod.pos.last().unwrap().y < rod.pos[0].y - 0.10);
            for p in &rod.pos {
                assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
                if let Some((d, _)) = avatar.eval_sdf_within(*p, 0.05) {
                    assert!(d >= 0.005, "cord penetrated avatar: sdf={d}");
                }
            }
        }

        let nv = ds.render_vert_count();
        let mut r_pos = vec![0.0; nv * 3];
        let mut r_nrm = vec![0.0; nv * 3];
        ds.write_render_geometry(
            &cloth_pos,
            &cloth_normals,
            &vert_thick,
            &avatar,
            0,
            &mut r_pos,
            &mut r_nrm,
        );
        assert!(r_pos.iter().all(|v| v.is_finite()));
        assert!(r_nrm.iter().all(|v| v.is_finite()));
    }
}
