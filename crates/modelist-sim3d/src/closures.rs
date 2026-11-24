//! Multi-layer garment ordering and hardware/closure physics (*braguettes*, zippers, buttons).
//!
//! Combines explicit layer-order indices with 1D Stable Cosserat Rods (zipper spines) and
//! ring-to-post button couplings so multi-layered plackets and flies stay ordered and bear
//! tension without expensive offline C-IPC.

use crate::avatar::Vec3;
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

