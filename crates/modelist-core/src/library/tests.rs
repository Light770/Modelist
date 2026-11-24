//! Production invariants for the starter library: every style template must draft a valid,
//! sewable, gradeable pattern across the whole size run and under random bodies / style values.

use super::*;
use crate::blocks::{build_default_variables, build_template_pieces};
use crate::curve::edge_total_length;
use crate::grading::compute_cross_size_qa;
use crate::model::{
    polygon_signed_area, BodyMeasurements, EaseTable, PatternProject, SeamDriver,
    SizeTable,
};
use crate::offset::{evaluate_piece, has_self_intersection};
use crate::seam::{interface_total_length, synthesize_partner_notches};
use proptest::prelude::*;
use std::collections::HashSet;

const SIZES: [&str; 6] = ["XS", "S", "M", "L", "XL", "XXL"];

type Build = (
    Vec<PatternPiece>,
    StitchingGraph,
    HashMap<String, f64>,
    HashMap<String, String>,
);

fn build_with(tid: &str, m: &BodyMeasurements, vars: &[ParametricVar]) -> Build {
    build_template_pieces(tid, m, &EaseTable::default(), vars, &[], &[])
}

fn build(tid: &str, size: &str) -> Build {
    build_with(
        tid,
        &BodyMeasurements::from_size_preset(size),
        &build_default_variables(tid),
    )
}

fn style_ids() -> Vec<&'static str> {
    TEMPLATE_CATALOG
        .iter()
        .filter(|t| t.category == TemplateCategory::Style)
        .map(|t| t.id)
        .collect()
}

fn piece<'a>(pieces: &'a [PatternPiece], name: &str) -> &'a PatternPiece {
    pieces
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("piece `{name}` missing"))
}

fn edge_len(p: &PatternPiece, idx: usize) -> f64 {
    edge_total_length(&p.seam_loop.edges[idx])
}

/// Horizontal span of a piece's seamline at height `y` (outermost crossings).
fn span_at_y(p: &PatternPiece, y: f64) -> f64 {
    let pts = p.seam_loop.flatten(0.05);
    let mut xs = Vec::new();
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        if (a.y - y) * (b.y - y) <= 0.0 && (a.y - b.y).abs() > 1e-9 {
            xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
        }
    }
    let lo = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

/// Every invariant a production-ready starter pattern must hold.
fn assert_production_valid(tag: &str, b: &Build) {
    let (pieces, stitching, _, errors) = b;
    assert!(errors.is_empty(), "[{tag}] DAG errors: {errors:?}");
    assert!(!pieces.is_empty() && !stitching.bindings.is_empty(), "[{tag}] empty draft");

    for bd in &stitching.bindings {
        let la = interface_total_length(&bd.interface_a, pieces);
        let lb = interface_total_length(&bd.interface_b, pieces);
        assert!(la > 1.0 && lb > 1.0, "[{tag}] binding `{}` has no length", bd.label);
        if bd.driver == SeamDriver::DriverA {
            let target = bd.ease.target_len_from_driver(la);
            assert!(
                (lb - target).abs() < 0.1,
                "[{tag}] binding `{}` off: la={la:.2} lb={lb:.2} target={target:.2}",
                bd.label
            );
        }
    }

    let mut evals: Vec<_> = pieces
        .iter()
        .map(|p| evaluate_piece(p, &stitching.bindings, pieces))
        .collect();
    synthesize_partner_notches(pieces, &stitching.bindings, &mut evals);
    for ep in &evals {
        let name = &ep.piece.name;
        assert!(
            ep.seam_polyline.iter().all(|p| p.x.is_finite() && p.y.is_finite()),
            "[{tag}] {name}: non-finite seamline"
        );
        assert!(polygon_signed_area(&ep.seam_polyline) > 0.0, "[{tag}] {name}: seam not CCW");
        assert!(polygon_signed_area(&ep.cut_polyline) > 0.0, "[{tag}] {name}: cut not CCW");
        assert!(
            !has_self_intersection(&ep.seam_polyline),
            "[{tag}] {name}: seamline self-intersects"
        );
        assert!(
            !has_self_intersection(&ep.cut_polyline),
            "[{tag}] {name}: cutline self-intersects"
        );
        assert!(ep.cut_area_mm2 > ep.area_mm2, "[{tag}] {name}: cutline inside seamline");
        for n in &ep.piece.notches {
            assert!(
                n.resolve_edge_idx(&ep.piece.seam_loop).is_some(),
                "[{tag}] {name}: notch lost its edge"
            );
        }
        for d in &ep.piece.darts {
            assert!(d.resolve_edge_idx(&ep.piece.seam_loop).is_some(), "[{tag}] {name}: dart lost its edge");
        }
    }
}

#[test]
fn catalog_is_consistent_and_every_entry_drafts() {
    let mut seen = HashSet::new();
    let body_keys = BodyMeasurements::default().to_var_map();
    let ease_keys = EaseTable::default().to_var_map();
    for t in TEMPLATE_CATALOG {
        assert!(seen.insert(t.id), "duplicate template id {}", t.id);
        assert_eq!(resolve_template_id(t.id), t.id);
        for k in t.measurement_keys {
            assert!(body_keys.contains_key(*k), "{}: unknown measurement key {k}", t.id);
        }
        for k in t.ease_keys {
            assert!(ease_keys.contains_key(*k), "{}: unknown ease key {k}", t.id);
        }
        let (pieces, stitching, _, errors) = build(t.id, "M");
        assert!(pieces.len() >= 2, "{} drafts too few pieces", t.id);
        assert!(!stitching.bindings.is_empty(), "{} has no seam bindings", t.id);
        assert!(errors.is_empty(), "{} DAG errors {errors:?}", t.id);
        if t.category == TemplateCategory::Style {
            assert!(library_default_variables(t.id).is_some(), "{} lacks variables", t.id);
        }
    }
    assert_eq!(resolve_template_id("no_such_template"), DEFAULT_TEMPLATE_ID);
    assert!(style_ids().len() >= 3);
}

#[test]
fn starter_styles_valid_across_size_run() {
    for tid in style_ids() {
        for size in SIZES {
            assert_production_valid(&format!("{tid} {size}"), &build(tid, size));
        }
    }
}

/// A fresh project for a template with the full size run visible.
fn project_for(tid: &str) -> PatternProject {
    let mut p = super::new_project(tid);
    p.name = tid.into();
    p.components[0].fabric = Default::default();
    p.components[0].role_fabrics = vec![];
    p.components[0].variables = build_default_variables(tid);
    p.size_table = SizeTable {
        sizes: SIZES.iter().map(|s| s.to_string()).collect(),
        active_size: "M".into(),
        visible_sizes: SIZES.iter().map(|s| s.to_string()).collect(),
        overrides: HashMap::new(),
    };
    p
}

/// Dev tool, not part of the suite: writes one graded SVG (XS-XXL nested on M) per style.
/// `MODELIST_EXPORT_DIR=/some/dir cargo test -p modelist-core export_starter_svgs -- --ignored`
#[test]
#[ignore]
fn export_starter_svgs() {
    let dir = std::env::var("MODELIST_EXPORT_DIR").expect("set MODELIST_EXPORT_DIR");
    for tid in style_ids() {
        let project = project_for(tid);
        let (pieces, stitching, _, _) = build(tid, "M");
        let mut evals: Vec<_> = pieces
            .iter()
            .map(|p| evaluate_piece(p, &stitching.bindings, &pieces))
            .collect();
        synthesize_partner_notches(&pieces, &stitching.bindings, &mut evals);
        let nest = crate::grading::evaluate_graded_nest(&project, &evals, &stitching.bindings);
        let svg = crate::interop::export_svg_mm(tid, "M", &evals, &nest);
        std::fs::write(format!("{dir}/{tid}.svg"), svg).expect("write svg");
    }
}

#[test]
fn starter_styles_cross_size_qa_all_ok() {
    for tid in style_ids() {
        let project = project_for(tid);
        for row in compute_cross_size_qa(&project) {
            assert_eq!(row.cells.len(), SIZES.len(), "{tid}: {} missing sizes", row.label);
            for c in &row.cells {
                assert_eq!(
                    c.status, "ok",
                    "{tid} {}: `{}` ease {:.2} mm out of range",
                    c.size_name, row.label, c.ease_mm
                );
            }
        }
    }
}

#[test]
fn starter_styles_are_deterministic_with_unique_ids() {
    for tid in style_ids() {
        let a = build(tid, "M");
        let b = build(tid, "M");
        assert_eq!(a.0, b.0, "{tid}: pieces differ between identical builds");
        assert_eq!(a.1, b.1, "{tid}: stitching differs between identical builds");
        let mut ids = HashSet::new();
        for p in &a.0 {
            assert!(ids.insert(p.id), "{tid}: duplicate piece id");
            for e in &p.seam_loop.edges {
                assert!(ids.insert(e.id), "{tid}: duplicate edge id in {}", p.name);
            }
            for n in &p.notches {
                assert!(ids.insert(n.id), "{tid}: duplicate notch id in {}", p.name);
            }
        }
    }
}

#[test]
fn boxy_tee_meets_its_spec() {
    let m = BodyMeasurements::from_size_preset("M");
    let (pieces, _, vals, _) = build(boxy_tee::TEMPLATE_ID, "M");
    let front = piece(&pieces, "Front Body");
    let back = piece(&pieces, "Back Body");
    let sleeve = piece(&pieces, "Drop-Shoulder Sleeve");
    let rib = piece(&pieces, "Neck Rib");
    let v = |k: &str| vals[k];

    // Finished chest = body + ease (front and back halves, both cut on fold).
    let half_w = front.seam_loop.edges[2].start.x - front.seam_loop.edges[0].start.x;
    assert!((4.0 * half_w - (m.bust_mm + v("tee_chest_ease"))).abs() < 0.5);
    // Dropped shoulder width.
    let sh_x = front.seam_loop.edges[3].start.x;
    assert!((2.0 * sh_x - (m.shoulder_width_mm + 2.0 * v("tee_shoulder_drop"))).abs() < 0.5);
    // Body length from the high point shoulder.
    let hps_y = front.seam_loop.edges[4].start.y;
    assert!((hps_y - front.seam_loop.edges[0].start.y - v("tee_body_len")).abs() < 0.5);
    // Seams that are sewn together match.
    assert!((edge_len(front, 1) - edge_len(back, 1)).abs() < 0.01, "side seams");
    assert!((edge_len(front, 3) - edge_len(back, 3)).abs() < 0.01, "shoulders");
    assert!((edge_len(sleeve, 1) - edge_len(sleeve, 4)).abs() < 0.01, "sleeve underarms");
    let armhole = edge_len(front, 2) + edge_len(back, 2);
    let cap = edge_len(sleeve, 2) + edge_len(sleeve, 3);
    assert!((cap - armhole - v("tee_cap_ease")).abs() < 0.05, "cap {cap:.2} vs armhole {armhole:.2}");
    // A drop shoulder needs a low cap; a set-in sleeve sits around 130-150 mm.
    let crest_y = sleeve.seam_loop.edges[3].start.y;
    let cap_h = crest_y - sleeve.seam_loop.edges[2].start.y;
    assert!((50.0..=125.0).contains(&cap_h), "cap height {cap_h:.1} mm");
    // Sleeve length is measured from the crest and stays true after the cap solve.
    let sleeve_len = crest_y - sleeve.seam_loop.edges[0].start.y;
    assert!((sleeve_len - v("tee_sleeve_len")).abs() < 0.5);
    // Rib = ratio x half neckline.
    let neck = edge_len(front, 4) + edge_len(back, 4);
    assert!((edge_len(rib, 0) - v("tee_rib_ratio") * neck).abs() < 0.05);
    assert!(front.seam_loop.edges[5].start.y < back.seam_loop.edges[5].start.y, "front neck deeper");
}

#[test]
fn wide_trousers_meet_their_spec() {
    for (tid, shape) in [
        (wide_trouser::WIDE_ID, wide_trouser::LegShape::Wide),
        (wide_trouser::BARREL_ID, wide_trouser::LegShape::Barrel),
    ] {
        let m = BodyMeasurements::from_size_preset("M");
        let d = wide_trouser::shape_defaults(shape);
        let (pieces, stitching, vals, _) = build(tid, "M");
        let front = piece(&pieces, "Front Leg");
        let back = piece(&pieces, "Back Leg");
        let wb = piece(&pieces, "Waistband (Half)");

        // Leg opening (one leg, full circumference) within 3 mm after seam fitting.
        let opening = edge_len(front, 0) + edge_len(back, 0);
        assert!((opening - d.leg_opening).abs() < 3.0, "{tid}: opening {opening:.1}");
        // Waistband = finished half waist; trouser waist = waistband + pleats + dart.
        let wb_seam = edge_len(wb, 1);
        assert!((2.0 * wb_seam - (m.waist_mm + vals["wt_waist_ease"])).abs() < 0.5, "{tid}: waistband");
        let pleat_notches = front
            .notches
            .iter()
            .filter(|n| n.edge_idx == 4)
            .count();
        assert_eq!(pleat_notches, 2 * d.pleat_count as usize, "{tid}: pleat notches");
        let dart = back.darts.first().map(|x| x.intake_mm).unwrap_or(0.0);
        let waist = edge_len(front, 4) + edge_len(back, 4);
        let intake = 2.0 * d.pleat_count * d.pleat_depth + dart;
        assert!((waist - wb_seam - intake).abs() < 0.1, "{tid}: waist intake");
        let wb_binding = stitching
            .bindings
            .iter()
            .find(|b| b.label.starts_with("Waistband"))
            .expect("waistband binding");
        assert!((wb_binding.ease.expected_ease_mm(wb_seam) - intake).abs() < 1e-9);
        // Hip girth at the hip line = body + ease (front + back, both legs) within 2 %.
        let y_hip = m.inseam_mm + 20.0 + m.crotch_depth_mm - m.waist_to_hip_mm;
        let hip = 2.0 * (span_at_y(front, y_hip) + span_at_y(back, y_hip));
        let target = m.hip_mm + d.hip_ease;
        assert!(((hip - target) / target).abs() < 0.02, "{tid}: hip {hip:.0} vs {target:.0}");
        // Leg shape: barrel is fuller at the knee than at the hem; wide tapers gently.
        let y_knee = (m.inseam_mm + 20.0) * 0.5 + 50.0;
        let knee = span_at_y(front, y_knee);
        let hem = edge_len(front, 0);
        match shape {
            wide_trouser::LegShape::Barrel => {
                assert!(knee > hem + 50.0, "{tid}: knee {knee:.0} vs hem {hem:.0}")
            }
            wide_trouser::LegShape::Wide => {
                assert!(knee > hem && knee < hem + 60.0, "{tid}: knee {knee:.0} vs hem {hem:.0}")
            }
        }
    }
}

#[test]
fn starter_styles_grade_monotonically() {
    let mut last_tee_chest = 0.0;
    let mut last_wb = [0.0_f64; 2];
    for size in SIZES {
        let (pieces, _, _, _) = build(boxy_tee::TEMPLATE_ID, size);
        let front = piece(&pieces, "Front Body");
        let chest = front.seam_loop.edges[2].start.x;
        assert!(chest > last_tee_chest, "tee chest must grow at {size}");
        last_tee_chest = chest;
        for (i, tid) in [wide_trouser::WIDE_ID, wide_trouser::BARREL_ID].iter().enumerate() {
            let (pieces, _, _, _) = build(tid, size);
            let wb = edge_len(piece(&pieces, "Waistband (Half)"), 1);
            assert!(wb > last_wb[i], "{tid} waistband must grow at {size}");
            last_wb[i] = wb;
        }
    }
}

fn with_overrides(tid: &str, overrides: &[(&str, f64)]) -> Vec<ParametricVar> {
    let mut vars = build_default_variables(tid);
    for (name, value) in overrides {
        if let Some(v) = vars.iter_mut().find(|v| v.name == *name) {
            v.expr = format!("{value:.3}");
        }
    }
    vars
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Random bodies and style values must still draft valid, matching patterns.
    #[test]
    fn starter_styles_survive_random_bodies_and_styles(
        bust in 760.0..1400.0f64,
        waist_ratio in 0.62..0.98f64,
        hip_extra in 0.0..160.0f64,
        back_waist_len in 360.0..500.0f64,
        shoulder in 330.0..500.0f64,
        armscye in 170.0..300.0f64,
        bicep in 230.0..460.0f64,
        inseam in 600.0..900.0f64,
        crotch_depth in 200.0..360.0f64,
        waist_to_hip in 160.0..260.0f64,
        chest_ease in 40.0..500.0f64,
        drop in 0.0..200.0f64,
        sleeve_len in 120.0..620.0f64,
        opening in 300.0..900.0f64,
        bulge in 0.0..100.0f64,
        pleats in 0u8..=2,
    ) {
        let mut m = BodyMeasurements::from_size_preset("M");
        m.bust_mm = bust;
        m.waist_mm = bust * waist_ratio;
        m.hip_mm = bust + hip_extra;
        m.back_waist_len_mm = back_waist_len;
        m.shoulder_width_mm = shoulder;
        m.armscye_depth_mm = armscye;
        m.bicep_mm = bicep;
        m.inseam_mm = inseam;
        m.crotch_depth_mm = crotch_depth;
        m.waist_to_hip_mm = waist_to_hip;

        let tee_vars = with_overrides(boxy_tee::TEMPLATE_ID, &[
            ("tee_chest_ease", chest_ease),
            ("tee_shoulder_drop", drop),
            ("tee_sleeve_len", sleeve_len),
        ]);
        assert_production_valid("tee/random", &build_with(boxy_tee::TEMPLATE_ID, &m, &tee_vars));

        for tid in [wide_trouser::WIDE_ID, wide_trouser::BARREL_ID] {
            let vars = with_overrides(tid, &[
                ("wt_leg_opening", opening),
                ("wt_knee_bulge", bulge),
                ("wt_pleat_count", pleats as f64),
            ]);
            assert_production_valid(&format!("{tid}/random"), &build_with(tid, &m, &vars));
        }
    }
}
