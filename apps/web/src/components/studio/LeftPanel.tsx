import React from 'react';
import { Eye, EyeOff, Footprints, Layers, Link2, Trash2 } from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { formatMm } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

export const LeftPanel: React.FC = () => {
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const startWalkSeam = useStudioStore((s) => s.startWalkSeam);

  if (!snapshot) return null;

  return (
    <aside
      className="w-64 shrink-0 border-r flex flex-col overflow-y-auto select-none"
      style={{
        background: 'var(--bg-panel)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {/* Pattern Pieces */}
      <section className="p-3 border-b" style={{ borderColor: 'var(--border-subtle)' }}>
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-[11px] font-semibold uppercase tracking-wider flex items-center gap-1.5" style={{ color: 'var(--text-secondary)' }}>
            <Layers className="w-3.5 h-3.5" />
            Pattern Pieces ({snapshot.pieces.length})
          </h2>
        </div>
        <div className="space-y-1.5">
          {snapshot.pieces.map((p) => {
            const selected = snapshot.selected_piece_id === p.id;
            return (
              <div
                key={p.id}
                onClick={() => setSnapshot(kernel.setSelection(p.id, null))}
                className="p-2 rounded-lg border cursor-pointer transition"
                style={{
                  background: selected ? 'var(--bg-surface)' : 'transparent',
                  borderColor: selected ? 'var(--accent)' : 'var(--border-subtle)',
                }}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="min-w-0">
                    <div className="text-xs font-semibold truncate">{p.name}</div>
                    <div className="text-[10px] font-mono-num" style={{ color: 'var(--text-muted)' }}>
                      {p.code} · Cut {p.cut_quantity} · {p.fabric_role}
                    </div>
                  </div>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setSnapshot(kernel.togglePieceVisibility(p.id));
                    }}
                    className="p-1 rounded hover:bg-[var(--bg-subtle)]"
                    title={p.visible ? 'Hide Piece' : 'Show Piece'}
                  >
                    {p.visible ? (
                      <Eye className="w-3.5 h-3.5" />
                    ) : (
                      <EyeOff className="w-3.5 h-3.5 opacity-45" />
                    )}
                  </button>
                </div>
                <div className="mt-1.5 flex items-center justify-between text-[10px] font-mono-num" style={{ color: 'var(--text-secondary)' }}>
                  <span>Perim {formatMm(p.perimeter_mm, unit, 0)}</span>
                  <span>{p.notch_count} notches</span>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {/* Stitching Graph */}
      <section className="p-3 border-b" style={{ borderColor: 'var(--border-subtle)' }}>
        <h2 className="text-[11px] font-semibold uppercase tracking-wider flex items-center gap-1.5 mb-2" style={{ color: 'var(--text-secondary)' }}>
          <Link2 className="w-3.5 h-3.5" />
          Stitching Graph ({snapshot.stitching_links.length})
        </h2>
        <div className="space-y-1.5">
          {snapshot.stitching_links.map((link) => {
            const badgeColor =
              link.status === 'ok'
                ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400'
                : link.status === 'tight'
                  ? 'bg-rose-500/15 text-rose-700 dark:text-rose-400'
                  : 'bg-amber-500/15 text-amber-700 dark:text-amber-400';
            return (
              <div
                key={link.id}
                className="p-2 rounded-lg border text-xs"
                style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
              >
                <div className="font-medium text-[11px] leading-snug">{link.label}</div>
                <div className="mt-1.5 flex items-center justify-between">
                  <span className={`px-1.5 py-0.5 rounded text-[10px] font-mono-num ${badgeColor}`}>
                    {link.ease_mm >= 0 ? '+' : ''}
                    {formatMm(link.ease_mm, unit, 1)} ease
                  </span>
                  <button
                    onClick={() => startWalkSeam(link.id)}
                    className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium hover:opacity-90 cursor-pointer"
                    style={{ background: 'var(--accent-soft)', color: 'var(--accent)' }}
                    title="Walk & inspect this stitched seam"
                  >
                    <Footprints className="w-3 h-3" />
                    Walk
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {/* Parametric Operation History */}
      <section className="p-3 flex-1">
        <h2 className="text-[11px] font-semibold uppercase tracking-wider mb-2" style={{ color: 'var(--text-secondary)' }}>
          Silent Math Ops ({snapshot.ops.length})
        </h2>
        {snapshot.ops.length === 0 ? (
          <p className="text-[11px] leading-relaxed" style={{ color: 'var(--text-muted)' }}>
            Direct drags, dart pivots, slash &amp; spread, notches, and SA edits record here as non-breaking parametric ops.
          </p>
        ) : (
          <div className="space-y-1">
            {snapshot.ops.map((op) => (
              <div
                key={op.id}
                className="px-2 py-1.5 rounded border text-[11px] flex items-center justify-between gap-2"
                style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
              >
                <span className="truncate">{op.summary}</span>
                <button
                  onClick={() => setSnapshot(kernel.removeOp(op.id))}
                  className="p-0.5 rounded hover:text-rose-500 cursor-pointer shrink-0"
                  title="Delete parametric op"
                >
                  <Trash2 className="w-3 h-3" />
                </button>
              </div>
            ))}
          </div>
        )}
      </section>
    </aside>
  );
};
