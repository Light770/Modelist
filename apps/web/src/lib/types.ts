export type DisplayUnit = 'mm' | 'cm' | 'in';

export type ThemeMode = 'paper' | 'dark';

export type WorkspaceMode = '2d' | 'split' | '3d';

export type Camera3DPreset = 'front' | 'back' | 'left' | 'right' | 'three_quarter' | 'top' | 'bottom';

export type ToolId =
  | 'select'
  | 'direct_edit'
  | 'sew_segment'
  | 'walk_seam'
  | 'true_corner'
  | 'dart_pivot'
  | 'slash_spread'
  | 'style_split'
  | 'draft_piece'
  | 'notch'
  | 'drill_hole'
  | 'measure'
  | 'pan';

export const SEAM_PALETTE = [
  '#0d9488', // Teal
  '#e11d48', // Rose
  '#7c3aed', // Violet
  '#d97706', // Amber
  '#0284c7', // Sky
  '#16a34a', // Emerald
  '#ea580c', // Orange
  '#4f46e5', // Indigo
];

export const FABRIC_SWATCHES: Array<{ name: string; hex: string }> = [
  { name: 'Raw Muslin', hex: '#e3ddd2' },
  { name: 'Atelier Navy', hex: '#2d4c73' },
  { name: 'Indigo Denim', hex: '#273c5a' },
  { name: 'Charcoal Wool', hex: '#4c525e' },
  { name: 'Terracotta', hex: '#8e4d42' },
  { name: 'Sage Linen', hex: '#5f7363' },
  { name: 'Champagne', hex: '#cfc6b8' },
  { name: 'Obsidian', hex: '#25282f' },
];

export type CornerRule =
  | { kind: 'miter'; limit: number }
  | { kind: 'bevel' }
  | { kind: 'square' }
  | { kind: 'folded_hem' }
  | { kind: 'matched_seam' }
  | { kind: 'dart_cap' };

export type EaseSpec =
  | { kind: 'exact' }
  | { kind: 'ease_mm'; mm: number }
  | { kind: 'ease_pct'; pct: number }
  | { kind: 'gather'; ratio: number }
  | { kind: 'pleats'; count: number; depth_mm: number };

export type SeamDriver = 'driver_a' | 'driver_b' | 'mutual' | 'monitor_only';
export type SeamType = 'plain' | 'overlock' | 'french' | 'flat_felled' | 'gathered';
export type GrainRotationMode = 'one_way' | 'two_way' | 'bias' | 'free';
export type GradeMode = 'parametric_mtm' | 'rule_based_rtw' | 'hybrid_differential';
export type StackAnchor = 'grainline_midpoint' | 'bust_or_waist_center' | 'top_left_anchor';
export type AvatarPose = 'a_pose' | 'arms_raised' | 't_pose';
export type AvatarBodyForm = 'female' | 'neutral' | 'male';
export type FitViewMode = 'material' | 'strain' | 'tension' | 'clearance' | 'xray';

export type FabricConstruction = 'woven' | 'knit' | 'nonwoven';
export type FabricFinish = 'plain' | 'twill' | 'satin' | 'jersey' | 'rib' | 'sheer' | 'terry';

/** Physical fabric description (mirrors `modelist_core::model::FabricSpec`). */
export interface FabricSpec {
  id: string;
  name: string;
  construction: FabricConstruction;
  composition: string;
  density_gsm: number;
  thickness_mm: number;
  stretch_warp_n_m: number;
  stretch_weft_n_m: number;
  shear_n_m: number;
  bend_warp_un_m: number;
  bend_weft_un_m: number;
  max_stretch_warp_pct: number;
  max_stretch_weft_pct: number;
  buckling_ratio: number;
  friction: number;
  damping: number;
  shrinkage_warp_pct: number;
  shrinkage_weft_pct: number;
  color_hex: string;
  finish: FabricFinish;
}

/** Numeric fabric property keys accepted by `kernel.setFabricProperty`. */
export type FabricPropertyKey =
  | 'density_gsm'
  | 'thickness_mm'
  | 'stretch_warp_n_m'
  | 'stretch_weft_n_m'
  | 'shear_n_m'
  | 'bend_warp_un_m'
  | 'bend_weft_un_m'
  | 'max_stretch_warp_pct'
  | 'max_stretch_weft_pct'
  | 'buckling_ratio'
  | 'friction'
  | 'damping'
  | 'shrinkage_warp_pct'
  | 'shrinkage_weft_pct';

/** Editable property metadata (mirrors `FABRIC_PROPERTIES`). */
export interface FabricPropertySpec {
  key: FabricPropertyKey;
  label: string;
  unit: string;
  group: 'Body' | 'Stretch' | 'Drape' | 'Contact' | 'Finishing';
  min: number;
  max: number;
  step: number;
  log_scale: boolean;
  help: string;
}

export interface FabricMetrics {
  bending_length_warp_cm: number;
  bending_length_weft_cm: number;
  drape: 'fluid' | 'soft' | 'medium' | 'crisp' | 'stiff';
}

export interface RoleFabricInfo {
  role: string;
  label: string;
  fabric: FabricSpec | null;
}

export interface DrapeQuality {
  poke_through_count: number;
  max_poke_through_mm: number;
  sharp_crease_pct: number;
  mean_dihedral_deg: number;
  self_intersections: number;
}

export interface MeasurementItem {
  key: string;
  label: string;
  value_mm: number;
  min_mm: number;
  max_mm: number;
}

export interface HemSpec {
  depth_mm: number;
  fold_count: number;
  turn_out?: boolean;
}

export interface ElasticBandSpec {
  target_ratio: number;
  width_mm: number;
  stiffness_n_m: number;
}

export interface EdgeInfo {
  edge_idx: number;
  edge_id: string;
  label: string;
  length_mm: number;
  sa_width_mm: number;
  corner_rule: CornerRule;
  is_cubic: boolean;
  is_fold: boolean;
  hem?: HemSpec | null;
  elastic?: ElasticBandSpec | null;
}

export interface DartInfo {
  id: string;
  name: string;
  edge_idx: number;
  intake_mm: number;
  apex_x: number;
  apex_y: number;
}

export interface PieceSummary {
  id: string;
  name: string;
  code: string;
  cut_quantity: number;
  visible: boolean;
  fabric_role: string;
  grain_rotation: GrainRotationMode;
  has_mirror_fold: boolean;
  perimeter_mm: number;
  area_mm2: number;
  cut_area_mm2: number;
  notch_count: number;
  drill_hole_count: number;
  warnings: string[];
  edges: EdgeInfo[];
  darts: DartInfo[];
}

export interface EdgeRefInfo {
  piece_id: string;
  piece_code: string;
  edge_id: string;
  edge_idx: number;
  edge_label: string;
  length_mm: number;
  dir: 1 | -1;
}

export interface StitchLinkSummary {
  id: string;
  label: string;
  interface_a_name: string;
  interface_b_name: string;
  edge_count_a: number;
  edge_count_b: number;
  edges_a: EdgeRefInfo[];
  edges_b: EdgeRefInfo[];
  len_a_mm: number;
  len_b_mm: number;
  ease_mm: number;
  ease_target_mm: number;
  ease: EaseSpec;
  driver: SeamDriver;
  seam_type: SeamType;
  auto_partner_notches: boolean;
  status: 'ok' | 'tight' | 'excess';
  color_idx: number;
}

export type OpSeverity = 'ok' | 'warning' | 'error';

export interface OpSummary {
  id: string;
  index: number;
  kind: string;
  summary: string;
  piece_ids: string[];
  binding_id: string | null;
  suppressed: boolean;
  after_rollback: boolean;
  reorder_min: number;
  reorder_max: number;
  severity: OpSeverity;
  diagnostic: string | null;
  editable: boolean;
  payload: Record<string, unknown>;
}

export interface VarSummary {
  name: string;
  expr: string;
  description: string;
  value: number | null;
  error: string | null;
}

export interface CrossSizeQaCell {
  size_name: string;
  len_a_mm: number;
  len_b_mm: number;
  ease_mm: number;
  status: 'ok' | 'tight' | 'excess';
}

export interface CrossSizeQaRow {
  link_id: string;
  label: string;
  target_ease_mm: number;
  cells: CrossSizeQaCell[];
}

export interface PomRow {
  pom_name: string;
  values_mm: Record<string, number>;
}

export interface CanvasLabel {
  x: number;
  y: number;
  text: string;
  sub: string;
  piece_id: string;
}

export interface EdgeDimLabel {
  piece_id: string;
  edge_idx: number;
  x: number;
  y: number;
  length_mm: number;
  is_selected_piece: boolean;
  is_selected_edge: boolean;
}

export interface PieceFill {
  piece_id: string;
  selected: boolean;
  coords: number[];
}

export interface UiSnapshot {
  project_name: string;
  template_id: string;
  grade_mode: GradeMode;
  stack_anchor: StackAnchor;
  active_size: string;
  sizes: string[];
  visible_sizes: string[];
  fabric: FabricSpec;
  fabric_catalog: FabricSpec[];
  fabric_edited: boolean;
  fabric_metrics: FabricMetrics;
  role_fabrics: RoleFabricInfo[];
  measurements: MeasurementItem[];
  ease: MeasurementItem[];
  variables: VarSummary[];
  pieces: PieceSummary[];
  stitching_links: StitchLinkSummary[];
  ops: OpSummary[];
  rollback_index: number | null;
  qa_matrix: CrossSizeQaRow[];
  pom_rows: PomRow[];
  labels: CanvasLabel[];
  edge_dims: EdgeDimLabel[];
  piece_fills: PieceFill[];
  selected_piece_id: string | null;
  selected_edge_idx: number | null;
  selected_binding_id: string | null;
  show_mirror_unfold: boolean;
  sim3d_active: boolean;
  can_undo: boolean;
  can_redo: boolean;
  solve_ms: number;
}

export interface SnapHit {
  kind: 'endpoint' | 'dart_apex' | 'midpoint' | 'on_curve' | 'grid';
  point: { x: number; y: number };
  piece_id: string | null;
  edge_idx: number | null;
  param_t: number | null;
  dist_mm: number;
}

export type HitResult =
  | { kind: 'none' }
  | { kind: 'handle'; piece_id: string; edge_idx: number; which: number; point: { x: number; y: number } }
  | { kind: 'vertex'; piece_id: string; edge_idx: number; point: { x: number; y: number } }
  | { kind: 'edge'; piece_id: string; edge_idx: number; param_t: number; point: { x: number; y: number } }
  | { kind: 'piece'; piece_id: string };

export interface SeamWalkFrame {
  link_id: string;
  progress_u: number;
  contact_pt: { x: number; y: number };
  tangent: { x: number; y: number };
  len_a_mm: number;
  len_b_mm: number;
  walked_a_mm: number;
  walked_b_mm: number;
  ease_delta_mm: number;
}

export interface PdfLayerPath {
  size_name: string;
  layer_kind: 'seam' | 'cut' | 'grain' | 'notch' | 'dart';
  pts: Array<{ x: number; y: number }>;
  closed: boolean;
}

export interface PdfLabelItem {
  pos: { x: number; y: number };
  text: string;
}

export interface PdfTileRect {
  row: number;
  col: number;
  x_min: number;
  y_min: number;
  x_max: number;
  y_max: number;
}

export interface PdfLayoutPlan {
  preset: string;
  page_width_mm: number;
  page_height_mm: number;
  margin_mm: number;
  overlap_mm: number;
  bounds_min_x: number;
  bounds_min_y: number;
  bounds_max_x: number;
  bounds_max_y: number;
  tiles: PdfTileRect[];
  paths: PdfLayerPath[];
  labels: PdfLabelItem[];
}

export interface SizeBundleEntry {
  size: string;
  quantity: number;
}

export interface MarkerConfig {
  roll_width_mm: number;
  kerf_mm: number;
  cost_per_meter: number;
  bundle: SizeBundleEntry[];
}

export interface MarkerPlacement {
  piece_code: string;
  piece_name: string;
  size_name: string;
  instance_idx: number;
  mirrored: boolean;
  rotation_deg: number;
  polygon: Array<{ x: number; y: number }>;
  grain_start: { x: number; y: number };
  grain_end: { x: number; y: number };
  label_pos: { x: number; y: number };
  area_mm2: number;
}

export interface MarkerPlan {
  roll_width_mm: number;
  used_length_mm: number;
  total_piece_area_mm2: number;
  efficiency_pct: number;
  waste_pct: number;
  estimated_cost: number;
  piece_count: number;
  unplaced_count: number;
  placements: MarkerPlacement[];
}

export interface Sim3dStats {
  vertex_count: number;
  triangle_count: number;
  sew_pair_count: number;
  step_count: number;
  max_strain_pct: number;
  avg_clearance_mm: number;
  min_clearance_mm: number;
  fabric_name: string;
  pose: AvatarPose;
  body_form: AvatarBodyForm;
  bust_y: number;
  waist_y: number;
  hip_y: number;
  crotch_y: number;
  neck_y: number;
  measured_bust_mm: number;
  measured_waist_mm: number;
  measured_hip_mm: number;
  measured_height_mm: number;
  darts_closed: number;
  motion_mm_s: number;
  solver_mode: string;
  mesh_spacing_mm: number;
  adaptive_mesh: boolean;
  min_edge_mm: number;
  max_edge_mm: number;
  pin_count: number;
  self_collision_enabled: boolean;
  wind_speed_mps: number;
}

export function formatMm(mm: number, unit: DisplayUnit, decimals = 1): string {
  if (unit === 'cm') return `${(mm / 10).toFixed(decimals)} cm`;
  if (unit === 'in') return `${(mm / 25.4).toFixed(decimals + 1)} in`;
  return `${mm.toFixed(decimals)} mm`;
}

export function mmToDisplay(mm: number, unit: DisplayUnit): number {
  if (unit === 'cm') return mm / 10;
  if (unit === 'in') return mm / 25.4;
  return mm;
}

export function displayToMm(val: number, unit: DisplayUnit): number {
  if (unit === 'cm') return val * 10;
  if (unit === 'in') return val * 25.4;
  return val;
}
