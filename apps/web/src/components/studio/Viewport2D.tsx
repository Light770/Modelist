import React, { useEffect, useRef } from 'react';
import { Maximize2, Minus, Plus } from 'lucide-react';
import {
  getHandlePoints,
  getRenderVersion,
  getSnapshot,
  getStrokeCoords,
  getStrokeMeta,
  kernel,
} from '../../lib/kernelClient';
import { formatMm, type HitResult, type SnapHit } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

const RULER_PX = 24;

const NEST_PALETTE = ['#64748b', '#2563eb', '#059669', '#d97706', '#7c3aed', '#dc2626'];

interface Camera {
  tx: number;
  ty: number;
  scale: number; // px per mm
}

interface DragState {
  mode: 'none' | 'pan' | 'vertex' | 'handle' | 'measure';
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

export const Viewport2D: React.FC = () => {
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
  const dirtyRef = useRef(true);
  const lastVersionRef = useRef(0);
  const lastTelemetryRef = useRef(0);
  const spaceHeldRef = useRef(false);

  const activeTool = useStudioStore((s) => s.activeTool);
  const unit = useStudioStore((s) => s.unit);
  const theme = useStudioStore((s) => s.theme);
  const showSA = useStudioStore((s) => s.showSeamAllowance);
  const showNotches = useStudioStore((s) => s.showNotches);
  const showGrain = useStudioStore((s) => s.showGrainlines);
  const showNest = useStudioStore((s) => s.showGradedNest);
  const snapshot = useStudioStore((s) => s.snapshot);
  const walkFrame = useStudioStore((s) => s.walkFrame);
  const activeWalkLinkId = useStudioStore((s) => s.activeWalkLinkId);
  const walkProgress = useStudioStore((s) => s.walkProgress);
  const slashCount = useStudioStore((s) => s.slashCount);
  const slashSpreadMm = useStudioStore((s) => s.slashSpreadMm);
  const slashHingeMode = useStudioStore((s) => s.slashHingeMode);
  const dartPivotAngleDeg = useStudioStore((s) => s.dartPivotAngleDeg);
  const notchStyle = useStudioStore((s) => s.notchStyle);
  const notchCount = useStudioStore((s) => s.notchCount);
  const fitViewSeq = useStudioStore((s) => s.fitViewSeq);
  const zoomStepDir = useStudioStore((s) => s.zoomStepDir);

  const fitView = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
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
    const availW = Math.max(200, rect.width - RULER_PX - 110);
    const availH = Math.max(200, rect.height - RULER_PX - 120);
    const scale = Math.max(0.15, Math.min(3.0, Math.min(availW / spanX, availH / spanY)));
    const midX = 0.5 * (minX + maxX);
    const midY = 0.5 * (minY + maxY);
    camRef.current = {
      scale,
      tx: RULER_PX + (rect.width - RULER_PX) * 0.5 - midX * scale,
      ty: RULER_PX + (rect.height - RULER_PX) * 0.5 + midY * scale,
    };
    useStudioStore.getState().setCursorTelemetry(
      useStudioStore.getState().cursorWorld,
      snapRef.current,
      scale * 100,
    );
    dirtyRef.current = true;
  };

  useEffect(() => {
    fitView();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fitViewSeq]);

  useEffect(() => {
    if (zoomStepDir === 0) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const cx = (rect.width + RULER_PX) * 0.5;
    const cy = (rect.height + RULER_PX) * 0.5;
    const factor = zoomStepDir > 0 ? 1.18 : 1 / 1.18;
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
  }, [zoomStepDir]);

  useEffect(() => {
    dirtyRef.current = true;
  }, [theme, showSA, showNotches, showGrain, showNest, unit, activeTool, snapshot]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.code === 'Space' && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) {
        spaceHeldRef.current = true;
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code === 'Space') spaceHeldRef.current = false;
    };
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
    };
  }, []);

  // Main Imperative 60-120 FPS Render Loop (zero React DOM overhead)
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

      ctx.save();
      ctx.scale(dpr, dpr);

      // Clear background
      ctx.fillStyle = bgCanvas;
      ctx.fillRect(0, 0, w, h);

      // World grid inside drafting area
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

      // Draw geometry directly from WASM typed arrays
      const state = useStudioStore.getState();
      const coords = getStrokeCoords();
      const meta = getStrokeMeta();
      const segCount = Math.min(coords.length >> 2, Math.floor(meta.length / 3));

      const toSX = (wx: number) => cam.tx + wx * cam.scale;
      const toSY = (wy: number) => cam.ty - wy * cam.scale;

      // Batch consecutive segments of the same style into single stroke calls for top FPS
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
        if (role === 4 && !state.showNotches) continue;
        if (role === 3 && !state.showGrainlines) continue;
        if (role === 5 && !state.showGradedNest) continue;

        if (role !== activeRole || (role === 5 && subIdx !== activeSub)) {
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
              ctx.lineWidth = 1.25;
              ctx.setLineDash([6, 4]);
              break;
            case 2: // Dart / Internal
              ctx.strokeStyle = mutedColor;
              ctx.lineWidth = 1.1;
              ctx.setLineDash([4, 3]);
              break;
            case 3: // Grainline
              ctx.strokeStyle = grainColor;
              ctx.lineWidth = 1.15;
              break;
            case 4: // Notch
              ctx.strokeStyle = notchColor;
              ctx.lineWidth = 1.75;
              break;
            case 5: // Graded size
              ctx.strokeStyle = NEST_PALETTE[subIdx % NEST_PALETTE.length];
              ctx.lineWidth = 1.05;
              ctx.setLineDash([5, 3]);
              break;
            case 6: // Handle guide
              ctx.strokeStyle = accentColor;
              ctx.lineWidth = 1.0;
              ctx.setLineDash([2, 2]);
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
              ctx.lineWidth = 3.0;
              break;
            case 10: // Mirror fold
              ctx.strokeStyle = '#7c3aed';
              ctx.lineWidth = 1.85;
              ctx.setLineDash([10, 4, 2, 4]);
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

      // Draw interactive handles for selected piece
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

      // Piece labels
      const snapUi = state.snapshot;
      if (snapUi) {
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        for (const lbl of snapUi.labels) {
          const sx = toSX(lbl.x);
          const sy = toSY(lbl.y);
          const isSel = lbl.piece_id === snapUi.selected_piece_id;
          ctx.font = '600 12px Inter, sans-serif';
          ctx.fillStyle = isSel ? accentColor : inkColor;
          ctx.fillText(lbl.text, sx, sy - 8);
          ctx.font = '500 10px "JetBrains Mono", monospace';
          ctx.fillStyle = mutedColor;
          ctx.fillText(lbl.sub, sx, sy + 8);
        }
      }

      // Walk-Seam contact marker
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

      // Snap indicator badge
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

      // Caliper measure line
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

  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    canvas.setPointerCapture(e.pointerId);
    const { sx, sy, wx, wy } = toWorld(e.clientX, e.clientY);
    const cam = camRef.current;
    const state = useStudioStore.getState();

    // Middle mouse or Pan tool or Spacebar -> Pan
    if (e.button === 1 || state.activeTool === 'pan' || spaceHeldRef.current) {
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
        state.setStatusMessage(`Added ${state.notchCount}x ${state.notchStyle} notch on edge #${hit.edge_idx}`);
        dirtyRef.current = true;
      } else {
        state.setStatusMessage('Click directly on a seamline edge to place a notch');
      }
      return;
    }

    // Dart Pivot tool
    if (state.activeTool === 'dart_pivot') {
      const piece = state.snapshot?.pieces.find((p) => p.id === (hit.kind !== 'none' ? hit.piece_id : state.snapshot?.selected_piece_id));
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
        state.setStatusMessage(`Pivoted ${dart.name} (${state.dartPivotAngleDeg}°) to edge #${hit.edge_idx}`);
        dirtyRef.current = true;
      } else {
        state.setStatusMessage('Select a piece with a dart, then click the target seamline edge to pivot into');
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
          state.setStatusMessage(`Applied ${state.slashCount}x Slash & Spread (+${state.slashSpreadMm} mm)`);
          dirtyRef.current = true;
        }
      } else {
        state.setStatusMessage('Click the target edge where fullness should spread open');
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

    if (hit.kind === 'piece') {
      state.setSnapshot(kernel.setSelection(hit.piece_id, null));
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
    void sx;
    void sy;
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const { sx, sy, wx, wy } = toWorld(e.clientX, e.clientY);
    cursorScreenRef.current = { x: sx, y: sy };
    const drag = dragRef.current;
    const cam = camRef.current;

    if (drag.mode === 'pan') {
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

    if (drag.mode === 'vertex' && drag.pieceId && drag.edgeIdx !== undefined) {
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
    const drag = dragRef.current;
    if (drag.mode === 'vertex' && drag.pieceId && drag.edgeIdx !== undefined) {
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

  const handleWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const { sx, sy, wx, wy } = toWorld(e.clientX, e.clientY);
    const cam = camRef.current;
    const factor = e.deltaY < 0 ? 1.12 : 1 / 1.12;
    const nextScale = Math.max(0.08, Math.min(12, cam.scale * factor));
    camRef.current = {
      scale: nextScale,
      tx: sx - wx * nextScale,
      ty: sy + wy * nextScale,
    };
    useStudioStore.getState().setCursorTelemetry({ x: wx, y: wy }, snapRef.current, nextScale * 100);
    dirtyRef.current = true;
  };

  const cursorStyle =
    activeTool === 'pan'
      ? 'grab'
      : activeTool === 'direct_edit' || activeTool === 'notch' || activeTool === 'measure'
        ? 'crosshair'
        : 'default';

  return (
    <div className="relative flex-1 h-full overflow-hidden">
      <canvas
        ref={canvasRef}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onWheel={handleWheel}
        style={{ cursor: cursorStyle }}
        className="w-full h-full block touch-none"
      />

      {/* Contextual Floating Tool HUD */}
      {activeTool === 'walk_seam' && snapshot && (
        <div
          className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-xl border shadow-lg px-4 py-3 flex flex-wrap items-center gap-4 text-xs backdrop-blur-md"
          style={{
            background: 'var(--bg-surface)',
            borderColor: 'var(--border-strong)',
          }}
        >
          <div className="flex items-center gap-2">
            <span className="font-semibold text-emerald-600 dark:text-emerald-400">Walk Seam</span>
            <select
              value={activeWalkLinkId ?? ''}
              onChange={(e) => useStudioStore.getState().startWalkSeam(e.target.value)}
              className="rounded border px-2 py-1 text-xs"
              style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}
            >
              {snapshot.stitching_links.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.label}
                </option>
              ))}
            </select>
          </div>

          <div className="flex items-center gap-2">
            <span style={{ color: 'var(--text-secondary)' }}>Progress</span>
            <input
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={walkProgress}
              onChange={(e) => useStudioStore.getState().updateWalkProgress(parseFloat(e.target.value))}
              className="w-36"
            />
            <span className="font-mono-num w-10 text-right">{(walkProgress * 100).toFixed(0)}%</span>
          </div>

          {walkFrame && (
            <div className="font-mono-num flex items-center gap-3">
              <span>A: {formatMm(walkFrame.walked_a_mm, unit)}</span>
              <span>B: {formatMm(walkFrame.walked_b_mm, unit)}</span>
              <span className="px-2 py-0.5 rounded bg-emerald-500/15 text-emerald-600 dark:text-emerald-400">
                Ease {walkFrame.ease_delta_mm >= 0 ? '+' : ''}
                {formatMm(walkFrame.ease_delta_mm, unit)}
              </span>
            </div>
          )}

          <button
            onClick={() => useStudioStore.getState().stopWalkSeam()}
            className="px-2.5 py-1 rounded border text-xs hover:bg-[var(--bg-subtle)] cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
          >
            Done
          </button>
        </div>
      )}

      {activeTool === 'dart_pivot' && (
        <div
          className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-xl border shadow-lg px-4 py-2.5 flex items-center gap-4 text-xs"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
        >
          <span className="font-semibold">Dart Pivot:</span>
          <label className="flex items-center gap-2">
            <span>Rotation Angle</span>
            <input
              type="range"
              min={4}
              max={30}
              step={1}
              value={dartPivotAngleDeg}
              onChange={(e) => useStudioStore.getState().setDartPivotAngle(Number(e.target.value))}
              className="w-28"
            />
            <span className="font-mono-num">{dartPivotAngleDeg}°</span>
          </label>
          <span style={{ color: 'var(--text-secondary)' }}>Click target seamline edge on canvas</span>
        </div>
      )}

      {activeTool === 'slash_spread' && (
        <div
          className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-xl border shadow-lg px-4 py-2.5 flex items-center gap-4 text-xs"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
        >
          <span className="font-semibold">Slash &amp; Spread:</span>
          <label className="flex items-center gap-1.5">
            <span>Slashes</span>
            <input
              type="number"
              min={1}
              max={6}
              value={slashCount}
              onChange={(e) =>
                useStudioStore
                  .getState()
                  .setSlashParams(Number(e.target.value), slashSpreadMm, slashHingeMode)
              }
              className="w-12 rounded border px-1.5 py-0.5 font-mono-num"
              style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}
            />
          </label>
          <label className="flex items-center gap-1.5">
            <span>Spread (mm)</span>
            <input
              type="number"
              min={5}
              max={80}
              value={slashSpreadMm}
              onChange={(e) =>
                useStudioStore
                  .getState()
                  .setSlashParams(slashCount, Number(e.target.value), slashHingeMode)
              }
              className="w-14 rounded border px-1.5 py-0.5 font-mono-num"
              style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}
            />
          </label>
          <select
            value={slashHingeMode}
            onChange={(e) =>
              useStudioStore
                .getState()
                .setSlashParams(
                  slashCount,
                  slashSpreadMm,
                  e.target.value as 'edge_hinge' | 'parallel_shift',
                )
            }
            className="rounded border px-2 py-0.5"
            style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}
          >
            <option value="edge_hinge">Edge Hinge Pivot</option>
            <option value="parallel_shift">Parallel Fullness</option>
          </select>
          <span style={{ color: 'var(--text-secondary)' }}>Click edge to open fullness</span>
        </div>
      )}

      {activeTool === 'notch' && (
        <div
          className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-xl border shadow-lg px-4 py-2.5 flex items-center gap-3 text-xs"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
        >
          <span className="font-semibold">Notch Tool:</span>
          <select
            value={notchStyle}
            onChange={(e) =>
              useStudioStore
                .getState()
                .setNotchOptions(e.target.value as typeof notchStyle, notchCount)
            }
            className="rounded border px-2 py-0.5"
            style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}
          >
            <option value="slit">Slit</option>
            <option value="v">V-Notch</option>
            <option value="t">T-Notch</option>
            <option value="u">U-Notch</option>
            <option value="castle">Castle</option>
          </select>
          <select
            value={notchCount}
            onChange={(e) =>
              useStudioStore.getState().setNotchOptions(notchStyle, Number(e.target.value))
            }
            className="rounded border px-2 py-0.5 font-mono-num"
            style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}
          >
            <option value={1}>1x Single</option>
            <option value={2}>2x Double</option>
            <option value={3}>3x Triple</option>
          </select>
          <span style={{ color: 'var(--text-secondary)' }}>Click any seamline edge</span>
        </div>
      )}

      {activeTool === 'direct_edit' && (
        <div
          className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded-full border shadow px-3.5 py-1.5 text-[11px]"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}
        >
          Drag vertices or blue Bézier handles · Hold <kbd className="font-mono-num font-semibold">Shift</kbd> for G1 tangent lock
        </div>
      )}

      {/* Bottom-Right Zoom Controls */}
      <div
        className="absolute bottom-4 right-4 rounded-lg border shadow-sm flex items-center overflow-hidden"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
      >
        <button
          onClick={() => useStudioStore.getState().requestZoomStep(-1)}
          className="p-2 hover:bg-[var(--bg-subtle)] cursor-pointer"
          title="Zoom Out"
        >
          <Minus className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={() => useStudioStore.getState().requestFitView()}
          className="px-2.5 py-2 border-x text-[11px] font-mono-num hover:bg-[var(--bg-subtle)] cursor-pointer flex items-center gap-1"
          style={{ borderColor: 'var(--border-subtle)' }}
          title="Fit All Pieces (0)"
        >
          <Maximize2 className="w-3 h-3" />
          Fit
        </button>
        <button
          onClick={() => useStudioStore.getState().requestZoomStep(1)}
          className="p-2 hover:bg-[var(--bg-subtle)] cursor-pointer"
          title="Zoom In"
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );
};
