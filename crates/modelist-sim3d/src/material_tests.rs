//! Material calibration tests: the solver must reproduce textbook fabric mechanics from the
//! physical properties in [`FabricSpec`], not just "look different" per preset.

use crate::avatar::Vec3;
use crate::material::SolverMaterial;
use crate::mesher::mesh_pattern_piece;
use crate::xpbd::{build_tethers, make_hinge, make_tri, ClothCore, SUBSTEPS};
use modelist_core::model::{
    det_id, ArrangementZone, ClosedPolyCurve, CornerRule, CurveSegment, FabricSpec,
    GrainRotationMode, GrainlineSpec, PatternPiece, Point2, PolyEdge,
};

/// Rectangle `w x h` mm with its grainline at `grain_deg` from +Y (0 = warp along Y).
pub(crate) fn rect_piece(w: f64, h: f64, grain_deg: f64) -> PatternPiece {
    let pts = [
        Point2::new(0.0, 0.0),
        Point2::new(w, 0.0),
        Point2::new(w, h),
        Point2::new(0.0, h),
    ];
    let edges = (0..4)
        .map(|i| PolyEdge {
            id: det_id(&format!("test:rect:edge:{i}")),
            start: pts[i],
            segment: CurveSegment::Line { end: pts[(i + 1) % 4] },
            sa_width_mm: 10.0,
            corner_rule: CornerRule::Square,
            label: format!("Edge {i}"),
            hem: None,
            elastic: None,
        })
        .collect();
    let a = grain_deg.to_radians();
    let c = Point2::new(w * 0.5, h * 0.5);
    let d = Point2::new(-a.sin() * 40.0, a.cos() * 40.0);
    PatternPiece {
        id: det_id("test:rect"),
        name: "Test strip".into(),
        code: "T-01".into(),
        cut_quantity: 1,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: c.sub(d),
            end: c.add(d),
            rotation_mode: GrainRotationMode::Free,
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

/// Builds a solver core for a flat piece, mapping 2D mm to 3D through `place`.
pub(crate) fn core_for(
    piece: &PatternPiece,
    fabric: &FabricSpec,
    spacing_mm: f64,
    place: impl Fn(Point2) -> Vec3,
) -> (ClothCore, Vec<Point2>) {
    let pm = mesh_pattern_piece(piece, false, false, spacing_mm).expect("mesh");
    let rest: Vec<Point2> = pm
        .raw_2d_mm
        .iter()
        .map(|p| Point2::new(p.x * 0.001, p.y * 0.001))
        .collect();
    let pos: Vec<Vec3> = pm.raw_2d_mm.iter().map(|p| place(*p)).collect();
    let tris = pm
        .triangles
        .iter()
        .zip(&pm.tri_grain)
        .filter_map(|(t, g)| make_tri(*t, [rest[t[0]], rest[t[1]], rest[t[2]]], *g, 0))
        .collect();
    let g0 = pm.tri_grain[0];
    let hinges = pm
        .hinges
        .iter()
        .filter_map(|h| make_hinge(*h, [rest[h[0]], rest[h[1]], rest[h[2]], rest[h[3]]], g0, 0))
        .collect();
    let mat = SolverMaterial::from_spec(fabric, false);
    let mut core = ClothCore::new(pos, tris, hinges, vec![mat]);
    let mut tethers = Vec::new();
    let area: f64 = pm
        .triangles
        .iter()
        .map(|t| 0.5 * rest[t[1]].sub(rest[t[0]]).perp_dot(rest[t[2]].sub(rest[t[0]])).abs())
        .sum();
    build_tethers(&rest, 0..rest.len(), g0, 0, (area / rest.len() as f64) as f32, &mut tethers);
    core.tethers = tethers;
    core.refresh_materials();
    (core, pm.raw_2d_mm)
}

/// Runs the shipped substep pipeline (minus sewing and contacts) for `frames` 60 Hz frames.
pub(crate) fn run(core: &mut ClothCore, frames: u32) {
    let dt = 1.0 / (60.0 * SUBSTEPS as f32);
    let g = Vec3::new(0.0, -9.81, 0.0);
    for _ in 0..frames {
        for _ in 0..SUBSTEPS {
            core.integrate_damped(dt, g, 1.0);
            core.solve_internal(dt);
        }
    }
}

fn max_strain(core: &ClothCore) -> f32 {
    core.tri_strains()
        .iter()
        .map(|&(u, v)| u.max(v))
        .fold(0.0, f32::max)
}

#[test]
fn woven_strip_hangs_without_stretching() {
    // 600 x 100 mm poplin strip hanging from its top edge: line tension at the top is about
    // 0.7 N/m, so a 3500-6000 N/m fabric must stay well under 1% strain.
    let piece = rect_piece(100.0, 600.0, 0.0);
    let fabric = FabricSpec::from_preset("cotton_poplin");
    let (mut core, raw) = core_for(&piece, &fabric, 20.0, |p| {
        Vec3::new(p.x as f32 * 0.001, 1.0 + p.y as f32 * 0.001, 0.0)
    });
    for (i, p) in raw.iter().enumerate() {
        if p.y > 599.0 {
            core.pinned[i] = true;
        }
    }
    core.refresh_materials();
    run(&mut core, 240);
    let s = max_strain(&core);
    assert!(s < 0.01, "poplin strip stretched {:.2}% under its own weight", s * 100.0);
}

/// Peirce cantilever (Shirley stiffness tester, ASTM D1388): a strip whose overhang equals twice
/// its bending length c = (B / w)^(1/3) droops until the chord to its tip makes 41.5 degrees.
fn cantilever_tip_angle_deg(bend_un_m: f64, density_gsm: f64, grain_deg: f64) -> f64 {
    let piece = rect_piece(150.0, 40.0, grain_deg);
    let mut fabric = FabricSpec::from_preset("cotton_twill");
    fabric.density_gsm = density_gsm;
    fabric.bend_warp_un_m = bend_un_m;
    fabric.bend_weft_un_m = bend_un_m;
    let fabric = fabric.normalized();
    let (mut core, raw) = core_for(&piece, &fabric, 20.0, |p| {
        Vec3::new(p.x as f32 * 0.001, 1.0, p.y as f32 * 0.001)
    });
    for (i, p) in raw.iter().enumerate() {
        if p.x <= 30.0 + 1e-6 {
            core.pinned[i] = true;
        }
    }
    core.refresh_materials();
    run(&mut core, 600);
    let tip: Vec<Vec3> = raw
        .iter()
        .enumerate()
        .filter(|(_, p)| p.x >= 149.0)
        .map(|(i, _)| core.pos[i])
        .collect();
    let n = tip.len() as f32;
    let tx = tip.iter().map(|p| p.x).sum::<f32>() / n;
    let ty = tip.iter().map(|p| p.y).sum::<f32>() / n;
    ((1.0 - ty) as f64).atan2((tx - 0.030) as f64).to_degrees()
}

#[test]
fn bending_rigidity_matches_peirce_cantilever() {
    // Overhang l = 120 mm, so choose B for c = l / 2 = 60 mm at two weights and both grains.
    for (gsm, grain) in [(200.0, 90.0), (120.0, 90.0), (200.0, 0.0)] {
        let w = gsm * 1e-3 * 9.81;
        let b_un_m = 0.06_f64.powi(3) * w * 1e6;
        let ang = cantilever_tip_angle_deg(b_un_m, gsm, grain);
        assert!(
            (ang - 41.5).abs() < 8.0,
            "cantilever at l = 2c drooped to {ang:.1} deg (expected 41.5) for {gsm} gsm, grain {grain}"
        );
    }
}

#[test]
fn stiffer_fabric_droops_less() {
    let soft = cantilever_tip_angle_deg(50.0, 200.0, 90.0);
    let stiff = cantilever_tip_angle_deg(800.0, 200.0, 90.0);
    assert!(soft > stiff + 10.0, "soft {soft:.1} deg vs stiff {stiff:.1} deg");
}


/// Hangs a 50 x 300 mm strip with `load_kg` spread over its bottom edge and returns the mean
/// strain along its length (from the hanging length).
fn loaded_strip_strain(preset: &str, grain_deg: f64, load_kg: f32) -> f32 {
    let piece = rect_piece(50.0, 300.0, grain_deg);
    let fabric = FabricSpec::from_preset(preset);
    let (mut core, raw) = core_for(&piece, &fabric, 12.0, |p| {
        Vec3::new(p.x as f32 * 0.001, 1.0 + p.y as f32 * 0.001, 0.0)
    });
    for (i, p) in raw.iter().enumerate() {
        if p.y > 299.0 {
            core.pinned[i] = true;
        }
    }
    core.refresh_materials();
    let bottom: Vec<usize> = (0..raw.len()).filter(|&i| raw[i].y < 1.0).collect();
    let per = load_kg / bottom.len() as f32;
    for &i in &bottom {
        let m = 1.0 / core.inv_mass[i];
        core.inv_mass[i] = 1.0 / (m + per);
    }
    run(&mut core, 400);
    let top = 1.3_f32;
    let low = bottom.iter().map(|&i| core.pos[i].y).sum::<f32>() / bottom.len() as f32;
    (top - low) / 0.300 - 1.0
}

#[test]
fn knit_stretches_by_its_modulus_and_more_across_the_grain() {
    // 50 g on a 50 mm wide strip: line tension ~9.8 N/m.
    let t = 0.05 * 9.81 / 0.05;
    let jersey = FabricSpec::from_preset("jersey_knit");
    let along_warp = loaded_strip_strain("jersey_knit", 0.0, 0.05);
    let along_weft = loaded_strip_strain("jersey_knit", 90.0, 0.05);
    let woven = loaded_strip_strain("cotton_poplin", 0.0, 0.05);
    let expect_warp = (t / jersey.stretch_warp_n_m) as f32;
    let expect_weft = (t / jersey.stretch_weft_n_m) as f32;
    assert!(along_weft > along_warp * 1.5, "weft {along_weft} vs warp {along_warp}");
    assert!(along_warp > woven * 2.5, "knit {along_warp} vs woven {woven}");
    assert!(
        (along_warp - expect_warp).abs() < expect_warp * 0.6,
        "jersey warp strain {along_warp:.3} vs {expect_warp:.3} predicted"
    );
    assert!(
        (along_weft - expect_weft).abs() < expect_weft * 0.6,
        "jersey weft strain {along_weft:.3} vs {expect_weft:.3} predicted"
    );
    // Wovens never pass their lock strain (2.5% warp for poplin).
    assert!(woven < 0.026, "poplin stretched {woven}");
}
