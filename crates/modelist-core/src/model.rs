use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const GEO_EPSILON: f64 = 1e-6;
pub const ARC_LEN_TOL_MM: f64 = 0.01;
pub const FLATTEN_TOL_MM: f64 = 0.05;

pub const MODELIST_NS: Uuid = Uuid::from_bytes([
    0x6d, 0x6f, 0x64, 0x65, 0x6c, 0x69, 0x73, 0x74, 0x2d, 0x63, 0x61, 0x64, 0x2d, 0x30, 0x31, 0x21,
]);

pub fn det_id(scope: &str) -> Uuid {
    Uuid::new_v5(&MODELIST_NS, scope.as_bytes())
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    #[inline]
    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    #[inline]
    pub fn scale(self, s: f64) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
        }
    }

    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    #[inline]
    pub fn perp_dot(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    #[inline]
    pub fn len_sq(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    #[inline]
    pub fn len(self) -> f64 {
        self.len_sq().sqrt()
    }

    #[inline]
    pub fn dist(self, other: Self) -> f64 {
        self.sub(other).len()
    }

    #[inline]
    pub fn normalized(self) -> Self {
        let l = self.len();
        if l < 1e-12 {
            Self::new(1.0, 0.0)
        } else {
            Self::new(self.x / l, self.y / l)
        }
    }

    #[inline]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }

    #[inline]
    pub fn rotate_around(self, pivot: Self, angle_rad: f64) -> Self {
        let (s, c) = angle_rad.sin_cos();
        let d = self.sub(pivot);
        Self::new(pivot.x + d.x * c - d.y * s, pivot.y + d.x * s + d.y * c)
    }

    #[inline]
    pub fn right_normal(self) -> Self {
        let n = self.normalized();
        Self::new(n.y, -n.x)
    }

    #[inline]
    pub fn to_kurbo(self) -> kurbo::Point {
        kurbo::Point::new(self.x, self.y)
    }

    #[inline]
    pub fn from_kurbo(p: kurbo::Point) -> Self {
        Self::new(p.x, p.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurveSegment {
    Line { end: Point2 },
    Cubic { c1: Point2, c2: Point2, end: Point2 },
}

impl CurveSegment {
    pub fn end_point(&self) -> Point2 {
        match *self {
            Self::Line { end } => end,
            Self::Cubic { end, .. } => end,
        }
    }

    pub fn set_end_point(&mut self, p: Point2) {
        match self {
            Self::Line { end } => *end = p,
            Self::Cubic { end, .. } => *end = p,
        }
    }

    pub fn transform(&mut self, f: impl Fn(Point2) -> Point2) {
        match self {
            Self::Line { end } => *end = f(*end),
            Self::Cubic { c1, c2, end } => {
                *c1 = f(*c1);
                *c2 = f(*c2);
                *end = f(*end);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CornerRule {
    Miter { limit: f64 },
    Bevel,
    Square,
    FoldedHem,
    MatchedSeam,
    DartCap,
}

impl Default for CornerRule {
    fn default() -> Self {
        Self::Miter { limit: 2.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PolyEdge {
    pub id: Uuid,
    pub start: Point2,
    pub segment: CurveSegment,
    pub sa_width_mm: f64,
    pub corner_rule: CornerRule,
    pub label: String,
}

impl PolyEdge {
    pub fn end(&self) -> Point2 {
        self.segment.end_point()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClosedPolyCurve {
    pub edges: Vec<PolyEdge>,
}

impl ClosedPolyCurve {
    pub fn from_edges(edges: Vec<PolyEdge>) -> Self {
        Self { edges }
    }

    pub fn edge_index_by_id(&self, edge_id: Uuid) -> Option<usize> {
        self.edges.iter().position(|e| e.id == edge_id)
    }

    pub fn edge_by_id(&self, edge_id: Uuid) -> Option<&PolyEdge> {
        self.edges.iter().find(|e| e.id == edge_id)
    }

    pub fn edge_by_id_mut(&mut self, edge_id: Uuid) -> Option<&mut PolyEdge> {
        self.edges.iter_mut().find(|e| e.id == edge_id)
    }

    pub fn flatten(&self, tol_mm: f64) -> Vec<Point2> {
        let mut pts = Vec::new();
        for edge in &self.edges {
            let sampled = crate::curve::flatten_edge(edge, tol_mm);
            if pts.is_empty() {
                pts.extend(sampled);
            } else if sampled.len() > 1 {
                pts.extend_from_slice(&sampled[1..]);
            }
        }
        if let (Some(first), Some(last)) = (pts.first().copied(), pts.last().copied()) {
            if first.dist(last) < 1e-4 && pts.len() > 2 {
                pts.pop();
            }
        }
        pts
    }

    pub fn signed_area(&self) -> f64 {
        let pts = self.flatten(FLATTEN_TOL_MM);
        polygon_signed_area(&pts)
    }

    pub fn ensure_ccw(&mut self) {
        if self.signed_area() < 0.0 {
            self.reverse();
        }
    }

    pub fn ensure_cw(&mut self) {
        if self.signed_area() > 0.0 {
            self.reverse();
        }
    }

    pub fn reverse(&mut self) {
        let mut rev = Vec::with_capacity(self.edges.len());
        for e in self.edges.iter().rev() {
            let new_start = e.end();
            let new_seg = match e.segment {
                CurveSegment::Line { .. } => CurveSegment::Line { end: e.start },
                CurveSegment::Cubic { c1, c2, .. } => CurveSegment::Cubic {
                    c1: c2,
                    c2: c1,
                    end: e.start,
                },
            };
            rev.push(PolyEdge {
                id: e.id,
                start: new_start,
                segment: new_seg,
                sa_width_mm: e.sa_width_mm,
                corner_rule: e.corner_rule,
                label: e.label.clone(),
            });
        }
        self.edges = rev;
    }

    pub fn sync_continuity(&mut self) {
        let n = self.edges.len();
        if n == 0 {
            return;
        }
        for i in 0..n {
            let next_start = self.edges[(i + 1) % n].start;
            self.edges[i].segment.set_end_point(next_start);
        }
    }
}

pub fn polygon_signed_area(pts: &[Point2]) -> f64 {
    if pts.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    0.5 * sum
}

pub fn polygon_perimeter(pts: &[Point2]) -> f64 {
    if pts.len() < 2 {
        return 0.0;
    }
    let mut len = 0.0;
    for i in 0..pts.len() {
        len += pts[i].dist(pts[(i + 1) % pts.len()]);
    }
    len
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DartFoldDir {
    TowardCf,
    TowardCb,
    TowardHem,
    TowardSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DartCapStyle {
    FoldedReflection,
    Pointed,
    Cutout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DartEntity {
    pub id: Uuid,
    pub name: String,
    pub apex: Point2,
    pub edge_idx: usize,
    #[serde(default)]
    pub edge_id: Option<Uuid>,
    pub leg_a_t: f64,
    pub leg_b_t: f64,
    pub intake_mm: f64,
    pub fold_dir: DartFoldDir,
    pub cap_style: DartCapStyle,
}

impl DartEntity {
    pub fn resolve_edge_idx(&self, loop_c: &ClosedPolyCurve) -> Option<usize> {
        if let Some(eid) = self.edge_id {
            if let Some(idx) = loop_c.edge_index_by_id(eid) {
                return Some(idx);
            }
        }
        (self.edge_idx < loop_c.edges.len()).then_some(self.edge_idx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotchStyle {
    Slit,
    V,
    U,
    T,
    Castle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotchAnchor {
    pub id: Uuid,
    pub edge_idx: usize,
    #[serde(default)]
    pub edge_id: Option<Uuid>,
    pub param_t: f64,
    pub style: NotchStyle,
    pub count: u8,
    pub depth_mm: f64,
}

impl NotchAnchor {
    pub fn resolve_edge_idx(&self, loop_c: &ClosedPolyCurve) -> Option<usize> {
        if let Some(eid) = self.edge_id {
            if let Some(idx) = loop_c.edge_index_by_id(eid) {
                return Some(idx);
            }
        }
        (self.edge_idx < loop_c.edges.len()).then_some(self.edge_idx)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectedNotch {
    pub id: Uuid,
    pub seam_pt: Point2,
    pub cut_pt: Point2,
    pub inward_dir: Point2,
    pub style: NotchStyle,
    pub count: u8,
    pub depth_mm: f64,
    pub warning: Option<String>,
    #[serde(default)]
    pub is_auto_partner: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GrainRotationMode {
    OneWay,
    TwoWay,
    Bias,
    Free,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GrainlineSpec {
    pub start: Point2,
    pub end: Point2,
    pub rotation_mode: GrainRotationMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DrillHole {
    pub id: Uuid,
    pub pos: Point2,
    pub radius_mm: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InternalLine {
    pub id: Uuid,
    pub label: String,
    pub pts: Vec<Point2>,
    pub dashed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GradeDelta {
    pub edge_idx: usize,
    pub size_name: String,
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArrangementZone {
    #[default]
    FrontTorso,
    BackTorso,
    SleeveLeft,
    SleeveRight,
    FrontSkirt,
    BackSkirt,
    FrontLeg,
    BackLeg,
    FlatCustom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PatternPiece {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub cut_quantity: u16,
    pub mirror_fold_edge: Option<usize>,
    pub grainline: GrainlineSpec,
    pub seam_loop: ClosedPolyCurve,
    pub cutouts: Vec<ClosedPolyCurve>,
    pub darts: Vec<DartEntity>,
    pub internal_lines: Vec<InternalLine>,
    pub drill_holes: Vec<DrillHole>,
    pub notches: Vec<NotchAnchor>,
    pub grade_deltas: Vec<GradeDelta>,
    pub visible: bool,
    pub fabric_role: String,
    #[serde(default)]
    pub arrangement_zone: ArrangementZone,
}

/// Deterministic UUID-based edge reference inside a SeamInterface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EdgeRef {
    pub piece_id: Uuid,
    pub edge_id: Uuid,
    pub dir: i8, // +1 forward along edge, -1 reversed
}

/// Ordered sequence of edges forming a logical connection boundary (GarmentCode-inspired).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SeamInterface {
    pub id: Uuid,
    pub name: String,
    pub edges: Vec<EdgeRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EaseSpec {
    Exact,
    EaseMm { mm: f64 },
    EasePct { pct: f64 },
    Gather { ratio: f64 },
    Pleats { count: u8, depth_mm: f64 },
}

impl Default for EaseSpec {
    fn default() -> Self {
        Self::Exact
    }
}

impl EaseSpec {
    pub fn target_len_from_driver(&self, driver_len: f64) -> f64 {
        match *self {
            Self::Exact => driver_len.max(5.0),
            Self::EaseMm { mm } => (driver_len + mm).max(5.0),
            Self::EasePct { pct } => (driver_len * (1.0 + pct / 100.0)).max(5.0),
            Self::Gather { ratio } => (driver_len * ratio.clamp(1.0, 4.0)).max(5.0),
            Self::Pleats { count, depth_mm } => {
                (driver_len + (count as f64) * 2.0 * depth_mm.max(0.0)).max(5.0)
            }
        }
    }

    pub fn expected_ease_mm(&self, driver_len: f64) -> f64 {
        self.target_len_from_driver(driver_len) - driver_len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SeamDriver {
    #[default]
    DriverA,
    DriverB,
    Mutual,
    MonitorOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SeamType {
    #[default]
    Plain,
    Overlock,
    French,
    FlatFelled,
    Gathered,
}

impl SeamType {
    pub fn recommended_sa_mm(self) -> f64 {
        match self {
            Self::Plain => 10.0,
            Self::Overlock => 7.0,
            Self::French => 15.0,
            Self::FlatFelled => 16.0,
            Self::Gathered => 12.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SeamBinding {
    pub id: Uuid,
    pub label: String,
    pub interface_a: SeamInterface,
    pub interface_b: SeamInterface,
    #[serde(default)]
    pub ease: EaseSpec,
    #[serde(default)]
    pub driver: SeamDriver,
    #[serde(default)]
    pub seam_type: SeamType,
    pub ease_min_mm: f64,
    pub ease_max_mm: f64,
    #[serde(default = "default_true")]
    pub auto_partner_notches: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StitchingGraph {
    pub bindings: Vec<SeamBinding>,
}

fn default_inseam() -> f64 {
    780.0
}
fn default_crotch_depth() -> f64 {
    265.0
}
fn default_thigh() -> f64 {
    580.0
}
fn default_knee() -> f64 {
    385.0
}
fn default_ankle() -> f64 {
    230.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BodyMeasurements {
    pub bust_mm: f64,
    pub waist_mm: f64,
    pub hip_mm: f64,
    pub back_waist_len_mm: f64,
    pub shoulder_width_mm: f64,
    pub armscye_depth_mm: f64,
    pub bicep_mm: f64,
    pub sleeve_len_mm: f64,
    pub wrist_mm: f64,
    pub skirt_len_mm: f64,
    pub waist_to_hip_mm: f64,
    #[serde(default = "default_inseam")]
    pub inseam_mm: f64,
    #[serde(default = "default_crotch_depth")]
    pub crotch_depth_mm: f64,
    #[serde(default = "default_thigh")]
    pub thigh_mm: f64,
    #[serde(default = "default_knee")]
    pub knee_mm: f64,
    #[serde(default = "default_ankle")]
    pub ankle_mm: f64,
}

impl Default for BodyMeasurements {
    fn default() -> Self {
        Self::from_size_preset("M")
    }
}

impl BodyMeasurements {
    pub fn from_size_preset(size: &str) -> Self {
        match size {
            "XS" => Self {
                bust_mm: 840.0,
                waist_mm: 660.0,
                hip_mm: 900.0,
                back_waist_len_mm: 395.0,
                shoulder_width_mm: 375.0,
                armscye_depth_mm: 195.0,
                bicep_mm: 270.0,
                sleeve_len_mm: 580.0,
                wrist_mm: 155.0,
                skirt_len_mm: 560.0,
                waist_to_hip_mm: 195.0,
                inseam_mm: 750.0,
                crotch_depth_mm: 245.0,
                thigh_mm: 520.0,
                knee_mm: 350.0,
                ankle_mm: 210.0,
            },
            "S" => Self {
                bust_mm: 880.0,
                waist_mm: 700.0,
                hip_mm: 940.0,
                back_waist_len_mm: 405.0,
                shoulder_width_mm: 385.0,
                armscye_depth_mm: 205.0,
                bicep_mm: 285.0,
                sleeve_len_mm: 590.0,
                wrist_mm: 160.0,
                skirt_len_mm: 575.0,
                waist_to_hip_mm: 200.0,
                inseam_mm: 765.0,
                crotch_depth_mm: 255.0,
                thigh_mm: 550.0,
                knee_mm: 368.0,
                ankle_mm: 220.0,
            },
            "M" => Self {
                bust_mm: 920.0,
                waist_mm: 740.0,
                hip_mm: 980.0,
                back_waist_len_mm: 415.0,
                shoulder_width_mm: 396.0,
                armscye_depth_mm: 215.0,
                bicep_mm: 300.0,
                sleeve_len_mm: 600.0,
                wrist_mm: 168.0,
                skirt_len_mm: 590.0,
                waist_to_hip_mm: 205.0,
                inseam_mm: 780.0,
                crotch_depth_mm: 265.0,
                thigh_mm: 580.0,
                knee_mm: 385.0,
                ankle_mm: 230.0,
            },
            "L" => Self {
                bust_mm: 980.0,
                waist_mm: 800.0,
                hip_mm: 1040.0,
                back_waist_len_mm: 425.0,
                shoulder_width_mm: 410.0,
                armscye_depth_mm: 226.0,
                bicep_mm: 320.0,
                sleeve_len_mm: 610.0,
                wrist_mm: 176.0,
                skirt_len_mm: 605.0,
                waist_to_hip_mm: 210.0,
                inseam_mm: 790.0,
                crotch_depth_mm: 278.0,
                thigh_mm: 620.0,
                knee_mm: 408.0,
                ankle_mm: 242.0,
            },
            "XL" => Self {
                bust_mm: 1060.0,
                waist_mm: 880.0,
                hip_mm: 1120.0,
                back_waist_len_mm: 432.0,
                shoulder_width_mm: 426.0,
                armscye_depth_mm: 238.0,
                bicep_mm: 345.0,
                sleeve_len_mm: 618.0,
                wrist_mm: 184.0,
                skirt_len_mm: 615.0,
                waist_to_hip_mm: 215.0,
                inseam_mm: 795.0,
                crotch_depth_mm: 290.0,
                thigh_mm: 660.0,
                knee_mm: 430.0,
                ankle_mm: 254.0,
            },
            "XXL" => Self {
                bust_mm: 1140.0,
                waist_mm: 970.0,
                hip_mm: 1200.0,
                back_waist_len_mm: 440.0,
                shoulder_width_mm: 442.0,
                armscye_depth_mm: 250.0,
                bicep_mm: 370.0,
                sleeve_len_mm: 625.0,
                wrist_mm: 192.0,
                skirt_len_mm: 625.0,
                waist_to_hip_mm: 220.0,
                inseam_mm: 800.0,
                crotch_depth_mm: 302.0,
                thigh_mm: 705.0,
                knee_mm: 455.0,
                ankle_mm: 266.0,
            },
            _ => Self::from_size_preset("M"),
        }
    }

    pub fn to_var_map(&self) -> HashMap<String, f64> {
        let mut m = HashMap::new();
        m.insert("bust".into(), self.bust_mm);
        m.insert("waist".into(), self.waist_mm);
        m.insert("hip".into(), self.hip_mm);
        m.insert("back_waist_len".into(), self.back_waist_len_mm);
        m.insert("shoulder_width".into(), self.shoulder_width_mm);
        m.insert("armscye_depth".into(), self.armscye_depth_mm);
        m.insert("bicep".into(), self.bicep_mm);
        m.insert("sleeve_len".into(), self.sleeve_len_mm);
        m.insert("wrist".into(), self.wrist_mm);
        m.insert("skirt_len".into(), self.skirt_len_mm);
        m.insert("waist_to_hip".into(), self.waist_to_hip_mm);
        m.insert("inseam".into(), self.inseam_mm);
        m.insert("crotch_depth".into(), self.crotch_depth_mm);
        m.insert("thigh".into(), self.thigh_mm);
        m.insert("knee".into(), self.knee_mm);
        m.insert("ankle".into(), self.ankle_mm);
        m
    }

    pub fn set_by_key(&mut self, key: &str, val: f64) -> bool {
        let v = val.max(20.0);
        match key {
            "bust" => self.bust_mm = v,
            "waist" => self.waist_mm = v,
            "hip" => self.hip_mm = v,
            "back_waist_len" => self.back_waist_len_mm = v,
            "shoulder_width" => self.shoulder_width_mm = v,
            "armscye_depth" => self.armscye_depth_mm = v,
            "bicep" => self.bicep_mm = v,
            "sleeve_len" => self.sleeve_len_mm = v,
            "wrist" => self.wrist_mm = v,
            "skirt_len" => self.skirt_len_mm = v,
            "waist_to_hip" => self.waist_to_hip_mm = v,
            "inseam" => self.inseam_mm = v,
            "crotch_depth" => self.crotch_depth_mm = v,
            "thigh" => self.thigh_mm = v,
            "knee" => self.knee_mm = v,
            "ankle" => self.ankle_mm = v,
            _ => return false,
        }
        true
    }

    pub fn entries(&self) -> Vec<(&'static str, &'static str, f64, f64, f64)> {
        vec![
            ("bust", "Bust Circumference", self.bust_mm, 720.0, 1450.0),
            ("waist", "Waist Circumference", self.waist_mm, 560.0, 1300.0),
            ("hip", "Hip Circumference", self.hip_mm, 780.0, 1550.0),
            ("back_waist_len", "Back Waist Length", self.back_waist_len_mm, 340.0, 520.0),
            ("shoulder_width", "Shoulder Width", self.shoulder_width_mm, 320.0, 520.0),
            ("armscye_depth", "Armscye Depth", self.armscye_depth_mm, 160.0, 310.0),
            ("bicep", "Bicep Circumference", self.bicep_mm, 220.0, 480.0),
            ("sleeve_len", "Sleeve Length", self.sleeve_len_mm, 450.0, 720.0),
            ("wrist", "Wrist Circumference", self.wrist_mm, 130.0, 260.0),
            ("skirt_len", "Skirt Length", self.skirt_len_mm, 350.0, 1050.0),
            ("waist_to_hip", "Waist to Hip", self.waist_to_hip_mm, 160.0, 260.0),
            ("inseam", "Trouser Inseam", self.inseam_mm, 620.0, 920.0),
            ("crotch_depth", "Crotch Rise Depth", self.crotch_depth_mm, 210.0, 360.0),
            ("thigh", "Thigh Circumference", self.thigh_mm, 440.0, 820.0),
            ("knee", "Knee Circumference", self.knee_mm, 300.0, 540.0),
            ("ankle", "Ankle / Hem Opening", self.ankle_mm, 180.0, 420.0),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EaseTable {
    pub bust_ease_mm: f64,
    pub waist_ease_mm: f64,
    pub hip_ease_mm: f64,
    pub bicep_ease_mm: f64,
    pub armscye_ease_mm: f64,
    pub sleeve_cap_ease_mm: f64,
}

impl Default for EaseTable {
    fn default() -> Self {
        Self {
            bust_ease_mm: 50.0,
            waist_ease_mm: 30.0,
            hip_ease_mm: 45.0,
            bicep_ease_mm: 45.0,
            armscye_ease_mm: 15.0,
            sleeve_cap_ease_mm: 22.0,
        }
    }
}

impl EaseTable {
    pub fn to_var_map(&self) -> HashMap<String, f64> {
        let mut m = HashMap::new();
        m.insert("ease_bust".into(), self.bust_ease_mm);
        m.insert("ease_waist".into(), self.waist_ease_mm);
        m.insert("ease_hip".into(), self.hip_ease_mm);
        m.insert("ease_bicep".into(), self.bicep_ease_mm);
        m.insert("ease_armscye".into(), self.armscye_ease_mm);
        m.insert("ease_cap".into(), self.sleeve_cap_ease_mm);
        m
    }

    pub fn set_by_key(&mut self, key: &str, val: f64) -> bool {
        match key {
            "ease_bust" => self.bust_ease_mm = val,
            "ease_waist" => self.waist_ease_mm = val,
            "ease_hip" => self.hip_ease_mm = val,
            "ease_bicep" => self.bicep_ease_mm = val,
            "ease_armscye" => self.armscye_ease_mm = val,
            "ease_cap" => self.sleeve_cap_ease_mm = val,
            _ => return false,
        }
        true
    }

    pub fn entries(&self) -> Vec<(&'static str, &'static str, f64, f64, f64)> {
        vec![
            ("ease_bust", "Bust Ease", self.bust_ease_mm, -20.0, 200.0),
            ("ease_waist", "Waist Ease", self.waist_ease_mm, -15.0, 180.0),
            ("ease_hip", "Hip Ease", self.hip_ease_mm, 0.0, 220.0),
            ("ease_bicep", "Bicep Ease", self.bicep_ease_mm, 10.0, 140.0),
            ("ease_armscye", "Armscye Drop Ease", self.armscye_ease_mm, 0.0, 60.0),
            ("ease_cap", "Sleeve Cap Ease", self.sleeve_cap_ease_mm, 0.0, 45.0),
        ]
    }
}

fn default_fabric_color() -> String {
    "#335482".into()
}

/// How a fabric is constructed. Drives defaults, grouping in the UI and the stretch behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FabricConstruction {
    #[default]
    Woven,
    Knit,
    Nonwoven,
}

/// Surface finish used by the 3D renderer. It changes the look only, never the physics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FabricFinish {
    #[default]
    Plain,
    Twill,
    Satin,
    Jersey,
    Rib,
    Sheer,
    Terry,
}

/// Physical fabric description consumed by the 3D solver.
///
/// Every mechanical field is a measurable quantity (KES-F / FAST style), so a mill sheet or a
/// lab test can be typed in directly:
/// - tensile stiffness in N/m: force per metre of width per unit strain, in the low-load region;
/// - shear stiffness in N/m: in-plane resistance to bias distortion;
/// - bending rigidity in uN*m (1 gf*cm2/cm = 98 uN*m), measured with the warp or the weft yarns bent;
/// - max stretch in %: strain at which the yarns jam and the fabric locks (wovens 2-5, knits 20-100).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FabricSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub construction: FabricConstruction,
    #[serde(default)]
    pub composition: String,
    pub density_gsm: f64,
    pub thickness_mm: f64,
    #[serde(default)]
    pub stretch_warp_n_m: f64,
    #[serde(default)]
    pub stretch_weft_n_m: f64,
    #[serde(default)]
    pub shear_n_m: f64,
    #[serde(default)]
    pub bend_warp_un_m: f64,
    #[serde(default)]
    pub bend_weft_un_m: f64,
    #[serde(default)]
    pub max_stretch_warp_pct: f64,
    #[serde(default)]
    pub max_stretch_weft_pct: f64,
    /// Resistance to in-plane compression relative to stretching (0.02..1). Lower values let the
    /// fabric gather into soft wrinkles instead of buckling into sharp creases.
    #[serde(default)]
    pub buckling_ratio: f64,
    /// Fabric-on-skin Coulomb friction coefficient.
    #[serde(default)]
    pub friction: f64,
    /// Internal damping, 0 (lively) to 1 (dead).
    #[serde(default)]
    pub damping: f64,
    #[serde(default)]
    pub shrinkage_warp_pct: f64,
    #[serde(default)]
    pub shrinkage_weft_pct: f64,
    #[serde(default = "default_fabric_color")]
    pub color_hex: String,
    #[serde(default)]
    pub finish: FabricFinish,
    // Legacy normalised 0..1 fields from v1 project files. Read once for migration, never written.
    #[serde(default, skip_serializing)]
    pub warp_stiffness: Option<f64>,
    #[serde(default, skip_serializing)]
    pub weft_stiffness: Option<f64>,
    #[serde(default, skip_serializing)]
    pub shear_stiffness: Option<f64>,
    #[serde(default, skip_serializing)]
    pub bend_stiffness: Option<f64>,
    #[serde(default, skip_serializing)]
    pub shrinkage_pct: Option<f64>,
}

/// Editable numeric fabric property: the single source of truth for keys, units and valid ranges.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FabricPropertySpec {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub group: &'static str,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Sliders should move on a logarithmic scale (stiffnesses span several decades).
    pub log_scale: bool,
    pub help: &'static str,
}

pub const FABRIC_PROPERTIES: &[FabricPropertySpec] = &[
    FabricPropertySpec { key: "density_gsm", label: "Weight", unit: "g/m²", group: "Body", min: 20.0, max: 800.0, step: 1.0, log_scale: false, help: "Mass per square metre. Heavier fabrics hang straighter and pull harder on the shoulders." },
    FabricPropertySpec { key: "thickness_mm", label: "Thickness", unit: "mm", group: "Body", min: 0.05, max: 5.0, step: 0.05, log_scale: false, help: "Sets how far the cloth rests from the body and from itself." },
    FabricPropertySpec { key: "stretch_warp_n_m", label: "Stretch stiffness, warp", unit: "N/m", group: "Stretch", min: 10.0, max: 50000.0, step: 1.0, log_scale: true, help: "Force per metre of width for 100% strain along the grainline. Wovens 2,000-15,000; knits 50-600." },
    FabricPropertySpec { key: "stretch_weft_n_m", label: "Stretch stiffness, weft", unit: "N/m", group: "Stretch", min: 10.0, max: 50000.0, step: 1.0, log_scale: true, help: "Same across the grain. Elastane knits are softest here." },
    FabricPropertySpec { key: "shear_n_m", label: "Shear stiffness", unit: "N/m", group: "Stretch", min: 1.0, max: 2000.0, step: 0.5, log_scale: true, help: "Resistance to bias distortion. Low values give the fluid bias drape of silk." },
    FabricPropertySpec { key: "max_stretch_warp_pct", label: "Max stretch, warp", unit: "%", group: "Stretch", min: 0.5, max: 200.0, step: 0.5, log_scale: false, help: "Strain at which the yarns lock. Wovens 2-5%, knits 15-60%." },
    FabricPropertySpec { key: "max_stretch_weft_pct", label: "Max stretch, weft", unit: "%", group: "Stretch", min: 0.5, max: 200.0, step: 0.5, log_scale: false, help: "Same across the grain. 2-way stretch knits reach 60-100%." },
    FabricPropertySpec { key: "bend_warp_un_m", label: "Bending rigidity, warp", unit: "µN·m", group: "Drape", min: 0.1, max: 2000.0, step: 0.1, log_scale: true, help: "Stiffness when the warp yarns bend (KES-B, 1 gf·cm²/cm = 98 µN·m). Silk 1, poplin 6, denim 200." },
    FabricPropertySpec { key: "bend_weft_un_m", label: "Bending rigidity, weft", unit: "µN·m", group: "Drape", min: 0.1, max: 2000.0, step: 0.1, log_scale: true, help: "Stiffness when the weft yarns bend." },
    FabricPropertySpec { key: "buckling_ratio", label: "Buckling ratio", unit: "×", group: "Drape", min: 0.02, max: 1.0, step: 0.01, log_scale: false, help: "Compression resistance relative to stretch. Lower gives softer, rounder wrinkles." },
    FabricPropertySpec { key: "friction", label: "Friction", unit: "μ", group: "Contact", min: 0.0, max: 1.2, step: 0.01, log_scale: false, help: "Fabric-on-skin friction. Silk slides (0.2), cotton grips (0.4), knits cling (0.5)." },
    FabricPropertySpec { key: "damping", label: "Damping", unit: "0-1", group: "Contact", min: 0.0, max: 1.0, step: 0.01, log_scale: false, help: "How quickly motion dies out. Higher settles faster but looks heavier." },
    FabricPropertySpec { key: "shrinkage_warp_pct", label: "Shrinkage, warp", unit: "%", group: "Finishing", min: -5.0, max: 15.0, step: 0.1, log_scale: false, help: "Residual shrinkage applied to the simulated rest length (positive shrinks)." },
    FabricPropertySpec { key: "shrinkage_weft_pct", label: "Shrinkage, weft", unit: "%", group: "Finishing", min: -5.0, max: 15.0, step: 0.1, log_scale: false, help: "Residual shrinkage across the grain." },
];

/// Bending length c = (B / w)^(1/3) (Peirce cantilever), in cm. The classic drape-stiffness index.
pub fn fabric_bending_length_cm(bend_un_m: f64, density_gsm: f64) -> f64 {
    let w = (density_gsm.max(1.0)) * 1e-3 * 9.81; // N/m2
    ((bend_un_m.max(0.0) * 1e-6) / w).cbrt() * 100.0
}

impl Default for FabricSpec {
    fn default() -> Self {
        Self::from_preset("cotton_poplin")
    }
}

#[allow(clippy::too_many_arguments)]
fn fabric_preset(
    id: &str,
    name: &str,
    construction: FabricConstruction,
    composition: &str,
    finish: FabricFinish,
    density_gsm: f64,
    thickness_mm: f64,
    stretch: (f64, f64),
    shear_n_m: f64,
    bend: (f64, f64),
    max_stretch: (f64, f64),
    buckling_ratio: f64,
    friction: f64,
    damping: f64,
    shrinkage: (f64, f64),
    color_hex: &str,
) -> FabricSpec {
    FabricSpec {
        id: id.into(),
        name: name.into(),
        construction,
        composition: composition.into(),
        density_gsm,
        thickness_mm,
        stretch_warp_n_m: stretch.0,
        stretch_weft_n_m: stretch.1,
        shear_n_m,
        bend_warp_un_m: bend.0,
        bend_weft_un_m: bend.1,
        max_stretch_warp_pct: max_stretch.0,
        max_stretch_weft_pct: max_stretch.1,
        buckling_ratio,
        friction,
        damping,
        shrinkage_warp_pct: shrinkage.0,
        shrinkage_weft_pct: shrinkage.1,
        color_hex: color_hex.into(),
        finish,
        warp_stiffness: None,
        weft_stiffness: None,
        shear_stiffness: None,
        bend_stiffness: None,
        shrinkage_pct: None,
    }
}

/// Preset ids in catalog order (wovens first, then knits).
pub const FABRIC_PRESET_IDS: &[&str] = &[
    "cotton_poplin",
    "linen_plain",
    "cotton_twill",
    "denim_12oz",
    "stretch_denim",
    "wool_suiting",
    "silk_charmeuse",
    "silk_crepe",
    "organza",
    "nylon_taslan",
    "jersey_knit",
    "cotton_jersey_heavy",
    "rib_knit",
    "french_terry",
];

impl FabricSpec {
    pub fn catalog() -> Vec<Self> {
        FABRIC_PRESET_IDS
            .iter()
            .filter_map(|id| Self::preset(id))
            .collect()
    }

    /// Returns the preset for `id`, or the default (cotton poplin) for unknown ids.
    pub fn from_preset(id: &str) -> Self {
        Self::preset(id).unwrap_or_else(|| Self::preset("cotton_poplin").expect("poplin preset"))
    }

    /// Library of starting points. Values are typical KES-F / FAST measurements for each quality.
    pub fn preset(id: &str) -> Option<Self> {
        use FabricConstruction::{Knit, Woven};
        use FabricFinish as F;
        Some(match id {
            "cotton_poplin" => fabric_preset(id, "Cotton Poplin (115 gsm)", Woven, "100% cotton", F::Plain, 115.0, 0.25, (6000.0, 3500.0), 60.0, (7.0, 5.0), (2.5, 4.0), 0.25, 0.40, 0.30, (0.0, 0.0), "#a9c0db"),
            "linen_plain" => fabric_preset(id, "Linen Plain Weave (180 gsm)", Woven, "100% linen", F::Plain, 180.0, 0.40, (8000.0, 6000.0), 80.0, (28.0, 22.0), (2.0, 2.5), 0.40, 0.42, 0.25, (1.0, 1.0), "#d8ccb4"),
            "cotton_twill" => fabric_preset(id, "Cotton Twill (270 gsm)", Woven, "100% cotton", F::Twill, 270.0, 0.60, (9000.0, 5000.0), 90.0, (45.0, 30.0), (2.0, 3.0), 0.30, 0.42, 0.30, (1.5, 1.0), "#6b6a4f"),
            "denim_12oz" => fabric_preset(id, "Rigid Denim 12 oz (405 gsm)", Woven, "100% cotton", F::Twill, 405.0, 1.15, (12000.0, 8000.0), 150.0, (220.0, 140.0), (2.0, 3.0), 0.35, 0.45, 0.35, (1.5, 1.0), "#273c5a"),
            "stretch_denim" => fabric_preset(id, "Stretch Denim 10 oz (340 gsm)", Woven, "98% cotton / 2% elastane", F::Twill, 340.0, 0.90, (9000.0, 700.0), 100.0, (120.0, 70.0), (2.0, 25.0), 0.30, 0.45, 0.35, (1.5, 2.0), "#34507a"),
            "wool_suiting" => fabric_preset(id, "Worsted Wool Suiting (260 gsm)", Woven, "100% wool", F::Twill, 260.0, 0.50, (4000.0, 3000.0), 45.0, (26.0, 19.0), (3.0, 4.0), 0.30, 0.40, 0.35, (0.5, 0.5), "#4c525e"),
            "silk_charmeuse" => fabric_preset(id, "Silk Charmeuse (85 gsm)", Woven, "100% silk", F::Satin, 85.0, 0.18, (2500.0, 1500.0), 15.0, (1.3, 1.0), (3.0, 5.0), 0.15, 0.22, 0.20, (0.0, 0.0), "#2d4c73"),
            "silk_crepe" => fabric_preset(id, "Silk Crepe de Chine (65 gsm)", Woven, "100% silk", F::Plain, 65.0, 0.20, (1800.0, 1200.0), 12.0, (1.4, 1.2), (4.0, 6.0), 0.15, 0.28, 0.20, (0.0, 0.0), "#c98f8a"),
            "organza" => fabric_preset(id, "Silk Organza (45 gsm)", Woven, "100% silk", F::Sheer, 45.0, 0.15, (4000.0, 3500.0), 40.0, (6.5, 6.0), (2.0, 2.5), 0.50, 0.25, 0.25, (0.0, 0.0), "#cfc6b8"),
            "nylon_taslan" => fabric_preset(id, "Nylon Taslan (110 gsm)", Woven, "100% nylon", F::Plain, 110.0, 0.20, (5000.0, 4000.0), 50.0, (8.0, 6.0), (3.0, 3.0), 0.45, 0.25, 0.25, (0.0, 0.0), "#30353b"),
            "jersey_knit" => fabric_preset(id, "Cotton-Elastane Jersey (190 gsm)", Knit, "95% cotton / 5% elastane", F::Jersey, 190.0, 0.55, (260.0, 110.0), 30.0, (3.0, 1.8), (40.0, 70.0), 0.15, 0.50, 0.30, (0.0, 0.0), "#8e4d42"),
            "cotton_jersey_heavy" => fabric_preset(id, "Heavyweight Cotton Jersey (240 gsm)", Knit, "100% cotton", F::Jersey, 240.0, 0.75, (450.0, 220.0), 35.0, (5.5, 3.5), (15.0, 30.0), 0.20, 0.50, 0.30, (0.0, 0.0), "#3a3c40"),
            "rib_knit" => fabric_preset(id, "2x2 Rib Knit (280 gsm)", Knit, "95% cotton / 5% elastane", F::Rib, 280.0, 1.00, (350.0, 70.0), 25.0, (6.0, 2.5), (30.0, 100.0), 0.15, 0.55, 0.30, (0.0, 0.0), "#3a3c40"),
            "french_terry" => fabric_preset(id, "French Terry (330 gsm)", Knit, "100% cotton", F::Terry, 330.0, 1.30, (500.0, 250.0), 40.0, (15.0, 9.0), (15.0, 35.0), 0.20, 0.55, 0.30, (0.0, 0.0), "#8d9096"),
            _ => return None,
        })
    }

    pub fn property(&self, key: &str) -> Option<f64> {
        Some(match key {
            "density_gsm" => self.density_gsm,
            "thickness_mm" => self.thickness_mm,
            "stretch_warp_n_m" => self.stretch_warp_n_m,
            "stretch_weft_n_m" => self.stretch_weft_n_m,
            "shear_n_m" => self.shear_n_m,
            "bend_warp_un_m" => self.bend_warp_un_m,
            "bend_weft_un_m" => self.bend_weft_un_m,
            "max_stretch_warp_pct" => self.max_stretch_warp_pct,
            "max_stretch_weft_pct" => self.max_stretch_weft_pct,
            "buckling_ratio" => self.buckling_ratio,
            "friction" => self.friction,
            "damping" => self.damping,
            "shrinkage_warp_pct" => self.shrinkage_warp_pct,
            "shrinkage_weft_pct" => self.shrinkage_weft_pct,
            _ => return None,
        })
    }

    /// Sets one property, clamped to its valid range. Returns false for unknown keys or NaN.
    pub fn set_property(&mut self, key: &str, value: f64) -> bool {
        let Some(spec) = FABRIC_PROPERTIES.iter().find(|p| p.key == key) else {
            return false;
        };
        if !value.is_finite() {
            return false;
        }
        let v = value.max(spec.min).min(spec.max);
        let slot = match key {
            "density_gsm" => &mut self.density_gsm,
            "thickness_mm" => &mut self.thickness_mm,
            "stretch_warp_n_m" => &mut self.stretch_warp_n_m,
            "stretch_weft_n_m" => &mut self.stretch_weft_n_m,
            "shear_n_m" => &mut self.shear_n_m,
            "bend_warp_un_m" => &mut self.bend_warp_un_m,
            "bend_weft_un_m" => &mut self.bend_weft_un_m,
            "max_stretch_warp_pct" => &mut self.max_stretch_warp_pct,
            "max_stretch_weft_pct" => &mut self.max_stretch_weft_pct,
            "buckling_ratio" => &mut self.buckling_ratio,
            "friction" => &mut self.friction,
            "damping" => &mut self.damping,
            "shrinkage_warp_pct" => &mut self.shrinkage_warp_pct,
            "shrinkage_weft_pct" => &mut self.shrinkage_weft_pct,
            _ => return false,
        };
        *slot = v;
        true
    }

    /// Migrates v1 files (normalised sliders) and clamps every property into its valid range,
    /// so the solver never sees zero, negative or non-finite material data.
    pub fn normalized(mut self) -> Self {
        let has_physics = self.stretch_warp_n_m > 0.0
            && self.stretch_weft_n_m > 0.0
            && self.shear_n_m > 0.0
            && self.bend_warp_un_m > 0.0
            && self.bend_weft_un_m > 0.0;
        if !has_physics {
            let legacy = (
                self.warp_stiffness,
                self.weft_stiffness,
                self.shear_stiffness,
                self.bend_stiffness,
            );
            let base = match Self::preset(&self.id) {
                Some(p) => p,
                None => {
                    // Unknown v1 fabric: map the 0..1 sliders onto physical decades.
                    let s = |x: Option<f64>, d: f64| x.unwrap_or(d).clamp(0.0, 1.0);
                    let (w, f, g, b) = (
                        s(legacy.0, 0.9),
                        s(legacy.1, 0.85),
                        s(legacy.2, 0.45),
                        s(legacy.3, 0.24),
                    );
                    let mut p = Self::from_preset("cotton_poplin");
                    p.stretch_warp_n_m = 50.0 * 300f64.powf(w);
                    p.stretch_weft_n_m = 50.0 * 300f64.powf(f);
                    p.shear_n_m = 5.0 * 40f64.powf(g);
                    p.bend_warp_un_m = 0.5 * 400f64.powf(b);
                    p.bend_weft_un_m = p.bend_warp_un_m * 0.75;
                    let knit = w < 0.6 || f < 0.6;
                    p.construction = if knit {
                        FabricConstruction::Knit
                    } else {
                        FabricConstruction::Woven
                    };
                    p.max_stretch_warp_pct = if knit { 30.0 } else { 3.0 };
                    p.max_stretch_weft_pct = if knit { 50.0 } else { 4.0 };
                    p
                }
            };
            self.construction = base.construction;
            if self.composition.is_empty() {
                self.composition = base.composition.clone();
            }
            self.stretch_warp_n_m = base.stretch_warp_n_m;
            self.stretch_weft_n_m = base.stretch_weft_n_m;
            self.shear_n_m = base.shear_n_m;
            self.bend_warp_un_m = base.bend_warp_un_m;
            self.bend_weft_un_m = base.bend_weft_un_m;
            self.max_stretch_warp_pct = base.max_stretch_warp_pct;
            self.max_stretch_weft_pct = base.max_stretch_weft_pct;
            self.buckling_ratio = base.buckling_ratio;
            self.friction = base.friction;
            self.damping = base.damping;
            self.finish = base.finish;
            if let Some(sh) = self.shrinkage_pct {
                self.shrinkage_warp_pct = sh;
                self.shrinkage_weft_pct = sh;
            }
        }
        self.warp_stiffness = None;
        self.weft_stiffness = None;
        self.shear_stiffness = None;
        self.bend_stiffness = None;
        self.shrinkage_pct = None;

        let fallback = Self::from_preset("cotton_poplin");
        for spec in FABRIC_PROPERTIES {
            let cur = self.property(spec.key).unwrap_or(f64::NAN);
            let v = if cur.is_finite() {
                cur
            } else {
                fallback.property(spec.key).unwrap_or(spec.min)
            };
            self.set_property(spec.key, v);
        }
        let valid_hex = self.color_hex.len() == 7
            && self.color_hex.starts_with('#')
            && self.color_hex[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !valid_hex {
            self.color_hex = default_fabric_color();
        }
        if self.id.trim().is_empty() {
            self.id = "custom".into();
        }
        if self.name.trim().is_empty() {
            self.name = "Custom Fabric".into();
        }
        self
    }

    /// Fabric values exposed to the formula engine as read-only variables (see
    /// [`PatternProject::sync_fabric_variables`]), so patterns can respond to the cloth.
    pub fn formula_inputs(&self) -> Vec<(&'static str, f64, &'static str)> {
        vec![
            ("fabric_weight", self.density_gsm, "Shell fabric weight (g/m²)"),
            ("fabric_thickness", self.thickness_mm, "Shell fabric thickness (mm), e.g. for turn of cloth"),
            ("fabric_stretch_warp", self.max_stretch_warp_pct, "Usable stretch along the grain (%)"),
            ("fabric_stretch_weft", self.max_stretch_weft_pct, "Usable stretch across the grain (%), e.g. for negative ease"),
            ("fabric_shrink_warp", self.shrinkage_warp_pct, "Residual shrinkage along the grain (%)"),
            ("fabric_shrink_weft", self.shrinkage_weft_pct, "Residual shrinkage across the grain (%)"),
        ]
    }
}

/// Fabric assigned to a piece role other than the shell (e.g. "rib", "lining", "contrast").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RoleFabric {
    pub role: String,
    pub fabric: FabricSpec,
}

/// Normalises a piece `fabric_role` ("Shell (interfaced)", "Rib", ...) into a material key.
/// Every flavour of the main fabric maps to `"shell"`.
pub fn fabric_role_key(role: &str) -> String {
    let base = role.split('(').next().unwrap_or("").trim().to_lowercase();
    match base.as_str() {
        "" | "shell" | "self" | "main" | "imported" | "garmentcode" => "shell".into(),
        other => other.to_string(),
    }
}

/// True for interfaced pieces (waistbands, collars, cuffs), which the solver stiffens.
pub fn fabric_role_is_interfaced(role: &str) -> bool {
    role.to_lowercase().contains("interfac")
}

const FABRIC_VAR_DESC_PREFIX: &str = "Fabric (read-only):";

/// True for the read-only formula variables mirrored from the fabric.
pub fn is_fabric_variable(name: &str) -> bool {
    name.starts_with("fabric_")
}

fn format_fabric_value(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ParametricVar {
    pub name: String,
    pub expr: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GradeMode {
    ParametricMtm,
    RuleBasedRtw,
    HybridDifferential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StackAnchor {
    GrainlineMidpoint,
    BustOrWaistCenter,
    TopLeftAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HingeMode {
    EdgeHinge,
    ParallelShift,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ParametricOp {
    MoveVertex {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        dx: f64,
        dy: f64,
    },
    MoveHandle {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        which: u8,
        dx: f64,
        dy: f64,
        lock_g1: bool,
    },
    SetSeamAllowance {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        width_mm: f64,
        corner_rule: CornerRule,
    },
    AddNotch {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        param_t: f64,
        style: NotchStyle,
        count: u8,
        depth_mm: f64,
    },
    AddDrillHole {
        id: Uuid,
        piece_id: Uuid,
        x: f64,
        y: f64,
        label: String,
    },
    SetGrainRotation {
        id: Uuid,
        piece_id: Uuid,
        mode: GrainRotationMode,
    },
    TrueCornerG1 {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        square_90: bool,
    },
    PivotDart {
        id: Uuid,
        piece_id: Uuid,
        dart_id: Uuid,
        target_edge_idx: usize,
        target_t: f64,
        angle_deg: f64,
    },
    SlashSpread {
        id: Uuid,
        piece_id: Uuid,
        start_edge_idx: usize,
        end_edge_idx: usize,
        slash_count: u8,
        spread_mm: f64,
        hinge_mode: HingeMode,
    },
    SplitStyleLine {
        id: Uuid,
        piece_id: Uuid,
        edge_a_idx: usize,
        t_a: f64,
        edge_b_idx: usize,
        t_b: f64,
        label_a: String,
        label_b: String,
    },
    UpdateSeamBinding {
        id: Uuid,
        binding_id: Uuid,
        driver: SeamDriver,
        ease: EaseSpec,
        seam_type: SeamType,
        auto_partner_notches: bool,
    },
    AddSeamBinding {
        id: Uuid,
        binding: SeamBinding,
    },
    ReplaceSeamBinding {
        id: Uuid,
        binding: SeamBinding,
    },
    DeleteSeamBinding {
        id: Uuid,
        binding_id: Uuid,
    },
    MovePiece {
        id: Uuid,
        piece_id: Uuid,
        dx: f64,
        dy: f64,
    },
    ConvertEdgeCurve {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        to_cubic: bool,
    },
    SplitEdgePoint {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        param_t: f64,
    },
    SetEdgeLength {
        id: Uuid,
        piece_id: Uuid,
        edge_idx: usize,
        target_len_mm: f64,
    },
    MirrorPiece {
        id: Uuid,
        piece_id: Uuid,
    },
}

impl ParametricOp {
    pub fn id(&self) -> Uuid {
        match *self {
            Self::MoveVertex { id, .. }
            | Self::MoveHandle { id, .. }
            | Self::SetSeamAllowance { id, .. }
            | Self::AddNotch { id, .. }
            | Self::AddDrillHole { id, .. }
            | Self::SetGrainRotation { id, .. }
            | Self::TrueCornerG1 { id, .. }
            | Self::PivotDart { id, .. }
            | Self::SlashSpread { id, .. }
            | Self::SplitStyleLine { id, .. }
            | Self::UpdateSeamBinding { id, .. }
            | Self::AddSeamBinding { id, .. }
            | Self::ReplaceSeamBinding { id, .. }
            | Self::DeleteSeamBinding { id, .. }
            | Self::MovePiece { id, .. }
            | Self::ConvertEdgeCurve { id, .. }
            | Self::SplitEdgePoint { id, .. }
            | Self::SetEdgeLength { id, .. }
            | Self::MirrorPiece { id, .. } => id,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::MoveVertex { edge_idx, dx, dy, .. } => {
                format!("Move Vertex #{edge_idx} ({dx:+.1}, {dy:+.1} mm)")
            }
            Self::MoveHandle { edge_idx, which, lock_g1, .. } => {
                format!("Curve Handle #{edge_idx}.{which}{}", if *lock_g1 { " [G1]" } else { "" })
            }
            Self::SetSeamAllowance { edge_idx, width_mm, corner_rule, .. } => {
                format!("SA Edge #{edge_idx}: {width_mm:.1} mm ({corner_rule:?})")
            }
            Self::AddNotch { edge_idx, style, count, .. } => {
                format!("Notch Edge #{edge_idx}: {count}x {style:?}")
            }
            Self::AddDrillHole { label, .. } => format!("Drill Hole ({label})"),
            Self::SetGrainRotation { mode, .. } => format!("Grain Mode: {mode:?}"),
            Self::TrueCornerG1 { edge_idx, square_90, .. } => {
                format!(
                    "True Corner #{edge_idx} ({})",
                    if *square_90 { "90° Square" } else { "G1 Smooth" }
                )
            }
            Self::PivotDart { target_edge_idx, angle_deg, .. } => {
                format!("Pivot Dart -> Edge #{target_edge_idx} ({angle_deg:.1}°)")
            }
            Self::SlashSpread { slash_count, spread_mm, hinge_mode, .. } => {
                format!("Slash & Spread ({slash_count}x, {spread_mm:.1} mm, {hinge_mode:?})")
            }
            Self::SplitStyleLine { label_a, label_b, .. } => {
                format!("Style-Line Split ({label_a} / {label_b})")
            }
            Self::UpdateSeamBinding { driver, ease, .. } => {
                format!("Seam Binding ({driver:?}, {ease:?})")
            }
            Self::AddSeamBinding { binding, .. } => {
                format!("Link Seam: {}", binding.label)
            }
            Self::ReplaceSeamBinding { binding, .. } => {
                format!("Edit Seam: {}", binding.label)
            }
            Self::DeleteSeamBinding { .. } => "Delete Seam Binding".into(),
            Self::MovePiece { dx, dy, .. } => {
                format!("Move Piece ({dx:+.0}, {dy:+.0} mm)")
            }
            Self::ConvertEdgeCurve { edge_idx, to_cubic, .. } => {
                format!(
                    "Edge #{edge_idx} -> {}",
                    if *to_cubic { "Cubic Curve" } else { "Straight Line" }
                )
            }
            Self::SplitEdgePoint { edge_idx, param_t, .. } => {
                format!("Insert Point on Edge #{edge_idx} @ {:.0}%", param_t * 100.0)
            }
            Self::SetEdgeLength { edge_idx, target_len_mm, .. } => {
                format!("Set Edge #{edge_idx} Length -> {target_len_mm:.1} mm")
            }
            Self::MirrorPiece { .. } => "Flip / Mirror Piece".into(),
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::MoveVertex { .. } => "move_vertex",
            Self::MoveHandle { .. } => "move_handle",
            Self::SetSeamAllowance { .. } => "set_seam_allowance",
            Self::AddNotch { .. } => "add_notch",
            Self::AddDrillHole { .. } => "add_drill_hole",
            Self::SetGrainRotation { .. } => "set_grain_rotation",
            Self::TrueCornerG1 { .. } => "true_corner_g1",
            Self::PivotDart { .. } => "pivot_dart",
            Self::SlashSpread { .. } => "slash_spread",
            Self::SplitStyleLine { .. } => "split_style_line",
            Self::UpdateSeamBinding { .. } => "update_seam_binding",
            Self::AddSeamBinding { .. } => "add_seam_binding",
            Self::ReplaceSeamBinding { .. } => "replace_seam_binding",
            Self::DeleteSeamBinding { .. } => "delete_seam_binding",
            Self::MovePiece { .. } => "move_piece",
            Self::ConvertEdgeCurve { .. } => "convert_edge_curve",
            Self::SplitEdgePoint { .. } => "split_edge_point",
            Self::SetEdgeLength { .. } => "set_edge_length",
            Self::MirrorPiece { .. } => "mirror_piece",
        }
    }

    pub fn target_piece_ids(&self) -> Vec<Uuid> {
        match self {
            Self::MoveVertex { piece_id, .. }
            | Self::MoveHandle { piece_id, .. }
            | Self::SetSeamAllowance { piece_id, .. }
            | Self::AddNotch { piece_id, .. }
            | Self::AddDrillHole { piece_id, .. }
            | Self::SetGrainRotation { piece_id, .. }
            | Self::TrueCornerG1 { piece_id, .. }
            | Self::PivotDart { piece_id, .. }
            | Self::SlashSpread { piece_id, .. }
            | Self::SplitStyleLine { piece_id, .. }
            | Self::MovePiece { piece_id, .. }
            | Self::ConvertEdgeCurve { piece_id, .. }
            | Self::SplitEdgePoint { piece_id, .. }
            | Self::SetEdgeLength { piece_id, .. }
            | Self::MirrorPiece { piece_id, .. } => vec![*piece_id],
            Self::AddSeamBinding { binding, .. } | Self::ReplaceSeamBinding { binding, .. } => {
                let mut ids = Vec::new();
                for er in binding
                    .interface_a
                    .edges
                    .iter()
                    .chain(binding.interface_b.edges.iter())
                {
                    if !ids.contains(&er.piece_id) {
                        ids.push(er.piece_id);
                    }
                }
                ids
            }
            Self::UpdateSeamBinding { .. } | Self::DeleteSeamBinding { .. } => Vec::new(),
        }
    }

    pub fn target_binding_id(&self) -> Option<Uuid> {
        match self {
            Self::UpdateSeamBinding { binding_id, .. }
            | Self::DeleteSeamBinding { binding_id, .. } => Some(*binding_id),
            Self::AddSeamBinding { binding, .. } | Self::ReplaceSeamBinding { binding, .. } => {
                Some(binding.id)
            }
            _ => None,
        }
    }
}

/// Moves a group of pieces (and the seams between them) into a fresh ID namespace.
///
/// Every entity ID owned by `pieces` (piece, edges, cutout edges, darts, notches, drill holes,
/// internal lines) and every binding / interface ID in `bindings` becomes
/// `det_id("{ns}:{old}")`, so the result is deterministic for a given `ns`. Edge references and
/// dart/notch edge links are rewritten when they point inside the group; references to pieces
/// outside the group are left untouched. Used when inserting content whose deterministic IDs
/// may already exist in the project (re-importing a file, inserting a template twice).
/// Returns the old -> new map.
pub fn remap_entity_ids(
    pieces: &mut [PatternPiece],
    bindings: &mut [SeamBinding],
    ns: &str,
) -> HashMap<Uuid, Uuid> {
    let mut map: HashMap<Uuid, Uuid> = HashMap::new();
    let fresh = |map: &mut HashMap<Uuid, Uuid>, old: Uuid| -> Uuid {
        *map.entry(old)
            .or_insert_with(|| det_id(&format!("{ns}:{old}")))
    };
    for p in pieces.iter_mut() {
        p.id = fresh(&mut map, p.id);
        for e in p.seam_loop.edges.iter_mut() {
            e.id = fresh(&mut map, e.id);
        }
        for c in p.cutouts.iter_mut() {
            for e in c.edges.iter_mut() {
                e.id = fresh(&mut map, e.id);
            }
        }
        for d in p.darts.iter_mut() {
            d.id = fresh(&mut map, d.id);
        }
        for n in p.notches.iter_mut() {
            n.id = fresh(&mut map, n.id);
        }
        for h in p.drill_holes.iter_mut() {
            h.id = fresh(&mut map, h.id);
        }
        for l in p.internal_lines.iter_mut() {
            l.id = fresh(&mut map, l.id);
        }
    }
    // Second pass: links inside the group (edge ids are all mapped by now).
    let relink = |id: &mut Uuid| {
        if let Some(n) = map.get(id) {
            *id = *n;
        }
    };
    for p in pieces.iter_mut() {
        for d in p.darts.iter_mut() {
            if let Some(e) = d.edge_id.as_mut() {
                relink(e);
            }
        }
        for n in p.notches.iter_mut() {
            if let Some(e) = n.edge_id.as_mut() {
                relink(e);
            }
        }
    }
    let mut out = map.clone();
    for b in bindings.iter_mut() {
        let nb = det_id(&format!("{ns}:{}", b.id));
        out.insert(b.id, nb);
        b.id = nb;
        for iface in [&mut b.interface_a, &mut b.interface_b] {
            let ni = det_id(&format!("{ns}:{}", iface.id));
            out.insert(iface.id, ni);
            iface.id = ni;
            for er in iface.edges.iter_mut() {
                relink(&mut er.piece_id);
                relink(&mut er.edge_id);
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SizeTable {
    pub sizes: Vec<String>,
    pub active_size: String,
    pub visible_sizes: Vec<String>,
    pub overrides: HashMap<String, BodyMeasurements>,
}

impl Default for SizeTable {
    fn default() -> Self {
        Self {
            sizes: vec!["XS", "S", "M", "L", "XL", "XXL"]
                .into_iter()
                .map(String::from)
                .collect(),
            active_size: "M".into(),
            visible_sizes: vec!["M".into()],
            overrides: HashMap::new(),
        }
    }
}

impl SizeTable {
    pub fn measurements_for(&self, size: &str, base: &BodyMeasurements) -> BodyMeasurements {
        if size == self.active_size {
            return base.clone();
        }
        if let Some(m) = self.overrides.get(size) {
            return m.clone();
        }
        let preset_active = BodyMeasurements::from_size_preset(&self.active_size);
        let preset_target = BodyMeasurements::from_size_preset(size);
        BodyMeasurements {
            bust_mm: base.bust_mm + (preset_target.bust_mm - preset_active.bust_mm),
            waist_mm: base.waist_mm + (preset_target.waist_mm - preset_active.waist_mm),
            hip_mm: base.hip_mm + (preset_target.hip_mm - preset_active.hip_mm),
            back_waist_len_mm: base.back_waist_len_mm
                + (preset_target.back_waist_len_mm - preset_active.back_waist_len_mm),
            shoulder_width_mm: base.shoulder_width_mm
                + (preset_target.shoulder_width_mm - preset_active.shoulder_width_mm),
            armscye_depth_mm: base.armscye_depth_mm
                + (preset_target.armscye_depth_mm - preset_active.armscye_depth_mm),
            bicep_mm: base.bicep_mm + (preset_target.bicep_mm - preset_active.bicep_mm),
            sleeve_len_mm: base.sleeve_len_mm
                + (preset_target.sleeve_len_mm - preset_active.sleeve_len_mm),
            wrist_mm: base.wrist_mm + (preset_target.wrist_mm - preset_active.wrist_mm),
            skirt_len_mm: base.skirt_len_mm
                + (preset_target.skirt_len_mm - preset_active.skirt_len_mm),
            waist_to_hip_mm: base.waist_to_hip_mm
                + (preset_target.waist_to_hip_mm - preset_active.waist_to_hip_mm),
            inseam_mm: base.inseam_mm + (preset_target.inseam_mm - preset_active.inseam_mm),
            crotch_depth_mm: base.crotch_depth_mm
                + (preset_target.crotch_depth_mm - preset_active.crotch_depth_mm),
            thigh_mm: base.thigh_mm + (preset_target.thigh_mm - preset_active.thigh_mm),
            knee_mm: base.knee_mm + (preset_target.knee_mm - preset_active.knee_mm),
            ankle_mm: base.ankle_mm + (preset_target.ankle_mm - preset_active.ankle_mm),
        }
    }
}

/// Version of the `.modelist` project schema written by this build. Files without the field are
/// version 0 (one garment per file) and are migrated on load.
pub const PROJECT_FORMAT_VERSION: u32 = 1;

fn default_origin() -> Point2 {
    Point2::new(0.0, 0.0)
}

/// One garment of a project (a tee, a vest, trousers): its template, ease, formulas, fabrics,
/// construction history and pieces. Every component of a file is drafted on the same body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GarmentComponent {
    /// Written once when the component is created and saved with it.
    pub id: Uuid,
    pub name: String,
    pub template_id: String,
    /// Namespace for the IDs its template drafts (empty for the first component of a file and
    /// for migrated files, so their IDs stay as they were). See [`remap_entity_ids`].
    #[serde(default)]
    pub id_ns: String,
    pub ease: EaseTable,
    #[serde(default)]
    pub fabric: FabricSpec,
    /// Fabrics for piece roles other than the shell (rib, lining, contrast...).
    #[serde(default)]
    pub role_fabrics: Vec<RoleFabric>,
    pub variables: Vec<ParametricVar>,
    pub ops: Vec<ParametricOp>,
    pub custom_pieces: Vec<PatternPiece>,
    #[serde(default)]
    pub suppressed_ops: Vec<Uuid>,
    #[serde(default)]
    pub rollback_index: Option<usize>,
    /// Garment layer in the 3D fit (0 = next to the body). Tucking a tee into trousers is
    /// giving the tee the lower layer.
    #[serde(default)]
    pub layer: u8,
    /// Where its drafted pieces sit on the 2D canvas, mm.
    #[serde(default = "default_origin")]
    pub offset_2d: Point2,
    /// Shown on the 2D canvas and in exports.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Shown in the 3D fit. A hidden garment is still worn (the others drape over it), it is
    /// only not drawn, so hiding an outer layer reveals what is under it.
    #[serde(default = "default_true")]
    pub visible_3d: bool,
}

impl GarmentComponent {
    /// Fabric used for a piece with the given `fabric_role` (falls back to the shell fabric).
    pub fn fabric_for_role(&self, role: &str) -> &FabricSpec {
        let key = fabric_role_key(role);
        if key != "shell" {
            if let Some(rf) = self.role_fabrics.iter().find(|r| r.role == key) {
                return &rf.fabric;
            }
        }
        &self.fabric
    }

    /// Assigns (or clears with `None`) the fabric of a non-shell role.
    pub fn set_role_fabric(&mut self, role: &str, fabric: Option<FabricSpec>) {
        let key = fabric_role_key(role);
        if key == "shell" {
            if let Some(f) = fabric {
                self.fabric = f.normalized();
            }
            return;
        }
        self.role_fabrics.retain(|r| r.role != key);
        if let Some(f) = fabric {
            self.role_fabrics.push(RoleFabric {
                role: key,
                fabric: f.normalized(),
            });
            self.role_fabrics.sort_by(|a, b| a.role.cmp(&b.role));
        }
    }

    /// Migrates and clamps every fabric of the component (call after loading a file).
    pub fn normalize_fabrics(&mut self) {
        self.fabric = self.fabric.clone().normalized();
        for rf in &mut self.role_fabrics {
            rf.role = fabric_role_key(&rf.role);
            rf.fabric = rf.fabric.clone().normalized();
        }
    }

    /// Mirrors the shell fabric into read-only `fabric_*` formula variables so patterns can be
    /// made parametric on the cloth (negative ease from stretch, shrinkage scaling, turn of cloth).
    /// Returns true when any variable changed.
    pub fn sync_fabric_variables(&mut self) -> bool {
        let mut changed = false;
        for (name, value, desc) in self.fabric.formula_inputs() {
            let expr = format_fabric_value(value);
            let description = format!("{FABRIC_VAR_DESC_PREFIX} {desc}");
            match self.variables.iter_mut().find(|v| v.name == name) {
                Some(v) => {
                    if v.expr != expr || v.description != description {
                        v.expr = expr;
                        v.description = description;
                        changed = true;
                    }
                }
                None => {
                    self.variables.push(ParametricVar {
                        name: name.into(),
                        expr,
                        description,
                    });
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn effective_rollback(&self) -> usize {
        self.rollback_index
            .map(|r| r.min(self.ops.len()))
            .unwrap_or(self.ops.len())
    }

    pub fn is_op_suppressed(&self, id: Uuid) -> bool {
        self.suppressed_ops.contains(&id)
    }

    pub fn active_ops(&self) -> Vec<ParametricOp> {
        let limit = self.effective_rollback();
        self.ops[..limit]
            .iter()
            .filter(|op| !self.suppressed_ops.contains(&op.id()))
            .cloned()
            .collect()
    }

    /// IDs this component stores: ops, bindings added by ops, custom pieces and their edges.
    pub fn stored_ids(&self) -> std::collections::HashSet<Uuid> {
        let mut ids = std::collections::HashSet::new();
        ids.insert(self.id);
        for op in &self.ops {
            ids.insert(op.id());
            if let ParametricOp::AddSeamBinding { binding, .. }
            | ParametricOp::ReplaceSeamBinding { binding, .. } = op
            {
                ids.insert(binding.id);
            }
        }
        for p in &self.custom_pieces {
            ids.insert(p.id);
            ids.extend(p.seam_loop.edges.iter().map(|e| e.id));
        }
        ids
    }
}

/// A `.modelist` project: one body, one size run, and the garments (components) worn on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PatternProject {
    #[serde(default)]
    pub format_version: u32,
    pub id: Uuid,
    pub name: String,
    pub measurements: BodyMeasurements,
    pub size_table: SizeTable,
    pub grade_mode: GradeMode,
    pub stack_anchor: StackAnchor,
    /// Never empty.
    pub components: Vec<GarmentComponent>,
    /// The component edits, the parameters page and the timeline apply to.
    #[serde(default)]
    pub active_component: usize,
}

/// Version 0 file layout: a single garment stored on the project itself.
#[derive(Deserialize, schemars::JsonSchema)]
struct ProjectV0 {
    id: Uuid,
    name: String,
    template_id: String,
    measurements: BodyMeasurements,
    ease: EaseTable,
    #[serde(default)]
    fabric: FabricSpec,
    #[serde(default)]
    role_fabrics: Vec<RoleFabric>,
    variables: Vec<ParametricVar>,
    size_table: SizeTable,
    grade_mode: GradeMode,
    stack_anchor: StackAnchor,
    ops: Vec<ParametricOp>,
    custom_pieces: Vec<PatternPiece>,
    #[serde(default)]
    suppressed_ops: Vec<Uuid>,
    #[serde(default)]
    rollback_index: Option<usize>,
}

impl PatternProject {
    /// Reads a project file of any version (version 0 becomes a single component that keeps
    /// every ID, so its timeline, seams and grade rules still match).
    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let version = value.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if version > PROJECT_FORMAT_VERSION {
            return Err(format!(
                "this file was written by a newer Modelist (format {version}, this build reads up to {PROJECT_FORMAT_VERSION})"
            ));
        }
        let mut project = if value.get("components").is_some() {
            serde_json::from_value::<PatternProject>(value).map_err(|e| e.to_string())?
        } else {
            let v0: ProjectV0 = serde_json::from_value(value).map_err(|e| e.to_string())?;
            PatternProject {
                format_version: PROJECT_FORMAT_VERSION,
                id: v0.id,
                name: v0.name.clone(),
                measurements: v0.measurements,
                size_table: v0.size_table,
                grade_mode: v0.grade_mode,
                stack_anchor: v0.stack_anchor,
                components: vec![GarmentComponent {
                    id: det_id(&format!("component:{}:0", v0.id)),
                    name: v0.name,
                    template_id: v0.template_id,
                    id_ns: String::new(),
                    ease: v0.ease,
                    fabric: v0.fabric,
                    role_fabrics: v0.role_fabrics,
                    variables: v0.variables,
                    ops: v0.ops,
                    custom_pieces: v0.custom_pieces,
                    suppressed_ops: v0.suppressed_ops,
                    rollback_index: v0.rollback_index,
                    layer: 0,
                    offset_2d: default_origin(),
                    visible: true,
                    visible_3d: true,
                }],
                active_component: 0,
            }
        };
        if project.components.is_empty() {
            return Err("the project has no garment".into());
        }
        project.format_version = PROJECT_FORMAT_VERSION;
        project.active_component = project.active_component.min(project.components.len() - 1);
        Ok(project)
    }

    pub fn comp(&self) -> &GarmentComponent {
        &self.components[self.active_component.min(self.components.len() - 1)]
    }

    pub fn comp_mut(&mut self) -> &mut GarmentComponent {
        let i = self.active_component.min(self.components.len() - 1);
        &mut self.components[i]
    }

    pub fn component_index(&self, id: Uuid) -> Option<usize> {
        self.components.iter().position(|c| c.id == id)
    }

    /// Fabric of a role in the active component.
    pub fn fabric_for_role(&self, role: &str) -> &FabricSpec {
        self.comp().fabric_for_role(role)
    }

    pub fn set_role_fabric(&mut self, role: &str, fabric: Option<FabricSpec>) {
        self.comp_mut().set_role_fabric(role, fabric);
    }

    /// Migrates and clamps every fabric of every component (call after loading a file).
    pub fn normalize_fabrics(&mut self) {
        for c in &mut self.components {
            c.normalize_fabrics();
        }
    }

    /// Mirrors each component's shell fabric into its `fabric_*` variables.
    pub fn sync_fabric_variables(&mut self) -> bool {
        let mut changed = false;
        for c in &mut self.components {
            changed |= c.sync_fabric_variables();
        }
        changed
    }

    pub fn effective_rollback(&self) -> usize {
        self.comp().effective_rollback()
    }

    pub fn is_op_suppressed(&self, id: Uuid) -> bool {
        self.comp().is_op_suppressed(id)
    }

    /// Every entity ID the project stores in any component. Template-drafted IDs are not stored;
    /// callers that need them add the evaluated pieces themselves.
    pub fn stored_ids(&self) -> std::collections::HashSet<Uuid> {
        let mut ids = std::collections::HashSet::new();
        for c in &self.components {
            ids.extend(c.stored_ids());
        }
        ids
    }

    /// Active (not rolled back, not suppressed) ops of the active component.
    pub fn active_ops(&self) -> Vec<ParametricOp> {
        self.comp().active_ops()
    }
}

/// Shifts every point of a piece by `(dx, dy)` mm.
pub fn translate_piece(p: &mut PatternPiece, dx: f64, dy: f64) {
    let mv = |q: &mut Point2| {
        q.x += dx;
        q.y += dy;
    };
    for loop_ in std::iter::once(&mut p.seam_loop).chain(p.cutouts.iter_mut()) {
        for e in &mut loop_.edges {
            mv(&mut e.start);
            e.segment.transform(|q| Point2::new(q.x + dx, q.y + dy));
        }
    }
    mv(&mut p.grainline.start);
    mv(&mut p.grainline.end);
    for d in &mut p.darts {
        mv(&mut d.apex);
    }
    for h in &mut p.drill_holes {
        mv(&mut h.pos);
    }
    for l in &mut p.internal_lines {
        for q in &mut l.pts {
            mv(q);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvaluatedPiece {
    pub piece: PatternPiece,
    pub seam_polyline: Vec<Point2>,
    pub cut_polyline: Vec<Point2>,
    pub cutout_cutlines: Vec<Vec<Point2>>,
    pub dart_cap_lines: Vec<Vec<Point2>>,
    pub projected_notches: Vec<ProjectedNotch>,
    pub edge_lengths_mm: Vec<f64>,
    pub perimeter_mm: f64,
    pub area_mm2: f64,
    pub cut_area_mm2: f64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvaluatedProject {
    pub pieces: Vec<EvaluatedPiece>,
    pub stitching: StitchingGraph,
    pub var_values: HashMap<String, f64>,
    pub var_errors: HashMap<String, String>,
    pub solve_ms: f64,
}
