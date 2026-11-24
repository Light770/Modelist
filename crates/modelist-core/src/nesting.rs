use crate::blocks::build_template_pieces;
use crate::model::{
    polygon_signed_area, EvaluatedPiece, GrainRotationMode, PatternProject, Point2,
};
use crate::offset::evaluate_piece;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeBundleEntry {
    pub size: String,
    pub quantity: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkerConfig {
    pub roll_width_mm: f64,
    pub kerf_mm: f64,
    pub cost_per_meter: f64,
    pub bundle: Vec<SizeBundleEntry>,
}

impl Default for MarkerConfig {
    fn default() -> Self {
        Self {
            roll_width_mm: 1450.0,
            kerf_mm: 4.0,
            cost_per_meter: 14.0,
            bundle: vec![SizeBundleEntry {
                size: "M".into(),
                quantity: 1,
            }],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkerPlacement {
    pub piece_code: String,
    pub piece_name: String,
    pub size_name: String,
    pub instance_idx: u16,
    pub mirrored: bool,
    pub rotation_deg: f64,
    pub polygon: Vec<Point2>,
    pub grain_start: Point2,
    pub grain_end: Point2,
    pub label_pos: Point2,
    pub area_mm2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkerPlan {
    pub roll_width_mm: f64,
    pub used_length_mm: f64,
    pub total_piece_area_mm2: f64,
    pub efficiency_pct: f64,
    pub waste_pct: f64,
    pub estimated_cost: f64,
    pub piece_count: usize,
    pub unplaced_count: usize,
    pub placements: Vec<MarkerPlacement>,
}

struct NestItem {
    piece_code: String,
    piece_name: String,
    size_name: String,
    instance_idx: u16,
    mirrored: bool,
    base_poly: Vec<Point2>,
    grain_start: Point2,
    grain_end: Point2,
    allowed_angles_deg: Vec<f64>,
    area_mm2: f64,
}

#[derive(Clone)]
struct PlacedPoly {
    pts: Vec<Point2>,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

/// Pure-Rust deterministic 2D irregular marker nesting engine (MIT/Apache-2.0)
/// respecting grainline rotation constraints, L/R mirrored cut pairs, roll width, and kerf padding.
pub fn compute_marker_plan(project: &PatternProject, config: &MarkerConfig) -> MarkerPlan {
    let roll_w = config.roll_width_mm.clamp(400.0, 3200.0);
    let kerf = config.kerf_mm.clamp(0.0, 25.0);

    let mut items: Vec<NestItem> = Vec::new();
    let active_bundle = if config.bundle.is_empty() {
        vec![SizeBundleEntry {
            size: project.size_table.active_size.clone(),
            quantity: 1,
        }]
    } else {
        config.bundle.clone()
    };

    let active_ops = project.active_ops();
    for entry in &active_bundle {
        if entry.quantity == 0 {
            continue;
        }
        let m = project
            .size_table
            .measurements_for(&entry.size, &project.measurements);
        let (pieces, stitching, _, _) = build_template_pieces(
            &project.template_id,
            &m,
            &project.ease,
            &project.variables,
            &active_ops,
            &project.custom_pieces,
        );
        let evals: Vec<EvaluatedPiece> = pieces
            .iter()
            .map(|p| evaluate_piece(p, &stitching.bindings, &pieces))
            .collect();

        for ep in evals {
            if !ep.piece.visible {
                continue;
            }
            let raw = if ep.cut_polyline.len() >= 3 {
                ep.cut_polyline.clone()
            } else {
                ep.seam_polyline.clone()
            };
            if raw.len() < 3 {
                continue;
            }
            let simplified = simplify_polygon(&raw, 1.2);
            let total_copies = (ep.piece.cut_quantity as u32 * entry.quantity as u32).min(24) as u16;
            let angles = allowed_angles(ep.piece.grainline.rotation_mode);

            for copy in 0..total_copies {
                let mirrored = ep.piece.cut_quantity % 2 == 0 && copy % 2 == 1;
                let (mut poly, mut g0, mut g1) = (
                    simplified.clone(),
                    ep.piece.grainline.start,
                    ep.piece.grainline.end,
                );
                if mirrored {
                    for pt in &mut poly {
                        pt.x = -pt.x;
                    }
                    poly.reverse();
                    g0.x = -g0.x;
                    g1.x = -g1.x;
                }
                // Rotate so grainline lies horizontally along the fabric roll warp (+X axis)
                let grain_ang = g1.sub(g0).y.atan2(g1.sub(g0).x);
                let align = -grain_ang;
                let poly_aligned: Vec<Point2> = poly
                    .iter()
                    .map(|p| p.rotate_around(Point2::ZERO, align))
                    .collect();
                let g0_a = g0.rotate_around(Point2::ZERO, align);
                let g1_a = g1.rotate_around(Point2::ZERO, align);
                let area = polygon_signed_area(&poly_aligned).abs();

                items.push(NestItem {
                    piece_code: ep.piece.code.clone(),
                    piece_name: ep.piece.name.clone(),
                    size_name: entry.size.clone(),
                    instance_idx: copy + 1,
                    mirrored,
                    base_poly: poly_aligned,
                    grain_start: g0_a,
                    grain_end: g1_a,
                    allowed_angles_deg: angles.clone(),
                    area_mm2: area,
                });
            }
        }
    }

    // Largest-area-first placement heuristic
    items.sort_by(|a, b| {
        b.area_mm2
            .partial_cmp(&a.area_mm2)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut placed: Vec<PlacedPoly> = Vec::new();
    let mut placements: Vec<MarkerPlacement> = Vec::new();
    let mut used_len = 0.0_f64;
    let mut placed_area = 0.0_f64;
    let mut unplaced_count = 0usize;

    for item in items {
        let mut best: Option<(f64, f64, f64, Vec<Point2>, Point2, Point2, PlacedPoly)> = None;

        for &ang_deg in &item.allowed_angles_deg {
            let rad = ang_deg.to_radians();
            let rot_pts: Vec<Point2> = item
                .base_poly
                .iter()
                .map(|p| p.rotate_around(Point2::ZERO, rad))
                .collect();
            let (min_x, min_y, max_x, max_y) = bbox(&rot_pts);
            let pw = max_x - min_x;
            let ph = max_y - min_y;
            if ph + kerf > roll_w {
                continue;
            }
            // Shift normalized so local bbox starts at (0, 0)
            let norm_pts: Vec<Point2> = rot_pts
                .iter()
                .map(|p| Point2::new(p.x - min_x, p.y - min_y))
                .collect();
            let g0_n = item
                .grain_start
                .rotate_around(Point2::ZERO, rad)
                .sub(Point2::new(min_x, min_y));
            let g1_n = item
                .grain_end
                .rotate_around(Point2::ZERO, rad)
                .sub(Point2::new(min_x, min_y));

            // Generate candidate Y tracks along fabric roll width
            let mut y_cands = vec![kerf * 0.5, (roll_w - ph - kerf * 0.5).max(kerf * 0.5)];
            for pl in &placed {
                let y_above = pl.max_y + kerf;
                if y_above + ph <= roll_w {
                    y_cands.push(y_above);
                }
                let y_below = pl.min_y - ph - kerf;
                if y_below >= 0.0 {
                    y_cands.push(y_below);
                }
            }
            let y_steps = 12;
            for k in 0..=y_steps {
                let y = (roll_w - ph) * (k as f64) / (y_steps as f64);
                y_cands.push(y.max(0.0));
            }
            y_cands.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            y_cands.dedup_by(|a, b| (*a - *b).abs() < 4.0);

            for y in y_cands {
                if y < -1e-4 || y + ph > roll_w + 1e-4 {
                    continue;
                }
                // Find smallest collision-free X via stepped scan + binary compaction
                let x_max_bound = used_len + pw + kerf + 20.0;
                let step_x = (pw * 0.12).clamp(12.0, 45.0);
                let mut found_x = None;
                let mut x = kerf * 0.5;
                while x <= x_max_bound {
                    let cand_poly = translate_placed(&norm_pts, x, y, pw, ph);
                    if !collides_any(&cand_poly, &placed, kerf) {
                        // Compact leftward with binary search for tight nesting
                        let mut hi = x;
                        let mut lo = (x - step_x).max(kerf * 0.5);
                        for _ in 0..7 {
                            let mid = 0.5 * (lo + hi);
                            let mid_poly = translate_placed(&norm_pts, mid, y, pw, ph);
                            if !collides_any(&mid_poly, &placed, kerf) {
                                hi = mid;
                            } else {
                                lo = mid;
                            }
                        }
                        found_x = Some(hi);
                        break;
                    }
                    x += step_x;
                }

                if let Some(px) = found_x {
                    let right_edge = px + pw;
                    let score = right_edge + y * 0.015;
                    if best.as_ref().map(|b| score < b.0).unwrap_or(true) {
                        let final_poly = translate_placed(&norm_pts, px, y, pw, ph);
                        let g0_f = g0_n.add(Point2::new(px, y));
                        let g1_f = g1_n.add(Point2::new(px, y));
                        best = Some((
                            score,
                            right_edge,
                            ang_deg,
                            final_poly.pts.clone(),
                            g0_f,
                            g1_f,
                            final_poly,
                        ));
                    }
                }
            }
        }

        if let Some((_, right_edge, ang_deg, pts, g0, g1, pl)) = best {
            used_len = used_len.max(right_edge + kerf * 0.5);
            placed_area += item.area_mm2;
            let lbl = g0.lerp(g1, 0.5);
            placed.push(pl);
            placements.push(MarkerPlacement {
                piece_code: item.piece_code,
                piece_name: item.piece_name,
                size_name: item.size_name,
                instance_idx: item.instance_idx,
                mirrored: item.mirrored,
                rotation_deg: ang_deg,
                polygon: pts,
                grain_start: g0,
                grain_end: g1,
                label_pos: lbl,
                area_mm2: item.area_mm2,
            });
        } else {
            unplaced_count += 1;
        }
    }

    let roll_area = (used_len * roll_w).max(1.0);
    let efficiency_pct = ((placed_area / roll_area) * 100.0).clamp(0.0, 100.0);
    let waste_pct = (100.0 - efficiency_pct).max(0.0);
    let estimated_cost = (used_len / 1000.0) * config.cost_per_meter.max(0.0);

    MarkerPlan {
        roll_width_mm: roll_w,
        used_length_mm: used_len,
        total_piece_area_mm2: placed_area,
        efficiency_pct,
        waste_pct,
        estimated_cost,
        piece_count: placements.len(),
        unplaced_count,
        placements,
    }
}

pub fn export_marker_svg(plan: &MarkerPlan, title: &str) -> String {
    let w = plan.used_length_mm.max(200.0) + 40.0;
    let h = plan.roll_width_mm + 40.0;
    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.1}mm\" height=\"{h:.1}mm\" viewBox=\"0 0 {w:.1} {h:.1}\">\n  <title>{title} Marker</title>\n"
    );
    svg.push_str(&format!(
        "  <rect x=\"20\" y=\"20\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#f8fafc\" stroke=\"#0f172a\" stroke-width=\"1.2\" stroke-dasharray=\"8 4\" />\n",
        plan.used_length_mm, plan.roll_width_mm
    ));
    for pl in &plan.placements {
        let mut d = String::new();
        for (i, p) in pl.polygon.iter().enumerate() {
            let x = p.x + 20.0;
            let y = plan.roll_width_mm - p.y + 20.0;
            if i == 0 {
                d.push_str(&format!("M {x:.2} {y:.2}"));
            } else {
                d.push_str(&format!(" L {x:.2} {y:.2}"));
            }
        }
        d.push_str(" Z");
        svg.push_str(&format!(
            "  <path d=\"{d}\" fill=\"rgba(37,99,235,0.09)\" stroke=\"#1e293b\" stroke-width=\"1.0\" />\n"
        ));
        let lx = pl.label_pos.x + 20.0;
        let ly = plan.roll_width_mm - pl.label_pos.y + 20.0;
        svg.push_str(&format!(
            "  <text x=\"{lx:.1}\" y=\"{ly:.1}\" font-family=\"sans-serif\" font-size=\"14\" text-anchor=\"middle\" fill=\"#0f172a\">{} [{}]</text>\n",
            pl.piece_code, pl.size_name
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

fn allowed_angles(mode: GrainRotationMode) -> Vec<f64> {
    match mode {
        GrainRotationMode::OneWay => vec![0.0],
        GrainRotationMode::TwoWay => vec![0.0, 180.0],
        GrainRotationMode::Bias => vec![45.0, -45.0, 135.0, -135.0],
        GrainRotationMode::Free => vec![0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0],
    }
}

fn simplify_polygon(pts: &[Point2], min_seg_mm: f64) -> Vec<Point2> {
    if pts.len() <= 16 {
        return pts.to_vec();
    }
    let mut out = vec![pts[0]];
    for &p in &pts[1..] {
        if out.last().unwrap().dist(p) >= min_seg_mm {
            out.push(p);
        }
    }
    if out.len() > 3 && out.first().unwrap().dist(*out.last().unwrap()) < min_seg_mm {
        out.pop();
    }
    out
}

fn bbox(pts: &[Point2]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in pts {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    (min_x, min_y, max_x, max_y)
}

fn translate_placed(norm_pts: &[Point2], dx: f64, dy: f64, pw: f64, ph: f64) -> PlacedPoly {
    PlacedPoly {
        pts: norm_pts
            .iter()
            .map(|p| Point2::new(p.x + dx, p.y + dy))
            .collect(),
        min_x: dx,
        min_y: dy,
        max_x: dx + pw,
        max_y: dy + ph,
    }
}

fn collides_any(cand: &PlacedPoly, others: &[PlacedPoly], kerf: f64) -> bool {
    for other in others {
        if cand.max_x + kerf <= other.min_x
            || other.max_x + kerf <= cand.min_x
            || cand.max_y + kerf <= other.min_y
            || other.max_y + kerf <= cand.min_y
        {
            continue;
        }
        if polygons_collide(&cand.pts, &other.pts, kerf) {
            return true;
        }
    }
    false
}

pub fn polygons_collide(a: &[Point2], b: &[Point2], kerf: f64) -> bool {
    let kerf_sq = (kerf * 0.5).max(0.25).powi(2);
    let na = a.len();
    let nb = b.len();

    // Check segment-segment intersections or near-kerf distance
    for i in 0..na {
        let a0 = a[i];
        let a1 = a[(i + 1) % na];
        for j in 0..nb {
            let b0 = b[j];
            let b1 = b[(j + 1) % nb];
            if segs_intersect_or_close(a0, a1, b0, b1, kerf_sq) {
                return true;
            }
        }
    }
    // Check containment
    point_in_poly(a[0], b) || point_in_poly(b[0], a)
}

fn segs_intersect_or_close(a0: Point2, a1: Point2, b0: Point2, b1: Point2, tol_sq: f64) -> bool {
    let da = a1.sub(a0);
    let db = b1.sub(b0);
    let denom = da.perp_dot(db);
    if denom.abs() > 1e-9 {
        let diff = b0.sub(a0);
        let u = diff.perp_dot(db) / denom;
        let v = diff.perp_dot(da) / denom;
        if (-1e-5..=1.0 + 1e-5).contains(&u) && (-1e-5..=1.0 + 1e-5).contains(&v) {
            return true;
        }
    }
    pt_seg_dist_sq(a0, b0, b1) < tol_sq
        || pt_seg_dist_sq(a1, b0, b1) < tol_sq
        || pt_seg_dist_sq(b0, a0, a1) < tol_sq
        || pt_seg_dist_sq(b1, a0, a1) < tol_sq
}

fn pt_seg_dist_sq(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b.sub(a);
    let l2 = ab.len_sq();
    if l2 < 1e-12 {
        return p.sub(a).len_sq();
    }
    let u = (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
    p.sub(a.add(ab.scale(u))).len_sq()
}

fn point_in_poly(p: Point2, poly: &[Point2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let a = poly[i];
        let b = poly[j];
        if ((a.y > p.y) != (b.y > p.y))
            && (p.x < (b.x - a.x) * (p.y - a.y) / ((b.y - a.y) + 1e-18) + a.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}
