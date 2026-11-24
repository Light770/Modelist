//! Relaxed trousers: pleated wide-leg and barrel-leg variants from one parametric draft.
//!
//! Pieces: front leg (cut 2) with front pleats, back leg (cut 2) with a waist dart, and a
//! half waistband (cut 2, folded) with a 35 mm underlap extension.
//!
//! Construction notes
//! - The waistband is the seam driver. The trouser waist (front + back) is drafted to the
//!   waistband length plus the pleat and dart intake, and bound with that intake as ease, so
//!   the pleats are explicit and editable instead of hidden in the outline.
//! - `wt_knee_bulge` bows the outseam (fully) and the inseam (45 %) outward at the knee.
//!   0 gives a straight wide leg; 30-60 mm with a narrower opening gives a barrel leg.
//! - Edge labels keep the 3D conventions (`crotch`, `seat`, `waist`) so the legs drape with
//!   the existing leg arrangement.

use super::{binding, cubic, eref, line, notch, param, sclamp, t_at_y, var, MITER};
use crate::model::{
    det_id, ArrangementZone, BeltLoopSpec, BodyMeasurements, ButtonSpec, ClosedPolyCurve,
    CornerRule, DartCapStyle, DartEntity, DartFoldDir, EaseSpec, EaseTable, GrainRotationMode,
    GrainlineSpec, InternalLine, NotchStyle, ParametricVar, PatternPiece, Point2, SeamType,
    StitchingGraph, ZipperSpec,
};
use std::collections::HashMap;

pub const WIDE_ID: &str = "wide_pleat_trouser";
pub const BARREL_ID: &str = "barrel_trouser";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegShape {
    Wide,
    Barrel,
}

/// Style defaults per leg shape (single source for the variables and the build fallbacks).
#[derive(Debug, Clone, Copy)]
pub struct ShapeDefaults {
    pub hip_ease: f64,
    pub leg_opening: f64,
    pub knee_bulge: f64,
    pub pleat_count: f64,
    pub pleat_depth: f64,
}

pub fn shape_defaults(shape: LegShape) -> ShapeDefaults {
    match shape {
        LegShape::Wide => ShapeDefaults {
            hip_ease: 100.0,
            leg_opening: 560.0,
            knee_bulge: 0.0,
            pleat_count: 2.0,
            pleat_depth: 16.0,
        },
        LegShape::Barrel => ShapeDefaults {
            hip_ease: 110.0,
            leg_opening: 400.0,
            knee_bulge: 45.0,
            pleat_count: 1.0,
            pleat_depth: 14.0,
        },
    }
}

pub fn default_variables(shape: LegShape) -> Vec<ParametricVar> {
    let d = shape_defaults(shape);
    vec![
        var("wt_waist_ease", "20", "Waist ease (mm)"),
        var("wt_hip_ease", &format!("{}", d.hip_ease), "Hip ease (mm)"),
        var("wt_rise_add", "15", "Waist raised above the natural waist (mm)"),
        var("wt_leg_len_add", "20", "Leg length past the inseam measurement (mm)"),
        var("wt_pleat_count", &format!("{}", d.pleat_count), "Front pleats per side (0 to 2)"),
        var(
            "wt_pleat_depth",
            &format!("{}", d.pleat_depth),
            "Pleat depth; each pleat takes twice this (mm)",
        ),
        var("wt_leg_opening", &format!("{}", d.leg_opening), "Hem circumference (mm)"),
        var(
            "wt_knee_bulge",
            &format!("{}", d.knee_bulge),
            "Outward curve of the leg at the knee (mm)",
        ),
        var("wt_front_fork", "(hip + wt_hip_ease) / 16", "Front crotch extension (mm)"),
        var("wt_back_fork", "(hip + wt_hip_ease) / 8 - 18", "Back crotch extension (mm)"),
        var("wt_waistband_h", "40", "Finished waistband height (mm)"),
    ]
}

pub fn build(
    template_id: &'static str,
    shape: LegShape,
    m: &BodyMeasurements,
    _ease: &EaseTable,
    vals: &HashMap<String, f64>,
) -> (Vec<PatternPiece>, StitchingGraph) {
    let id = |s: &str| det_id(&format!("{template_id}:{s}"));
    let code = match shape {
        LegShape::Wide => "WL",
        LegShape::Barrel => "BL",
    };
    let d = shape_defaults(shape);

    // --- Style parameters (DAG-evaluated; clamped so any input still drafts a valid pattern) ---
    let waist_ease = sclamp(param(vals, "wt_waist_ease", 20.0), -30.0, 300.0);
    let hip_ease = sclamp(param(vals, "wt_hip_ease", d.hip_ease), 0.0, 500.0);
    let rise_add = sclamp(param(vals, "wt_rise_add", 15.0), -80.0, 150.0);
    let len_add = sclamp(param(vals, "wt_leg_len_add", 20.0), -400.0, 250.0);
    let pleat_count = sclamp(param(vals, "wt_pleat_count", d.pleat_count).round(), 0.0, 2.0) as usize;
    let pleat_depth = sclamp(param(vals, "wt_pleat_depth", d.pleat_depth), 0.0, 45.0);
    let opening = sclamp(param(vals, "wt_leg_opening", d.leg_opening), 260.0, 1400.0);
    let bulge = sclamp(param(vals, "wt_knee_bulge", d.knee_bulge), 0.0, 140.0);
    let hip = m.hip_mm.max(500.0);
    let f_fork = sclamp(param(vals, "wt_front_fork", (hip + hip_ease) / 16.0), 30.0, 150.0);
    let b_fork = sclamp(
        param(vals, "wt_back_fork", (hip + hip_ease) / 8.0 - 18.0),
        f_fork + 10.0,
        240.0,
    );
    let wb_h = sclamp(param(vals, "wt_waistband_h", 40.0), 20.0, 100.0);

    // --- Levels (y up, hem at 0) ---
    let q = (hip + hip_ease) / 4.0;
    let leg_len = sclamp(m.inseam_mm + len_add, 200.0, 1150.0);
    let y_crotch = leg_len;
    let y_ws = y_crotch + sclamp(m.crotch_depth_mm + rise_add, 130.0, 520.0);
    let y_hip = sclamp(y_ws - rise_add - m.waist_to_hip_mm, y_crotch + 25.0, y_ws - 60.0);
    let y_knee = sclamp(leg_len * 0.5 + 50.0, 80.0, leg_len - 80.0);
    let pleat_total = pleat_count as f64 * 2.0 * pleat_depth;

    // Inseam: hem -> fork, bowed outward by 45 % of the bulge, slightly concave near the fork.
    let inseam_ctrl = |p_hem: Point2, p_fork: Point2| -> (Point2, Point2) {
        let b = 1.333 * 0.45 * bulge;
        (
            p_hem.lerp(p_fork, 1.0 / 3.0).add(Point2::new(b, 0.0)),
            p_hem.lerp(p_fork, 2.0 / 3.0).add(Point2::new(0.85 * b - 4.0, 0.0)),
        )
    };
    // Outseam in two spans meeting tangentially (G1) at the side hip point, so the hip girth is
    // exact on the hip line: waist -> hip follows the hip curve, hip -> hem runs the leg and
    // bows outward by the bulge at the knee.
    #[allow(clippy::too_many_arguments)]
    let outseam_edges = |hip_id, leg_id, p_waist: Point2, hip_pt: Point2, p_hem: Point2, y_fork: f64, hip_label: &str, leg_label: &str| {
        let s = (y_knee / y_fork).clamp(0.0, 1.0);
        let x_line_knee = p_hem.x + (hip_pt.x - p_hem.x) * s;
        let knee_ctrl = Point2::new(x_line_knee - 1.333 * bulge, y_knee);
        let t_dir = knee_ctrl.sub(hip_pt).normalized();
        let hip_span = (p_waist.y - hip_pt.y).max(20.0);
        let leg_span = (hip_pt.y - y_knee).max(20.0);
        (
            cubic(
                hip_id,
                p_waist,
                p_waist.add(Point2::new(0.0, -0.4 * hip_span)),
                hip_pt.sub(t_dir.scale(0.4 * hip_span)),
                hip_pt,
                15.0,
                MITER,
                hip_label,
            ),
            cubic(
                leg_id,
                hip_pt,
                hip_pt.add(t_dir.scale(0.45 * leg_span)),
                knee_ctrl,
                p_hem,
                15.0,
                MITER,
                leg_label,
            ),
        )
    };

    // Crotch curve: fork -> CF/CB point on the hip line. It leaves the fork horizontally and
    // arrives tangent to the straight CF/CB line (G1), so the hip line meets CF/CB exactly.
    let j_curve_ctrl = |fork: Point2,
                        hip_pt: Point2,
                        waist_pt: Point2,
                        fork_handle: f64,
                        dip: f64|
     -> (Point2, Point2) {
        let d = hip_pt.sub(waist_pt).normalized();
        let depth = (hip_pt.y - fork.y).max(10.0);
        (
            Point2::new(fork.x - fork_handle, fork.y - dip),
            hip_pt.add(d.scale(0.55 * depth)),
        )
    };

    // ============================ FRONT LEG ============================
    let fe = |s: &str| id(&format!("front:edge:{s}"));
    let x_side = 20.0;
    let x_cf_hip = x_side + (q - 10.0);
    let x_fork = x_cf_hip + f_fork;
    let x_crease = 0.5 * (x_side + x_fork) - 5.0;
    let f_hem_half = sclamp(opening / 4.0 - 10.0, 45.0, 0.5 * (x_fork - x_side) + 90.0);
    let f_waist_fin = sclamp((m.waist_mm + waist_ease) / 4.0 - 5.0, 70.0, 700.0);
    let f_waist_len = f_waist_fin + pleat_total;
    let x_cf_w = x_cf_hip - 10.0;
    let y_cf_w = y_ws - 8.0;
    let dx_fw = (f_waist_len * f_waist_len - 64.0).max(1.0).sqrt();
    let x_sw = x_cf_w - dx_fw;

    let f_p0 = Point2::new(x_crease - f_hem_half, 0.0);
    let f_p1 = Point2::new(x_crease + f_hem_half, 0.0);
    let f_p2 = Point2::new(x_fork, y_crotch);
    let f_hip_cf = Point2::new(x_cf_hip, y_hip);
    let f_p3 = Point2::new(x_cf_w, y_cf_w);
    let f_p4 = Point2::new(x_sw, y_ws);
    let (f_in_c1, f_in_c2) = inseam_ctrl(f_p1, f_p2);
    let (f_out_hip, f_out_leg) = outseam_edges(
        fe("outseam_hip"),
        fe("outseam"),
        f_p4,
        Point2::new(x_side, y_hip),
        f_p0,
        y_crotch,
        "Front Outseam (Hip)",
        "Front Outseam",
    );
    let (f_j_c1, f_j_c2) = j_curve_ctrl(f_p2, f_hip_cf, f_p3, 0.5 * f_fork, 0.0);
    let mut f_hem_edge = line(fe("hem"), f_p0, f_p1, 40.0, CornerRule::FoldedHem, "Front Hem");
    f_hem_edge.hem = Some(crate::model::HemSpec {
        depth_mm: 35.0,
        fold_count: 2,
        turn_out: false,
    });
    let front_edges = vec![
        f_hem_edge,
        cubic(fe("inseam"), f_p1, f_in_c1, f_in_c2, f_p2, 15.0, MITER, "Front Inseam"),
        cubic(fe("crotch"), f_p2, f_j_c1, f_j_c2, f_hip_cf, 12.0, MITER, "Front Crotch J-Curve"),
        line(fe("cf"), f_hip_cf, f_p3, 12.0, MITER, "Front Crotch (CF Line)"),
        line(fe("waist"), f_p3, f_p4, 10.0, MITER, "Front Waist (Pleated)"),
        f_out_hip,
        f_out_leg,
    ];

    // Pleats: measured from CF along the waist. Pleat 1 folds on the crease; pleat 2 sits
    // 36 mm toward the side, kept clear of the side-pocket zone.
    let waist_pt = |s: f64| f_p3.lerp(f_p4, s / f_waist_len);
    let mut pleats: Vec<(f64, f64)> = Vec::new();
    if pleat_count >= 1 && pleat_depth > 0.5 {
        let w = 2.0 * pleat_depth;
        let s_crease = (x_cf_w - x_crease) * f_waist_len / dx_fw;
        let s1 = sclamp(s_crease, 10.0, f_waist_len - w - 20.0);
        pleats.push((s1, s1 + w));
        if pleat_count >= 2 {
            let s2 = sclamp(s1 + w + 36.0, s1 + w + 6.0, f_waist_len - w - 25.0);
            pleats.push((s2, s2 + w));
        }
    }
    let mut front_notches = Vec::new();
    let mut front_lines = vec![
        InternalLine {
            id: id("front:line:knee"),
            label: "Knee Line".into(),
            pts: vec![
                Point2::new(x_crease - f_hem_half - bulge, y_knee),
                Point2::new(x_crease + f_hem_half + bulge * 0.45, y_knee),
            ],
            dashed: true,
        },
        InternalLine {
            id: id("front:line:hip"),
            label: "Hip Line".into(),
            pts: vec![Point2::new(x_side, y_hip), Point2::new(x_cf_hip, y_hip)],
            dashed: true,
        },
        InternalLine {
            id: id("front:line:crotch"),
            label: "Crotch Line".into(),
            pts: vec![Point2::new(x_side, y_crotch), Point2::new(x_fork, y_crotch)],
            dashed: true,
        },
    ];
    for (i, (a, b)) in pleats.iter().enumerate() {
        for (leg, s) in [("a", *a), ("b", *b)] {
            front_notches.push(notch(
                id(&format!("front:notch:pleat{}{leg}", i + 1)),
                4,
                &front_edges[4],
                s / f_waist_len,
                NotchStyle::Slit,
                1,
            ));
            let top = waist_pt(s);
            front_lines.push(InternalLine {
                id: id(&format!("front:line:pleat{}{leg}", i + 1)),
                label: format!(
                    "Pleat {} {}",
                    i + 1,
                    if leg == "a" { "Fold" } else { "Placement" }
                ),
                pts: vec![top, top.add(Point2::new(0.0, -95.0))],
                dashed: true,
            });
        }
    }
    front_notches.push(notch(
        id("front:notch:knee_in"),
        1,
        &front_edges[1],
        t_at_y(&front_edges[1], y_knee),
        NotchStyle::Slit,
        1,
    ));
    front_notches.push(notch(
        id("front:notch:knee_out"),
        6,
        &front_edges[6],
        t_at_y(&front_edges[6], y_knee),
        NotchStyle::Slit,
        1,
    ));
    front_notches.push(notch(
        id("front:notch:pocket"),
        5,
        &front_edges[5],
        t_at_y(&front_edges[5], sclamp(y_ws - 170.0, y_hip + 10.0, y_ws - 30.0)),
        NotchStyle::Slit,
        1,
    ));
    front_notches.push(notch(
        id("front:notch:fly_stop"),
        3,
        &front_edges[3],
        t_at_y(&front_edges[3], sclamp(y_cf_w - 185.0, y_hip + 10.0, y_cf_w - 30.0)),
        NotchStyle::Slit,
        1,
    ));
    let (f_in, f_cf, f_waist) = (front_edges[1].id, front_edges[3].id, front_edges[4].id);
    let (f_out_h, f_out) = (front_edges[5].id, front_edges[6].id);
    let mut front_loop = ClosedPolyCurve::from_edges(front_edges);
    front_loop.ensure_ccw();
    let front = PatternPiece {
        id: id("piece:front"),
        name: "Front Leg".into(),
        code: format!("{code}-F01"),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(x_crease, 100.0),
            end: Point2::new(x_crease, y_hip - 30.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: front_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: front_lines,
        drill_holes: vec![],
        notches: front_notches,
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::FrontLeg,
        drawstrings: vec![],
        zippers: vec![ZipperSpec {
            id: id("front:zipper:fly"),
            label: "Front Fly Zipper".into(),
            edge_id: f_cf,
            zip_fraction: 1.0,
            teeth_width_mm: 5.0,
            color_hex: "#b5b8be".into(),
        }],
        buttons: vec![],
        belt_loops: vec![],
    };

    // ============================ BACK LEG ============================
    let be = |s: &str| id(&format!("back:edge:{s}"));
    let b_hip_w = q + 10.0;
    let b_hem_half = sclamp(opening / 4.0 + 10.0, 50.0, 0.5 * (b_hip_w + b_fork) + 100.0);
    // Leave a clear gap after the front leg, including the outward bow of the back outseam.
    let front_max_x = x_fork.max(x_crease + f_hem_half + bulge * 0.6);
    let rel_hem_out = 0.5 * (b_hip_w + b_fork) - b_hem_half - 1.4 * bulge;
    let x_side_b = front_max_x + 150.0 + (-rel_hem_out).max(0.0);
    let x_cb_hip = x_side_b + b_hip_w;
    let x_fork_b = x_cb_hip + b_fork;
    let y_crotch_b = y_crotch - 10.0;
    let x_crease_b = 0.5 * (x_side_b + x_fork_b);
    let seat_in = 30.0;
    let b_waist_fin = sclamp((m.waist_mm + waist_ease) / 4.0 + 5.0, 70.0, 700.0);
    let dart = sclamp((b_hip_w - seat_in - b_waist_fin) * 0.4, 0.0, 30.0);
    let b_waist_len = b_waist_fin + dart;
    let x_cb_w = x_cb_hip - seat_in;
    let y_cb_w = y_ws + 25.0;
    let dx_bw = (b_waist_len * b_waist_len - 625.0).max(1.0).sqrt();
    let x_sw_b = x_cb_w - dx_bw;

    let b_p0 = Point2::new(x_crease_b - b_hem_half, 0.0);
    let b_p1 = Point2::new(x_crease_b + b_hem_half, 0.0);
    let b_p2 = Point2::new(x_fork_b, y_crotch_b);
    let b_hip_cb = Point2::new(x_cb_hip, y_hip);
    let b_p3 = Point2::new(x_cb_w, y_cb_w);
    let b_p4 = Point2::new(x_sw_b, y_ws);
    let (b_in_c1, b_in_c2) = inseam_ctrl(b_p1, b_p2);
    let (b_out_hip, b_out_leg) = outseam_edges(
        be("outseam_hip"),
        be("outseam"),
        b_p4,
        Point2::new(x_side_b, y_hip),
        b_p0,
        y_crotch_b,
        "Back Outseam (Hip)",
        "Back Outseam",
    );
    let (b_j_c1, b_j_c2) = j_curve_ctrl(b_p2, b_hip_cb, b_p3, 0.55 * b_fork, 2.0);
    let mut b_hem_edge = line(be("hem"), b_p0, b_p1, 40.0, CornerRule::FoldedHem, "Back Hem");
    b_hem_edge.hem = Some(crate::model::HemSpec {
        depth_mm: 35.0,
        fold_count: 2,
        turn_out: false,
    });
    let back_edges = vec![
        b_hem_edge,
        cubic(be("inseam"), b_p1, b_in_c1, b_in_c2, b_p2, 15.0, MITER, "Back Inseam"),
        cubic(be("crotch"), b_p2, b_j_c1, b_j_c2, b_hip_cb, 12.0, MITER, "Back Crotch Curve"),
        line(be("seat"), b_hip_cb, b_p3, 15.0, MITER, "Back Seat Seam"),
        line(be("waist"), b_p3, b_p4, 10.0, MITER, "Back Waist"),
        b_out_hip,
        b_out_leg,
    ];
    let back_notches = vec![
        notch(
            id("back:notch:knee_in"),
            1,
            &back_edges[1],
            t_at_y(&back_edges[1], y_knee),
            NotchStyle::Slit,
            2,
        ),
        notch(
            id("back:notch:knee_out"),
            6,
            &back_edges[6],
            t_at_y(&back_edges[6], y_knee),
            NotchStyle::Slit,
            2,
        ),
    ];
    let mut back_darts = Vec::new();
    let mut back_holes = Vec::new();
    if dart > 3.0 {
        let half = 0.5 * dart / b_waist_len;
        let apex = b_p3.lerp(b_p4, 0.5).add(Point2::new(0.0, -115.0));
        back_darts.push(DartEntity {
            id: id("back:dart:waist"),
            name: "Back Waist Dart".into(),
            apex,
            edge_idx: 4,
            edge_id: Some(back_edges[4].id),
            leg_a_t: 0.5 - half,
            leg_b_t: 0.5 + half,
            intake_mm: dart,
            fold_dir: DartFoldDir::TowardCb,
            cap_style: DartCapStyle::FoldedReflection,
        });
        back_holes.push(crate::model::DrillHole {
            id: id("back:drill:dart"),
            pos: apex,
            radius_mm: 2.0,
            label: "Back Dart Tip".into(),
        });
    }
    let (b_in, b_waist) = (back_edges[1].id, back_edges[4].id);
    let (b_out_h, b_out) = (back_edges[5].id, back_edges[6].id);
    let mut back_loop = ClosedPolyCurve::from_edges(back_edges);
    back_loop.ensure_ccw();
    let back = PatternPiece {
        id: id("piece:back"),
        name: "Back Leg".into(),
        code: format!("{code}-B02"),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(x_crease_b, 100.0),
            end: Point2::new(x_crease_b, y_hip - 30.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: back_loop,
        cutouts: vec![],
        darts: back_darts,
        internal_lines: vec![
            InternalLine {
                id: id("back:line:knee"),
                label: "Knee Line".into(),
                pts: vec![
                    Point2::new(x_crease_b - b_hem_half - bulge, y_knee),
                    Point2::new(x_crease_b + b_hem_half + bulge * 0.45, y_knee),
                ],
                dashed: true,
            },
            InternalLine {
                id: id("back:line:hip"),
                label: "Hip Line".into(),
                pts: vec![Point2::new(x_side_b, y_hip), Point2::new(x_cb_hip, y_hip)],
                dashed: true,
            },
        ],
        drill_holes: back_holes,
        notches: back_notches,
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::BackLeg,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    // ============================ WAISTBAND (half, cut 2, folded) ============================
    let we = |s: &str| id(&format!("waistband:edge:{s}"));
    let wb_len = f_waist_fin + b_waist_fin;
    let ext = 35.0;
    let (wx0, wy0) = (20.0, -(2.0 * wb_h) - 160.0);
    let w_p0 = Point2::new(wx0, wy0);
    let w_p1 = Point2::new(wx0 + ext, wy0);
    let w_p2 = Point2::new(wx0 + ext + wb_len, wy0);
    let w_p3 = Point2::new(wx0 + ext + wb_len, wy0 + 2.0 * wb_h);
    let w_p4 = Point2::new(wx0, wy0 + 2.0 * wb_h);
    let wb_edges = vec![
        line(we("extension"), w_p0, w_p1, 10.0, MITER, "Waistband Extension (Underlap)"),
        line(we("seam"), w_p1, w_p2, 10.0, MITER, "Waistband Seam"),
        line(we("cb"), w_p2, w_p3, 15.0, MITER, "Center Back Seam"),
        line(we("facing"), w_p3, w_p4, 10.0, MITER, "Waistband Facing Edge"),
        line(we("end"), w_p4, w_p0, 10.0, MITER, "Extension End"),
    ];
    let wb_notches = vec![notch(
        id("waistband:notch:side"),
        1,
        &wb_edges[1],
        f_waist_fin / wb_len,
        NotchStyle::Slit,
        1,
    )];
    let (wb_seam, wb_facing) = (wb_edges[1].id, wb_edges[3].id);
    let mut wb_loop = ClosedPolyCurve::from_edges(wb_edges);
    wb_loop.ensure_ccw();
    let waistband = PatternPiece {
        id: id("piece:waistband"),
        name: "Waistband (Half)".into(),
        code: format!("{code}-W03"),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(wx0 + 30.0, wy0 + wb_h * 0.5),
            end: Point2::new(wx0 + ext + wb_len - 30.0, wy0 + wb_h * 0.5),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: wb_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![InternalLine {
            id: id("waistband:line:fold"),
            label: "Fold Line".into(),
            pts: vec![
                Point2::new(wx0, wy0 + wb_h),
                Point2::new(wx0 + ext + wb_len, wy0 + wb_h),
            ],
            dashed: true,
        }],
        drill_holes: vec![],
        notches: wb_notches,
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell (interfaced)".into(),
        arrangement_zone: ArrangementZone::FlatCustom,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![ButtonSpec {
            id: id("waistband:button:front"),
            label: "Waistband Button".into(),
            edge_id: wb_seam,
            param_t: 0.04,
            diameter_mm: 16.0,
            color_hex: "#38332e".into(),
        }],
        belt_loops: vec![BeltLoopSpec {
            id: id("waistband:belt_loops"),
            label: "Belt Loops".into(),
            base_edge_id: wb_seam,
            top_edge_id: Some(wb_facing),
            positions_t: vec![0.18, 0.52, 0.92],
            width_mm: 11.0,
            arch_mm: 5.5,
        }],
    };

    let (fid, bid, wid) = (front.id, back.id, waistband.id);
    let stitching = StitchingGraph {
        bindings: vec![
            binding(
                id("binding:outseam_hip"),
                "Outseam, Waist to Hip (Front <-> Back)",
                (id("iface:front_out_hip"), "Front Outseam (Hip)", vec![eref(fid, f_out_h, 1)]),
                (id("iface:back_out_hip"), "Back Outseam (Hip)", vec![eref(bid, b_out_h, 1)]),
                EaseSpec::Exact,
                SeamType::Plain,
                -2.0,
                2.0,
            ),
            binding(
                id("binding:outseam"),
                "Outseam, Hip to Hem (Front <-> Back)",
                (id("iface:front_out"), "Front Outseam", vec![eref(fid, f_out, 1)]),
                (id("iface:back_out"), "Back Outseam", vec![eref(bid, b_out, 1)]),
                EaseSpec::Exact,
                SeamType::Plain,
                -2.0,
                2.0,
            ),
            binding(
                id("binding:inseam"),
                "Inseam (Front <-> Back)",
                (id("iface:front_in"), "Front Inseam", vec![eref(fid, f_in, 1)]),
                (id("iface:back_in"), "Back Inseam", vec![eref(bid, b_in, 1)]),
                // Back inseam 5 mm short, stretched into the front between crotch and knee.
                EaseSpec::EaseMm { mm: -5.0 },
                SeamType::Plain,
                -10.0,
                3.0,
            ),
            binding(
                id("binding:waistband"),
                "Waistband <-> Trouser Waist [F+B] (pleats + dart)",
                (id("iface:waistband"), "Waistband Seam", vec![eref(wid, wb_seam, 1)]),
                (
                    id("iface:trouser_waist"),
                    "Trouser Waist (CF to CB)",
                    vec![eref(fid, f_waist, 1), eref(bid, b_waist, -1)],
                ),
                EaseSpec::EaseMm {
                    mm: pleat_total + dart,
                },
                SeamType::Plain,
                0.0,
                200.0,
            ),
        ],
    };

    (vec![front, back, waistband], stitching)
}
