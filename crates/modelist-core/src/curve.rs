use crate::model::{CurveSegment, PolyEdge, Point2, ARC_LEN_TOL_MM};
use kurbo::{CubicBez, ParamCurve, ParamCurveDeriv};

/// 7-point Gauss-Legendre nodes and weights on [0, 1] (mapped from [-1, 1]).
const GL7_NODES: [f64; 7] = [
    0.025446043828620737,
    0.12923440720030278,
    0.2970774243113014,
    0.5,
    0.7029225756886986,
    0.8707655927996972,
    0.9745539561713793,
];
const GL7_WEIGHTS: [f64; 7] = [
    0.06474248308443485,
    0.13985269574463834,
    0.19091502525255947,
    0.2089795918367347,
    0.19091502525255947,
    0.13985269574463834,
    0.06474248308443485,
];

#[inline]
pub fn to_kurbo_cubic(start: Point2, c1: Point2, c2: Point2, end: Point2) -> CubicBez {
    CubicBez::new(
        start.to_kurbo(),
        c1.to_kurbo(),
        c2.to_kurbo(),
        end.to_kurbo(),
    )
}

pub fn eval_edge_point(edge: &PolyEdge, t: f64) -> Point2 {
    let t = t.clamp(0.0, 1.0);
    match edge.segment {
        CurveSegment::Line { end } => edge.start.lerp(end, t),
        CurveSegment::Cubic { c1, c2, end } => {
            Point2::from_kurbo(to_kurbo_cubic(edge.start, c1, c2, end).eval(t))
        }
    }
}

pub fn eval_edge_deriv(edge: &PolyEdge, t: f64) -> Point2 {
    let t = t.clamp(0.0, 1.0);
    match edge.segment {
        CurveSegment::Line { end } => end.sub(edge.start),
        CurveSegment::Cubic { c1, c2, end } => {
            let d = to_kurbo_cubic(edge.start, c1, c2, end).deriv().eval(t);
            Point2::new(d.x, d.y)
        }
    }
}

pub fn eval_edge_tangent(edge: &PolyEdge, t: f64) -> Point2 {
    let d = eval_edge_deriv(edge, t);
    if d.len() > 1e-10 {
        return d.normalized();
    }
    // Fallback near degenerate endpoint derivatives
    let dt = if t < 0.5 { 1e-4 } else { -1e-4 };
    let d2 = eval_edge_point(edge, (t + dt).clamp(0.0, 1.0)).sub(eval_edge_point(edge, t));
    if d2.len() > 1e-10 {
        if dt > 0.0 { d2.normalized() } else { d2.scale(-1.0).normalized() }
    } else {
        edge.end().sub(edge.start).normalized()
    }
}

/// Outward unit normal for a CCW outer loop (right-hand perpendicular to forward tangent).
pub fn eval_edge_outward_normal(edge: &PolyEdge, t: f64) -> Point2 {
    eval_edge_tangent(edge, t).right_normal()
}

fn gl7_cubic_subinterval(cb: &CubicBez, a: f64, b: f64) -> f64 {
    let deriv = cb.deriv();
    let span = b - a;
    if span <= 1e-15 {
        return 0.0;
    }
    let mut sum = 0.0;
    for (&node, &w) in GL7_NODES.iter().zip(GL7_WEIGHTS.iter()) {
        let u = a + span * node;
        let d = deriv.eval(u);
        sum += w * d.x.hypot(d.y);
    }
    sum * span
}

fn adaptive_cubic_arc_length(cb: &CubicBez, a: f64, b: f64, depth: u8) -> f64 {
    let whole = gl7_cubic_subinterval(cb, a, b);
    if depth >= 6 {
        return whole;
    }
    let mid = 0.5 * (a + b);
    let left = gl7_cubic_subinterval(cb, a, mid);
    let right = gl7_cubic_subinterval(cb, mid, b);
    let split = left + right;
    if (split - whole).abs() < 1e-5 {
        split
    } else {
        adaptive_cubic_arc_length(cb, a, mid, depth + 1)
            + adaptive_cubic_arc_length(cb, mid, b, depth + 1)
    }
}

pub fn edge_arc_length_to(edge: &PolyEdge, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return 0.0;
    }
    match edge.segment {
        CurveSegment::Line { end } => edge.start.dist(end) * t,
        CurveSegment::Cubic { c1, c2, end } => {
            let cb = to_kurbo_cubic(edge.start, c1, c2, end);
            adaptive_cubic_arc_length(&cb, 0.0, t, 0)
        }
    }
}

pub fn edge_total_length(edge: &PolyEdge) -> f64 {
    edge_arc_length_to(edge, 1.0)
}

/// Inverts arc length s -> parameter t in [0, 1] using Newton-Raphson + bracketed bisection.
/// Guarantees |L(t) - s| < ARC_LEN_TOL_MM (0.01 mm).
pub fn invert_edge_arc_length(edge: &PolyEdge, s_target: f64) -> f64 {
    let total = edge_total_length(edge);
    if total < 1e-9 || s_target <= 0.0 {
        return 0.0;
    }
    if s_target >= total {
        return 1.0;
    }
    match edge.segment {
        CurveSegment::Line { .. } => (s_target / total).clamp(0.0, 1.0),
        CurveSegment::Cubic { c1, c2, end } => {
            let cb = to_kurbo_cubic(edge.start, c1, c2, end);
            let deriv = cb.deriv();
            let mut lo = 0.0_f64;
            let mut hi = 1.0_f64;
            let mut t = (s_target / total).clamp(0.0, 1.0);
            let tol = ARC_LEN_TOL_MM * 0.25;

            for _ in 0..24 {
                let len = adaptive_cubic_arc_length(&cb, 0.0, t, 0);
                let err = len - s_target;
                if err.abs() <= tol {
                    return t;
                }
                if err < 0.0 {
                    lo = lo.max(t);
                } else {
                    hi = hi.min(t);
                }
                let d = deriv.eval(t);
                let speed = d.x.hypot(d.y);
                let next_t = if speed > 1e-6 {
                    let cand = t - err / speed;
                    if cand > lo && cand < hi {
                        cand
                    } else {
                        0.5 * (lo + hi)
                    }
                } else {
                    0.5 * (lo + hi)
                };
                if (next_t - t).abs() < 1e-11 {
                    return next_t;
                }
                t = next_t;
            }
            0.5 * (lo + hi)
        }
    }
}

pub fn split_edge_at_t(edge: &PolyEdge, t: f64) -> (PolyEdge, PolyEdge) {
    let t = t.clamp(0.0, 1.0);
    let mid = eval_edge_point(edge, t);
    match edge.segment {
        CurveSegment::Line { end } => {
            let e1 = PolyEdge {
                id: crate::model::det_id(&format!("{}:split_a", edge.id)),
                start: edge.start,
                segment: CurveSegment::Line { end: mid },
                sa_width_mm: edge.sa_width_mm,
                corner_rule: edge.corner_rule,
                label: edge.label.clone(),
                hem: edge.hem,
                elastic: edge.elastic,
            };
            let e2 = PolyEdge {
                id: crate::model::det_id(&format!("{}:split_b", edge.id)),
                start: mid,
                segment: CurveSegment::Line { end },
                sa_width_mm: edge.sa_width_mm,
                corner_rule: edge.corner_rule,
                label: edge.label.clone(),
                hem: edge.hem,
                elastic: edge.elastic,
            };
            (e1, e2)
        }
        CurveSegment::Cubic { c1, c2, end } => {
            let cb = to_kurbo_cubic(edge.start, c1, c2, end);
            let left = cb.subsegment(0.0..t);
            let right = cb.subsegment(t..1.0);
            let e1 = PolyEdge {
                id: crate::model::det_id(&format!("{}:split_a", edge.id)),
                start: Point2::from_kurbo(left.p0),
                segment: CurveSegment::Cubic {
                    c1: Point2::from_kurbo(left.p1),
                    c2: Point2::from_kurbo(left.p2),
                    end: Point2::from_kurbo(left.p3),
                },
                sa_width_mm: edge.sa_width_mm,
                corner_rule: edge.corner_rule,
                label: edge.label.clone(),
                hem: edge.hem,
                elastic: edge.elastic,
            };
            let e2 = PolyEdge {
                id: crate::model::det_id(&format!("{}:split_b", edge.id)),
                start: Point2::from_kurbo(right.p0),
                segment: CurveSegment::Cubic {
                    c1: Point2::from_kurbo(right.p1),
                    c2: Point2::from_kurbo(right.p2),
                    end: Point2::from_kurbo(right.p3),
                },
                sa_width_mm: edge.sa_width_mm,
                corner_rule: edge.corner_rule,
                label: edge.label.clone(),
                hem: edge.hem,
                elastic: edge.elastic,
            };
            (e1, e2)
        }
    }
}

pub fn flatten_edge(edge: &PolyEdge, tol_mm: f64) -> Vec<Point2> {
    let tol = tol_mm.max(1e-3);
    match edge.segment {
        CurveSegment::Line { end } => vec![edge.start, end],
        CurveSegment::Cubic { c1, c2, end } => {
            let cb = to_kurbo_cubic(edge.start, c1, c2, end);
            let mut out = vec![edge.start];
            kurbo::flatten(
                std::iter::once(kurbo::PathEl::MoveTo(cb.p0))
                    .chain(std::iter::once(kurbo::PathEl::CurveTo(cb.p1, cb.p2, cb.p3))),
                tol,
                |el| {
                    if let kurbo::PathEl::LineTo(p) = el {
                        out.push(Point2::from_kurbo(p));
                    }
                },
            );
            if out.last().map(|p| p.dist(end) > 1e-4).unwrap_or(true) {
                out.push(end);
            }
            out
        }
    }
}

/// Philip Schneider's Graphics Gems I cubic Bezier curve fitter.
/// Reconstructs clean smooth cubic segments from dense polygonal CAD contours.
pub fn fit_cubic_schneider(pts: &[Point2], max_err_mm: f64) -> Vec<CurveSegment> {
    if pts.len() < 2 {
        return Vec::new();
    }
    if pts.len() == 2 {
        return vec![CurveSegment::Line { end: pts[1] }];
    }
    let t_hat1 = pts[1].sub(pts[0]).normalized();
    let n = pts.len();
    let t_hat2 = pts[n - 2].sub(pts[n - 1]).normalized();
    let mut out = Vec::new();
    fit_cubic_recursive(pts, t_hat1, t_hat2, max_err_mm * max_err_mm, 0, &mut out);
    out
}

fn chord_parameterize(pts: &[Point2]) -> Vec<f64> {
    let mut u = Vec::with_capacity(pts.len());
    u.push(0.0);
    for i in 1..pts.len() {
        u.push(u[i - 1] + pts[i].dist(pts[i - 1]));
    }
    let total = *u.last().unwrap_or(&1.0);
    if total > 1e-9 {
        for v in &mut u {
            *v /= total;
        }
    } else {
        let denom = (pts.len() - 1).max(1) as f64;
        for (i, v) in u.iter_mut().enumerate() {
            *v = (i as f64) / denom;
        }
    }
    u
}

fn generate_bezier(pts: &[Point2], u: &[f64], t_hat1: Point2, t_hat2: Point2) -> (Point2, Point2) {
    let n = pts.len();
    let first = pts[0];
    let last = pts[n - 1];
    let mut c = [[0.0_f64; 2]; 2];
    let mut x = [0.0_f64; 2];

    for i in 0..n {
        let t = u[i];
        let it = 1.0 - t;
        let b0 = it * it * it;
        let b1 = 3.0 * t * it * it;
        let b2 = 3.0 * t * t * it;
        let b3 = t * t * t;

        let a1 = t_hat1.scale(b1);
        let a2 = t_hat2.scale(b2);
        c[0][0] += a1.dot(a1);
        c[0][1] += a1.dot(a2);
        c[1][0] = c[0][1];
        c[1][1] += a2.dot(a2);

        let tmp = pts[i]
            .sub(first.scale(b0 + b1))
            .sub(last.scale(b2 + b3));
        x[0] += a1.dot(tmp);
        x[1] += a2.dot(tmp);
    }

    let det_c0_c1 = c[0][0] * c[1][1] - c[1][0] * c[0][1];
    let (alpha_l, alpha_r) = if det_c0_c1.abs() > 1e-12 {
        let det_c0_x = c[0][0] * x[1] - c[1][0] * x[0];
        let det_x_c1 = x[0] * c[1][1] - x[1] * c[0][1];
        (det_x_c1 / det_c0_c1, det_c0_x / det_c0_c1)
    } else {
        let c01 = c[0][0] + c[0][1];
        let a = if c01.abs() > 1e-12 { x[0] / c01 } else { 0.0 };
        (a, a)
    };

    let seg_len = first.dist(last);
    let eps = 1e-6 * seg_len;
    if alpha_l < eps || alpha_r < eps {
        let dist = seg_len / 3.0;
        (first.add(t_hat1.scale(dist)), last.add(t_hat2.scale(dist)))
    } else {
        (
            first.add(t_hat1.scale(alpha_l)),
            last.add(t_hat2.scale(alpha_r)),
        )
    }
}

fn max_fit_error_sq(
    pts: &[Point2],
    c1: Point2,
    c2: Point2,
    u: &[f64],
) -> (f64, usize) {
    let cb = to_kurbo_cubic(pts[0], c1, c2, pts[pts.len() - 1]);
    let mut max_sq = 0.0;
    let mut split = pts.len() / 2;
    for (i, &t) in u.iter().enumerate().take(pts.len() - 1).skip(1) {
        let q = Point2::from_kurbo(cb.eval(t));
        let d_sq = pts[i].sub(q).len_sq();
        if d_sq >= max_sq {
            max_sq = d_sq;
            split = i;
        }
    }
    (max_sq, split.clamp(1, pts.len() - 2))
}

fn reparameterize(pts: &[Point2], c1: Point2, c2: Point2, u: &mut [f64]) {
    let cb = to_kurbo_cubic(pts[0], c1, c2, pts[pts.len() - 1]);
    let d1_cb = cb.deriv();
    for i in 1..(pts.len() - 1) {
        let t = u[i];
        let q = Point2::from_kurbo(cb.eval(t));
        let d1 = Point2::new(d1_cb.eval(t).x, d1_cb.eval(t).y);
        // Approximate second derivative using finite difference of d1
        let eps = 1e-4;
        let d1p = d1_cb.eval((t + eps).min(1.0));
        let d1m = d1_cb.eval((t - eps).max(0.0));
        let dt = ((t + eps).min(1.0) - (t - eps).max(0.0)).max(1e-6);
        let d2 = Point2::new((d1p.x - d1m.x) / dt, (d1p.y - d1m.y) / dt);
        let diff = q.sub(pts[i]);
        let num = diff.dot(d1);
        let den = d1.dot(d1) + diff.dot(d2);
        if den.abs() > 1e-9 {
            let next_t = t - num / den;
            if next_t > 0.0 && next_t < 1.0 {
                u[i] = next_t;
            }
        }
    }
}

fn fit_cubic_recursive(
    pts: &[Point2],
    t_hat1: Point2,
    t_hat2: Point2,
    err_sq_tol: f64,
    depth: u8,
    out: &mut Vec<CurveSegment>,
) {
    if pts.len() == 2 {
        out.push(CurveSegment::Line { end: pts[1] });
        return;
    }
    let mut u = chord_parameterize(pts);
    let (mut c1, mut c2) = generate_bezier(pts, &u, t_hat1, t_hat2);
    let (mut max_sq, mut split) = max_fit_error_sq(pts, c1, c2, &u);
    if max_sq <= err_sq_tol {
        out.push(CurveSegment::Cubic {
            c1,
            c2,
            end: pts[pts.len() - 1],
        });
        return;
    }
    if max_sq <= err_sq_tol * 4.0 {
        for _ in 0..4 {
            reparameterize(pts, c1, c2, &mut u);
            let next = generate_bezier(pts, &u, t_hat1, t_hat2);
            c1 = next.0;
            c2 = next.1;
            let res = max_fit_error_sq(pts, c1, c2, &u);
            max_sq = res.0;
            split = res.1;
            if max_sq <= err_sq_tol {
                out.push(CurveSegment::Cubic {
                    c1,
                    c2,
                    end: pts[pts.len() - 1],
                });
                return;
            }
        }
    }
    if depth >= 10 || pts.len() <= 3 {
        out.push(CurveSegment::Cubic {
            c1,
            c2,
            end: pts[pts.len() - 1],
        });
        return;
    }

    let t_center_left = pts[split - 1].sub(pts[split + 1]).normalized();
    let t_center_right = t_center_left.scale(-1.0);
    fit_cubic_recursive(
        &pts[..=split],
        t_hat1,
        t_center_left,
        err_sq_tol,
        depth + 1,
        out,
    );
    fit_cubic_recursive(
        &pts[split..],
        t_center_right,
        t_hat2,
        err_sq_tol,
        depth + 1,
        out,
    );
}
