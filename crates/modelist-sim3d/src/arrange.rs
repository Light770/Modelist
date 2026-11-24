//! Initial 3D placement ("arrangement") of 2D panels around the avatar.
//!
//! Panels are placed *isometrically*: torso and skirt panels are wrapped around an offset of the
//! body's convex cross-section by arc length, and hang 1:1 in height from the shoulder (tops) or
//! the waist (skirts). No panel starts compressed, so nothing has to un-crumple while it drapes.
//! Trims (neckbands, waistbands, cuffs) are laid directly onto the seam they are sewn to.

use crate::avatar::{ParametricAvatar, Vec3};
use crate::mesher::PanelMesh;
use modelist_core::model::{ArrangementZone, PatternPiece};
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Minimum / maximum initial distance between a wrapped panel and the body, m.
const D_MIN: f32 = 0.014;
const D_MAX: f32 = 0.16;
/// Height bin used to evaluate the wrap per row, m.
const ROW_BIN: f32 = 0.005;
/// Half-height of the window used to measure a row's width, m (spans boundary sample gaps).
const ROW_WINDOW: f32 = 0.012;
/// Max change of the wrap offset per metre of height (keeps the initial shape near-isometric).
const D_SLOPE: f32 = 0.30;
const WRAP_SAMPLES: usize = 144;

pub(crate) struct ArrangeCtx {
    /// Highest 2D y over all torso pieces (high point shoulder), mm.
    pub torso_top_mm: f64,
    /// Highest 2D y over all skirt pieces (waist), mm.
    pub skirt_top_mm: f64,
}

/// Places one panel. Returns one 3D point per mesh vertex.
pub(crate) fn place_panel(
    avatar: &ParametricAvatar,
    pm: &PanelMesh,
    piece: &PatternPiece,
    zone: ArrangementZone,
    side: f32,
    spawn_pair: bool,
    ctx: &ArrangeCtx,
) -> Vec<Vec3> {
    match zone {
        ArrangementZone::FrontTorso
        | ArrangementZone::BackTorso
        | ArrangementZone::FrontSkirt
        | ArrangementZone::BackSkirt => wrap_panel(avatar, pm, piece, zone, side, spawn_pair, ctx),
        ArrangementZone::SleeveLeft | ArrangementZone::SleeveRight => place_sleeve(avatar, pm, zone),
        ArrangementZone::FrontLeg | ArrangementZone::BackLeg => place_leg(avatar, pm, zone, side),
        ArrangementZone::FlatCustom => place_flat(avatar, pm),
    }
}

// ---------------------------------------------------------------------------------------------
// Torso & skirt wrap
// ---------------------------------------------------------------------------------------------

/// Offset path around the convex body section, starting at a given angle, for one full turn.
struct WrapPath {
    pts: Vec<(f32, f32)>,
    cum: Vec<f32>,
}

impl WrapPath {
    fn lookup(&self, s: f32) -> (f32, f32) {
        let s = s.max(0.0);
        let last = self.cum.len() - 1;
        if s >= self.cum[last] {
            return self.pts[last];
        }
        let j = match self
            .cum
            .binary_search_by(|c| c.partial_cmp(&s).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(j) => return self.pts[j],
            Err(j) => j.max(1),
        };
        let (c0, c1) = (self.cum[j - 1], self.cum[j]);
        let t = if c1 - c0 > 1e-9 { (s - c0) / (c1 - c0) } else { 0.0 };
        let (a, b) = (self.pts[j - 1], self.pts[j]);
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }

    /// Arc length from the start to the point a quarter turn away (centre line -> side seam).
    fn quarter(&self) -> f32 {
        self.cum[WRAP_SAMPLES / 4]
    }
}

fn is_center_label(label: &str) -> bool {
    let l = label.to_lowercase();
    l.contains("center") || l.contains("centre") || l.starts_with("cf") || l.starts_with("cb")
}

/// 2D x (mm) of the panel's centre line (CF/CB): fold line, labelled centre edge, or convention.
fn panel_center_x(pm: &PanelMesh, piece: &PatternPiece, side: f32, spawn_pair: bool) -> f64 {
    if let Some((a, _)) = pm.fold_line {
        return a.x;
    }
    let centre_edges: Vec<_> = piece
        .seam_loop
        .edges
        .iter()
        .filter(|e| is_center_label(&e.label))
        .map(|e| e.id)
        .collect();
    if !centre_edges.is_empty() {
        let mut sum = 0.0;
        let mut cnt = 0usize;
        for br in &pm.boundary_refs {
            if centre_edges.contains(&br.edge_id) {
                sum += pm.raw_2d_mm[br.vertex_idx].x;
                cnt += 1;
            }
        }
        if cnt > 0 {
            return sum / cnt as f64;
        }
    }
    let (min_x, max_x) = pm
        .raw_2d_mm
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
    if spawn_pair {
        if side > 0.0 {
            min_x
        } else {
            max_x
        }
    } else {
        0.5 * (min_x + max_x)
    }
}

fn wrap_panel(
    avatar: &ParametricAvatar,
    pm: &PanelMesh,
    piece: &PatternPiece,
    zone: ArrangementZone,
    side: f32,
    spawn_pair: bool,
    ctx: &ArrangeCtx,
) -> Vec<Vec3> {
    let is_front = matches!(zone, ArrangementZone::FrontTorso | ArrangementZone::FrontSkirt);
    let is_skirt = matches!(zone, ArrangementZone::FrontSkirt | ArrangementZone::BackSkirt);
    let center_x = panel_center_x(pm, piece, side, spawn_pair);
    let (top_mm, anchor_y) = if is_skirt {
        (ctx.skirt_top_mm, avatar.waist_y + 0.012)
    } else {
        (ctx.torso_top_mm, avatar.shoulder_y + 0.015)
    };
    let (yc_lo, yc_hi) = if is_skirt {
        (avatar.hip_y, avatar.waist_y)
    } else {
        (avatar.crotch_y + 0.03, avatar.shoulder_y - 0.012)
    };
    // Centre angle and the angular direction that leads towards the wearer's left (+X).
    let start_ang = if is_front { FRAC_PI_2 } else { -FRAC_PI_2 };
    let dir_plus_x = if is_front { -1.0 } else { 1.0 };
    // Pieces are drafted in either orientation, so derive the mapping from the seam sides:
    // the half carrying side +1 seam memberships always goes to the wearer's left (+X), the
    // side -1 half to the right. Sewing therefore never pulls a seam through the body.
    let primary: i8 = if side < 0.0 { -1 } else { 1 };
    let mut lean = 0.0_f64;
    for br in &pm.boundary_refs {
        if br.side_sign == primary {
            lean += pm.raw_2d_mm[br.vertex_idx].x - center_x;
        }
    }
    let half_sign: f32 = if lean < 0.0 { -1.0 } else { 1.0 };

    let n = pm.raw_2d_mm.len();
    let ys: Vec<f32> = pm
        .raw_2d_mm
        .iter()
        .map(|p| anchor_y - ((top_mm - p.y) * 0.001) as f32)
        .collect();
    let ss: Vec<f32> = pm
        .raw_2d_mm
        .iter()
        .map(|p| ((p.x - center_x) * 0.001) as f32)
        .collect();
    if n == 0 {
        return Vec::new();
    }
    let y_min = ys.iter().cloned().fold(f32::INFINITY, f32::min);
    let y_max = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let n_bins = (((y_max - y_min) / ROW_BIN).ceil() as usize + 1).max(1);

    // Smooth per-row body ellipses (contain the true section), one per height bin.
    let ellipses = row_ellipses(avatar, y_min, n_bins, yc_lo, yc_hi);
    let q0: Vec<f32> = ellipses
        .iter()
        .map(|e| e.arc_to_side(is_front, 0.0))
        .collect();

    // Row half-widths (centre line -> furthest edge), windowed so sparse boundary samples count.
    let mut half_w = vec![0.0_f32; n_bins];
    for i in 0..n {
        let lo = ((ys[i] - ROW_WINDOW - y_min) / ROW_BIN).floor().max(0.0) as usize;
        let hi = (((ys[i] + ROW_WINDOW - y_min) / ROW_BIN).ceil() as usize).min(n_bins - 1);
        for w in &mut half_w[lo..=hi] {
            *w = w.max(ss[i].abs());
        }
    }
    let mut offsets: Vec<f32> = (0..n_bins)
        .map(|k| ((half_w[k] - q0[k]) / FRAC_PI_2).clamp(D_MIN, D_MAX))
        .collect();
    // Limit how fast the offset changes with height so the start shape stays near-isometric.
    let max_step = D_SLOPE * ROW_BIN;
    for k in 1..n_bins {
        offsets[k] = offsets[k].max(offsets[k - 1] - max_step);
    }
    for k in (0..n_bins.saturating_sub(1)).rev() {
        offsets[k] = offsets[k].max(offsets[k + 1] - max_step);
    }

    let mut paths: HashMap<(usize, i8), WrapPath> = HashMap::new();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let f = ((ys[i] - y_min) / ROW_BIN).clamp(0.0, (n_bins - 1) as f32);
        let k0 = f.floor() as usize;
        let k1 = (k0 + 1).min(n_bins - 1);
        let t = f - k0 as f32;
        let on_primary_half = ss[i] * half_sign >= 0.0;
        let toward_plus_x = on_primary_half == (primary > 0);
        let sgn: i8 = if toward_plus_x { 1 } else { -1 };
        let mut pt = |k: usize| -> (f32, f32) {
            paths
                .entry((k, sgn))
                .or_insert_with(|| ellipses[k].path(is_front, toward_plus_x, offsets[k]))
                .lookup(ss[i].abs())
        };
        let a = pt(k0);
        let b = pt(k1);
        out.push(Vec3::new(a.0 + (b.0 - a.0) * t, ys[i], a.1 + (b.1 - a.1) * t));
    }
    let _ = (start_ang, dir_plus_x);
    out
}

/// Body cross-section model for one height: an ellipse with separate front/back depth, scaled so
/// it contains the real section.
#[derive(Debug, Clone, Copy)]
struct RowEllipse {
    cx: f32,
    cz: f32,
    rx: f32,
    rz_front: f32,
    rz_back: f32,
}

impl RowEllipse {
    fn point(&self, theta: f32, d: f32) -> (f32, f32) {
        let rz = if theta.sin() >= 0.0 { self.rz_front } else { self.rz_back };
        (
            self.cx + (self.rx + d) * theta.cos(),
            self.cz + (rz + d) * theta.sin(),
        )
    }

    /// Wrap path from the centre front (or back) towards +X or -X, for a full turn.
    fn path(&self, front: bool, toward_plus_x: bool, d: f32) -> WrapPath {
        let start = if front { FRAC_PI_2 } else { -FRAC_PI_2 };
        // +X is theta = 0: from the front (pi/2) that means decreasing theta, from the back increasing.
        let dir = match (front, toward_plus_x) {
            (true, true) | (false, false) => -1.0,
            _ => 1.0,
        };
        let m = WRAP_SAMPLES;
        let pts: Vec<(f32, f32)> = (0..=m)
            .map(|j| self.point(start + dir * TAU * (j as f32) / (m as f32), d))
            .collect();
        let mut cum = Vec::with_capacity(pts.len());
        let mut acc = 0.0;
        cum.push(0.0);
        for j in 1..pts.len() {
            let (a, b) = (pts[j - 1], pts[j]);
            acc += ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            cum.push(acc);
        }
        WrapPath { pts, cum }
    }

    /// Arc length from the centre line to the side seam position (quarter turn).
    fn arc_to_side(&self, front: bool, d: f32) -> f32 {
        self.path(front, true, d).quarter()
    }
}

/// Per-bin ellipses from `y0` upwards, smoothed over height so neighbouring rows never jump.
fn row_ellipses(avatar: &ParametricAvatar, y0: f32, n_bins: usize, yc_lo: f32, yc_hi: f32) -> Vec<RowEllipse> {
    let raw: Vec<RowEllipse> = (0..n_bins)
        .map(|k| fit_section(avatar, (y0 + k as f32 * ROW_BIN).clamp(yc_lo, yc_hi)))
        .collect();
    // Moving average over +-15 mm.
    let half = (0.015 / ROW_BIN).round() as isize;
    (0..n_bins)
        .map(|k| {
            let mut acc = RowEllipse { cx: 0.0, cz: 0.0, rx: 0.0, rz_front: 0.0, rz_back: 0.0 };
            let mut cnt = 0.0;
            for o in -half..=half {
                let j = (k as isize + o).clamp(0, n_bins as isize - 1) as usize;
                let e = raw[j];
                acc.cx += e.cx;
                acc.cz += e.cz;
                acc.rx += e.rx;
                acc.rz_front += e.rz_front;
                acc.rz_back += e.rz_back;
                cnt += 1.0;
            }
            RowEllipse {
                cx: acc.cx / cnt,
                cz: acc.cz / cnt,
                rx: acc.rx / cnt,
                rz_front: acc.rz_front / cnt,
                rz_back: acc.rz_back / cnt,
            }
        })
        .collect()
}

/// Fits the two-depth ellipse to the torso section at height `y` and inflates it to contain it.
fn fit_section(avatar: &ParametricAvatar, y: f32) -> RowEllipse {
    let n = 72;
    let pts: Vec<(f32, f32)> = (0..n)
        .map(|k| avatar.torso_surface_xz(y, TAU * k as f32 / n as f32))
        .collect();
    let (mut x0, mut x1, mut z0, mut z1) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    for p in &pts {
        x0 = x0.min(p.0);
        x1 = x1.max(p.0);
        z0 = z0.min(p.1);
        z1 = z1.max(p.1);
    }
    let cx = 0.5 * (x0 + x1);
    let cz = 0.5 * (z0 + z1);
    let mut e = RowEllipse {
        cx,
        cz,
        rx: (0.5 * (x1 - x0)).max(0.02),
        rz_front: (z1 - cz).max(0.02),
        rz_back: (cz - z0).max(0.02),
    };
    // Inflate so every section point lies inside the ellipse (squarish bodies bulge diagonally).
    let mut k = 1.0_f32;
    for p in &pts {
        let dx = (p.0 - cx) / e.rx;
        let rz = if p.1 >= cz { e.rz_front } else { e.rz_back };
        let dz = (p.1 - cz) / rz;
        k = k.max((dx * dx + dz * dz).sqrt());
    }
    e.rx *= k;
    e.rz_front *= k;
    e.rz_back *= k;
    e
}

// ---------------------------------------------------------------------------------------------
// Sleeves
// ---------------------------------------------------------------------------------------------

/// Local `[min_u, max_u]` of the vertices within +-22 mm of height `v`.
fn row_x_bounds(rest_uv: &[(f32, f32)], v: f32, fb_min: f32, fb_max: f32) -> (f32, f32) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for &(u2, v2) in rest_uv {
        if (v2 - v).abs() <= 0.022 {
            lo = lo.min(u2);
            hi = hi.max(u2);
        }
    }
    if lo.is_finite() && hi - lo >= 0.04 {
        (lo, hi)
    } else {
        (fb_min, fb_max)
    }
}

/// Height of the widest horizontal band (the sleeve's bicep line).
fn widest_band_v(rest_uv: &[(f32, f32)], fallback: f32) -> f32 {
    let mut best_v = fallback;
    let mut best_w = -1.0_f32;
    let mut vs: Vec<f32> = rest_uv.iter().map(|p| p.1).collect();
    vs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    vs.dedup_by(|a, b| (*a - *b).abs() < 0.004);
    for v in vs {
        let (lo, hi) = row_x_bounds(rest_uv, v, 0.0, 0.0);
        if hi - lo > best_w {
            best_w = hi - lo;
            best_v = v;
        }
    }
    best_v
}

fn uv_bounds(rest_uv: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    rest_uv.iter().fold(
        (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY),
        |(x0, x1, y0, y1), &(u, v)| (x0.min(u), x1.max(u), y0.min(v), y1.max(v)),
    )
}

fn place_sleeve(avatar: &ParametricAvatar, pm: &PanelMesh, zone: ArrangementZone) -> Vec<Vec3> {
    let s = if zone == ArrangementZone::SleeveRight { -1.0 } else { 1.0 };
    let (x_min, x_max, y_min, y_max) = uv_bounds(&pm.rest_uv);
    let span_x = (x_max - x_min).max(0.05);
    let v_widest = widest_band_v(&pm.rest_uv, 0.5 * (y_min + y_max));
    let (a0, a1) = avatar.arm_segment(s);
    let arm_len = a1.sub(a0).len().max(0.25);
    let axis = a1.sub(a0).normalized();
    let up = Vec3::new(0.0, 1.0, 0.0);
    let out_top = up.sub(axis.scale(up.dot(axis))).normalized();
    let mut fwd = out_top.cross(axis).normalized();
    if fwd.z < 0.0 {
        fwd = fwd.scale(-1.0);
    }
    let fwd_sign = if s > 0.0 { -1.0 } else { 1.0 };
    let cap_origin = a0.add(Vec3::new(0.0, 0.012, 0.0));

    pm.rest_uv
        .iter()
        .map(|&(u, v)| {
            // 1:1 along the arm from the cap crest; long sleeves simply extend past the wrist.
            let d_cap = (y_max - v).clamp(0.0, arm_len + 0.12);
            let arm_t = (d_cap / arm_len).clamp(0.0, 1.0);
            let center = cap_origin.add(axis.scale(d_cap));
            let (xn, row_w) = if v <= v_widest {
                let (lo, hi) = row_x_bounds(&pm.rest_uv, v, x_min, x_max);
                let rw = (hi - lo).max(0.04);
                (((u - lo) / rw).clamp(0.0, 1.0), rw)
            } else {
                ((u - x_min) / span_x, span_x)
            };
            // Isometric tube: the row wraps 92% of a circle whose arc equals the row width.
            let r = (avatar.arm_radius_at(arm_t) + 0.012).max(row_w / (TAU * 0.92));
            let ang = (xn - 0.5) * (row_w / r);
            let radial = out_top.scale(ang.cos()).add(fwd.scale(fwd_sign * ang.sin()));
            center.add(radial.scale(r))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Trouser legs
// ---------------------------------------------------------------------------------------------

fn place_leg(avatar: &ParametricAvatar, pm: &PanelMesh, zone: ArrangementZone, side: f32) -> Vec<Vec3> {
    let (x_min, x_max, y_min, y_max) = uv_bounds(&pm.rest_uv);
    let span_y = (y_max - y_min).max(0.05);
    let is_front = zone == ArrangementZone::FrontLeg;
    let (l0, l1) = avatar.leg_segment(side);
    let crotch_top = avatar.crotch_y - 0.014;
    let split_yn = 0.72_f32;
    let z_sign = if is_front { 1.0 } else { -1.0 };
    let blend_low = avatar.crotch_y - 0.050;

    pm.rest_uv
        .iter()
        .map(|&(u, v)| {
            let yn = (v - y_min) / span_y;
            let v_query = v.min(y_max - 0.032);
            let (row_min, row_max) = row_x_bounds(&pm.rest_uv, v_query, x_min, x_max);
            let y = if yn <= split_yn {
                avatar.ankle_y + (yn / split_yn) * (crotch_top - avatar.ankle_y)
            } else {
                crotch_top
                    + ((yn - split_yn) / (1.0 - split_yn)) * (avatar.waist_y - 0.008 - crotch_top)
            };
            let xn_row = ((u - row_min) / (row_max - row_min).max(0.04)).clamp(0.0, 1.0);
            let u_oi = if side > 0.0 { xn_row } else { 1.0 - xn_row };
            let pelvis_blend =
                ((y - blend_low) / (avatar.waist_y - blend_low).max(0.1)).clamp(0.0, 1.0);
            let leg_t = ((crotch_top - y) / (crotch_top - avatar.ankle_y).max(0.1)).clamp(0.0, 1.0);
            let cx_leg = l0.x + (l1.x - l0.x) * leg_t;
            let r_leg = (avatar.leg_radius_at(leg_t) + 0.018).max(0.052);
            let phi_leg = (u_oi - 0.5) * 2.75;
            let mut leg_x = cx_leg - side * r_leg * phi_leg.sin();
            if leg_x * side < 0.010 {
                leg_x = side * 0.010;
            }
            let fork_push = if leg_t < 0.16 && u_oi > 0.60 {
                ((0.16 - leg_t) / 0.16) * ((u_oi - 0.60) / 0.40) * 0.040
            } else {
                0.0
            };
            let leg_z = z_sign * (r_leg * phi_leg.cos() + fork_push);
            let yc = y.clamp(avatar.hip_y - 0.015, avatar.waist_y);
            let theta_p = (1.0 - u_oi) * 1.48;
            let theta = if is_front { side * theta_p } else { PI - side * theta_p };
            let ang = FRAC_PI_2 - theta;
            let (sx, sz) = avatar.torso_surface_xz(yc, ang);
            let pelv_x = sx + side * 0.022 * theta_p.sin();
            let pelv_z = sz + z_sign * 0.022 * theta_p.cos();
            let sb = pelvis_blend * pelvis_blend * (3.0 - 2.0 * pelvis_blend);
            Vec3::new(leg_x + (pelv_x - leg_x) * sb, y, leg_z + (pelv_z - leg_z) * sb)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Trims
// ---------------------------------------------------------------------------------------------

/// Fallback for trims with no sewn partner: a flat panel hanging in front of the body.
fn place_flat(avatar: &ParametricAvatar, pm: &PanelMesh) -> Vec<Vec3> {
    let (_, _, y_min, _) = uv_bounds(&pm.rest_uv);
    pm.rest_uv
        .iter()
        .map(|&(u, v)| Vec3::new(u, avatar.waist_y + 0.04 + (v - y_min), avatar.bust_rz + 0.085))
        .collect()
}

/// Seam anchor for trim placement: a trim vertex, where it must land, and the direction that
/// leads away from the garment it is sewn to.
pub(crate) struct TrimAnchor {
    pub local_idx: usize,
    pub target: Vec3,
    pub away: Vec3,
}

/// Lays a trim onto its partner seam: each vertex lands at the seam point nearest to it in 2D,
/// offset by its 2D distance from the seam along the "away from the garment" direction.
pub(crate) fn place_trim(pm: &PanelMesh, anchors: &[TrimAnchor]) -> Option<Vec<Vec3>> {
    if anchors.len() < 2 {
        return None;
    }
    let out = pm
        .raw_2d_mm
        .iter()
        .map(|p| {
            // Two nearest anchors (2D) give a seam point and the vertex's distance from it.
            let mut best = [(f64::INFINITY, 0usize); 2];
            for (k, a) in anchors.iter().enumerate() {
                let d = pm.raw_2d_mm[a.local_idx].dist(*p);
                if d < best[0].0 {
                    best[1] = best[0];
                    best[0] = (d, k);
                } else if d < best[1].0 {
                    best[1] = (d, k);
                }
            }
            let a0 = &anchors[best[0].1];
            let a1 = &anchors[best[1].1];
            let q0 = pm.raw_2d_mm[a0.local_idx];
            let q1 = pm.raw_2d_mm[a1.local_idx];
            let seg = q1.sub(q0);
            let l2 = seg.len_sq();
            let t = if l2 > 1e-9 {
                (p.sub(q0).dot(seg) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let foot = q0.add(seg.scale(t));
            let h = (p.dist(foot) * 0.001) as f32;
            let tf = t as f32;
            let target = a0.target.add(a1.target.sub(a0.target).scale(tf));
            let away = a0.away.add(a1.away.sub(a0.away).scale(tf)).normalized();
            target.add(away.scale(h))
        })
        .collect();
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// Dart pre-closing
// ---------------------------------------------------------------------------------------------

/// Returns a copy of the panel, for placement only, with every dart wedge closed in 2D.
///
/// Fabric on each side of a dart slides towards the dart's centre line, perpendicular to it, by
/// the wedge's half-width at that distance from the apex. The wedge closes exactly, positions
/// along the dart (heights, for a waist dart) are preserved, and the small shear that remains is
/// relaxed by the solver into the dart's 3D cone instead of hard sewing ripping the wedge shut.
pub(crate) fn close_darts_for_placement(pm: &PanelMesh) -> PanelMesh {
    use modelist_core::model::Point2;
    use std::collections::BTreeMap;
    let mut out = pm.clone();
    // (dart, side) -> (apex, leg A edge point, leg B edge point) vertex indices
    let mut darts: BTreeMap<(uuid::Uuid, i8), [Option<usize>; 3]> = BTreeMap::new();
    for d in &pm.dart_refs {
        let e = darts.entry((d.dart_id, d.side_sign)).or_insert([None; 3]);
        if d.frac >= 0.999 {
            e[0] = Some(d.vertex_idx);
        } else if d.frac <= 1e-6 {
            if d.leg == 0 {
                e[1] = Some(d.vertex_idx);
            } else {
                e[2] = Some(d.vertex_idx);
            }
        }
    }
    let mut pts: Vec<Point2> = out.raw_2d_mm.clone();
    for slots in darts.values() {
        let (Some(ia), Some(il), Some(ir)) = (slots[0], slots[1], slots[2]) else {
            continue;
        };
        let apex = pts[ia];
        let a = pts[il].sub(apex);
        let b = pts[ir].sub(apex);
        if a.len() < 1.0 || b.len() < 1.0 {
            continue;
        }
        let axis = a.normalized().add(b.normalized());
        if axis.len() < 1e-6 {
            continue;
        }
        let axis = axis.normalized();
        let perp = Point2::new(-axis.y, axis.x);
        // Half-widths of each leg per unit distance along the axis.
        let (pa, pb) = (a.dot(perp), b.dot(perp));
        let (la, lb) = (a.dot(axis), b.dot(axis));
        if la <= 1.0 || lb <= 1.0 || pa * pb >= 0.0 {
            continue; // not a proper wedge around its centre line
        }
        let slope_a = pa / la; // signed
        let slope_b = pb / lb;
        for p in pts.iter_mut() {
            let v = p.sub(apex);
            let t = v.dot(axis);
            if t <= 0.0 {
                continue;
            }
            let u = v.dot(perp);
            let shift = if u * slope_a > 0.0 {
                -slope_a * t
            } else if u * slope_b > 0.0 {
                -slope_b * t
            } else {
                0.0
            };
            *p = p.add(perp.scale(shift));
        }
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for p in &pm.raw_2d_mm {
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        y0 = y0.min(p.y);
        y1 = y1.max(p.y);
    }
    let (cx, cy) = (0.5 * (x0 + x1), 0.5 * (y0 + y1));
    out.rest_uv = pts
        .iter()
        .map(|p| (((p.x - cx) * 0.001) as f32, ((p.y - cy) * 0.001) as f32))
        .collect();
    out.raw_2d_mm = pts;
    out
}
