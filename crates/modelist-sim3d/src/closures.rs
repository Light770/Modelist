//! Multi-layer garment ordering and hardware/closure physics (*braguettes*, zippers, buttons).
//!
//! Combines explicit layer-order indices with 1D Stable Cosserat Rods (zipper spines) and
//! ring-to-post button couplings so multi-layered plackets and flies stay ordered and bear
//! tension without expensive offline C-IPC.

use crate::avatar::{ParametricAvatar, Vec3};
use crate::cosserat::CosseratRod;

/// Maps a piece's `fabric_role` to its stacking layer order (`0` innermost .. `2` outermost).
pub fn garment_layer_for_role(role: &str) -> u8 {
    let r = role.trim().to_ascii_lowercase();
    if r.contains("lining")
        || r.contains("fly_shield")
        || r.contains("shield")
        || r.contains("pocket")
        || r.contains("underlap")
    {
        0
    } else if r.contains("facing")
        || r.contains("collar")
        || r.contains("overlap")
        || r.contains("cuff_turnup")
        || r.contains("belt_loop")
    {
        2
    } else {
        1
    }
}

/// Zipper closure coupled to a 1D [`CosseratRod`] spine with an interactive `zip_fraction` (0..1).
pub struct ZipperClosure {
    pub left_verts: Vec<usize>,
    pub right_verts: Vec<usize>,
    pub rod: CosseratRod,
    pub zip_fraction: f32,
}

impl ZipperClosure {
    pub fn new(
        cloth_pos: &[Vec3],
        left_verts: Vec<usize>,
        right_verts: Vec<usize>,
        k_ss: f32,
        k_bt: f32,
    ) -> Self {
        let count = left_verts.len().min(right_verts.len());
        let mut rod_pos = Vec::with_capacity(count);
        for k in 0..count {
            let l = cloth_pos[left_verts[k]];
            let r = cloth_pos[right_verts[k]];
            rod_pos.push(l.add(r).scale(0.5));
        }
        let rod = CosseratRod::new(rod_pos, 0.008, k_ss, k_bt);
        Self {
            left_verts,
            right_verts,
            rod,
            zip_fraction: 1.0,
        }
    }

    pub fn step(&mut self, pos: &mut [Vec3], inv_mass: &[f32], dt: f32, gravity: Vec3) {
        let count = self.left_verts.len().min(self.right_verts.len()).min(self.rod.pos.len());
        if count == 0 {
            return;
        }
        self.rod.step(dt, gravity, 4, false);
        let denom = (count.saturating_sub(1)).max(1) as f32;
        for k in 0..count {
            let u = k as f32 / denom;
            if u > self.zip_fraction + 1e-4 {
                continue;
            }
            let li = self.left_verts[k];
            let ri = self.right_verts[k];
            let wl = inv_mass[li];
            let wr = inv_mass[ri];
            let wrod = self.rod.inv_mass[k];
            let wsum = wl + wr + wrod;
            if wsum <= 1e-12 {
                continue;
            }
            let target = pos[li]
                .scale(wl)
                .add(pos[ri].scale(wr))
                .add(self.rod.pos[k].scale(wrod))
                .scale(1.0 / wsum);
            if wl > 0.0 {
                pos[li] = pos[li].add(target.sub(pos[li]).scale(0.85));
            }
            if wr > 0.0 {
                pos[ri] = pos[ri].add(target.sub(pos[ri]).scale(0.85));
            }
            if wrod > 0.0 {
                self.rod.pos[k] = self.rod.pos[k].add(target.sub(self.rod.pos[k]).scale(0.85));
            }
        }
    }
}

/// Button-to-buttonhole ring constraint carrying placket/waistband tension across layers.
#[derive(Debug, Clone)]
pub struct ButtonClosure {
    pub post_vert: usize,
    pub ring_verts: Vec<usize>,
    pub shank_m: f32,
}

impl ButtonClosure {
    pub fn solve(&self, pos: &mut [Vec3], inv_mass: &[f32]) {
        if self.ring_verts.is_empty() || self.post_vert >= pos.len() {
            return;
        }
        let mut center = Vec3::ZERO;
        let mut cnt = 0usize;
        for &vi in &self.ring_verts {
            if vi < pos.len() {
                center = center.add(pos[vi]);
                cnt += 1;
            }
        }
        if cnt == 0 {
            return;
        }
        center = center.scale(1.0 / cnt as f32);
        let d = pos[self.post_vert].sub(center);
        let len = d.len();
        if len <= self.shank_m || len < 1e-9 {
            return;
        }
        let excess = len - self.shank_m;
        let dir = d.scale(1.0 / len);
        let wp = inv_mass[self.post_vert];
        let wr: f32 = self
            .ring_verts
            .iter()
            .filter_map(|&vi| inv_mass.get(vi).copied())
            .sum::<f32>()
            / cnt as f32;
        let wsum = wp + wr;
        if wsum <= 1e-12 {
            return;
        }
        pos[self.post_vert] = pos[self.post_vert].sub(dir.scale(excess * (wp / wsum)));
        let ring_shift = dir.scale(excess * (wr / wsum));
        for &vi in &self.ring_verts {
            if vi < pos.len() && inv_mass[vi] > 0.0 {
                pos[vi] = pos[vi].add(ring_shift);
            }
        }
    }
}

/// Internal tacking stitch (e.g. trouser fly J-stitch) coupling two cloth layers together.
#[derive(Debug, Clone, Default)]
pub struct InternalTack {
    pub pairs: Vec<(usize, usize)>,
}

impl InternalTack {
    pub fn solve(&self, pos: &mut [Vec3], inv_mass: &[f32]) {
        for &(a, b) in &self.pairs {
            if a >= pos.len() || b >= pos.len() {
                continue;
            }
            let wa = inv_mass[a];
            let wb = inv_mass[b];
            let wsum = wa + wb;
            if wsum <= 1e-12 {
                continue;
            }
            let d = pos[b].sub(pos[a]);
            pos[a] = pos[a].add(d.scale(wa / wsum));
            pos[b] = pos[b].sub(d.scale(wb / wsum));
        }
    }
}

const ZIP_TRACK_STATIONS: usize = 10;
const BTN_SIDES: usize = 12;
const BELT_LOOP_STATIONS: usize = 7;

fn outward_surface_normal(
    pos: Vec3,
    cloth_normals: &[f32],
    vert_idx: usize,
    avatar: &ParametricAvatar,
) -> Vec3 {
    let mut nrm = Vec3::new(
        cloth_normals.get(vert_idx * 3).copied().unwrap_or(0.0),
        cloth_normals.get(vert_idx * 3 + 1).copied().unwrap_or(0.0),
        cloth_normals.get(vert_idx * 3 + 2).copied().unwrap_or(1.0),
    );
    if let Some((_, sdf_n)) = avatar.eval_sdf_within(pos, 0.10) {
        if nrm.len_sq() < 1e-6 {
            nrm = sdf_n;
        } else if nrm.dot(sdf_n) < 0.0 {
            nrm = nrm.scale(-1.0);
        }
    } else if nrm.len_sq() < 1e-6 {
        nrm = Vec3::new(0.0, 0.0, 1.0);
    }
    nrm.normalized()
}

fn append_box_indices(base: u32, out: &mut Vec<u32>) {
    let quads: [[u32; 4]; 6] = [
        [0, 1, 2, 3], // back
        [4, 5, 6, 7], // front
        [0, 1, 5, 4], // bottom
        [2, 3, 7, 6], // top
        [0, 3, 7, 4], // left
        [1, 2, 6, 5], // right
    ];
    for q in quads {
        out.extend_from_slice(&[
            base + q[0],
            base + q[1],
            base + q[2],
            base + q[0],
            base + q[2],
            base + q[3],
        ]);
    }
}

fn write_box_geometry(
    center: Vec3,
    t1: Vec3,
    t2: Vec3,
    nrm: Vec3,
    half_ext: [f32; 3],
    base: usize,
    positions: &mut [f32],
    normals: &mut [f32],
) {
    let corners = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    for (k, c) in corners.iter().enumerate() {
        let off = t1
            .scale(c[0] * half_ext[0])
            .add(t2.scale(c[1] * half_ext[1]))
            .add(nrm.scale(c[2] * half_ext[2]));
        let p = center.add(off);
        let nn = off.normalized();
        let dst = base + k;
        positions[dst * 3..dst * 3 + 3].copy_from_slice(&[p.x, p.y, p.z]);
        normals[dst * 3..dst * 3 + 3].copy_from_slice(&[nn.x, nn.y, nn.z]);
    }
}

/// Interactive 3D zipper physics + metallic teeth track and slider mesh.
pub struct ZipperSim {
    pub piece_idx: u32,
    pub closure: ZipperClosure,
    pub teeth_width_m: f32,
    pub color_rgb: [f32; 3],
}

impl ZipperSim {
    pub fn render_vert_count(&self) -> usize {
        ZIP_TRACK_STATIONS * 4 + 16
    }

    pub fn append_indices(&self, base_vert: u32, out: &mut Vec<u32>) {
        for k in 0..(ZIP_TRACK_STATIONS as u32 - 1) {
            let r0 = base_vert + k * 4;
            let r1 = base_vert + (k + 1) * 4;
            for s in 0..4u32 {
                let s1 = (s + 1) % 4;
                out.extend_from_slice(&[r0 + s, r0 + s1, r1 + s1, r0 + s, r1 + s1, r1 + s]);
            }
        }
        let slider_base = base_vert + (ZIP_TRACK_STATIONS as u32) * 4;
        append_box_indices(slider_base, out);
        append_box_indices(slider_base + 8, out);
    }

    pub fn append_colors(&self, out: &mut Vec<f32>) {
        for _ in 0..ZIP_TRACK_STATIONS * 4 {
            out.extend_from_slice(&self.color_rgb);
        }
        let slider_col = [
            (self.color_rgb[0] * 0.78).min(1.0),
            (self.color_rgb[1] * 0.78).min(1.0),
            (self.color_rgb[2] * 0.78).min(1.0),
        ];
        for _ in 0..16 {
            out.extend_from_slice(&slider_col);
        }
    }

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
        let count = self
            .closure
            .left_verts
            .len()
            .min(self.closure.right_verts.len());
        if count < 2 {
            return;
        }
        let zf = self.closure.zip_fraction.clamp(0.0, 1.0);
        let sample_at = |u: f32| -> (Vec3, Vec3, Vec3) {
            let fidx = (u.clamp(0.0, 1.0) * (count - 1) as f32).clamp(0.0, (count - 1) as f32);
            let i0 = (fidx.floor() as usize).min(count - 1);
            let i1 = (i0 + 1).min(count - 1);
            let frac = fidx - i0 as f32;
            let vl0 = self.closure.left_verts[i0];
            let vr0 = self.closure.right_verts[i0];
            let vl1 = self.closure.left_verts[i1];
            let vr1 = self.closure.right_verts[i1];
            let p0 = cloth_pos[vl0].add(cloth_pos[vr0]).scale(0.5);
            let p1 = cloth_pos[vl1].add(cloth_pos[vr1]).scale(0.5);
            let p = p0.scale(1.0 - frac).add(p1.scale(frac));
            let nrm = outward_surface_normal(p, cloth_normals, vl0, avatar);
            let half_t = 0.5 * vert_thick.get(vl0).copied().unwrap_or(0.001);
            let mut tang = if i1 > i0 {
                p1.sub(p0)
            } else if i0 > 0 {
                let pm = cloth_pos[self.closure.left_verts[i0 - 1]]
                    .add(cloth_pos[self.closure.right_verts[i0 - 1]])
                    .scale(0.5);
                p0.sub(pm)
            } else {
                Vec3::new(0.0, 1.0, 0.0)
            };
            tang = tang.sub(nrm.scale(tang.dot(nrm)));
            if tang.len_sq() < 1e-8 {
                tang = Vec3::new(0.0, 1.0, 0.0);
            } else {
                tang = tang.normalized();
            }
            (p.add(nrm.scale(half_t + 0.0010)), nrm, tang)
        };

        let hw = (0.5 * self.teeth_width_m).max(0.0018);
        let ht = 0.0011_f32;
        for k in 0..ZIP_TRACK_STATIONS {
            let u = (k as f32 / (ZIP_TRACK_STATIONS - 1) as f32) * zf;
            let (c, nrm, tang) = sample_at(u);
            let lat = tang.cross(nrm).normalized();
            let ring = [
                c.sub(lat.scale(hw)),
                c.add(nrm.scale(ht)),
                c.add(lat.scale(hw)),
                c.sub(nrm.scale(ht * 0.5)),
            ];
            let r_nrm = [lat.scale(-1.0), nrm, lat, nrm.scale(-1.0)];
            for s in 0..4 {
                let dst = base_vert + k * 4 + s;
                positions[dst * 3..dst * 3 + 3].copy_from_slice(&[ring[s].x, ring[s].y, ring[s].z]);
                normals[dst * 3..dst * 3 + 3]
                    .copy_from_slice(&[r_nrm[s].x, r_nrm[s].y, r_nrm[s].z]);
            }
        }

        let (sc, sn, st) = sample_at(zf);
        let slat = st.cross(sn).normalized();
        let slider_base = base_vert + ZIP_TRACK_STATIONS * 4;
        write_box_geometry(
            sc.add(sn.scale(0.0018)),
            slat,
            st,
            sn,
            [hw * 1.45, 0.0055, 0.0020],
            slider_base,
            positions,
            normals,
        );
        let pull_c = sc
            .add(sn.scale(0.0038))
            .sub(st.scale(0.0090));
        write_box_geometry(
            pull_c,
            slat,
            st,
            sn,
            [hw * 0.95, 0.0075, 0.0009],
            slider_base + 8,
            positions,
            normals,
        );
    }
}

/// Button closure + 3D cylindrical disc button flush with the outer garment surface.
pub struct ButtonSim {
    pub piece_idx: u32,
    pub closure: ButtonClosure,
    pub anchor_vert: usize,
    pub diameter_m: f32,
    pub color_rgb: [f32; 3],
}

impl ButtonSim {
    pub fn render_vert_count(&self) -> usize {
        BTN_SIDES * 2 + 2
    }

    pub fn append_indices(&self, base_vert: u32, out: &mut Vec<u32>) {
        let sides = BTN_SIDES as u32;
        let top_c = base_vert + sides * 2;
        let bot_c = top_c + 1;
        for s in 0..sides {
            let s1 = (s + 1) % sides;
            let t0 = base_vert + s;
            let t1 = base_vert + s1;
            let b0 = base_vert + sides + s;
            let b1 = base_vert + sides + s1;
            out.extend_from_slice(&[top_c, t0, t1]);
            out.extend_from_slice(&[bot_c, b1, b0]);
            out.extend_from_slice(&[t0, b0, b1, t0, b1, t1]);
        }
    }

    pub fn append_colors(&self, out: &mut Vec<f32>) {
        for _ in 0..self.render_vert_count() {
            out.extend_from_slice(&self.color_rgb);
        }
    }

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
        if self.anchor_vert >= cloth_pos.len() {
            return;
        }
        let p = cloth_pos[self.anchor_vert];
        let nrm = outward_surface_normal(p, cloth_normals, self.anchor_vert, avatar);
        let half_t = 0.5 * vert_thick.get(self.anchor_vert).copied().unwrap_or(0.001);
        let bot = p.add(nrm.scale(half_t + 0.0006));
        let top = bot.add(nrm.scale(0.0026));
        let up = Vec3::new(0.0, 1.0, 0.0);
        let mut t1 = up.cross(nrm);
        if t1.len_sq() < 1e-6 {
            t1 = Vec3::new(1.0, 0.0, 0.0).cross(nrm);
        }
        t1 = t1.normalized();
        let t2 = nrm.cross(t1).normalized();
        let rad = (0.5 * self.diameter_m).max(0.004);
        for s in 0..BTN_SIDES {
            let ang = std::f32::consts::TAU * (s as f32 / BTN_SIDES as f32);
            let dir = t1.scale(ang.cos()).add(t2.scale(ang.sin()));
            let pt = top.add(dir.scale(rad));
            let pb = bot.add(dir.scale(rad));
            let dt = base_vert + s;
            let db = base_vert + BTN_SIDES + s;
            positions[dt * 3..dt * 3 + 3].copy_from_slice(&[pt.x, pt.y, pt.z]);
            normals[dt * 3..dt * 3 + 3].copy_from_slice(&[nrm.x, nrm.y, nrm.z]);
            positions[db * 3..db * 3 + 3].copy_from_slice(&[pb.x, pb.y, pb.z]);
            normals[db * 3..db * 3 + 3].copy_from_slice(&[-nrm.x, -nrm.y, -nrm.z]);
        }
        let top_c = base_vert + BTN_SIDES * 2;
        let bot_c = top_c + 1;
        positions[top_c * 3..top_c * 3 + 3].copy_from_slice(&[top.x, top.y, top.z]);
        normals[top_c * 3..top_c * 3 + 3].copy_from_slice(&[nrm.x, nrm.y, nrm.z]);
        positions[bot_c * 3..bot_c * 3 + 3].copy_from_slice(&[bot.x, bot.y, bot.z]);
        normals[bot_c * 3..bot_c * 3 + 3].copy_from_slice(&[-nrm.x, -nrm.y, -nrm.z]);
    }
}

/// Arched waistband belt loops (*passants*) spanning from `bottom_vert` to `top_vert`.
pub struct BeltLoopSim {
    pub piece_idx: u32,
    pub loops: Vec<(usize, usize)>,
    pub width_m: f32,
    pub arch_m: f32,
    pub color_rgb: [f32; 3],
}

impl BeltLoopSim {
    pub fn render_vert_count(&self) -> usize {
        self.loops.len() * BELT_LOOP_STATIONS * 4
    }

    pub fn append_indices(&self, base_vert: u32, out: &mut Vec<u32>) {
        let mut cursor = base_vert;
        for _ in 0..self.loops.len() {
            for k in 0..(BELT_LOOP_STATIONS as u32 - 1) {
                let r0 = cursor + k * 4;
                let r1 = cursor + (k + 1) * 4;
                for s in 0..4u32 {
                    let s1 = (s + 1) % 4;
                    out.extend_from_slice(&[r0 + s, r0 + s1, r1 + s1, r0 + s, r1 + s1, r1 + s]);
                }
            }
            cursor += (BELT_LOOP_STATIONS as u32) * 4;
        }
    }

    pub fn append_colors(&self, out: &mut Vec<f32>) {
        for _ in 0..self.render_vert_count() {
            out.extend_from_slice(&self.color_rgb);
        }
    }

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
        let hw = (0.5 * self.width_m).max(0.003);
        let ht = 0.0012_f32;
        let mut cursor = base_vert;
        for &(vb, vt) in &self.loops {
            let pb = cloth_pos[vb];
            let pt = cloth_pos[vt];
            let mid = pb.add(pt).scale(0.5);
            let nrm = outward_surface_normal(mid, cloth_normals, vt, avatar);
            let half_t = 0.5 * vert_thick.get(vt).copied().unwrap_or(0.001);
            let mut up = pt.sub(pb);
            if up.len_sq() < 1e-8 {
                up = Vec3::new(0.0, 1.0, 0.0);
            } else {
                up = up.normalized();
            }
            let lat = up.cross(nrm).normalized();
            for k in 0..BELT_LOOP_STATIONS {
                let u = k as f32 / (BELT_LOOP_STATIONS - 1) as f32;
                let arch = self.arch_m * (std::f32::consts::PI * u).sin();
                let c = pb.scale(1.0 - u).add(pt.scale(u)).add(nrm.scale(half_t + 0.0008 + arch));
                let ring = [
                    c.sub(lat.scale(hw)).add(nrm.scale(ht)),
                    c.add(lat.scale(hw)).add(nrm.scale(ht)),
                    c.add(lat.scale(hw)).sub(nrm.scale(ht)),
                    c.sub(lat.scale(hw)).sub(nrm.scale(ht)),
                ];
                let r_nrm = [nrm, nrm, nrm.scale(-1.0), nrm.scale(-1.0)];
                for s in 0..4 {
                    let dst = cursor + k * 4 + s;
                    positions[dst * 3..dst * 3 + 3]
                        .copy_from_slice(&[ring[s].x, ring[s].y, ring[s].z]);
                    normals[dst * 3..dst * 3 + 3]
                        .copy_from_slice(&[r_nrm[s].x, r_nrm[s].y, r_nrm[s].z]);
                }
            }
            cursor += BELT_LOOP_STATIONS * 4;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosserat_zipper_opens_and_closes() {
        let n = 8usize;
        let mut pos = Vec::with_capacity(n * 2);
        for k in 0..n {
            pos.push(Vec3::new(-0.02, k as f32 * 0.025, 0.0));
        }
        for k in 0..n {
            pos.push(Vec3::new(0.02, k as f32 * 0.025, 0.0));
        }
        let inv_mass = vec![20.0_f32; n * 2];
        let left: Vec<usize> = (0..n).collect();
        let right: Vec<usize> = (n..2 * n).collect();
        let mut zip = ZipperClosure::new(&pos, left.clone(), right.clone(), 1200.0, 0.08);
        zip.zip_fraction = 1.0;

        for _ in 0..24 {
            zip.step(&mut pos, &inv_mass, 1.0 / 120.0, Vec3::ZERO);
        }
        for k in 0..n {
            let gap = pos[left[k]].sub(pos[right[k]]).len();
            assert!(gap < 0.002, "Closed zipper gap at {k}: {gap:.4} m");
        }

        // Now test open zipper (zip_fraction = 0.0): top stays free when pulled apart.
        let mut pos_open = pos.clone();
        pos_open[left[n - 1]].x = -0.05;
        pos_open[right[n - 1]].x = 0.05;
        zip.zip_fraction = 0.0;
        zip.step(&mut pos_open, &inv_mass, 1.0 / 120.0, Vec3::ZERO);
        let top_gap = pos_open[left[n - 1]].sub(pos_open[right[n - 1]]).len();
        assert!(top_gap > 0.08, "Open zipper top should remain free, got {top_gap:.4} m");
    }

    #[test]
    fn test_button_closure_holds_placket_tension() {
        let mut pos = vec![
            Vec3::new(-0.05, 0.0, 0.0), // post on underlap
            Vec3::new(0.05, 0.01, 0.0), // ring 0 on overlap
            Vec3::new(0.05, -0.01, 0.0), // ring 1 on overlap
        ];
        let inv_mass = vec![10.0, 10.0, 10.0];
        let btn = ButtonClosure {
            post_vert: 0,
            ring_verts: vec![1, 2],
            shank_m: 0.004,
        };
        for _ in 0..10 {
            btn.solve(&mut pos, &inv_mass);
        }
        let center = pos[1].add(pos[2]).scale(0.5);
        let dist = pos[0].sub(center).len();
        assert!(dist <= btn.shank_m + 1e-3, "Button stretched to {dist:.4} m");
    }
}

