import React, { useEffect } from 'react';
import {
  ArrowLeftRight,
  ArrowRight,
  Check,
  Trash2,
  X,
} from 'lucide-react';
import {
  formatMm,
  type EaseSpec,
  type EdgeRefInfo,
  type SeamDriver,
  type SeamType,
} from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';
import { useT, type MessageKey } from '../../lib/i18n';
import { ExpressionInput } from './ExpressionInput';

const SEAM_TYPE_OPTIONS: Array<{ id: SeamType; label: MessageKey; saMm: number }> = [
  { id: 'plain', label: 'seam.plain', saMm: 10 },
  { id: 'overlock', label: 'seam.overlock', saMm: 7 },
  { id: 'french', label: 'seam.french', saMm: 15 },
  { id: 'flat_felled', label: 'seam.flatFelled', saMm: 16 },
  { id: 'gathered', label: 'seam.gathered', saMm: 12 },
];

export const SeamCommandDialog: React.FC = () => {
  const t = useT();
  const cmd = useStudioStore((s) => s.seamCommand);
  const unit = useStudioStore((s) => s.unit);
  const setSeamActiveSlot = useStudioStore((s) => s.setSeamActiveSlot);
  const flipSeamCommandEdgeDir = useStudioStore((s) => s.flipSeamCommandEdgeDir);
  const removeSeamCommandEdge = useStudioStore((s) => s.removeSeamCommandEdge);
  const patchSeamCommand = useStudioStore((s) => s.patchSeamCommand);
  const commitSeamCommand = useStudioStore((s) => s.commitSeamCommand);
  const closeSeamCommand = useStudioStore((s) => s.closeSeamCommand);
  const deleteActiveSeamCommand = useStudioStore((s) => s.deleteActiveSeamCommand);

  // Fusion 360 Dialog Keyboard Ergonomics: Tab switches Collector A <-> B, Enter commits, Esc cancels
  useEffect(() => {
    if (!cmd) return;
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const inInput = target && (target.tagName === 'INPUT' || target.tagName === 'SELECT');
      if (e.key === 'Escape') {
        e.preventDefault();
        closeSeamCommand();
      } else if (e.key === 'Tab' && !inInput) {
        e.preventDefault();
        setSeamActiveSlot(cmd.activeSlot === 'a' ? 'b' : 'a');
      } else if (e.key === 'Enter' && !inInput && cmd.edgesA.length > 0 && cmd.edgesB.length > 0) {
        e.preventDefault();
        commitSeamCommand();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [cmd, closeSeamCommand, commitSeamCommand, setSeamActiveSlot]);

  if (!cmd) return null;

  const lenA = cmd.edgesA.reduce((acc, e) => acc + e.length_mm, 0);
  const lenB = cmd.edgesB.reduce((acc, e) => acc + e.length_mm, 0);
  const deltaMm = lenB - lenA;
  const readyToConfig = cmd.edgesA.length > 0 && cmd.edgesB.length > 0;
  const ratioPct = lenA > 1 ? (deltaMm / lenA) * 100 : 0;

  const renderSlot = (
    slot: 'a' | 'b',
    title: string,
    accentHex: string,
    edges: EdgeRefInfo[],
    totalLen: number,
  ) => {
    const active = cmd.activeSlot === slot;
    return (
      <div
        onClick={() => setSeamActiveSlot(slot)}
        className="rounded border p-2 transition cursor-pointer space-y-1.5"
        style={{
          background: active ? `${accentHex}12` : 'var(--bg-panel)',
          borderColor: active ? accentHex : 'var(--border-subtle)',
          boxShadow: active ? `0 0 0 1px ${accentHex}40` : 'none',
        }}
      >
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-1.5">
            <span className="w-2.5 h-2.5 rounded-full shrink-0" style={{ background: accentHex }} />
            <span className="font-semibold text-[11px]">{title}</span>
          </div>
          <span className="font-mono-num text-[10px]" style={{ color: 'var(--text-secondary)' }}>
            {edges.length > 0 ? `${edges.length}e · ${formatMm(totalLen, unit, 1)}` : t('seam.clickEdges')}
          </span>
        </div>

        {edges.length === 0 ? (
          <div className="text-[10px] py-1 italic" style={{ color: active ? accentHex : 'var(--text-muted)' }}>
            {active
              ? t('seam.collecting')
              : t('seam.clickHere')}
          </div>
        ) : (
          <div className="space-y-1">
            {edges.map((ed) => (
              <div
                key={ed.edge_id}
                className="flex items-center justify-between gap-1.5 px-2 py-1 rounded border text-[10px]"
                style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
              >
                <span className="truncate font-medium">
                  {ed.piece_code} #{ed.edge_idx + 1} {ed.edge_label}
                </span>
                <div className="flex items-center gap-1 shrink-0 font-mono-num">
                  <span style={{ color: 'var(--text-muted)' }}>{formatMm(ed.length_mm, unit, 0)}</span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      flipSeamCommandEdgeDir(slot, ed.edge_id);
                    }}
                    className="px-1 py-0.5 rounded border hover:bg-[var(--bg-subtle)] cursor-pointer inline-flex items-center gap-0.5"
                    style={{ borderColor: 'var(--border-subtle)' }}
                    title={t('seam.flip')}
                  >
                    <ArrowLeftRight className="w-2.5 h-2.5" />
                    <span>{ed.dir > 0 ? '+' : '−'}</span>
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      removeSeamCommandEdge(slot, ed.edge_id);
                    }}
                    className="p-0.5 rounded hover:text-rose-500 cursor-pointer"
                    title={t('seam.remove')}
                  >
                    <X className="w-2.5 h-2.5" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Progressive nudge from Slot A -> Slot B */}
        {slot === 'a' && active && cmd.edgesA.length > 0 && cmd.edgesB.length === 0 && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              setSeamActiveSlot('b');
            }}
            className="w-full mt-1 py-1 rounded text-[10px] font-semibold inline-flex items-center justify-center gap-1 cursor-pointer"
            style={{ background: '#0d948818', color: '#0d9488' }}
          >
            <span>{t('seam.nextB')}</span>
            <ArrowRight className="w-3 h-3" />
          </button>
        )}
      </div>
    );
  };

  return (
    // Rendered inside the properties panel, which supplies the title bar and close button.
    <div className="flex-1 min-h-0 flex flex-col overflow-hidden select-none" role="group" aria-label={cmd.mode === 'edit' ? t('seam.editTitle') : t('seam.createTitle')}>
      {/* Scrollable Progressive Body */}
      <div className="p-3 flex-1 min-h-0 overflow-y-auto space-y-2.5 text-xs">
        {/* Step 1: Fusion 360 Geometry Collectors */}
        <div className="space-y-2">
          {renderSlot('a', t('seam.slotA'), '#2563eb', cmd.edgesA, lenA)}
          {renderSlot('b', t('seam.slotB'), '#0d9488', cmd.edgesB, lenB)}
        </div>

        {/* Step 2: Progressive Configuration — unfolds cleanly once both sides are selected! */}
        {readyToConfig && (
          <div className="pt-2.5 border-t space-y-2.5" style={{ borderColor: 'var(--border-subtle)' }}>
            {/* Live Length & Ease Balance Meter */}
            <div
              className="p-2 rounded border space-y-1.5"
              style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
            >
              <div className="flex items-center justify-between text-[10px] font-mono-num">
                <span>
                  A: <strong>{formatMm(lenA, unit, 1)}</strong>
                </span>
                <span
                  className={`px-1.5 py-0.5 rounded font-semibold ${
                    Math.abs(deltaMm) <= 3
                      ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400'
                      : 'bg-amber-500/15 text-amber-700 dark:text-amber-300'
                  }`}
                >
                  Δ {deltaMm >= 0 ? '+' : ''}
                  {formatMm(deltaMm, unit, 1)} ({ratioPct >= 0 ? '+' : ''}
                  {ratioPct.toFixed(1)}%)
                </span>
                <span>
                  B: <strong>{formatMm(lenB, unit, 1)}</strong>
                </span>
              </div>
              <div className="h-1.5 rounded-full overflow-hidden flex" style={{ background: 'var(--border-subtle)' }}>
                <div
                  style={{
                    width: `${(lenA / Math.max(lenA + lenB, 1)) * 100}%`,
                    background: '#2563eb',
                  }}
                />
                <div
                  style={{
                    width: `${(lenB / Math.max(lenA + lenB, 1)) * 100}%`,
                    background: '#0d9488',
                  }}
                />
              </div>
            </div>

            {/* Seam Name */}
            <label className="block space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>
                {t('seam.label')}
              </span>
              <input
                type="text"
                value={cmd.label}
                onChange={(e) => patchSeamCommand({ label: e.target.value })}
                placeholder={t('seam.labelPlaceholder')}
                className="w-full rounded border px-2 py-1 text-[11px]"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              />
            </label>

            {/* Driver Direction (Interdependency Rule) */}
            <div className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider block" style={{ color: 'var(--text-muted)' }}>
                {t('seam.dependency')}
              </span>
              <div className="grid grid-cols-4 gap-1 text-[10px]">
                {(
                  [
                    ['driver_a', 'A → B'],
                    ['driver_b', 'B → A'],
                    ['mutual', t('seam.mutual')],
                    ['monitor_only', t('seam.monitor')],
                  ] as Array<[SeamDriver, string]>
                ).map(([drv, lbl]) => (
                  <button
                    key={drv}
                    onClick={() => patchSeamCommand({ driver: drv })}
                    className="py-1 rounded border font-medium cursor-pointer text-center"
                    style={{
                      borderColor: cmd.driver === drv ? 'var(--accent)' : 'var(--border-subtle)',
                      background: cmd.driver === drv ? 'var(--accent-soft)' : 'var(--bg-panel)',
                      color: cmd.driver === drv ? 'var(--accent)' : 'var(--text-secondary)',
                    }}
                  >
                    {lbl}
                  </button>
                ))}
              </div>
            </div>

            {/* Ease & Fullness Configuration (Progressive Numeric Field) */}
            <div className="space-y-1.5">
              <span className="text-[10px] font-semibold uppercase tracking-wider block" style={{ color: 'var(--text-muted)' }}>
                {t('seam.easeRule')}
              </span>
              <div className="flex items-center gap-1.5">
                <select
                  value={cmd.ease.kind}
                  onChange={(e) => {
                    const k = e.target.value;
                    const next: EaseSpec =
                      k === 'exact'
                        ? { kind: 'exact' }
                        : k === 'ease_mm'
                          ? { kind: 'ease_mm', mm: Math.round(deltaMm) || 15 }
                          : k === 'ease_pct'
                            ? { kind: 'ease_pct', pct: 5 }
                            : k === 'gather'
                              ? { kind: 'gather', ratio: 1.5 }
                              : { kind: 'pleats', count: 3, depth_mm: 15 };
                    patchSeamCommand({ ease: next });
                  }}
                  className="flex-1 rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="exact">{t('seam.exact')}</option>
                  <option value="ease_mm">{t('seam.easeMm')}</option>
                  <option value="ease_pct">{t('seam.easePct')}</option>
                  <option value="gather">{t('seam.gather')}</option>
                  <option value="pleats">{t('seam.pleats')}</option>
                </select>

                {cmd.ease.kind === 'ease_mm' && (
                  <ExpressionInput
                    valueMm={cmd.ease.mm}
                    unit={unit}
                    onCommitMm={(mm) =>
                      patchSeamCommand({ ease: { kind: 'ease_mm', mm: Number(mm.toFixed(2)) } })
                    }
                    className="w-24 rounded border px-2 py-1 text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                    title={t('seam.easeTitle')}
                  />
                )}
                {cmd.ease.kind === 'ease_pct' && (
                  <input
                    type="number"
                    step={0.5}
                    value={cmd.ease.pct}
                    onChange={(e) =>
                      patchSeamCommand({ ease: { kind: 'ease_pct', pct: Number(e.target.value) } })
                    }
                    className="w-20 rounded border px-2 py-1 font-mono-num text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                    title={t('seam.easePctTitle')}
                  />
                )}
                {cmd.ease.kind === 'gather' && (
                  <input
                    type="number"
                    step={0.05}
                    min={1}
                    max={4}
                    value={cmd.ease.ratio}
                    onChange={(e) =>
                      patchSeamCommand({ ease: { kind: 'gather', ratio: Number(e.target.value) } })
                    }
                    className="w-20 rounded border px-2 py-1 font-mono-num text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                    title={t('seam.gatherTitle')}
                  />
                )}
                {cmd.ease.kind === 'pleats' && (
                  <input
                    type="number"
                    min={1}
                    max={12}
                    value={cmd.ease.count}
                    onChange={(e) =>
                      patchSeamCommand({
                        ease: {
                          kind: 'pleats',
                          count: Number(e.target.value),
                          depth_mm: cmd.ease.kind === 'pleats' ? cmd.ease.depth_mm : 15,
                        },
                      })
                    }
                    className="w-16 rounded border px-2 py-1 font-mono-num text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                    title={t('seam.pleatCount')}
                  />
                )}
              </div>
            </div>

            {/* Seam Construction & Options */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between gap-2">
                <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>
                  {t('seam.construction')}
                </span>
                <select
                  value={cmd.seamType}
                  onChange={(e) => patchSeamCommand({ seamType: e.target.value as SeamType })}
                  className="rounded border px-2 py-0.5 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                >
                  {SEAM_TYPE_OPTIONS.map((o) => (
                    <option key={o.id} value={o.id}>
                      {t(o.label)} ({o.saMm} mm)
                    </option>
                  ))}
                </select>
              </div>

              <label className="flex items-center gap-2 text-[11px] cursor-pointer">
                <input
                  type="checkbox"
                  checked={cmd.autoPartnerNotches}
                  onChange={(e) => patchSeamCommand({ autoPartnerNotches: e.target.checked })}
                />
                <span>{t('seam.autoNotches')}</span>
              </label>

              <label className="flex items-center gap-2 text-[11px] cursor-pointer">
                <input
                  type="checkbox"
                  checked={cmd.syncRecommendedSa}
                  onChange={(e) => patchSeamCommand({ syncRecommendedSa: e.target.checked })}
                />
                <span>{t('seam.syncSa')}</span>
              </label>
            </div>
          </div>
        )}
      </div>

      {/* Fusion 360 Command Dialog Action Footer */}
      <div
        className="px-3 py-2 border-t flex items-center justify-between gap-2 shrink-0"
        style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
      >
        {cmd.mode === 'edit' ? (
          <button
            onClick={deleteActiveSeamCommand}
            className="inline-flex items-center gap-1 px-2 py-1 rounded border text-[11px] text-rose-600 hover:bg-rose-500/10 cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
            title={t('seam.deleteTitle')}
          >
            <Trash2 className="w-3 h-3" />
            <span>{t('common.delete')}</span>
          </button>
        ) : (
          <span className="text-[10px] font-mono-num" style={{ color: 'var(--text-muted)' }}>
            Tab: switch slot
          </span>
        )}

        <div className="flex items-center gap-1.5">
          <button
            onClick={closeSeamCommand}
            className="px-2.5 py-1 rounded border text-[11px] hover:bg-[var(--bg-subtle)] cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
          >
            Cancel
          </button>
          <button
            onClick={commitSeamCommand}
            disabled={!readyToConfig}
            className="inline-flex items-center gap-1 px-3 py-1 rounded text-[11px] font-semibold text-white cursor-pointer disabled:opacity-40"
            style={{ background: 'var(--accent)' }}
          >
            <Check className="w-3.5 h-3.5" />
            <span>{cmd.mode === 'edit' ? t('common.apply') : t('seam.ok')}</span>
          </button>
        </div>
      </div>
    </div>
  );
};
