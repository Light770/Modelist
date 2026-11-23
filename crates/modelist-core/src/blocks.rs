use crate::expr::evaluate_dag;
use crate::model::{
    det_id, BodyMeasurements, ClosedPolyCurve, CornerRule, CurveSegment, DartCapStyle, DartEntity,
    DartFoldDir, DrillHole, EaseTable, GrainRotationMode, GrainlineSpec, InternalLine, NotchAnchor,
    NotchStyle, ParametricOp, ParametricVar, PatternPiece, Point2, PolyEdge, StitchLink,
    StitchingGraph,
};
use crate::ops::apply_ops_to_piece;
use std::collections::HashMap;

pub fn build_default_variables(template_id: &str) -> Vec<ParametricVar> {
    match template_id {
        "aline_skirt" => vec![
            ParametricVar {
                name: "skirt_q_waist".into(),
                expr: "(waist + ease_waist) / 4".into(),
                description: "Quarter waist including ease".into(),
            },
            ParametricVar {
                name: "skirt_q_hip".into(),
                expr: "(hip + ease_hip) / 4".into(),
                description: "Quarter hip including ease".into(),
            },
            ParametricVar {
                name: "skirt_flare".into(),
                expr: "max(35, (skirt_q_hip - skirt_q_waist) * 1.45)".into(),
                description: "A-line hem flare past hip width".into(),
            },
        ],
        _ => vec![
            ParametricVar {
                name: "q_bust".into(),
                expr: "(bust + ease_bust) / 4".into(),
                description: "Quarter bust with ease".into(),
            },
            ParametricVar {
                name: "q_waist".into(),
                expr: "(waist + ease_waist) / 4".into(),
                description: "Quarter waist with ease".into(),
            },
            ParametricVar {
                name: "arm_depth".into(),
                expr: "armscye_depth + ease_armscye".into(),
                description: "Armhole vertical depth".into(),
            },
            ParametricVar {
                name: "sleeve_bicep_half".into(),
                expr: "(bicep + ease_bicep) / 2".into(),
                description: "Half bicep width with ease".into(),
            },
        ],
    }
}

pub fn build_template_pieces(
    template_id: &str,
    measurements: &BodyMeasurements,
    ease: &EaseTable,
    vars: &[ParametricVar],
    ops: &[ParametricOp],
    custom_pieces: &[PatternPiece],
) -> (
    Vec<PatternPiece>,
    StitchingGraph,
    HashMap<String, f64>,
    HashMap<String, String>,
) {
    let mut base_inputs = measurements.to_var_map();
    base_inputs.extend(ease.to_var_map());
    let dag = evaluate_dag(&base_inputs, vars, None);
    let vals = &dag.values;

    let (mut pieces, stitching) = match template_id {
        "aline_skirt" => build_aline_skirt(measurements, ease, vals),
        _ => build_bodice_and_sleeve(measurements, ease, vals),
    };

    for cp in custom_pieces {
        pieces.push(cp.clone());
    }

    for piece in &mut pieces {
        apply_ops_to_piece(piece, ops);
    }

    (pieces, stitching, dag.values, dag.errors)
}

fn build_bodice_and_sleeve(
    m: &BodyMeasurements,
    ease: &EaseTable,
    vals: &HashMap<String, f64>,
) -> (Vec<PatternPiece>, StitchingGraph) {
    let q_bust = vals
        .get("q_bust")
        .copied()
        .unwrap_or((m.bust_mm + ease.bust_ease_mm) / 4.0);
    let q_waist = vals
        .get("q_waist")
        .copied()
        .unwrap_or((m.waist_mm + ease.waist_ease_mm) / 4.0);
    let arm_depth = vals
        .get("arm_depth")
        .copied()
        .unwrap_or(m.armscye_depth_mm + ease.armscye_ease_mm);
    let sleeve_half_w = vals
        .get("sleeve_bicep_half")
        .copied()
        .unwrap_or((m.bicep_mm + ease.bicep_ease_mm) / 2.0);

    let bodice_len = m.back_waist_len_mm.max(arm_depth + 90.0);
    let front_w = q_bust + 8.0;
    let back_w = (q_bust - 8.0).max(160.0);
    let side_y = (bodice_len - arm_depth).max(110.0);
    let half_sh = (m.shoulder_width_mm * 0.5).clamp(130.0, front_w - 25.0);
    let neck_w = (m.bust_mm * 0.075 + 4.0).clamp(58.0, half_sh - 35.0);
    let neck_d_front = (neck_w + 10.0).min(side_y - 35.0);
    let neck_d_back = 22.0;

    let front_dart_intake = (front_w - q_waist - 18.0).clamp(18.0, 44.0);
    let back_dart_intake = (back_w - q_waist - 14.0).clamp(16.0, 38.0);
    let front_waist_w = (q_waist + front_dart_intake).min(front_w - 6.0);
    let back_waist_w = (q_waist + back_dart_intake).min(back_w - 6.0);

    // Match front and back shoulder drop accurately so shoulders sew 1:1
    let sh_drop_f = 22.0;
    let sh_dx_f = half_sh - neck_w;
    let sh_len = sh_dx_f.hypot(sh_drop_f);
    let sh_drop_b = 20.0;
    let sh_dx_b = (sh_len * sh_len - sh_drop_b * sh_drop_b).max(100.0).sqrt();
    let back_sh_x = neck_w + sh_dx_b;

    // Match back side seam length to front side seam length cleanly
    let front_side_dx = front_w - front_waist_w;
    let front_side_len = front_side_dx.hypot(side_y);
    let back_side_dx = back_w - back_waist_w;
    let back_side_y = (front_side_len * front_side_len - back_side_dx * back_side_dx)
        .max(90.0 * 90.0)
        .sqrt();

    // --- PIECE 1: FRONT BODICE ---
    let fb_id = det_id("piece:bodice_front");
    let fb_p0 = Point2::new(0.0, 0.0);
    let fb_p1 = Point2::new(front_waist_w, 0.0);
    let fb_p2 = Point2::new(front_w, side_y);
    let fb_p3 = Point2::new(half_sh, bodice_len - sh_drop_f);
    let fb_p4 = Point2::new(neck_w, bodice_len);
    let fb_p5 = Point2::new(0.0, bodice_len - neck_d_front);

    let fb_arm_c1 = Point2::new(front_w - 2.0, side_y + (bodice_len - sh_drop_f - side_y) * 0.48);
    let fb_arm_c2 = Point2::new(half_sh + 10.0, side_y + (bodice_len - sh_drop_f - side_y) * 0.34);

    let fb_neck_c1 = Point2::new(neck_w - 4.0, bodice_len - neck_d_front * 0.52);
    let fb_neck_c2 = Point2::new(neck_w * 0.45, bodice_len - neck_d_front);

    let fb_dart_apex = Point2::new(front_waist_w * 0.46, side_y - 25.0);
    let fb_dart_a = Point2::new(front_waist_w * 0.46 - front_dart_intake * 0.5, 0.0);
    let fb_dart_b = Point2::new(front_waist_w * 0.46 + front_dart_intake * 0.5, 0.0);

    let fb_edges = vec![
        PolyEdge {
            id: det_id("fb:edge:waist"),
            start: fb_p0,
            segment: CurveSegment::Line { end: fb_p1 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Waist Hem".into(),
        },
        PolyEdge {
            id: det_id("fb:edge:side"),
            start: fb_p1,
            segment: CurveSegment::Line { end: fb_p2 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Side Seam".into(),
        },
        PolyEdge {
            id: det_id("fb:edge:armscye"),
            start: fb_p2,
            segment: CurveSegment::Cubic {
                c1: fb_arm_c1,
                c2: fb_arm_c2,
                end: fb_p3,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Armscye".into(),
        },
        PolyEdge {
            id: det_id("fb:edge:shoulder"),
            start: fb_p3,
            segment: CurveSegment::Line { end: fb_p4 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Shoulder".into(),
        },
        PolyEdge {
            id: det_id("fb:edge:neckline"),
            start: fb_p4,
            segment: CurveSegment::Cubic {
                c1: fb_neck_c1,
                c2: fb_neck_c2,
                end: fb_p5,
            },
            sa_width_mm: 6.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Neckline".into(),
        },
        PolyEdge {
            id: det_id("fb:edge:cf"),
            start: fb_p5,
            segment: CurveSegment::Line { end: fb_p0 },
            sa_width_mm: 0.0,
            corner_rule: CornerRule::Square,
            label: "Center Front (Fold)".into(),
        },
    ];

    let mut fb_loop = ClosedPolyCurve::from_edges(fb_edges);
    fb_loop.ensure_ccw();
    let front_arm_len = crate::curve::edge_total_length(&fb_loop.edges[2]);

    let front_bodice = PatternPiece {
        id: fb_id,
        name: "Front Bodice".into(),
        code: "FB-01".into(),
        cut_quantity: 1,
        mirror_fold_edge: Some(5),
        grainline: GrainlineSpec {
            start: Point2::new(32.0, 55.0),
            end: Point2::new(32.0, (bodice_len - neck_d_front - 35.0).max(120.0)),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: fb_loop,
        cutouts: vec![],
        darts: vec![DartEntity {
            id: det_id("fb:dart:waist"),
            name: "Front Waist Dart".into(),
            apex: fb_dart_apex,
            edge_idx: 0,
            leg_a_t: (fb_dart_a.x / front_waist_w).clamp(0.15, 0.48),
            leg_b_t: (fb_dart_b.x / front_waist_w).clamp(0.52, 0.85),
            intake_mm: front_dart_intake,
            fold_dir: DartFoldDir::TowardCf,
            cap_style: DartCapStyle::FoldedReflection,
        }],
        internal_lines: vec![InternalLine {
            id: det_id("fb:line:bust_level"),
            label: "Bust Line".into(),
            pts: vec![Point2::new(0.0, side_y), Point2::new(front_w, side_y)],
            dashed: true,
        }],
        drill_holes: vec![DrillHole {
            id: det_id("fb:drill:bust_apex"),
            pos: fb_dart_apex,
            radius_mm: 2.0,
            label: "Bust Apex".into(),
        }],
        notches: vec![
            NotchAnchor {
                id: det_id("fb:notch:armscye"),
                edge_idx: 2,
                param_t: 0.35,
                style: NotchStyle::V,
                count: 1,
                depth_mm: 5.0,
            },
            NotchAnchor {
                id: det_id("fb:notch:side"),
                edge_idx: 1,
                param_t: 0.5,
                style: NotchStyle::Slit,
                count: 1,
                depth_mm: 5.0,
            },
        ],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
    };

    // --- PIECE 2: BACK BODICE ---
    let bb_id = det_id("piece:bodice_back");
    let x_bb = front_w + 100.0;
    let bb_p0 = Point2::new(x_bb, 0.0);
    let bb_p1 = Point2::new(x_bb + back_waist_w, 0.0);
    let bb_p2 = Point2::new(x_bb + back_w, back_side_y);
    let bb_p3 = Point2::new(x_bb + back_sh_x, bodice_len - sh_drop_b);
    let bb_p4 = Point2::new(x_bb + neck_w, bodice_len);
    let bb_p5 = Point2::new(x_bb, bodice_len - neck_d_back);

    let bb_arm_c1 = Point2::new(
        x_bb + back_w - 4.0,
        back_side_y + (bodice_len - sh_drop_b - back_side_y) * 0.42,
    );
    let bb_arm_c2 = Point2::new(
        x_bb + back_sh_x + 4.0,
        back_side_y + (bodice_len - sh_drop_b - back_side_y) * 0.25,
    );
    let bb_neck_c1 = Point2::new(x_bb + neck_w - 6.0, bodice_len - neck_d_back * 0.72);
    let bb_neck_c2 = Point2::new(x_bb + neck_w * 0.42, bodice_len - neck_d_back);

    let bb_dart_apex = Point2::new(x_bb + back_waist_w * 0.44, back_side_y + 20.0);
    let bb_dart_a_x = back_waist_w * 0.44 - back_dart_intake * 0.5;
    let bb_dart_b_x = back_waist_w * 0.44 + back_dart_intake * 0.5;

    let bb_edges = vec![
        PolyEdge {
            id: det_id("bb:edge:waist"),
            start: bb_p0,
            segment: CurveSegment::Line { end: bb_p1 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Waist Hem".into(),
        },
        PolyEdge {
            id: det_id("bb:edge:side"),
            start: bb_p1,
            segment: CurveSegment::Line { end: bb_p2 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Side Seam".into(),
        },
        PolyEdge {
            id: det_id("bb:edge:armscye"),
            start: bb_p2,
            segment: CurveSegment::Cubic {
                c1: bb_arm_c1,
                c2: bb_arm_c2,
                end: bb_p3,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Armscye".into(),
        },
        PolyEdge {
            id: det_id("bb:edge:shoulder"),
            start: bb_p3,
            segment: CurveSegment::Line { end: bb_p4 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Shoulder".into(),
        },
        PolyEdge {
            id: det_id("bb:edge:neckline"),
            start: bb_p4,
            segment: CurveSegment::Cubic {
                c1: bb_neck_c1,
                c2: bb_neck_c2,
                end: bb_p5,
            },
            sa_width_mm: 6.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Neckline".into(),
        },
        PolyEdge {
            id: det_id("bb:edge:cb"),
            start: bb_p5,
            segment: CurveSegment::Line { end: bb_p0 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Square,
            label: "Center Back (Zip)".into(),
        },
    ];

    let mut bb_loop = ClosedPolyCurve::from_edges(bb_edges);
    bb_loop.ensure_ccw();
    let back_arm_len = crate::curve::edge_total_length(&bb_loop.edges[2]);

    let back_bodice = PatternPiece {
        id: bb_id,
        name: "Back Bodice".into(),
        code: "BB-02".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(x_bb + 36.0, 55.0),
            end: Point2::new(x_bb + 36.0, bodice_len - 55.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: bb_loop,
        cutouts: vec![],
        darts: vec![DartEntity {
            id: det_id("bb:dart:waist"),
            name: "Back Waist Dart".into(),
            apex: bb_dart_apex,
            edge_idx: 0,
            leg_a_t: (bb_dart_a_x / back_waist_w).clamp(0.15, 0.48),
            leg_b_t: (bb_dart_b_x / back_waist_w).clamp(0.52, 0.85),
            intake_mm: back_dart_intake,
            fold_dir: DartFoldDir::TowardCb,
            cap_style: DartCapStyle::FoldedReflection,
        }],
        internal_lines: vec![InternalLine {
            id: det_id("bb:line:waist_ref"),
            label: "Across Back".into(),
            pts: vec![
                Point2::new(x_bb, back_side_y + (bodice_len - back_side_y) * 0.45),
                Point2::new(
                    x_bb + back_w * 0.92,
                    back_side_y + (bodice_len - back_side_y) * 0.45,
                ),
            ],
            dashed: true,
        }],
        drill_holes: vec![DrillHole {
            id: det_id("bb:drill:dart_tip"),
            pos: bb_dart_apex,
            radius_mm: 2.0,
            label: "Back Dart Tip".into(),
        }],
        notches: vec![
            NotchAnchor {
                id: det_id("bb:notch:armscye"),
                edge_idx: 2,
                param_t: 0.35,
                style: NotchStyle::V,
                count: 2,
                depth_mm: 5.0,
            },
            NotchAnchor {
                id: det_id("bb:notch:side"),
                edge_idx: 1,
                param_t: 0.5,
                style: NotchStyle::Slit,
                count: 1,
                depth_mm: 5.0,
            },
        ],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
    };

    // --- PIECE 3: SET-IN SLEEVE ---
    // Tune cap height iteratively so total sleeve cap closely equals front + back armscye + sleeve_cap_ease
    let sl_id = det_id("piece:sleeve");
    let cx = x_bb + back_w + 115.0 + sleeve_half_w;
    let wrist_half_w = ((m.wrist_mm + 34.0) * 0.5).clamp(80.0, sleeve_half_w - 15.0);
    let y_cap = bodice_len;
    let y_hem = bodice_len - m.sleeve_len_mm.max(250.0);

    let target_cap_total = front_arm_len + back_arm_len + ease.sleeve_cap_ease_mm;
    let mut cap_h = (arm_depth * 0.68).max(95.0);

    for _ in 0..5 {
        let y_bicep = y_cap - cap_h;
        let test_back = PolyEdge {
            id: det_id("sl:tmp_b"),
            start: Point2::new(cx + sleeve_half_w, y_bicep),
            segment: CurveSegment::Cubic {
                c1: Point2::new(cx + sleeve_half_w * 0.52, y_bicep + cap_h * 0.14),
                c2: Point2::new(cx + sleeve_half_w * 0.42, y_cap),
                end: Point2::new(cx, y_cap),
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: String::new(),
        };
        let test_front = PolyEdge {
            id: det_id("sl:tmp_f"),
            start: Point2::new(cx, y_cap),
            segment: CurveSegment::Cubic {
                c1: Point2::new(cx - sleeve_half_w * 0.42, y_cap),
                c2: Point2::new(cx - sleeve_half_w * 0.52, y_bicep + cap_h * 0.10),
                end: Point2::new(cx - sleeve_half_w, y_bicep),
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: String::new(),
        };
        let curr_total =
            crate::curve::edge_total_length(&test_back) + crate::curve::edge_total_length(&test_front);
        let diff = target_cap_total - curr_total;
        cap_h = (cap_h + diff * 0.65).clamp(70.0, m.sleeve_len_mm * 0.45);
    }

    let y_bicep = y_cap - cap_h;
    let sl_p0 = Point2::new(cx - wrist_half_w, y_hem);
    let sl_p1 = Point2::new(cx + wrist_half_w, y_hem);
    let sl_p2 = Point2::new(cx + sleeve_half_w, y_bicep);
    let sl_p3 = Point2::new(cx, y_cap);
    let sl_p4 = Point2::new(cx - sleeve_half_w, y_bicep);

    let sl_edges = vec![
        PolyEdge {
            id: det_id("sl:edge:hem"),
            start: sl_p0,
            segment: CurveSegment::Line { end: sl_p1 },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Sleeve Hem".into(),
        },
        PolyEdge {
            id: det_id("sl:edge:back_underarm"),
            start: sl_p1,
            segment: CurveSegment::Line { end: sl_p2 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Underarm".into(),
        },
        PolyEdge {
            id: det_id("sl:edge:back_cap"),
            start: sl_p2,
            segment: CurveSegment::Cubic {
                c1: Point2::new(cx + sleeve_half_w * 0.52, y_bicep + cap_h * 0.14),
                c2: Point2::new(cx + sleeve_half_w * 0.42, y_cap),
                end: sl_p3,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Sleeve Cap".into(),
        },
        PolyEdge {
            id: det_id("sl:edge:front_cap"),
            start: sl_p3,
            segment: CurveSegment::Cubic {
                c1: Point2::new(cx - sleeve_half_w * 0.42, y_cap),
                c2: Point2::new(cx - sleeve_half_w * 0.52, y_bicep + cap_h * 0.10),
                end: sl_p4,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Sleeve Cap".into(),
        },
        PolyEdge {
            id: det_id("sl:edge:front_underarm"),
            start: sl_p4,
            segment: CurveSegment::Line { end: sl_p0 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Underarm".into(),
        },
    ];

    let mut sl_loop = ClosedPolyCurve::from_edges(sl_edges);
    sl_loop.ensure_ccw();

    let sleeve = PatternPiece {
        id: sl_id,
        name: "Set-In Sleeve".into(),
        code: "SL-03".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(cx, y_hem + 55.0),
            end: Point2::new(cx, y_cap - 55.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: sl_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![InternalLine {
            id: det_id("sl:line:bicep"),
            label: "Bicep Line".into(),
            pts: vec![sl_p4, sl_p2],
            dashed: true,
        }],
        drill_holes: vec![],
        notches: vec![
            NotchAnchor {
                id: det_id("sl:notch:back_cap"),
                edge_idx: 2,
                param_t: 0.42,
                style: NotchStyle::V,
                count: 2,
                depth_mm: 5.0,
            },
            NotchAnchor {
                id: det_id("sl:notch:cap_apex"),
                edge_idx: 2,
                param_t: 0.96,
                style: NotchStyle::Castle,
                count: 1,
                depth_mm: 5.0,
            },
            NotchAnchor {
                id: det_id("sl:notch:front_cap"),
                edge_idx: 3,
                param_t: 0.58,
                style: NotchStyle::V,
                count: 1,
                depth_mm: 5.0,
            },
        ],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
    };

    let stitching = StitchingGraph {
        links: vec![
            StitchLink {
                id: det_id("stitch:shoulder"),
                label: "Shoulder Seam (Front <-> Back)".into(),
                piece_a: fb_id,
                edge_a: 3,
                dir_a: 1,
                piece_b: bb_id,
                edge_b: 3,
                dir_b: 1,
                ease_target_mm: 0.0,
                ease_min_mm: -2.0,
                ease_max_mm: 2.0,
            },
            StitchLink {
                id: det_id("stitch:side"),
                label: "Side Seam (Front <-> Back)".into(),
                piece_a: fb_id,
                edge_a: 1,
                dir_a: 1,
                piece_b: bb_id,
                edge_b: 1,
                dir_b: 1,
                ease_target_mm: 0.0,
                ease_min_mm: -2.0,
                ease_max_mm: 2.0,
            },
            StitchLink {
                id: det_id("stitch:front_arm"),
                label: "Front Armscye <-> Front Sleeve Cap".into(),
                piece_a: fb_id,
                edge_a: 2,
                dir_a: 1,
                piece_b: sl_id,
                edge_b: 3,
                dir_b: -1,
                ease_target_mm: ease.sleeve_cap_ease_mm * 0.5,
                ease_min_mm: 2.0,
                ease_max_mm: 20.0,
            },
            StitchLink {
                id: det_id("stitch:back_arm"),
                label: "Back Armscye <-> Back Sleeve Cap".into(),
                piece_a: bb_id,
                edge_a: 2,
                dir_a: 1,
                piece_b: sl_id,
                edge_b: 2,
                dir_b: 1,
                ease_target_mm: ease.sleeve_cap_ease_mm * 0.5,
                ease_min_mm: 2.0,
                ease_max_mm: 22.0,
            },
        ],
    };

    (vec![front_bodice, back_bodice, sleeve], stitching)
}

fn build_aline_skirt(
    m: &BodyMeasurements,
    ease: &EaseTable,
    vals: &HashMap<String, f64>,
) -> (Vec<PatternPiece>, StitchingGraph) {
    let q_waist = vals
        .get("skirt_q_waist")
        .copied()
        .unwrap_or((m.waist_mm + ease.waist_ease_mm) / 4.0);
    let q_hip = vals
        .get("skirt_q_hip")
        .copied()
        .unwrap_or((m.hip_mm + ease.hip_ease_mm) / 4.0);
    let flare = vals
        .get("skirt_flare")
        .copied()
        .unwrap_or(((q_hip - q_waist) * 1.45).max(35.0));

    let len = m.skirt_len_mm.max(300.0);
    let hip_drop = m.waist_to_hip_mm.clamp(140.0, len - 80.0);
    let y_hip = len - hip_drop;

    let f_dart_intake = 22.0;
    let b_dart_intake = 28.0;
    let f_waist_w = q_waist + 10.0 + f_dart_intake;
    let b_waist_w = q_waist - 10.0 + b_dart_intake;
    let f_hip_w = q_hip + 10.0;
    let b_hip_w = q_hip - 10.0;
    let f_hem_w = f_hip_w + flare;
    let b_hem_w = b_hip_w + flare;

    let fs_id = det_id("piece:skirt_front");
    let fs_edges = vec![
        PolyEdge {
            id: det_id("fs:hem"),
            start: Point2::new(0.0, 0.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_hem_w * 0.35, 0.0),
                c2: Point2::new(f_hem_w * 0.72, 2.0),
                end: Point2::new(f_hem_w, 8.0),
            },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Front Hem".into(),
        },
        PolyEdge {
            id: det_id("fs:side"),
            start: Point2::new(f_hem_w, 8.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_hip_w + flare * 0.58, y_hip * 0.55),
                c2: Point2::new(f_hip_w, y_hip + hip_drop * 0.48),
                end: Point2::new(f_waist_w, len + 10.0),
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Side Seam".into(),
        },
        PolyEdge {
            id: det_id("fs:waist"),
            start: Point2::new(f_waist_w, len + 10.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_waist_w * 0.65, len + 3.0),
                c2: Point2::new(f_waist_w * 0.32, len),
                end: Point2::new(0.0, len),
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Waist".into(),
        },
        PolyEdge {
            id: det_id("fs:cf"),
            start: Point2::new(0.0, len),
            segment: CurveSegment::Line {
                end: Point2::new(0.0, 0.0),
            },
            sa_width_mm: 0.0,
            corner_rule: CornerRule::Square,
            label: "Center Front (Fold)".into(),
        },
    ];
    let mut fs_loop = ClosedPolyCurve::from_edges(fs_edges);
    fs_loop.ensure_ccw();

    let front_skirt = PatternPiece {
        id: fs_id,
        name: "Front A-Line Skirt".into(),
        code: "FS-01".into(),
        cut_quantity: 1,
        mirror_fold_edge: Some(3),
        grainline: GrainlineSpec {
            start: Point2::new(35.0, 60.0),
            end: Point2::new(35.0, len - 60.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: fs_loop,
        cutouts: vec![],
        darts: vec![DartEntity {
            id: det_id("fs:dart:waist"),
            name: "Front Waist Dart".into(),
            apex: Point2::new(f_waist_w * 0.45, len - 105.0),
            edge_idx: 2,
            leg_a_t: 0.42,
            leg_b_t: 0.58,
            intake_mm: f_dart_intake,
            fold_dir: DartFoldDir::TowardCf,
            cap_style: DartCapStyle::FoldedReflection,
        }],
        internal_lines: vec![InternalLine {
            id: det_id("fs:line:hip"),
            label: "Hip Line".into(),
            pts: vec![Point2::new(0.0, y_hip), Point2::new(f_hip_w, y_hip)],
            dashed: true,
        }],
        drill_holes: vec![DrillHole {
            id: det_id("fs:drill:dart"),
            pos: Point2::new(f_waist_w * 0.45, len - 105.0),
            radius_mm: 2.0,
            label: "Dart Apex".into(),
        }],
        notches: vec![NotchAnchor {
            id: det_id("fs:notch:hip"),
            edge_idx: 1,
            param_t: (y_hip / len).clamp(0.25, 0.85),
            style: NotchStyle::Slit,
            count: 1,
            depth_mm: 5.0,
        }],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
    };

    let bs_id = det_id("piece:skirt_back");
    let x_bs = f_hem_w + 110.0;
    let bs_edges = vec![
        PolyEdge {
            id: det_id("bs:hem"),
            start: Point2::new(x_bs, 0.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(x_bs + b_hem_w * 0.35, 0.0),
                c2: Point2::new(x_bs + b_hem_w * 0.72, 2.0),
                end: Point2::new(x_bs + b_hem_w, 8.0),
            },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Back Hem".into(),
        },
        PolyEdge {
            id: det_id("bs:side"),
            start: Point2::new(x_bs + b_hem_w, 8.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(x_bs + b_hip_w + flare * 0.58, y_hip * 0.55),
                c2: Point2::new(x_bs + b_hip_w, y_hip + hip_drop * 0.48),
                end: Point2::new(x_bs + b_waist_w, len + 10.0),
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Side Seam".into(),
        },
        PolyEdge {
            id: det_id("bs:waist"),
            start: Point2::new(x_bs + b_waist_w, len + 10.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(x_bs + b_waist_w * 0.65, len + 3.0),
                c2: Point2::new(x_bs + b_waist_w * 0.32, len),
                end: Point2::new(x_bs, len),
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Waist".into(),
        },
        PolyEdge {
            id: det_id("bs:cb"),
            start: Point2::new(x_bs, len),
            segment: CurveSegment::Line {
                end: Point2::new(x_bs, 0.0),
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Square,
            label: "Center Back (Zip)".into(),
        },
    ];
    let mut bs_loop = ClosedPolyCurve::from_edges(bs_edges);
    bs_loop.ensure_ccw();

    let back_skirt = PatternPiece {
        id: bs_id,
        name: "Back A-Line Skirt".into(),
        code: "BS-02".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(x_bs + 35.0, 60.0),
            end: Point2::new(x_bs + 35.0, len - 60.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: bs_loop,
        cutouts: vec![],
        darts: vec![DartEntity {
            id: det_id("bs:dart:waist"),
            name: "Back Waist Dart".into(),
            apex: Point2::new(x_bs + b_waist_w * 0.44, len - 130.0),
            edge_idx: 2,
            leg_a_t: 0.40,
            leg_b_t: 0.60,
            intake_mm: b_dart_intake,
            fold_dir: DartFoldDir::TowardCb,
            cap_style: DartCapStyle::FoldedReflection,
        }],
        internal_lines: vec![InternalLine {
            id: det_id("bs:line:hip"),
            label: "Hip Line".into(),
            pts: vec![
                Point2::new(x_bs, y_hip),
                Point2::new(x_bs + b_hip_w, y_hip),
            ],
            dashed: true,
        }],
        drill_holes: vec![DrillHole {
            id: det_id("bs:drill:dart"),
            pos: Point2::new(x_bs + b_waist_w * 0.44, len - 130.0),
            radius_mm: 2.0,
            label: "Dart Apex".into(),
        }],
        notches: vec![NotchAnchor {
            id: det_id("bs:notch:hip"),
            edge_idx: 1,
            param_t: (y_hip / len).clamp(0.25, 0.85),
            style: NotchStyle::Slit,
            count: 2,
            depth_mm: 5.0,
        }],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
    };

    let stitching = StitchingGraph {
        links: vec![StitchLink {
            id: det_id("stitch:skirt_side"),
            label: "Side Seam (Front <-> Back)".into(),
            piece_a: fs_id,
            edge_a: 1,
            dir_a: 1,
            piece_b: bs_id,
            edge_b: 1,
            dir_b: 1,
            ease_target_mm: 0.0,
            ease_min_mm: -2.0,
            ease_max_mm: 2.0,
        }],
    };

    (vec![front_skirt, back_skirt], stitching)
}
