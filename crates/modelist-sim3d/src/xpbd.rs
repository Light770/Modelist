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

use crate::arrange::{close_darts_for_placement, place_panel, place_trim, ArrangeCtx, TrimAnchor};
use crate::avatar::{AvatarBodyForm, AvatarMesh, AvatarPose, ParametricAvatar, Vec3};
use crate::closures::garment_layer_for_role;
use crate::collide::{push_triangle_off_points, BodyPointGrid};
use crate::diagnostics::{drape_quality, segment_triangle, DrapeQuality};
use crate::material::{MaterialSet, SolverMaterial, LOCK_STIFFENING};
use crate::mesher::{edge_sewn_length_mm, mesh_pattern_piece_with_stations, PanelMesh};
use crate::seam_physics::compute_seam_band_data;
use crate::self_collide::{resolve_self_contacts, ClothSpatialHash, SelfContact};
use crate::sizing_map::{SizingBounds, SizingMap};
use crate::vbd::{GrabState, VbdGpuMeta, VbdSolverState, VBD_SUBSTEPS};
use modelist_core::curve::{edge_total_length, eval_edge_point, invert_edge_arc_length};
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
pub(crate) const SEW_FRAMES: u32 = 30;
/// Frames over which gravity ramps up to full once seams are closed.
pub(crate) const SETTLE_FRAMES: u32 = 15;
/// Frames simulated before the first render so seams are already closed.
pub(crate) const INIT_FRAMES: u32 = SEW_FRAMES + 10;
/// Cloth triangles closer than this to the body get the body-point test, m.
const NEAR_BAND: f32 = 0.012;
/// Largest displacement a vertex may take in one substep, m (explosion guard).
pub(crate) const MAX_STEP: f32 = 0.02;
/// Largest outward speed a contact may give a vertex in one substep, m (no bouncing).
pub(crate) const MAX_SEPARATION: f32 = 0.0003;
/// How far a supported waistline may sink below where it was placed, m.
const WAIST_SLACK: f32 = 0.015;

/// Pins vertex `a` to the point `(1 - w) * b0 + w * b1` of the mating seam.
#[derive(Debug, Clone)]
pub(crate) struct BarySew {
    pub a: usize,
    pub b0: usize,
    pub b1: usize,
    pub w: f32,
}

/// `(piece, side, dart, leg)` -> leg vertices with their fraction from edge (0) to apex (1).
type DartLegMap = HashMap<(Uuid, i8, Uuid, u8), Vec<(usize, f64)>>;

#[derive(Debug, Clone)]
pub(crate) struct BoundVert {
    pub global_idx: usize,
    pub sewn_frac: f64,
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

#[derive(Debug, Clone)]
pub(crate) struct ElasticChain {
    pub verts: Vec<usize>,
    pub seg_rest_orig: Vec<f32>,
    pub target_ratio: f32,
    pub stiffness_n_m: f32,
    pub width_m: f32,
    pub total_orig: f32,
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
    pub elastic_chains: Vec<ElasticChain>,
    pub edges: Vec<EdgeLimit>,
    edge_max: Vec<f32>,
    tether_max: Vec<f32>,
    /// `(compliance, lock strain)` per tether.
    tether_alpha: Vec<(f32, f32)>,
    pub(crate) sew: Vec<BarySew>,
    pub mats: Vec<SolverMaterial>,
    tri_alpha: Vec<[f32; 3]>,
    hinge_alpha: Vec<f32>,
    pub(crate) vert_damp: Vec<f32>,
    pub(crate) vert_mass_mult: Vec<f32>,
    pub(crate) hinge_bend_mult: Vec<f32>,
    pub(crate) cross_seam_hinge_start: usize,
    pub(crate) cross_seam_enabled: bool,
    pub tri_elastic_target: Vec<f32>,
}

impl ClothCore {
    pub fn new(
        pos: Vec<Vec3>,
        tris: Vec<TriElem>,
        hinges: Vec<HingeElem>,
        mats: Vec<SolverMaterial>,
    ) -> Self {
        let n = pos.len();
        let nh = hinges.len();
        let ntri = tris.len();
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
            elastic_chains: Vec::new(),
            edges: Vec::new(),
            edge_max: Vec::new(),
            tether_max: Vec::new(),
            tether_alpha: Vec::new(),
            sew: Vec::new(),
            mats,
            tri_alpha: Vec::new(),
            hinge_alpha: Vec::new(),
            vert_damp: vec![0.0; n],
            vert_mass_mult: vec![1.0; n],
            hinge_bend_mult: vec![1.0; nh],
            cross_seam_hinge_start: nh,
            cross_seam_enabled: false,
            tri_elastic_target: vec![1.0; ntri],
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
            .enumerate()
            .map(|(hi, h)| {
                let m = &self.mats[h.slot as usize];
                let mult = self.hinge_bend_mult.get(hi).copied().unwrap_or(1.0).max(1.0);
                let b = (m.b_warp * h.warp_w + m.b_weft * (1.0 - h.warp_w)).max(1e-9)
                    * BEND_GAIN
                    * mult;
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
            let mmult = self.vert_mass_mult.get(i).copied().unwrap_or(1.0).max(1.0);
            self.inv_mass[i] = if self.pinned[i] {
                0.0
            } else {
                1.0 / (mass[i] * mmult).max(1e-7)
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

    pub fn solve_sew_inelastic(&mut self, max_pull: f32) {
        let pos = &mut self.pos;
        let prev = &mut self.prev;
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
            let da = corr.scale(wa);
            let db0 = corr.scale(wb0);
            let db1 = corr.scale(wb1);
            pos[s.a] = pos[s.a].add(da);
            prev[s.a] = prev[s.a].add(da);
            pos[s.b0] = pos[s.b0].sub(db0);
            prev[s.b0] = prev[s.b0].sub(db0);
            pos[s.b1] = pos[s.b1].sub(db1);
            prev[s.b1] = prev[s.b1].sub(db1);
        }
    }

    pub fn solve_membrane(&mut self, dt: f32, elastic_engage: f32) {
        let inv_dt2 = 1.0 / (dt * dt);
        let pos = &mut self.pos;
        let inv = &self.inv_mass;
        let engage = elastic_engage.clamp(0.0, 1.0);
        for (ti, (t, alpha)) in self.tris.iter().zip(&self.tri_alpha).enumerate() {
            let m = &self.mats[t.slot as usize];
            let [i0, i1, i2] = t.v;
            let w = [inv[i0], inv[i1], inv[i2]];
            if w[0] + w[1] + w[2] <= 0.0 {
                continue;
            }
            let el_target = self.tri_elastic_target.get(ti).copied().unwrap_or(1.0);
            let weft_scale = 1.0 - (1.0 - el_target) * engage;
            // Warp and weft stretch.
            for (axis, c, rest, lock, a0) in [
                (0, &t.cu, m.rest_warp, m.lock_warp, alpha[0]),
                (1, &t.cv, m.rest_weft * weft_scale, m.lock_weft, alpha[1]),
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

    /// Compliant contracting elastic band along an edge (jogger waistbands & ankle cuffs).
    pub fn solve_elastic_chains(&mut self, dt: f32, engage: f32) {
        if self.elastic_chains.is_empty() || engage <= 1e-4 {
            return;
        }
        let inv_dt2 = 1.0 / (dt * dt);
        let pos = &mut self.pos;
        let prev = &mut self.prev;
        let inv = &self.inv_mass;
        for ec in &self.elastic_chains {
            let n = ec.verts.len();
            if n < 2 {
                continue;
            }
            let ratio = 1.0 - (1.0 - ec.target_ratio) * engage.clamp(0.0, 1.0);
            let k_band = (ec.stiffness_n_m * ec.width_m.max(0.10)).max(28.0);

            // Per-segment elastic springs toward `orig * ratio` (critically damped via `prev`).
            for _pass in 0..2 {
                for k in 0..n - 1 {
                    let u = ec.verts[k];
                    let v = ec.verts[k + 1];
                    let wsum = inv[u] + inv[v];
                    if wsum <= 1e-12 {
                        continue;
                    }
                    let orig = ec.seg_rest_orig[k];
                    let target = orig * ratio;
                    let d = pos[v].sub(pos[u]);
                    let l = d.len();
                    if l <= target || l < 1e-9 {
                        continue;
                    }
                    let alpha = orig / k_band;
                    let dl = (l - target) / (wsum + alpha * inv_dt2);
                    let corr = d.scale(dl / l);
                    let du = corr.scale(inv[u]);
                    let dv = corr.scale(inv[v]);
                    pos[u] = pos[u].add(du);
                    pos[v] = pos[v].sub(dv);
                    prev[u] = prev[u].add(du.scale(0.85));
                    prev[v] = prev[v].sub(dv.scale(0.85));
                }
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
        self.solve_membrane(dt, 0.0);
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
        let limit = if self.cross_seam_enabled {
            self.hinges.len()
        } else {
            self.cross_seam_hinge_start.min(self.hinges.len())
        };
        for ((h, &alpha), lam) in self.hinges[..limit]
            .iter()
            .zip(&self.hinge_alpha[..limit])
            .zip(self.hinge_lambda[..limit].iter_mut())
        {
            let [a, b, c, d] = h.v;
            let k = h.k;
            let cvec = pos[a]
                .scale(k[0])
                .add(pos[b].scale(k[1]))
                .add(pos[c].scale(k[2]))
                .add(pos[d].scale(k[3]));
            let clen = cvec.len();
            let max_c = 0.65 * h.area2.sqrt();
            let cvec = if clen > max_c && clen > 1e-9 {
                cvec.scale((max_c + 0.25 * (clen - max_c)) / clen)
            } else {
                cvec
            };
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sim3dOptions {
    pub mesh_spacing_mm: f64,
    pub enable_self_collision: bool,
    pub wind_mps: [f32; 3],
    pub use_vbd: bool,
}

impl Default for Sim3dOptions {
    fn default() -> Self {
        Self {
            mesh_spacing_mm: MESH_SPACING_MM,
            enable_self_collision: true,
            wind_mps: [0.0, 0.0, 0.0],
            use_vbd: false,
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
    pub solver_mode: String,
    pub mesh_spacing_mm: f32,
    pub adaptive_mesh: bool,
    pub min_edge_mm: f32,
    pub max_edge_mm: f32,
    pub pin_count: usize,
    pub self_collision_enabled: bool,
    pub wind_speed_mps: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Contact {
    pub q: Vec3,
    pub n: Vec3,
    pub at: Vec3,
    pub dist: f32,
}

pub(crate) struct PanelRange {
    pub start: usize,
    pub end: usize,
    pub tri_start: usize,
    pub tri_end: usize,
}

/// Strictly decoupled, on-demand anisotropic VBD/XPBD cloth simulation of a garment on the avatar.
/// Allocated only while the 3D Fit Room is open; dropped on close.
pub struct ClothSimSession {
    pub avatar: ParametricAvatar,
    pub avatar_mesh: AvatarMesh,
    /// Shell fabric (kept for stats and API compatibility).
    pub fabric: FabricSpec,
    materials: MaterialSet,
    slot_roles: Vec<String>,
    options: Sim3dOptions,
    core: ClothCore,
    vbd: VbdSolverState,
    self_hash: ClothSpatialHash,
    self_contacts: Vec<SelfContact>,
    seam_welds: Vec<Vec<usize>>,
    tri_layer: Vec<u8>,
    avg_thickness: f32,
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
    p99_motion_mm_s: f32,
    max_motion_mm_s: f32,
    min_edge_mm: f32,
    max_edge_mm: f32,
    pub flat_positions: Vec<f32>,
    pub flat_normals: Vec<f32>,
    pub flat_strain_pct: Vec<f32>,
    pub flat_tension_kpa: Vec<f32>,
    pub flat_clearance_mm: Vec<f32>,
    pub flat_seam_mask: Vec<f32>,
    vert_render_thick: Vec<f32>,
    open_boundary_edges: Vec<(usize, usize)>,
    render_indices: Vec<u32>,
    render_positions: Vec<f32>,
    render_normals: Vec<f32>,
    render_colors: Vec<f32>,
    render_strain: Vec<f32>,
    render_tension: Vec<f32>,
    render_clearance: Vec<f32>,
    render_seam_mask: Vec<f32>,
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
        Self::new_with_options(
            pieces,
            stitching,
            measurements,
            materials,
            pose,
            body_form,
            Sim3dOptions::default(),
        )
    }

    pub fn new_with_options(
        pieces: &[PatternPiece],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        materials: &MaterialSet,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
        options: Sim3dOptions,
    ) -> Self {
        let mut sim = Self::new_unsettled_with_options(
            pieces,
            stitching,
            measurements,
            materials,
            pose,
            body_form,
            options,
        );
        sim.step(INIT_FRAMES);
        sim
    }

    /// Builds the session with panels at their initial arrangement, before any simulation.
    #[doc(hidden)]
    pub fn new_unsettled(
        pieces: &[PatternPiece],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        materials: &MaterialSet,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
    ) -> Self {
        Self::new_unsettled_with_options(
            pieces,
            stitching,
            measurements,
            materials,
            pose,
            body_form,
            Sim3dOptions::default(),
        )
    }

    #[doc(hidden)]
    pub fn new_unsettled_with_options(
        pieces: &[PatternPiece],
        stitching: &StitchingGraph,
        measurements: &BodyMeasurements,
        materials: &MaterialSet,
        pose: AvatarPose,
        body_form: AvatarBodyForm,
        options: Sim3dOptions,
    ) -> Self {
        let adaptive = options.mesh_spacing_mm <= 0.0;
        let spacing_mm = if adaptive { 0.0 } else { options.mesh_spacing_mm.clamp(6.0, 35.0) };
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
        let mut min_edge_mm = f64::INFINITY;
        let mut max_edge_mm = 0.0_f64;

        let mut edge_vert_map: HashMap<(Uuid, i8, Uuid), Vec<BoundVert>> = HashMap::new();
        let mut dart_map: DartLegMap = HashMap::new();
        let mut sewn_len: HashMap<(Uuid, Uuid), f64> = HashMap::new();
        let mut pre_sewn_len: HashMap<(Uuid, Uuid), f64> = HashMap::new();
        let mut piece_sizing_map: HashMap<Uuid, SizingMap> = HashMap::new();
        for p in pieces {
            for (ei, e) in p.seam_loop.edges.iter().enumerate() {
                let total = edge_total_length(e).max(1.0);
                sewn_len.insert((p.id, e.id), total);
                if p.visible {
                    pre_sewn_len.insert((p.id, e.id), edge_sewn_length_mm(p, ei).max(1.0));
                }
            }
            if adaptive && p.visible {
                let sm = materials.solver_material(&p.fabric_role);
                piece_sizing_map.insert(
                    p.id,
                    SizingMap::from_piece(p, stitching, pieces, &sm, SizingBounds::default()),
                );
            }
        }
        let edge_stations = build_conformal_edge_stations(
            pieces,
            stitching,
            &piece_sizing_map,
            &pre_sewn_len,
            if adaptive { 14.0 } else { spacing_mm },
        );
        // Trims are laid onto their partner seams once every other panel is placed.
        let mut trims: Vec<(usize, usize, PanelMesh)> = Vec::new(); // (piece idx, panel idx, mesh)

        for (pi, p) in pieces.iter().enumerate() {
            if !p.visible {
                continue;
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

            let piece_sizing = piece_sizing_map.get(&p.id);

            for &(mirror_x, side) in sides {
                let pm_opt = mesh_pattern_piece_with_stations(
                    p,
                    mirror_x,
                    unfold,
                    spacing_mm,
                    piece_sizing,
                    Some(&edge_stations),
                );
                let Some(pm) = pm_opt else {
                    continue;
                };
                min_edge_mm = min_edge_mm.min(pm.min_edge_mm);
                max_edge_mm = max_edge_mm.max(pm.max_edge_mm);
                let zone = if mirror_x && pm.arrangement_zone == ArrangementZone::SleeveLeft {
                    ArrangementZone::SleeveRight
                } else {
                    pm.arrangement_zone
                };
                let start = positions.len();
                let placed = if pm.darts_closed > 0 {
                    let closed = close_darts_for_placement(&pm);
                    place_panel(&avatar, &closed, p, zone, side as f32, spawn_pair, &ctx)
                } else {
                    place_panel(&avatar, &pm, p, zone, side as f32, spawn_pair, &ctx)
                };
                positions.extend(placed);
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

        // Seam-line length limits and elastic bands: every meshed panel edge, ordered along the edge.
        let mut chain_specs: Vec<(Vec<usize>, f32, u16)> = Vec::new();
        let mut elastic_specs: Vec<(Vec<usize>, f32, f32, f32)> = Vec::new();
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
                    let verts: Vec<usize> = ordered.into_iter().map(|o| o.1).collect();
                    let rest = (*sewn_len.get(&(p.id, e.id)).unwrap_or(&0.0) * 0.001) as f32;
                    if let Some(el) = e.elastic {
                        if el.target_ratio < 0.995 {
                            elastic_specs.push((
                                verts.clone(),
                                (el.target_ratio as f32).clamp(0.45, 0.99),
                                (el.stiffness_n_m as f32).clamp(40.0, 1200.0),
                                (el.width_mm as f32 * 0.001).clamp(0.008, 0.12),
                            ));
                        }
                    }
                    if rest > 0.01 {
                        chain_specs.push((verts, rest, slot));
                    }
                }
            }
        }

        // Seam bindings (arc-length matched on the sewn length, so closed darts never bunch).
        let mut seen: HashSet<(usize, usize, usize, i32)> = HashSet::new();
        for binding in &stitching.bindings {
            let (spans_a, spans_b) = aligned_interface_spans(
                &binding.interface_a.edges,
                &binding.interface_b.edges,
                &sewn_len,
            );
            for side in [1i8, -1i8] {
                let ca = build_interface_chain_with_spans(
                    &binding.interface_a.edges,
                    &spans_a,
                    side,
                    &edge_vert_map,
                );
                let cb = build_interface_chain_with_spans(
                    &binding.interface_b.edges,
                    &spans_b,
                    side,
                    &edge_vert_map,
                );
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
                    let (own_spans, other_spans) =
                        aligned_interface_spans(&own.edges, &other.edges, &sewn_len);
                    for side in [1i8, -1i8] {
                        let own_chain = build_interface_chain_with_spans(
                            &own.edges,
                            &own_spans,
                            side,
                            &edge_vert_map,
                        );
                        let other_chain = build_interface_chain_with_spans(
                            &other.edges,
                            &other_spans,
                            side,
                            &edge_vert_map,
                        );
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
        let (seam_welds, vert_root) = build_seam_welds(positions.len(), &sew);
        let interior_hinge_count = hinges.len();
        let cross_hinges = build_cross_seam_hinges(
            &tri_idx,
            &tri_grain,
            &tri_slot,
            &rest2d,
            &vert_root,
        );
        let mut hinges = hinges;
        hinges.extend(cross_hinges);

        let elastic_chains: Vec<ElasticChain> = elastic_specs
            .into_iter()
            .filter_map(|(verts, target_ratio, stiffness_n_m, width_m)| {
                let mut seg_rest_orig = Vec::with_capacity(verts.len().saturating_sub(1));
                let mut total_orig = 0.0_f32;
                for w in verts.windows(2) {
                    let l = rest2d[w[1]].sub(rest2d[w[0]]).len() as f32;
                    seg_rest_orig.push(l.max(1e-4));
                    total_orig += l;
                }
                if total_orig < 0.01 {
                    return None;
                }
                Some(ElasticChain {
                    verts,
                    seg_rest_orig,
                    target_ratio,
                    stiffness_n_m,
                    width_m,
                    total_orig,
                })
            })
            .collect();
        let mut core = ClothCore::new(positions, tris, hinges, mats);
        core.cross_seam_hinge_start = interior_hinge_count;
        core.tethers = tethers;
        core.chains = chains;
        core.elastic_chains = elastic_chains;
        core.refresh_materials();
        core.sew = sew;
        let n = core.pos.len();
        let ntri = core.tris.len();
        let mut vert_panel = vec![0u16; n];
        for (pi, pr) in panels.iter().enumerate() {
            for v in pr.start..pr.end {
                if v < n {
                    vert_panel[v] = pi as u16;
                }
            }
        }
        let mut vert_layer = vec![1u8; n];
        let mut tri_layer = vec![1u8; ntri];
        for (ti, t) in core.tris.iter().enumerate() {
            let role = slot_roles.get(t.slot as usize).map(|s| s.as_str()).unwrap_or("shell");
            let lyr = garment_layer_for_role(role);
            tri_layer[ti] = lyr;
            for &vi in &t.v {
                if vi < n {
                    vert_layer[vi] = lyr;
                }
            }
        }
        let seam_band = compute_seam_band_data(
            pieces,
            stitching,
            &rest2d,
            &panels,
            &vert_panel,
            &edge_vert_map,
            &hinge_idx,
        );
        core.vert_mass_mult = seam_band.vert_mass_mult;
        let mut hinge_bend_mult = seam_band.hinge_bend_mult;
        if core.hinges.len() > hinge_bend_mult.len() {
            for h in &core.hinges[hinge_bend_mult.len()..] {
                let m0 = seam_band.vert_seam_mask.get(h.v[0]).copied().unwrap_or(0.0);
                let m1 = seam_band.vert_seam_mask.get(h.v[1]).copied().unwrap_or(0.0);
                let mult = if m0.max(m1) > 0.5 { 1.25 } else { 1.15 };
                hinge_bend_mult.push(mult);
            }
        }
        core.hinge_bend_mult = hinge_bend_mult;
        let mut tri_elastic_target = vec![1.0_f32; ntri];
        for p in pieces.iter().filter(|p| p.visible) {
            for e in &p.seam_loop.edges {
                let Some(el) = e.elastic else { continue };
                if el.target_ratio >= 0.995 {
                    continue;
                }
                let band_m = (el.width_mm * 0.001).clamp(0.015, 0.08);
                let band_sq = band_m * band_m;
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
                        let pt = rest2d[bv.global_idx];
                        min_x = min_x.min(pt.x);
                        min_y = min_y.min(pt.y);
                        max_x = max_x.max(pt.x);
                        max_y = max_y.max(pt.y);
                    }
                    min_x -= band_m;
                    min_y -= band_m;
                    max_x += band_m;
                    max_y += band_m;
                    for ti in pr.tri_start..pr.tri_end.min(ntri) {
                        let tv = core.tris[ti].v;
                        let c = rest2d[tv[0]]
                            .add(rest2d[tv[1]])
                            .add(rest2d[tv[2]])
                            .scale(1.0 / 3.0);
                        if c.x < min_x || c.x > max_x || c.y < min_y || c.y > max_y {
                            continue;
                        }
                        let mut best_d2 = f64::INFINITY;
                        if bvs.len() == 1 {
                            best_d2 = c.sub(rest2d[bvs[0].global_idx]).len_sq();
                        } else {
                            for w in bvs.windows(2) {
                                let a = rest2d[w[0].global_idx];
                                let b = rest2d[w[1].global_idx];
                                let ab = b.sub(a);
                                let l2 = ab.len_sq();
                                let d2 = if l2 < 1e-12 {
                                    c.sub(a).len_sq()
                                } else {
                                    let u = (c.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
                                    c.sub(a.add(ab.scale(u))).len_sq()
                                };
                                if d2 < best_d2 {
                                    best_d2 = d2;
                                }
                            }
                        }
                        if best_d2 <= band_sq {
                            tri_elastic_target[ti] =
                                tri_elastic_target[ti].min(el.target_ratio as f32);
                        }
                    }
                }
            }
        }
        core.tri_elastic_target = tri_elastic_target;
        core.refresh_materials();
        let vbd = VbdSolverState::new(&core);
        let self_tris_init: Vec<[usize; 3]> = core.tris.iter().map(|t| t.v).collect();
        let self_hash = ClothSpatialHash::new(vert_panel, vert_layer, &core.sew, &seam_welds, &self_tris_init);
        let mut edge_root_counts: HashMap<(usize, usize), usize> = HashMap::new();
        for tri in triangles.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            for &(a, b) in &[(i0, i1), (i1, i2), (i2, i0)] {
                let ra = vert_root.get(a).copied().unwrap_or(a);
                let rb = vert_root.get(b).copied().unwrap_or(b);
                let key = if ra < rb { (ra, rb) } else { (rb, ra) };
                *edge_root_counts.entry(key).or_insert(0) += 1;
            }
        }
        let mut open_boundary_edges = Vec::new();
        for tri in triangles.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            for &(a, b) in &[(i0, i1), (i1, i2), (i2, i0)] {
                let ra = vert_root.get(a).copied().unwrap_or(a);
                let rb = vert_root.get(b).copied().unwrap_or(b);
                let key = if ra < rb { (ra, rb) } else { (rb, ra) };
                if edge_root_counts.get(&key).copied().unwrap_or(0) == 1 {
                    open_boundary_edges.push((a, b));
                }
            }
        }
        let n_render_verts = 2 * n + 4 * open_boundary_edges.len();
        let mut render_indices = Vec::with_capacity(triangles.len() * 2 + open_boundary_edges.len() * 6);
        render_indices.extend_from_slice(&triangles);
        for tri in triangles.chunks_exact(3) {
            render_indices.push(n as u32 + tri[0]);
            render_indices.push(n as u32 + tri[2]);
            render_indices.push(n as u32 + tri[1]);
        }
        for ei in 0..open_boundary_edges.len() {
            let b = (2 * n + 4 * ei) as u32;
            render_indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        let avg_thickness = if core.mats.is_empty() {
            0.0005
        } else {
            core.mats.iter().map(|m| m.thickness).sum::<f32>() / core.mats.len() as f32
        };
        let mut sim = Self {
            avatar,
            avatar_mesh,
            fabric: materials.shell.clone(),
            materials: materials.clone(),
            slot_roles,
            options: Sim3dOptions {
                mesh_spacing_mm: spacing_mm,
                ..options
            },
            core,
            vbd,
            self_hash,
            self_contacts: Vec::new(),
            seam_welds,
            tri_layer,
            avg_thickness,
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
            p99_motion_mm_s: 0.0,
            max_motion_mm_s: 0.0,
            min_edge_mm: if min_edge_mm.is_finite() { min_edge_mm as f32 } else { 0.0 },
            max_edge_mm: max_edge_mm as f32,
            flat_positions: vec![0.0; n * 3],
            flat_normals: vec![0.0; n * 3],
            flat_strain_pct: vec![0.0; n],
            flat_tension_kpa: vec![0.0; n],
            flat_clearance_mm: vec![0.0; n],
            flat_seam_mask: seam_band.vert_seam_mask,
            vert_render_thick: vec![0.001; n],
            open_boundary_edges,
            render_indices,
            render_positions: vec![0.0; n_render_verts * 3],
            render_normals: vec![0.0; n_render_verts * 3],
            render_colors: vec![0.5; n_render_verts * 3],
            render_strain: vec![0.0; n_render_verts],
            render_tension: vec![0.0; n_render_verts],
            render_clearance: vec![0.0; n_render_verts],
            render_seam_mask: vec![0.0; n_render_verts],
        };
        sim.refresh_contact_params();
        sim.refresh_render_thickness_and_colors();
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
        self.avg_thickness = if self.core.mats.is_empty() {
            0.0005
        } else {
            self.core.mats.iter().map(|m| m.thickness).sum::<f32>() / self.core.mats.len() as f32
        };
        self.refresh_contact_params();
        self.refresh_render_thickness_and_colors();
    }

    pub fn set_wind(&mut self, wind: [f32; 3]) {
        self.options.wind_mps = wind;
    }

    pub fn set_self_collision(&mut self, enabled: bool) {
        self.options.enable_self_collision = enabled;
    }

    pub fn pick_vertex(&self, ray_origin: [f32; 3], ray_dir: [f32; 3]) -> Option<(usize, [f32; 3])> {
        let orig = Vec3::new(ray_origin[0], ray_origin[1], ray_origin[2]);
        let dir = Vec3::new(ray_dir[0], ray_dir[1], ray_dir[2]).normalized();
        let mut best_t = f32::INFINITY;
        let mut best: Option<(usize, [f32; 3])> = None;
        for tri in self.triangles.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let a = self.core.pos[i0];
            let b = self.core.pos[i1];
            let c = self.core.pos[i2];
            if let Some(s) = segment_triangle(orig, orig.add(dir.scale(20.0)), a, b, c) {
                let t = s * 20.0;
                if t < best_t {
                    best_t = t;
                    let hit = orig.add(dir.scale(t));
                    let d0 = hit.sub(a).len_sq();
                    let d1 = hit.sub(b).len_sq();
                    let d2 = hit.sub(c).len_sq();
                    let v = if d0 <= d1 && d0 <= d2 {
                        i0
                    } else if d1 <= d2 {
                        i1
                    } else {
                        i2
                    };
                    best = Some((v, [hit.x, hit.y, hit.z]));
                }
            }
        }
        best
    }

    pub fn begin_grab(&mut self, vert_idx: usize, target: [f32; 3], radius_m: f32) {
        if vert_idx < self.core.pos.len() {
            self.vbd.grab = Some(GrabState {
                vert: vert_idx,
                target: Vec3::new(target[0], target[1], target[2]),
                radius_m: radius_m.clamp(0.03, 0.45),
            });
        }
    }

    pub fn move_grab(&mut self, target: [f32; 3]) {
        if let Some(ref mut g) = self.vbd.grab {
            g.target = Vec3::new(target[0], target[1], target[2]);
        }
    }

    pub fn end_grab(&mut self) {
        self.vbd.grab = None;
    }

    pub fn toggle_pin(&mut self, vert_idx: usize) -> bool {
        if vert_idx >= self.core.pos.len() {
            return false;
        }
        if self.vbd.user_pins.remove(&vert_idx).is_some() {
            false
        } else {
            self.vbd.user_pins.insert(vert_idx, self.core.pos[vert_idx]);
            true
        }
    }

    pub fn clear_pins(&mut self) {
        self.vbd.user_pins.clear();
    }

    pub fn pin_positions(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.vbd.user_pins.len() * 3);
        for (&vi, &p) in &self.vbd.user_pins {
            let pos = self.core.pos.get(vi).copied().unwrap_or(p);
            out.push(pos.x);
            out.push(pos.y);
            out.push(pos.z);
        }
        out
    }

    pub fn vbd_gpu_meta(&self) -> VbdGpuMeta {
        self.vbd.gpu_meta(&self.core)
    }

    pub fn vbd_gpu_u32(&self, kind: &str) -> Vec<u32> {
        self.vbd.gpu_u32_buffer(&self.core, kind)
    }

    pub fn vbd_gpu_f32(&self, kind: &str) -> Vec<f32> {
        self.vbd.gpu_f32_buffer(&self.core, kind)
    }

    fn refresh_contact_params(&mut self) {
        let n = self.core.pos.len();
        let mut off = vec![0.0_f32; n];
        let mut fr = vec![0.0_f32; n];
        let mut cnt = vec![0u32; n];
        for t in &self.core.tris {
            let m = &self.core.mats[t.slot as usize];
            for &v in &t.v {
                off[v] = off[v].max(m.contact_offset());
                fr[v] += m.friction;
                cnt[v] += 1;
            }
        }
        for i in 0..n {
            self.vert_offset[i] = if off[i] > 0.0 { off[i] } else { 0.003 };
            self.vert_friction[i] = if cnt[i] > 0 { fr[i] / cnt[i] as f32 } else { 0.4 };
        }
    }

    pub fn triangle_indices(&self) -> &[u32] {
        &self.triangles
    }

    pub fn render_positions(&self) -> &[f32] {
        &self.render_positions
    }

    pub fn render_normals(&self) -> &[f32] {
        &self.render_normals
    }

    pub fn render_indices(&self) -> &[u32] {
        &self.render_indices
    }

    pub fn render_colors(&self) -> &[f32] {
        &self.render_colors
    }

    pub fn render_strain(&self) -> &[f32] {
        &self.render_strain
    }

    pub fn render_tension(&self) -> &[f32] {
        &self.render_tension
    }

    pub fn render_clearance(&self) -> &[f32] {
        &self.render_clearance
    }

    pub fn render_seam_mask(&self) -> &[f32] {
        &self.render_seam_mask
    }

    fn refresh_render_thickness_and_colors(&mut self) {
        let n = self.core.pos.len();
        let mut thick = vec![0.0_f32; n];
        for t in &self.core.tris {
            let m = &self.core.mats[t.slot as usize];
            let role = self
                .slot_roles
                .get(t.slot as usize)
                .map(|r| r.to_lowercase())
                .unwrap_or_default();
            let role_mult = if role.contains("rib") || role.contains("waistband") || role.contains("cuff") {
                1.8
            } else {
                1.0
            };
            let t_m = m.thickness.max(0.0008) * role_mult;
            for &v in &t.v {
                thick[v] = thick[v].max(t_m);
            }
        }
        for i in 0..n {
            if thick[i] <= 0.0 {
                thick[i] = 0.0008;
            }
            if self.flat_seam_mask.get(i).copied().unwrap_or(0.0) >= 2.5 {
                thick[i] *= 1.5;
            }
        }
        for cluster in &self.seam_welds {
            if cluster.len() < 2 {
                continue;
            }
            let avg = cluster.iter().map(|&v| thick[v]).sum::<f32>() / cluster.len() as f32;
            for &v in cluster {
                thick[v] = avg;
            }
        }
        self.vert_render_thick = thick;

        let base_cols = self.cloth_colors();
        for i in 0..n {
            let r = base_cols[i * 3];
            let g = base_cols[i * 3 + 1];
            let b = base_cols[i * 3 + 2];
            self.render_colors[i * 3..i * 3 + 3].copy_from_slice(&[r, g, b]);
            let ii = n + i;
            self.render_colors[ii * 3..ii * 3 + 3].copy_from_slice(&[r * 0.78, g * 0.78, b * 0.78]);
        }
        for (ei, &(a, b)) in self.open_boundary_edges.iter().enumerate() {
            let base = 2 * n + 4 * ei;
            for (k, src) in [a, b, b, a].into_iter().enumerate() {
                let dst = (base + k) * 3;
                self.render_colors[dst] = base_cols[src * 3] * 0.90;
                self.render_colors[dst + 1] = base_cols[src * 3 + 1] * 0.90;
                self.render_colors[dst + 2] = base_cols[src * 3 + 2] * 0.90;
            }
        }
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

    pub fn seam_weld_cluster_count(&self) -> usize {
        self.seam_welds.len()
    }

    pub fn max_junction_cluster_size(&self) -> usize {
        self.seam_welds.iter().map(|c| c.len()).max().unwrap_or(0)
    }

    pub fn junction_cluster_count(&self, min_size: usize) -> usize {
        self.seam_welds.iter().filter(|c| c.len() >= min_size).count()
    }

    /// Returns `(current_mm, uncontracted_orig_mm)` summed over all active elastic bands.
    pub fn elastic_band_length_mm(&self) -> (f32, f32) {
        let mut cur = 0.0_f32;
        let mut orig = 0.0_f32;
        for ec in &self.core.elastic_chains {
            for w in ec.verts.windows(2) {
                cur += self.core.pos[w[1]].sub(self.core.pos[w[0]]).len();
            }
            orig += ec.total_orig;
        }
        (cur * 1000.0, orig * 1000.0)
    }

    pub fn p99_motion_mm_s(&self) -> f32 {
        self.p99_motion_mm_s
    }

    pub fn max_motion_mm_s(&self) -> f32 {
        self.max_motion_mm_s
    }

    pub fn max_weld_spread_mm(&self) -> f32 {
        let mut max_mm = 0.0_f32;
        for cluster in &self.seam_welds {
            if cluster.len() < 2 {
                continue;
            }
            let p0 = self.core.pos[cluster[0]];
            for &v in &cluster[1..] {
                max_mm = max_mm.max(self.core.pos[v].sub(p0).len() * 1000.0);
            }
        }
        max_mm
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
        let use_vbd_now = self.options.use_vbd;
        let wind = Vec3::new(
            self.options.wind_mps[0],
            self.options.wind_mps[1],
            self.options.wind_mps[2],
        );
        let self_thick = (2.0 * self.avg_thickness).max(0.0028);
        for _ in 0..num_steps {
            self.step_count += 1;
            let f = self.step_count as f32;
            let (sew_frames, settle_frames) = (SEW_FRAMES as f32, SETTLE_FRAMES as f32);
            self.core.cross_seam_enabled = f >= sew_frames;
            let run_vbd = use_vbd_now && f >= sew_frames;
            let substeps = if run_vbd { VBD_SUBSTEPS } else { SUBSTEPS };
            let dt = 1.0 / (60.0 * substeps as f32);
            let grav = if f < sew_frames {
                0.10
            } else {
                (0.10 + (f - sew_frames) / settle_frames).min(1.0)
            };
            let max_pull = if f < sew_frames {
                0.0005 + 0.0015 * (f / sew_frames)
            } else if f < sew_frames + settle_frames {
                0.004
            } else {
                0.05
            };
            let sew_scale = if f < sew_frames {
                0.35 + 0.65 * (f / sew_frames)
            } else {
                1.0
            };
            let gravity = Vec3::new(0.0, -9.81 * grav, 0.0);
            self.update_near_flags();
            let frame_start: Vec<Vec3> = self.core.pos.clone();

            let extra = if f < sew_frames {
                0.55
            } else if f < sew_frames + settle_frames {
                0.55 + 0.45 * (f - sew_frames) / settle_frames
            } else {
                1.0
            };
            let elastic_engage = if f < sew_frames {
                0.0
            } else {
                ((f - sew_frames) / settle_frames).clamp(0.0, 1.0)
            };
            let jitter_ramp = ((f - (sew_frames + settle_frames + 30.0)) / 25.0).clamp(0.0, 1.0);
            let self_tris: Vec<[usize; 3]> = if self.options.enable_self_collision && f >= sew_frames {
                self.core.tris.iter().map(|t| t.v).collect()
            } else {
                Vec::new()
            };
            for s in 0..substeps {
                if run_vbd {
                    self.vbd.step_substep(
                        &mut self.core,
                        &self.contacts,
                        &self.vert_offset,
                        dt,
                        gravity,
                        extra,
                        sew_scale,
                        wind,
                        false,
                    );
                    self.support_waistline();
                    self.core.solve_tethers(dt);
                    self.core.solve_bending(dt);
                    self.core.solve_strain_limits();
                    self.core.solve_chain_limits();
                    self.core.solve_elastic_chains(dt, elastic_engage);
                    self.core.solve_sew(max_pull);
                    self.collide_vertices(true);
                    self.collide_body_points();
                    if !self_tris.is_empty() {
                        if s % 4 == 0 {
                            self.self_hash
                                .rebuild(&self.core.pos, &self_tris, self_thick + 0.002);
                        }
                        self.self_hash.detect(
                            &self.core.pos,
                            &self_tris,
                            &self.tri_layer,
                            self_thick,
                            &mut self.self_contacts,
                        );
                        resolve_self_contacts(
                            &mut self.core.pos,
                            &mut self.core.prev,
                            &self.core.inv_mass,
                            &self.self_contacts,
                            0.06,
                        );
                    }
                    self.core.solve_sew_inelastic(max_pull * 0.5);
                    self.solve_seam_welds(f >= sew_frames);
                    for (&vi, &pin_pos) in &self.vbd.user_pins {
                        self.core.pos[vi] = pin_pos;
                        self.core.prev[vi] = pin_pos;
                    }
                    if jitter_ramp > 0.0
                        && self.vbd.grab.is_none()
                        && wind.len_sq() <= 1e-6
                    {
                        self.damp_micro_jitter(jitter_ramp);
                    }
                } else {
                    self.core.integrate_damped(dt, gravity, extra);
                    if wind.len_sq() > 1e-6 {
                        self.vbd.compute_aero(&self.core, dt, wind);
                        for i in 0..self.core.pos.len() {
                            if self.core.inv_mass[i] > 0.0 {
                                self.core.pos[i] = self.core.pos[i]
                                    .add(self.vbd.aero_acc[i].scale(dt * dt));
                            }
                        }
                    }
                    if let Some(grab) = self.vbd.grab {
                        let center = self.core.pos[grab.vert];
                        let r = grab.radius_m.max(1e-4);
                        for i in 0..self.core.pos.len() {
                            if self.core.inv_mass[i] <= 0.0 {
                                continue;
                            }
                            let d = self.core.pos[i].sub(center).len();
                            if d <= r {
                                let w = (-2.0 * (d / r) * (d / r)).exp();
                                let target_i = grab.target.add(self.core.pos[i].sub(center));
                                self.core.pos[i] = self.core.pos[i]
                                    .add(target_i.sub(self.core.pos[i]).scale(0.35 * w));
                            }
                        }
                    }
                    self.core.solve_sew(max_pull);
                    self.core.begin_substep();
                    self.core.solve_membrane(dt, elastic_engage);
                    self.core.solve_tethers(dt);
                    for _ in 0..BEND_PASSES {
                        self.core.solve_bending(dt);
                    }
                    self.support_waistline();
                    self.core.solve_strain_limits();
                    self.core.solve_chain_limits();
                    self.core.solve_elastic_chains(dt, elastic_engage);
                    if !self_tris.is_empty() {
                        if s % 4 == 0 {
                            self.self_hash
                                .rebuild(&self.core.pos, &self_tris, self_thick + 0.002);
                        }
                        self.self_hash.detect(
                            &self.core.pos,
                            &self_tris,
                            &self.tri_layer,
                            self_thick,
                            &mut self.self_contacts,
                        );
                        resolve_self_contacts(
                            &mut self.core.pos,
                            &mut self.core.prev,
                            &self.core.inv_mass,
                            &self.self_contacts,
                            0.06,
                        );
                    }
                    self.core.solve_sew(max_pull);
                    self.collide_vertices(true);
                    self.collide_body_points();
                    self.core.solve_sew_inelastic(max_pull * 0.5);
                    self.solve_seam_welds(f >= sew_frames);
                    for (&vi, &pin_pos) in &self.vbd.user_pins {
                        self.core.pos[vi] = pin_pos;
                        self.core.prev[vi] = pin_pos;
                    }
                    if jitter_ramp > 0.0
                        && self.vbd.grab.is_none()
                        && wind.len_sq() <= 1e-6
                    {
                        self.damp_micro_jitter(jitter_ramp);
                    }
                }
            }

            let mut sum = 0.0_f32;
            let mut max_sp = 0.0_f32;
            let mut speeds = Vec::with_capacity(frame_start.len());
            for (a, b) in frame_start.iter().zip(&self.core.pos) {
                let d = b.sub(*a).len();
                sum += d;
                let sp = d * 60.0 * 1000.0;
                max_sp = max_sp.max(sp);
                speeds.push(sp);
            }
            speeds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.motion_mm_s = sum / frame_start.len().max(1) as f32 * 60.0 * 1000.0;
            self.max_motion_mm_s = max_sp;
            self.p99_motion_mm_s = if speeds.is_empty() {
                0.0
            } else {
                speeds[((speeds.len() - 1) as f32 * 0.99) as usize]
            };
        }
        self.sync_diagnostic_buffers();
    }

    /// Smoothly damps sub-threshold constraint-fight micro-jitter once the garment has settled.
    fn damp_micro_jitter(&mut self, strength: f32) {
        const SLEEP_THRESH: f32 = 0.00006;
        const DAMP_THRESH: f32 = 0.00035;
        let strength = strength.clamp(0.0, 1.0);
        for i in 0..self.core.pos.len() {
            if self.core.inv_mass[i] <= 0.0 {
                continue;
            }
            let v = self.core.pos[i].sub(self.core.prev[i]);
            let speed = v.len();
            if speed <= SLEEP_THRESH {
                let keep = 1.0 - strength;
                self.core.prev[i] = self.core.pos[i].sub(v.scale(keep));
            } else if speed < DAMP_THRESH {
                let t = ((speed - SLEEP_THRESH) / (DAMP_THRESH - SLEEP_THRESH)).clamp(0.0, 1.0);
                let keep = 1.0 - strength * (1.0 - t * t);
                self.core.prev[i] = self.core.pos[i].sub(v.scale(keep));
            }
        }
    }

    /// Projects each closed 1:1 / multi-way seam weld cluster to its mass-weighted centroid
    /// after collision so seams stay watertight (`0.0 mm` slit) without velocity spikes.
    fn solve_seam_welds(&mut self, snap_lock: bool) {
        if !snap_lock || self.seam_welds.is_empty() {
            return;
        }
        for cluster in &self.seam_welds {
            if cluster.len() < 2 {
                continue;
            }
            let p0 = self.core.pos[cluster[0]];
            let mut max_d2 = 0.0_f32;
            for &v in &cluster[1..] {
                max_d2 = max_d2.max(self.core.pos[v].sub(p0).len_sq());
            }
            // Only snap-weld once panels have converged within 25 mm.
            if max_d2 > 0.025 * 0.025 {
                continue;
            }
            let mut pinned_pos: Option<Vec3> = None;
            let mut w_sum = 0.0_f32;
            let mut c_sum = Vec3::ZERO;
            let mut v_sum = Vec3::ZERO;
            let mut max_off = 0.003_f32;
            for &v in cluster {
                max_off = max_off.max(self.vert_offset[v]);
                if let Some(&pin_p) = self.vbd.user_pins.get(&v) {
                    pinned_pos = Some(pin_p);
                }
                let im = self.core.inv_mass[v];
                let w = if im > 1e-12 { 1.0 / im } else { 1.0 };
                w_sum += w;
                c_sum = c_sum.add(self.core.pos[v].scale(w));
                v_sum = v_sum.add(self.core.pos[v].sub(self.core.prev[v]).scale(w));
            }
            if w_sum <= 1e-12 {
                continue;
            }
            let mut c = pinned_pos.unwrap_or_else(|| c_sum.scale(1.0 / w_sum));
            let v_mean = if pinned_pos.is_some() {
                Vec3::ZERO
            } else {
                v_sum.scale(1.0 / w_sum)
            };
            if let Some((d, n)) = self.avatar.eval_sdf_within(c, max_off + 0.004) {
                if d < max_off {
                    c = c.add(n.scale(max_off - d));
                }
            }
            for &v in cluster {
                if !self.vbd.user_pins.contains_key(&v) {
                    self.core.pos[v] = c;
                    self.core.prev[v] = c.sub(v_mean);
                }
            }
        }
        // Post-weld barycentric lock for any remaining non-1:1 T-junction constraints.
        for sc in &self.core.sew {
            if sc.w <= 0.015 || sc.w >= 0.985 || self.vbd.user_pins.contains_key(&sc.a) {
                continue;
            }
            let pb = self.core.pos[sc.b0]
                .scale(1.0 - sc.w)
                .add(self.core.pos[sc.b1].scale(sc.w));
            if pb.sub(self.core.pos[sc.a]).len_sq() > 0.015 * 0.015 {
                continue;
            }
            let mut c = pb;
            let off = self.vert_offset[sc.a];
            if let Some((d, n)) = self.avatar.eval_sdf_within(c, off + 0.004) {
                if d < off {
                    c = c.add(n.scale(off - d));
                }
            }
            let v0 = self.core.pos[sc.b0].sub(self.core.prev[sc.b0]);
            let v1 = self.core.pos[sc.b1].sub(self.core.prev[sc.b1]);
            let v_seg = v0.scale(1.0 - sc.w).add(v1.scale(sc.w));
            self.core.pos[sc.a] = c;
            self.core.prev[sc.a] = c.sub(v_seg);
        }
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
                self.core.pos[i] = p;
                continue;
            }
            let near = c.dist < off + 0.02;
            if c.dist < 0.0 || moved > 0.004 || (near && moved > 0.0015) {
                let r = off + moved + 0.03;
                match self.avatar.eval_sdf_within(p, r) {
                    Some((d, n)) => {
                        let n_smooth = if c.dist >= 0.0 && c.dist < off + 0.015 && c.n.dot(n) > 0.5 {
                            c.n.scale(0.60).add(n.scale(0.40)).normalized()
                        } else {
                            n
                        };
                        c.q = p.sub(n_smooth.scale(d));
                        c.n = n_smooth;
                        c.dist = d.max(0.0);
                        c.at = p;
                    }
                    None => {
                        c.dist = r;
                        c.at = p;
                        self.core.pos[i] = p;
                        continue;
                    }
                }
            }
            let d = p.sub(c.q).dot(c.n);
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
            solver_mode: if self.options.use_vbd {
                "vbd".into()
            } else {
                "xpbd".into()
            },
            mesh_spacing_mm: self.options.mesh_spacing_mm as f32,
            adaptive_mesh: self.options.mesh_spacing_mm <= 0.0,
            min_edge_mm: self.min_edge_mm,
            max_edge_mm: self.max_edge_mm,
            pin_count: self.vbd.user_pins.len(),
            self_collision_enabled: self.options.enable_self_collision,
            wind_speed_mps: Vec3::new(
                self.options.wind_mps[0],
                self.options.wind_mps[1],
                self.options.wind_mps[2],
            )
            .len(),
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
        // Blend vertex normals across closed seam welds so lighting flows seamlessly without cracks.
        for cluster in &self.seam_welds {
            if cluster.len() < 2 {
                continue;
            }
            let p0 = self.core.pos[cluster[0]];
            if cluster[1..].iter().any(|&v| self.core.pos[v].sub(p0).len_sq() > 0.006 * 0.006) {
                continue;
            }
            let mut sum_n = Vec3::ZERO;
            for &v in cluster {
                sum_n = sum_n.add(norm_acc[v]);
            }
            if sum_n.len_sq() <= 1e-12 {
                continue;
            }
            let avg_dir = sum_n.normalized();
            for &v in cluster {
                let nv = norm_acc[v];
                if nv.len_sq() > 1e-12 && nv.dot(avg_dir) > 0.0 {
                    norm_acc[v] = nv.normalized().scale(0.15).add(avg_dir.scale(0.85));
                }
            }
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

            let half_t = 0.5 * self.vert_render_thick[i];
            let p_out = p.add(nrm.scale(half_t));
            let p_in = p.sub(nrm.scale(half_t));
            self.render_positions[i * 3..i * 3 + 3].copy_from_slice(&[p_out.x, p_out.y, p_out.z]);
            self.render_normals[i * 3..i * 3 + 3].copy_from_slice(&[nrm.x, nrm.y, nrm.z]);
            self.render_strain[i] = self.flat_strain_pct[i];
            self.render_tension[i] = self.flat_tension_kpa[i];
            self.render_clearance[i] = self.flat_clearance_mm[i];
            self.render_seam_mask[i] = self.flat_seam_mask[i];

            let ii = n + i;
            self.render_positions[ii * 3..ii * 3 + 3].copy_from_slice(&[p_in.x, p_in.y, p_in.z]);
            self.render_normals[ii * 3..ii * 3 + 3].copy_from_slice(&[-nrm.x, -nrm.y, -nrm.z]);
            self.render_strain[ii] = self.flat_strain_pct[i];
            self.render_tension[ii] = self.flat_tension_kpa[i];
            self.render_clearance[ii] = self.flat_clearance_mm[i];
            self.render_seam_mask[ii] = self.flat_seam_mask[i];
        }
        for (ei, &(a, b)) in self.open_boundary_edges.iter().enumerate() {
            let rv = |idx: usize| {
                Vec3::new(
                    self.render_positions[idx * 3],
                    self.render_positions[idx * 3 + 1],
                    self.render_positions[idx * 3 + 2],
                )
            };
            let a_out = rv(a);
            let b_out = rv(b);
            let b_in = rv(n + b);
            let a_in = rv(n + a);
            let mut rn = b_out.sub(a_out).cross(a_in.sub(a_out)).normalized();
            if rn.len_sq() < 0.25 {
                rn = Vec3::new(
                    self.flat_normals[a * 3],
                    self.flat_normals[a * 3 + 1],
                    self.flat_normals[a * 3 + 2],
                );
            }
            let base = 2 * n + 4 * ei;
            let pts = [a_out, b_out, b_in, a_in];
            let srcs = [a, b, b, a];
            for k in 0..4 {
                let dst = base + k;
                let pt = pts[k];
                let src = srcs[k];
                self.render_positions[dst * 3..dst * 3 + 3].copy_from_slice(&[pt.x, pt.y, pt.z]);
                self.render_normals[dst * 3..dst * 3 + 3].copy_from_slice(&[rn.x, rn.y, rn.z]);
                self.render_strain[dst] = self.flat_strain_pct[src];
                self.render_tension[dst] = self.flat_tension_kpa[src];
                self.render_clearance[dst] = self.flat_clearance_mm[src];
                self.render_seam_mask[dst] = self.flat_seam_mask[src];
            }
        }
    }
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

fn aligned_interface_spans(
    edges_a: &[EdgeRef],
    edges_b: &[EdgeRef],
    sewn_len: &HashMap<(Uuid, Uuid), f64>,
) -> (Vec<(f64, f64, usize, f64)>, Vec<(f64, f64, usize, f64)>) {
    let build_splits = |edges: &[EdgeRef]| -> (Vec<f64>, Vec<f64>) {
        let lens: Vec<f64> = edges
            .iter()
            .map(|er| *sewn_len.get(&(er.piece_id, er.edge_id)).unwrap_or(&1.0))
            .collect();
        let tot: f64 = lens.iter().sum::<f64>().max(1.0);
        let mut splits = Vec::with_capacity(edges.len() + 1);
        splits.push(0.0);
        let mut cum = 0.0;
        for &l in &lens {
            cum += l;
            splits.push((cum / tot).clamp(0.0, 1.0));
        }
        if let Some(last) = splits.last_mut() {
            *last = 1.0;
        }
        (splits, lens)
    };
    let (mut sa, lens_a) = build_splits(edges_a);
    let (mut sb, lens_b) = build_splits(edges_b);
    if edges_a.len() >= 2 && edges_b.len() >= 2 {
        let mut min_j = 1usize;
        for i in 1..edges_a.len() {
            let mut best: Option<(f64, usize)> = None;
            for j in min_j..edges_b.len() {
                let d = (sa[i] - sb[j]).abs();
                if d <= 0.08 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, j));
                }
            }
            let Some((d, j)) = best else { continue };
            if i + 1 < edges_a.len() && (sa[i + 1] - sb[j]).abs() < d {
                continue;
            }
            let c = 0.5 * (sa[i] + sb[j]);
            if c > sa[i - 1] + 1e-4
                && c < sa[i + 1] - 1e-4
                && c > sb[j - 1] + 1e-4
                && c < sb[j + 1] - 1e-4
            {
                sa[i] = c;
                sb[j] = c;
                min_j = j + 1;
            }
        }
    }
    let spans_a = (0..edges_a.len())
        .map(|i| (sa[i], sa[i + 1], i, lens_a[i]))
        .collect();
    let spans_b = (0..edges_b.len())
        .map(|j| (sb[j], sb[j + 1], j, lens_b[j]))
        .collect();
    (spans_a, spans_b)
}

fn build_interface_chain_with_spans(
    edges: &[EdgeRef],
    spans: &[(f64, f64, usize, f64)],
    side: i8,
    edge_vert_map: &HashMap<(Uuid, i8, Uuid), Vec<BoundVert>>,
) -> Vec<(usize, f64)> {
    let mut chain = Vec::new();
    for &(u0, u1, ei, _) in spans {
        let er = &edges[ei];
        let vs = edge_vert_map
            .get(&(er.piece_id, side, er.edge_id))
            .or_else(|| edge_vert_map.get(&(er.piece_id, 1, er.edge_id)));
        if let Some(vs) = vs {
            let span = (u1 - u0).max(1e-6);
            for bv in vs {
                let local = if er.dir >= 0 {
                    bv.sewn_frac
                } else {
                    1.0 - bv.sewn_frac
                };
                chain.push((bv.global_idx, (u0 + local * span).clamp(0.0, 1.0)));
            }
        }
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
        while seg + 1 < dst.len() && dst[seg].1 < u - 1e-3 {
            seg += 1;
        }
        let mut matched_exact = false;
        let mut k = seg;
        while k < dst.len() && dst[k].1 <= u + 1e-3 {
            if (dst[k].1 - u).abs() <= 1e-3 {
                matched_exact = true;
                if dst[k].0 != a {
                    out.push(BarySew {
                        a,
                        b0: dst[k].0,
                        b1: dst[k].0,
                        w: 0.0,
                    });
                }
            }
            k += 1;
        }
        if matched_exact {
            continue;
        }
        let mut b_idx = seg.min(dst.len() - 2);
        while b_idx + 1 < dst.len() && dst[b_idx + 1].1 <= u {
            b_idx += 1;
        }
        if b_idx >= dst.len() - 1 {
            b_idx = dst.len() - 2;
        }
        while b_idx > 0 && (dst[b_idx + 1].1 - dst[b_idx].1).abs() <= 1e-6 {
            b_idx -= 1;
        }
        let (b0, u0) = dst[b_idx];
        let (b1, u1) = dst[b_idx + 1];
        if a == b0 || a == b1 {
            continue;
        }
        let du = u1 - u0;
        let w = if du <= 1e-6 {
            0.0
        } else {
            ((u - u0) / du).clamp(0.0, 1.0) as f32
        };
        out.push(BarySew { a, b0, b1, w });
    }
}

fn build_conformal_edge_stations(
    pieces: &[PatternPiece],
    stitching: &StitchingGraph,
    sizing_map: &HashMap<Uuid, SizingMap>,
    pre_sewn_len: &HashMap<(Uuid, Uuid), f64>,
    default_spacing_mm: f64,
) -> HashMap<Uuid, Vec<f64>> {
    let piece_by_id: HashMap<Uuid, &PatternPiece> = pieces
        .iter()
        .filter(|p| p.visible)
        .map(|p| (p.id, p))
        .collect();
    let mut edge_stations: HashMap<Uuid, Vec<f64>> = HashMap::new();
    let mut mandatory_stations: HashMap<Uuid, Vec<f64>> = HashMap::new();

    let desired_sub_steps = |er: &EdgeRef, u0_local: f64, u1_local: f64| -> (usize, f64) {
        let len_eff = *pre_sewn_len.get(&(er.piece_id, er.edge_id)).unwrap_or(&20.0);
        let frac_span = (u1_local - u0_local).abs().clamp(1e-6, 1.0);
        let sub_len = (len_eff * frac_span).max(1.0);
        let Some(piece) = piece_by_id.get(&er.piece_id) else {
            return (1, sub_len);
        };
        let Some(edge) = piece.seam_loop.edges.iter().find(|e| e.id == er.edge_id) else {
            return (1, sub_len);
        };
        let ds = if let Some(smap) = sizing_map.get(&er.piece_id) {
            let full_len = edge_total_length(edge).max(1.0);
            let mut min_ds = 30.0_f64;
            for k in 0..=6 {
                let lf = u0_local + (u1_local - u0_local) * (k as f64 / 6.0);
                let s = (lf * full_len).clamp(0.0, full_len);
                let t = invert_edge_arc_length(edge, s);
                let pt = eval_edge_point(edge, t);
                min_ds = min_ds.min(smap.eval_mm(pt));
            }
            min_ds.clamp(4.0, 30.0)
        } else {
            default_spacing_mm.clamp(6.0, 35.0)
        };
        let steps = ((sub_len / ds).ceil() as usize).clamp(1, 64);
        (steps, sub_len)
    };

    let merge_edge_stations = |map: &mut HashMap<Uuid, Vec<f64>>,
                               mand_map: &mut HashMap<Uuid, Vec<f64>>,
                               edge_id: Uuid,
                               len_eff: f64,
                               new_pts: &[f64],
                               new_mandatory: &[f64]| {
        let min_du = (3.5 / len_eff.max(4.0)).clamp(1e-4, 0.25);
        let mand = mand_map.entry(edge_id).or_default();
        mand.push(0.0);
        mand.push(1.0);
        for &m in new_mandatory {
            mand.push(m.clamp(0.0, 1.0));
        }
        mand.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        mand.dedup_by(|a, b| (*a - *b).abs() < 1e-4);

        let entry = map.entry(edge_id).or_default();
        let mut combined: Vec<(f64, bool)> = Vec::with_capacity(entry.len() + new_pts.len() + mand.len());
        for &m in mand.iter() {
            combined.push((m, true));
        }
        for &u in entry.iter().chain(new_pts.iter()) {
            let uc = u.clamp(0.0, 1.0);
            if mand.iter().all(|&m| (uc - m).abs() >= min_du) {
                combined.push((uc, false));
            }
        }
        combined.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.1.cmp(&a.1))
        });
        let mut clean: Vec<f64> = Vec::with_capacity(combined.len());
        for (uc, is_mand) in combined {
            if is_mand {
                if clean.last().is_none_or(|&prev| (uc - prev).abs() >= 1e-4) {
                    clean.push(uc);
                }
            } else if clean.last().is_none_or(|&prev| uc - prev >= min_du) {
                clean.push(uc);
            }
        }
        *entry = clean;
    };

    let mut add_interface_pair = |edges_a: &[EdgeRef], edges_b: &[EdgeRef]| {
        if edges_a.is_empty() || edges_b.is_empty() {
            return;
        }
        if !edges_a
            .iter()
            .chain(edges_b.iter())
            .all(|er| piece_by_id.contains_key(&er.piece_id))
        {
            return;
        }
        let (spans_a, spans_b) = aligned_interface_spans(edges_a, edges_b, pre_sewn_len);
        let mut splits: Vec<f64> = vec![0.0, 1.0];
        for &(u0, u1, _, _) in spans_a.iter().chain(spans_b.iter()) {
            splits.push(u0);
            splits.push(u1);
        }
        splits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        splits.dedup_by(|a, b| (*a - *b).abs() < 1e-4);

        let find_span = |spans: &[(f64, f64, usize, f64)], umid: f64| -> (f64, f64, usize, f64) {
            spans
                .iter()
                .copied()
                .find(|&(u0, u1, _, _)| umid >= u0 - 1e-6 && umid <= u1 + 1e-6)
                .unwrap_or(spans[0])
        };

        let mut iface_stations: Vec<f64> = vec![0.0];
        for w in splits.windows(2) {
            let (u0, u1) = (w[0], w[1]);
            if u1 - u0 < 1e-4 {
                continue;
            }
            let umid = 0.5 * (u0 + u1);
            let (a0, a1, eia, _) = find_span(&spans_a, umid);
            let (b0, b1, eib, _) = find_span(&spans_b, umid);
            let era = &edges_a[eia];
            let erb = &edges_b[eib];
            let la0 = ((u0 - a0) / (a1 - a0).max(1e-6)).clamp(0.0, 1.0);
            let la1 = ((u1 - a0) / (a1 - a0).max(1e-6)).clamp(0.0, 1.0);
            let lb0 = ((u0 - b0) / (b1 - b0).max(1e-6)).clamp(0.0, 1.0);
            let lb1 = ((u1 - b0) / (b1 - b0).max(1e-6)).clamp(0.0, 1.0);
            let (loc_a0, loc_a1) = if era.dir >= 0 { (la0, la1) } else { (1.0 - la1, 1.0 - la0) };
            let (loc_b0, loc_b1) = if erb.dir >= 0 { (lb0, lb1) } else { (1.0 - lb1, 1.0 - lb0) };
            let (na, sub_la) = desired_sub_steps(era, loc_a0, loc_a1);
            let (nb, sub_lb) = desired_sub_steps(erb, loc_b0, loc_b1);
            let max_safe = ((sub_la.min(sub_lb) / 3.8).floor() as usize).clamp(1, 64);
            let n_sub = na.max(nb).min(max_safe).clamp(1, 64);
            for k in 1..=n_sub {
                iface_stations.push(u0 + (u1 - u0) * (k as f64 / n_sub as f64));
            }
        }
        for (u0, u1, er, len_eff) in spans_a
            .into_iter()
            .map(|(u0, u1, ei, l)| (u0, u1, &edges_a[ei], l))
            .chain(
                spans_b
                    .into_iter()
                    .map(|(u0, u1, ei, l)| (u0, u1, &edges_b[ei], l)),
            )
        {
            let span = (u1 - u0).max(1e-6);
            let project_list = |src: &[f64]| -> Vec<f64> {
                let mut out = Vec::new();
                for &u in src {
                    if u >= u0 - 1e-6 && u <= u1 + 1e-6 {
                        let mut loc = ((u - u0) / span).clamp(0.0, 1.0);
                        if er.dir < 0 {
                            loc = 1.0 - loc;
                        }
                        out.push(loc);
                    }
                }
                out
            };
            let local = project_list(&iface_stations);
            let mand = project_list(&splits);
            merge_edge_stations(
                &mut edge_stations,
                &mut mandatory_stations,
                er.edge_id,
                len_eff,
                &local,
                &mand,
            );
        }
    };

    for binding in &stitching.bindings {
        add_interface_pair(&binding.interface_a.edges, &binding.interface_b.edges);
    }
    for p in pieces.iter().filter(|p| p.visible) {
        if matches!(p.arrangement_zone, ArrangementZone::SleeveLeft | ArrangementZone::SleeveRight)
            && p.seam_loop.edges.len() >= 5
        {
            let e_bu = [EdgeRef {
                piece_id: p.id,
                edge_id: p.seam_loop.edges[1].id,
                dir: 1,
            }];
            let e_fu = [EdgeRef {
                piece_id: p.id,
                edge_id: p.seam_loop.edges[4].id,
                dir: -1,
            }];
            add_interface_pair(&e_bu, &e_fu);
        }
    }
    edge_stations
}

fn build_seam_welds(n_verts: usize, sew: &[BarySew]) -> (Vec<Vec<usize>>, Vec<usize>) {
    let mut parent: Vec<usize> = (0..n_verts).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let union = |parent: &mut [usize], a: usize, b: usize| {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            parent[hi] = lo;
        }
    };
    for sc in sew {
        if sc.w <= 0.015 {
            union(&mut parent, sc.a, sc.b0);
        } else if sc.w >= 0.985 {
            union(&mut parent, sc.a, sc.b1);
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for v in 0..n_verts {
        let r = find(&mut parent, v);
        groups.entry(r).or_default().push(v);
    }
    let mut vert_root: Vec<usize> = (0..n_verts).collect();
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for (root, mut vs) in groups {
        if (2..=8).contains(&vs.len()) {
            vs.sort_unstable();
            for &v in &vs {
                vert_root[v] = root;
            }
            clusters.push(vs);
        }
    }
    clusters.sort_by_key(|c| c[0]);
    (clusters, vert_root)
}

fn build_cross_seam_hinges(
    tri_idx: &[[usize; 3]],
    tri_grain: &[Point2],
    tri_slot: &[u16],
    rest2d: &[Point2],
    vert_root: &[usize],
) -> Vec<HingeElem> {
    let mut edge_count: HashMap<(usize, usize), usize> = HashMap::new();
    for &[a, b, c] in tri_idx {
        for (u, v) in [(a, b), (b, c), (c, a)] {
            let key = if u < v { (u, v) } else { (v, u) };
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }
    #[derive(Clone, Copy)]
    struct BoundEdgeTri {
        u: usize,
        v: usize,
        opp: usize,
        slot: u16,
        grain: Point2,
    }
    let mut welded_groups: HashMap<(usize, usize), Vec<BoundEdgeTri>> = HashMap::new();
    for (ti, &[a, b, c]) in tri_idx.iter().enumerate() {
        let slot = tri_slot[ti];
        let grain = tri_grain[ti];
        for (u, v, opp) in [(a, b, c), (b, c, a), (c, a, b)] {
            let key = if u < v { (u, v) } else { (v, u) };
            if edge_count.get(&key).copied().unwrap_or(0) != 1 {
                continue;
            }
            let ru = vert_root[u];
            let rv = vert_root[v];
            if ru == rv {
                continue;
            }
            let wkey = if ru < rv { (ru, rv) } else { (rv, ru) };
            welded_groups.entry(wkey).or_default().push(BoundEdgeTri {
                u,
                v,
                opp,
                slot,
                grain,
            });
        }
    }
    let mut out = Vec::new();
    let mut keys: Vec<(usize, usize)> = welded_groups.keys().copied().collect();
    keys.sort_unstable();
    for wkey in keys {
        let group = &welded_groups[&wkey];
        if group.len() != 2 {
            continue;
        }
        let ea = group[0];
        let mut eb = group[1];
        if vert_root[ea.u] != vert_root[eb.u] {
            std::mem::swap(&mut eb.u, &mut eb.v);
        }
        if vert_root[ea.u] != vert_root[eb.u] || vert_root[ea.v] != vert_root[eb.v] {
            continue;
        }
        if ea.opp == eb.opp || vert_root[ea.opp] == vert_root[eb.opp] {
            continue;
        }
        let da = rest2d[ea.v].sub(rest2d[ea.u]);
        let db = rest2d[eb.v].sub(rest2d[eb.u]);
        let la = da.len();
        let lb = db.len();
        if la < 1e-4 || lb < 1e-4 || (la - lb).abs() / la.max(lb) > 0.35 {
            continue;
        }
        let dir_a = da.scale(1.0 / la);
        let dir_b = db.scale(1.0 / lb);
        let va = rest2d[ea.opp].sub(rest2d[ea.u]);
        let vb = rest2d[eb.opp].sub(rest2d[eb.u]);
        let ua = va.dot(dir_a);
        let ha = va.perp_dot(dir_a).abs();
        let ub = vb.dot(dir_b);
        let hb = vb.perp_dot(dir_b).abs();
        if ha < 1e-4 || hb < 1e-4 {
            continue;
        }
        let l = 0.5 * (la + lb);
        let p0 = Point2::new(0.0, 0.0);
        let p1 = Point2::new(l, 0.0);
        let p2 = Point2::new(ua * (l / la), ha);
        let p3 = Point2::new(ub * (l / lb), -hb);
        let ca = dir_a.dot(ea.grain.normalized());
        let cb = dir_b.dot(eb.grain.normalized());
        let warp_w = (1.0 - 0.5 * (ca * ca + cb * cb)).clamp(0.0, 1.0) as f32;
        if let Some(mut h1) = make_hinge(
            [ea.u, ea.v, ea.opp, eb.opp],
            [p0, p1, p2, p3],
            Point2::new(1.0, 0.0),
            ea.slot,
        ) {
            h1.warp_w = warp_w;
            h1.area2 *= 2.0;
            out.push(h1);
        }
        if let Some(mut h2) = make_hinge(
            [eb.u, eb.v, eb.opp, ea.opp],
            [p0, p1, p3, p2],
            Point2::new(1.0, 0.0),
            eb.slot,
        ) {
            h2.warp_w = warp_w;
            h2.area2 *= 2.0;
            out.push(h2);
        }
    }
    out
}
