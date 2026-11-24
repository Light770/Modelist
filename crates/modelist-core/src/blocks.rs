use crate::expr::evaluate_dag;
use crate::model::{
    det_id, ArrangementZone, BodyMeasurements, ClosedPolyCurve, CornerRule, CurveSegment,
    DartCapStyle, DartEntity, DartFoldDir, DrillHole, EaseSpec, EaseTable, EdgeRef,
    GrainRotationMode, GrainlineSpec, InternalLine, NotchAnchor, NotchStyle, ParametricOp,
    ParametricVar, PatternPiece, Point2, PolyEdge, SeamBinding, SeamDriver, SeamInterface,
    SeamType, StitchingGraph,
};
use crate::ops::{apply_ops_to_piece, apply_style_line_split};
use crate::seam::solve_seam_bindings;
use std::collections::HashMap;

pub fn build_default_variables(template_id: &str) -> Vec<ParametricVar> {
    if let Some(vars) = crate::library::library_default_variables(template_id) {
        return vars;
    }
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
        "trouser_block" => vec![
            ParametricVar {
                name: "trouser_q_hip".into(),
                expr: "(hip + ease_hip) / 4".into(),
                description: "Quarter hip with ease".into(),
            },
            ParametricVar {
                name: "trouser_q_waist".into(),
                expr: "(waist + ease_waist) / 4".into(),
                description: "Quarter waist with ease".into(),
            },
            ParametricVar {
                name: "front_fork".into(),
                expr: "hip / 20 + 8".into(),
                description: "Front crotch fork extension".into(),
            },
            ParametricVar {
                name: "back_fork".into(),
                expr: "hip / 10 + 14".into(),
                description: "Back crotch fork extension".into(),
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
    build_template_pieces_in(template_id, "", Point2::new(0.0, 0.0), measurements, ease, vars, ops, custom_pieces)
}

/// Drafts one garment. Drafted pieces (not `custom_pieces`, which are stored with their final
/// IDs and positions) get their IDs moved into the namespace `id_ns` (empty = none) and are
/// shifted by `offset` on the canvas, both before the ops replay, so ops recorded on the
/// displayed pieces find them again.
#[allow(clippy::too_many_arguments)]
pub fn build_template_pieces_in(
    template_id: &str,
    id_ns: &str,
    offset: Point2,
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

    let library_draft =
        crate::library::build_library_template(template_id, measurements, ease, vals);
    let (mut pieces, mut stitching) = match (library_draft, template_id) {
        (Some(draft), _) => draft,
        (None, "aline_skirt") => build_aline_skirt(measurements, ease, vals),
        (None, "trouser_block") => build_trouser_block(measurements, ease, vals),
        (None, _) => build_bodice_and_sleeve(measurements, ease, vals),
    };
    if !id_ns.is_empty() {
        crate::model::remap_entity_ids(&mut pieces, &mut stitching.bindings, id_ns);
    }
    if offset.x != 0.0 || offset.y != 0.0 {
        for p in &mut pieces {
            crate::model::translate_piece(p, offset.x, offset.y);
        }
    }

    for cp in custom_pieces {
        pieces.push(cp.clone());
    }

    // 1. Apply piece-local non-destructive parametric ops
    for piece in &mut pieces {
        apply_ops_to_piece(piece, ops);
    }

    // 2. Apply garment-level ops (style-line splits, seam binding overrides & custom bindings)
    for op in ops {
        match op {
            ParametricOp::SplitStyleLine {
                piece_id,
                edge_a_idx,
                t_a,
                edge_b_idx,
                t_b,
                label_a,
                label_b,
                ..
            } => {
                apply_style_line_split(
                    &mut pieces,
                    &mut stitching,
                    *piece_id,
                    *edge_a_idx,
                    *t_a,
                    *edge_b_idx,
                    *t_b,
                    label_a,
                    label_b,
                );
            }
            ParametricOp::UpdateSeamBinding {
                binding_id,
                driver,
                ease,
                seam_type,
                auto_partner_notches,
                ..
            } => {
                if let Some(b) = stitching.bindings.iter_mut().find(|b| b.id == *binding_id) {
                    b.driver = *driver;
                    b.ease = *ease;
                    b.seam_type = *seam_type;
                    b.auto_partner_notches = *auto_partner_notches;
                }
            }
            ParametricOp::AddSeamBinding { binding, .. } => {
                if !stitching.bindings.iter().any(|b| b.id == binding.id) {
                    stitching.bindings.push(binding.clone());
                }
            }
            ParametricOp::ReplaceSeamBinding { binding, .. } => {
                if let Some(existing) = stitching.bindings.iter_mut().find(|b| b.id == binding.id) {
                    *existing = binding.clone();
                } else {
                    stitching.bindings.push(binding.clone());
                }
            }
            ParametricOp::DeleteSeamBinding { binding_id, .. } => {
                stitching.bindings.retain(|b| b.id != *binding_id);
            }
            _ => {}
        }
    }

    // 3. Solve interdependent SeamBindings (Driver -> Follower arc-length matching!)
    solve_seam_bindings(&mut pieces, &stitching.bindings);

    for p in &mut pieces {
        p.seam_loop.sync_continuity();
        p.seam_loop.ensure_ccw();
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

    let sh_drop_f = 22.0;
    let sh_dx_f = half_sh - neck_w;
    let sh_len = sh_dx_f.hypot(sh_drop_f);
    let sh_drop_b = 20.0;
    let sh_dx_b = (sh_len * sh_len - sh_drop_b * sh_drop_b).max(100.0).sqrt();
    let back_sh_x = neck_w + sh_dx_b;

    let front_side_dx = front_w - front_waist_w;
    let front_side_len = front_side_dx.hypot(side_y);
    let back_side_dx = back_w - back_waist_w;
    let back_side_y = (front_side_len * front_side_len - back_side_dx * back_side_dx)
        .max(90.0 * 90.0)
        .sqrt();

    // --- PIECE 1: FRONT BODICE ---
    let fb_id = det_id("piece:bodice_front");
    let fb_e_waist = det_id("fb:edge:waist");
    let fb_e_side = det_id("fb:edge:side");
    let fb_e_arm = det_id("fb:edge:armscye");
    let fb_e_sh = det_id("fb:edge:shoulder");
    let fb_e_neck = det_id("fb:edge:neckline");
    let fb_e_cf = det_id("fb:edge:cf");

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
            id: fb_e_waist,
            start: fb_p0,
            segment: CurveSegment::Line { end: fb_p1 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Waist Hem".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fb_e_side,
            start: fb_p1,
            segment: CurveSegment::Line { end: fb_p2 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Side Seam".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fb_e_arm,
            start: fb_p2,
            segment: CurveSegment::Cubic {
                c1: fb_arm_c1,
                c2: fb_arm_c2,
                end: fb_p3,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Armscye".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fb_e_sh,
            start: fb_p3,
            segment: CurveSegment::Line { end: fb_p4 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Shoulder".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fb_e_neck,
            start: fb_p4,
            segment: CurveSegment::Cubic {
                c1: fb_neck_c1,
                c2: fb_neck_c2,
                end: fb_p5,
            },
            sa_width_mm: 6.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Neckline".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fb_e_cf,
            start: fb_p5,
            segment: CurveSegment::Line { end: fb_p0 },
            sa_width_mm: 0.0,
            corner_rule: CornerRule::Square,
            label: "Center Front (Fold)".into(),
            hem: None,
            elastic: None,
},
    ];

    let mut fb_loop = ClosedPolyCurve::from_edges(fb_edges);
    fb_loop.ensure_ccw();

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
            edge_id: Some(fb_e_waist),
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
                edge_id: Some(fb_e_arm),
                param_t: 0.35,
                style: NotchStyle::V,
                count: 1,
                depth_mm: 5.0,
            },
            NotchAnchor {
                id: det_id("fb:notch:side"),
                edge_idx: 1,
                edge_id: Some(fb_e_side),
                param_t: 0.5,
                style: NotchStyle::Slit,
                count: 1,
                depth_mm: 5.0,
            },
        ],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::FrontTorso,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    // --- PIECE 2: BACK BODICE ---
    let bb_id = det_id("piece:bodice_back");
    let bb_e_waist = det_id("bb:edge:waist");
    let bb_e_side = det_id("bb:edge:side");
    let bb_e_arm = det_id("bb:edge:armscye");
    let bb_e_sh = det_id("bb:edge:shoulder");
    let bb_e_neck = det_id("bb:edge:neckline");
    let bb_e_cb = det_id("bb:edge:cb");

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
            id: bb_e_waist,
            start: bb_p0,
            segment: CurveSegment::Line { end: bb_p1 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Waist Hem".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bb_e_side,
            start: bb_p1,
            segment: CurveSegment::Line { end: bb_p2 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Side Seam".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bb_e_arm,
            start: bb_p2,
            segment: CurveSegment::Cubic {
                c1: bb_arm_c1,
                c2: bb_arm_c2,
                end: bb_p3,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Armscye".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bb_e_sh,
            start: bb_p3,
            segment: CurveSegment::Line { end: bb_p4 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Shoulder".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bb_e_neck,
            start: bb_p4,
            segment: CurveSegment::Cubic {
                c1: bb_neck_c1,
                c2: bb_neck_c2,
                end: bb_p5,
            },
            sa_width_mm: 6.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Neckline".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bb_e_cb,
            start: bb_p5,
            segment: CurveSegment::Line { end: bb_p0 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Square,
            label: "Center Back (Zip)".into(),
            hem: None,
            elastic: None,
},
    ];

    let mut bb_loop = ClosedPolyCurve::from_edges(bb_edges);
    bb_loop.ensure_ccw();

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
            edge_id: Some(bb_e_waist),
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
        notches: vec![NotchAnchor {
            id: det_id("bb:notch:armscye"),
            edge_idx: 2,
            edge_id: Some(bb_e_arm),
            param_t: 0.35,
            style: NotchStyle::V,
            count: 2,
            depth_mm: 5.0,
        }],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::BackTorso,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    // --- PIECE 3: SET-IN SLEEVE ---
    // Initial cap height is nominal; the general SeamBinding Fit-to-Partner solver tunes it automatically!
    let sl_id = det_id("piece:sleeve");
    let sl_e_hem = det_id("sl:edge:hem");
    let sl_e_bu = det_id("sl:edge:back_underarm");
    let sl_e_bc = det_id("sl:edge:back_cap");
    let sl_e_fc = det_id("sl:edge:front_cap");
    let sl_e_fu = det_id("sl:edge:front_underarm");

    let cx = x_bb + back_w + 115.0 + sleeve_half_w;
    let wrist_half_w = ((m.wrist_mm + 34.0) * 0.5).clamp(80.0, sleeve_half_w - 15.0);
    let y_cap = bodice_len;
    let y_hem = bodice_len - m.sleeve_len_mm.max(250.0);
    let cap_h = (arm_depth * 0.68).max(95.0);
    let y_bicep = y_cap - cap_h;

    let sl_p0 = Point2::new(cx - wrist_half_w, y_hem);
    let sl_p1 = Point2::new(cx + wrist_half_w, y_hem);
    let sl_p2 = Point2::new(cx + sleeve_half_w, y_bicep);
    let sl_p3 = Point2::new(cx, y_cap);
    let sl_p4 = Point2::new(cx - sleeve_half_w, y_bicep);

    let sl_edges = vec![
        PolyEdge {
            id: sl_e_hem,
            start: sl_p0,
            segment: CurveSegment::Line { end: sl_p1 },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Sleeve Hem".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: sl_e_bu,
            start: sl_p1,
            segment: CurveSegment::Line { end: sl_p2 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Underarm".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: sl_e_bc,
            start: sl_p2,
            segment: CurveSegment::Cubic {
                c1: Point2::new(cx + sleeve_half_w * 0.52, y_bicep + cap_h * 0.14),
                c2: Point2::new(cx + sleeve_half_w * 0.42, y_cap),
                end: sl_p3,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Sleeve Cap".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: sl_e_fc,
            start: sl_p3,
            segment: CurveSegment::Cubic {
                c1: Point2::new(cx - sleeve_half_w * 0.42, y_cap),
                c2: Point2::new(cx - sleeve_half_w * 0.52, y_bicep + cap_h * 0.10),
                end: sl_p4,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Sleeve Cap".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: sl_e_fu,
            start: sl_p4,
            segment: CurveSegment::Line { end: sl_p0 },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Underarm".into(),
            hem: None,
            elastic: None,
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
        notches: vec![NotchAnchor {
            id: det_id("sl:notch:cap_apex"),
            edge_idx: 2,
            edge_id: Some(sl_e_bc),
            param_t: 0.96,
            style: NotchStyle::Castle,
            count: 1,
            depth_mm: 5.0,
        }],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::SleeveLeft,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    // GarmentCode-inspired multi-edge SeamInterfaces & interdependent SeamBindings
    let stitching = StitchingGraph {
        bindings: vec![
            SeamBinding {
                id: det_id("binding:shoulder"),
                label: "Shoulder Seam (Front <-> Back)".into(),
                interface_a: SeamInterface {
                    id: det_id("iface:front_shoulder"),
                    name: "Front Shoulder".into(),
                    edges: vec![EdgeRef {
                        piece_id: fb_id,
                        edge_id: fb_e_sh,
                        dir: 1,
                    }],
                },
                interface_b: SeamInterface {
                    id: det_id("iface:back_shoulder"),
                    name: "Back Shoulder".into(),
                    edges: vec![EdgeRef {
                        piece_id: bb_id,
                        edge_id: bb_e_sh,
                        dir: 1,
                    }],
                },
                ease: EaseSpec::Exact,
                driver: SeamDriver::DriverA,
                seam_type: SeamType::Plain,
                ease_min_mm: -2.0,
                ease_max_mm: 2.0,
                auto_partner_notches: true,
            },
            SeamBinding {
                id: det_id("binding:side"),
                label: "Side Seam (Front <-> Back)".into(),
                interface_a: SeamInterface {
                    id: det_id("iface:front_side"),
                    name: "Front Side".into(),
                    edges: vec![EdgeRef {
                        piece_id: fb_id,
                        edge_id: fb_e_side,
                        dir: 1,
                    }],
                },
                interface_b: SeamInterface {
                    id: det_id("iface:back_side"),
                    name: "Back Side".into(),
                    edges: vec![EdgeRef {
                        piece_id: bb_id,
                        edge_id: bb_e_side,
                        dir: 1,
                    }],
                },
                ease: EaseSpec::Exact,
                driver: SeamDriver::DriverA,
                seam_type: SeamType::Plain,
                ease_min_mm: -2.0,
                ease_max_mm: 2.0,
                auto_partner_notches: true,
            },
            SeamBinding {
                id: det_id("binding:armhole_sleeve"),
                label: "Bodice Armhole [F+B] <-> Sleeve Cap".into(),
                interface_a: SeamInterface {
                    id: det_id("iface:bodice_armhole"),
                    name: "Bodice Armhole".into(),
                    edges: vec![
                        EdgeRef {
                            piece_id: fb_id,
                            edge_id: fb_e_arm,
                            dir: 1,
                        },
                        EdgeRef {
                            piece_id: bb_id,
                            edge_id: bb_e_arm,
                            dir: -1,
                        },
                    ],
                },
                interface_b: SeamInterface {
                    id: det_id("iface:sleeve_cap"),
                    name: "Sleeve Cap".into(),
                    edges: vec![
                        EdgeRef {
                            piece_id: sl_id,
                            edge_id: sl_e_fc,
                            dir: -1,
                        },
                        EdgeRef {
                            piece_id: sl_id,
                            edge_id: sl_e_bc,
                            dir: -1,
                        },
                    ],
                },
                ease: EaseSpec::EaseMm {
                    mm: ease.sleeve_cap_ease_mm,
                },
                driver: SeamDriver::DriverA,
                seam_type: SeamType::Plain,
                ease_min_mm: 8.0,
                ease_max_mm: 40.0,
                auto_partner_notches: true,
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
    let fs_e_hem = det_id("fs:hem");
    let fs_e_side = det_id("fs:side");
    let fs_e_waist = det_id("fs:waist");
    let fs_e_cf = det_id("fs:cf");

    let fs_edges = vec![
        PolyEdge {
            id: fs_e_hem,
            start: Point2::new(0.0, 0.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_hem_w * 0.35, 0.0),
                c2: Point2::new(f_hem_w * 0.72, 2.0),
                end: Point2::new(f_hem_w, 8.0),
            },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Front Hem".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fs_e_side,
            start: Point2::new(f_hem_w, 8.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_hip_w + flare * 0.58, y_hip * 0.55),
                c2: Point2::new(f_hip_w, y_hip + hip_drop * 0.48),
                end: Point2::new(f_waist_w, len + 10.0),
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Side Seam".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fs_e_waist,
            start: Point2::new(f_waist_w, len + 10.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_waist_w * 0.65, len + 3.0),
                c2: Point2::new(f_waist_w * 0.32, len),
                end: Point2::new(0.0, len),
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Waist".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: fs_e_cf,
            start: Point2::new(0.0, len),
            segment: CurveSegment::Line {
                end: Point2::new(0.0, 0.0),
            },
            sa_width_mm: 0.0,
            corner_rule: CornerRule::Square,
            label: "Center Front (Fold)".into(),
            hem: None,
            elastic: None,
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
            edge_id: Some(fs_e_waist),
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
            edge_id: Some(fs_e_side),
            param_t: (y_hip / len).clamp(0.25, 0.85),
            style: NotchStyle::Slit,
            count: 1,
            depth_mm: 5.0,
        }],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::FrontSkirt,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    let bs_id = det_id("piece:skirt_back");
    let bs_e_hem = det_id("bs:hem");
    let bs_e_side = det_id("bs:side");
    let bs_e_waist = det_id("bs:waist");
    let bs_e_cb = det_id("bs:cb");
    let x_bs = f_hem_w + 110.0;

    let bs_edges = vec![
        PolyEdge {
            id: bs_e_hem,
            start: Point2::new(x_bs, 0.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(x_bs + b_hem_w * 0.35, 0.0),
                c2: Point2::new(x_bs + b_hem_w * 0.72, 2.0),
                end: Point2::new(x_bs + b_hem_w, 8.0),
            },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Back Hem".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bs_e_side,
            start: Point2::new(x_bs + b_hem_w, 8.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(x_bs + b_hip_w + flare * 0.58, y_hip * 0.55),
                c2: Point2::new(x_bs + b_hip_w, y_hip + hip_drop * 0.48),
                end: Point2::new(x_bs + b_waist_w, len + 10.0),
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Side Seam".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bs_e_waist,
            start: Point2::new(x_bs + b_waist_w, len + 10.0),
            segment: CurveSegment::Cubic {
                c1: Point2::new(x_bs + b_waist_w * 0.65, len + 3.0),
                c2: Point2::new(x_bs + b_waist_w * 0.32, len),
                end: Point2::new(x_bs, len),
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Waist".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bs_e_cb,
            start: Point2::new(x_bs, len),
            segment: CurveSegment::Line {
                end: Point2::new(x_bs, 0.0),
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Square,
            label: "Center Back (Zip)".into(),
            hem: None,
            elastic: None,
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
            edge_id: Some(bs_e_waist),
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
        notches: vec![],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::BackSkirt,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    let stitching = StitchingGraph {
        bindings: vec![SeamBinding {
            id: det_id("binding:skirt_side"),
            label: "Side Seam (Front <-> Back)".into(),
            interface_a: SeamInterface {
                id: det_id("iface:fs_side"),
                name: "Front Side Seam".into(),
                edges: vec![EdgeRef {
                    piece_id: fs_id,
                    edge_id: fs_e_side,
                    dir: 1,
                }],
            },
            interface_b: SeamInterface {
                id: det_id("iface:bs_side"),
                name: "Back Side Seam".into(),
                edges: vec![EdgeRef {
                    piece_id: bs_id,
                    edge_id: bs_e_side,
                    dir: 1,
                }],
            },
            ease: EaseSpec::Exact,
            driver: SeamDriver::DriverA,
            seam_type: SeamType::Plain,
            ease_min_mm: -2.0,
            ease_max_mm: 2.0,
            auto_partner_notches: true,
        }],
    };

    (vec![front_skirt, back_skirt], stitching)
}

/// Tailored Trouser Sloper (Stage 2 complete block library: Bodice, Sleeve, Skirt, Trouser).
fn build_trouser_block(
    m: &BodyMeasurements,
    ease: &EaseTable,
    vals: &HashMap<String, f64>,
) -> (Vec<PatternPiece>, StitchingGraph) {
    let q_hip = vals
        .get("trouser_q_hip")
        .copied()
        .unwrap_or((m.hip_mm + ease.hip_ease_mm) / 4.0);
    let q_waist = vals
        .get("trouser_q_waist")
        .copied()
        .unwrap_or((m.waist_mm + ease.waist_ease_mm) / 4.0);
    let f_fork = vals
        .get("front_fork")
        .copied()
        .unwrap_or(m.hip_mm / 20.0 + 8.0);
    let b_fork = vals
        .get("back_fork")
        .copied()
        .unwrap_or(m.hip_mm / 10.0 + 14.0);

    let inseam = m.inseam_mm.max(550.0);
    let rise = m.crotch_depth_mm.max(200.0);
    let total_len = inseam + rise;
    let y_knee = inseam * 0.54;

    let f_hem_half = ((m.ankle_mm + 45.0) * 0.25 - 5.0).max(52.0);
    let b_hem_half = f_hem_half + 10.0;
    let f_knee_half = ((m.knee_mm + 45.0) * 0.25 - 4.0).max(f_hem_half + 10.0);
    let b_knee_half = f_knee_half + 10.0;

    // --- PIECE 1: FRONT TROUSER ---
    let ft_id = det_id("piece:trouser_front");
    let ft_e_hem = det_id("ft:edge:hem");
    let ft_e_in = det_id("ft:edge:inseam");
    let ft_e_crotch = det_id("ft:edge:crotch");
    let ft_e_waist = det_id("ft:edge:waist");
    let ft_e_out = det_id("ft:edge:outseam");

    let f_crease = q_hip * 0.5 + 15.0;
    let f_dart_intake = 20.0;
    let f_waist_w = q_waist + f_dart_intake - 5.0;
    let f_cf_x = f_crease + q_hip * 0.48;
    let f_crotch_x = f_cf_x + f_fork;
    let f_out_hip_x = f_crease - q_hip * 0.52;
    let f_out_waist_x = (f_cf_x - f_waist_w).max(f_out_hip_x + 5.0);

    let ft_p0 = Point2::new(f_crease - f_hem_half, 0.0);
    let ft_p1 = Point2::new(f_crease + f_hem_half, 0.0);
    let ft_p2 = Point2::new(f_crotch_x, inseam);
    let ft_p3 = Point2::new(f_cf_x - 8.0, total_len - 8.0);
    let ft_p4 = Point2::new(f_out_waist_x, total_len + 6.0);

    let ft_edges = vec![
        PolyEdge {
            id: ft_e_hem,
            start: ft_p0,
            segment: CurveSegment::Line { end: ft_p1 },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Front Hem".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: ft_e_in,
            start: ft_p1,
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_crease + f_knee_half, y_knee * 0.62),
                c2: Point2::new(f_crease + f_knee_half + 10.0, y_knee + (inseam - y_knee) * 0.45),
                end: ft_p2,
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Inseam".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: ft_e_crotch,
            start: ft_p2,
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_cf_x + f_fork * 0.12, inseam + 8.0),
                c2: Point2::new(f_cf_x - 3.0, inseam + rise * 0.45),
                end: ft_p3,
            },
            sa_width_mm: 12.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Crotch J-Curve".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: ft_e_waist,
            start: ft_p3,
            segment: CurveSegment::Cubic {
                c1: ft_p3.lerp(ft_p4, 0.35).add(Point2::new(0.0, -3.0)),
                c2: ft_p3.lerp(ft_p4, 0.72),
                end: ft_p4,
            },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Waist".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: ft_e_out,
            start: ft_p4,
            segment: CurveSegment::Cubic {
                c1: Point2::new(f_out_hip_x - 4.0, inseam + rise * 0.42),
                c2: Point2::new(f_crease - f_knee_half, y_knee),
                end: ft_p0,
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Front Outseam".into(),
            hem: None,
            elastic: None,
},
    ];

    let mut ft_loop = ClosedPolyCurve::from_edges(ft_edges);
    ft_loop.ensure_ccw();

    let front_trouser = PatternPiece {
        id: ft_id,
        name: "Front Trouser".into(),
        code: "FT-01".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(f_crease, 80.0),
            end: Point2::new(f_crease, total_len - 80.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: ft_loop,
        cutouts: vec![],
        darts: vec![DartEntity {
            id: det_id("ft:dart:waist"),
            name: "Front Waist Dart".into(),
            apex: Point2::new(f_crease, total_len - 105.0),
            edge_idx: 3,
            edge_id: Some(ft_e_waist),
            leg_a_t: 0.42,
            leg_b_t: 0.56,
            intake_mm: f_dart_intake,
            fold_dir: DartFoldDir::TowardCf,
            cap_style: DartCapStyle::FoldedReflection,
        }],
        internal_lines: vec![
            InternalLine {
                id: det_id("ft:line:knee"),
                label: "Knee Line".into(),
                pts: vec![
                    Point2::new(f_crease - f_knee_half, y_knee),
                    Point2::new(f_crease + f_knee_half, y_knee),
                ],
                dashed: true,
            },
            InternalLine {
                id: det_id("ft:line:crotch_lvl"),
                label: "Crotch Depth".into(),
                pts: vec![
                    Point2::new(f_out_hip_x, inseam),
                    Point2::new(f_crotch_x, inseam),
                ],
                dashed: true,
            },
        ],
        drill_holes: vec![DrillHole {
            id: det_id("ft:drill:dart"),
            pos: Point2::new(f_crease, total_len - 105.0),
            radius_mm: 2.0,
            label: "Front Dart Tip".into(),
        }],
        notches: vec![
            NotchAnchor {
                id: det_id("ft:notch:knee_in"),
                edge_idx: 1,
                edge_id: Some(ft_e_in),
                param_t: 0.54,
                style: NotchStyle::Slit,
                count: 1,
                depth_mm: 5.0,
            },
            NotchAnchor {
                id: det_id("ft:notch:knee_out"),
                edge_idx: 4,
                edge_id: Some(ft_e_out),
                param_t: 0.58,
                style: NotchStyle::Slit,
                count: 1,
                depth_mm: 5.0,
            },
        ],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::FrontLeg,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    // --- PIECE 2: BACK TROUSER ---
    let bt_id = det_id("piece:trouser_back");
    let bt_e_hem = det_id("bt:edge:hem");
    let bt_e_in = det_id("bt:edge:inseam");
    let bt_e_crotch = det_id("bt:edge:crotch");
    let bt_e_waist = det_id("bt:edge:waist");
    let bt_e_out = det_id("bt:edge:outseam");

    let x_shift = f_crotch_x + 140.0;
    let b_crease = x_shift + q_hip * 0.55;
    let b_dart_intake = 30.0;
    let b_waist_w = q_waist + b_dart_intake + 6.0;
    let b_cb_x = b_crease + q_hip * 0.44;
    let b_crotch_x = b_cb_x + b_fork;
    let b_out_hip_x = b_crease - q_hip * 0.56;
    let b_out_waist_x = b_cb_x - 24.0 - b_waist_w;

    let bt_p0 = Point2::new(b_crease - b_hem_half, 0.0);
    let bt_p1 = Point2::new(b_crease + b_hem_half, 0.0);
    let bt_p2 = Point2::new(b_crotch_x, inseam - 10.0);
    let bt_p3 = Point2::new(b_cb_x - 24.0, total_len + 24.0);
    let bt_p4 = Point2::new(b_out_waist_x, total_len + 6.0);

    let bt_edges = vec![
        PolyEdge {
            id: bt_e_hem,
            start: bt_p0,
            segment: CurveSegment::Line { end: bt_p1 },
            sa_width_mm: 30.0,
            corner_rule: CornerRule::FoldedHem,
            label: "Back Hem".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bt_e_in,
            start: bt_p1,
            segment: CurveSegment::Cubic {
                c1: Point2::new(b_crease + b_knee_half, y_knee * 0.62),
                c2: Point2::new(b_crease + b_knee_half + 16.0, y_knee + (inseam - y_knee) * 0.45),
                end: bt_p2,
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Inseam".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bt_e_crotch,
            start: bt_p2,
            segment: CurveSegment::Cubic {
                c1: Point2::new(b_cb_x + b_fork * 0.08, inseam - 2.0),
                c2: Point2::new(b_cb_x - 10.0, inseam + rise * 0.46),
                end: bt_p3,
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Seat Seam".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bt_e_waist,
            start: bt_p3,
            segment: CurveSegment::Line { end: bt_p4 },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Waist".into(),
            hem: None,
            elastic: None,
},
        PolyEdge {
            id: bt_e_out,
            start: bt_p4,
            segment: CurveSegment::Cubic {
                c1: Point2::new(b_out_hip_x - 6.0, inseam + rise * 0.44),
                c2: Point2::new(b_crease - b_knee_half, y_knee),
                end: bt_p0,
            },
            sa_width_mm: 15.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
            label: "Back Outseam".into(),
            hem: None,
            elastic: None,
},
    ];

    let mut bt_loop = ClosedPolyCurve::from_edges(bt_edges);
    bt_loop.ensure_ccw();

    let back_trouser = PatternPiece {
        id: bt_id,
        name: "Back Trouser".into(),
        code: "BT-02".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(b_crease, 80.0),
            end: Point2::new(b_crease, total_len - 80.0),
            rotation_mode: GrainRotationMode::TwoWay,
        },
        seam_loop: bt_loop,
        cutouts: vec![],
        darts: vec![DartEntity {
            id: det_id("bt:dart:waist"),
            name: "Back Waist Dart".into(),
            apex: Point2::new(b_crease, total_len - 130.0),
            edge_idx: 3,
            edge_id: Some(bt_e_waist),
            leg_a_t: 0.40,
            leg_b_t: 0.58,
            intake_mm: b_dart_intake,
            fold_dir: DartFoldDir::TowardCb,
            cap_style: DartCapStyle::FoldedReflection,
        }],
        internal_lines: vec![InternalLine {
            id: det_id("bt:line:knee"),
            label: "Knee Line".into(),
            pts: vec![
                Point2::new(b_crease - b_knee_half, y_knee),
                Point2::new(b_crease + b_knee_half, y_knee),
            ],
            dashed: true,
        }],
        drill_holes: vec![DrillHole {
            id: det_id("bt:drill:dart"),
            pos: Point2::new(b_crease, total_len - 130.0),
            radius_mm: 2.0,
            label: "Back Dart Tip".into(),
        }],
        notches: vec![],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::BackLeg,
        drawstrings: vec![],
        zippers: vec![],
        buttons: vec![],
        belt_loops: vec![],
    };

    let stitching = StitchingGraph {
        bindings: vec![
            SeamBinding {
                id: det_id("binding:trouser_outseam"),
                label: "Outseam (Front <-> Back)".into(),
                interface_a: SeamInterface {
                    id: det_id("iface:ft_out"),
                    name: "Front Outseam".into(),
                    edges: vec![EdgeRef {
                        piece_id: ft_id,
                        edge_id: ft_e_out,
                        dir: 1,
                    }],
                },
                interface_b: SeamInterface {
                    id: det_id("iface:bt_out"),
                    name: "Back Outseam".into(),
                    edges: vec![EdgeRef {
                        piece_id: bt_id,
                        edge_id: bt_e_out,
                        dir: 1,
                    }],
                },
                ease: EaseSpec::Exact,
                driver: SeamDriver::DriverA,
                seam_type: SeamType::Plain,
                ease_min_mm: -2.0,
                ease_max_mm: 2.0,
                auto_partner_notches: true,
            },
            SeamBinding {
                id: det_id("binding:trouser_inseam"),
                label: "Inseam (Front <-> Back)".into(),
                interface_a: SeamInterface {
                    id: det_id("iface:ft_in"),
                    name: "Front Inseam".into(),
                    edges: vec![EdgeRef {
                        piece_id: ft_id,
                        edge_id: ft_e_in,
                        dir: 1,
                    }],
                },
                interface_b: SeamInterface {
                    id: det_id("iface:bt_in"),
                    name: "Back Inseam".into(),
                    edges: vec![EdgeRef {
                        piece_id: bt_id,
                        edge_id: bt_e_in,
                        dir: 1,
                    }],
                },
                ease: EaseSpec::EaseMm { mm: -5.0 }, // Slight back inseam stretching into front knee
                driver: SeamDriver::DriverA,
                seam_type: SeamType::Plain,
                ease_min_mm: -10.0,
                ease_max_mm: 3.0,
                auto_partner_notches: true,
            },
        ],
    };

    (vec![front_trouser, back_trouser], stitching)
}

/// Every garment of a project drafted on the same body.
pub struct ProjectDraft {
    pub pieces: Vec<PatternPiece>,
    pub stitching: StitchingGraph,
    /// Component index of each piece.
    pub piece_component: Vec<usize>,
    /// Formula values and errors per component.
    pub var_values: Vec<HashMap<String, f64>>,
    pub var_errors: Vec<HashMap<String, String>>,
}

/// Drafts one component on the given body.
pub fn build_component_pieces(
    comp: &crate::model::GarmentComponent,
    measurements: &BodyMeasurements,
) -> (Vec<PatternPiece>, StitchingGraph, HashMap<String, f64>, HashMap<String, String>) {
    build_template_pieces_in(
        &comp.template_id,
        &comp.id_ns,
        comp.offset_2d,
        measurements,
        &comp.ease,
        &comp.variables,
        &comp.active_ops(),
        &comp.custom_pieces,
    )
}

/// Drafts every component of the project on `measurements` (the project body, or a graded
/// size) and merges them. Each component keeps its own pieces, seams and formulas.
pub fn build_project_pieces(
    project: &crate::model::PatternProject,
    measurements: &BodyMeasurements,
) -> ProjectDraft {
    let mut out = ProjectDraft {
        pieces: Vec::new(),
        stitching: StitchingGraph::default(),
        piece_component: Vec::new(),
        var_values: Vec::new(),
        var_errors: Vec::new(),
    };
    for (ci, comp) in project.components.iter().enumerate() {
        let (mut pieces, stitching, vals, errs) = build_component_pieces(comp, measurements);
        if !comp.visible {
            for p in &mut pieces {
                p.visible = false;
            }
        }
        out.piece_component.extend(std::iter::repeat(ci).take(pieces.len()));
        out.pieces.extend(pieces);
        out.stitching.bindings.extend(stitching.bindings);
        out.var_values.push(vals);
        out.var_errors.push(errs);
    }
    out
}
