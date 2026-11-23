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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosedPolyCurve {
    pub edges: Vec<PolyEdge>,
}

impl ClosedPolyCurve {
    pub fn from_edges(edges: Vec<PolyEdge>) -> Self {
        Self { edges }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DartFoldDir {
    TowardCf,
    TowardCb,
    TowardHem,
    TowardSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DartCapStyle {
    FoldedReflection,
    Pointed,
    Cutout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DartEntity {
    pub id: Uuid,
    pub name: String,
    pub apex: Point2,
    pub edge_idx: usize,
    pub leg_a_t: f64,
    pub leg_b_t: f64,
    pub intake_mm: f64,
    pub fold_dir: DartFoldDir,
    pub cap_style: DartCapStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotchStyle {
    Slit,
    V,
    U,
    T,
    Castle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotchAnchor {
    pub id: Uuid,
    pub edge_idx: usize,
    pub param_t: f64,
    pub style: NotchStyle,
    pub count: u8,
    pub depth_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectedNotch {
    pub id: Uuid,
    pub seam_pt: Point2,
    pub cut_pt: Point2,
    pub inward_dir: Point2,
    pub style: NotchStyle,
    pub count: u8,
    pub depth_mm: f64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrainRotationMode {
    OneWay,
    TwoWay,
    Bias,
    Free,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrainlineSpec {
    pub start: Point2,
    pub end: Point2,
    pub rotation_mode: GrainRotationMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillHole {
    pub id: Uuid,
    pub pos: Point2,
    pub radius_mm: f64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InternalLine {
    pub id: Uuid,
    pub label: String,
    pub pts: Vec<Point2>,
    pub dashed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeDelta {
    pub edge_idx: usize,
    pub size_name: String,
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StitchLink {
    pub id: Uuid,
    pub label: String,
    pub piece_a: Uuid,
    pub edge_a: usize,
    pub dir_a: i8,
    pub piece_b: Uuid,
    pub edge_b: usize,
    pub dir_b: i8,
    pub ease_target_mm: f64,
    pub ease_min_mm: f64,
    pub ease_max_mm: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StitchingGraph {
    pub links: Vec<StitchLink>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParametricVar {
    pub name: String,
    pub expr: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradeMode {
    ParametricMtm,
    RuleBasedRtw,
    HybridDifferential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackAnchor {
    GrainlineMidpoint,
    BustOrWaistCenter,
    TopLeftAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HingeMode {
    EdgeHinge,
    ParallelShift,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

impl ParametricOp {
    pub fn id(&self) -> Uuid {
        match *self {
            Self::MoveVertex { id, .. }
            | Self::MoveHandle { id, .. }
            | Self::SetSeamAllowance { id, .. }
            | Self::AddNotch { id, .. }
            | Self::PivotDart { id, .. }
            | Self::SlashSpread { id, .. } => id,
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
            Self::PivotDart { target_edge_idx, angle_deg, .. } => {
                format!("Pivot Dart -> Edge #{target_edge_idx} ({angle_deg:.1}°)")
            }
            Self::SlashSpread { slash_count, spread_mm, hinge_mode, .. } => {
                format!("Slash & Spread ({slash_count}x, {spread_mm:.1} mm, {hinge_mode:?})")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        // Preserve user's custom proportions relative to active size preset
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternProject {
    pub id: Uuid,
    pub name: String,
    pub template_id: String,
    pub measurements: BodyMeasurements,
    pub ease: EaseTable,
    pub variables: Vec<ParametricVar>,
    pub size_table: SizeTable,
    pub grade_mode: GradeMode,
    pub stack_anchor: StackAnchor,
    pub ops: Vec<ParametricOp>,
    pub custom_pieces: Vec<PatternPiece>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedProject {
    pub pieces: Vec<EvaluatedPiece>,
    pub stitching: StitchingGraph,
    pub var_values: HashMap<String, f64>,
    pub var_errors: HashMap<String, String>,
    pub solve_ms: f64,
}
