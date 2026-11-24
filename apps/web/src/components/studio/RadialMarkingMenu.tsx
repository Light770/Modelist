import React, { useMemo, useState } from 'react';
import {
  ALL_SECTORS,
  build2DRadialWheel,
  buildPatternOpsSubWheel,
  SECTOR_ANGLES_DEG,
  sectorFromDelta,
  type RadialItem,
  type SectorDir,
} from '../../lib/cad/radial-menu';
import { kernel } from '../../lib/kernelClient';
import type { HitResult, PieceSummary } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';
import { useCatalogT, useT } from '../../lib/i18n';

interface RadialMarkingMenuProps {
  /** Canvas-relative X/Y coordinates where right-click occurred */
  x: number;
  y: number;
  viewportWidth: number;
  viewportHeight: number;
  hit: HitResult;
  onClose: () => void;
  onFitView: () => void;
}

const WHEEL_SIZE = 220;
const WHEEL_CENTER = WHEEL_SIZE / 2;
const ITEM_RADIUS = 72;

export const RadialMarkingMenu: React.FC<RadialMarkingMenuProps> = ({
  x,
  y,
  viewportWidth,
  viewportHeight,
  hit,
  onClose,
  onFitView,
}) => {
  const t = useT();
  const ct = useCatalogT();
  // Radial items carry English labels from lib/cad; translate them by id at display time.
  const radialLabel = (item: { id: string; label: string }) => {
    if (item.id === 'repeat') {
      const m = item.label.match(/^Repeat: (.*)$/);
      return m ? t('radialItem.repeatCmd', { command: m[1] }) : t('radialItem.repeat');
    }
    return ct('radialItem', item.id, item.label);
  };
  const radialShort = (item: { id: string; label: string; shortLabel?: string }) => {
    const shortKeys: Record<string, string> = {
      repeat: 'repeatShort',
      sew_segment: 'sewShort',
      pattern_ops: 'opsShort',
      cancel_cmd: 'cancelShort',
    };
    if (item.shortLabel && shortKeys[item.id]) return ct('radialItem', shortKeys[item.id], item.shortLabel);
    return radialLabel(item);
  };
  const snapshot = useStudioStore((s) => s.snapshot);
  const seamCommand = useStudioStore((s) => s.seamCommand);
  const lastCommandId = useStudioStore((s) => s.lastCommandId);
  const executeCommand = useStudioStore((s) => s.executeCommand);
  const repeatLastCommand = useStudioStore((s) => s.repeatLastCommand);
  const commitSeamCommand = useStudioStore((s) => s.commitSeamCommand);
  const closeSeamCommand = useStudioStore((s) => s.closeSeamCommand);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);

  const [subMenu, setSubMenu] = useState<'root' | 'pattern_ops'>('root');
  const [hoverSector, setHoverSector] = useState<SectorDir | null>(null);
  const [pointerRel, setPointerRel] = useState<{ dx: number; dy: number }>({ dx: 0, dy: 0 });

  const inCommand = Boolean(seamCommand);
  const lastCommandLabel = useMemo(() => {
    if (!lastCommandId) return null;
    return lastCommandId.replace(/_/g, ' ');
  }, [lastCommandId]);

  const wheel = useMemo(
    () =>
      subMenu === 'pattern_ops'
        ? buildPatternOpsSubWheel()
        : build2DRadialWheel(lastCommandLabel, inCommand),
    [inCommand, lastCommandLabel, subMenu],
  );

  // Clamp wheel center cleanly inside viewport bounds
  const pad = WHEEL_CENTER + 16;
  const cx = Math.max(pad, Math.min(x, Math.max(pad, viewportWidth - pad)));
  const cy = Math.max(pad, Math.min(y, Math.max(pad, viewportHeight - pad - 150)));

  const ctxPiece: PieceSummary | undefined =
    hit.kind !== 'none'
      ? snapshot?.pieces.find((p) => p.id === hit.piece_id)
      : undefined;
  const ctxEdge =
    ctxPiece && (hit.kind === 'edge' || hit.kind === 'vertex')
      ? ctxPiece.edges.find((e) => e.edge_idx === hit.edge_idx)
      : undefined;

  const activateItem = (item: RadialItem) => {
    if (item.disabled) return;
    if (item.subMenu === 'pattern_ops') {
      setSubMenu('pattern_ops');
      setHoverSector(null);
      return;
    }
    onClose();
    if (!item.commandId) return;
    if (item.commandId === '__repeat__') {
      repeatLastCommand();
    } else if (item.commandId === '__commit_command__') {
      commitSeamCommand();
    } else if (item.commandId === '__cancel_command__') {
      closeSeamCommand();
    } else if (item.commandId === 'fit_view') {
      onFitView();
    } else {
      executeCommand(item.commandId);
    }
  };

  return (
    <div
      className="absolute inset-0 z-30 select-none"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}
      onContextMenu={(e) => e.preventDefault()}
    >
      {/* 8-Sector Radial Compass Wheel */}
      <div
        className="absolute"
        style={{
          left: cx - WHEEL_CENTER,
          top: cy - WHEEL_CENTER,
          width: WHEEL_SIZE,
          height: WHEEL_SIZE,
        }}
        onPointerMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const dx = e.clientX - (rect.left + WHEEL_CENTER);
          const dy = e.clientY - (rect.top + WHEEL_CENTER);
          setPointerRel({ dx, dy });
          setHoverSector(sectorFromDelta(dx, dy, 18));
        }}
        onPointerLeave={() => {
          setHoverSector(null);
          setPointerRel({ dx: 0, dy: 0 });
        }}
      >
        {/* Subtle radial direction line from cursor center */}
        <svg
          width={WHEEL_SIZE}
          height={WHEEL_SIZE}
          className="pointer-events-none absolute inset-0"
        >
          <circle
            cx={WHEEL_CENTER}
            cy={WHEEL_CENTER}
            r={ITEM_RADIUS}
            fill="none"
            stroke="var(--border-subtle)"
            strokeDasharray="3 3"
            opacity={0.7}
          />
          {hoverSector && (
            <line
              x1={WHEEL_CENTER}
              y1={WHEEL_CENTER}
              x2={WHEEL_CENTER + pointerRel.dx}
              y2={WHEEL_CENTER + pointerRel.dy}
              stroke="var(--accent)"
              strokeWidth={1.6}
              opacity={0.7}
            />
          )}
        </svg>

        {/* Center hub button */}
        <button
          type="button"
          onClick={() => (subMenu === 'pattern_ops' ? setSubMenu('root') : onClose())}
          className="absolute w-8 h-8 -ml-4 -mt-4 rounded-full border shadow-md flex items-center justify-center text-[10px] font-semibold cursor-pointer"
          style={{
            left: WHEEL_CENTER,
            top: WHEEL_CENTER,
            background: 'var(--bg-surface)',
            borderColor: 'var(--border-strong)',
            color: 'var(--text-secondary)',
          }}
          title={subMenu === 'pattern_ops' ? t('radial.back') : t('common.close')}
        >
          {subMenu === 'pattern_ops' ? '←' : '×'}
        </button>

        {ALL_SECTORS.map((dir) => {
          const item = wheel[dir];
          const rad = (SECTOR_ANGLES_DEG[dir] * Math.PI) / 180;
          const ix = WHEEL_CENTER + Math.cos(rad) * ITEM_RADIUS;
          const iy = WHEEL_CENTER + Math.sin(rad) * ITEM_RADIUS;
          const isHot = hoverSector === dir && !item.disabled;
          return (
            <button
              key={dir}
              type="button"
              disabled={item.disabled}
              onClick={() => activateItem(item)}
              className="absolute -translate-x-1/2 -translate-y-1/2 px-2 py-1 rounded-full border shadow-md text-[10px] font-medium whitespace-nowrap transition cursor-pointer disabled:opacity-35"
              style={{
                left: ix,
                top: iy,
                background: isHot ? 'var(--accent)' : 'var(--bg-surface)',
                borderColor: isHot ? 'var(--accent)' : 'var(--border-strong)',
                color: isHot ? '#ffffff' : 'var(--text-primary)',
              }}
              title={radialLabel(item)}
            >
              {radialShort(item)}
            </button>
          );
        })}
      </div>

      {/* Contextual Linear Actions Card (Anchored cleanly below the wheel) */}
      {(ctxEdge || ctxPiece || hit.kind === 'none') && (
        <div
          className="absolute min-w-[216px] max-w-[248px] rounded-xl border shadow-xl py-1.5 text-xs backdrop-blur-md"
          style={{
            left: Math.max(12, Math.min(cx - 108, viewportWidth - 256)),
            top: Math.min(cy + WHEEL_CENTER - 6, viewportHeight - 190),
            background: 'var(--bg-surface)',
            borderColor: 'var(--border-strong)',
          }}
        >
          {hit.kind === 'edge' && ctxPiece && ctxEdge && (
            <>
              <div
                className="px-3 py-1 text-[10px] font-semibold uppercase tracking-wider truncate"
                style={{ color: 'var(--text-muted)' }}
              >
                {ctxPiece.code} · {ctxEdge.label}
              </div>
              <button
                onClick={() => {
                  setSnapshot(kernel.convertEdgeCurve(ctxPiece.id, ctxEdge.edge_idx, !ctxEdge.is_cubic));
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
              >
                {ctxEdge.is_cubic ? t('radial.toStraight') : t('radial.toCurve')}
              </button>
              <button
                onClick={() => {
                  setSnapshot(
                    kernel.splitEdgePoint(ctxPiece.id, ctxEdge.edge_idx, hit.param_t),
                  );
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
              >
                Insert Point Here (Split Edge)
              </button>
              <button
                onClick={() => {
                  const raw = window.prompt(
                    t('radial.lengthPrompt', { label: ctxEdge.label, length: ctxEdge.length_mm.toFixed(1) }),
                    ctxEdge.length_mm.toFixed(1),
                  );
                  if (raw) {
                    const val = parseFloat(raw);
                    if (Number.isFinite(val) && val > 5) {
                      setSnapshot(kernel.setEdgeLength(ctxPiece.id, ctxEdge.edge_idx, val));
                    }
                  }
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
              >
                Set Edge Length (mm)…
              </button>
              <button
                onClick={() => {
                  const st = useStudioStore.getState();
                  const existing = st.snapshot?.stitching_links.find(
                    (l) =>
                      l.edges_a.some((er) => er.edge_id === ctxEdge.edge_id) ||
                      l.edges_b.some((er) => er.edge_id === ctxEdge.edge_id),
                  );
                  if (existing) {
                    st.openEditSeamCommand(existing);
                  } else {
                    st.openCreateSeamCommand({
                      piece_id: ctxPiece.id,
                      piece_code: ctxPiece.code,
                      edge_id: ctxEdge.edge_id,
                      edge_idx: ctxEdge.edge_idx,
                      edge_label: ctxEdge.label,
                      length_mm: ctxEdge.length_mm,
                      dir: 1,
                    });
                  }
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer font-medium"
                style={{ color: 'var(--accent)' }}
              >
                {snapshot?.stitching_links.some(
                  (l) =>
                    l.edges_a.some((er) => er.edge_id === ctxEdge.edge_id) ||
                    l.edges_b.some((er) => er.edge_id === ctxEdge.edge_id),
                )
                  ? t('radial.editSeam')
                  : t('radial.sewFromEdge')}
              </button>
              <div className="my-1 border-t" style={{ borderColor: 'var(--border-subtle)' }} />
              <div className="px-3 py-1 flex items-center gap-1">
                <span className="text-[10px] mr-1" style={{ color: 'var(--text-muted)' }}>
                  SA:
                </span>
                {[0, 7, 10, 15, 30].map((mm) => (
                  <button
                    key={mm}
                    onClick={() => {
                      setSnapshot(
                        kernel.setEdgeSa(
                          ctxPiece.id,
                          ctxEdge.edge_idx,
                          mm,
                          mm === 30 ? { kind: 'folded_hem' } : { kind: 'miter', limit: 2.5 },
                        ),
                      );
                      onClose();
                    }}
                    className="px-1.5 py-0.5 rounded border text-[10px] font-mono-num hover:bg-[var(--bg-subtle)] cursor-pointer"
                    style={{ borderColor: 'var(--border-subtle)' }}
                  >
                    {mm}
                  </button>
                ))}
              </div>
            </>
          )}

          {hit.kind === 'vertex' && ctxPiece && (
            <>
              <div
                className="px-3 py-1 text-[10px] font-semibold uppercase tracking-wider"
                style={{ color: 'var(--text-muted)' }}
              >
                Corner Vertex #{hit.edge_idx + 1}
              </div>
              <button
                onClick={() => {
                  setSnapshot(kernel.trueCorner(ctxPiece.id, hit.edge_idx, true));
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
              >
                True Corner: 90° Square-Off
              </button>
              <button
                onClick={() => {
                  setSnapshot(kernel.trueCorner(ctxPiece.id, hit.edge_idx, false));
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
              >
                True Corner: G1 Smooth Curve
              </button>
            </>
          )}

          {ctxPiece && (
            <>
              {hit.kind !== 'piece' && (
                <div className="my-1 border-t" style={{ borderColor: 'var(--border-subtle)' }} />
              )}
              <button
                onClick={() => {
                  setSnapshot(kernel.mirrorPiece(ctxPiece.id));
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
              >
                Flip / Mirror {ctxPiece.code}
              </button>
              <button
                onClick={() => {
                  setSnapshot(kernel.duplicatePiece(ctxPiece.id));
                  onClose();
                }}
                className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
              >
                Duplicate {ctxPiece.code}
              </button>
            </>
          )}

          {hit.kind === 'none' && (
            <button
              onClick={() => {
                onClose();
                onFitView();
              }}
              className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
            >
              Fit All Pieces to Viewport (0)
            </button>
          )}
        </div>
      )}
    </div>
  );
};
