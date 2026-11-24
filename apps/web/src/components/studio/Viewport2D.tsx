import React, { useEffect, useRef, useState } from 'react';
import { Maximize2, Minus, Plus } from 'lucide-react';
import {
  getHandlePoints,
  getRenderVersion,
  getSnapshot,
  getStrokeCoords,
  getStrokeMeta,
  kernel,
} from '../../lib/kernelClient';
import { formatMm, SEAM_PALETTE, type EdgeRefInfo, type HitResult, type SnapHit } from '../../lib/types';
import { detectFlickGesture } from '../../lib/cad/radial-menu';
import { wheelZoomFactor } from '../../lib/cad/zoom';
import { useStudioStore } from '../../store/studioStore';
import { t as tr, useT } from '../../lib/i18n';
import { RadialMarkingMenu } from './RadialMarkingMenu';

const RULER_PX = 24;
const NEST_PALETTE = ['#64748b', '#2563eb', '#059669', '#d97706', '#7c3aed', '#dc2626'];

/** Space at the canvas bottom kept for the navigation bar, px. */
const NAV_BAR_PX = 44;

/**
 * Canvas margins hidden by the rulers, the floating panels and the navigation bar, so "fit"
 * centres the pattern in the part of the canvas the user can actually see.
 */
function visibleInsets(rect: DOMRect) {
  const ins = { left: RULER_PX, right: 0, top: RULER_PX, bottom: NAV_BAR_PX };
  document.querySelectorAll<HTMLElement>('[data-floating-panel]').forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.bottom <= rect.top || r.top >= rect.bottom || r.right <= rect.left || r.left >= rect.right) return;
    if (r.height < rect.height * 0.4) {
      // Collapsed or short panel: it only hides a strip at the top.
      ins.top = Math.max(ins.top, r.bottom - rect.top + 8);
      return;
    }
    const fromLeft = r.right - rect.left;
    const fromRight = rect.right - r.left;
    if (fromLeft < fromRight) ins.left = Math.max(ins.left, fromLeft + 8);
    else ins.right = Math.max(ins.right, fromRight + 8);
  });
  return ins;
}

interface Camera {
  tx: number;
  ty: number;
  scale: number; // px per mm
}

interface DragState {
  mode: 'none' | 'pan' | 'vertex' | 'handle' | 'piece' | 'measure' | 'draft_rect';
  startClientX: number;
  startClientY: number;
  startCamTx: number;
  startCamTy: number;
  startWorldX: number;
  startWorldY: number;
  pieceId?: string;
  edgeIdx?: number;
  which?: number;
  lastDx: number;
  lastDy: number;
}

interface ContextMenuState {
  x: number;
  y: number;
  hit: HitResult;
}

export const Viewport2D: React.FC<{ style?: React.CSSProperties }> = ({ style }) => {
  const t = useT();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const camRef = useRef<Camera>({ tx: 140, ty: 560, scale: 0.85 });
  const dragRef = useRef<DragState>({
    mode: 'none',
    startClientX: 0,
    startClientY: 0,
    startCamTx: 0,
    startCamTy: 0,
    startWorldX: 0,
    startWorldY: 0,
    lastDx: 0,
    lastDy: 0,
  });
  const hoverRef = useRef<HitResult>({ kind: 'none' });
  const snapRef = useRef<SnapHit | null>(null);
  const cursorScreenRef = useRef<{ x: number; y: number } | null>(null);
  const measureSegRef = useRef<{ a: { x: number; y: number }; b: { x: number; y: number } } | null>(null);
  const draftRectRef = useRef<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const dirtyRef = useRef(true);
  const lastVersionRef = useRef(0);
  const lastTelemetryRef = useRef(0);
  const spaceHeldRef = useRef(false);
  const spaceDraggedRef = useRef(false);
  const rmbRef = useRef<{
    active: boolean;
    startClientX: number;
    startClientY: number;
    sx: number;
    sy: number;
    startTime: number;
    hit: HitResult;
  } | null>(null);

  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);
  const [vpSize, setVpSize] = useState<{ w: number; h: number }>({ w: 900, h: 640 });

  const activeTool = useStudioStore((s) => s.activeTool);
  const unit = useStudioStore((s) => s.unit);
  const theme = useStudioStore((s) => s.theme);
  const showSA = useStudioStore((s) => s.showSeamAllowance);
  const showNotches = useStudioStore((s) => s.showNotches);
  const showGrain = useStudioStore((s) => s.showGrainlines);
  const showNest = useStudioStore((s) => s.showGradedNest);
  const showDimensions = useStudioStore((s) => s.showDimensions);
  const showSeamColors = useStudioStore((s) => s.showSeamColors);
  const showMirrorUnfold = useStudioStore((s) => s.showMirrorUnfold);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const snapshot = useStudioStore((s) => s.snapshot);
  const seamCommand = useStudioStore((s) => s.seamCommand);
  const fitViewSeq = useStudioStore((s) => s.fitViewSeq);
  const zoomStepSeq = useStudioStore((s) => s.zoomStepSeq);
  const toggleLayer = useStudioStore((s) => s.toggleLayer);
  const toggleMirrorUnfold = useStudioStore((s) => s.toggleMirrorUnfold);

  const fitView = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    if (rect.width < 40 || rect.height < 40) return;
    const coords = getStrokeCoords();
    if (coords.length < 4) return;
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (let i = 0; i < coords.length; i += 2) {
      const x = coords[i];
      const y = coords[i + 1];
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
    if (!isFinite(minX)) return;
    const spanX = Math.max(80, maxX - minX);
    const spanY = Math.max(80, maxY - minY);
    const ins = visibleInsets(rect);
    const viewW = Math.max(160, rect.width - ins.left - ins.right);
    const viewH = Math.max(160, rect.height - ins.top - ins.bottom);
    const availW = Math.max(120, viewW - 56);
    const availH = Math.max(120, viewH - 56);
    const scale = Math.max(0.12, Math.min(3.0, Math.min(availW / spanX, availH / spanY)));
    const midX = 0.5 * (minX + maxX);
    const midY = 0.5 * (minY + maxY);
    camRef.current = {
      scale,
      tx: ins.left + viewW * 0.5 - midX * scale,
      ty: ins.top + viewH * 0.5 + midY * scale,
    };
    useStudioStore.getState().setCursorTelemetry(
      useStudioStore.getState().cursorWorld,
      snapRef.current,
      scale * 100,
    );
    dirtyRef.current = true;
  };

  useEffect(() => {
    const id = requestAnimationFrame(() => fitView());
    return () => cancelAnimationFrame(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fitViewSeq]);

  useEffect(() => {
    if (zoomStepSeq === 0) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const ins = visibleInsets(rect);
    const cx = ins.left + (rect.width - ins.left - ins.right) * 0.5;
    const cy = ins.top + (rect.height - ins.top - ins.bottom) * 0.5;
    const factor = useStudioStore.getState().zoomStepFactor;
    const cam = camRef.current;
    const wx = (cx - cam.tx) / cam.scale;
    const wy = (cam.ty - cy) / cam.scale;
    const nextScale = Math.max(0.08, Math.min(12, cam.scale * factor));
    camRef.current = {
      scale: nextScale,
      tx: cx - wx * nextScale,
      ty: cy + wy * nextScale,
    };
    useStudioStore.getState().setCursorTelemetry(
      useStudioStore.getState().cursorWorld,
      snapRef.current,
      nextScale * 100,
    );
    dirtyRef.current = true;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [zoomStepSeq]);

  useEffect(() => {
    dirtyRef.current = true;
  }, [
    theme,
    showSA,
    showNotches,
    showGrain,
    showNest,
    showDimensions,
    showSeamColors,
    showMirrorUnfold,
    workspaceMode,
    seamCommand,
    unit,
    activeTool,
    snapshot,
  ]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const cr = entry.contentRect;
        if (cr.width > 0 && cr.height > 0) {
          setVpSize({ w: cr.width, h: cr.height });
        }
      }
      dirtyRef.current = true;
    });
    ro.observe(canvas);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const inInput =
        target &&
        (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT');
      if (e.code === 'Space' && !inInput) {
        e.preventDefault();
        if (!e.repeat) {
          spaceDraggedRef.current = false;
        }
        spaceHeldRef.current = true;
      }
      if (e.key === 'Escape') {
        setCtxMenu(null);
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code === 'Space') {
        const wasHeld = spaceHeldRef.current;
        const wasDragged = spaceDraggedRef.current;
        spaceHeldRef.current = false;
        spaceDraggedRef.current = false;
        const target = e.target as HTMLElement | null;
        const inInput =
          target &&
          (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT');
        const st = useStudioStore.getState();
        if (wasHeld && !wasDragged && !inInput && !st.shortcutToolbox?.open && !st.editingOpId) {
          st.repeatLastCommand();
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
    };
  }, []);

  // Main Imperative 60-120 FPS Render Loop (zero React DOM overhead for geometry)
  useEffect(() => {
    let rafId = 0;

    const renderFrame = () => {
      rafId = requestAnimationFrame(renderFrame);
      const canvas = canvasRef.current;
      if (!canvas) return;

      const ver = getRenderVersion();
      if (ver !== lastVersionRef.current) {
        lastVersionRef.current = ver;
        dirtyRef.current = true;
      }
      if (!dirtyRef.current) return;
      dirtyRef.current = false;

      const rect = canvas.getBoundingClientRect();
      if (rect.width < 10 || rect.height < 10) return;
      const dpr = window.devicePixelRatio || 1;
      const targetW = Math.max(1, Math.floor(rect.width * dpr));
      const targetH = Math.max(1, Math.floor(rect.height * dpr));
      if (canvas.width !== targetW || canvas.height !== targetH) {
        canvas.width = targetW;
        canvas.height = targetH;
      }

      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      const css = getComputedStyle(document.documentElement);
      const bgCanvas = css.getPropertyValue('--bg-canvas').trim() || '#f7f5f0';
      const inkColor = css.getPropertyValue('--text-primary').trim() || '#17191e';
      const mutedColor = css.getPropertyValue('--text-muted').trim() || '#7c8391';
      const accentColor = css.getPropertyValue('--accent').trim() || '#1e5eff';
      const saColor = css.getPropertyValue('--sa-color').trim() || '#c97212';
      const notchColor = css.getPropertyValue('--notch-color').trim() || '#d12e2e';
      const grainColor = css.getPropertyValue('--grain-color').trim() || '#1e5eff';
      const walkColor = css.getPropertyValue('--walk-color').trim() || '#0e9f6e';
      const gridMinor = css.getPropertyValue('--grid-minor').trim() || 'rgba(0,0,0,0.04)';
      const gridMajor = css.getPropertyValue('--grid-major').trim() || 'rgba(0,0,0,0.09)';
      const rulerBg = css.getPropertyValue('--ruler-bg').trim() || '#edeae2';
      const rulerText = css.getPropertyValue('--ruler-text').trim() || '#656b77';
      const borderColor = css.getPropertyValue('--border-subtle').trim() || '#ded9ce';

      const w = rect.width;
      const h = rect.height;
      const cam = camRef.current;
      const state = useStudioStore.getState();
      const isDark = state.theme === 'dark';

      const toSX = (wx: number) => cam.tx + wx * cam.scale;
      const toSY = (wy: number) => cam.ty - wy * cam.scale;

      ctx.save();
      ctx.scale(dpr, dpr);

      // Clear drafting background
      ctx.fillStyle = bgCanvas;
      ctx.fillRect(0, 0, w, h);

      // Clip inside drafting rulers
      ctx.save();
      ctx.beginPath();
      ctx.rect(RULER_PX, RULER_PX, w - RULER_PX, h - RULER_PX);
      ctx.clip();

      const minorStepMm = cam.scale > 1.4 ? 10 : cam.scale > 0.45 ? 25 : 50;
      const majorStepMm = minorStepMm * 4;
      const wxMin = (RULER_PX - cam.tx) / cam.scale;
      const wxMax = (w - cam.tx) / cam.scale;
      const wyMin = (cam.ty - h) / cam.scale;
      const wyMax = (cam.ty - RULER_PX) / cam.scale;

      ctx.lineWidth = 1;
      for (let gx = Math.floor(wxMin / minorStepMm) * minorStepMm; gx <= wxMax; gx += minorStepMm) {
        const sx = cam.tx + gx * cam.scale;
        const isMajor = Math.abs(gx % majorStepMm) < 1e-4;
        ctx.strokeStyle = isMajor ? gridMajor : gridMinor;
        ctx.beginPath();
        ctx.moveTo(sx, RULER_PX);
        ctx.lineTo(sx, h);
        ctx.stroke();
      }
      for (let gy = Math.floor(wyMin / minorStepMm) * minorStepMm; gy <= wyMax; gy += minorStepMm) {
        const sy = cam.ty - gy * cam.scale;
        const isMajor = Math.abs(gy % majorStepMm) < 1e-4;
        ctx.strokeStyle = isMajor ? gridMajor : gridMinor;
        ctx.beginPath();
        ctx.moveTo(RULER_PX, sy);
        ctx.lineTo(w, sy);
        ctx.stroke();
      }

      // 1. Translucent Pattern Cardstock Paper Fill (CLO3D / Modaris Visual Hierarchy)
      const snapUi = state.snapshot;
      if (snapUi?.piece_fills) {
        for (const pf of snapUi.piece_fills) {
          if (pf.coords.length < 6) continue;
          ctx.beginPath();
          ctx.moveTo(toSX(pf.coords[0]), toSY(pf.coords[1]));
          for (let i = 2; i < pf.coords.length; i += 2) {
            ctx.lineTo(toSX(pf.coords[i]), toSY(pf.coords[i + 1]));
          }
          ctx.closePath();
          if (pf.selected) {
            ctx.fillStyle = isDark ? 'rgba(59, 130, 246, 0.14)' : 'rgba(255, 255, 255, 0.86)';
          } else {
            ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 0.05)' : 'rgba(255, 255, 255, 0.62)';
          }
          ctx.fill();
        }
      }

      // 2. Geometry strokes directly from WASM typed arrays
      const coords = getStrokeCoords();
      const meta = getStrokeMeta();
      const segCount = Math.min(coords.length >> 2, Math.floor(meta.length / 3));

      let activeRole = -1;
      let activeSub = -1;
      let pathOpen = false;

      const flushPath = () => {
        if (pathOpen) {
          ctx.stroke();
          pathOpen = false;
        }
      };

      for (let i = 0; i < segCount; i++) {
        const role = meta[i * 3];
        const subIdx = meta[i * 3 + 2];

        if (role === 1 && !state.showSeamAllowance) continue;
        if ((role === 4 || role === 13) && !state.showNotches) continue;
        if (role === 3 && !state.showGrainlines) continue;
        if (role === 5 && !state.showGradedNest) continue;
        if (role === 12 && !state.showMirrorUnfold) continue;
        if ((role === 14 || role === 15) && !state.showSeamColors) continue;

        const needsSubSplit = role === 5 || role === 14 || role === 18;
        if (role !== activeRole || (needsSubSplit && subIdx !== activeSub)) {
          flushPath();
          activeRole = role;
          activeSub = subIdx;
          ctx.beginPath();
          ctx.setLineDash([]);
          switch (role) {
            case 0: // Seamline
              ctx.strokeStyle = inkColor;
              ctx.lineWidth = 1.65;
              break;
            case 1: // Cutline (SA)
              ctx.strokeStyle = saColor;
              ctx.lineWidth = 1.15;
              ctx.setLineDash([6, 4]);
              break;
            case 2: // Dart / Internal
              ctx.strokeStyle = mutedColor;
              ctx.lineWidth = 1.05;
              ctx.setLineDash([4, 3]);
              break;
            case 3: // Grainline
              ctx.strokeStyle = grainColor;
              ctx.lineWidth = 1.1;
              break;
            case 4: // User Notch
              ctx.strokeStyle = notchColor;
              ctx.lineWidth = 1.75;
              break;
            case 5: // Graded size
              ctx.strokeStyle = NEST_PALETTE[subIdx % NEST_PALETTE.length];
              ctx.lineWidth = 1.05;
              ctx.setLineDash([5, 3]);
              break;
            case 6: // Handle guide / selected seam leader
              ctx.strokeStyle = accentColor;
              ctx.lineWidth = 1.1;
              ctx.setLineDash([4, 3]);
              break;
            case 7: // Walk-seam ghost
              ctx.strokeStyle = walkColor;
              ctx.lineWidth = 2.0;
              ctx.setLineDash([8, 4]);
              break;
            case 8: // Dart cap
              ctx.strokeStyle = saColor;
              ctx.lineWidth = 1.0;
              ctx.setLineDash([2, 3]);
              break;
            case 9: // Selected edge
              ctx.strokeStyle = accentColor;
              ctx.lineWidth = 3.1;
              break;
            case 10: // Mirror fold
              ctx.strokeStyle = '#7c3aed';
              ctx.lineWidth = 1.85;
              ctx.setLineDash([10, 4, 2, 4]);
              break;
            case 11: // Drill hole
              ctx.strokeStyle = notchColor;
              ctx.lineWidth = 1.25;
              break;
            case 12: // Symmetry unfolded mirror ghost
              ctx.strokeStyle = 'rgba(124, 58, 237, 0.34)';
              ctx.lineWidth = 1.25;
              ctx.setLineDash([4, 4]);
              break;
            case 13: // Auto-projected partner notch
              ctx.strokeStyle = walkColor;
              ctx.lineWidth = 1.8;
              break;
            case 14: // CLO3D-style Color-Coded Seam Binding Ribbon + Match Chevron
              ctx.strokeStyle = SEAM_PALETTE[subIdx % SEAM_PALETTE.length];
              ctx.lineWidth = 2.5;
              break;
            case 15: // Out-of-Tolerance Seam Length Mismatch Warning (Red!)
              ctx.strokeStyle = '#e11d48';
              ctx.lineWidth = 3.0;
              break;
            case 16: // Fusion 360 Seam Collector Interface A (Vivid Cobalt Blue)
              ctx.strokeStyle = '#2563eb';
              ctx.lineWidth = 4.2;
              break;
            case 17: // Fusion 360 Seam Collector Interface B (Vivid Teal)
              ctx.strokeStyle = '#0d9488';
              ctx.lineWidth = 4.2;
              break;
            case 18: // Selected Seam Binding Highlight
              ctx.strokeStyle = SEAM_PALETTE[subIdx % SEAM_PALETTE.length];
              ctx.lineWidth = 4.4;
              break;
            default:
              ctx.strokeStyle = inkColor;
              ctx.lineWidth = 1.2;
          }
        }

        const x0 = toSX(coords[i * 4]);
        const y0 = toSY(coords[i * 4 + 1]);
        const x1 = toSX(coords[i * 4 + 2]);
        const y1 = toSY(coords[i * 4 + 3]);
        ctx.moveTo(x0, y0);
        ctx.lineTo(x1, y1);
        pathOpen = true;
      }
      flushPath();
      ctx.setLineDash([]);

      // 3. Interactive vertices & Bézier handles for selected piece
      const handles = getHandlePoints();
      for (let i = 0; i < handles.length; i += 5) {
        const hx = toSX(handles[i]);
        const hy = toSY(handles[i + 1]);
        const kind = handles[i + 2];
        if (kind === 0) {
          ctx.fillStyle = '#ffffff';
          ctx.strokeStyle = inkColor;
          ctx.lineWidth = 1.6;
          ctx.fillRect(hx - 4, hy - 4, 8, 8);
          ctx.strokeRect(hx - 4, hy - 4, 8, 8);
        } else if (kind === 1 || kind === 2) {
          ctx.fillStyle = accentColor;
          ctx.beginPath();
          ctx.arc(hx, hy, 4, 0, Math.PI * 2);
          ctx.fill();
          ctx.strokeStyle = '#ffffff';
          ctx.lineWidth = 1.2;
          ctx.stroke();
        } else if (kind === 3) {
          ctx.fillStyle = saColor;
          ctx.beginPath();
          ctx.moveTo(hx, hy - 5.5);
          ctx.lineTo(hx + 5.5, hy);
          ctx.lineTo(hx, hy + 5.5);
          ctx.lineTo(hx - 5.5, hy);
          ctx.closePath();
          ctx.fill();
        }
      }

      // 4. Inline Edge Length Dimension Badges (CLO3D Show Line Length!)
      if (state.showDimensions && snapUi?.edge_dims && cam.scale > 0.25) {
        ctx.font = '600 9.5px "JetBrains Mono", monospace';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        for (const dim of snapUi.edge_dims) {
          if (!dim.is_selected_piece && !dim.is_selected_edge) continue;
          if (dim.length_mm < 15) continue;
          const sx = toSX(dim.x);
          const sy = toSY(dim.y);
          const text = formatMm(dim.length_mm, state.unit, state.unit === 'mm' ? 0 : 1);
          const tw = ctx.measureText(text).width + 8;
          ctx.fillStyle = dim.is_selected_edge
            ? accentColor
            : isDark
              ? 'rgba(24, 27, 34, 0.88)'
              : 'rgba(255, 255, 255, 0.90)';
          ctx.strokeStyle = dim.is_selected_edge ? accentColor : borderColor;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.roundRect(sx - tw / 2, sy - 8, tw, 16, 4);
          ctx.fill();
          ctx.stroke();
          ctx.fillStyle = dim.is_selected_edge ? '#ffffff' : inkColor;
          ctx.fillText(text, sx, sy + 0.5);
        }
      }

      // 5. Piece labels
      if (snapUi) {
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        for (const lbl of snapUi.labels) {
          const sx = toSX(lbl.x);
          const sy = toSY(lbl.y);
          const isSel = lbl.piece_id === snapUi.selected_piece_id;
          ctx.font = '600 11.5px Inter, sans-serif';
          ctx.fillStyle = isSel ? accentColor : inkColor;
          ctx.fillText(lbl.text, sx, sy - 7);
          ctx.font = '500 9.5px "JetBrains Mono", monospace';
          ctx.fillStyle = mutedColor;
          ctx.fillText(lbl.sub, sx, sy + 8);
        }
      }

      // 6. Live Draft-New-Piece Rectangle Preview
      if (draftRectRef.current) {
        const dr = draftRectRef.current;
        const x0 = toSX(Math.min(dr.x0, dr.x1));
        const y0 = toSY(Math.max(dr.y0, dr.y1));
        const rw = Math.abs(dr.x1 - dr.x0) * cam.scale;
        const rh = Math.abs(dr.y1 - dr.y0) * cam.scale;
        ctx.fillStyle = 'rgba(30, 94, 255, 0.12)';
        ctx.strokeStyle = accentColor;
        ctx.lineWidth = 1.6;
        ctx.setLineDash([5, 3]);
        ctx.fillRect(x0, y0, rw, rh);
        ctx.strokeRect(x0, y0, rw, rh);
        ctx.setLineDash([]);
        const dimTxt = `${formatMm(Math.abs(dr.x1 - dr.x0), state.unit, 0)} × ${formatMm(Math.abs(dr.y1 - dr.y0), state.unit, 0)}`;
        ctx.font = '600 11px "JetBrains Mono", monospace';
        ctx.fillStyle = accentColor;
        ctx.textAlign = 'center';
        ctx.fillText(dimTxt, x0 + rw * 0.5, y0 + rh * 0.5);
      }

      // 7. Walk-Seam contact marker
      if (state.walkFrame) {
        const cp = state.walkFrame.contact_pt;
        const cx = toSX(cp.x);
        const cy = toSY(cp.y);
        ctx.fillStyle = walkColor;
        ctx.beginPath();
        ctx.arc(cx, cy, 5.5, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 1.8;
        ctx.stroke();
      }

      // 8. Snap indicator badge & Fusion 360 Hover Pre-Selection Micro-Tooltip
      const snapHit = snapRef.current;
      if (snapHit) {
        const sx = toSX(snapHit.point.x);
        const sy = toSY(snapHit.point.y);
        ctx.strokeStyle = '#10b981';
        ctx.lineWidth = 1.8;
        ctx.beginPath();
        ctx.arc(sx, sy, 7, 0, Math.PI * 2);
        ctx.stroke();
      }

      const hov = hoverRef.current;
      if (hov.kind === 'edge' && snapUi && dragRef.current.mode === 'none') {
        const hp = snapUi.pieces.find((p) => p.id === hov.piece_id);
        const he = hp?.edges.find((e) => e.edge_idx === hov.edge_idx);
        if (hp && he) {
          const sx = toSX(hov.point.x);
          const sy = toSY(hov.point.y);
          const slotCol = state.seamCommand
            ? state.seamCommand.activeSlot === 'a'
              ? '#2563eb'
              : '#0d9488'
            : accentColor;
          ctx.fillStyle = slotCol;
          ctx.beginPath();
          ctx.arc(sx, sy, 4.5, 0, Math.PI * 2);
          ctx.fill();
          ctx.strokeStyle = '#ffffff';
          ctx.lineWidth = 1.5;
          ctx.stroke();

          const ownerBinding = snapUi.stitching_links.find(
            (l) =>
              l.edges_a.some((er) => er.edge_id === he.edge_id) ||
              l.edges_b.some((er) => er.edge_id === he.edge_id),
          );
          const tag = state.seamCommand
            ? `+ ${hp.code} ${he.label} (${formatMm(he.length_mm, state.unit, 0)}) → Slot ${state.seamCommand.activeSlot.toUpperCase()}`
            : ownerBinding
              ? `${hp.code} ${he.label} · ${formatMm(he.length_mm, state.unit, 1)} · ⛓ ${ownerBinding.label}`
              : `${hp.code} ${he.label} · ${formatMm(he.length_mm, state.unit, 1)}`;
          ctx.font = '600 10px Inter, sans-serif';
          const tw = ctx.measureText(tag).width + 14;
          const bx = Math.min(w - tw - 10, Math.max(RULER_PX + 8, sx + 12));
          const by = Math.max(RULER_PX + 24, sy - 26);
          ctx.fillStyle = isDark ? 'rgba(20, 23, 30, 0.94)' : 'rgba(255, 255, 255, 0.96)';
          ctx.strokeStyle = slotCol;
          ctx.lineWidth = 1.2;
          ctx.beginPath();
          ctx.roundRect(bx, by, tw, 20, 6);
          ctx.fill();
          ctx.stroke();
          ctx.fillStyle = inkColor;
          ctx.textAlign = 'left';
          ctx.textBaseline = 'middle';
          ctx.fillText(tag, bx + 7, by + 10);
        }
      }

      // 9. Caliper measure line
      if (measureSegRef.current) {
        const { a, b } = measureSegRef.current;
        const ax = toSX(a.x);
        const ay = toSY(a.y);
        const bx = toSX(b.x);
        const by = toSY(b.y);
        ctx.strokeStyle = accentColor;
        ctx.lineWidth = 1.75;
        ctx.setLineDash([4, 3]);
        ctx.beginPath();
        ctx.moveTo(ax, ay);
        ctx.lineTo(bx, by);
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillStyle = accentColor;
        ctx.beginPath();
        ctx.arc(ax, ay, 3.5, 0, Math.PI * 2);
        ctx.arc(bx, by, 3.5, 0, Math.PI * 2);
        ctx.fill();
      }

      ctx.restore(); // End viewport clip

      // Dynamic Top & Left Rulers
      ctx.fillStyle = rulerBg;
      ctx.fillRect(RULER_PX, 0, w - RULER_PX, RULER_PX);
      ctx.fillRect(0, RULER_PX, RULER_PX, h - RULER_PX);
      ctx.fillRect(0, 0, RULER_PX, RULER_PX);

      ctx.strokeStyle = borderColor;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, RULER_PX);
      ctx.lineTo(w, RULER_PX);
      ctx.moveTo(RULER_PX, 0);
      ctx.lineTo(RULER_PX, h);
      ctx.stroke();

      const rulerStep = majorStepMm;
      ctx.fillStyle = rulerText;
      ctx.strokeStyle = rulerText;
      ctx.font = '500 9px "JetBrains Mono", monospace';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'top';

      for (let gx = Math.floor(wxMin / rulerStep) * rulerStep; gx <= wxMax; gx += rulerStep) {
        const sx = cam.tx + gx * cam.scale;
        if (sx < RULER_PX) continue;
        ctx.beginPath();
        ctx.moveTo(sx, RULER_PX - 7);
        ctx.lineTo(sx, RULER_PX);
        ctx.stroke();
        const label =
          state.unit === 'cm'
            ? `${(gx / 10).toFixed(0)}`
            : state.unit === 'in'
              ? `${(gx / 25.4).toFixed(1)}`
              : `${gx.toFixed(0)}`;
        ctx.fillText(label, sx + 3, 4);
      }

      for (let gy = Math.floor(wyMin / rulerStep) * rulerStep; gy <= wyMax; gy += rulerStep) {
        const sy = cam.ty - gy * cam.scale;
        if (sy < RULER_PX) continue;
        ctx.beginPath();
        ctx.moveTo(RULER_PX - 7, sy);
        ctx.lineTo(RULER_PX, sy);
        ctx.stroke();
        const label =
          state.unit === 'cm'
            ? `${(gy / 10).toFixed(0)}`
            : state.unit === 'in'
              ? `${(gy / 25.4).toFixed(1)}`
              : `${gy.toFixed(0)}`;
        ctx.save();
        ctx.translate(4, sy - 3);
        ctx.rotate(-Math.PI / 2);
        ctx.fillText(label, 0, 0);
        ctx.restore();
      }

      // Cursor tracking hairline ticks on rulers
      if (cursorScreenRef.current) {
        const { x: cx, y: cy } = cursorScreenRef.current;
        ctx.strokeStyle = accentColor;
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.moveTo(cx, 0);
        ctx.lineTo(cx, RULER_PX);
        ctx.moveTo(0, cy);
        ctx.lineTo(RULER_PX, cy);
        ctx.stroke();
      }

      ctx.restore();
    };

    rafId = requestAnimationFrame(renderFrame);
    return () => cancelAnimationFrame(rafId);
  }, []);

  const toWorld = (clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return { sx: 0, sy: 0, wx: 0, wy: 0 };
    const rect = canvas.getBoundingClientRect();
    const sx = clientX - rect.left;
    const sy = clientY - rect.top;
    const cam = camRef.current;
    return {
      sx,
      sy,
      wx: (sx - cam.tx) / cam.scale,
      wy: (cam.ty - sy) / cam.scale,
    };
  };

  const handleContextMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    if (e.button === 2) {
      setCtxMenu(null);
      canvas.setPointerCapture(e.pointerId);
      const { sx, sy, wx, wy } = toWorld(e.clientX, e.clientY);
      const cam = camRef.current;
      const hit = kernel.hitTest(wx, wy, 12 / cam.scale);
      rmbRef.current = {
        active: true,
        startClientX: e.clientX,
        startClientY: e.clientY,
        sx,
        sy,
        startTime: performance.now(),
        hit,
      };
      return;
    }
    setCtxMenu(null);
    canvas.setPointerCapture(e.pointerId);
    const { wx, wy } = toWorld(e.clientX, e.clientY);
    const cam = camRef.current;
    const state = useStudioStore.getState();

    // Middle mouse or Pan tool or Spacebar -> Pan
    if (e.button === 1 || state.activeTool === 'pan' || spaceHeldRef.current) {
      if (spaceHeldRef.current) {
        spaceDraggedRef.current = true;
      }
      dragRef.current = {
        mode: 'pan',
        startClientX: e.clientX,
        startClientY: e.clientY,
        startCamTx: cam.tx,
        startCamTy: cam.ty,
        startWorldX: wx,
        startWorldY: wy,
        lastDx: 0,
        lastDy: 0,
      };
      return;
    }

    // Draft New Pattern Piece (click-drag rectangle)
    if (state.activeTool === 'draft_piece') {
      const snapped = kernel.querySnap(wx, wy, 10 / cam.scale)?.point ?? { x: wx, y: wy };
      draftRectRef.current = { x0: snapped.x, y0: snapped.y, x1: snapped.x, y1: snapped.y };
      dragRef.current = {
        mode: 'draft_rect',
        startClientX: e.clientX,
        startClientY: e.clientY,
        startCamTx: cam.tx,
        startCamTy: cam.ty,
        startWorldX: snapped.x,
        startWorldY: snapped.y,
        lastDx: 0,
        lastDy: 0,
      };
      dirtyRef.current = true;
      return;
    }

    if (state.activeTool === 'measure') {
      const snapped = kernel.querySnap(wx, wy, 12 / cam.scale)?.point ?? { x: wx, y: wy };
      measureSegRef.current = { a: snapped, b: snapped };
      dragRef.current = {
        mode: 'measure',
        startClientX: e.clientX,
        startClientY: e.clientY,
        startCamTx: cam.tx,
        startCamTy: cam.ty,
        startWorldX: snapped.x,
        startWorldY: snapped.y,
        lastDx: 0,
        lastDy: 0,
      };
      dirtyRef.current = true;
      return;
    }

    const hit = kernel.hitTest(wx, wy, 12 / cam.scale);

    // Fusion 360 Floating Seam Interface Command Collector (click any edge to toggle into Slot A or B!)
    if (state.seamCommand) {
      if (hit.kind === 'edge') {
        const piece = state.snapshot?.pieces.find((p) => p.id === hit.piece_id);
        const edge = piece?.edges.find((ed) => ed.edge_idx === hit.edge_idx);
        if (piece && edge) {
          const info: EdgeRefInfo = {
            piece_id: piece.id,
            piece_code: piece.code,
            edge_id: edge.edge_id,
            edge_idx: edge.edge_idx,
            edge_label: edge.label,
            length_mm: edge.length_mm,
            dir: 1,
          };
          state.toggleSeamCommandEdge(info);
          dirtyRef.current = true;
        }
      } else {
        state.setStatusMessage(
          tr('v2d.collectEdges', { slot: state.seamCommand.activeSlot.toUpperCase() }),
        );
      }
      return;
    }

    // True Corner (G1 / 90° Square-Off)
    if (state.activeTool === 'true_corner') {
      if (hit.kind === 'vertex') {
        const nextSnap = kernel.trueCorner(hit.piece_id, hit.edge_idx, state.trueCornerSquare90);
        state.setSnapshot(nextSnap);
        state.setStatusMessage(
          tr('v2d.truedCorner', {
            index: hit.edge_idx + 1,
            mode: state.trueCornerSquare90 ? tr('v2d.square90') : tr('v2d.g1Smooth'),
          }),
        );
        dirtyRef.current = true;
      } else {
        state.setStatusMessage(tr('v2d.trueCornerHint'));
      }
      return;
    }

    // Drill Hole placement tool
    if (state.activeTool === 'drill_hole') {
      const targetPieceId =
        hit.kind !== 'none' ? hit.piece_id : state.snapshot?.selected_piece_id;
      if (targetPieceId) {
        const nextSnap = kernel.addDrillHole(targetPieceId, wx, wy, 'Punch');
        state.setSnapshot(nextSnap);
        state.setStatusMessage(tr('v2d.drillPlaced', { x: wx.toFixed(0), y: wy.toFixed(0) }));
        dirtyRef.current = true;
      } else {
        state.setStatusMessage(tr('v2d.drillHint'));
      }
      return;
    }

    // Style-Line Split tool
    if (state.activeTool === 'style_split') {
      if (hit.kind === 'edge') {
        const first = state.styleSplitAnchor;
        if (!first || first.pieceId !== hit.piece_id || first.edgeIdx === hit.edge_idx) {
          state.setStyleSplitAnchor({
            pieceId: hit.piece_id,
            edgeIdx: hit.edge_idx,
            paramT: hit.param_t,
          });
          state.setStatusMessage(tr('v2d.styleAnchor', { index: hit.edge_idx + 1 }));
        } else {
          const nextSnap = kernel.splitStyleLine(
            hit.piece_id,
            first.edgeIdx,
            first.paramT,
            hit.edge_idx,
            hit.param_t,
            '',
            '',
          );
          state.setSnapshot(nextSnap);
          state.setStyleSplitAnchor(null);
          state.setStatusMessage(tr('v2d.styleDone'));
          dirtyRef.current = true;
        }
      } else {
        state.setStatusMessage(tr('v2d.styleHint'));
      }
      return;
    }

    // Notch placement tool
    if (state.activeTool === 'notch') {
      if (hit.kind === 'edge') {
        const nextSnap = kernel.addNotch(
          hit.piece_id,
          hit.edge_idx,
          hit.param_t,
          state.notchStyle,
          state.notchCount,
          5,
        );
        state.setSnapshot(nextSnap);
        state.setStatusMessage(tr('v2d.notchAdded', { count: state.notchCount, style: state.notchStyle }));
        dirtyRef.current = true;
      } else {
        state.setStatusMessage(tr('v2d.notchHint'));
      }
      return;
    }

    // Dart Pivot tool
    if (state.activeTool === 'dart_pivot') {
      const piece = state.snapshot?.pieces.find(
        (p) => p.id === (hit.kind !== 'none' ? hit.piece_id : state.snapshot?.selected_piece_id),
      );
      const dart = piece?.darts[0];
      if (hit.kind === 'edge' && piece && dart) {
        const nextSnap = kernel.pivotDart(
          piece.id,
          dart.id,
          hit.edge_idx,
          hit.param_t,
          state.dartPivotAngleDeg,
        );
        state.setSnapshot(nextSnap);
        state.setStatusMessage(tr('v2d.pivoted', { dart: dart.name, angle: state.dartPivotAngleDeg, index: hit.edge_idx + 1 }));
        dirtyRef.current = true;
      } else {
        state.setStatusMessage(tr('v2d.pivotHint'));
      }
      return;
    }

    // Slash & Spread tool
    if (state.activeTool === 'slash_spread') {
      if (hit.kind === 'edge') {
        const piece = state.snapshot?.pieces.find((p) => p.id === hit.piece_id);
        if (piece) {
          const n = piece.edges.length;
          const oppEdge = (hit.edge_idx + Math.floor(n / 2)) % n;
          const nextSnap = kernel.slashAndSpread(
            piece.id,
            oppEdge,
            hit.edge_idx,
            state.slashCount,
            state.slashSpreadMm,
            state.slashHingeMode,
          );
          state.setSnapshot(nextSnap);
          state.setStatusMessage(tr('v2d.slashApplied', { count: state.slashCount, mm: state.slashSpreadMm }));
          dirtyRef.current = true;
        }
      } else {
        state.setStatusMessage(tr('v2d.slashHint'));
      }
      return;
    }

    // Select or Direct Edit
    if (hit.kind === 'handle') {
      dragRef.current = {
        mode: 'handle',
        startClientX: e.clientX,
        startClientY: e.clientY,
        startCamTx: cam.tx,
        startCamTy: cam.ty,
        startWorldX: wx,
        startWorldY: wy,
        pieceId: hit.piece_id,
        edgeIdx: hit.edge_idx,
        which: hit.which,
        lastDx: 0,
        lastDy: 0,
      };
      return;
    }

    if (hit.kind === 'vertex') {
      state.setSnapshot(kernel.setSelection(hit.piece_id, hit.edge_idx));
      dragRef.current = {
        mode: 'vertex',
        startClientX: e.clientX,
        startClientY: e.clientY,
        startCamTx: cam.tx,
        startCamTy: cam.ty,
        startWorldX: wx,
        startWorldY: wy,
        pieceId: hit.piece_id,
        edgeIdx: hit.edge_idx,
        lastDx: 0,
        lastDy: 0,
      };
      dirtyRef.current = true;
      return;
    }

    if (hit.kind === 'edge') {
      state.setSnapshot(kernel.setSelection(hit.piece_id, hit.edge_idx));
      dirtyRef.current = true;
      return;
    }

    // Clicking inside a piece selects it AND lets you drag to reposition it freely on the table!
    if (hit.kind === 'piece') {
      state.setSnapshot(kernel.setSelection(hit.piece_id, null));
      dragRef.current = {
        mode: 'piece',
        startClientX: e.clientX,
        startClientY: e.clientY,
        startCamTx: cam.tx,
        startCamTy: cam.ty,
        startWorldX: wx,
        startWorldY: wy,
        pieceId: hit.piece_id,
        lastDx: 0,
        lastDy: 0,
      };
      dirtyRef.current = true;
      return;
    }

    // Empty space click -> pan on drag
    dragRef.current = {
      mode: 'pan',
      startClientX: e.clientX,
      startClientY: e.clientY,
      startCamTx: cam.tx,
      startCamTy: cam.ty,
      startWorldX: wx,
      startWorldY: wy,
      lastDx: 0,
      lastDy: 0,
    };
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (rmbRef.current?.active) return;
    const { sx, sy, wx, wy } = toWorld(e.clientX, e.clientY);
    cursorScreenRef.current = { x: sx, y: sy };
    const drag = dragRef.current;
    const cam = camRef.current;

    if (drag.mode === 'pan') {
      if (
        spaceHeldRef.current &&
        Math.hypot(e.clientX - drag.startClientX, e.clientY - drag.startClientY) > 3
      ) {
        spaceDraggedRef.current = true;
      }
      camRef.current = {
        ...cam,
        tx: drag.startCamTx + (e.clientX - drag.startClientX),
        ty: drag.startCamTy + (e.clientY - drag.startClientY),
      };
      dirtyRef.current = true;
      return;
    }

    const snapHit = kernel.querySnap(wx, wy, 10 / cam.scale);
    snapRef.current = snapHit;

    if (drag.mode === 'draft_rect' && draftRectRef.current) {
      const endPt = snapHit?.point ?? { x: wx, y: wy };
      draftRectRef.current = { ...draftRectRef.current, x1: endPt.x, y1: endPt.y };
      dirtyRef.current = true;
    } else if (drag.mode === 'piece' && drag.pieceId) {
      const dx = wx - drag.startWorldX;
      const dy = wy - drag.startWorldY;
      if (Math.hypot(dx, dy) > 2) {
        drag.lastDx = dx;
        drag.lastDy = dy;
        kernel.movePiece(drag.pieceId, dx, dy, false);
        dirtyRef.current = true;
      }
    } else if (drag.mode === 'vertex' && drag.pieceId && drag.edgeIdx !== undefined) {
      const dx = wx - drag.startWorldX;
      const dy = wy - drag.startWorldY;
      drag.lastDx = dx;
      drag.lastDy = dy;
      kernel.moveVertex(drag.pieceId, drag.edgeIdx, dx, dy, false);
      dirtyRef.current = true;
    } else if (
      drag.mode === 'handle' &&
      drag.pieceId &&
      drag.edgeIdx !== undefined &&
      drag.which !== undefined
    ) {
      const dx = wx - drag.startWorldX;
      const dy = wy - drag.startWorldY;
      drag.lastDx = dx;
      drag.lastDy = dy;
      kernel.moveHandle(drag.pieceId, drag.edgeIdx, drag.which, dx, dy, e.shiftKey, false);
      dirtyRef.current = true;
    } else if (drag.mode === 'measure' && measureSegRef.current) {
      const endPt = snapHit?.point ?? { x: wx, y: wy };
      measureSegRef.current = { a: measureSegRef.current.a, b: endPt };
      const dx = endPt.x - measureSegRef.current.a.x;
      const dy = endPt.y - measureSegRef.current.a.y;
      useStudioStore.getState().setMeasureReadout({
        dx,
        dy,
        dist: Math.hypot(dx, dy),
        angleDeg: (Math.atan2(dy, dx) * 180) / Math.PI,
      });
      dirtyRef.current = true;
    } else {
      hoverRef.current = kernel.hitTest(wx, wy, 10 / cam.scale);
      dirtyRef.current = true;
    }

    const now = performance.now();
    if (now - lastTelemetryRef.current > 45) {
      lastTelemetryRef.current = now;
      useStudioStore.getState().setCursorTelemetry({ x: wx, y: wy }, snapHit);
    }
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button === 2 && rmbRef.current?.active) {
      const rmb = rmbRef.current;
      rmbRef.current = null;
      const dx = e.clientX - rmb.startClientX;
      const dy = e.clientY - rmb.startClientY;
      const elapsed = performance.now() - rmb.startTime;
      const st = useStudioStore.getState();
      const flick = detectFlickGesture(dx, dy, elapsed, Boolean(st.seamCommand));

      if (flick) {
        if (flick.type === 'repeat') st.repeatLastCommand();
        else if (flick.type === 'commit') st.commitSeamCommand();
        else if (flick.type === 'cancel') st.closeSeamCommand();
        else if (flick.type === 'undo') st.executeCommand('undo');
        else if (flick.type === 'redo') st.executeCommand('redo');
        else if (flick.type === 'command') st.executeCommand(flick.commandId);
        else if (flick.type === 'flip_last' && st.seamCommand) {
          const slot = st.seamCommand.activeSlot;
          const list = slot === 'a' ? st.seamCommand.edgesA : st.seamCommand.edgesB;
          const last = list[list.length - 1];
          if (last) {
            st.flipSeamCommandEdgeDir(slot, last.edge_id);
            st.setStatusMessage(tr('v2d.flipped', { code: last.piece_code, edge: last.edge_label }));
          }
        }
        return;
      }

      if (rmb.hit.kind === 'edge' || rmb.hit.kind === 'vertex') {
        st.setSnapshot(kernel.setSelection(rmb.hit.piece_id, rmb.hit.edge_idx));
      } else if (rmb.hit.kind === 'piece') {
        st.setSnapshot(kernel.setSelection(rmb.hit.piece_id, null));
      }
      setCtxMenu({ x: rmb.sx, y: rmb.sy, hit: rmb.hit });
      return;
    }

    const drag = dragRef.current;
    if (drag.mode === 'draft_rect' && draftRectRef.current) {
      const dr = draftRectRef.current;
      draftRectRef.current = null;
      if (Math.abs(dr.x1 - dr.x0) > 15 && Math.abs(dr.y1 - dr.y0) > 15) {
        const nextSnap = kernel.addRectPiece('', dr.x0, dr.y0, dr.x1, dr.y1);
        useStudioStore.getState().setSnapshot(nextSnap);
        useStudioStore
          .getState()
          .setStatusMessage(
            tr('v2d.drafted', { w: Math.abs(dr.x1 - dr.x0).toFixed(0), h: Math.abs(dr.y1 - dr.y0).toFixed(0) }),
          );
      }
    } else if (drag.mode === 'piece' && drag.pieceId) {
      if (Math.hypot(drag.lastDx, drag.lastDy) > 2) {
        kernel.movePiece(drag.pieceId, drag.lastDx, drag.lastDy, true);
        useStudioStore.getState().setSnapshot(getSnapshot());
      }
    } else if (drag.mode === 'vertex' && drag.pieceId && drag.edgeIdx !== undefined) {
      kernel.moveVertex(drag.pieceId, drag.edgeIdx, drag.lastDx, drag.lastDy, true);
      useStudioStore.getState().setSnapshot(getSnapshot());
    } else if (
      drag.mode === 'handle' &&
      drag.pieceId &&
      drag.edgeIdx !== undefined &&
      drag.which !== undefined
    ) {
      kernel.moveHandle(
        drag.pieceId,
        drag.edgeIdx,
        drag.which,
        drag.lastDx,
        drag.lastDy,
        e.shiftKey,
        true,
      );
      useStudioStore.getState().setSnapshot(getSnapshot());
    } else if (drag.mode === 'pan') {
      const moved = Math.hypot(e.clientX - drag.startClientX, e.clientY - drag.startClientY);
      if (moved < 3 && useStudioStore.getState().activeTool === 'select') {
        useStudioStore.getState().setSnapshot(kernel.setSelection(null, null));
      }
    }
    dragRef.current.mode = 'none';
    dirtyRef.current = true;
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const { sx, sy, wx, wy } = toWorld(e.clientX, e.clientY);
      const cam = camRef.current;
      const factor = wheelZoomFactor(e);
      const nextScale = Math.max(0.08, Math.min(12, cam.scale * factor));
      camRef.current = {
        scale: nextScale,
        tx: sx - wx * nextScale,
        ty: sy + wy * nextScale,
      };
      useStudioStore.getState().setCursorTelemetry({ x: wx, y: wy }, snapRef.current, nextScale * 100);
      dirtyRef.current = true;
    };
    canvas.addEventListener('wheel', onWheel, { passive: false });
    return () => canvas.removeEventListener('wheel', onWheel);
  }, []);

  const cursorStyle =
    activeTool === 'pan'
      ? 'grab'
      : activeTool === 'direct_edit' ||
          activeTool === 'sew_segment' ||
          activeTool === 'draft_piece' ||
          activeTool === 'notch' ||
          activeTool === 'measure' ||
          activeTool === 'true_corner' ||
          activeTool === 'style_split' ||
          activeTool === 'drill_hole'
        ? 'crosshair'
        : 'default';

  return (
    <div style={style} className="relative flex-1 min-w-[240px] h-full overflow-hidden">
      <canvas
        data-viewport2d="true"
        ref={canvasRef}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onContextMenu={handleContextMenu}
        style={{ cursor: cursorStyle }}
        className="w-full h-full block touch-none"
      />

      {/* Fusion 360 8-Sector Radial Marking Menu + Contextual Menu */}
      {ctxMenu && (
        <RadialMarkingMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          viewportWidth={vpSize.w}
          viewportHeight={vpSize.h}
          hit={ctxMenu.hit}
          onClose={() => setCtxMenu(null)}
          onFitView={fitView}
        />
      )}

      {/* Navigation bar (bottom centre): zoom, fit and display toggles */}
      <div
        className="floating-panel absolute bottom-2 left-1/2 -translate-x-1/2 z-10 h-[30px] px-1 flex items-center gap-0.5 select-none text-[11px] whitespace-nowrap"
        role="toolbar"
        aria-label={t('nav.title')}
      >
        <button
          onClick={() => useStudioStore.getState().requestZoomStep(-1)}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] hover:bg-[var(--bg-hover)] cursor-pointer"
          title={t('v2d.zoomOut')}
          aria-label={t('v2d.zoomOut')}
        >
          <Minus className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={() => fitView()}
          className="h-[24px] px-1.5 flex items-center gap-1 rounded-[3px] hover:bg-[var(--bg-hover)] cursor-pointer"
          title={t('v2d.fitAll')}
        >
          <Maximize2 className="w-3.5 h-3.5" />
          {t('nav.fit')}
        </button>
        <button
          onClick={() => useStudioStore.getState().requestZoomStep(1)}
          className="w-[26px] h-[24px] flex items-center justify-center rounded-[3px] hover:bg-[var(--bg-hover)] cursor-pointer"
          title={t('v2d.zoomIn')}
          aria-label={t('v2d.zoomIn')}
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
        <div className="w-px h-4 mx-1" style={{ background: 'var(--border-subtle)' }} />
        {(
          [
            ['showSeamAllowance', 'nav.sa', 'nav.saTitle', showSA],
            ['showDimensions', 'nav.dims', 'nav.dimsTitle', showDimensions],
            ['showSeamColors', 'nav.seams', 'nav.seamsTitle', showSeamColors],
            ['showNotches', 'nav.notches', 'nav.notchesTitle', showNotches],
            ['showGrainlines', 'nav.grain', 'nav.grainTitle', showGrain],
            ['showGradedNest', 'nav.nest', 'nav.nestTitle', showNest],
          ] as const
        ).map(([key, label, title, active]) => (
          <button
            key={key}
            onClick={() => toggleLayer(key)}
            aria-pressed={active}
            className="h-[24px] px-1.5 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
            style={{
              background: active ? 'var(--accent-soft)' : undefined,
              color: active ? 'var(--accent)' : 'var(--text-secondary)',
            }}
            title={t(title)}
          >
            {t(label)}
          </button>
        ))}
        <button
          onClick={toggleMirrorUnfold}
          aria-pressed={showMirrorUnfold}
          className="h-[24px] px-1.5 rounded-[3px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={{
            background: showMirrorUnfold ? 'rgba(124, 58, 237, 0.14)' : undefined,
            color: showMirrorUnfold ? '#7c3aed' : 'var(--text-secondary)',
          }}
          title={t('v2d.ghost')}
        >
          {t('nav.symmetry')}
        </button>
      </div>
    </div>
  );
};
