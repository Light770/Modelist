#[cfg(test)]
mod tests {
    use crate::blocks::{build_default_variables, build_template_pieces};
    use crate::curve::{edge_arc_length_to, edge_total_length, invert_edge_arc_length};
    use crate::expr::evaluate_dag;
    use crate::grading::{compute_cross_size_qa, grade_piece_rtw_proportional};
    use crate::interop::{export_dxf_aama, import_dxf_aama};
    use crate::model::{
        det_id, polygon_signed_area, BodyMeasurements, ClosedPolyCurve, CornerRule, CurveSegment,
        EaseTable, GradeDelta, GradeMode, HingeMode, ParametricVar, PatternPiece, PatternProject,
        Point2, PolyEdge, SizeTable, StackAnchor,
    };
    use crate::offset::{compute_variable_sa_cutline, evaluate_piece, has_self_intersection};
    use crate::ops::{apply_dart_pivot, apply_slash_and_spread, compute_seam_walk_frame};
    use crate::snap::SpatialSnapIndex;
    use proptest::prelude::*;
    use std::collections::HashMap;

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
    fn test_folded_hem_flare_on_sleeve() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (pieces, stitching, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        for p in &pieces {
            let ep = evaluate_piece(p, &stitching.links, &pieces);
            assert!(
                polygon_signed_area(&ep.seam_polyline) > 0.0,
                "{} seamline must be CCW",
                p.name
            );
            assert!(
                polygon_signed_area(&ep.cut_polyline) > 0.0,
                "{} cutline must be CCW",
                p.name
            );
            assert!(
                !has_self_intersection(&ep.cut_polyline),
                "{} cutline must not self-intersect",
                p.name
            );
            assert!(ep.cut_area_mm2 > ep.area_mm2);
        }
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
        let orig_side_len = edge_total_length(&front.seam_loop.edges[1]);
        let orig_arm_len = edge_total_length(&front.seam_loop.edges[2]);

        apply_dart_pivot(&mut front, dart_id, 1, 0.5, 10.0);
        assert_eq!(front.darts[0].edge_idx, 1);
        // Armscye edge was rotated rigidly without distortion
        let new_arm_len = edge_total_length(&front.seam_loop.edges[2]);
        assert!(
            (new_arm_len - orig_arm_len).abs() < 1e-4,
            "Rigidly rotated armscye length changed: {orig_arm_len} -> {new_arm_len}"
        );
        let _ = orig_side_len;

        let mut front2 = build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]).0[0].clone();
        let area_before = front2.seam_loop.signed_area().abs();
        apply_slash_and_spread(&mut front2, 3, 0, 3, 18.0, HingeMode::EdgeHinge);
        let area_after = front2.seam_loop.signed_area().abs();
        assert!(area_after > area_before, "Slash & spread must increase piece fullness/area");
    }

    #[test]
    fn test_seam_walk_and_cross_size_qa() {
        let m = BodyMeasurements::default();
        let e = EaseTable::default();
        let vars = build_default_variables("bodice_sleeve");
        let (pieces, stitching, _, _) =
            build_template_pieces("bodice_sleeve", &m, &e, &vars, &[], &[]);
        let link = &stitching.links[2]; // Front armscye <-> sleeve cap
        let pa = pieces.iter().find(|p| p.id == link.piece_a).unwrap();
        let pb = pieces.iter().find(|p| p.id == link.piece_b).unwrap();
        let frame = compute_seam_walk_frame(link, pa, pb, 0.5).unwrap();
        assert!(frame.walked_a_mm > 0.0 && frame.walked_b_mm > 0.0);

        let proj = PatternProject {
            id: det_id("proj:test"),
            name: "Test".into(),
            template_id: "bodice_sleeve".into(),
            measurements: m,
            ease: e,
            variables: vars,
            size_table: SizeTable::default(),
            grade_mode: GradeMode::ParametricMtm,
            stack_anchor: StackAnchor::GrainlineMidpoint,
            ops: vec![],
            custom_pieces: vec![],
        };
        let qa = compute_cross_size_qa(&proj);
        assert_eq!(qa.len(), 4);
        assert_eq!(qa[0].cells.len(), 6);
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
        let l0 = edge_total_length(&p.seam_loop.edges[1]);
        let l1 = edge_total_length(&graded.seam_loop.edges[1]);
        assert!(l1 > l0);
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
            .map(|p| evaluate_piece(p, &stitching.links, &pieces))
            .collect();
        let dxf = export_dxf_aama(&evals, "M");
        assert!(dxf.contains("AC1009"));
        assert!(dxf.contains("BLOCKS"));
        assert!(!dxf.contains("LWPOLYLINE"));
        assert!(!dxf.contains("SPLINE"));

        let imported = import_dxf_aama(&dxf).expect("DXF re-import must succeed");
        assert_eq!(imported.len(), evals.len());
        for (orig, imp) in evals.iter().zip(imported.iter()) {
            let orig_area = orig.area_mm2;
            let imp_area = imp.seam_loop.signed_area().abs();
            let rel_err = (orig_area - imp_area).abs() / orig_area.max(1.0);
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
            .map(|p| evaluate_piece(p, &stitching.links, &pieces))
            .collect();
        let idx = SpatialSnapIndex::build(&evals);
        let hit = idx.query_snap(0.8, 0.5, 5.0).expect("Should snap to origin vertex");
        assert_eq!(hit.kind, crate::snap::SnapKind::Endpoint);
        assert!(hit.dist_mm < 1.5);
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
}
