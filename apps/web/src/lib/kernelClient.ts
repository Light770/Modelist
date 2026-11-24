import initWasm, { ModelistSession, fabric_property_specs_json } from '../wasm-pkg/modelist_wasm.js';
import { saveAutosave } from './localFiles';
import type {
  AvatarBodyForm,
  AvatarPose,
  CornerRule,
  DrapeQuality,
  EaseSpec,
  FabricFinish,
  FabricPropertyKey,
  FabricPropertySpec,
  GrainRotationMode,
  HitResult,
  MarkerConfig,
  MarkerPlan,
  PdfLayoutPlan,
  SeamDriver,
  SeamType,
  SeamWalkFrame,
  Sim3dStats,
  SnapHit,
  UiSnapshot,
} from './types';

let wasmReady: Promise<void> | null = null;
let session: ModelistSession | null = null;
let renderVersion = 1;
let sim3dVersion = 1;
let autosaveTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Open documents (one tab each). Every document owns its own WASM session, so undo history,
 * selection and the 3D simulation stay per document; `session` points at the active one.
 */
interface DocumentSlot {
  id: string;
  session: ModelistSession;
}
const documents: DocumentSlot[] = [];
let activeDocumentId = '';
let documentCounter = 0;

function bumpRender() {
  renderVersion += 1;
  sim3dVersion += 1;
}

function scheduleAutosave() {
  if (!session) return;
  if (autosaveTimer) clearTimeout(autosaveTimer);
  autosaveTimer = setTimeout(() => {
    if (session) {
      saveAutosave(session.serialize_project_json());
    }
  }, 600);
}

async function ensureWasm() {
  if (!wasmReady) {
    wasmReady = initWasm().then(() => undefined);
  }
  await wasmReady;
}

function activate(slot: DocumentSlot) {
  if (session && session !== slot.session) {
    // Only the visible document keeps a 3D simulation allocated.
    session.dispose_sim3d();
  }
  session = slot.session;
  activeDocumentId = slot.id;
  bumpRender();
}

export async function initKernel(templateId = 'bodice_sleeve'): Promise<UiSnapshot> {
  await ensureWasm();
  if (!session) {
    await openDocument(templateId);
  } else {
    session.load_template(templateId);
    bumpRender();
  }
  return getSnapshot();
}

/** Opens a new document tab from a template and makes it active. */
export async function openDocument(templateId: string): Promise<{ id: string; snapshot: UiSnapshot }> {
  await ensureWasm();
  documentCounter += 1;
  const slot: DocumentSlot = { id: `doc-${documentCounter}`, session: new ModelistSession(templateId) };
  documents.push(slot);
  activate(slot);
  return { id: slot.id, snapshot: getSnapshot() };
}

/** Makes an open document active and returns its snapshot. */
export function switchDocument(id: string): UiSnapshot | null {
  const slot = documents.find((d) => d.id === id);
  if (!slot) return null;
  if (slot.id !== activeDocumentId) activate(slot);
  return getSnapshot();
}

/** Closes a document (never the last one). Returns the id now active. */
export function closeDocument(id: string): string | null {
  if (documents.length < 2) return null;
  const idx = documents.findIndex((d) => d.id === id);
  if (idx < 0) return null;
  const [slot] = documents.splice(idx, 1);
  if (slot.id === activeDocumentId) {
    activate(documents[Math.min(idx, documents.length - 1)]);
  }
  slot.session.dispose_sim3d();
  slot.session.free();
  return activeDocumentId;
}

export function getActiveDocumentId(): string {
  return activeDocumentId;
}

export function getSession(): ModelistSession {
  if (!session) throw new Error('WASM Kernel not initialized');
  return session;
}

export function getRenderVersion(): number {
  return renderVersion;
}

export function getSim3dVersion(): number {
  return sim3dVersion;
}

let fabricPropertySpecs: FabricPropertySpec[] | null = null;

/** Editable fabric properties (keys, units, ranges, help), from the kernel. */
export function getFabricPropertySpecs(): FabricPropertySpec[] {
  if (!fabricPropertySpecs) {
    fabricPropertySpecs = JSON.parse(fabric_property_specs_json()) as FabricPropertySpec[];
  }
  return fabricPropertySpecs;
}

export function getSnapshot(): UiSnapshot {
  return JSON.parse(getSession().get_ui_snapshot_json()) as UiSnapshot;
}

export function getStrokeCoords(): Float32Array {
  return getSession().get_stroke_coords();
}

export function getStrokeMeta(): Uint32Array {
  return getSession().get_stroke_meta();
}

export function getHandlePoints(): Float32Array {
  return getSession().get_handle_points();
}

export const kernel = {
  loadTemplate(templateId: string): UiSnapshot {
    getSession().load_template(templateId);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setProjectName(name: string): UiSnapshot {
    getSession().set_project_name(name);
    scheduleAutosave();
    return getSnapshot();
  },

  /** Adds a garment from the template library; it becomes the active garment. */
  addComponent(templateId: string, name = ''): UiSnapshot {
    getSession().add_component(templateId, name);
    bumpRender();
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  removeComponent(componentId: string): UiSnapshot {
    getSession().remove_component(componentId);
    bumpRender();
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  setActiveComponent(componentId: string): UiSnapshot {
    getSession().set_active_component(componentId);
    bumpRender();
    return getSnapshot();
  },

  renameComponent(componentId: string, name: string): UiSnapshot {
    getSession().rename_component(componentId, name);
    scheduleAutosave();
    return getSnapshot();
  },

  /** 3D layer of a garment (0 = next to the body); re-dresses the 3D fit when it is open. */
  setComponentLayer(componentId: string, layer: number): UiSnapshot {
    getSession().set_component_layer(componentId, Math.max(0, Math.min(255, Math.round(layer))));
    bumpRender();
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  /** Moves a garment one place outward (worn over the next one) or inward in wearing order. */
  moveComponentLayer(componentId: string, outward: boolean): UiSnapshot {
    getSession().move_component_layer(componentId, outward);
    bumpRender();
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  /** Shows or hides a garment everywhere (canvas, exports, 3D). */
  setComponentVisible(componentId: string, visible: boolean): UiSnapshot {
    getSession().set_component_visible(componentId, visible);
    bumpRender();
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  /** Shows or hides a garment in the 3D view only (it stays worn). */
  setComponentVisible3d(componentId: string, visible: boolean): UiSnapshot {
    getSession().set_component_visible_3d(componentId, visible);
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  serializeProject(): string {
    return getSession().serialize_project_json();
  },

  loadProjectJson(json: string): UiSnapshot {
    getSession().load_project_json(json);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setMeasurement(key: string, valMm: number): UiSnapshot {
    getSession().set_measurement(key, valMm);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setEase(key: string, valMm: number): UiSnapshot {
    getSession().set_ease(key, valMm);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setFabricPreset(presetId: string): UiSnapshot {
    getSession().set_fabric_preset(presetId);
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  setFabricColor(colorHex: string): UiSnapshot {
    getSession().set_fabric_color(colorHex);
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  /** Edits one physical property of the shell fabric; formulas using fabric_* update too. */
  setFabricProperty(key: FabricPropertyKey, value: number): UiSnapshot {
    getSession().set_fabric_property(key, value);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setFabricFinish(finish: FabricFinish): UiSnapshot {
    getSession().set_fabric_finish(finish);
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  resetFabricToPreset(): UiSnapshot {
    getSession().reset_fabric_to_preset();
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  /** Assigns a preset to a piece role (e.g. "rib"); '' means "same as shell". */
  setRoleFabricPreset(role: string, presetId: string): UiSnapshot {
    getSession().set_role_fabric_preset(role, presetId);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setRoleFabricProperty(role: string, key: FabricPropertyKey, value: number): UiSnapshot {
    getSession().set_role_fabric_property(role, key, value);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setRoleFabricColor(role: string, colorHex: string): UiSnapshot {
    getSession().set_role_fabric_color(role, colorHex);
    sim3dVersion += 1;
    scheduleAutosave();
    return getSnapshot();
  },

  movePiece(pieceId: string, totalDx: number, totalDy: number, commit: boolean) {
    getSession().move_piece(pieceId, totalDx, totalDy, commit);
    bumpRender();
    if (commit) scheduleAutosave();
  },

  convertEdgeCurve(pieceId: string, edgeIdx: number, toCubic: boolean): UiSnapshot {
    getSession().convert_edge_curve(pieceId, edgeIdx, toCubic);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  splitEdgePoint(pieceId: string, edgeIdx: number, paramT: number): UiSnapshot {
    getSession().split_edge_point(pieceId, edgeIdx, paramT);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setEdgeLength(pieceId: string, edgeIdx: number, targetLenMm: number): UiSnapshot {
    getSession().set_edge_length(pieceId, edgeIdx, targetLenMm);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  mirrorPiece(pieceId: string): UiSnapshot {
    getSession().mirror_piece(pieceId);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  duplicatePiece(pieceId: string): UiSnapshot {
    getSession().duplicate_piece(pieceId);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  addRectPiece(name: string, x0: number, y0: number, x1: number, y1: number): UiSnapshot {
    getSession().add_rect_piece(name, x0, y0, x1, y1);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setShowMirrorUnfold(show: boolean): UiSnapshot {
    getSession().set_show_mirror_unfold(show);
    bumpRender();
    return getSnapshot();
  },

  setVariable(name: string, expr: string, description = ''): UiSnapshot {
    getSession().set_variable(name, expr, description);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  deleteVariable(name: string): UiSnapshot {
    getSession().delete_variable(name);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setGradeMode(mode: string): UiSnapshot {
    getSession().set_grade_mode(mode);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setStackAnchor(anchor: string): UiSnapshot {
    getSession().set_stack_anchor(anchor);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setActiveSize(size: string): UiSnapshot {
    getSession().set_active_size(size);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setVisibleSizes(sizes: string[]): UiSnapshot {
    getSession().set_visible_sizes(JSON.stringify(sizes));
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setSelection(pieceId: string | null, edgeIdx: number | null): UiSnapshot {
    getSession().set_selection(pieceId ?? undefined, edgeIdx ?? undefined);
    bumpRender();
    return getSnapshot();
  },

  setSelectedBinding(bindingId: string | null): UiSnapshot {
    getSession().set_selected_binding(bindingId ?? undefined);
    bumpRender();
    return getSnapshot();
  },

  setSeamDraftPreview(
    edgesA: Array<{ piece_id: string; edge_id: string; dir: 1 | -1 }>,
    edgesB: Array<{ piece_id: string; edge_id: string; dir: 1 | -1 }>,
  ) {
    getSession().set_seam_draft_preview(JSON.stringify(edgesA), JSON.stringify(edgesB));
    bumpRender();
  },

  upsertSeamBinding(params: {
    bindingId: string | null;
    label: string;
    edgesA: Array<{ piece_id: string; edge_id: string; dir: 1 | -1 }>;
    edgesB: Array<{ piece_id: string; edge_id: string; dir: 1 | -1 }>;
    driver: SeamDriver;
    ease: EaseSpec;
    seamType: SeamType;
    autoPartnerNotches: boolean;
    syncRecommendedSa: boolean;
  }): { bindingId: string; snapshot: UiSnapshot } {
    const bindingId = getSession().upsert_seam_binding(
      params.bindingId ?? undefined,
      params.label,
      JSON.stringify(params.edgesA),
      JSON.stringify(params.edgesB),
      params.driver,
      JSON.stringify(params.ease),
      params.seamType,
      params.autoPartnerNotches,
      params.syncRecommendedSa,
    );
    bumpRender();
    scheduleAutosave();
    return { bindingId, snapshot: getSnapshot() };
  },

  deleteSeamBinding(bindingId: string): UiSnapshot {
    getSession().delete_seam_binding(bindingId);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  togglePieceVisibility(pieceId: string): UiSnapshot {
    getSession().toggle_piece_visibility(pieceId);
    bumpRender();
    return getSnapshot();
  },

  moveVertex(pieceId: string, edgeIdx: number, totalDx: number, totalDy: number, commit: boolean) {
    getSession().move_vertex(pieceId, edgeIdx, totalDx, totalDy, commit);
    bumpRender();
    if (commit) scheduleAutosave();
  },

  moveHandle(
    pieceId: string,
    edgeIdx: number,
    which: number,
    totalDx: number,
    totalDy: number,
    lockG1: boolean,
    commit: boolean,
  ) {
    getSession().move_handle(pieceId, edgeIdx, which, totalDx, totalDy, lockG1, commit);
    bumpRender();
    if (commit) scheduleAutosave();
  },

  setEdgeSa(pieceId: string, edgeIdx: number, widthMm: number, cornerRule: CornerRule): UiSnapshot {
    getSession().set_edge_sa(pieceId, edgeIdx, widthMm, JSON.stringify(cornerRule));
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setEdgeFinish(
    pieceId: string,
    edgeIdx: number,
    hem: { depth_mm: number; fold_count: number; turn_out?: boolean } | null,
    elastic: { target_ratio: number; width_mm: number; stiffness_n_m: number } | null,
  ): UiSnapshot {
    getSession().set_edge_finish(
      pieceId,
      edgeIdx,
      JSON.stringify(hem),
      JSON.stringify(elastic),
    );
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  addNotch(
    pieceId: string,
    edgeIdx: number,
    paramT: number,
    style: string,
    count: number,
    depthMm = 5,
  ): UiSnapshot {
    getSession().add_notch(pieceId, edgeIdx, paramT, style, count, depthMm);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  addDrillHole(pieceId: string, x: number, y: number, label = 'Drill'): UiSnapshot {
    getSession().add_drill_hole(pieceId, x, y, label);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setGrainRotation(pieceId: string, mode: GrainRotationMode): UiSnapshot {
    getSession().set_grain_rotation(pieceId, mode);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  trueCorner(pieceId: string, edgeIdx: number, square90: boolean): UiSnapshot {
    getSession().true_corner(pieceId, edgeIdx, square90);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  splitStyleLine(
    pieceId: string,
    edgeAIdx: number,
    tA: number,
    edgeBIdx: number,
    tB: number,
    labelA = '',
    labelB = '',
  ): UiSnapshot {
    getSession().split_style_line(pieceId, edgeAIdx, tA, edgeBIdx, tB, labelA, labelB);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  updateSeamBinding(
    bindingId: string,
    driver: SeamDriver,
    ease: EaseSpec,
    seamType: SeamType,
    autoPartnerNotches: boolean,
  ): UiSnapshot {
    getSession().update_seam_binding(
      bindingId,
      driver,
      JSON.stringify(ease),
      seamType,
      autoPartnerNotches,
    );
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  createSeamBinding(
    label: string,
    pieceAId: string,
    edgeAId: string,
    pieceBId: string,
    edgeBId: string,
    driver: SeamDriver,
    ease: EaseSpec,
  ): UiSnapshot {
    getSession().create_seam_binding(
      label,
      pieceAId,
      edgeAId,
      pieceBId,
      edgeBId,
      driver,
      JSON.stringify(ease),
    );
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  autoDetectSeams(): { count: number; snapshot: UiSnapshot } {
    const count = getSession().auto_detect_seams();
    bumpRender();
    if (count > 0) scheduleAutosave();
    return { count, snapshot: getSnapshot() };
  },

  pivotDart(
    pieceId: string,
    dartId: string,
    targetEdgeIdx: number,
    targetT: number,
    angleDeg: number,
  ): UiSnapshot {
    getSession().pivot_dart(pieceId, dartId, targetEdgeIdx, targetT, angleDeg);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  slashAndSpread(
    pieceId: string,
    startEdgeIdx: number,
    endEdgeIdx: number,
    slashCount: number,
    spreadMm: number,
    hingeMode: string,
  ): UiSnapshot {
    getSession().slash_and_spread(
      pieceId,
      startEdgeIdx,
      endEdgeIdx,
      slashCount,
      spreadMm,
      hingeMode,
    );
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  removeOp(opId: string): UiSnapshot {
    getSession().remove_op(opId);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  setRollbackIndex(index: number | null): UiSnapshot {
    getSession().set_rollback_index(index === null ? -1 : index);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  toggleSuppressOp(opId: string): UiSnapshot {
    getSession().toggle_suppress_op(opId);
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  reorderOp(fromIdx: number, toIdx: number): { moved: boolean; snapshot: UiSnapshot } {
    const moved = getSession().reorder_op(fromIdx, toIdx);
    bumpRender();
    if (moved) scheduleAutosave();
    return { moved, snapshot: getSnapshot() };
  },

  updateOp(opId: string, patch: Record<string, unknown>): UiSnapshot {
    getSession().update_op_json(opId, JSON.stringify(patch));
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  undo(): UiSnapshot | null {
    if (!getSession().undo()) return null;
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  redo(): UiSnapshot | null {
    if (!getSession().redo()) return null;
    bumpRender();
    scheduleAutosave();
    return getSnapshot();
  },

  querySnap(wx: number, wy: number, worldTolMm: number): SnapHit | null {
    return JSON.parse(getSession().query_snap(wx, wy, worldTolMm)) as SnapHit | null;
  },

  hitTest(wx: number, wy: number, worldTolMm: number): HitResult {
    return JSON.parse(getSession().hit_test(wx, wy, worldTolMm)) as HitResult;
  },

  computeSeamWalk(linkId: string, progressU: number): SeamWalkFrame | null {
    const res = JSON.parse(getSession().compute_seam_walk(linkId, progressU)) as SeamWalkFrame | null;
    bumpRender();
    return res;
  },

  clearSeamWalk() {
    getSession().clear_seam_walk();
    bumpRender();
  },

  exportDxfAama(): string {
    return getSession().export_dxf_aama();
  },

  exportAamaRul(): string {
    return getSession().export_aama_rul();
  },

  importDxfAama(dxfText: string): { message: string; snapshot: UiSnapshot } {
    const message = getSession().import_dxf_aama(dxfText);
    bumpRender();
    scheduleAutosave();
    return { message, snapshot: getSnapshot() };
  },

  importGarmentCodeJson(jsonText: string): { message: string; snapshot: UiSnapshot } {
    const message = getSession().import_garmentcode_json(jsonText);
    bumpRender();
    scheduleAutosave();
    return { message, snapshot: getSnapshot() };
  },

  exportSvgMm(includeNest: boolean): string {
    return getSession().export_svg_mm(includeNest);
  },

  exportPomCsv(): string {
    return getSession().export_pom_csv();
  },

  getPdfLayoutPlan(preset: string, unfoldOnFold: boolean): PdfLayoutPlan {
    return JSON.parse(getSession().get_pdf_layout_json(preset, unfoldOnFold)) as PdfLayoutPlan;
  },

  computeMarkerPlan(cfg: MarkerConfig): MarkerPlan {
    return JSON.parse(getSession().compute_marker_plan_json(JSON.stringify(cfg))) as MarkerPlan;
  },

  exportMarkerSvg(cfg: MarkerConfig): string {
    return getSession().export_marker_svg(JSON.stringify(cfg));
  },

  // --- On-Demand 3D Virtual Fit API (Strictly Decoupled) ---
  initSim3d(
    pose: AvatarPose,
    bodyForm: AvatarBodyForm = 'female',
    spacingMm = 0,
    selfCollision = true,
  ): Sim3dStats {
    const stats = JSON.parse(
      getSession().init_sim3d_with_options(pose, bodyForm, spacingMm, selfCollision),
    ) as Sim3dStats;
    sim3dVersion += 1;
    return stats;
  },

  stepSim3d(steps = 1): Sim3dStats | null {
    return JSON.parse(getSession().step_sim3d(steps)) as Sim3dStats | null;
  },

  resetSim3d(
    pose: AvatarPose,
    bodyForm: AvatarBodyForm = 'female',
    spacingMm = 0,
    selfCollision = true,
  ): Sim3dStats {
    const stats = JSON.parse(
      getSession().init_sim3d_with_options(pose, bodyForm, spacingMm, selfCollision),
    ) as Sim3dStats;
    sim3dVersion += 1;
    return stats;
  },

  setSim3dWind(wx: number, wy: number, wz: number) {
    getSession().set_sim3d_wind(wx, wy, wz);
  },

  setSim3dSelfCollision(enabled: boolean) {
    getSession().set_sim3d_self_collision(enabled);
  },

  pickSim3dVertex(
    ox: number,
    oy: number,
    oz: number,
    dx: number,
    dy: number,
    dz: number,
  ): { vert: number; pos: [number, number, number] } | null {
    return JSON.parse(getSession().pick_sim3d_vertex(ox, oy, oz, dx, dy, dz)) as {
      vert: number;
      pos: [number, number, number];
    } | null;
  },

  grabSim3dStart(vert: number, x: number, y: number, z: number, radius = 0.14) {
    getSession().grab_sim3d_start(vert, x, y, z, radius);
  },

  grabSim3dMove(x: number, y: number, z: number) {
    getSession().grab_sim3d_move(x, y, z);
  },

  grabSim3dEnd() {
    getSession().grab_sim3d_end();
  },

  toggleSim3dPin(vert: number): boolean {
    return getSession().toggle_sim3d_pin(vert);
  },

  clearSim3dPins() {
    getSession().clear_sim3d_pins();
  },

  getSim3dPinPositions(): Float32Array {
    return getSession().get_sim3d_pin_positions();
  },

  getSim3dVbdGpuMeta(): {
    vertex_count: number;
    tri_count: number;
    color_count: number;
    color_offsets: number[];
  } | null {
    return JSON.parse(getSession().get_sim3d_vbd_gpu_meta_json());
  },

  getSim3dVbdGpuU32(kind: string): Uint32Array {
    return getSession().get_sim3d_vbd_gpu_u32(kind);
  },

  getSim3dVbdGpuF32(kind: string): Float32Array {
    return getSession().get_sim3d_vbd_gpu_f32(kind);
  },

  disposeSim3d() {
    getSession().dispose_sim3d();
  },

  /** Poke-through / crease / self-intersection metrics; costs a few ms, poll sparingly. */
  getSim3dQuality(): DrapeQuality | null {
    return JSON.parse(getSession().get_sim3d_quality_json()) as DrapeQuality | null;
  },

  getSim3dBuffers() {
    const s = getSession();
    return {
      clothPos: s.get_sim3d_cloth_positions(),
      clothNorm: s.get_sim3d_cloth_normals(),
      clothStrain: s.get_sim3d_cloth_strain(),
      clothTension: s.get_sim3d_cloth_tension(),
      clothClearance: s.get_sim3d_cloth_clearance(),
      clothSeamMask: s.get_sim3d_cloth_seam_mask(),
      clothIdx: s.get_sim3d_cloth_indices(),
      clothColor: s.get_sim3d_cloth_colors(),
      avatarPos: s.get_sim3d_avatar_positions(),
      avatarNorm: s.get_sim3d_avatar_normals(),
      avatarMask: s.get_sim3d_avatar_torso_mask(),
      avatarIdx: s.get_sim3d_avatar_indices(),
    };
  },
};
