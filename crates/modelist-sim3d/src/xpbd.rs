//! Anisotropic XPBD cloth solver (CPU / WASM fallback path).
//!
//! Physics model, all in SI units from [`FabricSpec`] (via [`SolverMaterial`]):
//! - **Membrane**: per-triangle strain-based constraints in the grain frame: warp stretch, weft
//!   stretch and warp/weft shear, each with compliance `1 / (stiffness * area)`. Strain past the
//!   fabric's lock point stiffens 40x (yarn jamming); compression is softened by the buckling
//!   ratio so wrinkles form as soft folds instead of sharp creases.
//! - **Bending**: quadratic isometric bending (Bergou et al. 2006) on every interior hinge with
//!   compliance `(A0 + A1) / B`, where `B` blends warp/weft bending rigidity by hinge direction.
//!   That scale reproduces the continuum energy `B * kappa^2 / 2` per unit area on a cylinder.
//! - **Sewing**: barycentric seam and dart constraints, ramped in gently while panels close.
//! - **Contact**: vertex-vs-SDF with cached contact planes and Coulomb friction, plus body-point
//!   vs cloth-triangle pushes for small features (see [`crate::collide`]).
//!
//! Substep-based XPBD ("small steps"): one constraint pass per substep, 12 substeps per frame.

use crate::arrange::{
    close_darts_for_placement, place_panel, place_trim, ArrangeCtx, InnerShell, TrimAnchor,
};
use crate::avatar::{AvatarBodyForm, AvatarMesh, AvatarPose, ParametricAvatar, Vec3};
use crate::collide::{push_triangle_off_points, BodyPointGrid};
use crate::diagnostics::{drape_quality, DrapeQuality};
use crate::material::{MaterialSet, SolverMaterial, LOCK_STIFFENING};
use crate::mesher::{mesh_pattern_piece, PanelMesh};
use modelist_core::curve::edge_total_length;
use modelist_core::model::{
    ArrangementZone, BodyMeasurements, EdgeRef, FabricSpec, PatternPiece, Point2, StitchingGraph,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// XPBD substeps per 60 Hz frame.
pub const SUBSTEPS: u32 = 12;
/// Target cloth mesh edge length, mm.
pub const MESH_SPACING_MM: f64 = 20.0;
/// Frames of gentle seam closing under low gravity.
const SEW_FRAMES: u32 = 30;
/// Frames over which gravity ramps up to full once seams are closed.
const SETTLE_FRAMES: u32 = 15;
/// Frames simulated before the first render so seams are already closed.
const INIT_FRAMES: u32 = SEW_FRAMES + 10;
/// Cloth triangles closer than this to the body get the body-point test, m.
const NEAR_BAND: f32 = 0.012;
/// Largest displacement a vertex may take in one substep, m (explosion guard).
const MAX_STEP: f32 = 0.02;
/// Largest outward speed a contact may give a vertex in one substep, m (no bouncing).
const MAX_SEPARATION: f32 = 0.0003;
/// How far a supported waistline may sink below where it was placed, m.
const WAIST_SLACK: f32 = 0.015;
/// Clearance an outer garment layer is placed at around the inner layer's cloth, m.
const LAYER_PLACE_GAP: f32 = 0.010;
/// Air gap kept between two garment layers on top of their half thicknesses, m.
const LAYER_AIR_GAP: f32 = 0.0012;
/// Search radius for the cloth of another layer, m.
const LAYER_SEARCH: f32 = 0.03;
/// Deepest a vertex may sit on the wrong side of another layer and still be pushed back, m.
/// Deeper "hits" are cloth on the far side of a fold or limb, not a crossing.
const LAYER_MAX_DEPTH: f32 = 0.02;
/// Least cosine between a vertex normal and another layer's triangle normal for the two to be
/// stacked. Parts that sit side by side (an inner sleeve against an outer side panel at the
/// armpit) face each other and are not layered relative to one another.
const LAYER_MIN_ALIGN: f32 = 0.3;
/// Largest push a layer contact applies per substep, m: small overlaps left by the placement
/// resolve over a few frames instead of in one jolt.
const LAYER_MAX_PUSH: f32 = 0.001;
/// Longest gentle sewing phase of an outer layer, frames. An outer garment is wrapped around
/// the bigger inner one, so its seams start wide open; strong sewing only starts once they meet.
const LAYER_MAX_SEW_FRAMES: u32 = 150;
/// Seam gap below which an outer layer's gentle sewing phase may end, mm.
const LAYER_SEWN_GAP_MM: f32 = 12.0;
/// Frames all layers then settle together.
const LAYER_COUPLE_FRAMES: u32 = 10;

/// Pins vertex `a` to the point `(1 - w) * b0 + w * b1` of the mating seam.
#[derive(Debug, Clone)]
struct BarySew {
    a: usize,
    b0: usize,
    b1: usize,
    w: f32,
}

/// `(piece, side, dart, leg)` -> leg vertices with their fraction from edge (0) to apex (1).
type DartLegMap = HashMap<(Uuid, i8, Uuid, u8), Vec<(usize, f64)>>;

#[derive(Debug, Clone)]
struct BoundVert {
    global_idx: usize,
    sewn_frac: f64,
}

/// Membrane element: `f_u = sum cu_i x_i` and `f_v = sum cv_i x_i` are the deformed warp and weft
/// directions (columns of the deformation gradient).
#[derive(Debug, Clone)]
pub(crate) struct TriElem {
    pub v: [usize; 3],
    pub cu: [f32; 3],
    pub cv: [f32; 3],
    pub area: f32,
    pub slot: u16,
    /// Rest positions in the grain frame `(warp, weft)`, metres.
    pub ruv: [(f32, f32); 3],
}

#[derive(Debug, Clone)]
pub(crate) struct HingeElem {
    pub v: [usize; 4],
    pub k: [f32; 4],
    /// Rest area of the two triangles, m2.
    pub area2: f32,
    /// sin^2 of the angle between the hinge edge and the warp (1 = warp yarns are bent).
    pub warp_w: f32,
    pub slot: u16,
}

/// One-way long-range stretch limit between two vertices of the same panel, aligned with the
/// warp or the weft. An inextensible sheet can bend and fold but the straight-line distance
/// between two of its points never exceeds their flat distance, so the tether only engages once
/// the yarns pass their lock strain. It makes stretch converge across a whole panel in a single
/// pass instead of one triangle per iteration.
#[derive(Debug, Clone)]
pub(crate) struct Tether {
    pub i: usize,
    pub j: usize,
    pub rest: f32,
    /// 0 = warp-aligned, 1 = weft-aligned
    pub axis: u8,
    pub slot: u16,
    /// Fabric width this tether stands for, m. Sized from the mesh density so that all tethers
    /// crossing a section add up to the fabric's real tensile stiffness.
    pub width: f32,
}

/// Mesh edge with a hard stretch limit (Provot strain limiting). Its limit blends the fabric's
/// warp and weft lock strain by direction, plus the extra bias extension that shear allows.
#[derive(Debug, Clone)]
pub(crate) struct EdgeLimit {
    pub i: usize,
    pub j: usize,
    pub rest: f32,
    /// cos^2 of the angle to the warp.
    pub warp_c2: f32,
    pub slot: u16,
}

/// Extra extension a 45-degree bias edge may take through yarn shear before it locks.
const BIAS_EXTENSION: f32 = 0.12;

/// Length limit on a whole seam line (one panel edge, darts folded): a drawstring that pulls
/// waistlines, necklines and hems back to their sewn length in one projection, which per-edge
/// passes cannot do for a stretched hoop.
#[derive(Debug, Clone)]
pub(crate) struct ChainLimit {
    pub verts: Vec<usize>,
    pub max_len: f32,
}
/// Strain-limit passes per substep.
const LIMIT_PASSES: u32 = 2;
/// Bending passes per substep (with accumulated multipliers).
pub(crate) const BEND_PASSES: u32 = 2;
/// Bending gain compensating the unconverged XPBD iterations at the shipped substep/pass count.
/// Calibrated so the Peirce cantilever (ASTM D1388) test droops to 41.5 +- 5 degrees at both
/// 12 mm and 20 mm mesh spacing and any fabric weight (see `material_tests`).
const BEND_GAIN: f32 = 2.0;

/// Solver core without the avatar: particles, membrane, bending and sewing. Shared by the
/// fitting session and the material calibration tests.
pub(crate) struct ClothCore {
    pub pos: Vec<Vec3>,
    pub prev: Vec<Vec3>,
    pub inv_mass: Vec<f32>,
    pub pinned: Vec<bool>,
    pub tris: Vec<TriElem>,
    pub hinges: Vec<HingeElem>,
    hinge_lambda: Vec<Vec3>,
    pub tethers: Vec<Tether>,
    pub chains: Vec<ChainLimit>,
    pub edges: Vec<EdgeLimit>,
    edge_max: Vec<f32>,
    tether_max: Vec<f32>,
    /// `(compliance, lock strain)` per tether.
    tether_alpha: Vec<(f32, f32)>,
    sew: Vec<BarySew>,
    pub mats: Vec<SolverMaterial>,
    tri_alpha: Vec<[f32; 3]>,
    hinge_alpha: Vec<f32>,
    vert_damp: Vec<f32>,
}

impl ClothCore {
    pub fn new(
        pos: Vec<Vec3>,
        tris: Vec<TriElem>,
        hinges: Vec<HingeElem>,
        mats: Vec<SolverMaterial>,
    ) -> Self {
        let n = pos.len();
        let mut core = Self {
            prev: pos.clone(),
            pos,
            inv_mass: vec![0.0; n],
            pinned: vec![false; n],
            tris,
            hinges,
            hinge_lambda: Vec::new(),
            tethers: Vec::new(),
            chains: Vec::new(),
            edges: Vec::new(),
            edge_max: Vec::new(),
            tether_max: Vec::new(),
            tether_alpha: Vec::new(),
            sew: Vec::new(),
            mats,
            tri_alpha: Vec::new(),
            hinge_alpha: Vec::new(),
            vert_damp: vec![0.0; n],
        };
        core.refresh_materials();
        core
    }

    /// Recomputes lumped masses, compliances and damping after a material change.
    pub fn refresh_materials(&mut self) {
        let n = self.pos.len();
        let mut mass = vec![0.0_f32; n];
        let mut damp = vec![0.0_f32; n];
        let mut cnt = vec![0u32; n];
        self.tri_alpha.clear();
        for t in &self.tris {
            let m = &self.mats[t.slot as usize];
            let a = t.area.max(1e-8);
            for &vi in &t.v {
                mass[vi] += m.density * a / 3.0;
                damp[vi] += m.damping_rate;
                cnt[vi] += 1;
            }
            self.tri_alpha.push([
                1.0 / (m.e_warp * a),
                1.0 / (m.e_weft * a),
                1.0 / (m.g_shear * a),
            ]);
        }
        self.hinge_alpha = self
            .hinges
            .iter()
            .map(|h| {
                let m = &self.mats[h.slot as usize];
                let b = (m.b_warp * h.warp_w + m.b_weft * (1.0 - h.warp_w)).max(1e-9) * BEND_GAIN;
                h.area2.max(1e-8) / b
            })
            .collect();
        if self.edges.is_empty() && !self.tris.is_empty() {
            self.edges = build_edge_limits(&self.tris);
        }
        self.edge_max = self
            .edges
            .iter()
            .map(|e| {
                let m = &self.mats[e.slot as usize];
                let c2 = e.warp_c2;
                let s2 = 1.0 - c2;
                let rest_scale = m.rest_warp * c2 + m.rest_weft * s2;
                let lock = m.lock_warp * c2 + m.lock_weft * s2 + BIAS_EXTENSION * 4.0 * c2 * s2;
                e.rest * rest_scale * (1.0 + lock)
            })
            .collect();
        self.tether_max = self
            .tethers
            .iter()
            .map(|t| {
                let m = &self.mats[t.slot as usize];
                let rest = if t.axis == 0 { m.rest_warp } else { m.rest_weft };
                t.rest * rest
            })
            .collect();
        self.tether_alpha = self
            .tethers
            .iter()
            .map(|t| {
                let m = &self.mats[t.slot as usize];
                let (e, lock) = if t.axis == 0 {
                    (m.e_warp, m.lock_warp)
                } else {
                    (m.e_weft, m.lock_weft)
                };
                // A strip of the tether's share of width acting over its length: k = E * w / L.
                (t.rest / (e * t.width.max(1e-6)), lock)
            })
            .collect();
        for i in 0..n {
            self.inv_mass[i] = if self.pinned[i] {
                0.0
            } else {
                1.0 / mass[i].max(1e-7)
            };
            self.vert_damp[i] = if cnt[i] > 0 {
                damp[i] / cnt[i] as f32
            } else {
                2.0
            };
        }
    }

    /// Integration with an extra per-substep velocity factor (quasi-static phases use < 1).
    pub fn integrate_damped(&mut self, dt: f32, gravity: Vec3, extra: f32) {
        let g = gravity.scale(dt * dt);
        for i in 0..self.pos.len() {
            if self.inv_mass[i] <= 0.0 {
                self.prev[i] = self.pos[i];
                continue;
            }
            let decay = (-self.vert_damp[i] * dt).exp() * extra;
            let mut v = self.pos[i].sub(self.prev[i]).scale(decay);
            let vl = v.len();
            if vl > MAX_STEP {
                v = v.scale(MAX_STEP / vl);
            }
            self.prev[i] = self.pos[i];
            self.pos[i] = self.pos[i].add(v).add(g);
        }
    }

    pub fn solve_sew(&mut self, max_pull: f32) {
        let pos = &mut self.pos;
        let inv = &self.inv_mass;
        for s in &self.sew {
            let pb = pos[s.b0].scale(1.0 - s.w).add(pos[s.b1].scale(s.w));
            let d = pb.sub(pos[s.a]);
            let len = d.len();
            if len < 1e-7 {
                continue;
            }
            let wa = inv[s.a];
            let wb0 = inv[s.b0] * (1.0 - s.w);
            let wb1 = inv[s.b1] * s.w;
            let wsum = wa + (1.0 - s.w) * wb0 + s.w * wb1;
            if wsum <= 1e-12 {
                continue;
            }
            let scale = if len > max_pull { max_pull / len } else { 1.0 };
            let corr = d.scale(scale / wsum);
            pos[s.a] = pos[s.a].add(corr.scale(wa));
            pos[s.b0] = pos[s.b0].sub(corr.scale(wb0));
            pos[s.b1] = pos[s.b1].sub(corr.scale(wb1));
        }
    }

    pub fn solve_membrane(&mut self, dt: f32) {
        let inv_dt2 = 1.0 / (dt * dt);
        let pos = &mut self.pos;
        let inv = &self.inv_mass;
        for (t, alpha) in self.tris.iter().zip(&self.tri_alpha) {
            let m = &self.mats[t.slot as usize];
            let [i0, i1, i2] = t.v;
            let w = [inv[i0], inv[i1], inv[i2]];
            if w[0] + w[1] + w[2] <= 0.0 {
                continue;
            }
            // Warp and weft stretch.
            for (axis, c, rest, lock, a0) in [
                (0, &t.cu, m.rest_warp, m.lock_warp, alpha[0]),
                (1, &t.cv, m.rest_weft, m.lock_weft, alpha[1]),
            ] {
                let _ = axis;
                let f = pos[i0]
                    .scale(c[0])
                    .add(pos[i1].scale(c[1]))
                    .add(pos[i2].scale(c[2]));
                let l = f.len();
                if l < 1e-9 {
                    continue;
                }
                let cval = l - rest;
                let strain = cval / rest;
                let a = if strain > lock {
                    a0 / LOCK_STIFFENING
                } else if cval < 0.0 {
                    a0 / m.buckling
                } else {
                    a0
                };
                let wsum = w[0] * c[0] * c[0] + w[1] * c[1] * c[1] + w[2] * c[2] * c[2];
                let dl = -cval / (wsum + a * inv_dt2);
                let n = f.scale(dl / l);
                pos[i0] = pos[i0].add(n.scale(w[0] * c[0]));
                pos[i1] = pos[i1].add(n.scale(w[1] * c[1]));
                pos[i2] = pos[i2].add(n.scale(w[2] * c[2]));
            }
            // Warp/weft shear: cosine of the angle between the deformed yarn directions.
            let fu = pos[i0]
                .scale(t.cu[0])
                .add(pos[i1].scale(t.cu[1]))
                .add(pos[i2].scale(t.cu[2]));
            let fv = pos[i0]
                .scale(t.cv[0])
                .add(pos[i1].scale(t.cv[1]))
                .add(pos[i2].scale(t.cv[2]));
            let (lu, lv) = (fu.len(), fv.len());
            if lu < 1e-9 || lv < 1e-9 {
                continue;
            }
            let nu = fu.scale(1.0 / lu);
            let nv = fv.scale(1.0 / lv);
            let cval = nu.dot(nv);
            let dcdu = nv.sub(nu.scale(cval)).scale(1.0 / lu);
            let dcdv = nu.sub(nv.scale(cval)).scale(1.0 / lv);
            let g = [
                dcdu.scale(t.cu[0]).add(dcdv.scale(t.cv[0])),
                dcdu.scale(t.cu[1]).add(dcdv.scale(t.cv[1])),
                dcdu.scale(t.cu[2]).add(dcdv.scale(t.cv[2])),
            ];
            let wsum = w[0] * g[0].len_sq() + w[1] * g[1].len_sq() + w[2] * g[2].len_sq();
            if wsum < 1e-12 {
                continue;
            }
            let dl = -cval / (wsum + alpha[2] * inv_dt2);
            pos[i0] = pos[i0].add(g[0].scale(w[0] * dl));
            pos[i1] = pos[i1].add(g[1].scale(w[1] * dl));
            pos[i2] = pos[i2].add(g[2].scale(w[2] * dl));
        }
    }

    /// Pulls every over-long seam line back to its maximum length (one projection per line).
    pub fn solve_chain_limits(&mut self) {
        let pos = &mut self.pos;
        let inv = &self.inv_mass;
        let mut grad: Vec<Vec3> = Vec::new();
        for c in &self.chains {
            let n = c.verts.len();
            if n < 2 {
                continue;
            }
            let mut len = 0.0;
            grad.clear();
            grad.resize(n, Vec3::ZERO);
            for k in 0..n - 1 {
                let d = pos[c.verts[k + 1]].sub(pos[c.verts[k]]);
                let l = d.len();
                len += l;
                if l > 1e-9 {
                    let u = d.scale(1.0 / l);
                    grad[k] = grad[k].sub(u);
                    grad[k + 1] = grad[k + 1].add(u);
                }
            }
            let cval = len - c.max_len;
            if cval <= 0.0 {
                continue;
            }
            let mut wsum = 0.0;
            for k in 0..n {
                wsum += inv[c.verts[k]] * grad[k].len_sq();
            }
            if wsum < 1e-12 {
                continue;
            }
            let dl = -cval / wsum;
            for (&v, g) in c.verts.iter().zip(&grad) {
                pos[v] = pos[v].add(g.scale(inv[v] * dl));
            }
        }
    }

    /// Clamps every over-stretched mesh edge back to its lock length.
    pub fn solve_strain_limits(&mut self) {
        let pos = &mut self.pos;
        let inv = &self.inv_mass;
        for _ in 0..LIMIT_PASSES {
            for (e, &max_len) in self.edges.iter().zip(&self.edge_max) {
                let d = pos[e.j].sub(pos[e.i]);
                let l2 = d.len_sq();
                if l2 <= max_len * max_len {
                    continue;
                }
                let wsum = inv[e.i] + inv[e.j];
                if wsum <= 0.0 {
                    continue;
                }
                let l = l2.sqrt();
                let corr = d.scale((l - max_len) / (l * wsum));
                pos[e.i] = pos[e.i].add(corr.scale(inv[e.i]));
                pos[e.j] = pos[e.j].sub(corr.scale(inv[e.j]));
            }
        }
    }

    pub fn solve_tethers(&mut self, dt: f32) {
        let inv_dt2 = 1.0 / (dt * dt);
        let pos = &mut self.pos;
        let inv = &self.inv_mass;
        for ((t, &rest), &(alpha, lock)) in self
            .tethers
            .iter()
            .zip(&self.tether_max)
            .zip(&self.tether_alpha)
        {
            let d = pos[t.j].sub(pos[t.i]);
            let l = d.len();
            if l <= rest || l < 1e-9 {
                continue; // one-way: folding and bending never engage a tether
            }
            let wsum = inv[t.i] + inv[t.j];
            if wsum <= 0.0 {
                continue;
            }
            let c = l - rest;
            let a = if c / rest > lock { alpha / LOCK_STIFFENING } else { alpha };
            let dl = c / (wsum + a * inv_dt2);
            let corr = d.scale(dl / l);
            pos[t.i] = pos[t.i].add(corr.scale(inv[t.i]));
            pos[t.j] = pos[t.j].sub(corr.scale(inv[t.j]));
        }
    }

    /// Internal cloth forces for one substep: membrane, long-range tethers, stretch limits and
    /// bending. Sewing and contacts are handled by the caller.
    #[cfg(test)]
    pub fn solve_internal(&mut self, dt: f32) {
        self.begin_substep();
        self.solve_membrane(dt);
        self.solve_tethers(dt);
        self.solve_strain_limits();
        for _ in 0..BEND_PASSES {
            self.solve_bending(dt);
        }
    }

    /// Resets the bending multipliers; call once at the start of every substep.
    pub fn begin_substep(&mut self) {
        self.hinge_lambda.clear();
        self.hinge_lambda.resize(self.hinges.len(), Vec3::ZERO);
    }

    /// One XPBD pass over the bending hinges with accumulated multipliers, so repeated passes
    /// converge to the true compliance instead of over-stiffening.
    pub fn solve_bending(&mut self, dt: f32) {
        if self.hinge_lambda.len() != self.hinges.len() {
            self.begin_substep();
        }
        let inv_dt2 = 1.0 / (dt * dt);
        let pos = &mut self.pos;
        let inv = &self.inv_mass;
        for ((h, &alpha), lam) in self
            .hinges
            .iter()
            .zip(&self.hinge_alpha)
            .zip(self.hinge_lambda.iter_mut())
        {
            let [a, b, c, d] = h.v;
            let k = h.k;
            let cvec = pos[a]
                .scale(k[0])
                .add(pos[b].scale(k[1]))
                .add(pos[c].scale(k[2]))
                .add(pos[d].scale(k[3]));
            let wsum = inv[a] * k[0] * k[0]
                + inv[b] * k[1] * k[1]
                + inv[c] * k[2] * k[2]
                + inv[d] * k[3] * k[3];
            if wsum < 1e-12 {
                continue;
            }
            let at = alpha * inv_dt2;
            let dl = cvec.add(lam.scale(at)).scale(-1.0 / (wsum + at));
            *lam = lam.add(dl);
            pos[a] = pos[a].add(dl.scale(inv[a] * k[0]));
            pos[b] = pos[b].add(dl.scale(inv[b] * k[1]));
            pos[c] = pos[c].add(dl.scale(inv[c] * k[2]));
            pos[d] = pos[d].add(dl.scale(inv[d] * k[3]));
        }
    }

    /// Per-triangle `(warp strain, weft strain)`.
    pub fn tri_strains(&self) -> Vec<(f32, f32)> {
        self.tris
            .iter()
            .map(|t| {
                let m = &self.mats[t.slot as usize];
                let [i0, i1, i2] = t.v;
                let fu = self.pos[i0]
                    .scale(t.cu[0])
                    .add(self.pos[i1].scale(t.cu[1]))
                    .add(self.pos[i2].scale(t.cu[2]));
                let fv = self.pos[i0]
                    .scale(t.cv[0])
                    .add(self.pos[i1].scale(t.cv[1]))
                    .add(self.pos[i2].scale(t.cv[2]));
                (fu.len() / m.rest_warp - 1.0, fv.len() / m.rest_weft - 1.0)
            })
            .collect()
    }
}

/// Builds the grain-frame membrane element for a triangle with 2D rest positions (m).
pub(crate) fn make_tri(v: [usize; 3], p: [Point2; 3], grain: Point2, slot: u16) -> Option<TriElem> {
    let g = grain.normalized();
    let gp = Point2::new(-g.y, g.x);
    let uv = |q: Point2| (q.dot(g), q.dot(gp));
    let (u0, v0) = uv(p[0]);
    let (u1, v1) = uv(p[1]);
    let (u2, v2) = uv(p[2]);
    let (a, b, c, d) = (u1 - u0, u2 - u0, v1 - v0, v2 - v0);
    let det = a * d - b * c;
    if det.abs() < 1e-12 {
        return None;
    }
    let cu1 = (d / det) as f32;
    let cu2 = (-c / det) as f32;
    let cv1 = (-b / det) as f32;
    let cv2 = (a / det) as f32;
    Some(TriElem {
        v,
        cu: [-(cu1 + cu2), cu1, cu2],
        cv: [-(cv1 + cv2), cv1, cv2],
        area: (0.5 * det.abs()) as f32,
        slot,
        ruv: [
            (u0 as f32, v0 as f32),
            (u1 as f32, v1 as f32),
            (u2 as f32, v2 as f32),
        ],
    })
}

fn tri_grain_for_slot(grain: &[Point2], slots: &[u16], slot: u16) -> Point2 {
    slots
        .iter()
        .position(|s| *s == slot)
        .map(|k| grain[k])
        .unwrap_or(Point2::new(0.0, 1.0))
}

/// Unique mesh edges with their rest length and grain direction.
pub(crate) fn build_edge_limits(tris: &[TriElem]) -> Vec<EdgeLimit> {
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut out = Vec::with_capacity(tris.len() * 3 / 2 + 8);
    for t in tris {
        for (a, b) in [(0usize, 1usize), (1, 2), (2, 0)] {
            let (i, j) = (t.v[a], t.v[b]);
            let key = if i < j { (i, j) } else { (j, i) };
            if !seen.insert(key) {
                continue;
            }
            let du = t.ruv[b].0 - t.ruv[a].0;
            let dv = t.ruv[b].1 - t.ruv[a].1;
            let rest = (du * du + dv * dv).sqrt();
            if rest < 1e-6 {
                continue;
            }
            out.push(EdgeLimit {
                i: key.0,
                j: key.1,
                rest,
                warp_c2: (du * du) / (rest * rest),
                slot: t.slot,
            });
        }
    }
    out
}

/// Builds the isometric bending hinge for edge `(x0, x1)` with opposite vertices `x2`, `x3`.
pub(crate) fn make_hinge(v: [usize; 4], p: [Point2; 4], grain: Point2, slot: u16) -> Option<HingeElem> {
    let cot = |o: Point2, a: Point2, b: Point2| -> f64 {
        let u = a.sub(o);
        let w = b.sub(o);
        let cr = u.perp_dot(w).abs();
        if cr < 1e-12 {
            0.0
        } else {
            u.dot(w) / cr
        }
    };
    let [x0, x1, x2, x3] = p;
    let c01 = cot(x0, x1, x2);
    let c02 = cot(x0, x1, x3);
    let c03 = cot(x1, x0, x2);
    let c04 = cot(x1, x0, x3);
    let a0 = 0.5 * x1.sub(x0).perp_dot(x2.sub(x0)).abs();
    let a1 = 0.5 * x1.sub(x0).perp_dot(x3.sub(x0)).abs();
    if a0 < 1e-12 || a1 < 1e-12 {
        return None;
    }
    let k = [c03 + c04, c01 + c02, -c01 - c03, -c02 - c04];
    if k.iter().any(|x| !x.is_finite() || x.abs() > 1e3) {
        return None;
    }
    let e = x1.sub(x0).normalized();
    let cos_g = e.dot(grain.normalized());
    Some(HingeElem {
        v,
        k: [k[0] as f32, k[1] as f32, k[2] as f32, k[3] as f32],
        area2: (a0 + a1) as f32,
        warp_w: (1.0 - cos_g * cos_g).clamp(0.0, 1.0) as f32,
        slot,
    })
}

/// Builds warp/weft-aligned tethers for one panel. `rest` holds 2D rest positions in metres,
/// `grain` the panel's unit warp direction.
pub(crate) fn build_tethers(
    rest: &[Point2],
    range: std::ops::Range<usize>,
    grain: Point2,
    slot: u16,
    vertex_area: f32,
    out: &mut Vec<Tether>,
) {
    const SPANS: [f64; 2] = [0.07, 0.18];
    // Each vertex starts about one tether per span and axis, so a section crossed by tethers of
    // span L sees (L / vertex_area) of them per metre: width = vertex_area / L / spans gives a
    // total of exactly E per metre of fabric.
    let width_for = |span: f64| vertex_area / (span as f32 * SPANS.len() as f32);
    const COS_TOL: f64 = 0.985; // ~10 degrees
    let g = grain.normalized();
    let gp = Point2::new(-g.y, g.x);
    let cell = 0.03;
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for i in range.clone() {
        let p = rest[i];
        grid.entry(((p.x / cell).floor() as i64, (p.y / cell).floor() as i64))
            .or_default()
            .push(i);
    }
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for i in range {
        let p = rest[i];
        for (axis, dir) in [(0u8, g), (1u8, gp)] {
            for sign in [1.0, -1.0] {
                for span in SPANS {
                    let target = p.add(dir.scale(sign * span));
                    let (cx, cy) = ((target.x / cell).floor() as i64, (target.y / cell).floor() as i64);
                    let mut best: Option<(f64, usize)> = None;
                    for gx in cx - 1..=cx + 1 {
                        for gy in cy - 1..=cy + 1 {
                            let Some(list) = grid.get(&(gx, gy)) else { continue };
                            for &j in list {
                                if j == i {
                                    continue;
                                }
                                let d = rest[j].sub(p);
                                let l = d.len();
                                if l < span * 0.7 || l > span * 1.3 {
                                    continue;
                                }
                                if (d.dot(dir) * sign) / l < COS_TOL {
                                    continue;
                                }
                                let err = rest[j].dist(target);
                                if best.is_none_or(|b| err < b.0) {
                                    best = Some((err, j));
                                }
                            }
                        }
                    }
                    if let Some((_, j)) = best {
                        let key = if i < j { (i, j) } else { (j, i) };
                        if seen.insert(key) {
                            out.push(Tether {
                                i,
                                j,
                                rest: rest[i].dist(rest[j]) as f32,
                                axis,
                                slot,
                                width: width_for(span),
                            });
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sim3dStats {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub sew_pair_count: usize,
    pub step_count: u32,
    pub max_strain_pct: f32,
    pub avg_clearance_mm: f32,
    pub min_clearance_mm: f32,
    pub fabric_name: String,
    pub pose: AvatarPose,
    pub body_form: AvatarBodyForm,
    pub bust_y: f32,
    pub waist_y: f32,
    pub hip_y: f32,
    pub crotch_y: f32,
    pub neck_y: f32,
    pub measured_bust_mm: f32,
    pub measured_waist_mm: f32,
    pub measured_hip_mm: f32,
    pub measured_height_mm: f32,
    /// Darts cut into the 3D panels and sewn shut.
    pub darts_closed: usize,
    /// Mean vertex speed over the last frame, mm/s (near 0 once the drape has settled).
    pub motion_mm_s: f32,
    /// While garments are still being put on: the layer being dressed (1-based) and how many
    /// layers there are. Both 0 once dressed.
    #[serde(default)]
    pub dressing_layer: usize,
    #[serde(default)]
    pub dressing_layers: usize,
}

#[derive(Debug, Clone, Copy)]
struct Contact {
    q: Vec3,
    n: Vec3,
    at: Vec3,
    dist: f32,
}

/// What is left to put on when garments are dressed one layer at a time.
struct DressPlan {
    pieces: Vec<PatternPiece>,
    layers: Vec<u8>,
    stitching: StitchingGraph,
    measurements: BodyMeasurements,
    materials: MaterialSet,
    pose: AvatarPose,
    body_form: AvatarBodyForm,
    /// Distinct layers, inner first.
    levels: Vec<u8>,
    /// Index in `levels` of the layer being put on now.
    stage: usize,
    /// Inverse masses to restore when the stage ends (the lower layers are frozen meanwhile).
    saved_inv_mass: Vec<f32>,
}

/// One-sided contact between a cloth vertex and a triangle of another garment layer.
/// `side = +1`: the vertex belongs to an outer layer and must stay outside the triangle;
/// `side = -1`: it belongs to an inner layer and must stay under it.
#[derive(Debug, Clone, Copy)]
struct LayerContact {
    tri: u32,
    bary: [f32; 3],
    side: f32,
    /// Stacked layers (normals aligned): the lower layer has priority and the side is fixed by
    /// the layer order. Otherwise the parts sit side by side and simply keep apart on the side
    /// they are on now, both giving way.
    stacked: bool,
}

struct PanelRange {
    start: usize,
    end: usize,
    tri_start: usize,
    tri_end: usize,
}

/// Strictly decoupled, on-demand anisotropic XPBD cloth simulation of a garment on the avatar.
/// Allocated only while the 3D Fit Room is open; dropped on close.
pub struct ClothSimSession {
    pub avatar: ParametricAvatar,
    pub avatar_mesh: AvatarMesh,
    /// Shell fabric (kept for stats and API compatibility).
    pub fabric: FabricSpec,
    materials: MaterialSet,
    slot_roles: Vec<String>,
    core: ClothCore,
    /// Waist edge chains `(ordered vertices, sewn rest length m)`.
    waist_chains: Vec<(Vec<usize>, f32)>,
    /// Waistline support for lower-body garments: `(waist edge vertices, lowest allowed mean y)`.
    waist_support: Vec<(Vec<usize>, f32)>,
    triangles: Vec<u32>,
    body_points: BodyPointGrid,
    contacts: Vec<Contact>,
    vert_offset: Vec<f32>,
    vert_friction: Vec<f32>,
    tri_near: Vec<bool>,
    scratch: Vec<u32>,
    step_count: u32,
    darts_closed: usize,
    motion_mm_s: f32,
    /// Garment layer of each vertex (0 = closest to the body).
    vert_layer: Vec<u8>,
    /// Index (in the `pieces` the session was built from) of each vertex's piece.
    vert_piece: Vec<u32>,
    /// Distinct layers present, ascending. More than one turns on layer contacts.
    layers: Vec<u8>,
    vert_thickness: Vec<f32>,
    /// Per vertex: the nearest triangle of a lower layer and of a higher layer, refreshed every
    /// frame.
    layer_contacts: Vec<[Option<LayerContact>; 2]>,
    /// How far each vertex can still move towards the body this substep, m (set by the body
    /// contact; layer contacts never push cloth further in than this).
    vert_room: Vec<f32>,
    /// Frames of gentle sewing before gravity and full seam strength come in.
    sew_frames: u32,
    /// Set while garments are still being put on (see [`Self::start_dressing`]).
    dress: Option<Box<DressPlan>>,
    pub flat_positions: Vec<f32>,
    pub flat_normals: Vec<f32>,
    pub flat_strain_pct: Vec<f32>,
    pub flat_tension_kpa: Vec<f32>,
    pub flat_clearance_mm: Vec<f32>,
}

impl ClothSimSession {
    pub fn new(
        pieces: &[PatternPiece],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        fabric: &FabricSpec,
        pose: AvatarPose,
    ) -> Self {
        Self::new_with_form(pieces, stitching, measurements, fabric, pose, AvatarBodyForm::Female)
    }

    pub fn new_with_form(
        pieces: &[PatternPiece],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        fabric: &FabricSpec,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Self {
        Self::new_with_materials(
            pieces,
            stitching,
            measurements,
            &MaterialSet::single(fabric),
            pose,
            body_form,
        )
    }

    pub fn new_with_materials(
        pieces: &[PatternPiece],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        materials: &MaterialSet,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Self {
        Self::new_layered(pieces, &[], stitching, measurements, materials, pose, body_form)
    }

    /// Builds and settles a session where `layers[i]` is the garment layer of `pieces[i]`
    /// (0 = closest to the body; missing entries are 0).
    ///
    /// Layered garments are dressed the way a person dresses: each layer is placed around the
    /// layers under it and drapes alone while those stay put, then every layer settles together
    /// with one-sided contacts between layers. Because the layer order says which side of the
    /// other garment each vertex belongs on, a vertex that ends up on the wrong side is always
    /// pushed back out; garments can never tangle the way free cloth-cloth collision can.
    pub fn new_layered(
        pieces: &[PatternPiece],
        layers: &[u8],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        materials: &MaterialSet,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Self {
        let mut sim =
            Self::start_dressing(pieces, layers, stitching, measurements, materials, pose, body_form);
        if sim.dress.is_some() {
            while sim.dress.is_some() {
                sim.step(1);
            }
            sim.step(LAYER_COUPLE_FRAMES);
        }
        sim
    }

    /// Like [`Self::new_layered`], but returns as soon as the innermost layer has draped. The
    /// other layers are then put on one after the other by the following [`Self::step`] calls
    /// (the 3D view steps once per frame), so the page never freezes and the designer watches
    /// the outfit being dressed. A single layer behaves exactly like `new_with_materials`.
    pub fn start_dressing(
        pieces: &[PatternPiece],
        layers: &[u8],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        materials: &MaterialSet,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Self {
        let layer_of = |pi: usize| layers.get(pi).copied().unwrap_or(0);
        let mut levels: Vec<u8> = (0..pieces.len()).filter(|&i| pieces[i].visible).map(layer_of).collect();
        levels.sort_unstable();
        levels.dedup();
        if levels.len() <= 1 {
            let mut sim = Self::new_unsettled(pieces, layers, &[], stitching, measurements, materials, pose, body_form);
            sim.step(INIT_FRAMES);
            return sim;
        }
        let plan = DressPlan {
            pieces: pieces.to_vec(),
            layers: (0..pieces.len()).map(layer_of).collect(),
            stitching: stitching.clone(),
            measurements: measurements.clone(),
            materials: materials.clone(),
            pose,
            body_form,
            levels,
            stage: 0,
            saved_inv_mass: Vec::new(),
        };
        let mut sim = Self::dress_stage(plan, 0, &[]);
        while sim.dress.as_ref().map(|d| d.stage == 0).unwrap_or(false) {
            sim.step(1);
        }
        sim
    }

    /// Builds the scene for one dressing stage: every layer up to `levels[stage]`, the lower ones
    /// kept frozen exactly where they `settled` (their vertices come first, in the same order),
    /// the new one placed around that settled cloth. Wrapping around where the inner garment
    /// started would be wrong: a loose garment hangs away from the lower back and under the bust.
    fn dress_stage(mut plan: DressPlan, stage: usize, settled: &[Vec3]) -> Self {
        let level = plan.levels[stage];
        let keep: Vec<usize> = (0..plan.pieces.len()).filter(|&i| plan.layers[i] <= level).collect();
        let sub_pieces: Vec<PatternPiece> = keep.iter().map(|&i| plan.pieces[i].clone()).collect();
        let sub_layers: Vec<u8> = keep.iter().map(|&i| plan.layers[i]).collect();
        let mut sim = Self::new_unsettled(
            &sub_pieces,
            &sub_layers,
            settled,
            &plan.stitching,
            &plan.measurements,
            &plan.materials,
            plan.pose,
            plan.body_form,
        );
        // Piece indices refer to this stage's subset: map them to the full outfit.
        for vp in &mut sim.vert_piece {
            *vp = keep[*vp as usize] as u32;
        }
        let saved = sim.core.inv_mass.clone();
        for w in sim.core.inv_mass.iter_mut().take(settled.len()) {
            *w = 0.0;
        }
        sim.sew_frames = SEW_FRAMES;
        plan.stage = stage;
        plan.saved_inv_mass = saved;
        sim.dress = Some(Box::new(plan));
        sim
    }

    /// Called after every frame while dressing: gentle sewing lasts until the new layer's seams
    /// meet (bounded), then it settles; at the end of a stage the next layer is put on.
    fn advance_dressing(&mut self) {
        let Some(plan) = self.dress.as_ref() else { return };
        if self.step_count + 1 == self.sew_frames
            && self.sew_frames < LAYER_MAX_SEW_FRAMES
            && self.seam_gap_stats_mm().1 > LAYER_SEWN_GAP_MM
        {
            self.sew_frames += 5;
        }
        if self.step_count < self.sew_frames + SETTLE_FRAMES {
            return;
        }
        let last = plan.stage + 1 == plan.levels.len();
        let mut plan = self.dress.take().expect("dressing plan");
        self.core.inv_mass = std::mem::take(&mut plan.saved_inv_mass);
        if last {
            return;
        }
        let settled = self.core.pos.clone();
        let next = plan.stage + 1;
        *self = Self::dress_stage(*plan, next, &settled);
    }

    /// `(layer being put on, number of layers)` while the outfit is still being dressed.
    pub fn dressing_progress(&self) -> Option<(usize, usize)> {
        self.dress.as_ref().map(|d| (d.stage + 1, d.levels.len()))
    }


    /// Builds the session with panels at their initial arrangement, before any simulation.
    #[doc(hidden)]
    pub fn new_unsettled(
        pieces: &[PatternPiece],
        layers: &[u8],
        settled: &[Vec3],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        materials: &MaterialSet,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Self {
        let avatar = ParametricAvatar::from_measurements_with_form(measurements, pose, body_form);
        let avatar_mesh = avatar.build_render_mesh();
        let body_points = BodyPointGrid::build(&avatar_mesh, 0.02);

        // Vertical anchors for isometric placement (style-split panels keep their relative height).
        let mut torso_top = f64::NEG_INFINITY;
        let mut skirt_top = f64::NEG_INFINITY;
        for p in pieces.iter().filter(|p| p.visible) {
            let top = p
                .seam_loop
                .edges
                .iter()
                .map(|e| e.start.y)
                .fold(f64::NEG_INFINITY, f64::max);
            match p.arrangement_zone {
                ArrangementZone::FrontTorso | ArrangementZone::BackTorso => {
                    torso_top = torso_top.max(top)
                }
                ArrangementZone::FrontSkirt | ArrangementZone::BackSkirt => {
                    skirt_top = skirt_top.max(top)
                }
                _ => {}
            }
        }
        let ctx = ArrangeCtx {
            torso_top_mm: if torso_top.is_finite() { torso_top } else { 0.0 },
            skirt_top_mm: if skirt_top.is_finite() { skirt_top } else { 0.0 },
        };

        let mut slot_roles: Vec<String> = Vec::new();
        let mut positions: Vec<Vec3> = Vec::new();
        let mut rest2d: Vec<Point2> = Vec::new();
        let mut tri_idx: Vec<[usize; 3]> = Vec::new();
        let mut tri_grain: Vec<Point2> = Vec::new();
        let mut tri_slot: Vec<u16> = Vec::new();
        let mut hinge_idx: Vec<[usize; 4]> = Vec::new();
        let mut hinge_slot: Vec<u16> = Vec::new();
        let mut hinge_grain: Vec<Point2> = Vec::new();
        let mut panels: Vec<PanelRange> = Vec::new();
        let mut sew: Vec<BarySew> = Vec::new();
        let mut darts_closed = 0usize;

        let mut edge_vert_map: HashMap<(Uuid, i8, Uuid), Vec<BoundVert>> = HashMap::new();
        let mut dart_map: DartLegMap = HashMap::new();
        let mut sewn_len: HashMap<(Uuid, Uuid), f64> = HashMap::new();
        for p in pieces {
            for e in &p.seam_loop.edges {
                sewn_len.insert((p.id, e.id), edge_total_length(e).max(1.0));
            }
        }
        // Trims are laid onto their partner seams once every other panel is placed.
        let mut trims: Vec<(usize, usize, PanelMesh)> = Vec::new(); // (piece idx, panel idx, mesh)

        // Pieces are placed inner layer first, so each outer layer wraps around the cloth
        // already placed under it.
        let layer_of = |pi: usize| layers.get(pi).copied().unwrap_or(0);
        let mut order: Vec<usize> = (0..pieces.len()).collect();
        order.sort_by_key(|&pi| layer_of(pi));
        let mut vert_layer: Vec<u8> = Vec::new();
        let mut vert_piece: Vec<u32> = Vec::new();
        let mut placed_zones: Vec<(ArrangementZone, usize, usize, u8)> = Vec::new();
        let mut shell = InnerShell { gap: LAYER_PLACE_GAP, ..Default::default() };
        let mut shell_layer: Option<u8> = None;

        for pi in order {
            let p = &pieces[pi];
            if !p.visible {
                continue;
            }
            let layer = layer_of(pi);
            if shell_layer != Some(layer) {
                shell = InnerShell { gap: LAYER_PLACE_GAP, ..Default::default() };
                for &(z, a, b, l) in &placed_zones {
                    if l < layer {
                        shell.add(z, &positions[a..b]);
                    }
                }
                shell_layer = Some(layer);
            }
            let unfold = p.mirror_fold_edge.is_some();
            let spawn_pair = !unfold && p.cut_quantity == 2;
            let sides: &[(bool, i8)] = if spawn_pair {
                &[(false, 1), (true, -1)]
            } else {
                &[(false, 1)]
            };
            let slot = match slot_roles.iter().position(|r| *r == p.fabric_role) {
                Some(s) => s as u16,
                None => {
                    slot_roles.push(p.fabric_role.clone());
                    (slot_roles.len() - 1) as u16
                }
            };

            for &(mirror_x, side) in sides {
                let Some(pm) = mesh_pattern_piece(p, mirror_x, unfold, MESH_SPACING_MM) else {
                    continue;
                };
                let zone = if mirror_x && pm.arrangement_zone == ArrangementZone::SleeveLeft {
                    ArrangementZone::SleeveRight
                } else {
                    pm.arrangement_zone
                };
                let start = positions.len();
                let placed = if pm.darts_closed > 0 {
                    let closed = close_darts_for_placement(&pm);
                    place_panel(&avatar, &closed, p, zone, side as f32, spawn_pair, &ctx, &shell)
                } else {
                    place_panel(&avatar, &pm, p, zone, side as f32, spawn_pair, &ctx, &shell)
                };
                positions.extend(placed);
                vert_layer.resize(positions.len(), layer);
                vert_piece.resize(positions.len(), pi as u32);
                if positions.len() <= settled.len() {
                    // A lower layer that already draped: keep it where it settled.
                    let end = positions.len();
                    positions[start..].copy_from_slice(&settled[start..end]);
                }
                placed_zones.push((zone, start, positions.len(), layer));
                rest2d.extend(pm.raw_2d_mm.iter().map(|q| Point2::new(q.x * 0.001, q.y * 0.001)));
                let tri_start = tri_idx.len();
                for (k, t) in pm.triangles.iter().enumerate() {
                    tri_idx.push([start + t[0], start + t[1], start + t[2]]);
                    tri_grain.push(pm.tri_grain[k]);
                    tri_slot.push(slot);
                }
                let grain0 = pm.tri_grain.first().copied().unwrap_or(Point2::new(0.0, 1.0));
                for h in &pm.hinges {
                    hinge_idx.push([start + h[0], start + h[1], start + h[2], start + h[3]]);
                    hinge_slot.push(slot);
                    hinge_grain.push(grain0);
                }
                for br in &pm.boundary_refs {
                    edge_vert_map
                        .entry((p.id, br.side_sign, br.edge_id))
                        .or_default()
                        .push(BoundVert {
                            global_idx: start + br.vertex_idx,
                            sewn_frac: br.sewn_frac,
                        });
                }
                for dr in &pm.dart_refs {
                    dart_map
                        .entry((p.id, dr.side_sign, dr.dart_id, dr.leg))
                        .or_default()
                        .push((start + dr.vertex_idx, dr.frac));
                }
                for &(eid, len) in &pm.edge_sewn_len_mm {
                    sewn_len.insert((p.id, eid), len.max(1.0));
                }
                darts_closed += pm.darts_closed;

                // Close the sleeve tube (back underarm <-> front underarm).
                if matches!(zone, ArrangementZone::SleeveLeft | ArrangementZone::SleeveRight)
                    && p.seam_loop.edges.len() >= 5
                {
                    let e_bu = p.seam_loop.edges[1].id;
                    let e_fu = p.seam_loop.edges[4].id;
                    if let (Some(va), Some(vb)) = (
                        edge_vert_map.get(&(p.id, side, e_bu)),
                        edge_vert_map.get(&(p.id, side, e_fu)),
                    ) {
                        let ca: Vec<(usize, f64)> =
                            va.iter().map(|v| (v.global_idx, v.sewn_frac)).collect();
                        let cb: Vec<(usize, f64)> =
                            vb.iter().map(|v| (v.global_idx, 1.0 - v.sewn_frac)).collect();
                        pair_chains_barycentric(&ca, &cb, &mut sew);
                    }
                }
                let panel_idx = panels.len();
                panels.push(PanelRange {
                    start,
                    end: positions.len(),
                    tri_start,
                    tri_end: tri_idx.len(),
                });
                if zone == ArrangementZone::FlatCustom {
                    trims.push((pi, panel_idx, pm));
                }
            }

            // Close centre seams between mirrored L/R copies (centre back/front, crotch, seat).
            if spawn_pair {
                for edge in &p.seam_loop.edges {
                    let lbl = edge.label.to_lowercase();
                    let centre = lbl.contains("center back")
                        || lbl.contains("centre back")
                        || lbl.contains("center front")
                        || lbl.contains("centre front");
                    let legs = matches!(
                        p.arrangement_zone,
                        ArrangementZone::FrontLeg | ArrangementZone::BackLeg
                    ) && (lbl.contains("crotch") || lbl.contains("seat"));
                    if !(centre || legs) {
                        continue;
                    }
                    if let (Some(vl), Some(vr)) = (
                        edge_vert_map.get(&(p.id, 1, edge.id)),
                        edge_vert_map.get(&(p.id, -1, edge.id)),
                    ) {
                        let ca: Vec<(usize, f64)> =
                            vl.iter().map(|v| (v.global_idx, v.sewn_frac)).collect();
                        let cb: Vec<(usize, f64)> =
                            vr.iter().map(|v| (v.global_idx, v.sewn_frac)).collect();
                        pair_chains_barycentric(&ca, &cb, &mut sew);
                    }
                }
            }
        }

        // Ordered vertex chains of every waist edge (for fit diagnostics).
        let mut waist_chains: Vec<(Vec<usize>, f32)> = Vec::new();
        for p in pieces.iter().filter(|p| p.visible) {
            for e in p.seam_loop.edges.iter().filter(|e| e.label.to_lowercase().contains("waist")) {
                for side in [1i8, -1i8] {
                    if let Some(vs) = edge_vert_map.get(&(p.id, side, e.id)) {
                        let mut ordered: Vec<(f64, usize)> = vs.iter().map(|v| (v.sewn_frac, v.global_idx)).collect();
                        ordered.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                        let rest_m = (*sewn_len.get(&(p.id, e.id)).unwrap_or(&0.0) * 0.001) as f32;
                        waist_chains.push((ordered.into_iter().map(|o| o.1).collect(), rest_m));
                    }
                }
            }
        }

        // Seam-line length limits: every meshed panel edge, ordered along the edge.
        let mut chain_specs: Vec<(Vec<usize>, f32, u16)> = Vec::new();
        for p in pieces.iter().filter(|p| p.visible) {
            let slot = slot_roles.iter().position(|r| *r == p.fabric_role).unwrap_or(0) as u16;
            for e in &p.seam_loop.edges {
                for side in [1i8, -1i8] {
                    let Some(vs) = edge_vert_map.get(&(p.id, side, e.id)) else { continue };
                    if vs.len() < 3 {
                        continue;
                    }
                    let mut ordered: Vec<(f64, usize)> = vs.iter().map(|v| (v.sewn_frac, v.global_idx)).collect();
                    ordered.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                    ordered.dedup_by(|a, b| a.1 == b.1);
                    let rest = (*sewn_len.get(&(p.id, e.id)).unwrap_or(&0.0) * 0.001) as f32;
                    if rest > 0.01 {
                        chain_specs.push((ordered.into_iter().map(|o| o.1).collect(), rest, slot));
                    }
                }
            }
        }

        // Seam bindings (arc-length matched on the sewn length, so closed darts never bunch).
        let mut seen: HashSet<(usize, usize, usize, i32)> = HashSet::new();
        for binding in &stitching.bindings {
            for side in [1i8, -1i8] {
                let ca = build_interface_chain(&binding.interface_a.edges, side, &edge_vert_map, &sewn_len);
                let cb = build_interface_chain(&binding.interface_b.edges, side, &edge_vert_map, &sewn_len);
                if ca.is_empty() || cb.is_empty() {
                    continue;
                }
                let mut local = Vec::new();
                pair_chains_barycentric(&ca, &cb, &mut local);
                for sc in local {
                    if seen.insert((sc.a, sc.b0, sc.b1, (sc.w * 1000.0).round() as i32)) {
                        sew.push(sc);
                    }
                }
            }
        }

        // Dart legs are sewn back together (edge end to edge end, apex to apex).
        let mut dart_keys: Vec<(Uuid, i8, Uuid)> = dart_map
            .keys()
            .filter(|k| k.3 == 0)
            .map(|k| (k.0, k.1, k.2))
            .collect();
        dart_keys.sort();
        for (pid, side, did) in dart_keys {
            if let (Some(a), Some(b)) = (
                dart_map.get(&(pid, side, did, 0)),
                dart_map.get(&(pid, side, did, 1)),
            ) {
                pair_chains_barycentric(a, b, &mut sew);
            }
        }

        // Trims: lay each one onto the seam it is sewn to.
        if !trims.is_empty() {
            let trim_pieces: HashSet<Uuid> = trims.iter().map(|(pi, _, _)| pieces[*pi].id).collect();
            let (nb_sum, nb_cnt) = neighbour_sums(&positions, &tri_idx);
            for (pi, panel_idx, pm) in &trims {
                let piece = &pieces[*pi];
                let range = &panels[*panel_idx];
                let mut anchors: Vec<TrimAnchor> = Vec::new();
                for binding in &stitching.bindings {
                    let a_has = binding.interface_a.edges.iter().any(|e| e.piece_id == piece.id);
                    let b_has = binding.interface_b.edges.iter().any(|e| e.piece_id == piece.id);
                    let (own, other) = match (a_has, b_has) {
                        (true, false) => (&binding.interface_a, &binding.interface_b),
                        (false, true) => (&binding.interface_b, &binding.interface_a),
                        _ => continue,
                    };
                    if other.edges.iter().any(|e| trim_pieces.contains(&e.piece_id)) {
                        continue;
                    }
                    for side in [1i8, -1i8] {
                        let own_chain = build_interface_chain(&own.edges, side, &edge_vert_map, &sewn_len);
                        let other_chain =
                            build_interface_chain(&other.edges, side, &edge_vert_map, &sewn_len);
                        if other_chain.len() < 2 {
                            continue;
                        }
                        for &(g, u) in &own_chain {
                            if g < range.start || g >= range.end {
                                continue;
                            }
                            let (target, away) =
                                chain_point(&other_chain, u, &positions, &nb_sum, &nb_cnt);
                            anchors.push(TrimAnchor {
                                local_idx: g - range.start,
                                target,
                                away,
                            });
                        }
                    }
                }
                if let Some(newpos) = place_trim(pm, &anchors) {
                    positions[range.start..range.end].copy_from_slice(&newpos);
                }
            }
        }

        // Settled lower layers keep their draped shape (trims included).
        if !settled.is_empty() && settled.len() <= positions.len() {
            positions[..settled.len()].copy_from_slice(settled);
        }

        // Make every panel's front face point away from the body (correct two-sided shading).
        for pr in &panels {
            let mut score = 0.0_f32;
            let step = ((pr.tri_end - pr.tri_start) / 40).max(1);
            for t in (pr.tri_start..pr.tri_end).step_by(step) {
                let [a, b, c] = tri_idx[t];
                let n = positions[b].sub(positions[a]).cross(positions[c].sub(positions[a]));
                let ctr = positions[a].add(positions[b]).add(positions[c]).scale(1.0 / 3.0);
                let (_, out) = avatar.eval_sdf(ctr);
                score += n.normalized().dot(out);
            }
            if score < 0.0 {
                for t in &mut tri_idx[pr.tri_start..pr.tri_end] {
                    t.swap(1, 2);
                }
            }
        }

        // Waistline support: skirts, trousers and the waistbands sewn to them keep their
        // waist near where it was placed (stands in for waistband and closure tension).
        let lower_body: HashSet<Uuid> = pieces
            .iter()
            .filter(|p| {
                matches!(
                    p.arrangement_zone,
                    ArrangementZone::FrontSkirt
                        | ArrangementZone::BackSkirt
                        | ArrangementZone::FrontLeg
                        | ArrangementZone::BackLeg
                )
            })
            .map(|p| p.id)
            .collect();
        let mut supported: HashSet<Uuid> = lower_body.clone();
        for b in &stitching.bindings {
            let touches_lower = b
                .interface_a
                .edges
                .iter()
                .chain(&b.interface_b.edges)
                .any(|e| lower_body.contains(&e.piece_id));
            if touches_lower {
                for e in b.interface_a.edges.iter().chain(&b.interface_b.edges) {
                    if let Some(p) = pieces.iter().find(|p| p.id == e.piece_id) {
                        if p.arrangement_zone == ArrangementZone::FlatCustom {
                            supported.insert(p.id);
                        }
                    }
                }
            }
        }
        let mut waist_support: Vec<(Vec<usize>, f32)> = Vec::new();
        for p in pieces.iter().filter(|p| p.visible && supported.contains(&p.id)) {
            for e in p.seam_loop.edges.iter().filter(|e| e.label.to_lowercase().contains("waist")) {
                for side in [1i8, -1i8] {
                    if let Some(vs) = edge_vert_map.get(&(p.id, side, e.id)) {
                        let mut verts: Vec<usize> = vs.iter().map(|v| v.global_idx).collect();
                        verts.sort_unstable();
                        verts.dedup();
                        if verts.is_empty() {
                            continue;
                        }
                        let mean = verts.iter().map(|&v| positions[v].y).sum::<f32>() / verts.len() as f32;
                        waist_support.push((verts, mean - WAIST_SLACK));
                    }
                }
            }
        }

        // Elements.
        let mats: Vec<SolverMaterial> = slot_roles.iter().map(|r| materials.solver_material(r)).collect();
        let mut tris = Vec::with_capacity(tri_idx.len());
        let mut triangles = Vec::with_capacity(tri_idx.len() * 3);
        for (k, t) in tri_idx.iter().enumerate() {
            if let Some(te) = make_tri(*t, [rest2d[t[0]], rest2d[t[1]], rest2d[t[2]]], tri_grain[k], tri_slot[k]) {
                tris.push(te);
                triangles.extend([t[0] as u32, t[1] as u32, t[2] as u32]);
            }
        }
        let hinges: Vec<HingeElem> = hinge_idx
            .iter()
            .enumerate()
            .filter_map(|(k, h)| {
                make_hinge(
                    *h,
                    [rest2d[h[0]], rest2d[h[1]], rest2d[h[2]], rest2d[h[3]]],
                    hinge_grain[k],
                    hinge_slot[k],
                )
            })
            .collect();

        let mut tethers = Vec::new();
        for pr in &panels {
            if pr.tri_start >= pr.tri_end {
                continue;
            }
            let area: f64 = tri_idx[pr.tri_start..pr.tri_end]
                .iter()
                .map(|t| 0.5 * rest2d[t[1]].sub(rest2d[t[0]]).perp_dot(rest2d[t[2]].sub(rest2d[t[0]])).abs())
                .sum();
            let vertex_area = (area / (pr.end - pr.start).max(1) as f64) as f32;
            build_tethers(
                &rest2d,
                pr.start..pr.end,
                tri_grain[pr.tri_start],
                tri_slot[pr.tri_start],
                vertex_area,
                &mut tethers,
            );
        }
        let chains: Vec<ChainLimit> = chain_specs
            .into_iter()
            .map(|(verts, rest, slot)| {
                // Average lock strain along the line, from its 2D direction against the grain.
                let m = &mats[slot as usize];
                let mut lock_sum = 0.0;
                let mut w_sum = 0.0;
                let g = tri_grain_for_slot(&tri_grain, &tri_slot, slot);
                for w in verts.windows(2) {
                    let d = rest2d[w[1]].sub(rest2d[w[0]]);
                    let l = d.len();
                    if l < 1e-6 {
                        continue;
                    }
                    let c = (d.dot(g) / l) as f32;
                    let c2 = c * c;
                    let s2 = 1.0 - c2;
                    lock_sum += (m.lock_warp * c2 + m.lock_weft * s2 + BIAS_EXTENSION * 4.0 * c2 * s2) * l as f32;
                    w_sum += l as f32;
                }
                let lock = if w_sum > 0.0 { lock_sum / w_sum } else { m.lock_weft };
                ChainLimit { verts, max_len: rest * (1.0 + lock) }
            })
            .collect();
        let mut core = ClothCore::new(positions, tris, hinges, mats);
        core.tethers = tethers;
        core.chains = chains;
        core.refresh_materials();
        core.sew = sew;
        let n = core.pos.len();
        let ntri = core.tris.len();
        let mut sim = Self {
            avatar,
            avatar_mesh,
            fabric: materials.shell.clone(),
            materials: materials.clone(),
            slot_roles,
            core,
            waist_chains,
            waist_support,
            triangles,
            body_points,
            contacts: vec![
                Contact {
                    q: Vec3::ZERO,
                    n: Vec3::new(0.0, 1.0, 0.0),
                    at: Vec3::new(1e3, 1e3, 1e3),
                    dist: -1.0,
                };
                n
            ],
            vert_offset: vec![0.003; n],
            vert_friction: vec![0.4; n],
            tri_near: vec![true; ntri],
            scratch: Vec::with_capacity(64),
            step_count: 0,
            darts_closed,
            motion_mm_s: 0.0,
            layers: {
                let mut l = vert_layer.clone();
                l.sort_unstable();
                l.dedup();
                l
            },
            vert_layer,
            vert_piece,
            vert_thickness: vec![0.001; n],
            layer_contacts: vec![[None, None]; n],
            vert_room: vec![f32::INFINITY; n],
            sew_frames: SEW_FRAMES,
            dress: None,
            flat_positions: vec![0.0; n * 3],
            flat_normals: vec![0.0; n * 3],
            flat_strain_pct: vec![0.0; n],
            flat_tension_kpa: vec![0.0; n],
            flat_clearance_mm: vec![0.0; n],
        };
        sim.refresh_contact_params();
        sim.sync_diagnostic_buffers();
        sim
    }

    /// Updates the shell fabric live (no re-arrangement; the drape re-settles).
    pub fn set_fabric(&mut self, fabric: FabricSpec) {
        let mut set = self.materials.clone();
        set.shell = fabric.normalized();
        self.set_materials(set);
    }

    /// Updates every fabric live (shell and role overrides).
    pub fn set_materials(&mut self, materials: MaterialSet) {
        self.fabric = materials.shell.clone();
        self.core.mats = self
            .slot_roles
            .iter()
            .map(|r| materials.solver_material(r))
            .collect();
        self.materials = materials;
        self.core.refresh_materials();
        self.refresh_contact_params();
    }

    fn refresh_contact_params(&mut self) {
        let n = self.core.pos.len();
        let mut off = vec![0.0_f32; n];
        let mut fr = vec![0.0_f32; n];
        let mut cnt = vec![0u32; n];
        let mut thick = vec![0.0_f32; n];
        for t in &self.core.tris {
            let m = &self.core.mats[t.slot as usize];
            for &v in &t.v {
                thick[v] = thick[v].max(m.thickness);
                off[v] = off[v].max(m.contact_offset());
                fr[v] += m.friction;
                cnt[v] += 1;
            }
        }
        for i in 0..n {
            self.vert_offset[i] = if off[i] > 0.0 { off[i] } else { 0.003 };
            self.vert_friction[i] = if cnt[i] > 0 { fr[i] / cnt[i] as f32 } else { 0.4 };
            self.vert_thickness[i] = if thick[i] > 0.0 { thick[i] } else { 0.001 };
        }
    }

    pub fn triangle_indices(&self) -> &[u32] {
        &self.triangles
    }

    /// Per-vertex RGB (0..1) taken from the colour of each piece role's fabric.
    pub fn cloth_colors(&self) -> Vec<f32> {
        let slot_rgb: Vec<[f32; 3]> = self
            .slot_roles
            .iter()
            .map(|r| hex_to_rgb(&self.materials.fabric_for_role(r).color_hex))
            .collect();
        let n = self.core.pos.len();
        let mut out = vec![0.5_f32; n * 3];
        let mut done = vec![false; n];
        for t in &self.core.tris {
            let rgb = slot_rgb.get(t.slot as usize).copied().unwrap_or([0.5, 0.5, 0.5]);
            for &v in &t.v {
                if !done[v] {
                    done[v] = true;
                    out[v * 3..v * 3 + 3].copy_from_slice(&rgb);
                }
            }
        }
        out
    }

    /// Returns `(avg_seam_gap_mm, max_seam_gap_mm)` over every sewing constraint.
    pub fn seam_gap_stats_mm(&self) -> (f32, f32) {
        if self.core.sew.is_empty() {
            return (0.0, 0.0);
        }
        let mut sum = 0.0_f32;
        let mut max = 0.0_f32;
        for s in &self.core.sew {
            let pb = self.core.pos[s.b0]
                .scale(1.0 - s.w)
                .add(self.core.pos[s.b1].scale(s.w));
            let gap = pb.sub(self.core.pos[s.a]).len() * 1000.0;
            sum += gap;
            max = max.max(gap);
        }
        (sum / self.core.sew.len() as f32, max)
    }

    /// Advances the simulation by `num_steps` 60 Hz frames.
    pub fn step(&mut self, num_steps: u32) {
        let dt = 1.0 / (60.0 * SUBSTEPS as f32);
        for _ in 0..num_steps {
            self.step_count += 1;
            let f = self.step_count as f32;
            // Close seams gently under low gravity, then bring in the full drape weight.
            // Sewing phase: seams close slowly under low gravity so panels slide around the body
            // instead of tearing; then the seams lock and the full drape weight comes in.
            let (sew_frames, settle_frames) = (self.sew_frames as f32, SETTLE_FRAMES as f32);
            let grav = if f < sew_frames {
                0.10
            } else {
                (0.10 + (f - sew_frames) / settle_frames).min(1.0)
            };
            let max_pull = if f < sew_frames {
                // An outer layer whose seams are still open past the usual sewing time pulls a
                // little harder (single garments never get here).
                let extended = ((f - SEW_FRAMES as f32) / 30.0).clamp(0.0, 1.0);
                0.0005 + 0.0015 * (f / SEW_FRAMES as f32).min(1.0) + 0.002 * extended
            } else if f < sew_frames + settle_frames {
                0.004
            } else {
                0.05
            };
            let gravity = Vec3::new(0.0, -9.81 * grav, 0.0);
            self.update_near_flags();
            let layered = self.layers.len() > 1;
            // Contacts barely change between frames: refresh them every other frame.
            if layered {
                self.find_layer_contacts();
            }
            let frame_start: Vec<Vec3> = self.core.pos.clone();

            // Quasi-static sewing: velocities are mostly discarded so initial strain and seam
            // gaps relax without launching panels; damping eases off as gravity comes in.
            let extra = if f < sew_frames {
                0.55
            } else if f < sew_frames + settle_frames {
                0.55 + 0.45 * (f - sew_frames) / settle_frames
            } else {
                1.0
            };
            for s in 0..SUBSTEPS {
                self.core.integrate_damped(dt, gravity, extra);
                self.core.solve_sew(max_pull);
                self.core.begin_substep();
                self.core.solve_membrane(dt);
                self.core.solve_tethers(dt);
                for _ in 0..BEND_PASSES {
                    self.core.solve_bending(dt);
                }
                self.support_waistline();
                self.core.solve_strain_limits();
                self.core.solve_chain_limits();
                self.core.solve_sew(max_pull);
                self.collide_vertices(true);
                if layered {
                    self.solve_layer_contacts();
                }
                if s % 2 == 1 {
                    self.collide_body_points();
                }
            }

            let mut sum = 0.0_f32;
            for (a, b) in frame_start.iter().zip(&self.core.pos) {
                sum += b.sub(*a).len();
            }
            self.motion_mm_s = sum / frame_start.len().max(1) as f32 * 60.0 * 1000.0;
            self.advance_dressing();
        }
        self.sync_diagnostic_buffers();
    }

    /// Garment layer of every vertex (0 = closest to the body).
    pub fn vertex_layers(&self) -> &[u8] {
        &self.vert_layer
    }

    /// Index of each vertex's piece in the `pieces` the session was built from.
    pub fn vertex_pieces(&self) -> &[u32] {
        &self.vert_piece
    }

    /// Triangle indices without the triangles of hidden pieces (`shown[piece] == false`). The
    /// hidden cloth is still simulated, so the garments over it keep their drape.
    pub fn triangle_indices_shown(&self, shown: &[bool]) -> Vec<u32> {
        self.triangles
            .chunks_exact(3)
            .filter(|t| shown.get(self.vert_piece[t[0] as usize] as usize).copied().unwrap_or(true))
            .flatten()
            .copied()
            .collect()
    }

    /// Finds, for every vertex, the nearest triangle of a lower layer (it must stay outside it)
    /// and of a higher layer (it must stay under it). Only triangles that the vertex projects
    /// into count, so cloth hanging past another layer's hem is free to fall inwards.
    fn find_layer_contacts(&mut self) {
        let pos = &self.core.pos;
        // Area-weighted vertex normals (every panel's faces point away from the body).
        let mut vn = vec![Vec3::ZERO; pos.len()];
        let mut tn = Vec::with_capacity(self.core.tris.len());
        for t in &self.core.tris {
            let n = pos[t.v[1]].sub(pos[t.v[0]]).cross(pos[t.v[2]].sub(pos[t.v[0]]));
            for &i in &t.v {
                vn[i] = vn[i].add(n);
            }
            tn.push(n.normalized());
        }
        for n in &mut vn {
            *n = n.normalized();
        }
        // Flat grid: every triangle once, by its centre; cells of 50 mm searched +-1 cover any
        // triangle (up to ~20 mm across) within LAYER_SEARCH of a vertex.
        let hc = 0.05_f32;
        let key = |p: Vec3| -> (i32, i32, i32) {
            ((p.x / hc).floor() as i32, (p.y / hc).floor() as i32, (p.z / hc).floor() as i32)
        };
        let mut keyed: Vec<((i32, i32, i32), u32)> = self
            .core
            .tris
            .iter()
            .enumerate()
            .map(|(k, t)| {
                let ctr = pos[t.v[0]].add(pos[t.v[1]]).add(pos[t.v[2]]).scale(1.0 / 3.0);
                (key(ctr), k as u32)
            })
            .collect();
        keyed.sort_unstable_by_key(|e| e.0);
        let mut cells: HashMap<(i32, i32, i32), (u32, u32)> = HashMap::with_capacity(keyed.len() / 4);
        let mut i = 0;
        while i < keyed.len() {
            let mut j = i + 1;
            while j < keyed.len() && keyed[j].0 == keyed[i].0 {
                j += 1;
            }
            cells.insert(keyed[i].0, (i as u32, j as u32));
            i = j;
        }
        for v in 0..pos.len() {
            let lv = self.vert_layer[v];
            let p = pos[v];
            let c = key(p);
            let mut best: [(f32, Option<LayerContact>); 2] = [(f32::INFINITY, None); 2];
            for x in c.0 - 1..=c.0 + 1 {
                for y in c.1 - 1..=c.1 + 1 {
                    for z in c.2 - 1..=c.2 + 1 {
                        let Some(&(s0, s1)) = cells.get(&(x, y, z)) else { continue };
                        for &(_, k) in &keyed[s0 as usize..s1 as usize] {
                            let t = &self.core.tris[k as usize];
                            let lt = self.vert_layer[t.v[0]];
                            if lt == lv {
                                continue;
                            }
                            let stacked = vn[v].dot(tn[k as usize]) >= LAYER_MIN_ALIGN;
                            let slot = if lt < lv { 0 } else { 1 };
                            let Some((d, bary)) = project_on_triangle(p, pos[t.v[0]], pos[t.v[1]], pos[t.v[2]]) else {
                                continue;
                            };
                            if d.abs() > LAYER_SEARCH || d.abs() >= best[slot].0 {
                                continue;
                            }
                            let side = if stacked {
                                if lt < lv { 1.0 } else { -1.0 }
                            } else if d.abs() > 0.0005 {
                                d.signum()
                            } else {
                                continue;
                            };
                            best[slot] = (d.abs(), Some(LayerContact { tri: k, bary, side, stacked }));
                        }
                    }
                }
            }
            self.layer_contacts[v] = [best[0].1, best[1].1];
        }
    }

    /// Signed clearance of vertex `v` from the other layer in contact `c` (negative = wrong side).
    fn layer_clearance(&self, v: usize, c: &LayerContact) -> Option<(f32, Vec3)> {
        let t = self.core.tris[c.tri as usize].v;
        let x = [self.core.pos[t[0]], self.core.pos[t[1]], self.core.pos[t[2]]];
        let nraw = x[1].sub(x[0]).cross(x[2].sub(x[0]));
        if nraw.len_sq() < 1e-14 {
            return None;
        }
        let n = nraw.normalized();
        let b = c.bary;
        let q = x[0].scale(b[0]).add(x[1].scale(b[1])).add(x[2].scale(b[2]));
        Some((self.core.pos[v].sub(q).dot(n) * c.side, n.scale(c.side)))
    }

    /// One-sided, mass-weighted, inelastic push between a vertex and the other layer's triangle,
    /// with Coulomb friction on their relative sliding. Frozen cloth (inverse mass 0) stays put.
    fn solve_layer_contacts(&mut self) {
        for v in 0..self.core.pos.len() {
            for c in self.layer_contacts[v] {
                let Some(c) = c else { continue };
                let Some((d, dir)) = self.layer_clearance(v, &c) else { continue };
                let t = self.core.tris[c.tri as usize].v;
                let gap = 0.5 * (self.vert_thickness[v] + self.vert_thickness[t[0]]) + LAYER_AIR_GAP;
                if d >= gap || d < -LAYER_MAX_DEPTH {
                    continue;
                }
                let b = c.bary;
                let inv = &self.core.inv_mass;
                // Lower layers have priority: the outer side of a contact always gives way, so an
                // inner garment drapes exactly as it would alone (no self-collision is needed to
                // keep it from being shoved into hollows such as under the bust).
                let tri_w = [inv[t[0]] * b[0], inv[t[1]] * b[1], inv[t[2]] * b[2]];
                let (wv, wt) = if !c.stacked {
                    (inv[v], tri_w)
                } else if self.vert_layer[v] > self.vert_layer[t[0]] {
                    (inv[v], [0.0; 3])
                } else {
                    (0.0, tri_w)
                };
                let wsum = wv + wt[0] * b[0] + wt[1] * b[1] + wt[2] * b[2];
                if wsum <= 1e-12 {
                    continue;
                }
                // Cloth arriving from the correct side is stopped at once; cloth already on the
                // wrong side (placement overlaps) is eased back out.
                let push = if d >= 0.0 { gap - d } else { (gap - d).min(gap + LAYER_MAX_PUSH) };
                let lambda = push / wsum;
                let dv = dir.scale(wv * lambda);
                let p_new = self.core.pos[v].add(dv);
                // Inelastic: no velocity into the other layer, at most a crawl away from it.
                let vel = p_new.sub(self.core.prev[v]);
                let vn = vel.dot(dir);
                let vn_new = vn.clamp(0.0, MAX_SEPARATION);
                self.core.pos[v] = p_new;
                self.core.prev[v] = p_new.sub(vel.sub(dir.scale(vn))).sub(dir.scale(vn_new));
                for k in 0..3 {
                    let dk = dir.scale(wt[k] * lambda);
                    self.core.pos[t[k]] = self.core.pos[t[k]].sub(dk);
                    self.core.prev[t[k]] = self.core.prev[t[k]].sub(dk);
                }
                if wv > 0.0 {
                    let mu = 0.5 * (self.vert_friction[v] + self.vert_friction[t[0]]);
                    let vel_v = self.core.pos[v].sub(self.core.prev[v]);
                    let mut vel_t = Vec3::ZERO;
                    for k in 0..3 {
                        vel_t = vel_t.add(self.core.pos[t[k]].sub(self.core.prev[t[k]]).scale(b[k]));
                    }
                    let rel = vel_v.sub(vel_t);
                    let rt = rel.sub(dir.scale(rel.dot(dir)));
                    let lt = rt.len();
                    let damp = if lt <= mu * push { 1.0 } else { mu * push / lt };
                    self.core.prev[v] = self.core.prev[v].add(rt.scale(damp));
                }
            }
        }
    }

    /// Vertices found more than 1 mm on the wrong side of another garment layer (places where
    /// one garment shows through another).
    pub fn layer_crossings(&mut self) -> usize {
        if self.layers.len() <= 1 {
            return 0;
        }
        self.find_layer_contacts();
        (0..self.core.pos.len())
            .filter(|&v| {
                self.layer_contacts[v].iter().flatten().filter(|c| c.stacked).any(|c| {
                    self.layer_clearance(v, c)
                        .map(|(d, _)| (-LAYER_MAX_DEPTH..-0.001).contains(&d))
                        .unwrap_or(false)
                })
            })
            .count()
    }

    fn update_near_flags(&mut self) {
        for (k, t) in self.core.tris.iter().enumerate() {
            self.tri_near[k] = t.v.iter().any(|&v| {
                let c = &self.contacts[v];
                let moved = self.core.pos[v].sub(c.at).len();
                c.dist - moved < self.vert_offset[v] + NEAR_BAND
            });
        }
    }

    /// Keeps each waistline's mean height above its floor (a uniform, velocity-free lift), so a
    /// skirt or trouser cannot slide down while the line keeps whatever shape the body gives it.
    fn support_waistline(&mut self) {
        for (verts, min_mean) in &self.waist_support {
            let mean = verts.iter().map(|&v| self.core.pos[v].y).sum::<f32>() / verts.len() as f32;
            let dy = min_mean - mean;
            if dy > 0.0 {
                let lift = 0.5 * dy;
                for &v in verts {
                    if self.core.inv_mass[v] <= 0.0 {
                        continue; // a frozen layer keeps its place while another one drapes
                    }
                    self.core.pos[v].y += lift;
                    self.core.prev[v].y += lift;
                }
            }
        }
    }

    fn collide_vertices(&mut self, with_friction: bool) {
        for i in 0..self.core.pos.len() {
            if self.core.inv_mass[i] <= 0.0 {
                continue;
            }
            let mut p = self.core.pos[i];
            if p.y < 0.005 {
                p.y = 0.005;
            }
            let off = self.vert_offset[i];
            let c = &mut self.contacts[i];
            let moved = p.sub(c.at).len();
            if c.dist >= 0.0 && c.dist - moved > off + 0.004 {
                self.vert_room[i] = c.dist - moved - off;
                self.core.pos[i] = p;
                continue;
            }
            let near = c.dist < off + 0.02;
            if c.dist < 0.0 || moved > 0.004 || (near && moved > 0.0015) {
                let r = off + moved + 0.03;
                match self.avatar.eval_sdf_within(p, r) {
                    Some((d, n)) => {
                        c.q = p.sub(n.scale(d));
                        c.n = n;
                        c.dist = d.max(0.0);
                        c.at = p;
                    }
                    None => {
                        c.dist = r;
                        c.at = p;
                        self.vert_room[i] = r - off;
                        self.core.pos[i] = p;
                        continue;
                    }
                }
            }
            let d = p.sub(c.q).dot(c.n);
            self.vert_room[i] = (d - off).max(0.0);
            if d < off {
                // Inelastic contact: the push-out moves the vertex but never launches it, inward
                // velocity is removed, and Coulomb friction acts on the sliding part.
                let push = off - d;
                let n = c.n;
                let p_new = p.add(n.scale(push));
                let v = p_new.sub(self.core.prev[i]);
                let vn = v.dot(n);
                let vt = v.sub(n.scale(vn));
                let vn_new = vn.clamp(0.0, MAX_SEPARATION);
                let lt = vt.len();
                let mu = if with_friction { self.vert_friction[i] } else { 0.0 };
                // Friction acts on the velocity only: it damps sliding over time but never
                // undoes the corrections seams and stretch limits just made this substep.
                let vt_new = if lt <= mu * push {
                    Vec3::ZERO
                } else {
                    vt.scale(1.0 - mu * push / lt)
                };
                p = p_new;
                self.core.prev[i] = p.sub(n.scale(vn_new)).sub(vt_new);
            }
            self.core.pos[i] = p;
        }
    }

    fn collide_body_points(&mut self) {
        for (k, t) in self.core.tris.iter().enumerate() {
            if !self.tri_near[k] {
                continue;
            }
            let off = self.vert_offset[t.v[0]]
                .max(self.vert_offset[t.v[1]])
                .max(self.vert_offset[t.v[2]]);
            push_triangle_off_points(
                &self.body_points,
                &mut self.core.pos,
                &mut self.core.prev,
                &self.core.inv_mass,
                t.v,
                off,
                &mut self.scratch,
            );
        }
    }

    pub fn stats(&self) -> Sim3dStats {
        // Peak strain as the 99th percentile, so a single sliver at a seam corner or dart tip
        // does not dominate the readout (the heat map still shows every vertex).
        let mut sorted = self.flat_strain_pct.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let max_strain = if sorted.is_empty() {
            0.0
        } else {
            sorted[((sorted.len() - 1) as f32 * 0.99) as usize]
        };
        let mut sum_clear = 0.0_f32;
        let mut min_clear = f32::INFINITY;
        for &c in &self.flat_clearance_mm {
            sum_clear += c;
            min_clear = min_clear.min(c);
        }
        let n = self.flat_clearance_mm.len().max(1) as f32;
        Sim3dStats {
            vertex_count: self.core.pos.len(),
            triangle_count: self.triangles.len() / 3,
            sew_pair_count: self.core.sew.len(),
            step_count: self.step_count,
            max_strain_pct: max_strain,
            avg_clearance_mm: sum_clear / n,
            min_clearance_mm: if min_clear.is_finite() { min_clear } else { 0.0 },
            fabric_name: self.fabric.name.clone(),
            pose: self.avatar.pose,
            body_form: self.avatar.body_form,
            bust_y: self.avatar.bust_y,
            waist_y: self.avatar.waist_y,
            hip_y: self.avatar.hip_y,
            crotch_y: self.avatar.crotch_y,
            neck_y: self.avatar.neck_y,
            measured_bust_mm: self.avatar.measured.bust_mm,
            measured_waist_mm: self.avatar.measured.waist_mm,
            measured_hip_mm: self.avatar.measured.hip_mm,
            measured_height_mm: self.avatar.measured.height_mm,
            darts_closed: self.darts_closed,
            motion_mm_s: self.motion_mm_s,
            dressing_layer: self.dressing_progress().map(|d| d.0).unwrap_or(0),
            dressing_layers: self.dressing_progress().map(|d| d.1).unwrap_or(0),
        }
    }

    /// Total 3D length of all waist edges vs their sewn rest length, mm: `(current, rest)`.
    pub fn waist_length_mm(&self) -> (f32, f32) {
        let mut cur = 0.0;
        let mut rest = 0.0;
        for (chain, r) in &self.waist_chains {
            for w in chain.windows(2) {
                cur += self.core.pos[w[1]].sub(self.core.pos[w[0]]).len();
            }
            rest += r;
        }
        (cur * 1000.0, rest * 1000.0)
    }

    /// Visual drape-quality metrics (poke-through, creases, self-intersections). Costs a few
    /// milliseconds, so call it on demand rather than every frame.
    pub fn drape_quality(&self) -> DrapeQuality {
        drape_quality(
            &self.flat_positions,
            &self.triangles,
            &self.avatar_mesh.positions,
            &self.avatar_mesh.normals,
        )
    }

    fn sync_diagnostic_buffers(&mut self) {
        let n = self.core.pos.len();
        let mut norm_acc = vec![Vec3::ZERO; n];
        for tri in self.triangles.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let fnv = self.core.pos[i1]
                .sub(self.core.pos[i0])
                .cross(self.core.pos[i2].sub(self.core.pos[i0]));
            norm_acc[i0] = norm_acc[i0].add(fnv);
            norm_acc[i1] = norm_acc[i1].add(fnv);
            norm_acc[i2] = norm_acc[i2].add(fnv);
        }
        let strains = self.core.tri_strains();
        let mut strain_sum = vec![0.0_f32; n];
        let mut tension_sum = vec![0.0_f32; n];
        let mut cnt = vec![0u32; n];
        for (t, &(su, sv)) in self.core.tris.iter().zip(&strains) {
            let m = &self.core.mats[t.slot as usize];
            let pct = su.max(sv).max(0.0) * 100.0;
            // Membrane stress in kPa: line tension (N/m) over thickness.
            let kpa = (m.e_warp * su.max(0.0)).max(m.e_weft * sv.max(0.0)) / m.thickness.max(1e-5) * 1e-3;
            for &v in &t.v {
                strain_sum[v] += pct;
                tension_sum[v] += kpa;
                cnt[v] += 1;
            }
        }
        for i in 0..n {
            let p = self.core.pos[i];
            let nrm = norm_acc[i].normalized();
            self.flat_positions[i * 3] = p.x;
            self.flat_positions[i * 3 + 1] = p.y;
            self.flat_positions[i * 3 + 2] = p.z;
            self.flat_normals[i * 3] = nrm.x;
            self.flat_normals[i * 3 + 1] = nrm.y;
            self.flat_normals[i * 3 + 2] = nrm.z;
            let c = cnt[i].max(1) as f32;
            self.flat_strain_pct[i] = strain_sum[i] / c;
            self.flat_tension_kpa[i] = tension_sum[i] / c;
            let clr = match self.avatar.eval_sdf_within(p, 0.085) {
                Some((sdf, nn)) => {
                    let ct = &mut self.contacts[i];
                    ct.q = p.sub(nn.scale(sdf));
                    ct.n = nn;
                    ct.dist = sdf.max(0.0);
                    ct.at = p;
                    sdf
                }
                None => 0.085,
            };
            self.flat_clearance_mm[i] = (clr * 1000.0).max(0.0);
        }
    }
}

/// Projects `p` on the plane of triangle `abc` (normal `(b - a) x (c - a)`). Returns the signed
/// distance along that normal and the barycentric coordinates of the foot, or `None` when the
/// foot falls outside the triangle (a 20 % margin closes gaps at folded edges).
fn project_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<(f32, [f32; 3])> {
    const MARGIN: f32 = -0.2;
    let ab = b.sub(a);
    let ac = c.sub(a);
    let n = ab.cross(ac);
    let n2 = n.len_sq();
    if n2 < 1e-14 {
        return None;
    }
    let ap = p.sub(a);
    let d = ap.dot(n) / n2.sqrt();
    let w_b = ap.cross(ac).dot(n) / n2;
    let w_c = ab.cross(ap).dot(n) / n2;
    let w_a = 1.0 - w_b - w_c;
    if w_a < MARGIN || w_b < MARGIN || w_c < MARGIN {
        return None;
    }
    let (ca, cb, cc) = (w_a.max(0.0), w_b.max(0.0), w_c.max(0.0));
    let s = (ca + cb + cc).max(1e-6);
    Some((d, [ca / s, cb / s, cc / s]))
}

fn hex_to_rgb(hex: &str) -> [f32; 3] {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return [0.5, 0.5, 0.5];
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map(|v| v as f32 / 255.0).unwrap_or(0.5);
    [c(0), c(2), c(4)]
}

/// Neighbour position sums per vertex (for "away from the panel" directions at edges).
fn neighbour_sums(pos: &[Vec3], tris: &[[usize; 3]]) -> (Vec<Vec3>, Vec<u32>) {
    let mut sum = vec![Vec3::ZERO; pos.len()];
    let mut cnt = vec![0u32; pos.len()];
    for t in tris {
        for k in 0..3 {
            let a = t[k];
            for j in 1..3 {
                let b = t[(k + j) % 3];
                sum[a] = sum[a].add(pos[b]);
                cnt[a] += 1;
            }
        }
    }
    (sum, cnt)
}

/// Point and outward ("away from its panel") direction at chain parameter `u`.
fn chain_point(
    chain: &[(usize, f64)],
    u: f64,
    pos: &[Vec3],
    nb_sum: &[Vec3],
    nb_cnt: &[u32],
) -> (Vec3, Vec3) {
    let k = chain
        .iter()
        .position(|c| c.1 >= u)
        .unwrap_or(chain.len() - 1)
        .max(1);
    let (g0, u0) = chain[k - 1];
    let (g1, u1) = chain[k];
    let t = if u1 - u0 > 1e-9 {
        ((u - u0) / (u1 - u0)).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let p = pos[g0].add(pos[g1].sub(pos[g0]).scale(t));
    let away_at = |g: usize| -> Vec3 {
        if nb_cnt[g] == 0 {
            return Vec3::new(0.0, 1.0, 0.0);
        }
        pos[g].sub(nb_sum[g].scale(1.0 / nb_cnt[g] as f32))
    };
    let tangent = pos[g1].sub(pos[g0]).normalized();
    let mut away = away_at(g0).add(away_at(g1));
    away = away.sub(tangent.scale(away.dot(tangent)));
    let away = if away.len() < 1e-6 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        away.normalized()
    };
    (p, away)
}

fn build_interface_chain(
    edges: &[EdgeRef],
    side: i8,
    edge_vert_map: &HashMap<(Uuid, i8, Uuid), Vec<BoundVert>>,
    sewn_len: &HashMap<(Uuid, Uuid), f64>,
) -> Vec<(usize, f64)> {
    let lengths: Vec<f64> = edges
        .iter()
        .map(|er| *sewn_len.get(&(er.piece_id, er.edge_id)).unwrap_or(&1.0))
        .collect();
    let total: f64 = lengths.iter().sum::<f64>().max(1.0);
    let mut cum = 0.0;
    let mut chain = Vec::new();
    for (ei, er) in edges.iter().enumerate() {
        let vs = edge_vert_map
            .get(&(er.piece_id, side, er.edge_id))
            .or_else(|| edge_vert_map.get(&(er.piece_id, 1, er.edge_id)));
        if let Some(vs) = vs {
            for bv in vs {
                let local = if er.dir >= 0 {
                    bv.sewn_frac
                } else {
                    1.0 - bv.sewn_frac
                };
                chain.push((bv.global_idx, ((cum + local * lengths[ei]) / total).clamp(0.0, 1.0)));
            }
        }
        cum += lengths[ei];
    }
    chain.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    chain
}

fn pair_chains_barycentric(chain_a: &[(usize, f64)], chain_b: &[(usize, f64)], out: &mut Vec<BarySew>) {
    let mut a = chain_a.to_vec();
    let mut b = chain_b.to_vec();
    a.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal));
    b.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal));
    a.dedup_by(|x, y| x.0 == y.0 && (x.1 - y.1).abs() < 1e-5);
    b.dedup_by(|x, y| x.0 == y.0 && (x.1 - y.1).abs() < 1e-5);
    if a.is_empty() || b.is_empty() {
        return;
    }
    emit_one_way(&a, &b, out);
    emit_one_way(&b, &a, out);
}

fn emit_one_way(src: &[(usize, f64)], dst: &[(usize, f64)], out: &mut Vec<BarySew>) {
    if dst.len() == 1 {
        let b0 = dst[0].0;
        for &(a, _) in src {
            if a != b0 {
                out.push(BarySew { a, b0, b1: b0, w: 0.0 });
            }
        }
        return;
    }
    let mut seg = 0usize;
    for &(a, u) in src {
        while seg + 2 < dst.len() && dst[seg + 1].1 < u {
            seg += 1;
        }
        let (b0, u0) = dst[seg];
        let (b1, u1) = dst[seg + 1];
        if a == b0 || a == b1 {
            continue;
        }
        let du = u1 - u0;
        let w = if du < 1e-6 {
            0.5
        } else {
            ((u - u0) / du).clamp(0.0, 1.0) as f32
        };
        out.push(BarySew { a, b0, b1, w });
    }
}
