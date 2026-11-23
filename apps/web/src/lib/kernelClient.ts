import initWasm, { ModelistSession } from '../wasm-pkg/modelist_wasm.js';
import { saveAutosave } from './localFiles';
import type {
  CornerRule,
  HitResult,
  PdfLayoutPlan,
  SeamWalkFrame,
  SnapHit,
  UiSnapshot,
} from './types';

let wasmReady: Promise<void> | null = null;
let session: ModelistSession | null = null;
let renderVersion = 1;
let autosaveTimer: ReturnType<typeof setTimeout> | null = null;

function bumpRender() {
  renderVersion += 1;
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

export async function initKernel(templateId = 'bodice_sleeve'): Promise<UiSnapshot> {
  if (!wasmReady) {
    wasmReady = initWasm().then(() => undefined);
  }
  await wasmReady;
  if (!session) {
    session = new ModelistSession(templateId);
  } else {
    session.load_template(templateId);
  }
  bumpRender();
  return getSnapshot();
}

export function getSession(): ModelistSession {
  if (!session) throw new Error('WASM Kernel not initialized');
  return session;
}

export function getRenderVersion(): number {
  return renderVersion;
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

  importDxfAama(dxfText: string): { message: string; snapshot: UiSnapshot } {
    const message = getSession().import_dxf_aama(dxfText);
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
};
