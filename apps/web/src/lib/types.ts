export type DisplayUnit = 'mm' | 'cm' | 'in';

export type ThemeMode = 'paper' | 'dark';

export type ToolId =
  | 'select'
  | 'direct_edit'
  | 'walk_seam'
  | 'dart_pivot'
  | 'slash_spread'
  | 'notch'
  | 'measure'
  | 'pan';

export type CornerRule =
  | { kind: 'miter'; limit: number }
  | { kind: 'bevel' }
  | { kind: 'square' }
  | { kind: 'folded_hem' }
  | { kind: 'matched_seam' }
  | { kind: 'dart_cap' };

export type GradeMode = 'parametric_mtm' | 'rule_based_rtw' | 'hybrid_differential';
export type StackAnchor = 'grainline_midpoint' | 'bust_or_waist_center' | 'top_left_anchor';

export interface MeasurementItem {
  key: string;
  label: string;
  value_mm: number;
  min_mm: number;
  max_mm: number;
}

export interface EdgeInfo {
  edge_idx: number;
  label: string;
  length_mm: number;
  sa_width_mm: number;
  corner_rule: CornerRule;
  is_cubic: boolean;
  is_fold: boolean;
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
  perimeter_mm: number;
  area_mm2: number;
  cut_area_mm2: number;
  notch_count: number;
  warnings: string[];
  edges: EdgeInfo[];
  darts: DartInfo[];
}

export interface StitchLinkSummary {
  id: string;
  label: string;
  piece_a_id: string;
  piece_a_name: string;
  edge_a: number;
  piece_b_id: string;
  piece_b_name: string;
  edge_b: number;
  len_a_mm: number;
  len_b_mm: number;
  ease_mm: number;
  ease_target_mm: number;
  status: 'ok' | 'tight' | 'excess';
}

export interface OpSummary {
  id: string;
  summary: string;
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

export interface UiSnapshot {
  project_name: string;
  template_id: string;
  grade_mode: GradeMode;
  stack_anchor: StackAnchor;
  active_size: string;
  sizes: string[];
  visible_sizes: string[];
  measurements: MeasurementItem[];
  ease: MeasurementItem[];
  variables: VarSummary[];
  pieces: PieceSummary[];
  stitching_links: StitchLinkSummary[];
  ops: OpSummary[];
  qa_matrix: CrossSizeQaRow[];
  pom_rows: PomRow[];
  labels: CanvasLabel[];
  selected_piece_id: string | null;
  selected_edge_idx: number | null;
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
