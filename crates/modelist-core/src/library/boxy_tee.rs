//! Oversized drop-shoulder tee (unisex).
//!
//! Pieces: front and back cut on the fold, a low-cap sleeve (cut 2) and a folded neck rib.
//! Drafted from bust/chest, back waist length, shoulder width, armscye depth and bicep, with
//! every style choice (ease, drop, slope, lengths, rib ratio) exposed as an editable variable.
//!
//! Construction notes
//! - Front and back share the armhole, side seam and shoulder line, so those seams match by
//!   construction; only the necklines differ.
//! - The sleeve cap height is solved at draft time so the cap equals the armhole plus
//!   `tee_cap_ease`. The fit-to-partner solver therefore has nothing to correct, and the
//!   sleeve length stays exactly `tee_sleeve_len`.
//! - The neck rib is drafted to `tee_rib_ratio` of the half neckline (CF to CB) and bound to it
//!   with a percentage ease, so it follows neckline edits.

use super::{binding, cubic, eref, line, notch, param, sclamp, solve_increasing, var, MITER};
use crate::curve::edge_total_length;
use crate::model::{
    det_id, ArrangementZone, BodyMeasurements, ClosedPolyCurve, CornerRule, EaseSpec, EaseTable,
    GrainRotationMode, GrainlineSpec, InternalLine, NotchStyle, ParametricVar, PatternPiece,
    Point2, PolyEdge, SeamType, StitchingGraph,
};
use std::collections::HashMap;
use uuid::Uuid;

pub const TEMPLATE_ID: &str = "boxy_tee";

pub fn default_variables() -> Vec<ParametricVar> {
    vec![
        var("tee_chest_ease", "260", "Chest ease over the body bust/chest (mm)"),
        var(
            "tee_half_width",
            "(bust + tee_chest_ease) / 4",
            "Front and back width at the underarm (mm)",
        ),
        var(
            "tee_body_len",
            "back_waist_len + 290",
            "Body length, high point shoulder to hem (mm)",
        ),
        var(
            "tee_shoulder_drop",
            "90",
            "Shoulder extension past the natural shoulder, per side (mm)",
        ),
        var("tee_shoulder_slope", "12", "Shoulder seam slope (degrees)"),
        var(
            "tee_armhole_depth",
            "armscye_depth + 40",
            "Armhole depth below the dropped shoulder point (mm)",
        ),
        var(
            "tee_neck_width",
            "bust * 0.075 + 28",
            "Half neck width, centre front to side neck point (mm)",
        ),
        var(
            "tee_front_neck_drop",
            "tee_neck_width * 0.9",
            "Front neck depth below the side neck point (mm)",
        ),
        var("tee_back_neck_drop", "22", "Back neck depth below the side neck point (mm)"),
        var("tee_hem_flare", "0", "Side seam flare at the hem, per side (mm)"),
        var("tee_sleeve_len", "230", "Sleeve length from the dropped shoulder point (mm)"),
        var("tee_bicep_ease", "180", "Bicep ease (mm)"),
        var(
            "tee_sleeve_opening",
            "(bicep + tee_bicep_ease) * 0.88",
            "Sleeve opening circumference (mm)",
        ),
        var("tee_cap_ease", "4", "Sleeve cap ease into the armhole (mm)"),
        var("tee_rib_ratio", "0.85", "Neck rib length as a share of the neckline"),
        var("tee_rib_height", "20", "Finished neck rib height (mm)"),
    ]
}

/// Ids of the body edges the stitching graph needs.
struct BodyEdges {
    side: Uuid,
    armhole: Uuid,
    shoulder: Uuid,
    neckline: Uuid,
}

pub fn build(
    m: &BodyMeasurements,
    _ease: &EaseTable,
    vals: &HashMap<String, f64>,
) -> (Vec<PatternPiece>, StitchingGraph) {
    let id = |s: &str| det_id(&format!("{TEMPLATE_ID}:{s}"));

    // --- Style parameters (DAG-evaluated; clamped so any input still drafts a valid pattern) ---
    let chest_ease = sclamp(param(vals, "tee_chest_ease", 260.0), -40.0, 800.0);
    let hw = sclamp(
        param(vals, "tee_half_width", (m.bust_mm + chest_ease) / 4.0),
        150.0,
        900.0,
    );
    let body_len = sclamp(
        param(vals, "tee_body_len", m.back_waist_len_mm + 290.0),
        380.0,
        1300.0,
    );
    let drop = sclamp(param(vals, "tee_shoulder_drop", 90.0), 0.0, 250.0);
    let slope = sclamp(param(vals, "tee_shoulder_slope", 12.0), 0.0, 30.0).to_radians();
    let neck_w = sclamp(
        param(vals, "tee_neck_width", m.bust_mm * 0.075 + 28.0),
        50.0,
        hw * 0.45,
    );
    let sh_x = sclamp(m.shoulder_width_mm * 0.5 + drop, neck_w + 60.0, hw + 90.0);
    let sh_rise = ((sh_x - neck_w) * slope.tan()).min(body_len * 0.25);
    let y_sh = body_len - sh_rise;
    let arm_depth = sclamp(
        param(vals, "tee_armhole_depth", m.armscye_depth_mm + 40.0),
        120.0,
        y_sh - 100.0,
    );
    let y_ua = y_sh - arm_depth;
    let f_neck = sclamp(
        param(vals, "tee_front_neck_drop", neck_w * 0.9),
        25.0,
        (y_sh - y_ua.max(0.0) - 40.0).min(220.0),
    );
    let b_neck = sclamp(param(vals, "tee_back_neck_drop", 22.0), 8.0, f_neck);
    let hem_w = sclamp(
        hw + param(vals, "tee_hem_flare", 0.0),
        hw - 60.0,
        hw + 200.0,
    );

    // Shoulder line direction (shoulder point -> side neck point) and its downward normal.
    // The armhole leaves the shoulder point, and the neckline leaves the side neck point, at
    // right angles to the shoulder seam so front and back join without a kink.
    let sh_dir = Point2::new(neck_w - sh_x, sh_rise).normalized();
    let down = Point2::new(-sh_dir.y, sh_dir.x);
    // Drop-shoulder armholes only scoop a little at the underarm, and not at all once the
    // shoulder point sits outside the chest line.
    let scoop = sclamp(
        0.12 * arm_depth + 0.5 * (hw - sh_x).max(0.0) - 0.4 * (sh_x - hw).max(0.0),
        6.0,
        70.0,
    );

    let body = |tag: &str, x0: f64, neck_drop: f64| -> (PatternPiece, BodyEdges) {
        let is_front = tag == "front";
        let e = |s: &str| id(&format!("{tag}:edge:{s}"));
        let p0 = Point2::new(x0, 0.0);
        let p1 = Point2::new(x0 + hem_w, 0.0);
        let p2 = Point2::new(x0 + hw, y_ua);
        let p3 = Point2::new(x0 + sh_x, y_sh);
        let p4 = Point2::new(x0 + neck_w, body_len);
        let p5 = Point2::new(x0, body_len - neck_drop);

        let edges = vec![
            line(
                e("hem"),
                p0,
                p1,
                25.0,
                CornerRule::FoldedHem,
                if is_front { "Front Hem" } else { "Back Hem" },
            ),
            line(e("side"), p1, p2, 10.0, MITER, "Side Seam"),
            cubic(
                e("armhole"),
                p2,
                Point2::new(p2.x - scoop, p2.y),
                p3.add(down.scale(0.42 * arm_depth)),
                p3,
                10.0,
                MITER,
                if is_front { "Front Armhole" } else { "Back Armhole" },
            ),
            line(
                e("shoulder"),
                p3,
                p4,
                10.0,
                MITER,
                if is_front { "Front Shoulder" } else { "Back Shoulder" },
            ),
            cubic(
                e("neckline"),
                p4,
                p4.add(down.scale(neck_drop * 0.5)),
                Point2::new(x0 + neck_w * 0.42, body_len - neck_drop),
                p5,
                8.0,
                MITER,
                if is_front { "Front Neckline" } else { "Back Neckline" },
            ),
            line(
                e("fold"),
                p5,
                p0,
                0.0,
                CornerRule::Square,
                if is_front { "Center Front (Fold)" } else { "Center Back (Fold)" },
            ),
        ];
        let ids = BodyEdges {
            side: edges[1].id,
            armhole: edges[2].id,
            shoulder: edges[3].id,
            neckline: edges[4].id,
        };
        // Single notch on the front armhole, double on the back (industry convention).
        let notches = vec![notch(
            id(&format!("{tag}:notch:armhole")),
            2,
            &edges[2],
            0.42,
            NotchStyle::Slit,
            if is_front { 1 } else { 2 },
        )];
        let mut seam_loop = ClosedPolyCurve::from_edges(edges);
        seam_loop.ensure_ccw();
        let grain_x = x0 + hw * 0.38;
        let piece = PatternPiece {
            id: id(&format!("piece:{tag}")),
            name: if is_front { "Front Body" } else { "Back Body" }.into(),
            code: if is_front { "TE-F01" } else { "TE-B02" }.into(),
            cut_quantity: 1,
            mirror_fold_edge: Some(5),
            grainline: GrainlineSpec {
                start: Point2::new(grain_x, 60.0),
                end: Point2::new(grain_x, (body_len - neck_drop - 40.0).max(160.0)),
                rotation_mode: GrainRotationMode::OneWay,
            },
            seam_loop,
            cutouts: vec![],
            darts: vec![],
            internal_lines: vec![InternalLine {
                id: id(&format!("{tag}:line:chest")),
                label: "Chest Line".into(),
                pts: vec![Point2::new(x0, y_ua), Point2::new(x0 + hw, y_ua)],
                dashed: true,
            }],
            drill_holes: vec![],
            notches,
            grade_deltas: vec![],
            visible: true,
            fabric_role: "Shell".into(),
            arrangement_zone: if is_front {
                ArrangementZone::FrontTorso
            } else {
                ArrangementZone::BackTorso
            },
        };
        (piece, ids)
    };

    let body_span = hem_w.max(hw).max(sh_x);
    let x_back = body_span + 120.0;
    let (front, fe) = body("front", 0.0, f_neck);
    let (back, be) = body("back", x_back, b_neck);

    // --- Sleeve: low cap solved against the armhole ---
    let armhole_len = edge_total_length(&front.seam_loop.edges[2])
        + edge_total_length(&back.seam_loop.edges[2]);
    let cap_ease = sclamp(param(vals, "tee_cap_ease", 4.0), -10.0, 40.0);
    let cap_target = armhole_len + cap_ease;
    let bicep_ease = sclamp(param(vals, "tee_bicep_ease", 180.0), 20.0, 500.0);
    // A flat cap is already ~2 x bicep_half long, so the bicep is capped below half the target.
    let bh = sclamp((m.bicep_mm + bicep_ease) / 2.0, 90.0, cap_target * 0.46);
    let opening = param(vals, "tee_sleeve_opening", (m.bicep_mm + bicep_ease) * 0.88);
    let oh = sclamp(opening / 2.0, 60.0, bh);
    let sl_len = sclamp(param(vals, "tee_sleeve_len", 230.0), 70.0, 900.0);

    let sl = |s: &str| id(&format!("sleeve:edge:{s}"));
    let cx = x_back + body_span + 140.0 + bh;
    let y_cap = body_len;
    let cap_edges = |h: f64| -> (PolyEdge, PolyEdge) {
        let y_b = y_cap - h;
        let p2 = Point2::new(cx + bh, y_b);
        let p3 = Point2::new(cx, y_cap);
        let p4 = Point2::new(cx - bh, y_b);
        (
            cubic(
                sl("back_cap"),
                p2,
                Point2::new(cx + bh * 0.58, y_b + h * 0.22),
                Point2::new(cx + bh * 0.34, y_cap),
                p3,
                10.0,
                MITER,
                "Back Sleeve Cap",
            ),
            cubic(
                sl("front_cap"),
                p3,
                Point2::new(cx - bh * 0.34, y_cap),
                Point2::new(cx - bh * 0.58, y_b + h * 0.18),
                p4,
                10.0,
                MITER,
                "Front Sleeve Cap",
            ),
        )
    };
    let cap_len = |h: f64| {
        let (b, f) = cap_edges(h);
        edge_total_length(&b) + edge_total_length(&f)
    };
    let cap_h = solve_increasing(cap_len, cap_target, 8.0, sclamp(sl_len - 40.0, 20.0, 320.0));
    let y_bicep = y_cap - cap_h;
    let y_hem = y_cap - sl_len.max(cap_h + 40.0);
    let (back_cap, front_cap) = cap_edges(cap_h);
    let s_p0 = Point2::new(cx - oh, y_hem);
    let s_p1 = Point2::new(cx + oh, y_hem);
    let (bc_start, fc_end) = (back_cap.start, front_cap.end());
    let sleeve_edges = vec![
        line(sl("hem"), s_p0, s_p1, 25.0, CornerRule::FoldedHem, "Sleeve Hem"),
        line(sl("back_underarm"), s_p1, bc_start, 10.0, MITER, "Back Underarm"),
        back_cap,
        front_cap,
        line(sl("front_underarm"), fc_end, s_p0, 10.0, MITER, "Front Underarm"),
    ];
    let (s_bc, s_fc) = (sleeve_edges[2].id, sleeve_edges[3].id);
    let sleeve_notches = vec![notch(
        id("sleeve:notch:crest"),
        2,
        &sleeve_edges[2],
        0.995,
        NotchStyle::Castle,
        1,
    )];
    let mut sleeve_loop = ClosedPolyCurve::from_edges(sleeve_edges);
    sleeve_loop.ensure_ccw();
    let sleeve = PatternPiece {
        id: id("piece:sleeve"),
        name: "Drop-Shoulder Sleeve".into(),
        code: "TE-S03".into(),
        cut_quantity: 2,
        mirror_fold_edge: None,
        grainline: GrainlineSpec {
            start: Point2::new(cx, y_hem + 30.0),
            end: Point2::new(cx, (y_bicep - 10.0).max(y_hem + 60.0)),
            rotation_mode: GrainRotationMode::OneWay,
        },
        seam_loop: sleeve_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![InternalLine {
            id: id("sleeve:line:bicep"),
            label: "Bicep Line".into(),
            pts: vec![Point2::new(cx - bh, y_bicep), Point2::new(cx + bh, y_bicep)],
            dashed: true,
        }],
        drill_holes: vec![],
        notches: sleeve_notches,
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Shell".into(),
        arrangement_zone: ArrangementZone::SleeveLeft,
    };

    // --- Neck rib: half length (CF fold to CB seam), cut double height and folded ---
    let neck_half = edge_total_length(&front.seam_loop.edges[4])
        + edge_total_length(&back.seam_loop.edges[4]);
    let rib_ratio = sclamp(param(vals, "tee_rib_ratio", 0.85), 0.6, 1.0);
    let rib_h = sclamp(param(vals, "tee_rib_height", 20.0), 8.0, 60.0);
    let rib_len = neck_half * rib_ratio;
    let (rx0, ry0) = (0.0, -(2.0 * rib_h) - 140.0);
    let r = |s: &str| id(&format!("rib:edge:{s}"));
    let r_p0 = Point2::new(rx0, ry0);
    let r_p1 = Point2::new(rx0 + rib_len, ry0);
    let r_p2 = Point2::new(rx0 + rib_len, ry0 + 2.0 * rib_h);
    let r_p3 = Point2::new(rx0, ry0 + 2.0 * rib_h);
    let rib_edges = vec![
        line(r("seam"), r_p0, r_p1, 8.0, MITER, "Neck Seam"),
        line(r("cb"), r_p1, r_p2, 8.0, MITER, "Center Back Seam"),
        line(r("inner"), r_p2, r_p3, 8.0, MITER, "Neck Seam (Inner Layer)"),
        line(r("fold"), r_p3, r_p0, 0.0, CornerRule::Square, "Center Front (Fold)"),
    ];
    let r_seam = rib_edges[0].id;
    let mut rib_loop = ClosedPolyCurve::from_edges(rib_edges);
    rib_loop.ensure_ccw();
    let rib = PatternPiece {
        id: id("piece:rib"),
        name: "Neck Rib".into(),
        code: "TE-R04".into(),
        cut_quantity: 1,
        mirror_fold_edge: Some(3),
        grainline: GrainlineSpec {
            start: Point2::new(rx0 + rib_len * 0.5, ry0 + 4.0),
            end: Point2::new(rx0 + rib_len * 0.5, ry0 + 2.0 * rib_h - 4.0),
            rotation_mode: GrainRotationMode::OneWay,
        },
        seam_loop: rib_loop,
        cutouts: vec![],
        darts: vec![],
        internal_lines: vec![InternalLine {
            id: id("rib:line:fold"),
            label: "Fold Line".into(),
            pts: vec![
                Point2::new(rx0, ry0 + rib_h),
                Point2::new(rx0 + rib_len, ry0 + rib_h),
            ],
            dashed: true,
        }],
        drill_holes: vec![],
        notches: vec![],
        grade_deltas: vec![],
        visible: true,
        fabric_role: "Rib".into(),
        arrangement_zone: ArrangementZone::FlatCustom,
    };

    let (fid, bid, sid, rid) = (front.id, back.id, sleeve.id, rib.id);
    let stitching = StitchingGraph {
        bindings: vec![
            binding(
                id("binding:shoulder"),
                "Shoulder Seam (Front <-> Back)",
                (id("iface:front_shoulder"), "Front Shoulder", vec![eref(fid, fe.shoulder, 1)]),
                (id("iface:back_shoulder"), "Back Shoulder", vec![eref(bid, be.shoulder, 1)]),
                EaseSpec::Exact,
                SeamType::Overlock,
                -2.0,
                2.0,
            ),
            binding(
                id("binding:side"),
                "Side Seam (Front <-> Back)",
                (id("iface:front_side"), "Front Side", vec![eref(fid, fe.side, 1)]),
                (id("iface:back_side"), "Back Side", vec![eref(bid, be.side, 1)]),
                EaseSpec::Exact,
                SeamType::Overlock,
                -2.0,
                2.0,
            ),
            binding(
                id("binding:armhole_sleeve"),
                "Body Armhole [F+B] <-> Sleeve Cap",
                (
                    id("iface:body_armhole"),
                    "Body Armhole",
                    vec![eref(fid, fe.armhole, 1), eref(bid, be.armhole, -1)],
                ),
                (
                    id("iface:sleeve_cap"),
                    "Sleeve Cap",
                    vec![eref(sid, s_fc, -1), eref(sid, s_bc, -1)],
                ),
                EaseSpec::EaseMm { mm: cap_ease },
                SeamType::Overlock,
                -5.0,
                15.0,
            ),
            binding(
                id("binding:neck_rib"),
                "Neckline [F+B] <-> Neck Rib",
                (
                    id("iface:neckline"),
                    "Neckline (CF to CB)",
                    vec![eref(fid, fe.neckline, -1), eref(bid, be.neckline, 1)],
                ),
                (id("iface:neck_rib"), "Neck Rib", vec![eref(rid, r_seam, 1)]),
                EaseSpec::EasePct {
                    pct: (rib_ratio - 1.0) * 100.0,
                },
                SeamType::Overlock,
                -(neck_half * 0.4),
                0.0,
            ),
        ],
    };

    (vec![front, back, sleeve, rib], stitching)
}
