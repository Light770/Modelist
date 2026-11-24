//! Stable Cosserat Rods (Hsu, Wang, Wu, Yuksel — SIGGRAPH 2025, Algorithm 1 & 2).
//!
//! Clean-room Rust implementation of the split position / quasi-static quaternion orientation
//! optimization for 1D elastic trims (drawstrings, corset boning, straps, piping).
//! - Position step: VBD diagonal Hessian + exact stretch/shear gradient (Eq. 28-29).
//! - Orientation step: closed-form Gauss-Seidel local relaxation (Eq. 15-18, Eq. 22) with
//!   optional fixed-point `iterate_lambda` refinement (Algorithm 1, Eq. 25-27).

use crate::avatar::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Quat {
    pub const IDENTITY: Self = Self {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub fn from_imag(v: Vec3) -> Self {
        Self {
            w: 0.0,
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }

    #[inline]
    pub fn mul(self, r: Self) -> Self {
        Self {
            w: self.w * r.w - self.x * r.x - self.y * r.y - self.z * r.z,
            x: self.w * r.x + self.x * r.w + self.y * r.z - self.z * r.y,
            y: self.w * r.y - self.x * r.z + self.y * r.w + self.z * r.x,
            z: self.w * r.z + self.x * r.y - self.y * r.x + self.z * r.w,
        }
    }

    #[inline]
    pub fn conj(self) -> Self {
        Self {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }

    #[inline]
    pub fn add(self, o: Self) -> Self {
        Self {
            w: self.w + o.w,
            x: self.x + o.x,
            y: self.y + o.y,
            z: self.z + o.z,
        }
    }

    #[inline]
    pub fn sub(self, o: Self) -> Self {
        Self {
            w: self.w - o.w,
            x: self.x - o.x,
            y: self.y - o.y,
            z: self.z - o.z,
        }
    }

    #[inline]
    pub fn scale(self, s: f32) -> Self {
        Self {
            w: self.w * s,
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    #[inline]
    pub fn norm_sq(self) -> f32 {
        self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z
    }

    #[inline]
    pub fn norm(self) -> f32 {
        self.norm_sq().sqrt()
    }

    #[inline]
    pub fn normalized(self) -> Self {
        let n = self.norm();
        if n < 1e-12 {
            Self::IDENTITY
        } else {
            self.scale(1.0 / n)
        }
    }

    /// Rotates world basis `e3 = (0, 0, 1)` by `self`: `d3 = q * e3 * conj(q)`.
    #[inline]
    pub fn rotate_e3(self) -> Vec3 {
        Vec3::new(
            2.0 * (self.x * self.z + self.w * self.y),
            2.0 * (self.y * self.z - self.w * self.x),
            self.w * self.w - self.x * self.x - self.y * self.y + self.z * self.z,
        )
    }

    /// Shortest-arc quaternion rotating `e3 = (0, 0, 1)` onto unit vector `dir`.
    pub fn from_e3_to(dir: Vec3) -> Self {
        let d = dir.normalized();
        let dot = d.z.clamp(-1.0, 1.0);
        if dot > 0.99999 {
            return Self::IDENTITY;
        }
        if dot < -0.99999 {
            return Self {
                w: 0.0,
                x: 1.0,
                y: 0.0,
                z: 0.0,
            };
        }
        let axis = Vec3::new(0.0, 0.0, 1.0).cross(d);
        Self {
            w: 1.0 + dot,
            x: axis.x,
            y: axis.y,
            z: axis.z,
        }
        .normalized()
    }
}

const E3_QUAT: Quat = Quat {
    w: 0.0,
    x: 0.0,
    y: 0.0,
    z: 1.0,
};

/// 1D Cosserat rod discretized as `N` vertices and `N - 1` oriented segments.
pub struct CosseratRod {
    pub pos: Vec<Vec3>,
    pub prev: Vec<Vec3>,
    pub inv_mass: Vec<f32>,
    pub q: Vec<Quat>,
    pub q0: Vec<Quat>,
    pub gamma: Vec<f32>,
    pub rest_len: Vec<f32>,
    pub k_ss: Vec<f32>,
    pub k_bt: Vec<f32>,
}

impl CosseratRod {
    pub fn new(pos: Vec<Vec3>, mass_per_vert: f32, k_ss_val: f32, k_bt_val: f32) -> Self {
        let n = pos.len();
        let n_seg = n.saturating_sub(1);
        let mut q = Vec::with_capacity(n_seg);
        let mut rest_len = Vec::with_capacity(n_seg);
        for i in 0..n_seg {
            let d = pos[i + 1].sub(pos[i]);
            let l = d.len().max(1e-5);
            rest_len.push(l);
            q.push(Quat::from_e3_to(d));
        }
        let mut q0 = Vec::with_capacity(n_seg.saturating_sub(1));
        for i in 0..n_seg.saturating_sub(1) {
            q0.push(q[i].conj().mul(q[i + 1]));
        }
        let inv_m = if mass_per_vert > 0.0 {
            1.0 / mass_per_vert
        } else {
            0.0
        };
        Self {
            prev: pos.clone(),
            pos,
            inv_mass: vec![inv_m; n],
            q,
            q0,
            gamma: vec![1.0; n_seg],
            rest_len,
            k_ss: vec![k_ss_val; n_seg],
            k_bt: vec![k_bt_val; n_seg.saturating_sub(1)],
        }
    }

    /// Advances the rod by `dt` seconds using Algorithm 2 (Hsu et al. 2025).
    pub fn step(&mut self, dt: f32, gravity: Vec3, iterations: u32, exact_lambda: bool) {
        let n = self.pos.len();
        let n_seg = self.q.len();
        if n < 2 || dt <= 0.0 {
            return;
        }
        let inv_dt2 = 1.0 / (dt * dt);
        let mut y_target = vec![Vec3::ZERO; n];
        for i in 0..n {
            if self.inv_mass[i] <= 0.0 {
                y_target[i] = self.pos[i];
                self.prev[i] = self.pos[i];
            } else {
                let v_dt = self.pos[i].sub(self.prev[i]).scale(0.995);
                y_target[i] = self.pos[i].add(v_dt).add(gravity.scale(dt * dt));
                self.prev[i] = self.pos[i];
                self.pos[i] = y_target[i];
            }
        }

        for _ in 0..iterations {
            // 1. VBD position update (Eq. 28-29)
            for i in 0..n {
                if self.inv_mass[i] <= 0.0 {
                    continue;
                }
                let m_dt2 = (1.0 / self.inv_mass[i]) * inv_dt2;
                let mut grad = self.pos[i].sub(y_target[i]).scale(m_dt2);
                let mut h_iso = m_dt2;

                if i > 0 {
                    let s = i - 1;
                    let l = self.rest_len[s];
                    let k = self.k_ss[s];
                    let c = self.pos[i].sub(self.pos[i - 1]).scale(1.0 / l).sub(self.q[s].rotate_e3());
                    grad = grad.add(c.scale(k / l));
                    h_iso += k / (l * l);
                }
                if i < n_seg {
                    let s = i;
                    let l = self.rest_len[s];
                    let k = self.k_ss[s];
                    let c = self.pos[i + 1].sub(self.pos[i]).scale(1.0 / l).sub(self.q[s].rotate_e3());
                    grad = grad.sub(c.scale(k / l));
                    h_iso += k / (l * l);
                }
                self.pos[i] = self.pos[i].sub(grad.scale(1.0 / h_iso.max(1e-8)));
            }

            // 2. Closed-form Gauss-Seidel quasi-static orientation update (Eq. 15-18, 22, Alg. 1)
            for i in 0..n_seg {
                let l = self.rest_len[i];
                let v = Quat::from_imag(self.pos[i + 1].sub(self.pos[i]).scale(-2.0 * self.k_ss[i] / l));
                let mut b = Quat {
                    w: 0.0,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                };
                if i > 0 {
                    let q_prev_target = self.q[i - 1].mul(self.q0[i - 1]);
                    let phi = if self.q[i].sub(q_prev_target).norm_sq()
                        <= self.q[i].add(q_prev_target).norm_sq()
                    {
                        1.0
                    } else {
                        -1.0
                    };
                    b = b.add(q_prev_target.scale(self.k_bt[i - 1] * phi));
                }
                if i + 1 < n_seg {
                    let q_next_target = self.q[i + 1].mul(self.q0[i].conj());
                    let phi = if self.q[i].sub(q_next_target).norm_sq()
                        <= self.q[i].add(q_next_target).norm_sq()
                    {
                        1.0
                    } else {
                        -1.0
                    };
                    b = b.add(q_next_target.scale(self.k_bt[i] * phi));
                }

                let v_norm = v.norm();
                let b_norm = b.norm();
                let vbe3 = v.mul(b).mul(E3_QUAT);
                let lambda = if exact_lambda && b_norm > 1e-12 {
                    let lam_g = v_norm + self.gamma[i].clamp(1e-3, 1.0) * b_norm;
                    let lam_fp = (vbe3.add(b.scale(lam_g)).norm() + v_norm * v_norm).sqrt();
                    self.gamma[i] = ((lam_fp - v_norm) / b_norm).clamp(1e-3, 1.0);
                    lam_fp
                } else {
                    v_norm + b_norm
                };

                let q_new = vbe3.add(b.scale(lambda));
                if q_new.norm_sq() > 1e-20 {
                    self.q[i] = q_new.normalized();
                } else {
                    self.q[i] = Quat::from_e3_to(self.pos[i + 1].sub(self.pos[i]));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosserat_unit_quaternion_and_rest_invariance() {
        let pos: Vec<Vec3> = (0..10).map(|i| Vec3::new(0.0, 0.0, i as f32 * 0.05)).collect();
        let mut rod = CosseratRod::new(pos.clone(), 0.02, 500.0, 0.05);
        rod.inv_mass[0] = 0.0;
        for _ in 0..10 {
            rod.step(1.0 / 60.0, Vec3::ZERO, 4, false);
        }
        for q in &rod.q {
            assert!((q.norm() - 1.0).abs() < 1e-6);
        }
        for (a, b) in pos.iter().zip(&rod.pos) {
            assert!(a.sub(*b).len() < 1e-5);
        }
    }

    #[test]
    fn test_cosserat_cantilever_stable_at_large_timestep() {
        let pos_init: Vec<Vec3> = (0..16).map(|i| Vec3::new(i as f32 * 0.03, 0.0, 0.0)).collect();
        let mut rod_approx = CosseratRod::new(pos_init.clone(), 0.015, 2000.0, 0.25);
        let mut rod_exact = CosseratRod::new(pos_init, 0.015, 2000.0, 0.25);
        rod_approx.inv_mass[0] = 0.0;
        rod_exact.inv_mass[0] = 0.0;

        let g = Vec3::new(0.0, -9.81, 0.0);
        for _ in 0..30 {
            rod_approx.step(1.0 / 30.0, g, 4, false);
            rod_exact.step(1.0 / 30.0, g, 4, true);
        }
        let tip_a = rod_approx.pos.last().unwrap();
        let tip_e = rod_exact.pos.last().unwrap();
        assert!(tip_a.y.is_finite() && tip_a.y < -0.005);
        assert!((tip_a.y - tip_e.y).abs() < 0.01, "approx={} exact={}", tip_a.y, tip_e.y);
    }
}
