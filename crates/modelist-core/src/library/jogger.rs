//! Relaxed Jogger Sweatpants: fleece/jersey leg panels with elastic waistband, elastic ankle
//! cuffs, and an ankle hem (*ourlet*).

use super::{binding, cubic, eref, line, notch, param, sclamp, t_at_y, var, MITER};
use crate::curve::edge_total_length;
use crate::model::{
    det_id, ArrangementZone, BodyMeasurements, ClosedPolyCurve, CornerRule, DrawstringSpec,
    EaseSpec, EaseTable, ElasticBandSpec, GrainRotationMode, GrainlineSpec, HemSpec, InternalLine,
    NotchStyle, ParametricVar, PatternPiece, Point2, SeamType, StitchingGraph,
};
use std::collections::HashMap;

pub const TEMPLATE_ID: &str = "jogger_pants";

pub fn default_variables() -> Vec<ParametricVar> {
    vec![
        var("jg_waist_ease", "160", "Relaxed pull-on waist ease before elastic (mm)"),
        var("jg_hip_ease", "70", "Relaxed hip ease (mm)"),
        var("jg_rise_add", "15", "Waist rise above natural waist (mm)"),
        var("jg_leg_len_add", "-55", "Leg length adjustment above ankle cuff (mm)"),
        var("jg_ankle_opening", "360", "Leg hem circumference before cuff gather (mm)"),
        var("jg_cuff_h", "55", "Ribbed ankle cuff height (mm)"),
        var("jg_waistband_h", "48", "Elastic waistband height (mm)"),
        var("jg_waist_elastic_ratio", "0.80", "Waist elastic contraction ratio (0.55-0.95)"),
        var("jg_ankle_elastic_ratio", "0.78", "Ankle cuff elastic contraction ratio (0.55-0.95)"),
        var("jg_drawstring_len", "190", "Waist drawstring tail length (mm)"),
        var("jg_drawstring_tighten", "0.92", "Waist drawstring cinch ratio (0.70-1.00)"),
    ]
}

pub fn build(
    m: &BodyMeasurements,
    _ease: &EaseTable,
    vals: &HashMap<String, f64>,
) -> (Vec<PatternPiece>, StitchingGraph) {
    let id = |s: &str| det_id(&format!("{TEMPLATE_ID}:{s}"));

    let waist_ease = sclamp(param(vals, "jg_waist_ease", 160.0), 10.0, 260.0);
    let hip_ease = sclamp(param(vals, "jg_hip_ease", 70.0), 20.0, 340.0);
    let rise_add = sclamp(param(vals, "jg_rise_add", 15.0), -60.0, 120.0);
    let len_add = sclamp(param(vals, "jg_leg_len_add", -55.0), -300.0, 200.0);
    let ankle_open = sclamp(param(vals, "jg_ankle_opening", 360.0), 200.0, 620.0);
    let cuff_h = sclamp(param(vals, "jg_cuff_h", 55.0), 25.0, 110.0);
    let wb_h = sclamp(param(vals, "jg_waistband_h", 48.0), 25.0, 95.0);
    let waist_el_ratio = sclamp(param(vals, "jg_waist_elastic_ratio", 0.80), 0.50, 0.96);
    let ankle_el_ratio = sclamp(param(vals, "jg_ankle_elastic_ratio", 0.78), 0.50, 0.96);
    let ds_tail_len = sclamp(param(vals, "jg_drawstring_len", 190.0), 80.0, 320.0);
    let ds_tighten = sclamp(param(vals, "jg_drawstring_tighten", 0.92), 0.70, 1.0);

    let hip = m.hip_mm.max(500.0);
    let q = (hip + hip_ease) / 4.0;
    let leg_len = sclamp(m.inseam_mm + len_add, 220.0, 1100.0);
    let y_crotch = leg_len;
    let y_ws = y_crotch + sclamp(m.crotch_depth_mm + rise_add, 140.0, 480.0);
    let y_hip = sclamp(y_ws - rise_add - m.waist_to_hip_mm, y_crotch + 25.0, y_ws - 55.0);
    let y_knee = sclamp(leg_len * 0.5 + 40.0, 80.0, leg_len - 80.0);

    let f_fork = sclamp((hip + hip_ease) / 16.0, 30.0, 140.0);
    let b_fork = sclamp((hip + hip_ease) / 8.0 - 16.0, f_fork + 10.0, 220.0);

    // Relaxed ribbed waistband cut with negative ease (~88% of natural waist) so it hugs the body;
    // leg top keeps full pull-on ease and gathers into the waistband.
    let wb_half_len = sclamp((m.waist_mm * 0.88) / 2.0, 200.0, 900.0);
    let leg_waist_half = sclamp((m.waist_mm + waist_ease) / 2.0, wb_half_len + 20.0, 960.0);
    let f_waist_len = leg_waist_half * 0.48;
    let b_waist_len = leg_waist_half * 0.52;

    let j_curve_ctrl = |fork: Point2, hip_pt: Point2, waist_pt: Point2, fork_handle: f64, dip: f64| {
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
    let x_cf_hip = x_side + (q - 8.0);
    let x_fork = x_cf_hip + f_fork;
    let x_crease = 0.5 * (x_side + x_fork) - 4.0;
    let f_hem_half = sclamp(ankle_open / 4.0 - 6.0, 45.0, 0.5 * (x_fork - x_side) + 40.0);
    let x_cf_w = x_cf_hip - 8.0;
    let y_cf_w = y_ws;
    let dx_fw = (f_waist_len * f_waist_len - 4.0).max(1.0).sqrt();
    let x_sw = x_cf_w - dx_fw;

    let f_p0 = Point2::new(x_crease - f_hem_half, 0.0);
    let f_p1 = Point2::new(x_crease + f_hem_half, 0.0);
    let f_p2 = Point2::new(x_fork, y_crotch);
    let f_hip_cf = Point2::new(x_cf_hip, y_hip);
    let f_p3 = Point2::new(x_cf_w, y_cf_w);
    let f_p4 = Point2::new(x_sw, y_ws);
    let f_side_hip = Point2::new(x_side, y_hip);

    let hip_span_f = (f_p4.y - y_hip).max(20.0);
    let leg_span_f = (y_hip - y_knee).max(20.0);

    let (f_j_c1, f_j_c2) = j_curve_ctrl(f_p2, f_hip_cf, f_p3, 0.5 * f_fork, 0.0);
    let front_edges = vec![
        line(fe("hem"), f_p0, f_p1, 10.0, MITER, "Front Hem"),
        cubic(
            fe("inseam"),
            f_p1,
            f_p1.lerp(f_p2, 1.0 / 3.0),
            f_p1.lerp(f_p2, 2.0 / 3.0).add(Point2::new(-3.0, 0.0)),
            f_p2,
            12.0,
            MITER,
            "Front Inseam",
        ),
        cubic(fe("crotch"), f_p2, f_j_c1, f_j_c2, f_hip_cf, 12.0, MITER, "Front Crotch J-Curve"),
        line(fe("cf"), f_hip_cf, f_p3, 12.0, MITER, "Front Crotch (CF Line)"),
        line(fe("waist"), f_p3, f_p4, 10.0, MITER, "Front Waist"),
        cubic(
            fe("outseam_hip"),
            f_p4,
            f_p4.add(Point2::new(0.0, -0.35 * hip_span_f)),
            f_side_hip.add(Point2::new(0.0, 0.45 * hip_span_f)),
            f_side_hip,
            12.0,
            MITER,
            "Front Outseam (Hip)",
        ),
        cubic(
            fe("outseam"),
            f_side_hip,
            f_side_hip.add(Point2::new(0.0, -0.35 * leg_span_f)),
            Point2::new(f_p0.lerp(f_side_hip, y_knee / y_crotch).x, y_knee),
            f_p0,
            12.0,
            MITER,
            "Front Outseam",
        ),
    ];
    let f_hem_actual = edge_total_length(&front_edges[0]);
    let f_waist_actual = edge_total_length(&front_edges[4]);
    let (f_hem, f_in, f_waist) = (front_edges[0].id, front_edges[1].id, front_edges[4].id);
    let (f_out_h, f_out) = (front_edges[5].id, front_edges[6].id);
    let front_notches = vec![
        notch(id("front:notch:knee_out"), 6, &front_edges[6], t_at_y(&front_edges[6], y_knee), NotchStyle::Slit, 1),
        notch(id("front:notch:knee_in"), 1, &front_edges[1], t_at_y(&front_edges[1], y_knee), NotchStyle::Slit, 1),
    ];
    let mut front_loop = ClosedPolyCurve::from_edges(front_edges);
    front_loop.ensure_ccw();
    let front = PatternPiece {
        id: id("piece:front"),
        name: "Front Leg".into(),
        code: "JG-F01".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(x_crease, 80.0),
            end: Point2::new(x_crease, y_hip - 30.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: front_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![InternalLine {
            id: id("front:line:knee"),
            label: "Knee Line".into(),
            pts: vec![
                Point2::new(x_crease - f_hem_half, y_knee),
                Point2::new(x_crease + f_hem_half, y_knee),
            ],
            dashed: true,
        }],
        drill_holes: vec![],
        notches: front_notches,
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::FrontLeg,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    // ============================ BACK LEG ============================
    let be = |s: &str| id(&format!("back:edge:{s}"));
    let x_side_b = (x_fork + 90.0).max(x_cf_hip + 140.0);
    let x_cb_hip = x_side_b + (q + 8.0);
    let x_fork_b = x_cb_hip + b_fork;
    let x_crease_b = 0.5 * (x_side_b + x_fork_b) - 8.0;
    let b_hem_half = sclamp(ankle_open / 4.0 + 6.0, 50.0, 0.5 * (x_fork_b - x_side_b) + 40.0);
    let x_cb_w = x_cb_hip - 22.0;
    let y_cb_w = y_ws + 8.0;
    let dx_bw = (b_waist_len * b_waist_len - (y_cb_w - y_ws) * (y_cb_w - y_ws)).max(1.0).sqrt();
    let x_sw_b = x_cb_w - dx_bw;

    let b_p0 = Point2::new(x_crease_b - b_hem_half, 0.0);
    let b_p1 = Point2::new(x_crease_b + b_hem_half, 0.0);
    let b_p2 = Point2::new(x_fork_b, y_crotch);
    let b_hip_cb = Point2::new(x_cb_hip, y_hip);
    let b_p3 = Point2::new(x_cb_w, y_cb_w);
    let b_p4 = Point2::new(x_sw_b, y_ws);
    let b_side_hip = Point2::new(x_side_b, y_hip);

    let hip_span_b = (b_p4.y - y_hip).max(20.0);
    let leg_span_b = (y_hip - y_knee).max(20.0);
    let (b_j_c1, b_j_c2) = j_curve_ctrl(b_p2, b_hip_cb, b_p3, 0.5 * b_fork, 4.0);
    let back_edges = vec![
        line(be("hem"), b_p0, b_p1, 10.0, MITER, "Back Hem"),
        cubic(
            be("inseam"),
            b_p1,
            b_p1.lerp(b_p2, 1.0 / 3.0),
            b_p1.lerp(b_p2, 2.0 / 3.0).add(Point2::new(-4.0, 0.0)),
            b_p2,
            12.0,
            MITER,
            "Back Inseam",
        ),
        cubic(be("seat"), b_p2, b_j_c1, b_j_c2, b_hip_cb, 12.0, MITER, "Back Seat Curve"),
        line(be("cb"), b_hip_cb, b_p3, 12.0, MITER, "Back Seat (CB Line)"),
        line(be("waist"), b_p3, b_p4, 10.0, MITER, "Back Waist"),
        cubic(
            be("outseam_hip"),
            b_p4,
            b_p4.add(Point2::new(0.0, -0.35 * hip_span_b)),
            b_side_hip.add(Point2::new(0.0, 0.45 * hip_span_b)),
            b_side_hip,
            12.0,
            MITER,
            "Back Outseam (Hip)",
        ),
        cubic(
            be("outseam"),
            b_side_hip,
            b_side_hip.add(Point2::new(0.0, -0.35 * leg_span_b)),
            Point2::new(b_p0.lerp(b_side_hip, y_knee / y_crotch).x, y_knee),
            b_p0,
            12.0,
            MITER,
            "Back Outseam",
        ),
    ];
    let b_hem_actual = edge_total_length(&back_edges[0]);
    let b_waist_actual = edge_total_length(&back_edges[4]);
    let (b_hem, b_in, b_waist) = (back_edges[0].id, back_edges[1].id, back_edges[4].id);
    let (b_out_h, b_out) = (back_edges[5].id, back_edges[6].id);
    let back_notches = vec![
        notch(id("back:notch:knee_out"), 6, &back_edges[6], t_at_y(&back_edges[6], y_knee), NotchStyle::Slit, 2),
        notch(id("back:notch:knee_in"), 1, &back_edges[1], t_at_y(&back_edges[1], y_knee), NotchStyle::Slit, 2),
    ];
    let mut back_loop = ClosedPolyCurve::from_edges(back_edges);
    back_loop.ensure_ccw();
    let back = PatternPiece {
        id: id("piece:back"),
        name: "Back Leg".into(),
        code: "JG-B02".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(x_crease_b, 80.0),
            end: Point2::new(x_crease_b, y_hip - 30.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: back_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![InternalLine {
            id: id("back:line:knee"),
            label: "Knee Line".into(),
            pts: vec![
                Point2::new(x_crease_b - b_hem_half, y_knee),
                Point2::new(x_crease_b + b_hem_half, y_knee),
            ],
            dashed: true,
        }],
        drill_holes: vec![],
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

    // ============================ ELASTIC WAISTBAND (half, cut 2) ============================
    let we = |s: &str| id(&format!("waistband:edge:{s}"));
    let (wx0, wy0) = (20.0, -wb_h - 140.0);
    let w_p0 = Point2::new(wx0, wy0);
    let w_p1 = Point2::new(wx0 + wb_half_len, wy0);
    let w_p2 = Point2::new(wx0 + wb_half_len, wy0 + wb_h);
    let w_p3 = Point2::new(wx0, wy0 + wb_h);
    let waist_el = Some(ElasticBandSpec {
        target_ratio: waist_el_ratio,
        width_mm: wb_h,
        stiffness_n_m: 1100.0,
    });
    let mut wb_seam_edge = line(we("seam"), w_p0, w_p1, 10.0, MITER, "Waistband Seam");
    wb_seam_edge.elastic = waist_el;
    let mut wb_top_edge = line(we("top"), w_p2, w_p3, 10.0, CornerRule::FoldedHem, "Waistband Top");
    wb_top_edge.elastic = waist_el;
    wb_top_edge.hem = Some(HemSpec {
        depth_mm: 15.0,
        fold_count: 1,
        turn_out: false,
    });
    let wb_edges = vec![
        wb_seam_edge,
        line(we("cb"), w_p1, w_p2, 10.0, MITER, "Center Back Seam"),
        wb_top_edge,
        line(we("cf"), w_p3, w_p0, 10.0, MITER, "Center Front Seam"),
    ];
    let (wb_seam, wb_top) = (wb_edges[0].id, wb_edges[2].id);
    let mut wb_loop = ClosedPolyCurve::from_edges(wb_edges);
    wb_loop.ensure_ccw();
    let waistband = PatternPiece {
        id: id("piece:waistband"),
        name: "Elastic Waistband (Half)".into(),
        code: "JG-W03".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(wx0 + wb_half_len * 0.5, wy0 + 8.0),
            end: Point2::new(wx0 + wb_half_len * 0.5, wy0 + wb_h - 8.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: wb_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![],
        drill_holes: vec![],
        notches: vec![],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Rib".into(),
        arrangement_zone: ArrangementZone::FlatCustom,
        drawstrings: vec![DrawstringSpec {
            id: id("waistband:drawstring"),
            label: "Waist Drawstring".into(),
            casing_edge_id: wb_top,
            opposite_edge_id: Some(wb_seam),
            tail_length_mm: ds_tail_len,
            cord_radius_mm: 3.0,
            eyelet_offset_mm: 20.0,
            tighten_ratio: ds_tighten,
            color_hex: "#ebe7de".into(),
        }],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    // ============================ ANKLE CUFF (cut 2) ============================
    let ce = |s: &str| id(&format!("cuff:edge:{s}"));
    let leg_hem_total = f_hem_actual + b_hem_actual;
    let cuff_len = sclamp(leg_hem_total * 0.92, 160.0, 560.0);
    let (cx0, cy0) = (20.0, -wb_h - cuff_h - 210.0);
    let c_p0 = Point2::new(cx0, cy0);
    let c_p1 = Point2::new(cx0 + cuff_len, cy0);
    let c_p2 = Point2::new(cx0 + cuff_len, cy0 + cuff_h);
    let c_p3 = Point2::new(cx0, cy0 + cuff_h);
    let ankle_el = Some(ElasticBandSpec {
        target_ratio: ankle_el_ratio,
        width_mm: cuff_h * 0.8,
        stiffness_n_m: 320.0,
    });
    let mut cuff_hem_edge = line(ce("hem"), c_p0, c_p1, 20.0, CornerRule::FoldedHem, "Ankle Cuff Hem");
    cuff_hem_edge.hem = Some(HemSpec {
        depth_mm: 20.0,
        fold_count: 1,
        turn_out: false,
    });
    cuff_hem_edge.elastic = ankle_el;
    let mut cuff_top_edge = line(ce("top"), c_p2, c_p3, 10.0, MITER, "Ankle Cuff Seam");
    cuff_top_edge.elastic = ankle_el;
    let cuff_edges = vec![
        cuff_hem_edge,
        line(ce("end_b"), c_p1, c_p2, 10.0, MITER, "Cuff Seam B"),
        cuff_top_edge,
        line(ce("end_a"), c_p3, c_p0, 10.0, MITER, "Cuff Seam A"),
    ];
    let (c_end_b, c_top, c_end_a) = (cuff_edges[1].id, cuff_edges[2].id, cuff_edges[3].id);
    let mut cuff_loop = ClosedPolyCurve::from_edges(cuff_edges);
    cuff_loop.ensure_ccw();
    let cuff = PatternPiece {
        id: id("piece:cuff"),
        name: "Ankle Cuff".into(),
        code: "JG-C04".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(cx0 + cuff_len * 0.5, cy0 + 8.0),
            end: Point2::new(cx0 + cuff_len * 0.5, cy0 + cuff_h - 8.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: cuff_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![],
        drill_holes: vec![],
        notches: vec![],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Rib".into(),
        arrangement_zone: ArrangementZone::FlatCustom,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    let (fid, bid, wid, cid) = (front.id, back.id, waistband.id, cuff.id);
    let waist_ease_actual = (f_waist_actual + b_waist_actual) - wb_half_len;
    let cuff_ease_actual = leg_hem_total - cuff_len;

    let stitching = StitchingGraph {
        bindings: vec![
            binding(
                id("binding:outseam_hip"),
                "Outseam, Waist to Hip (Front <-> Back)",
                (id("iface:front_out_hip"), "Front Outseam (Hip)", vec![eref(fid, f_out_h, 1)]),
                (id("iface:back_out_hip"), "Back Outseam (Hip)", vec![eref(bid, b_out_h, 1)]),
                EaseSpec::Exact,
                SeamType::Overlock,
                -2.0,
                2.0,
            ),
            binding(
                id("binding:outseam"),
                "Outseam, Hip to Hem (Front <-> Back)",
                (id("iface:front_out"), "Front Outseam", vec![eref(fid, f_out, 1)]),
                (id("iface:back_out"), "Back Outseam", vec![eref(bid, b_out, 1)]),
                EaseSpec::Exact,
                SeamType::Overlock,
                -2.0,
                2.0,
            ),
            binding(
                id("binding:inseam"),
                "Inseam (Front <-> Back)",
                (id("iface:front_in"), "Front Inseam", vec![eref(fid, f_in, 1)]),
                (id("iface:back_in"), "Back Inseam", vec![eref(bid, b_in, 1)]),
                EaseSpec::Exact,
                SeamType::Overlock,
                -2.0,
                2.0,
            ),
            binding(
                id("binding:waistband"),
                "Elastic Waistband <-> Trouser Waist [F+B]",
                (id("iface:waistband"), "Waistband Seam", vec![eref(wid, wb_seam, 1)]),
                (
                    id("iface:trouser_waist"),
                    "Trouser Waist (CF to CB)",
                    vec![eref(fid, f_waist, 1), eref(bid, b_waist, -1)],
                ),
                EaseSpec::EaseMm { mm: waist_ease_actual },
                SeamType::Overlock,
                -5.0,
                300.0,
            ),
            binding(
                id("binding:ankle_cuff"),
                "Ankle Cuff <-> Leg Hem [F+B]",
                (id("iface:cuff_top"), "Ankle Cuff Seam", vec![eref(cid, c_top, -1)]),
                (
                    id("iface:leg_hem"),
                    "Leg Hem (Front + Back)",
                    vec![eref(fid, f_hem, 1), eref(bid, b_hem, -1)],
                ),
                EaseSpec::EaseMm { mm: cuff_ease_actual },
                SeamType::Overlock,
                -5.0,
                160.0,
            ),
            binding(
                id("binding:cuff_ring"),
                "Ankle Cuff Ring Seam",
                (id("iface:cuff_end_a"), "Cuff Seam A", vec![eref(cid, c_end_a, 1)]),
                (id("iface:cuff_end_b"), "Cuff Seam B", vec![eref(cid, c_end_b, -1)]),
                EaseSpec::Exact,
                SeamType::Overlock,
                -2.0,
                2.0,
            ),
        ],
    };

    (vec![front, back, waistband, cuff], stitching)
}

