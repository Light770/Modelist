import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  Check,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  EyeOff,
  Filter,
  Layers,
  Link2,
  Pause,
  PenTool,
  Play,
  Scissors,
  Sparkles,
  Trash2,
  X,
  XCircle,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import {
  isDropSlotValid,
  nextRollbackStep,
  opCategory,
  targetIndexFromDropSlot,
} from '../../lib/cad/timeline-deps';
import type { CornerRule, DisplayUnit, OpSummary } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';
import { useT } from '../../lib/i18n';
import { ExpressionInput } from './ExpressionInput';

function iconForOp(kind: string) {
  const cat = opCategory(kind);
  if (cat === 'seam') return Link2;
  if (cat === 'cut') return Scissors;
  if (cat === 'curve') return PenTool;
  return Sparkles;
}

function accentForOp(op: OpSummary): string {
  const cat = opCategory(op.kind);
  if (cat === 'seam') return '#0d9488';
  if (cat === 'cut') return '#d97706';
  if (cat === 'curve') return '#2563eb';
  if (cat === 'marking') return '#7c3aed';
  return '#64748b';
}

export const ParametricTimeline: React.FC = () => {
  const t = useT();
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const editingOpId = useStudioStore((s) => s.editingOpId);
  const filterByPiece = useStudioStore((s) => s.timelineFilterBySelectedPiece);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);
  const openEditOp = useStudioStore((s) => s.openEditOp);
  const closeEditOp = useStudioStore((s) => s.closeEditOp);
  const openEditSeamCommand = useStudioStore((s) => s.openEditSeamCommand);
  const setTimelineFilterBySelectedPiece = useStudioStore((s) => s.setTimelineFilterBySelectedPiece);

  const [playing, setPlaying] = useState(false);
  const [dragOpIdx, setDragOpIdx] = useState<number | null>(null);
  const [draggingRollback, setDraggingRollback] = useState(false);
  const [dropSlot, setDropSlot] = useState<number | null>(null);
  const [chipMenu, setChipMenu] = useState<{ op: OpSummary; x: number; y: number } | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);

  const ops = snapshot?.ops ?? [];
  const totalOps = ops.length;
  const rollbackIndex = snapshot?.rollback_index ?? null;
  const effectiveRollback = rollbackIndex === null ? totalOps : Math.max(0, Math.min(totalOps, rollbackIndex));

  // History playback animation loop (steps smoothly until reaching end)
  useEffect(() => {
    if (!playing) return;
    if (totalOps === 0) {
      setPlaying(false);
      return;
    }
    const timer = window.setInterval(() => {
      const curSnap = useStudioStore.getState().snapshot;
      const curLen = curSnap?.ops.length ?? 0;
      const curRb = curSnap?.rollback_index ?? null;
      const next = nextRollbackStep(curRb, curLen, 'next');
      setSnapshot(kernel.setRollbackIndex(next));
      if (next === null) {
        setPlaying(false);
      }
    }, 450);
    return () => window.clearInterval(timer);
  }, [playing, setSnapshot, totalOps]);

  useEffect(() => {
    if (!chipMenu) return;
    const onDown = (e: PointerEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setChipMenu(null);
      }
    };
    window.addEventListener('pointerdown', onDown);
    return () => window.removeEventListener('pointerdown', onDown);
  }, [chipMenu]);

  const visibleOps = useMemo(() => {
    if (!filterByPiece || !snapshot?.selected_piece_id) return ops;
    const pid = snapshot.selected_piece_id;
    return ops.filter((o) => o.piece_ids.length === 0 || o.piece_ids.includes(pid));
  }, [filterByPiece, ops, snapshot?.selected_piece_id]);

  if (!snapshot) return null;

  const isFiltered = Boolean(filterByPiece && snapshot.selected_piece_id);
  const draggedOp = dragOpIdx !== null ? ops[dragOpIdx] : undefined;

  const handleOpenEdit = (op: OpSummary) => {
    if (op.binding_id && (op.kind === 'add_seam_binding' || op.kind === 'replace_seam_binding' || op.kind === 'update_seam_binding')) {
      const link = snapshot.stitching_links.find((l) => l.id === op.binding_id);
      if (link) {
        openEditSeamCommand(link);
        return;
      }
    }
    if (op.editable) {
      openEditOp(op.id);
    }
  };

  const renderSlot = (slotIdx: number, allowOpReorder: boolean) => {
    const isRbHere = effectiveRollback === slotIdx;
    const isHot = dropSlot === slotIdx;
    const opValid =
      dragOpIdx !== null && draggedOp && allowOpReorder
        ? isDropSlotValid(dragOpIdx, slotIdx, draggedOp.reorder_min, draggedOp.reorder_max)
        : false;
    const opBlocked =
      dragOpIdx !== null &&
      draggedOp &&
      allowOpReorder &&
      targetIndexFromDropSlot(dragOpIdx, slotIdx) !== dragOpIdx &&
      !opValid;

    return (
      <div
        key={`slot-${slotIdx}`}
        onDragOver={(e) => {
          if (draggingRollback || (dragOpIdx !== null && allowOpReorder)) {
            e.preventDefault();
            if (dropSlot !== slotIdx) setDropSlot(slotIdx);
          }
        }}
        onDragLeave={() => {
          if (dropSlot === slotIdx) setDropSlot(null);
        }}
        onDrop={(e) => {
          e.preventDefault();
          if (draggingRollback) {
            const nextRb = slotIdx >= totalOps ? null : slotIdx;
            setSnapshot(kernel.setRollbackIndex(nextRb));
            setStatusMessage(
              nextRb === null
                ? t('timeline.rolledEnd')
                : t('timeline.rolledBack', { step: nextRb, total: totalOps }),
            );
          } else if (dragOpIdx !== null && draggedOp && allowOpReorder) {
            const toIdx = targetIndexFromDropSlot(dragOpIdx, slotIdx);
            if (isDropSlotValid(dragOpIdx, slotIdx, draggedOp.reorder_min, draggedOp.reorder_max)) {
              const res = kernel.reorderOp(dragOpIdx, toIdx);
              setSnapshot(res.snapshot);
              if (res.moved) {
                setStatusMessage(t('timeline.reordered', { index: toIdx + 1 }));
              }
            } else {
              setStatusMessage(t('timeline.blocked'));
            }
          }
          setDragOpIdx(null);
          setDraggingRollback(false);
          setDropSlot(null);
        }}
        className="relative flex items-center justify-center shrink-0 h-6 transition-all"
        style={{
          width: isRbHere ? 14 : dragOpIdx !== null || draggingRollback ? 12 : 8,
        }}
        title={isRbHere ? t('timeline.rollbackMarker') : undefined}
      >
        {/* Dependency drop bar */}
        {dragOpIdx !== null && isHot && (
          <div
            className="w-1 h-5 rounded-full"
            style={{
              background: opValid ? '#10b981' : opBlocked ? '#e11d48' : 'var(--text-muted)',
            }}
          />
        )}

        {/* Draggable Rollback Marker (Fusion 360 golden/amber vertical handle) */}
        {isRbHere && !dragOpIdx && (
          <div
            draggable={!isFiltered && totalOps > 0}
            onDragStart={(e) => {
              e.dataTransfer.effectAllowed = 'move';
              e.dataTransfer.setData('text/plain', 'rollback');
              setDraggingRollback(true);
              setPlaying(false);
            }}
            onDragEnd={() => {
              setDraggingRollback(false);
              setDropSlot(null);
            }}
            className={`flex flex-col items-center justify-center h-6 ${totalOps > 0 ? 'cursor-ew-resize' : ''}`}
          >
            <span
              className="w-2.5 h-1.5 rounded-t-xs"
              style={{ background: rollbackIndex !== null ? '#f59e0b' : 'var(--accent)' }}
            />
            <span
              className="w-0.5 flex-1"
              style={{ background: rollbackIndex !== null ? '#f59e0b' : 'var(--accent)' }}
            />
          </div>
        )}

        {!isRbHere && !(dragOpIdx !== null && isHot) && (
          <span className="opacity-30 text-[10px] pointer-events-none">›</span>
        )}
      </div>
    );
  };

  const editingOp = editingOpId ? ops.find((o) => o.id === editingOpId) ?? null : null;

  return (
    <>
      <div className="h-full flex-1 min-w-0 px-2 flex items-center gap-1 overflow-x-auto select-none text-[10px]">
        {/* Fusion-style Timeline Transport Controls */}
        <div
          className="inline-flex items-center rounded-md border px-0.5 py-0.5 gap-0.5 shrink-0"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
        >
          <button
            disabled={totalOps === 0}
            onClick={() => {
              setPlaying(false);
              setSnapshot(kernel.setRollbackIndex(0));
            }}
            className="p-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer disabled:opacity-35"
            title={t('timeline.toStart')}
          >
            <ChevronsLeft className="w-3 h-3" />
          </button>
          <button
            disabled={totalOps === 0 || effectiveRollback === 0}
            onClick={() => {
              setPlaying(false);
              setSnapshot(kernel.setRollbackIndex(nextRollbackStep(rollbackIndex, totalOps, 'prev')));
            }}
            className="p-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer disabled:opacity-35"
            title={t('timeline.stepBack')}
          >
            <ChevronLeft className="w-3 h-3" />
          </button>
          <button
            disabled={totalOps === 0}
            onClick={() => {
              if (!playing && rollbackIndex === null && totalOps > 0) {
                setSnapshot(kernel.setRollbackIndex(0));
              }
              setPlaying((p) => !p);
            }}
            className="p-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer disabled:opacity-35"
            style={{ color: playing ? 'var(--accent)' : undefined }}
            title={playing ? t('timeline.pause') : t('timeline.play')}
          >
            {playing ? <Pause className="w-3 h-3" /> : <Play className="w-3 h-3" />}
          </button>
          <button
            disabled={totalOps === 0 || rollbackIndex === null}
            onClick={() => {
              setPlaying(false);
              setSnapshot(kernel.setRollbackIndex(nextRollbackStep(rollbackIndex, totalOps, 'next')));
            }}
            className="p-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer disabled:opacity-35"
            title={t('timeline.stepForward')}
          >
            <ChevronRight className="w-3 h-3" />
          </button>
          <button
            disabled={totalOps === 0 || rollbackIndex === null}
            onClick={() => {
              setPlaying(false);
              setSnapshot(kernel.setRollbackIndex(null));
            }}
            className="p-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer disabled:opacity-35"
            title={t('timeline.toEnd')}
          >
            <ChevronsRight className="w-3 h-3" />
          </button>
        </div>

        {/* Piece Timeline Filter Toggle */}
        {snapshot.selected_piece_id && (
          <button
            onClick={() => setTimelineFilterBySelectedPiece(!filterByPiece)}
            className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border shrink-0 cursor-pointer transition"
            style={{
              background: isFiltered ? 'var(--accent-soft)' : 'var(--bg-surface)',
              borderColor: isFiltered ? 'var(--accent)' : 'var(--border-subtle)',
              color: isFiltered ? 'var(--accent)' : 'var(--text-secondary)',
            }}
            title={t('timeline.filterTitle')}
          >
            <Filter className="w-2.5 h-2.5" />
            <span>{t('timeline.activePiece')}</span>
          </button>
        )}

        {/* Base Sloper Origin Chip */}
        <div
          onDoubleClick={() => {
            setPlaying(false);
            setSnapshot(kernel.setRollbackIndex(0));
          }}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded border font-medium shrink-0 cursor-pointer"
          style={{
            background: 'var(--bg-surface)',
            borderColor: 'var(--border-strong)',
            color: 'var(--text-primary)',
          }}
          title={t('timeline.baseTitle', { template: snapshot.template_id })}
        >
          <Layers className="w-3 h-3" style={{ color: 'var(--accent)' }} />
          <span>{t('timeline.base')}</span>
        </div>

        {totalOps === 0 ? (
          <span className="italic text-[10px] pl-1 truncate min-w-0" style={{ color: 'var(--text-muted)' }}>
            {t('timeline.empty')}
          </span>
        ) : (
          <div className="flex items-center">
            {renderSlot(0, !isFiltered)}
            {visibleOps.map((op) => {
              const Icon = iconForOp(op.kind);
              const catColor = accentForOp(op);
              const showSlotAfter = !isFiltered;
              return (
                <React.Fragment key={op.id}>
                  <div
                    draggable={!isFiltered}
                    onDragStart={(e) => {
                      if (isFiltered) return;
                      e.dataTransfer.effectAllowed = 'move';
                      e.dataTransfer.setData('text/plain', op.id);
                      setDragOpIdx(op.index);
                      setPlaying(false);
                    }}
                    onDragEnd={() => {
                      setDragOpIdx(null);
                      setDropSlot(null);
                    }}
                    onDoubleClick={() => handleOpenEdit(op)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setChipMenu({ op, x: e.clientX, y: e.clientY });
                    }}
                    className={`group relative inline-flex items-center gap-1 px-2 py-0.5 rounded border shrink-0 transition ${
                      !isFiltered ? 'cursor-grab active:cursor-grabbing' : 'cursor-pointer'
                    } ${op.after_rollback ? 'opacity-40 border-dashed' : ''} ${
                      op.suppressed ? 'opacity-55' : ''
                    } hover:border-[var(--accent)]`}
                    style={{
                      background: 'var(--bg-surface)',
                      borderColor:
                        op.severity === 'error'
                          ? '#e11d48'
                          : op.severity === 'warning'
                            ? '#d97706'
                            : 'var(--border-subtle)',
                      borderTopWidth: 2,
                      borderTopColor: catColor,
                      color: 'var(--text-secondary)',
                    }}
                    title={`#${op.index + 1}: ${op.summary}${
                      op.diagnostic ? ` — ${op.diagnostic}` : ''
                    }${op.suppressed ? ' (Suppressed)' : ''}${
                      op.after_rollback ? ' (After Rollback)' : ''
                    } · Double-click to edit`}
                  >
                    <Icon className="w-2.5 h-2.5 opacity-80 shrink-0" style={{ color: catColor }} />
                    <span
                      className={`max-w-[140px] truncate ${op.suppressed ? 'line-through' : ''}`}
                    >
                      {op.summary}
                    </span>

                    {op.suppressed && (
                      <EyeOff className="w-2.5 h-2.5 opacity-70 shrink-0" />
                    )}
                    {op.severity === 'warning' && (
                      <AlertTriangle className="w-2.5 h-2.5 text-amber-500 shrink-0" />
                    )}
                    {op.severity === 'error' && (
                      <XCircle className="w-2.5 h-2.5 text-rose-500 shrink-0" />
                    )}

                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setSnapshot(kernel.removeOp(op.id));
                      }}
                      className="ml-0.5 opacity-45 hover:opacity-100 hover:text-rose-500 cursor-pointer"
                      title={t('timeline.deleteTitle')}
                    >
                      <X className="w-2.5 h-2.5" />
                    </button>
                  </div>
                  {showSlotAfter ? renderSlot(op.index + 1, true) : <span className="opacity-30 px-1">›</span>}
                </React.Fragment>
              );
            })}
          </div>
        )}

        {/* Active Rollback Banner */}
        {rollbackIndex !== null && totalOps > 0 && (
          <button
            onClick={() => {
              setPlaying(false);
              setSnapshot(kernel.setRollbackIndex(null));
            }}
            className="ml-auto shrink-0 px-2 py-0.5 rounded-full font-semibold text-[10px] cursor-pointer"
            style={{ background: '#f59e0b22', color: '#d97706', border: '1px solid #f59e0b55' }}
          >
            Rolled back ({effectiveRollback}/{totalOps}) · Roll to End
          </button>
        )}
      </div>

      {/* Feature Context Menu */}
      {chipMenu && (
        <div
          ref={menuRef}
          className="fixed z-50 w-52 rounded-md border shadow-xl py-1 text-xs select-none"
          style={{
            left: Math.min(chipMenu.x, window.innerWidth - 220),
            top: Math.max(12, chipMenu.y - 165),
            background: 'var(--bg-surface)',
            borderColor: 'var(--border-strong)',
          }}
        >
          <div
            className="px-3 py-1 text-[10px] font-semibold truncate border-b mb-1"
            style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-muted)' }}
          >
            #{chipMenu.op.index + 1} · {chipMenu.op.summary}
          </div>
          {(chipMenu.op.editable || chipMenu.op.binding_id) && (
            <button
              onClick={() => {
                const target = chipMenu.op;
                setChipMenu(null);
                handleOpenEdit(target);
              }}
              className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer font-medium"
            >
              {t('timeline.editParams')}
            </button>
          )}
          <button
            onClick={() => {
              setSnapshot(kernel.toggleSuppressOp(chipMenu.op.id));
              setChipMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
          >
            {chipMenu.op.suppressed ? t('timeline.unsuppress') : t('timeline.suppress')}
          </button>
          <button
            onClick={() => {
              setSnapshot(kernel.setRollbackIndex(chipMenu.op.index));
              setChipMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
          >
            {t('timeline.rollBefore')}
          </button>
          <button
            onClick={() => {
              const next = chipMenu.op.index + 1 >= totalOps ? null : chipMenu.op.index + 1;
              setSnapshot(kernel.setRollbackIndex(next));
              setChipMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
          >
            {t('timeline.rollAfter')}
          </button>
          {chipMenu.op.piece_ids.length > 0 && (
            <button
              onClick={() => {
                setSnapshot(kernel.setSelection(chipMenu.op.piece_ids[0], null));
                setChipMenu(null);
              }}
              className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] cursor-pointer"
            >
              {t('timeline.locate')}
            </button>
          )}
          <div className="my-1 border-t" style={{ borderColor: 'var(--border-subtle)' }} />
          <button
            onClick={() => {
              setSnapshot(kernel.removeOp(chipMenu.op.id));
              setChipMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 hover:bg-rose-500/10 text-rose-600 cursor-pointer flex items-center gap-1.5"
          >
            <Trash2 className="w-3 h-3" />
            <span>{t('timeline.delete')}</span>
          </button>
        </div>
      )}

      {/* Double-Click In-Place Feature Parameter Editor */}
      {editingOp && (
        <OpEditModal
          op={editingOp}
          unit={unit}
          onClose={closeEditOp}
          onApply={(patch) => {
            setSnapshot(kernel.updateOp(editingOp.id, patch));
            setStatusMessage(t('timeline.updated', { index: editingOp.index + 1 }));
            closeEditOp();
          }}
        />
      )}
    </>
  );
};

interface OpEditModalProps {
  op: OpSummary;
  unit: DisplayUnit;
  onClose: () => void;
  onApply: (patch: Record<string, unknown>) => void;
}

const OpEditModal: React.FC<OpEditModalProps> = ({ op, unit, onClose, onApply }) => {
  const t = useT();
  const [form, setForm] = useState<Record<string, unknown>>(() => ({ ...op.payload }));

  useEffect(() => {
    setForm({ ...op.payload });
  }, [op]);

  const num = (k: string, fallback = 0) =>
    typeof form[k] === 'number' && Number.isFinite(form[k] as number) ? (form[k] as number) : fallback;
  const str = (k: string, fallback = '') =>
    typeof form[k] === 'string' ? (form[k] as string) : fallback;
  const bool = (k: string, fallback = false) =>
    typeof form[k] === 'boolean' ? (form[k] as boolean) : fallback;

  return (
    <div
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
        }
      }}
      className="fixed inset-0 z-50 bg-black/30 backdrop-blur-[1px] flex items-center justify-center p-4"
    >
      <div
        className="w-80 rounded-md border shadow-2xl overflow-hidden text-xs"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
      >
        <div
          className="px-3 py-2 border-b flex items-center justify-between"
          style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
        >
          <span className="font-semibold truncate">{t('timeline.editTitle', { index: op.index + 1, summary: op.summary })}</span>
          <button
            onClick={onClose}
            className="p-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>

        <div className="p-3 space-y-2.5">
          {(op.kind === 'move_vertex' || op.kind === 'move_piece' || op.kind === 'move_handle') && (
            <>
              <FieldRow label={t('timeline.deltaX', { unit })}>
                <ExpressionInput
                  valueMm={num('dx')}
                  unit={unit}
                  onCommitMm={(mm) => setForm((f) => ({ ...f, dx: mm }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <FieldRow label={t('timeline.deltaY', { unit })}>
                <ExpressionInput
                  valueMm={num('dy')}
                  unit={unit}
                  onCommitMm={(mm) => setForm((f) => ({ ...f, dy: mm }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              {op.kind === 'move_handle' && (
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={bool('lock_g1')}
                    onChange={(e) => setForm((f) => ({ ...f, lock_g1: e.target.checked }))}
                  />
                  <span>{t('timeline.lockG1')}</span>
                </label>
              )}
            </>
          )}

          {op.kind === 'set_seam_allowance' && (
            <>
              <FieldRow label={t('timeline.saWidth', { unit })}>
                <ExpressionInput
                  valueMm={num('width_mm', 10)}
                  unit={unit}
                  onCommitMm={(mm) => setForm((f) => ({ ...f, width_mm: Math.max(0, mm) }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <FieldRow label={t('timeline.cornerRule')}>
                <select
                  value={((form.corner_rule as CornerRule | undefined)?.kind) ?? 'miter'}
                  onChange={(e) => {
                    const kind = e.target.value as CornerRule['kind'];
                    const rule: CornerRule = kind === 'miter' ? { kind: 'miter', limit: 2.5 } : { kind };
                    setForm((f) => ({ ...f, corner_rule: rule }));
                  }}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="miter">{t('inspector.ruleMiter')}</option>
                  <option value="folded_hem">{t('inspector.ruleFoldedHem')}</option>
                  <option value="matched_seam">{t('timeline.ruleMatched')}</option>
                  <option value="square">{t('inspector.ruleSquare')}</option>
                  <option value="bevel">{t('inspector.ruleBevel')}</option>
                  <option value="dart_cap">{t('inspector.ruleDartCap')}</option>
                </select>
              </FieldRow>
            </>
          )}

          {op.kind === 'set_edge_length' && (
            <FieldRow label={t('timeline.targetLength', { unit })}>
              <ExpressionInput
                valueMm={num('target_len_mm', 100)}
                unit={unit}
                onCommitMm={(mm) => setForm((f) => ({ ...f, target_len_mm: Math.max(5, mm) }))}
                className="w-full rounded border px-2 py-1 text-[11px]"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              />
            </FieldRow>
          )}

          {op.kind === 'pivot_dart' && (
            <>
              <FieldRow label={t('timeline.pivotAngle')}>
                <ExpressionInput
                  valueMm={num('angle_deg', 14)}
                  unit="deg"
                  onCommitMm={(deg) => setForm((f) => ({ ...f, angle_deg: Math.max(1, Math.min(60, deg)) }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <FieldRow label={t('timeline.splitT')}>
                <input
                  type="number"
                  step={0.05}
                  min={0.05}
                  max={0.95}
                  value={num('target_t', 0.5)}
                  onChange={(e) => setForm((f) => ({ ...f, target_t: Number(e.target.value) }))}
                  className="w-full rounded border px-2 py-1 font-mono-num text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
            </>
          )}

          {op.kind === 'slash_spread' && (
            <>
              <FieldRow label={t('timeline.spreadWidth', { unit })}>
                <ExpressionInput
                  valueMm={num('spread_mm', 18)}
                  unit={unit}
                  onCommitMm={(mm) => setForm((f) => ({ ...f, spread_mm: mm }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <FieldRow label={t('timeline.slashCount')}>
                <input
                  type="number"
                  min={1}
                  max={6}
                  value={num('slash_count', 3)}
                  onChange={(e) => setForm((f) => ({ ...f, slash_count: Number(e.target.value) }))}
                  className="w-full rounded border px-2 py-1 font-mono-num text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <FieldRow label={t('timeline.hingeMode')}>
                <select
                  value={str('hinge_mode', 'edge_hinge')}
                  onChange={(e) => setForm((f) => ({ ...f, hinge_mode: e.target.value }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="edge_hinge">{t('timeline.edgeHinge')}</option>
                  <option value="parallel_shift">{t('timeline.parallelShift')}</option>
                </select>
              </FieldRow>
            </>
          )}

          {op.kind === 'add_notch' && (
            <>
              <FieldRow label={t('timeline.positionT')}>
                <input
                  type="number"
                  step={0.02}
                  min={0.02}
                  max={0.98}
                  value={num('param_t', 0.5)}
                  onChange={(e) => setForm((f) => ({ ...f, param_t: Number(e.target.value) }))}
                  className="w-full rounded border px-2 py-1 font-mono-num text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <FieldRow label={t('timeline.notchStyle')}>
                <div className="flex gap-1.5">
                  <select
                    value={str('style', 'v')}
                    onChange={(e) => setForm((f) => ({ ...f, style: e.target.value }))}
                    className="flex-1 rounded border px-2 py-1 text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  >
                    <option value="slit">{t('timeline.notchSlit')}</option>
                    <option value="v">{t('timeline.notchV')}</option>
                    <option value="t">{t('timeline.notchT')}</option>
                    <option value="u">{t('timeline.notchU')}</option>
                    <option value="castle">{t('timeline.notchCastle')}</option>
                  </select>
                  <select
                    value={num('count', 1)}
                    onChange={(e) => setForm((f) => ({ ...f, count: Number(e.target.value) }))}
                    className="w-16 rounded border px-2 py-1 font-mono-num text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  >
                    <option value={1}>1x</option>
                    <option value={2}>2x</option>
                    <option value={3}>3x</option>
                  </select>
                </div>
              </FieldRow>
            </>
          )}

          {op.kind === 'true_corner_g1' && (
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={bool('square_90', true)}
                onChange={(e) => setForm((f) => ({ ...f, square_90: e.target.checked }))}
              />
              <span>{t('timeline.square90')}</span>
            </label>
          )}

          {op.kind === 'convert_edge_curve' && (
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={bool('to_cubic', true)}
                onChange={(e) => setForm((f) => ({ ...f, to_cubic: e.target.checked }))}
              />
              <span>{t('timeline.smoothCubic')}</span>
            </label>
          )}

          {op.kind === 'split_edge_point' && (
            <FieldRow label={t('timeline.splitFraction')}>
              <input
                type="number"
                step={0.05}
                min={0.08}
                max={0.92}
                value={num('param_t', 0.5)}
                onChange={(e) => setForm((f) => ({ ...f, param_t: Number(e.target.value) }))}
                className="w-full rounded border px-2 py-1 font-mono-num text-[11px]"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              />
            </FieldRow>
          )}

          {op.kind === 'split_style_line' && (
            <>
              <FieldRow label={t('timeline.panelA')}>
                <input
                  type="text"
                  value={str('label_a')}
                  onChange={(e) => setForm((f) => ({ ...f, label_a: e.target.value }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <FieldRow label={t('timeline.panelB')}>
                <input
                  type="text"
                  value={str('label_b')}
                  onChange={(e) => setForm((f) => ({ ...f, label_b: e.target.value }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <div className="grid grid-cols-2 gap-2">
                <FieldRow label={t('timeline.cutStart')}>
                  <input
                    type="number"
                    step={0.05}
                    min={0.1}
                    max={0.9}
                    value={num('t_a', 0.5)}
                    onChange={(e) => setForm((f) => ({ ...f, t_a: Number(e.target.value) }))}
                    className="w-full rounded border px-2 py-1 font-mono-num text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  />
                </FieldRow>
                <FieldRow label={t('timeline.cutEnd')}>
                  <input
                    type="number"
                    step={0.05}
                    min={0.1}
                    max={0.9}
                    value={num('t_b', 0.5)}
                    onChange={(e) => setForm((f) => ({ ...f, t_b: Number(e.target.value) }))}
                    className="w-full rounded border px-2 py-1 font-mono-num text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  />
                </FieldRow>
              </div>
            </>
          )}

          {op.kind === 'add_drill_hole' && (
            <>
              <FieldRow label={t('timeline.drillLabel')}>
                <input
                  type="text"
                  value={str('label', t('timeline.drillDefault'))}
                  onChange={(e) => setForm((f) => ({ ...f, label: e.target.value }))}
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                />
              </FieldRow>
              <div className="grid grid-cols-2 gap-2">
                <FieldRow label={`X (${unit})`}>
                  <ExpressionInput
                    valueMm={num('x')}
                    unit={unit}
                    onCommitMm={(mm) => setForm((f) => ({ ...f, x: mm }))}
                    className="w-full rounded border px-2 py-1 text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  />
                </FieldRow>
                <FieldRow label={`Y (${unit})`}>
                  <ExpressionInput
                    valueMm={num('y')}
                    unit={unit}
                    onCommitMm={(mm) => setForm((f) => ({ ...f, y: mm }))}
                    className="w-full rounded border px-2 py-1 text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  />
                </FieldRow>
              </div>
            </>
          )}

          {op.kind === 'set_grain_rotation' && (
            <FieldRow label={t('timeline.grainMode')}>
              <select
                value={str('mode', 'two_way')}
                onChange={(e) => setForm((f) => ({ ...f, mode: e.target.value }))}
                className="w-full rounded border px-2 py-1 text-[11px]"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              >
                <option value="one_way">{t('timeline.grainOneWay')}</option>
                <option value="two_way">{t('timeline.grainTwoWay')}</option>
                <option value="bias">{t('inspector.grainBias')}</option>
                <option value="free">{t('inspector.grainFree')}</option>
              </select>
            </FieldRow>
          )}
        </div>

        <div
          className="px-3 py-2 border-t flex items-center justify-end gap-1.5"
          style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
        >
          <button
            onClick={onClose}
            className="px-2.5 py-1 rounded border text-[11px] hover:bg-[var(--bg-subtle)] cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
          >
            Cancel
          </button>
          <button
            onClick={() => onApply(form)}
            className="inline-flex items-center gap-1 px-3 py-1 rounded text-[11px] font-semibold text-white cursor-pointer"
            style={{ background: 'var(--accent)' }}
          >
            <Check className="w-3 h-3" />
            <span>{t('common.apply')}</span>
          </button>
        </div>
      </div>
    </div>
  );
};

const FieldRow: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
  <label className="block space-y-1">
    <span className="text-[10px] font-semibold uppercase tracking-wider block" style={{ color: 'var(--text-muted)' }}>
      {label}
    </span>
    {children}
  </label>
);
