use crate::blocks::build_template_pieces;
use crate::curve::edge_total_length;
use crate::model::{
    CurveSegment, EvaluatedPiece, GradeMode, PatternPiece, PatternProject, Point2, StackAnchor,
    StitchLink,
};
use crate::offset::evaluate_piece;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradedPieceInstance {
    pub size_name: String,
    pub evaluated: EvaluatedPiece,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossSizeQaCell {
    pub size_name: String,
    pub len_a_mm: f64,
    pub len_b_mm: f64,
    pub ease_mm: f64,
    pub status: String, // "ok" | "tight" | "excess"
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossSizeQaRow {
    pub link_id: String,
    pub label: String,
    pub target_ease_mm: f64,
    pub cells: Vec<CrossSizeQaCell>,
}

/// Proportional Bezier Handle Scaling (Anti-Kink RTW grading):
/// Scales handle lengths by graded chord ratio rho = |d'| / |d| and rotates by chord delta dphi
/// so curves remain smooth without inflection inversions across XS–5XL.
pub fn grade_piece_rtw_proportional(
    base_piece: &PatternPiece,
    target_size: &str,
) -> PatternPiece {
    let mut graded = base_piece.clone();
    let n = graded.seam_loop.edges.len();
    if n == 0 {
        return graded;
    }

    let mut deltas = vec![Point2::ZERO; n];
    for gd in &base_piece.grade_deltas {
        if gd.size_name == target_size && gd.edge_idx < n {
            deltas[gd.edge_idx] = Point2::new(gd.dx, gd.dy);
        }
    }

    let orig_edges = base_piece.seam_loop.edges.clone();
    for i in 0..n {
        let next_i = (i + 1) % n;
        let a = orig_edges[i].start;
        let b = orig_edges[i].end();
        let a_new = a.add(deltas[i]);
        let b_new = b.add(deltas[next_i]);

        graded.seam_loop.edges[i].start = a_new;
        match orig_edges[i].segment {
            CurveSegment::Line { .. } => {
                graded.seam_loop.edges[i].segment = CurveSegment::Line { end: b_new };
            }
            CurveSegment::Cubic { c1, c2, .. } => {
                let d_old = b.sub(a);
                let d_new = b_new.sub(a_new);
                let l_old = d_old.len().max(1e-6);
                let l_new = d_new.len().max(1e-6);
                let rho = (l_new / l_old).clamp(0.4, 2.5);
                let ang_old = d_old.y.atan2(d_old.x);
                let ang_new = d_new.y.atan2(d_new.x);
                let dphi = ang_new - ang_old;
                let (s, c) = dphi.sin_cos();

                let h1 = c1.sub(a);
                let h1_scaled = Point2::new(
                    (h1.x * c - h1.y * s) * rho,
                    (h1.x * s + h1.y * c) * rho,
                );
                let h2 = c2.sub(b);
                let h2_scaled = Point2::new(
                    (h2.x * c - h2.y * s) * rho,
                    (h2.x * s + h2.y * c) * rho,
                );

                graded.seam_loop.edges[i].segment = CurveSegment::Cubic {
                    c1: a_new.add(h1_scaled),
                    c2: b_new.add(h2_scaled),
                    end: b_new,
                };
            }
        }
    }
    graded.seam_loop.sync_continuity();
    graded
}

fn anchor_point(piece: &PatternPiece, anchor: StackAnchor) -> Point2 {
    match anchor {
        StackAnchor::GrainlineMidpoint => piece.grainline.start.lerp(piece.grainline.end, 0.5),
        StackAnchor::BustOrWaistCenter => {
            if let Some(d) = piece.darts.first() {
                d.apex
            } else {
                piece.grainline.start.lerp(piece.grainline.end, 0.5)
            }
        }
        StackAnchor::TopLeftAnchor => {
            let mut min_x = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for e in &piece.seam_loop.edges {
                min_x = min_x.min(e.start.x);
                max_y = max_y.max(e.start.y);
            }
            if min_x.is_finite() {
                Point2::new(min_x, max_y)
            } else {
                Point2::ZERO
            }
        }
    }
}

fn translate_piece(piece: &mut PatternPiece, offset: Point2) {
    for e in &mut piece.seam_loop.edges {
        e.start = e.start.add(offset);
        e.segment.transform(|p| p.add(offset));
    }
    for hole in &mut piece.cutouts {
        for e in &mut hole.edges {
            e.start = e.start.add(offset);
            e.segment.transform(|p| p.add(offset));
        }
    }
    for d in &mut piece.darts {
        d.apex = d.apex.add(offset);
    }
    for l in &mut piece.internal_lines {
        for p in &mut l.pts {
            *p = p.add(offset);
        }
    }
    for dh in &mut piece.drill_holes {
        dh.pos = dh.pos.add(offset);
    }
    piece.grainline.start = piece.grainline.start.add(offset);
    piece.grainline.end = piece.grainline.end.add(offset);
}

/// Evaluates visible graded size nests stacked onto the active base piece anchor.
pub fn evaluate_graded_nest(
    project: &PatternProject,
    active_evaluated: &[EvaluatedPiece],
    stitching: &[StitchLink],
) -> HashMap<String, Vec<EvaluatedPiece>> {
    let mut nest = HashMap::new();
    let active_anchors: HashMap<uuid::Uuid, Point2> = active_evaluated
        .iter()
        .map(|ep| (ep.piece.id, anchor_point(&ep.piece, project.stack_anchor)))
        .collect();

    for size in &project.size_table.visible_sizes {
        if size == &project.size_table.active_size {
            nest.insert(size.clone(), active_evaluated.to_vec());
            continue;
        }
        let m = project
            .size_table
            .measurements_for(size, &project.measurements);
        let (mut pieces, _, _, _) = build_template_pieces(
            &project.template_id,
            &m,
            &project.ease,
            &project.variables,
            &project.ops,
            &project.custom_pieces,
        );

        if project.grade_mode == GradeMode::RuleBasedRtw {
            // Apply RTW proportional handle scaling from base piece grade deltas when defined
            pieces = active_evaluated
                .iter()
                .map(|base| grade_piece_rtw_proportional(&base.piece, size))
                .collect();
        } else if project.grade_mode == GradeMode::HybridDifferential {
            for p in &mut pieces {
                *p = grade_piece_rtw_proportional(p, size);
            }
        }

        for p in &mut pieces {
            if let Some(&target_anchor) = active_anchors.get(&p.id) {
                let curr = anchor_point(p, project.stack_anchor);
                translate_piece(p, target_anchor.sub(curr));
            }
        }

        let evals = pieces
            .iter()
            .map(|p| evaluate_piece(p, stitching, &pieces))
            .collect();
        nest.insert(size.clone(), evals);
    }
    nest
}

/// Computes the Cross-Size Stitching Balance QA Matrix across all sizes in `size_table.sizes`.
pub fn compute_cross_size_qa(project: &PatternProject) -> Vec<CrossSizeQaRow> {
    let mut rows: HashMap<String, CrossSizeQaRow> = HashMap::new();

    for size in &project.size_table.sizes {
        let m = project
            .size_table
            .measurements_for(size, &project.measurements);
        let (pieces, stitching, _, _) = build_template_pieces(
            &project.template_id,
            &m,
            &project.ease,
            &project.variables,
            &project.ops,
            &project.custom_pieces,
        );
        let by_id: HashMap<uuid::Uuid, &PatternPiece> =
            pieces.iter().map(|p| (p.id, p)).collect();

        for link in &stitching.links {
            let (Some(pa), Some(pb)) = (by_id.get(&link.piece_a), by_id.get(&link.piece_b)) else {
                continue;
            };
            let (Some(ea), Some(eb)) = (
                pa.seam_loop.edges.get(link.edge_a),
                pb.seam_loop.edges.get(link.edge_b),
            ) else {
                continue;
            };
            let len_a = edge_total_length(ea);
            let len_b = edge_total_length(eb);
            let ease = len_b - len_a;
            let status = if ease < link.ease_min_mm {
                "tight".to_string()
            } else if ease > link.ease_max_mm {
                "excess".to_string()
            } else {
                "ok".to_string()
            };

            let row = rows
                .entry(link.id.to_string())
                .or_insert_with(|| CrossSizeQaRow {
                    link_id: link.id.to_string(),
                    label: link.label.clone(),
                    target_ease_mm: link.ease_target_mm,
                    cells: Vec::new(),
                });
            row.cells.push(CrossSizeQaCell {
                size_name: size.clone(),
                len_a_mm: len_a,
                len_b_mm: len_b,
                ease_mm: ease,
                status,
            });
        }
    }

    let mut out: Vec<CrossSizeQaRow> = rows.into_values().collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}
