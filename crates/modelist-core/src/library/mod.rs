//! Starter pattern library.
//!
//! One catalog (`TEMPLATE_CATALOG`) describes every template the studio can open, so the kernel,
//! the WASM bridge and the UI read the same list instead of hard-coding ids. Style templates
//! (the relaxed-silhouette starters) are drafted by the builder modules below; the fitted
//! foundation blocks stay in `blocks.rs`.
//!
//! Adding a pattern: add a `TemplateInfo` row, a builder module exposing `default_variables()`
//! and `build()`, and register both in `library_default_variables` / `build_library_template`.
//! The library test-suite (`library::tests`) then validates it across the whole size run.

use crate::curve::eval_edge_point;
use crate::model::{
    BodyMeasurements, CornerRule, CurveSegment, EaseSpec, EaseTable, EdgeRef, NotchAnchor,
    NotchStyle, ParametricVar, PatternPiece, Point2, PolyEdge, SeamBinding, SeamDriver,
    SeamInterface, SeamType, StitchingGraph,
};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

pub mod boxy_tee;
pub mod jogger;
pub mod wide_trouser;

#[cfg(test)]
mod tests;

/// Whether a template is a foundation block (sloper) or a ready-to-adapt starter style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateCategory {
    Block,
    Style,
}

/// Body region the 3D Fit Room should frame for this template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRegion {
    Torso,
    Hips,
    Legs,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub category: TemplateCategory,
    /// `top` | `skirt` | `trousers`
    pub garment: &'static str,
    /// `fitted` | `a_line` | `boxy` | `wide` | `barrel`
    pub silhouette: &'static str,
    pub description: &'static str,
    pub tags: &'static [&'static str],
    /// Body measurements this template actually drafts from (shown first in the inspector).
    pub measurement_keys: &'static [&'static str],
    /// Global ease-table keys this template reads. Style templates keep their ease in variables.
    pub ease_keys: &'static [&'static str],
    pub view_region: ViewRegion,
    /// Fabric preset a new project starts with (see `FabricSpec::preset`).
    pub default_fabric: &'static str,
    /// Default fabrics for other piece roles, as `(role, preset)` pairs.
    pub role_fabrics: &'static [(&'static str, &'static str)],
}

pub const DEFAULT_TEMPLATE_ID: &str = "bodice_sleeve";

pub static TEMPLATE_CATALOG: &[TemplateInfo] = &[
    TemplateInfo {
        id: "bodice_sleeve",
        name: "Fitted Bodice & Set-In Sleeve",
        category: TemplateCategory::Block,
        garment: "top",
        silhouette: "fitted",
        description: "Darted torso block with a set-in sleeve. The base for fitted tops and dresses.",
        tags: &["block", "set-in sleeve", "darts"],
        measurement_keys: &[
            "bust",
            "waist",
            "back_waist_len",
            "shoulder_width",
            "armscye_depth",
            "bicep",
            "sleeve_len",
            "wrist",
        ],
        ease_keys: &["ease_bust", "ease_waist", "ease_armscye", "ease_bicep", "ease_cap"],
        view_region: ViewRegion::Torso,
        default_fabric: "cotton_poplin",
        role_fabrics: &[],
    },
    TemplateInfo {
        id: "aline_skirt",
        name: "Tailored A-Line Skirt",
        category: TemplateCategory::Block,
        garment: "skirt",
        silhouette: "a_line",
        description: "Cut-on-fold front and zipped back skirt block with a parametric flare.",
        tags: &["block", "skirt"],
        measurement_keys: &["waist", "hip", "skirt_len", "waist_to_hip"],
        ease_keys: &["ease_waist", "ease_hip"],
        view_region: ViewRegion::Hips,
        default_fabric: "wool_suiting",
        role_fabrics: &[],
    },
    TemplateInfo {
        id: "trouser_block",
        name: "Tailored Trouser Block",
        category: TemplateCategory::Block,
        garment: "trousers",
        silhouette: "fitted",
        description: "Front and back trouser slopers with darts, crease grainlines and knee notches.",
        tags: &["block", "trousers"],
        measurement_keys: &[
            "waist",
            "hip",
            "inseam",
            "crotch_depth",
            "thigh",
            "knee",
            "ankle",
            "waist_to_hip",
        ],
        ease_keys: &["ease_waist", "ease_hip"],
        view_region: ViewRegion::Legs,
        default_fabric: "wool_suiting",
        role_fabrics: &[],
    },
    TemplateInfo {
        id: boxy_tee::TEMPLATE_ID,
        name: "Oversized Drop-Shoulder Tee",
        category: TemplateCategory::Style,
        garment: "top",
        silhouette: "boxy",
        description: "Unisex boxy tee with dropped shoulders, a low-cap sleeve and a neck rib. \
                      The body for oversized tees, sweatshirts and hoodies.",
        tags: &["unisex", "oversized", "drop shoulder", "knit", "streetwear"],
        measurement_keys: &["bust", "back_waist_len", "shoulder_width", "armscye_depth", "bicep"],
        ease_keys: &[],
        view_region: ViewRegion::Torso,
        default_fabric: "cotton_jersey_heavy",
        role_fabrics: &[("rib", "rib_knit")],
    },
    TemplateInfo {
        id: wide_trouser::WIDE_ID,
        name: "Pleated Wide-Leg Trouser",
        category: TemplateCategory::Style,
        garment: "trousers",
        silhouette: "wide",
        description: "Relaxed trouser with two front pleats, a wide straight leg and a straight \
                      waistband.",
        tags: &["unisex", "pleated", "wide leg", "relaxed tailoring"],
        measurement_keys: &["waist", "hip", "inseam", "crotch_depth", "waist_to_hip"],
        ease_keys: &[],
        view_region: ViewRegion::Legs,
        default_fabric: "wool_suiting",
        role_fabrics: &[],
    },
    TemplateInfo {
        id: wide_trouser::BARREL_ID,
        name: "Barrel-Leg Trouser",
        category: TemplateCategory::Style,
        garment: "trousers",
        silhouette: "barrel",
        description: "Curved barrel leg, full at the knee and tapered to the hem, with one front \
                      pleat and a straight waistband.",
        tags: &["unisex", "barrel leg", "curved leg", "trend"],
        measurement_keys: &["waist", "hip", "inseam", "crotch_depth", "waist_to_hip"],
        ease_keys: &[],
        view_region: ViewRegion::Legs,
        default_fabric: "cotton_twill",
        role_fabrics: &[],
    },
    TemplateInfo {
        id: jogger::TEMPLATE_ID,
        name: "Relaxed Jogger Sweatpants",
        category: TemplateCategory::Style,
        garment: "trousers",
        silhouette: "jogger",
        description: "Relaxed fleece sweatpants with an elastic gathered waistband, cuffed rib ankle bands, and finished ankle hems.",
        tags: &["unisex", "jogger", "elastic", "fleece", "cuff"],
        measurement_keys: &["waist", "hip", "inseam", "crotch_depth", "waist_to_hip"],
        ease_keys: &[],
        view_region: ViewRegion::Legs,
        default_fabric: "fleece_sweatshirt",
        role_fabrics: &[("rib", "rib_cuff_elastic")],
    },
];

pub fn template_catalog() -> &'static [TemplateInfo] {
    TEMPLATE_CATALOG
}

pub fn find_template(id: &str) -> Option<&'static TemplateInfo> {
    TEMPLATE_CATALOG.iter().find(|t| t.id == id)
}

/// Maps any requested id onto a catalog id (unknown ids open the default block).
pub fn resolve_template_id(id: &str) -> &'static str {
    find_template(id).map(|t| t.id).unwrap_or(DEFAULT_TEMPLATE_ID)
}

/// Default style variables for library-built templates (`None` for the `blocks.rs` blocks).
pub(crate) fn library_default_variables(template_id: &str) -> Option<Vec<ParametricVar>> {
    match template_id {
        boxy_tee::TEMPLATE_ID => Some(boxy_tee::default_variables()),
        wide_trouser::WIDE_ID => Some(wide_trouser::default_variables(wide_trouser::LegShape::Wide)),
        wide_trouser::BARREL_ID => {
            Some(wide_trouser::default_variables(wide_trouser::LegShape::Barrel))
        }
        jogger::TEMPLATE_ID => Some(jogger::default_variables()),
        _ => None,
    }
}

/// Drafts a library-built template (`None` for the `blocks.rs` blocks).
pub(crate) fn build_library_template(
    template_id: &str,
    m: &BodyMeasurements,
    ease: &EaseTable,
    vals: &HashMap<String, f64>,
) -> Option<(Vec<PatternPiece>, StitchingGraph)> {
    match template_id {
        boxy_tee::TEMPLATE_ID => Some(boxy_tee::build(m, ease, vals)),
        wide_trouser::WIDE_ID => Some(wide_trouser::build(
            wide_trouser::WIDE_ID,
            wide_trouser::LegShape::Wide,
            m,
            ease,
            vals,
        )),
        wide_trouser::BARREL_ID => Some(wide_trouser::build(
            wide_trouser::BARREL_ID,
            wide_trouser::LegShape::Barrel,
            m,
            ease,
            vals,
        )),
        jogger::TEMPLATE_ID => Some(jogger::build(m, ease, vals)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Shared drafting helpers
// ---------------------------------------------------------------------------------------------

pub(crate) const MITER: CornerRule = CornerRule::Miter { limit: 2.5 };

/// Clamp that never panics: non-finite input falls back to `lo`, and inverted bounds return `lo`.
/// Builders run inside WASM, where a panicking `f64::clamp` would trap the whole studio.
pub(crate) fn sclamp(v: f64, lo: f64, hi: f64) -> f64 {
    let v = if v.is_finite() { v } else { lo };
    if hi < lo {
        return lo;
    }
    v.max(lo).min(hi)
}

/// Reads an evaluated DAG variable, falling back when it is missing or non-finite.
pub(crate) fn param(vals: &HashMap<String, f64>, key: &str, fallback: f64) -> f64 {
    match vals.get(key) {
        Some(v) if v.is_finite() => *v,
        _ => fallback,
    }
}

pub(crate) fn var(name: &str, expr: &str, description: &str) -> ParametricVar {
    ParametricVar {
        name: name.into(),
        expr: expr.into(),
        description: description.into(),
    }
}

pub(crate) fn line(
    id: Uuid,
    start: Point2,
    end: Point2,
    sa_width_mm: f64,
    corner_rule: CornerRule,
    label: &str,
) -> PolyEdge {
    PolyEdge {
        id,
        start,
        segment: CurveSegment::Line { end },
        sa_width_mm,
        corner_rule,
        label: label.into(),
        hem: None,
        elastic: None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cubic(
    id: Uuid,
    start: Point2,
    c1: Point2,
    c2: Point2,
    end: Point2,
    sa_width_mm: f64,
    corner_rule: CornerRule,
    label: &str,
) -> PolyEdge {
    PolyEdge {
        id,
        start,
        segment: CurveSegment::Cubic { c1, c2, end },
        sa_width_mm,
        corner_rule,
        label: label.into(),
        hem: None,
        elastic: None,
    }
}

pub(crate) fn notch(
    id: Uuid,
    edge_idx: usize,
    edge: &PolyEdge,
    param_t: f64,
    style: NotchStyle,
    count: u8,
) -> NotchAnchor {
    NotchAnchor {
        id,
        edge_idx,
        edge_id: Some(edge.id),
        param_t: sclamp(param_t, 0.001, 0.999),
        style,
        count,
        depth_mm: 5.0,
    }
}

pub(crate) fn eref(piece_id: Uuid, edge_id: Uuid, dir: i8) -> EdgeRef {
    EdgeRef {
        piece_id,
        edge_id,
        dir,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn binding(
    id: Uuid,
    label: &str,
    iface_a: (Uuid, &str, Vec<EdgeRef>),
    iface_b: (Uuid, &str, Vec<EdgeRef>),
    ease: EaseSpec,
    seam_type: SeamType,
    ease_min_mm: f64,
    ease_max_mm: f64,
) -> SeamBinding {
    SeamBinding {
        id,
        label: label.into(),
        interface_a: SeamInterface {
            id: iface_a.0,
            name: iface_a.1.into(),
            edges: iface_a.2,
        },
        interface_b: SeamInterface {
            id: iface_b.0,
            name: iface_b.1.into(),
            edges: iface_b.2,
        },
        ease,
        driver: SeamDriver::DriverA,
        seam_type,
        ease_min_mm,
        ease_max_mm,
        auto_partner_notches: true,
    }
}

/// Curve parameter where an edge whose `y` changes monotonically crosses height `y`.
/// Used to place knee, hip and pocket notches at true heights rather than guessed parameters.
pub(crate) fn t_at_y(edge: &PolyEdge, y: f64) -> f64 {
    let y0 = eval_edge_point(edge, 0.0).y;
    let y1 = eval_edge_point(edge, 1.0).y;
    if (y1 - y0).abs() < 1e-9 {
        return 0.5;
    }
    let rising = y1 > y0;
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        let ym = eval_edge_point(edge, mid).y;
        if (ym < y) == rising {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Bisection for a monotonically increasing `f` on `[lo, hi]`; returns the clamped bound
/// when `target` lies outside the bracket.
pub(crate) fn solve_increasing(f: impl Fn(f64) -> f64, target: f64, lo: f64, hi: f64) -> f64 {
    if f(lo) >= target {
        return lo;
    }
    if f(hi) <= target {
        return hi;
    }
    let (mut a, mut b) = (lo, hi);
    for _ in 0..80 {
        let mid = 0.5 * (a + b);
        if f(mid) < target {
            a = mid;
        } else {
            b = mid;
        }
    }
    0.5 * (a + b)
}

/// A new garment component drafted from a catalog template.
///
/// `seq` numbers the components ever created in the project (0 for the first one). The first
/// component keeps the template's own IDs; later ones get an ID namespace so the same template
/// can be worn twice. `offset` places its pieces on the 2D canvas.
pub fn new_component(template_id: &str, seq: usize, offset: Point2) -> crate::model::GarmentComponent {
    use crate::model::{det_id, fabric_role_key, FabricSpec, GarmentComponent, RoleFabric};
    let tmpl = resolve_template_id(template_id);
    let info = find_template(tmpl);
    let fabric = info.map(|t| FabricSpec::from_preset(t.default_fabric)).unwrap_or_default();
    let role_fabrics = info
        .map(|t| {
            t.role_fabrics
                .iter()
                .map(|(role, preset)| RoleFabric {
                    role: fabric_role_key(role),
                    fabric: FabricSpec::from_preset(preset),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut comp = GarmentComponent {
        id: det_id(&format!("component:{tmpl}:{seq}")),
        name: info.map(|t| t.name).unwrap_or("Garment").into(),
        template_id: tmpl.into(),
        id_ns: if seq == 0 { String::new() } else { format!("component:{tmpl}:{seq}") },
        ease: EaseTable::default(),
        fabric,
        role_fabrics,
        variables: crate::blocks::build_default_variables(tmpl),
        ops: Vec::new(),
        custom_pieces: Vec::new(),
        suppressed_ops: Vec::new(),
        rollback_index: None,
        layer: 0,
        offset_2d: offset,
        visible: true,
        visible_3d: true,
    };
    comp.sync_fabric_variables();
    comp
}

/// A new project wearing one garment drafted from a catalog template, on the default body.
pub fn new_project(template_id: &str) -> crate::model::PatternProject {
    use crate::model::{det_id, GradeMode, PatternProject, SizeTable, StackAnchor, PROJECT_FORMAT_VERSION};
    let comp = new_component(template_id, 0, Point2::new(0.0, 0.0));
    PatternProject {
        format_version: PROJECT_FORMAT_VERSION,
        id: det_id(&format!("project:{}", comp.template_id)),
        name: comp.name.clone(),
        measurements: BodyMeasurements::default(),
        size_table: SizeTable::default(),
        grade_mode: GradeMode::ParametricMtm,
        stack_anchor: StackAnchor::GrainlineMidpoint,
        components: vec![comp],
        active_component: 0,
    }
}

/// Moves a garment on the canvas: its drafted pieces (through `offset_2d`), its stored custom
/// pieces, and the ops that hold absolute positions.
pub fn shift_component(comp: &mut crate::model::GarmentComponent, dx: f64, dy: f64) {
    use crate::model::ParametricOp;
    comp.offset_2d = Point2::new(comp.offset_2d.x + dx, comp.offset_2d.y + dy);
    for p in &mut comp.custom_pieces {
        crate::model::translate_piece(p, dx, dy);
    }
    for op in &mut comp.ops {
        if let ParametricOp::AddDrillHole { x, y, .. } = op {
            *x += dx;
            *y += dy;
        }
    }
}

/// A copy of a garment with every ID moved to the namespace `ns`, so it can join a project that
/// may already hold the original (a duplicate, or a garment inserted from the library twice).
///
/// Drafted IDs follow from the namespace (`det_id("{ns}:{template id}")`); the IDs the garment
/// stores (its own, ops, custom pieces, seams it added) are renamed the same way, and every
/// reference to them inside the garment is rewritten, so its history still finds its pieces.
pub fn relocate_component(
    comp: &crate::model::GarmentComponent,
    ns: &str,
    measurements: &BodyMeasurements,
) -> crate::model::GarmentComponent {
    use crate::model::det_id;
    // The template's own IDs (no namespace, no ops), and what they are called in `comp`.
    let (originals, stitching, _, _) = crate::blocks::build_template_pieces_in(
        &comp.template_id,
        "",
        Point2::new(0.0, 0.0),
        measurements,
        &comp.ease,
        &comp.variables,
        &[],
        &[],
    );
    let mut drafted: Vec<Uuid> = Vec::new();
    for p in &originals {
        drafted.push(p.id);
        drafted.extend(p.seam_loop.edges.iter().map(|e| e.id));
        drafted.extend(p.cutouts.iter().flat_map(|c| c.edges.iter().map(|e| e.id)));
        drafted.extend(p.darts.iter().map(|d| d.id));
        drafted.extend(p.notches.iter().map(|n| n.id));
        drafted.extend(p.drill_holes.iter().map(|h| h.id));
        drafted.extend(p.internal_lines.iter().map(|l| l.id));
    }
    for b in &stitching.bindings {
        drafted.extend([b.id, b.interface_a.id, b.interface_b.id]);
    }
    let named = |o: Uuid, space: &str| if space.is_empty() { o } else { det_id(&format!("{space}:{o}")) };
    let map: HashMap<Uuid, Uuid> = drafted.iter().map(|&o| (named(o, &comp.id_ns), named(o, ns))).collect();

    fn walk(v: &mut serde_json::Value, f: &dyn Fn(Uuid) -> Uuid) {
        match v {
            serde_json::Value::String(s) => {
                if let Ok(u) = Uuid::parse_str(s) {
                    *s = f(u).to_string();
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(|x| walk(x, f)),
            serde_json::Value::Object(o) => o.values_mut().for_each(|x| walk(x, f)),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(comp).expect("component serializes");
    walk(&mut value, &|u| map.get(&u).copied().unwrap_or_else(|| det_id(&format!("{ns}:{u}"))));
    let mut out: crate::model::GarmentComponent = serde_json::from_value(value).expect("component round-trips");
    out.id = det_id(ns);
    out.id_ns = ns.to_string();
    out
}
