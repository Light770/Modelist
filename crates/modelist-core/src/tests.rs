#[cfg(test)]
mod tests {
    use crate::blocks::{build_default_variables, build_template_pieces};
    use crate::curve::{edge_arc_length_to, edge_total_length, invert_edge_arc_length};
    use crate::expr::evaluate_dag;
    use crate::grading::{compute_cross_size_qa, grade_piece_rtw_proportional};
    use crate::interop::{
        export_aama_rul, export_dxf_aama, import_dxf_aama, import_garmentcode_json,
    };
    use crate::model::{
        det_id, polygon_signed_area, ArrangementZone, BodyMeasurements, ClosedPolyCurve,
        CornerRule, CurveSegment, EaseSpec, EaseTable, FabricSpec, GradeDelta,
        HingeMode, ParametricOp, ParametricVar, PatternPiece, PatternProject, Point2, PolyEdge,
        SeamDriver,
    };
    use crate::nesting::{compute_marker_plan, polygons_collide, MarkerConfig, SizeBundleEntry};
    use crate::offset::{compute_variable_sa_cutline, evaluate_piece, has_self_intersection};
    use crate::ops::{apply_dart_pivot, apply_slash_and_spread};
    use crate::seam::{
        auto_detect_seam_bindings, compute_binding_walk_frame, interface_total_length,
        synthesize_partner_notches,
    };
    use crate::snap::SpatialSnapIndex;
    use proptest::prelude::*;
    use std::collections::HashMap;

    /// A one-garment project on the given body, ease and formulas.
    fn test_project(
        tmpl: &str,
        m: BodyMeasurements,
        e: EaseTable,
        vars: Vec<ParametricVar>,
    ) -> PatternProject {
        let mut p = crate::library::new_project(tmpl);
        p.measurements = m;
        p.components[0].ease = e;
        p.components[0].fabric = FabricSpec::default();
        p.components[0].role_fabrics = vec![];
        p.components[0].variables = vars;
        p
    }

    fn square_piece(sa: f64, rule: CornerRule) -> PatternPiece {
        let p0 = Point2::new(0.0, 0.0);
        let p1 = Point2::new(100.0, 0.0);
        let p2 = Point2::new(100.0, 100.0);
        let p3 = Point2::new(0.0, 100.0);
        let edges = vec![
            PolyEdge {
                id: det_id("sq:0"),
                start: p0,
                segment: CurveSegment::Line { end: p1 },
                sa_width_mm: sa,
                corner_rule: rule,
                label: "Bottom".into(),
            },
            PolyEdge {
                id: det_id("sq:1"),
                start: p1,
                segment: CurveSegment::Line { end: p2 },
                sa_width_mm: sa,
                corner_rule: rule,
                label: "Right".into(),
            },
            PolyEdge {
                id: det_id("sq:2"),
                start: p2,
                segment: CurveSegment::Line { end: p3 },
                sa_width_mm: sa,
                corner_rule: rule,
                label: "Top".into(),
            },
            PolyEdge {
                id: det_id("sq:3"),
                start: p3,
                segment: CurveSegment::Line { end: p0 },
                sa_width_mm: sa,
                corner_rule: rule,
                label: "Left".into(),
            },
        ];
        PatternPiece {
            id: det_id("sq:piece"),
            name: "Square".into(),
            code: "SQ".into(),
            cut_quantity: 1,
            mirror_fold_edge: None,
            grainline: crate::model::GrainlineSpec {
                start: Point2::new(50.0, 20.0),
                end: Point2::new(50.0, 80.0),
                rotation_mode: crate::model::GrainRotationMode::TwoWay,
            },
            seam_loop: ClosedPolyCurve::from_edges(edges),
            cutouts: vec![],
            darts: vec![],
            internal_lines: vec![],
            drill_holes: vec![],
            notches: vec![],
            grade_deltas: vec![],
            visible: true,
            fabric_role: "Shell".into(),
            arrangement_zone: ArrangementZone::FlatCustom,
        }
    }

    #[test]
    fn test_square_seam_allowance_exact_area_and_ccw() {
        let sq = square_piece(10.0, CornerRule::Miter { limit: 2.5 });
        let cut = compute_variable_sa_cutline(&sq, &[], std::slice::from_ref(&sq));
        let area = polygon_signed_area(&cut);
        assert!(area > 0.0, "Cutline must be CCW");
        assert!(
            (area - 14_400.0).abs() < 1e-2,
            "100x100 + 10mm SA must equal 14,400 mm^2, got {area}"
        );
        assert!(!has_self_intersection(&cut));
    }

    #[test]
    fn test_all_three_blocks_valid_ccw_and_interdependent_seams() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        for tmpl in ["bodice_sleeve", "aline_skirt", "trouser_block"] {
            let vars = build_default_variables(tmpl);
            let (pieces, stitching, _, _) =
                build_template_pieces(tmpl, &m, &e, &vars, &[], &[]);
            assert!(!pieces.is_empty());
            assert!(!stitching.bindings.is_empty());

            // Check every DriverA binding solved within < 0.1 mm!
            for b in &stitching.bindings {
                if b.driver == SeamDriver::DriverA {
                    let la = interface_total_length(&b.interface_a, &pieces);
                    let lb = interface_total_length(&b.interface_b, &pieces);
                    let target = b.ease.target_len_from_driver(la);
                    assert!(
                        (lb - target).abs() < 0.1,
                        "[{tmpl}] Binding `{}` failed fit-to-partner: la={la:.2}, lb={lb:.2}, target={target:.2}",
                        b.label
                    );
                }
            }

            let mut evals: Vec<_> = pieces
                .iter()
                .map(|p| evaluate_piece(p, &stitching.bindings, &pieces))
                .collect();
            synthesize_partner_notches(&pieces, &stitching.bindings, &mut evals);

            for ep in &evals {
                assert!(polygon_signed_area(&ep.seam_polyline) > 0.0, "{} seam CCW", ep.piece.name);
                assert!(polygon_signed_area(&ep.cut_polyline) > 0.0, "{} cut CCW", ep.piece.name);
                assert!(!has_self_intersection(&ep.cut_polyline), "{} no self-intersections", ep.piece.name);
                assert!(ep.cut_area_mm2 > ep.area_mm2);
            }
        }
    }

    #[test]
    fn test_interdependent_seam_propagates_manual_driver_edit() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let fb_id = det_id("piece:bodice_front");
        // Manually pull Front Bodice shoulder point up by +25 mm (lengthening Front Armscye)
        let ops = vec![ParametricOp::MoveVertex {
            id: det_id("op:test_drag"),
            piece_id: fb_id,
            edge_idx: 3,
            dx: 0.0,
            dy: 25.0,
        }];
        let (pieces, stitching, _, _) = build_template_pieces("bodice_sleeve", &m, &e, &vars, &ops, &[]);
        let arm_binding = stitching
            .bindings
            .iter()
            .find(|b| b.id == det_id("binding:armhole_sleeve"))
            .unwrap();
        let arm_len = interface_total_length(&arm_binding.interface_a, &pieces);
        let cap_len = interface_total_length(&arm_binding.interface_b, &pieces);
        let target = arm_binding.ease.target_len_from_driver(arm_len);
        assert!(
            (cap_len - target).abs() < 0.1,
            "Sleeve cap must adapt automatically after manual armscye drag: cap={cap_len:.2}, target={target:.2}"
        );
    }

    #[test]
    fn test_partner_notch_projection_and_auto_seam_detection() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (pieces, stitching, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        let mut evals: Vec<_> = pieces
            .iter()
            .map(|p| evaluate_piece(p, &stitching.bindings, &pieces))
            .collect();
        synthesize_partner_notches(&pieces, &stitching.bindings, &mut evals);

        let sleeve_eval = evals.iter().find(|e| e.piece.code == "SL-03").unwrap();
        assert!(
            sleeve_eval.projected_notches.iter().any(|n| n.is_auto_partner),
            "Front/Back armscye notches must auto-project onto Set-In Sleeve cap"
        );

        // Auto-seam detection with zero existing bindings should recover matching seams
        let detected = auto_detect_seam_bindings(&pieces, &[]);
        assert!(
            !detected.is_empty(),
            "Berthouzoz-style auto seam detection must find matching cross-piece seams"
        );
    }

    #[test]
    fn test_style_line_split_creates_two_stitched_pieces() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let bb_id = det_id("piece:bodice_back");
        let ops = vec![ParametricOp::SplitStyleLine {
            id: det_id("op:yoke_split"),
            piece_id: bb_id,
            edge_a_idx: 2,
            t_a: 0.45,
            edge_b_idx: 5,
            t_b: 0.25,
            label_a: "Back Yoke".into(),
            label_b: "Lower Back".into(),
        }];
        let (pieces, stitching, _, _) = build_template_pieces("bodice_sleeve", &m, &e, &vars, &ops, &[]);
        assert_eq!(pieces.len(), 4);
        assert!(stitching.bindings.iter().any(|b| b.label.contains("Back Yoke")));
    }

    #[test]
    fn test_dag_topological_determinism_and_cycle_rejection() {
        let mut inputs = HashMap::new();
        inputs.insert("bust".into(), 920.0);
        inputs.insert("ease_bust".into(), 50.0);

        let vars = vec![
            ParametricVar {
                name: "b".into(),
                expr: "a * 2".into(),
                description: String::new(),
            },
            ParametricVar {
                name: "a".into(),
                expr: "(bust + ease_bust) / 4".into(),
                description: String::new(),
            },
        ];
        let res = evaluate_dag(&inputs, &vars, None);
        assert!(res.errors.is_empty());
        assert!((res.values["a"] - 242.5).abs() < 1e-9);
        assert!((res.values["b"] - 485.0).abs() < 1e-9);

        let cyclic = vec![
            ParametricVar {
                name: "x".into(),
                expr: "y + 1".into(),
                description: String::new(),
            },
            ParametricVar {
                name: "y".into(),
                expr: "x + 1".into(),
                description: String::new(),
            },
        ];
        let cyc_res = evaluate_dag(&inputs, &cyclic, None);
        assert_eq!(cyc_res.errors.len(), 2);
    }

    #[test]
    fn test_dart_pivot_and_slash_spread() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (mut pieces, _, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        let mut front = pieces.remove(0);
        let dart_id = front.darts[0].id;
        let orig_arm_len = edge_total_length(&front.seam_loop.edges[2]);

        apply_dart_pivot(&mut front, dart_id, 1, 0.5, 10.0);
        assert_eq!(front.darts[0].edge_idx, 1);
        let new_arm_len = edge_total_length(&front.seam_loop.edges[2]);
        assert!((new_arm_len - orig_arm_len).abs() < 1e-4);

        let mut front2 = build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]).0[0].clone();
        let area_before = front2.seam_loop.signed_area().abs();
        apply_slash_and_spread(&mut front2, 3, 0, 3, 18.0, HingeMode::EdgeHinge);
        assert!(front2.seam_loop.signed_area().abs() > area_before);
    }

    #[test]
    fn test_multi_edge_seam_walk_and_cross_size_qa_and_rul() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (pieces, stitching, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        let arm_binding = &stitching.bindings[2];
        let (frame, ghost_pid) = compute_binding_walk_frame(arm_binding, &pieces, 0.5).unwrap();
        assert_eq!(ghost_pid, det_id("piece:sleeve"));
        assert!(frame.walked_a_mm > 0.0 && frame.walked_b_mm > 0.0);

        let proj = test_project("bodice_sleeve", m, e, vars);
        let qa = compute_cross_size_qa(&proj);
        assert_eq!(qa.len(), 3);
        assert_eq!(qa[0].cells.len(), 6);

        let rul = export_aama_rul(&proj);
        assert!(rul.contains("UNITS: METRIC"));
        assert!(rul.contains("SIZE LIST: XS S M L XL XXL"));
        assert!(rul.contains("RULE: 1"));
    }

    #[test]
    fn test_marker_nesting_non_overlapping_within_roll() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let proj = test_project("bodice_sleeve", m, e, vars);
        let cfg = MarkerConfig {
            roll_width_mm: 1450.0,
            kerf_mm: 4.0,
            cost_per_meter: 15.0,
            bundle: vec![SizeBundleEntry {
                size: "M".into(),
                quantity: 1,
            }],
        };
        let plan = compute_marker_plan(&proj, &cfg);
        assert_eq!(plan.unplaced_count, 0);
        assert_eq!(plan.piece_count, 5); // 1 Front + 2 Back + 2 Sleeve
        assert!(plan.efficiency_pct > 25.0 && plan.efficiency_pct <= 100.0);

        // Verify zero collisions and roll-width bounds
        for (i, a) in plan.placements.iter().enumerate() {
            for p in &a.polygon {
                assert!(p.y >= -0.5 && p.y <= cfg.roll_width_mm + 0.5);
                assert!(p.x >= -0.5 && p.x <= plan.used_length_mm + 0.5);
            }
            for b in &plan.placements[(i + 1)..] {
                assert!(
                    !polygons_collide(&a.polygon, &b.polygon, 0.5),
                    "Marker placements {} and {} overlap!",
                    a.piece_code,
                    b.piece_code
                );
            }
        }
    }

    #[test]
    fn test_garmentcode_json_import() {
        let sample = r#"{
            "pattern": {
                "panels": {
                    "front_panel": {
                        "vertices": [[0, 0], [20, 0], [20, 40], [0, 40]],
                        "edges": [
                            {"endpoints": [0, 1]},
                            {"endpoints": [1, 2], "curvature": [0.5, 0.08]},
                            {"endpoints": [2, 3]},
                            {"endpoints": [3, 0]}
                        ]
                    },
                    "back_panel": {
                        "vertices": [[0, 0], [20, 0], [20, 40], [0, 40]],
                        "edges": [
                            {"endpoints": [0, 1]},
                            {"endpoints": [1, 2]},
                            {"endpoints": [2, 3]},
                            {"endpoints": [3, 0]}
                        ]
                    }
                },
                "stitches": [
                    [{"panel": "front_panel", "edge": 1}, {"panel": "back_panel", "edge": 1}]
                ]
            }
        }"#;
        let (pieces, bindings) = import_garmentcode_json(sample).expect("GarmentCode JSON import");
        assert_eq!(pieces.len(), 2);
        assert_eq!(bindings.len(), 1);
        assert!((pieces[0].seam_loop.signed_area() - 80_000.0).abs() < 5_000.0);
    }

    #[test]
    fn test_rtw_proportional_bezier_grading() {
        let mut p = square_piece(10.0, CornerRule::Miter { limit: 2.5 });
        p.seam_loop.edges[1].segment = CurveSegment::Cubic {
            c1: Point2::new(125.0, 25.0),
            c2: Point2::new(125.0, 75.0),
            end: Point2::new(100.0, 100.0),
        };
        p.grade_deltas.push(GradeDelta {
            edge_idx: 2,
            size_name: "XL".into(),
            dx: 20.0,
            dy: 40.0,
        });
        let graded = grade_piece_rtw_proportional(&p, "XL");
        assert!(edge_total_length(&graded.seam_loop.edges[1]) > edge_total_length(&p.seam_loop.edges[1]));
    }

    #[test]
    fn test_dxf_r12_compliance_and_round_trip() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("aline_skirt");
        let (pieces, stitching, _, _) =
            build_template_pieces("aline_skirt", &m, &e, &vars, &[], &[]);
        let evals: Vec<_> = pieces
            .iter()
            .map(|p| evaluate_piece(p, &stitching.bindings, &pieces))
            .collect();
        let dxf = export_dxf_aama(&evals, "M");
        assert!(dxf.contains("AC1009"));
        assert!(dxf.contains("BLOCKS"));
        assert!(!dxf.contains("LWPOLYLINE"));
        assert!(!dxf.contains("SPLINE"));

        let imported = import_dxf_aama(&dxf).expect("DXF re-import must succeed");
        assert_eq!(imported.len(), evals.len());
        for (orig, imp) in evals.iter().zip(imported.iter()) {
            let rel_err = (orig.area_mm2 - imp.seam_loop.signed_area().abs()).abs() / orig.area_mm2.max(1.0);
            assert!(rel_err < 0.005, "DXF round-trip area error {rel_err}");
            assert_eq!(imp.notches.len(), orig.projected_notches.len());
        }
    }

    #[test]
    fn test_rstar_snap_index() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (pieces, stitching, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        let evals: Vec<_> = pieces
            .iter()
            .map(|p| evaluate_piece(p, &stitching.bindings, &pieces))
            .collect();
        let idx = SpatialSnapIndex::build(&evals);
        let hit = idx.query_snap(0.8, 0.5, 5.0).expect("Should snap to origin vertex");
        assert_eq!(hit.kind, crate::snap::SnapKind::Endpoint);
        assert!(hit.dist_mm < 1.5);
    }

    #[test]
    fn test_ease_spec_modes() {
        assert!((EaseSpec::Exact.target_len_from_driver(200.0) - 200.0).abs() < 1e-6);
        assert!((EaseSpec::EaseMm { mm: 22.0 }.target_len_from_driver(200.0) - 222.0).abs() < 1e-6);
        assert!((EaseSpec::EasePct { pct: 10.0 }.target_len_from_driver(200.0) - 220.0).abs() < 1e-6);
        assert!((EaseSpec::Gather { ratio: 1.5 }.target_len_from_driver(200.0) - 300.0).abs() < 1e-6);
        assert!(
            (EaseSpec::Pleats {
                count: 3,
                depth_mm: 15.0
            }
            .target_len_from_driver(200.0)
                - 290.0)
                .abs()
                < 1e-6
        );
    }

    proptest! {
        #[test]
        fn prop_arc_length_inversion_under_0_01_mm(
            c1x in -80.0_f64..180.0,
            c1y in 10.0_f64..180.0,
            c2x in -80.0_f64..180.0,
            c2y in 10.0_f64..180.0,
            endx in 40.0_f64..240.0,
            endy in -40.0_f64..200.0,
            frac in 0.05_f64..0.95
        ) {
            let edge = PolyEdge {
                id: det_id("prop:cubic"),
                start: Point2::ZERO,
                segment: CurveSegment::Cubic {
                    c1: Point2::new(c1x, c1y),
                    c2: Point2::new(c2x, c2y),
                    end: Point2::new(endx, endy),
                },
                sa_width_mm: 10.0,
                corner_rule: CornerRule::Miter { limit: 2.5 },
                label: "Prop".into(),
            };
            let total = edge_total_length(&edge);
            let s_target = total * frac;
            let t = invert_edge_arc_length(&edge, s_target);
            let s_actual = edge_arc_length_to(&edge, t);
            prop_assert!((s_actual - s_target).abs() < 0.01);
        }

        #[test]
        fn prop_variable_sa_cutline_no_self_intersections(
            sa0 in 0.0_f64..30.0,
            sa1 in 5.0_f64..25.0,
            sa2 in 5.0_f64..25.0,
            sa3 in 0.0_f64..20.0
        ) {
            let mut sq = square_piece(10.0, CornerRule::Miter { limit: 2.5 });
            sq.seam_loop.edges[0].sa_width_mm = sa0;
            sq.seam_loop.edges[1].sa_width_mm = sa1;
            sq.seam_loop.edges[2].sa_width_mm = sa2;
            sq.seam_loop.edges[3].sa_width_mm = sa3;
            let cut = compute_variable_sa_cutline(&sq, &[], std::slice::from_ref(&sq));
            prop_assert!(polygon_signed_area(&cut) > 0.0);
            prop_assert!(!has_self_intersection(&cut));
        }
    }

    #[test]
    fn test_expr_inline_physical_units() {
        let mut inputs = HashMap::new();
        inputs.insert("waist".into(), 740.0);
        let vars = vec![
            ParametricVar {
                name: "mixed_cm_mm".into(),
                expr: "10 cm + 5 mm".into(),
                description: String::new(),
            },
            ParametricVar {
                name: "inches_val".into(),
                expr: "2 in".into(),
                description: String::new(),
            },
            ParametricVar {
                name: "q_waist_ease".into(),
                expr: "(waist + 2 cm) / 4".into(),
                description: String::new(),
            },
        ];
        let res = evaluate_dag(&inputs, &vars, None);
        assert!(res.errors.is_empty(), "Unexpected DAG errors: {:?}", res.errors);
        assert!((res.values["mixed_cm_mm"] - 105.0).abs() < 1e-6);
        assert!((res.values["inches_val"] - 50.8).abs() < 1e-6);
        assert!((res.values["q_waist_ease"] - 190.0).abs() < 1e-6);
    }

    #[test]
    fn test_timeline_rollback_and_suppress_active_ops() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let fb_id = det_id("piece:bodice_front");
        let op1_id = det_id("op:tl:1");
        let op2_id = det_id("op:tl:2");

        let mut proj = test_project("bodice_sleeve", m.clone(), e.clone(), vars.clone());
        proj.components[0].ops = vec![
                ParametricOp::SetSeamAllowance {
                    id: op1_id,
                    piece_id: fb_id,
                    edge_idx: 1,
                    width_mm: 24.0,
                    corner_rule: CornerRule::Miter { limit: 2.5 },
                },
                ParametricOp::SplitStyleLine {
                    id: op2_id,
                    piece_id: det_id("piece:bodice_back"),
                    edge_a_idx: 2,
                    t_a: 0.45,
                    edge_b_idx: 5,
                    t_b: 0.25,
                    label_a: "Yoke".into(),
                    label_b: "Lower".into(),
                },
            ];
        proj.components[0].suppressed_ops = vec![op1_id];
        proj.components[0].rollback_index = Some(1);

        assert_eq!(proj.active_ops().len(), 0);
        proj.comp_mut().suppressed_ops.clear();
        assert_eq!(proj.active_ops().len(), 1);
        let (pieces_rb1, _, _, _) = build_template_pieces(
            &proj.comp().template_id,
            &m,
            &e,
            &vars,
            &proj.active_ops(),
            &proj.comp().custom_pieces,
        );
        assert_eq!(pieces_rb1.len(), 3, "Rolled back before SplitStyleLine -> 3 pieces");
        assert!((pieces_rb1[0].seam_loop.edges[1].sa_width_mm - 24.0).abs() < 1e-6);

        proj.comp_mut().rollback_index = None;
        let (pieces_full, _, _, _) = build_template_pieces(
            &proj.comp().template_id,
            &m,
            &e,
            &vars,
            &proj.active_ops(),
            &proj.comp().custom_pieces,
        );
        assert_eq!(pieces_full.len(), 4, "Rolled to end -> 4 pieces after split");
    }

    #[test]
    fn test_timeline_dependency_reorder_ranges() {
        let fb_id = det_id("piece:bodice_front");
        let bb_id = det_id("piece:bodice_back");
        let sl_id = det_id("piece:sleeve");
        let op0 = ParametricOp::SetSeamAllowance {
            id: det_id("dep:0"),
            piece_id: fb_id,
            edge_idx: 0,
            width_mm: 25.0,
            corner_rule: CornerRule::FoldedHem,
        };
        let op1 = ParametricOp::SplitStyleLine {
            id: det_id("dep:1"),
            piece_id: bb_id,
            edge_a_idx: 1,
            t_a: 0.4,
            edge_b_idx: 4,
            t_b: 0.3,
            label_a: "A".into(),
            label_b: "B".into(),
        };
        let mut op2 = ParametricOp::AddNotch {
            id: det_id("dep:2"),
            piece_id: sl_id,
            edge_idx: 1,
            param_t: 0.5,
            style: crate::model::NotchStyle::V,
            count: 2,
            depth_mm: 5.0,
        };
        let op3 = ParametricOp::AddNotch {
            id: det_id("dep:3"),
            piece_id: bb_id,
            edge_idx: 2,
            param_t: 0.5,
            style: crate::model::NotchStyle::Slit,
            count: 1,
            depth_mm: 4.0,
        };

        let ops = vec![op0, op1, op2.clone(), op3];
        // Op 0 (Front Bodice SA) is independent of SplitStyleLine on Back Bodice & Sleeve notch
        assert_eq!(crate::timeline::compute_reorder_range(&ops, 0), (0, 3));
        // Op 3 shares Back Bodice with SplitStyleLine at idx 1, so cannot move before idx 2
        assert_eq!(crate::timeline::compute_reorder_range(&ops, 3), (2, 3));

        // Patching op2 in place works cleanly
        crate::timeline::patch_op_from_json(
            &mut op2,
            &serde_json::json!({ "param_t": 0.35, "count": 3, "style": "castle" }),
        )
        .expect("patch should succeed");
        match op2 {
            ParametricOp::AddNotch {
                param_t,
                count,
                style,
                ..
            } => {
                assert!((param_t - 0.35).abs() < 1e-6);
                assert_eq!(count, 3);
                assert_eq!(style, crate::model::NotchStyle::Castle);
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod fabric_tests {
    use crate::model::{fabric_bending_length_cm, FabricSpec, FABRIC_PRESET_IDS, FABRIC_PROPERTIES};

    #[test]
    fn v1_fabric_files_migrate_to_physical_properties() {
        // A v1 project stored normalised sliders and a single shrinkage value.
        let v1 = r##"{"id":"jersey_knit","name":"Cotton-Elastane Jersey (190 gsm)","density_gsm":190.0,
            "warp_stiffness":0.52,"weft_stiffness":0.3,"shear_stiffness":0.25,"bend_stiffness":0.1,
            "shrinkage_pct":2.0,"thickness_mm":0.55,"color_hex":"#8e4d42"}"##;
        let f: FabricSpec = serde_json::from_str(v1).unwrap();
        let f = f.normalized();
        let preset = FabricSpec::preset("jersey_knit").unwrap();
        assert_eq!(f.stretch_weft_n_m, preset.stretch_weft_n_m);
        assert_eq!(f.shrinkage_warp_pct, 2.0);
        assert_eq!(f.color_hex, "#8e4d42");
        assert!(f.warp_stiffness.is_none());
        // Unknown v1 fabrics map their sliders onto physical decades.
        let custom = r#"{"id":"my_satin","name":"My satin","density_gsm":120.0,"warp_stiffness":0.9,
            "weft_stiffness":0.9,"shear_stiffness":0.2,"bend_stiffness":0.05,"shrinkage_pct":0.0,
            "thickness_mm":0.2}"#;
        let c: FabricSpec = serde_json::from_str::<FabricSpec>(custom).unwrap().normalized();
        assert!(c.stretch_warp_n_m > 1000.0 && c.bend_warp_un_m < 5.0);
        // Re-serialising never writes the legacy fields.
        assert!(!serde_json::to_string(&c).unwrap().contains("warp_stiffness"));
    }

    #[test]
    fn properties_clamp_and_presets_are_plausible() {
        let mut f = FabricSpec::from_preset("cotton_poplin");
        assert!(f.set_property("stretch_weft_n_m", -5.0));
        assert!(f.stretch_weft_n_m >= 10.0);
        assert!(!f.set_property("bend_warp_un_m", f64::NAN));
        assert!(!f.set_property("nope", 1.0));
        for id in FABRIC_PRESET_IDS {
            let p = FabricSpec::preset(id).unwrap();
            for spec in FABRIC_PROPERTIES {
                let v = p.property(spec.key).unwrap();
                assert!(v >= spec.min && v <= spec.max, "{id}.{} = {v} out of range", spec.key);
            }
            // Bending length (Peirce): silks ~1 cm, shirtings ~2 cm, denim ~3.5 cm.
            let c = fabric_bending_length_cm(p.bend_warp_un_m, p.density_gsm);
            assert!((0.6..5.0).contains(&c), "{id} bending length {c:.2} cm");
        }
        let silk = FabricSpec::preset("silk_charmeuse").unwrap();
        let denim = FabricSpec::preset("denim_12oz").unwrap();
        assert!(
            fabric_bending_length_cm(denim.bend_warp_un_m, denim.density_gsm)
                > 2.5 * fabric_bending_length_cm(silk.bend_warp_un_m, silk.density_gsm)
        );
    }

    #[test]
    fn fabric_values_are_mirrored_into_formula_variables() {
        use crate::blocks::build_default_variables;
        let mut p = crate::library::new_component("boxy_tee", 0, crate::model::Point2::ZERO);
        p.fabric = FabricSpec::from_preset("jersey_knit");
        p.variables = build_default_variables("boxy_tee");
        assert!(p.sync_fabric_variables());
        assert!(!p.sync_fabric_variables(), "second sync is a no-op");
        let stretch = p.variables.iter().find(|v| v.name == "fabric_stretch_weft").unwrap();
        assert_eq!(stretch.expr, "70");
        p.fabric = FabricSpec::from_preset("cotton_poplin");
        assert!(p.sync_fabric_variables());
        let stretch = p.variables.iter().find(|v| v.name == "fabric_stretch_weft").unwrap();
        assert_eq!(stretch.expr, "4");
        // Roles fall back to the shell fabric.
        p.set_role_fabric("Rib", Some(FabricSpec::from_preset("rib_knit")));
        assert_eq!(p.fabric_for_role("Rib").id, "rib_knit");
        assert_eq!(p.fabric_for_role("Shell (interfaced)").id, "cotton_poplin");
        p.set_role_fabric("rib", None);
        assert_eq!(p.fabric_for_role("Rib").id, "cotton_poplin");
    }

    #[test]
    fn remap_entity_ids_is_deterministic_and_keeps_links() {
        use crate::blocks::{build_default_variables, build_template_pieces};
        use crate::model::{det_id, BodyMeasurements, EaseTable};
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("boxy_tee");
        let (mut pieces, stitching, _, _) =
            build_template_pieces("boxy_tee", &m, &e, &vars, &[], &[]);
        let mut bindings = stitching.bindings.clone();
        let orig_pieces = pieces.clone();
        let orig_bindings = bindings.clone();
        let map = crate::model::remap_entity_ids(&mut pieces, &mut bindings, "component:a");

        // Every owned ID changed, deterministically.
        for (o, n) in orig_pieces.iter().zip(&pieces) {
            assert_ne!(o.id, n.id);
            assert_eq!(map[&o.id], n.id);
            assert_eq!(n.id, det_id(&format!("component:a:{}", o.id)));
            for (oe, ne) in o.seam_loop.edges.iter().zip(&n.seam_loop.edges) {
                assert_eq!(map[&oe.id], ne.id);
            }
        }
        // Seams still point at real pieces and edges of the remapped group.
        for b in &bindings {
            for er in b.interface_a.edges.iter().chain(&b.interface_b.edges) {
                let p = pieces.iter().find(|p| p.id == er.piece_id).expect("piece link");
                assert!(p.seam_loop.edge_index_by_id(er.edge_id).is_some(), "edge link");
            }
        }
        assert_eq!(bindings.len(), orig_bindings.len());
        // Notch and dart edge links follow their edges.
        for p in &pieces {
            for n in &p.notches {
                if let Some(eid) = n.edge_id {
                    assert!(p.seam_loop.edge_index_by_id(eid).is_some());
                }
            }
        }
        // Same namespace -> same IDs; another namespace -> disjoint IDs.
        let (mut again, _, _, _) = build_template_pieces("boxy_tee", &m, &e, &vars, &[], &[]);
        crate::model::remap_entity_ids(&mut again, &mut [], "component:a");
        assert_eq!(again[0].id, pieces[0].id);
        let (mut other, _, _, _) = build_template_pieces("boxy_tee", &m, &e, &vars, &[], &[]);
        crate::model::remap_entity_ids(&mut other, &mut [], "component:b");
        assert!(other.iter().all(|o| pieces.iter().all(|p| p.id != o.id)));
    }

    #[test]
    fn version_0_files_become_one_component_with_the_same_ids() {
        use crate::model::{det_id, PatternProject};
        // A file as written before components existed (one garment on the project).
        let mut legacy = serde_json::to_value(crate::library::new_project("boxy_tee")).unwrap();
        let comp = legacy["components"][0].clone();
        let obj = legacy.as_object_mut().unwrap();
        obj.remove("components");
        obj.remove("active_component");
        obj.remove("format_version");
        for key in ["template_id", "ease", "fabric", "role_fabrics", "variables", "ops", "custom_pieces", "suppressed_ops", "rollback_index"] {
            obj.insert(key.into(), comp[key].clone());
        }
        let op_id = det_id("op:legacy:1");
        let front = det_id("boxy_tee:piece:front");
        obj.insert(
            "ops".into(),
            serde_json::json!([{ "op": "set_seam_allowance", "id": op_id, "piece_id": front, "edge_idx": 0, "width_mm": 22.0, "corner_rule": { "kind": "miter", "limit": 2.5 } }]),
        );
        let p = PatternProject::from_json(&legacy.to_string()).expect("v0 loads");
        assert_eq!(p.format_version, crate::model::PROJECT_FORMAT_VERSION);
        assert_eq!(p.components.len(), 1);
        assert_eq!(p.comp().template_id, "boxy_tee");
        assert_eq!(p.comp().ops.len(), 1);
        let draft = crate::blocks::build_project_pieces(&p, &p.measurements);
        let fp = draft.pieces.iter().find(|x| x.id == front).expect("IDs kept without prefix");
        assert!((fp.seam_loop.edges[0].sa_width_mm - 22.0).abs() < 1e-9, "old op still applies");
        // And a version-1 file round-trips.
        let again = PatternProject::from_json(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(again, p);
        // Files from a newer build are refused rather than misread.
        let mut newer = serde_json::to_value(&p).unwrap();
        newer["format_version"] = serde_json::json!(99);
        assert!(PatternProject::from_json(&newer.to_string()).is_err());
    }

    #[test]
    fn the_same_template_can_be_worn_twice() {
        use crate::model::{det_id, CornerRule, ParametricOp, Point2};
        let mut p = crate::library::new_project("boxy_tee");
        let second = crate::library::new_component("boxy_tee", 1, Point2::new(1500.0, 0.0));
        p.components.push(second);
        let draft = crate::blocks::build_project_pieces(&p, &p.measurements);
        let n = draft.pieces.len();
        assert_eq!(draft.piece_component.iter().filter(|&&c| c == 0).count(), n / 2);
        let ids: std::collections::HashSet<_> = draft.pieces.iter().map(|x| x.id).collect();
        assert_eq!(ids.len(), n, "no piece ID shared between the two tees");
        let edges: std::collections::HashSet<_> =
            draft.pieces.iter().flat_map(|x| x.seam_loop.edges.iter().map(|e| e.id)).collect();
        assert_eq!(edges.len(), draft.pieces.iter().map(|x| x.seam_loop.edges.len()).sum::<usize>());
        // The second tee sits 1500 mm to the right and keeps its own seams.
        let x0 = |c: usize| {
            draft.pieces.iter().zip(&draft.piece_component).filter(|(_, &k)| k == c)
                .flat_map(|(x, _)| x.seam_loop.edges.iter().map(|e| e.start.x)).fold(f64::INFINITY, f64::min)
        };
        assert!((x0(1) - x0(0) - 1500.0).abs() < 1e-6);
        for b in &draft.stitching.bindings {
            let owners: std::collections::HashSet<usize> = b.interface_a.edges.iter().chain(&b.interface_b.edges)
                .map(|er| draft.piece_component[draft.pieces.iter().position(|x| x.id == er.piece_id).expect("seam piece exists")])
                .collect();
            assert_eq!(owners.len(), 1, "a seam stays inside its garment");
        }
        // Ops recorded on the second tee's (prefixed, shifted) pieces apply to it only.
        let target = draft.pieces[n / 2].id;
        p.components[1].ops.push(ParametricOp::SetSeamAllowance {
            id: det_id("op:second:1"),
            piece_id: target,
            edge_idx: 0,
            width_mm: 31.0,
            corner_rule: CornerRule::Miter { limit: 2.5 },
        });
        let draft2 = crate::blocks::build_project_pieces(&p, &p.measurements);
        let piece = draft2.pieces.iter().find(|x| x.id == target).unwrap();
        assert!((piece.seam_loop.edges[0].sa_width_mm - 31.0).abs() < 1e-9);
        assert!((draft2.pieces[0].seam_loop.edges[0].sa_width_mm - 31.0).abs() > 1e-6);
        // A hidden garment drafts invisible pieces (not drawn, exported or worn).
        p.components[1].visible = false;
        let draft3 = crate::blocks::build_project_pieces(&p, &p.measurements);
        assert!(draft3.pieces.iter().zip(&draft3.piece_component).all(|(x, &c)| x.visible == (c == 0)));
    }
}
