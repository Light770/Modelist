use crate::curve::{edge_total_length, fit_cubic_schneider};
use crate::model::{
    det_id, polygon_signed_area, ArrangementZone, ClosedPolyCurve, CornerRule, CurveSegment,
    DrillHole, EaseSpec, EdgeRef, EvaluatedPiece, GrainRotationMode, GrainlineSpec, NotchAnchor,
    NotchStyle, PatternPiece, PatternProject, Point2, PolyEdge, SeamBinding, SeamDriver,
    SeamInterface, SeamType, FLATTEN_TOL_MM,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Strict DXF R12 (AC1009) ASTM D6673-10 / AAMA exporter.
/// Each pattern piece is encapsulated in one BLOCK, using canonical layers 1..15.
pub fn export_dxf_aama(pieces: &[EvaluatedPiece], size_name: &str) -> String {
    let mut out = String::with_capacity(32 * 1024);

    // HEADER (minimal R12, omit $MODEL_SPACE incompatibilities)
    push_pair(&mut out, 0, "SECTION");
    push_pair(&mut out, 2, "HEADER");
    push_pair(&mut out, 9, "$ACADVER");
    push_pair(&mut out, 1, "AC1009");
    push_pair(&mut out, 9, "$INSUNITS");
    push_pair(&mut out, 70, "4"); // 4 = millimeters
    push_pair(&mut out, 0, "ENDSEC");

    // TABLES (Layers 1..15)
    push_pair(&mut out, 0, "SECTION");
    push_pair(&mut out, 2, "TABLES");
    push_pair(&mut out, 0, "TABLE");
    push_pair(&mut out, 2, "LAYER");
    push_pair(&mut out, 70, "12");
    for (layer, color) in [
        ("1", 7),
        ("2", 3),
        ("3", 4),
        ("4", 1),
        ("6", 6),
        ("7", 5),
        ("8", 2),
        ("11", 7),
        ("13", 1),
        ("14", 3),
        ("15", 7),
    ] {
        push_pair(&mut out, 0, "LAYER");
        push_pair(&mut out, 2, layer);
        push_pair(&mut out, 70, "0");
        push_pair(&mut out, 62, &color.to_string());
        push_pair(&mut out, 6, "CONTINUOUS");
    }
    push_pair(&mut out, 0, "ENDTAB");
    push_pair(&mut out, 0, "ENDSEC");

    // BLOCKS (One BLOCK per PatternPiece)
    push_pair(&mut out, 0, "SECTION");
    push_pair(&mut out, 2, "BLOCKS");
    let mut block_names = Vec::with_capacity(pieces.len());

    for (idx, ep) in pieces.iter().enumerate() {
        let p = &ep.piece;
        let bname = sanitize_block_name(&format!("{}_{}_{}", p.code, idx + 1, size_name));
        block_names.push(bname.clone());

        push_pair(&mut out, 0, "BLOCK");
        push_pair(&mut out, 8, "0");
        push_pair(&mut out, 2, &bname);
        push_pair(&mut out, 70, "0");
        push_pt(&mut out, 0.0, 0.0);

        // Layer 1: Cut boundary (or seam boundary if no cutline)
        let cut_pts = if ep.cut_polyline.len() >= 3 {
            &ep.cut_polyline
        } else {
            &ep.seam_polyline
        };
        write_dxf_polyline(&mut out, "1", cut_pts, true);

        // Layer 14: Sew line (canonical seamline)
        if ep.seam_polyline.len() >= 3 {
            write_dxf_polyline(&mut out, "14", &ep.seam_polyline, true);
        }

        // Layer 2 (Turn Points) & Layer 3 (Curve Points)
        for edge in &p.seam_loop.edges {
            write_dxf_point(&mut out, "2", edge.start);
            if matches!(edge.segment, CurveSegment::Cubic { .. }) {
                for t in [0.25, 0.5, 0.75] {
                    let pt = crate::curve::eval_edge_point(edge, t);
                    write_dxf_point(&mut out, "3", pt);
                }
            }
        }

        // Layer 4: Notches
        for n in &ep.projected_notches {
            let tip = n.cut_pt.add(n.inward_dir.scale(n.depth_mm));
            write_dxf_line(&mut out, "4", n.cut_pt, tip);
        }

        // Layer 6: Mirror / Fold axis
        if let Some(m_idx) = p.mirror_fold_edge {
            if let Some(edge) = p.seam_loop.edges.get(m_idx) {
                write_dxf_line(&mut out, "6", edge.start, edge.end());
            }
        }

        // Layer 7: Grainline
        write_dxf_line(&mut out, "7", p.grainline.start, p.grainline.end);

        // Layer 8: Internal lines & dart legs
        for il in &p.internal_lines {
            if il.pts.len() >= 2 {
                write_dxf_polyline(&mut out, "8", &il.pts, false);
            }
        }
        for d in &p.darts {
            if let Some(ei) = d.resolve_edge_idx(&p.seam_loop) {
                let edge = &p.seam_loop.edges[ei];
                let a = crate::curve::eval_edge_point(edge, d.leg_a_t);
                let b = crate::curve::eval_edge_point(edge, d.leg_b_t);
                write_dxf_polyline(&mut out, "8", &[a, d.apex, b], false);
            }
        }

        // Layer 11: Cutouts
        for cutout in &ep.cutout_cutlines {
            write_dxf_polyline(&mut out, "11", cutout, true);
        }

        // Layer 13: Drill holes
        for dh in &p.drill_holes {
            write_dxf_point(&mut out, "13", dh.pos);
        }

        // Layer 15: Piece Metadata TEXT
        let label_pos = p.grainline.start.lerp(p.grainline.end, 0.5);
        write_dxf_text(
            &mut out,
            "15",
            label_pos,
            10.0,
            &format!("{} | {} | CUT {} | SIZE {}", p.name, p.code, p.cut_quantity, size_name),
        );

        push_pair(&mut out, 0, "ENDBLK");
        push_pair(&mut out, 8, "0");
    }
    push_pair(&mut out, 0, "ENDSEC");

    // ENTITIES: INSERT block instances at origin
    push_pair(&mut out, 0, "SECTION");
    push_pair(&mut out, 2, "ENTITIES");
    for bname in block_names {
        push_pair(&mut out, 0, "INSERT");
        push_pair(&mut out, 8, "1");
        push_pair(&mut out, 2, &bname);
        push_pt(&mut out, 0.0, 0.0);
    }
    push_pair(&mut out, 0, "ENDSEC");
    push_pair(&mut out, 0, "EOF");
    out
}

fn sanitize_block_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

fn push_pair(out: &mut String, code: i32, val: &str) {
    out.push_str(&format!("{code}\n{val}\n"));
}

fn push_pt(out: &mut String, x: f64, y: f64) {
    out.push_str(&format!("10\n{x:.4}\n20\n{y:.4}\n30\n0.0\n"));
}

fn write_dxf_point(out: &mut String, layer: &str, p: Point2) {
    push_pair(out, 0, "POINT");
    push_pair(out, 8, layer);
    push_pt(out, p.x, p.y);
}

fn write_dxf_line(out: &mut String, layer: &str, a: Point2, b: Point2) {
    push_pair(out, 0, "LINE");
    push_pair(out, 8, layer);
    out.push_str(&format!(
        "10\n{:.4}\n20\n{:.4}\n30\n0.0\n11\n{:.4}\n21\n{:.4}\n31\n0.0\n",
        a.x, a.y, b.x, b.y
    ));
}

fn write_dxf_polyline(out: &mut String, layer: &str, pts: &[Point2], closed: bool) {
    if pts.len() < 2 {
        return;
    }
    push_pair(out, 0, "POLYLINE");
    push_pair(out, 8, layer);
    push_pair(out, 66, "1");
    push_pair(out, 70, if closed { "1" } else { "0" });
    push_pt(out, 0.0, 0.0);
    for p in pts {
        push_pair(out, 0, "VERTEX");
        push_pair(out, 8, layer);
        push_pt(out, p.x, p.y);
    }
    push_pair(out, 0, "SEQEND");
    push_pair(out, 8, layer);
}

fn write_dxf_text(out: &mut String, layer: &str, p: Point2, height: f64, text: &str) {
    push_pair(out, 0, "TEXT");
    push_pair(out, 8, layer);
    push_pt(out, p.x, p.y);
    out.push_str(&format!("40\n{height:.2}\n1\n{text}\n"));
}

/// Imports strict or loose DXF-AAMA / ASTM D6673-10 files into editable PatternPieces.
/// Reconstructs smooth cubic Beziers from dense factory polylines using Schneider's fitter.
pub fn import_dxf_aama(dxf_text: &str) -> Result<Vec<PatternPiece>, String> {
    let pairs = parse_dxf_pairs(dxf_text);
    if pairs.is_empty() {
        return Err("Empty or invalid DXF file".into());
    }

    #[derive(Default)]
    struct RawBlock {
        name: String,
        polylines: Vec<(String, Vec<Point2>, bool)>,
        lines: Vec<(String, Point2, Point2)>,
        points: Vec<(String, Point2)>,
        texts: Vec<String>,
    }

    let mut blocks: Vec<RawBlock> = Vec::new();
    let mut top_entities = RawBlock {
        name: "Imported Piece".into(),
        ..Default::default()
    };

    let mut in_blocks = false;
    let mut in_entities = false;
    let mut curr_block: Option<RawBlock> = None;

    let mut i = 0;
    while i < pairs.len() {
        let (code, ref val) = pairs[i];
        if code == 0 && val == "SECTION" && i + 1 < pairs.len() && pairs[i + 1].0 == 2 {
            in_blocks = pairs[i + 1].1 == "BLOCKS";
            in_entities = pairs[i + 1].1 == "ENTITIES";
            i += 2;
            continue;
        }
        if code == 0 && val == "ENDSEC" {
            in_blocks = false;
            in_entities = false;
            i += 1;
            continue;
        }
        if in_blocks && code == 0 && val == "BLOCK" {
            let mut b = RawBlock::default();
            i += 1;
            while i < pairs.len() && pairs[i].0 != 0 {
                if pairs[i].0 == 2 {
                    b.name = pairs[i].1.clone();
                }
                i += 1;
            }
            curr_block = Some(b);
            continue;
        }
        if in_blocks && code == 0 && val == "ENDBLK" {
            if let Some(b) = curr_block.take() {
                blocks.push(b);
            }
            i += 1;
            continue;
        }

        let target = if let Some(ref mut b) = curr_block {
            Some(b)
        } else if in_entities {
            Some(&mut top_entities)
        } else {
            None
        };

        if let (Some(blk), 0) = (target, code) {
            match val.as_str() {
                "POLYLINE" | "LWPOLYLINE" => {
                    let is_lw = val == "LWPOLYLINE";
                    let mut layer = "1".to_string();
                    let mut closed = false;
                    let mut pts = Vec::new();
                    let mut curr_x: Option<f64> = None;
                    i += 1;
                    while i < pairs.len() {
                        let (c, ref v) = pairs[i];
                        if c == 0 {
                            if is_lw || v == "SEQEND" {
                                if !is_lw {
                                    i += 1;
                                }
                                break;
                            }
                            if v == "VERTEX" {
                                let mut vx = 0.0;
                                let mut vy = 0.0;
                                i += 1;
                                while i < pairs.len() && pairs[i].0 != 0 {
                                    match pairs[i].0 {
                                        10 => vx = pairs[i].1.parse().unwrap_or(0.0),
                                        20 => vy = pairs[i].1.parse().unwrap_or(0.0),
                                        _ => {}
                                    }
                                    i += 1;
                                }
                                pts.push(Point2::new(vx, vy));
                                continue;
                            }
                            break;
                        }
                        match c {
                            8 => layer = v.clone(),
                            70 => closed = v.parse::<i32>().unwrap_or(0) & 1 != 0,
                            10 if is_lw => curr_x = v.parse().ok(),
                            20 if is_lw => {
                                if let (Some(x), Ok(y)) = (curr_x.take(), v.parse::<f64>()) {
                                    pts.push(Point2::new(x, y));
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                    if pts.len() >= 2 {
                        blk.polylines.push((layer, pts, closed));
                    }
                    continue;
                }
                "LINE" => {
                    let mut layer = "1".to_string();
                    let (mut x1, mut y1, mut x2, mut y2) = (0.0, 0.0, 0.0, 0.0);
                    i += 1;
                    while i < pairs.len() && pairs[i].0 != 0 {
                        match pairs[i].0 {
                            8 => layer = pairs[i].1.clone(),
                            10 => x1 = pairs[i].1.parse().unwrap_or(0.0),
                            20 => y1 = pairs[i].1.parse().unwrap_or(0.0),
                            11 => x2 = pairs[i].1.parse().unwrap_or(0.0),
                            21 => y2 = pairs[i].1.parse().unwrap_or(0.0),
                            _ => {}
                        }
                        i += 1;
                    }
                    blk.lines
                        .push((layer, Point2::new(x1, y1), Point2::new(x2, y2)));
                    continue;
                }
                "POINT" => {
                    let mut layer = "2".to_string();
                    let (mut x, mut y) = (0.0, 0.0);
                    i += 1;
                    while i < pairs.len() && pairs[i].0 != 0 {
                        match pairs[i].0 {
                            8 => layer = pairs[i].1.clone(),
                            10 => x = pairs[i].1.parse().unwrap_or(0.0),
                            20 => y = pairs[i].1.parse().unwrap_or(0.0),
                            _ => {}
                        }
                        i += 1;
                    }
                    blk.points.push((layer, Point2::new(x, y)));
                    continue;
                }
                "TEXT" => {
                    let mut txt = String::new();
                    i += 1;
                    while i < pairs.len() && pairs[i].0 != 0 {
                        if pairs[i].0 == 1 {
                            txt = pairs[i].1.clone();
                        }
                        i += 1;
                    }
                    if !txt.is_empty() {
                        blk.texts.push(txt);
                    }
                    continue;
                }
                _ => {}
            }
        }
        i += 1;
    }

    if blocks.is_empty() && (!top_entities.polylines.is_empty() || !top_entities.lines.is_empty()) {
        blocks.push(top_entities);
    }

    let mut pieces = Vec::new();
    for (idx, blk) in blocks.into_iter().enumerate() {
        // Prefer Layer 14 (sew line) if present, else Layer 1 (boundary)
        let boundary_pts = pick_boundary(&blk.polylines, &blk.lines);
        if boundary_pts.len() < 3 {
            continue;
        }

        let turn_pts: Vec<Point2> = blk
            .points
            .iter()
            .filter(|(l, _)| l == "2")
            .map(|(_, p)| *p)
            .collect();

        let mut poly_ccw = boundary_pts.clone();
        if polygon_signed_area(&poly_ccw) < 0.0 {
            poly_ccw.reverse();
        }
        let edges = reconstruct_bezier_edges(&poly_ccw, &turn_pts, idx);
        let mut seam_loop = ClosedPolyCurve::from_edges(edges);
        seam_loop.sync_continuity();
        seam_loop.ensure_ccw();

        let grainline = blk
            .lines
            .iter()
            .find(|(l, _, _)| l == "7")
            .map(|(_, a, b)| GrainlineSpec {
                start: *a,
                end: *b,
                rotation_mode: GrainRotationMode::TwoWay,
            })
            .unwrap_or_else(|| {
                let c = centroid(&poly_ccw);
                GrainlineSpec {
                    start: Point2::new(c.x, c.y - 60.0),
                    end: Point2::new(c.x, c.y + 60.0),
                    rotation_mode: GrainRotationMode::TwoWay,
                }
            });

        let mut notches = Vec::new();
        for (ni, (l, a, _b)) in blk.lines.iter().enumerate() {
            if l == "4" {
                if let Some((e_idx, t)) = project_point_to_loop(&seam_loop, *a) {
                    notches.push(NotchAnchor {
                        id: det_id(&format!("dxf_import:{idx}:notch:{ni}")),
                        edge_idx: e_idx,
                        edge_id: seam_loop.edges.get(e_idx).map(|e| e.id),
                        param_t: t,
                        style: NotchStyle::Slit,
                        count: 1,
                        depth_mm: 5.0,
                    });
                }
            }
        }

        let drill_holes: Vec<DrillHole> = blk
            .points
            .iter()
            .filter(|(l, _)| l == "13")
            .enumerate()
            .map(|(di, (_, p))| DrillHole {
                id: det_id(&format!("dxf_import:{idx}:drill:{di}")),
                pos: *p,
                radius_mm: 2.0,
                label: "Drill".into(),
            })
            .collect();

        let name = blk
            .texts
            .first()
            .and_then(|t| t.split('|').next())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if blk.name.is_empty() {
                    format!("DXF Piece {}", idx + 1)
                } else {
                    blk.name.clone()
                }
            });

        pieces.push(PatternPiece {
            id: det_id(&format!("dxf_import:piece:{idx}:{name}")),
            name,
            code: format!("DXF-{:02}", idx + 1),
            cut_quantity: 1,
            mirror_fold_edge: None,
            grainline,
            seam_loop,
            cutouts: vec![],
            darts: vec![],
            internal_lines: vec![],
            drill_holes,
            notches,
            grade_deltas: vec![],
            visible: true,
            fabric_role: "Imported".into(),
            arrangement_zone: ArrangementZone::FlatCustom,
        });
    }

    if pieces.is_empty() {
        return Err("No valid closed pattern contours found in DXF".into());
    }
    Ok(pieces)
}

fn parse_dxf_pairs(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).collect();
    let mut pairs = Vec::with_capacity(lines.len() / 2);
    let mut i = 0;
    while i + 1 < lines.len() {
        if let Ok(code) = lines[i].parse::<i32>() {
            pairs.push((code, lines[i + 1].to_string()));
            i += 2;
        } else {
            i += 1;
        }
    }
    pairs
}

fn pick_boundary(
    polylines: &[(String, Vec<Point2>, bool)],
    lines: &[(String, Point2, Point2)],
) -> Vec<Point2> {
    for target_layer in ["14", "1"] {
        if let Some((_, pts, _)) = polylines.iter().find(|(l, p, _)| l == target_layer && p.len() >= 3) {
            return dedupe_close_pts(pts);
        }
    }
    if let Some((_, pts, _)) = polylines.iter().find(|(_, p, _)| p.len() >= 3) {
        return dedupe_close_pts(pts);
    }
    // Chain disjoint LINE fragments on Layer 14 or Layer 1
    for target_layer in ["14", "1"] {
        let segs: Vec<(Point2, Point2)> = lines
            .iter()
            .filter(|(l, _, _)| l == target_layer)
            .map(|(_, a, b)| (*a, *b))
            .collect();
        if segs.len() >= 3 {
            let chained = chain_segments(&segs);
            if chained.len() >= 3 {
                return dedupe_close_pts(&chained);
            }
        }
    }
    Vec::new()
}

fn dedupe_close_pts(pts: &[Point2]) -> Vec<Point2> {
    let mut out: Vec<Point2> = Vec::with_capacity(pts.len());
    for &p in pts {
        if out.last().map(|last| last.dist(p) > 1e-3).unwrap_or(true) {
            out.push(p);
        }
    }
    if out.len() > 2 && out.first().unwrap().dist(*out.last().unwrap()) < 1e-3 {
        out.pop();
    }
    out
}

fn chain_segments(segs: &[(Point2, Point2)]) -> Vec<Point2> {
    let mut remaining = segs.to_vec();
    let (a, b) = remaining.remove(0);
    let mut chain = vec![a, b];
    while !remaining.is_empty() {
        let tail = *chain.last().unwrap();
        let mut best = None;
        let mut best_d = 0.5_f64;
        for (idx, &(s, e)) in remaining.iter().enumerate() {
            if tail.dist(s) < best_d {
                best_d = tail.dist(s);
                best = Some((idx, e));
            } else if tail.dist(e) < best_d {
                best_d = tail.dist(e);
                best = Some((idx, s));
            }
        }
        if let Some((idx, next_pt)) = best {
            remaining.remove(idx);
            chain.push(next_pt);
        } else {
            break;
        }
    }
    chain
}

fn reconstruct_bezier_edges(pts: &[Point2], turn_pts: &[Point2], piece_idx: usize) -> Vec<PolyEdge> {
    let n = pts.len();
    let mut corners = vec![false; n];
    for i in 0..n {
        let p = pts[i];
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 1) % n];
        let v1 = p.sub(prev).normalized();
        let v2 = next.sub(p).normalized();
        let dot = v1.dot(v2).clamp(-1.0, 1.0);
        let turn_deg = dot.acos().to_degrees();
        let near_turn_pt = turn_pts.iter().any(|tp| tp.dist(p) < 0.5);
        if near_turn_pt || turn_deg > 22.0 {
            corners[i] = true;
        }
    }
    if !corners.iter().any(|&c| c) {
        corners[0] = true;
    }

    let corner_indices: Vec<usize> = corners
        .iter()
        .enumerate()
        .filter_map(|(i, &c)| if c { Some(i) } else { None })
        .collect();

    let mut edges = Vec::new();
    for k in 0..corner_indices.len() {
        let start_i = corner_indices[k];
        let end_i = corner_indices[(k + 1) % corner_indices.len()];
        let mut span = Vec::new();
        let mut curr = start_i;
        loop {
            span.push(pts[curr]);
            if curr == end_i && span.len() > 1 {
                break;
            }
            curr = (curr + 1) % n;
            if span.len() > n + 1 {
                break;
            }
        }
        if span.len() == 2 {
            edges.push(PolyEdge {
                id: det_id(&format!("dxf:{piece_idx}:e:{}", edges.len())),
                start: span[0],
                segment: CurveSegment::Line { end: span[1] },
                sa_width_mm: 10.0,
                corner_rule: CornerRule::Miter { limit: 2.5 },
                label: format!("Edge {}", edges.len() + 1),
            });
        } else {
            let segs = fit_cubic_schneider(&span, 0.35);
            let mut cursor = span[0];
            for seg in segs {
                let end = seg.end_point();
                edges.push(PolyEdge {
                    id: det_id(&format!("dxf:{piece_idx}:e:{}", edges.len())),
                    start: cursor,
                    segment: seg,
                    sa_width_mm: 10.0,
                    corner_rule: CornerRule::Miter { limit: 2.5 },
                    label: format!("Edge {}", edges.len() + 1),
                });
                cursor = end;
            }
        }
    }
    edges
}

fn project_point_to_loop(loop_c: &ClosedPolyCurve, target: Point2) -> Option<(usize, f64)> {
    let mut best = None;
    let mut best_d = f64::INFINITY;
    for (ei, edge) in loop_c.edges.iter().enumerate() {
        for s in 0..=20 {
            let t = (s as f64) / 20.0;
            let p = crate::curve::eval_edge_point(edge, t);
            let d = p.dist(target);
            if d < best_d {
                best_d = d;
                best = Some((ei, t.clamp(0.05, 0.95)));
            }
        }
    }
    best
}

fn centroid(pts: &[Point2]) -> Point2 {
    let mut sx = 0.0;
    let mut sy = 0.0;
    for p in pts {
        sx += p.x;
        sy += p.y;
    }
    let n = pts.len().max(1) as f64;
    Point2::new(sx / n, sy / n)
}

/// Exact-millimeter SVG exporter with per-size groups and seam/cut/notch/grain layers.
pub fn export_svg_mm(
    project_name: &str,
    active_size: &str,
    evaluated: &[EvaluatedPiece],
    graded_nest: &HashMap<String, Vec<EvaluatedPiece>>,
) -> String {
    let mut all_pts = Vec::new();
    for ep in evaluated {
        all_pts.extend(&ep.seam_polyline);
        all_pts.extend(&ep.cut_polyline);
    }
    for list in graded_nest.values() {
        for ep in list {
            all_pts.extend(&ep.seam_polyline);
            all_pts.extend(&ep.cut_polyline);
        }
    }

    let (min_x, min_y, max_x, max_y) = bbox_of(&all_pts);
    let pad = 25.0;
    let w = (max_x - min_x + 2.0 * pad).max(100.0);
    let h = (max_y - min_y + 2.0 * pad).max(100.0);

    let mut svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.2}mm\" height=\"{h:.2}mm\" viewBox=\"0 0 {w:.2} {h:.2}\">\n  <title>{}</title>\n",
        xml_escape(project_name)
    );

    let to_svg = |p: Point2| -> (f64, f64) { (p.x - min_x + pad, max_y - p.y + pad) };

    // Graded nest layers
    let palette = ["#64748b", "#2563eb", "#059669", "#d97706", "#7c3aed", "#dc2626"];
    let mut size_names: Vec<&String> = graded_nest.keys().collect();
    size_names.sort();
    for (si, sname) in size_names.iter().enumerate() {
        if *sname == active_size {
            continue;
        }
        let color = palette[si % palette.len()];
        svg.push_str(&format!(
            "  <g id=\"size-{}\" stroke=\"{color}\" fill=\"none\" stroke-width=\"0.4\" stroke-dasharray=\"4 2\">\n",
            xml_escape(sname)
        ));
        if let Some(list) = graded_nest.get(*sname) {
            for ep in list {
                let d = poly_to_svg_path(&ep.seam_polyline, &to_svg);
                svg.push_str(&format!("    <path d=\"{d}\" />\n"));
            }
        }
        svg.push_str("  </g>\n");
    }

    svg.push_str(&format!("  <g id=\"size-{}-active\">\n", xml_escape(active_size)));
    for ep in evaluated {
        let cut_d = poly_to_svg_path(&ep.cut_polyline, &to_svg);
        let seam_d = poly_to_svg_path(&ep.seam_polyline, &to_svg);
        svg.push_str(&format!(
            "    <g id=\"piece-{}\">\n",
            xml_escape(&ep.piece.code)
        ));
        if !cut_d.is_empty() {
            svg.push_str(&format!(
                "      <path d=\"{cut_d}\" fill=\"none\" stroke=\"#d97706\" stroke-width=\"0.55\" stroke-dasharray=\"5 3\" />\n"
            ));
        }
        svg.push_str(&format!(
            "      <path d=\"{seam_d}\" fill=\"rgba(15,23,42,0.03)\" stroke=\"#111827\" stroke-width=\"0.75\" />\n"
        ));
        // Grainline
        let (gx0, gy0) = to_svg(ep.piece.grainline.start);
        let (gx1, gy1) = to_svg(ep.piece.grainline.end);
        svg.push_str(&format!(
            "      <line x1=\"{gx0:.2}\" y1=\"{gy0:.2}\" x2=\"{gx1:.2}\" y2=\"{gy1:.2}\" stroke=\"#2563eb\" stroke-width=\"0.45\" />\n"
        ));
        // Notches
        for n in &ep.projected_notches {
            let (nx0, ny0) = to_svg(n.cut_pt);
            let tip = n.cut_pt.add(n.inward_dir.scale(n.depth_mm));
            let (nx1, ny1) = to_svg(tip);
            svg.push_str(&format!(
                "      <line x1=\"{nx0:.2}\" y1=\"{ny0:.2}\" x2=\"{nx1:.2}\" y2=\"{ny1:.2}\" stroke=\"#dc2626\" stroke-width=\"0.6\" />\n"
            ));
        }
        // Label
        let mid = ep.piece.grainline.start.lerp(ep.piece.grainline.end, 0.5);
        let (lx, ly) = to_svg(mid);
        svg.push_str(&format!(
            "      <text x=\"{lx:.2}\" y=\"{ly:.2}\" font-family=\"sans-serif\" font-size=\"6\" fill=\"#111827\">{} ({})</text>\n",
            xml_escape(&ep.piece.name),
            xml_escape(active_size)
        ));
        svg.push_str("    </g>\n");
    }
    svg.push_str("  </g>\n</svg>\n");
    svg
}

fn poly_to_svg_path(pts: &[Point2], to_svg: &impl Fn(Point2) -> (f64, f64)) -> String {
    if pts.len() < 2 {
        return String::new();
    }
    let mut d = String::new();
    for (i, &p) in pts.iter().enumerate() {
        let (x, y) = to_svg(p);
        if i == 0 {
            d.push_str(&format!("M {x:.2} {y:.2}"));
        } else {
            d.push_str(&format!(" L {x:.2} {y:.2}"));
        }
    }
    d.push_str(" Z");
    d
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PomRow {
    pub pom_name: String,
    pub values_mm: BTreeMap<String, f64>,
}

pub fn compute_pom_table(project: &PatternProject) -> Vec<PomRow> {
    let mut rows_map: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    for size in &project.size_table.sizes {
        let m = project
            .size_table
            .measurements_for(size, &project.measurements);
        let pieces: Vec<PatternPiece> = crate::blocks::build_project_pieces(project, &m)
            .pieces
            .into_iter()
            .filter(|p| p.visible)
            .collect();
        for p in &pieces {
            for (ei, e) in p.seam_loop.edges.iter().enumerate() {
                let key = format!("{} — {} (#{})", p.code, e.label, ei + 1);
                let len = edge_total_length(e);
                rows_map
                    .entry(key)
                    .or_default()
                    .insert(size.clone(), (len * 100.0).round() / 100.0);
            }
        }
    }
    rows_map
        .into_iter()
        .map(|(pom_name, values_mm)| PomRow { pom_name, values_mm })
        .collect()
}

pub fn export_pom_csv(project: &PatternProject) -> String {
    let rows = compute_pom_table(project);
    let mut out = String::from("Point of Measurement");
    for s in &project.size_table.sizes {
        out.push_str(&format!(",{s} (mm)"));
    }
    out.push('\n');
    for r in rows {
        out.push_str(&format!("\"{}\"", r.pom_name.replace('"', "\"\"")));
        for s in &project.size_table.sizes {
            let v = r.values_mm.get(s).copied().unwrap_or(0.0);
            out.push_str(&format!(",{v:.2}"));
        }
        out.push('\n');
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfLayerPath {
    pub size_name: String,
    pub layer_kind: String, // "seam" | "cut" | "grain" | "notch" | "dart"
    pub pts: Vec<Point2>,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfLabelItem {
    pub pos: Point2,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfTileRect {
    pub row: usize,
    pub col: usize,
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfLayoutPlan {
    pub preset: String,
    pub page_width_mm: f64,
    pub page_height_mm: f64,
    pub margin_mm: f64,
    pub overlap_mm: f64,
    pub bounds_min_x: f64,
    pub bounds_min_y: f64,
    pub bounds_max_x: f64,
    pub bounds_max_y: f64,
    pub tiles: Vec<PdfTileRect>,
    pub paths: Vec<PdfLayerPath>,
    pub labels: Vec<PdfLabelItem>,
}

/// Computes all PDF geometry coordinates in Rust (including Projector unfolded fold pieces
/// and optimal-origin A4/Letter tile grid culling) so TypeScript only serializes vectors.
pub fn compute_pdf_layout_plan(
    preset: &str,
    unfold_on_fold: bool,
    active_size: &str,
    evaluated: &[EvaluatedPiece],
    graded_nest: &HashMap<String, Vec<EvaluatedPiece>>,
) -> PdfLayoutPlan {
    let mut paths: Vec<PdfLayerPath> = Vec::new();
    let mut labels: Vec<PdfLabelItem> = Vec::new();
    let mut all_pts: Vec<Point2> = Vec::new();

    let mut append_piece_set = |size_name: &str, list: &[EvaluatedPiece], is_active: bool| {
        for ep in list {
            let mut seam = ep.seam_polyline.clone();
            let mut cut = ep.cut_polyline.clone();

            // Projector / optional auto-unfold across mirror fold edge
            if unfold_on_fold {
                if let Some(m_idx) = ep.piece.mirror_fold_edge {
                    if let Some(fold_edge) = ep.piece.seam_loop.edges.get(m_idx) {
                        let a = fold_edge.start;
                        let b = fold_edge.end();
                        seam = unfold_polygon(&seam, a, b);
                        if !cut.is_empty() {
                            cut = unfold_polygon(&cut, a, b);
                        }
                    }
                }
            }

            all_pts.extend(&seam);
            all_pts.extend(&cut);

            paths.push(PdfLayerPath {
                size_name: size_name.to_string(),
                layer_kind: "seam".into(),
                pts: seam,
                closed: true,
            });
            if is_active && cut.len() >= 3 {
                paths.push(PdfLayerPath {
                    size_name: size_name.to_string(),
                    layer_kind: "cut".into(),
                    pts: cut,
                    closed: true,
                });
            }
            if is_active {
                paths.push(PdfLayerPath {
                    size_name: size_name.to_string(),
                    layer_kind: "grain".into(),
                    pts: vec![ep.piece.grainline.start, ep.piece.grainline.end],
                    closed: false,
                });
                for n in &ep.projected_notches {
                    let tip = n.cut_pt.add(n.inward_dir.scale(n.depth_mm));
                    paths.push(PdfLayerPath {
                        size_name: size_name.to_string(),
                        layer_kind: "notch".into(),
                        pts: vec![n.cut_pt, tip],
                        closed: false,
                    });
                }
                for il in &ep.piece.internal_lines {
                    if il.pts.len() >= 2 {
                        paths.push(PdfLayerPath {
                            size_name: size_name.to_string(),
                            layer_kind: "dart".into(),
                            pts: il.pts.clone(),
                            closed: false,
                        });
                    }
                }
                for d in &ep.piece.darts {
                    if let Some(ei) = d.resolve_edge_idx(&ep.piece.seam_loop) {
                        let edge = &ep.piece.seam_loop.edges[ei];
                        let da = crate::curve::eval_edge_point(edge, d.leg_a_t);
                        let db = crate::curve::eval_edge_point(edge, d.leg_b_t);
                        paths.push(PdfLayerPath {
                            size_name: size_name.to_string(),
                            layer_kind: "dart".into(),
                            pts: vec![da, d.apex, db],
                            closed: false,
                        });
                    }
                }
                let mid = ep.piece.grainline.start.lerp(ep.piece.grainline.end, 0.5);
                labels.push(PdfLabelItem {
                    pos: mid,
                    text: format!("{} [{}] Cut {}", ep.piece.name, size_name, ep.piece.cut_quantity),
                });
            }
        }
    };

    for (sname, list) in graded_nest {
        if sname != active_size {
            append_piece_set(sname, list, false);
        }
    }
    append_piece_set(active_size, evaluated, true);

    let (min_x, min_y, max_x, max_y) = bbox_of(&all_pts);

    let (page_w, page_h, margin, overlap): (f64, f64, f64, f64) = match preset {
        "letter_tiled" => (215.9, 279.4, 12.0, 12.0),
        "a0" => (841.0, 1189.0, 20.0, 0.0),
        "projector" => {
            let pan = 300.0;
            let pw = (max_x - min_x + 2.0 * pan).max(800.0);
            let ph = (max_y - min_y + 2.0 * pan).max(600.0);
            return PdfLayoutPlan {
                preset: preset.to_string(),
                page_width_mm: pw,
                page_height_mm: ph,
                margin_mm: pan,
                overlap_mm: 0.0,
                bounds_min_x: min_x - pan,
                bounds_min_y: min_y - pan,
                bounds_max_x: max_x + pan,
                bounds_max_y: max_y + pan,
                tiles: vec![PdfTileRect {
                    row: 0,
                    col: 0,
                    x_min: min_x - pan,
                    y_min: min_y - pan,
                    x_max: max_x + pan,
                    y_max: max_y + pan,
                }],
                paths,
                labels,
            };
        }
        _ => (210.0, 297.0, 12.0, 12.0), // a4_tiled default
    };

    if preset == "a0" {
        return PdfLayoutPlan {
            preset: preset.to_string(),
            page_width_mm: page_w.max(max_x - min_x + 40.0),
            page_height_mm: page_h.max(max_y - min_y + 40.0),
            margin_mm: margin,
            overlap_mm: 0.0,
            bounds_min_x: min_x - margin,
            bounds_min_y: min_y - margin,
            bounds_max_x: max_x + margin,
            bounds_max_y: max_y + margin,
            tiles: vec![PdfTileRect {
                row: 0,
                col: 0,
                x_min: min_x - margin,
                y_min: min_y - margin,
                x_max: max_x + margin,
                y_max: max_y + margin,
            }],
            paths,
            labels,
        };
    }

    // Search optimal grid origin across a 4x4 phase offset to minimize non-empty tiles
    let step_x = page_w - 2.0 * margin - overlap;
    let step_y = page_h - 2.0 * margin - overlap;
    let mut best_tiles: Option<Vec<PdfTileRect>> = None;

    for ox_i in 0..4 {
        for oy_i in 0..4 {
            let x0 = min_x - (ox_i as f64) * (step_x * 0.2);
            let y0 = min_y - (oy_i as f64) * (step_y * 0.2);
            let cols = (((max_x - x0) / step_x).ceil() as usize).max(1);
            let rows = (((max_y - y0) / step_y).ceil() as usize).max(1);
            let mut candidate = Vec::new();
            for r in 0..rows {
                for c in 0..cols {
                    let tx0 = x0 + (c as f64) * step_x;
                    let ty0 = y0 + (r as f64) * step_y;
                    let tx1 = tx0 + page_w - 2.0 * margin;
                    let ty1 = ty0 + page_h - 2.0 * margin;
                    if tile_has_geometry(tx0, ty0, tx1, ty1, &paths) {
                        candidate.push(PdfTileRect {
                            row: r,
                            col: c,
                            x_min: tx0,
                            y_min: ty0,
                            x_max: tx1,
                            y_max: ty1,
                        });
                    }
                }
            }
            if best_tiles
                .as_ref()
                .map(|b| candidate.len() < b.len())
                .unwrap_or(true)
            {
                best_tiles = Some(candidate);
            }
        }
    }

    PdfLayoutPlan {
        preset: preset.to_string(),
        page_width_mm: page_w,
        page_height_mm: page_h,
        margin_mm: margin,
        overlap_mm: overlap,
        bounds_min_x: min_x,
        bounds_min_y: min_y,
        bounds_max_x: max_x,
        bounds_max_y: max_y,
        tiles: best_tiles.unwrap_or_default(),
        paths,
        labels,
    }
}

fn tile_has_geometry(x0: f64, y0: f64, x1: f64, y1: f64, paths: &[PdfLayerPath]) -> bool {
    for p in paths {
        for w in p.pts.windows(2) {
            let minx = w[0].x.min(w[1].x);
            let maxx = w[0].x.max(w[1].x);
            let miny = w[0].y.min(w[1].y);
            let maxy = w[0].y.max(w[1].y);
            if maxx >= x0 && minx <= x1 && maxy >= y0 && miny <= y1 {
                return true;
            }
        }
    }
    false
}

fn unfold_polygon(pts: &[Point2], a: Point2, b: Point2) -> Vec<Point2> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let dir = b.sub(a).normalized();
    let reflect = |p: Point2| -> Point2 {
        let ap = p.sub(a);
        let proj = dir.scale(ap.dot(dir));
        let perp = ap.sub(proj);
        a.add(proj).sub(perp)
    };
    let mut out = pts.to_vec();
    for p in pts.iter().rev() {
        out.push(reflect(*p));
    }
    out
}

fn bbox_of(pts: &[Point2]) -> (f64, f64, f64, f64) {
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
    if !min_x.is_finite() {
        (0.0, 0.0, 200.0, 200.0)
    } else {
        (min_x, min_y, max_x, max_y)
    }
}

pub fn serialize_evaluated_polygon_points(piece: &PatternPiece) -> Vec<Point2> {
    piece.seam_loop.flatten(FLATTEN_TOL_MM)
}

/// ANSI / AAMA `.rul` Grade Rule Table sidecar exporter (Stage 10 production handoff).
/// Computes exact per-vertex (dx, dy) deltas in millimeters across every size relative to base size.
pub fn export_aama_rul(project: &PatternProject) -> String {
    let base_size = &project.size_table.active_size;
    let sizes = &project.size_table.sizes;

    let mut per_size_pieces: HashMap<String, Vec<PatternPiece>> = HashMap::new();
    for size in sizes {
        let m = project
            .size_table
            .measurements_for(size, &project.measurements);
        let pieces: Vec<PatternPiece> = crate::blocks::build_project_pieces(project, &m)
            .pieces
            .into_iter()
            .filter(|p| p.visible)
            .collect();
        per_size_pieces.insert(size.clone(), pieces);
    }

    let Some(base_pieces) = per_size_pieces.get(base_size) else {
        return String::new();
    };

    let mut rule_entries: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    for (pi, bp) in base_pieces.iter().enumerate() {
        let base_anchor = bp.grainline.start.lerp(bp.grainline.end, 0.5);
        for (ei, edge) in bp.seam_loop.edges.iter().enumerate() {
            let base_rel = edge.start.sub(base_anchor);
            let mut deltas = Vec::with_capacity(sizes.len());
            for s in sizes {
                let delta = per_size_pieces
                    .get(s)
                    .and_then(|list| list.get(pi))
                    .and_then(|gp| {
                        let g_anchor = gp.grainline.start.lerp(gp.grainline.end, 0.5);
                        gp.seam_loop
                            .edge_by_id(edge.id)
                            .or_else(|| gp.seam_loop.edges.get(ei))
                            .map(|ge| ge.start.sub(g_anchor).sub(base_rel))
                    })
                    .unwrap_or(Point2::ZERO);
                deltas.push((delta.x, delta.y));
            }
            rule_entries.push((format!("{} V{} ({})", bp.code, ei + 1, edge.label), deltas));
        }
    }

    let mut out = String::with_capacity(8192);
    out.push_str("AUTHOR: Modelist CAD\n");
    out.push_str(&format!("STYLE: {}\n", project.name));
    out.push_str("PRODUCT: AAMA/ASTM D6673 Grade Rule Table\n");
    out.push_str("VERSION: 1.0\n");
    out.push_str("UNITS: METRIC\n");
    out.push_str(&format!("NUMBER OF SIZES: {}\n", sizes.len()));
    out.push_str(&format!("SIZE LIST: {}\n", sizes.join(" ")));
    out.push_str(&format!("SAMPLE SIZE: {base_size}\n"));
    out.push_str(&format!("NUMBER OF RULES: {}\n", rule_entries.len()));

    for (idx, (comment, deltas)) in rule_entries.iter().enumerate() {
        out.push_str(&format!("RULE: {} /* {comment} */\n", idx + 1));
        for (dx, dy) in deltas {
            out.push_str(&format!("DELTA: {dx:.3} {dy:.3}\n"));
        }
    }
    out.push_str("END\n");
    out
}

/// GarmentCode / GarmentCodeData specification JSON importer (ETH Zurich / Design2GarmentCode interop).
/// Converts panels (cm -> mm), relative quadratic/cubic curvatures, and stitches directly into
/// Modelist PatternPieces + UUID-anchored SeamBindings.
pub fn import_garmentcode_json(
    json_text: &str,
) -> Result<(Vec<PatternPiece>, Vec<SeamBinding>), String> {
    let root: serde_json::Value =
        serde_json::from_str(json_text).map_err(|e| format!("Invalid JSON: {e}"))?;
    let pattern = root
        .get("pattern")
        .unwrap_or(&root);
    let panels_obj = pattern
        .get("panels")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Missing `pattern.panels` object in GarmentCode JSON".to_string())?;

    let mut pieces = Vec::new();
    let mut panel_edge_map: HashMap<(String, usize), (uuid::Uuid, uuid::Uuid)> = HashMap::new();
    let mut cursor_x = 0.0_f64;

    let mut panel_names: Vec<String> = panels_obj.keys().cloned().collect();
    panel_names.sort();

    for (p_idx, pname) in panel_names.iter().enumerate() {
        let pval = &panels_obj[pname];
        let verts_val = pval
            .get("vertices")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("Panel `{pname}` missing `vertices`"))?;
        let edges_val = pval
            .get("edges")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("Panel `{pname}` missing `edges`"))?;

        // GarmentCode uses centimeters -> convert to exact f64 millimeters (x 10.0)
        let raw_verts: Vec<Point2> = verts_val
            .iter()
            .filter_map(|pt| {
                let arr = pt.as_array()?;
                Some(Point2::new(
                    arr.first()?.as_f64()? * 10.0,
                    arr.get(1)?.as_f64()? * 10.0,
                ))
            })
            .collect();
        if raw_verts.len() < 3 {
            continue;
        }
        let (min_x, min_y, max_x, max_y) = bbox_of(&raw_verts);
        let shift = Point2::new(cursor_x - min_x, -min_y);
        let verts: Vec<Point2> = raw_verts.iter().map(|v| v.add(shift)).collect();

        let pid = det_id(&format!("garmentcode:panel:{pname}"));
        let mut poly_edges = Vec::new();

        for (ei, eval) in edges_val.iter().enumerate() {
            let eps = match eval.get("endpoints").and_then(|v| v.as_array()) {
                Some(a) if a.len() >= 2 => (
                    a[0].as_u64().unwrap_or(0) as usize,
                    a[1].as_u64().unwrap_or(0) as usize,
                ),
                _ => continue,
            };
            if eps.0 >= verts.len() || eps.1 >= verts.len() {
                continue;
            }
            let p0 = verts[eps.0];
            let p1 = verts[eps.1];
            let eid = det_id(&format!("garmentcode:edge:{pname}:{ei}"));
            panel_edge_map.insert((pname.clone(), ei), (pid, eid));

            let segment = parse_gc_curvature(p0, p1, eval.get("curvature"));
            poly_edges.push(PolyEdge {
                id: eid,
                start: p0,
                segment,
                sa_width_mm: 10.0,
                corner_rule: CornerRule::Miter { limit: 2.5 },
                label: format!("Edge {}", ei + 1),
            });
        }

        if poly_edges.len() < 3 {
            continue;
        }
        let mut loop_c = ClosedPolyCurve::from_edges(poly_edges);
        loop_c.sync_continuity();
        loop_c.ensure_ccw();

        let cx = cursor_x + (max_x - min_x) * 0.5;
        let h = (max_y - min_y).max(60.0);
        let lower = pname.to_lowercase();
        let zone = if lower.contains("sleeve") {
            if lower.contains("r") {
                ArrangementZone::SleeveRight
            } else {
                ArrangementZone::SleeveLeft
            }
        } else if lower.contains("skirt") {
            if lower.contains("back") {
                ArrangementZone::BackSkirt
            } else {
                ArrangementZone::FrontSkirt
            }
        } else if lower.contains("back") {
            ArrangementZone::BackTorso
        } else {
            ArrangementZone::FrontTorso
        };

        pieces.push(PatternPiece {
            id: pid,
            name: pname.replace('_', " "),
            code: format!("GC-{:02}", p_idx + 1),
            cut_quantity: 1,
            mirror_fold_edge: None,
            grainline: GrainlineSpec {
                start: Point2::new(cx, h * 0.18),
                end: Point2::new(cx, h * 0.82),
                rotation_mode: GrainRotationMode::TwoWay,
            },
            seam_loop: loop_c,
            cutouts: vec![],
            darts: vec![],
            internal_lines: vec![],
            drill_holes: vec![],
            notches: vec![],
            grade_deltas: vec![],
            visible: true,
            fabric_role: "GarmentCode".into(),
            arrangement_zone: zone,
        });

        cursor_x += (max_x - min_x) + 110.0;
    }

    let mut bindings = Vec::new();
    if let Some(stitches) = pattern.get("stitches").and_then(|v| v.as_array()) {
        for (si, st) in stitches.iter().enumerate() {
            let Some(pair) = st.as_array() else {
                continue;
            };
            if pair.len() < 2 {
                continue;
            }
            let parse_side = |v: &serde_json::Value| -> Option<(String, usize)> {
                let panel = v.get("panel")?.as_str()?.to_string();
                let edge = v.get("edge")?.as_u64()? as usize;
                Some((panel, edge))
            };
            let (Some((pa, ea)), Some((pb, eb))) = (parse_side(&pair[0]), parse_side(&pair[1]))
            else {
                continue;
            };
            let (Some(&(pid_a, eid_a)), Some(&(pid_b, eid_b))) = (
                panel_edge_map.get(&(pa.clone(), ea)),
                panel_edge_map.get(&(pb.clone(), eb)),
            ) else {
                continue;
            };
            let bid = det_id(&format!("garmentcode:stitch:{si}:{pa}:{ea}:{pb}:{eb}"));
            bindings.push(SeamBinding {
                id: bid,
                label: format!("GC Stitch {}: {} #{} <-> {} #{}", si + 1, pa, ea + 1, pb, eb + 1),
                interface_a: SeamInterface {
                    id: det_id(&format!("{bid}:ia")),
                    name: format!("{pa} E{}", ea + 1),
                    edges: vec![EdgeRef {
                        piece_id: pid_a,
                        edge_id: eid_a,
                        dir: 1,
                    }],
                },
                interface_b: SeamInterface {
                    id: det_id(&format!("{bid}:ib")),
                    name: format!("{pb} E{}", eb + 1),
                    edges: vec![EdgeRef {
                        piece_id: pid_b,
                        edge_id: eid_b,
                        dir: -1,
                    }],
                },
                ease: EaseSpec::Exact,
                driver: SeamDriver::MonitorOnly,
                seam_type: SeamType::Plain,
                ease_min_mm: -3.0,
                ease_max_mm: 3.0,
                auto_partner_notches: true,
            });
        }
    }

    if pieces.is_empty() {
        return Err("No valid panels parsed from GarmentCode JSON".into());
    }
    Ok((pieces, bindings))
}

fn parse_gc_curvature(p0: Point2, p1: Point2, curv: Option<&serde_json::Value>) -> CurveSegment {
    let Some(c) = curv else {
        return CurveSegment::Line { end: p1 };
    };
    let edge_vec = p1.sub(p0);
    let perp = Point2::new(-edge_vec.y, edge_vec.x);

    // Format A: [rel_t, rel_h] quadratic relative point
    if let Some(arr) = c.as_array() {
        if arr.len() >= 2 {
            if let (Some(rt), Some(rh)) = (arr[0].as_f64(), arr[1].as_f64()) {
                let q = p0.add(edge_vec.scale(rt)).add(perp.scale(rh));
                return quad_to_cubic(p0, q, p1);
            }
        }
    }

    // Format B: { "type": "quadratic"|"cubic", "params": [[t, h], ...] }
    if let Some(obj) = c.as_object() {
        let params = obj.get("params").and_then(|v| v.as_array());
        if let Some(ps) = params {
            if ps.len() == 1 {
                if let Some(a) = ps[0].as_array() {
                    if let (Some(rt), Some(rh)) = (a.first().and_then(|v| v.as_f64()), a.get(1).and_then(|v| v.as_f64())) {
                        let q = p0.add(edge_vec.scale(rt)).add(perp.scale(rh));
                        return quad_to_cubic(p0, q, p1);
                    }
                }
            } else if ps.len() >= 2 {
                let p_rel = |v: &serde_json::Value| -> Option<Point2> {
                    let a = v.as_array()?;
                    let rt = a.first()?.as_f64()?;
                    let rh = a.get(1)?.as_f64()?;
                    Some(p0.add(edge_vec.scale(rt)).add(perp.scale(rh)))
                };
                if let (Some(c1), Some(c2)) = (p_rel(&ps[0]), p_rel(&ps[1])) {
                    return CurveSegment::Cubic { c1, c2, end: p1 };
                }
            }
        }
    }

    CurveSegment::Line { end: p1 }
}

fn quad_to_cubic(p0: Point2, q: Point2, p1: Point2) -> CurveSegment {
    CurveSegment::Cubic {
        c1: p0.add(q.sub(p0).scale(2.0 / 3.0)),
        c2: p1.add(q.sub(p1).scale(2.0 / 3.0)),
        end: p1,
    }
}
