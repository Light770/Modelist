import React from 'react';
import { formatMm } from '../../lib/types';
import { useT } from '../../lib/i18n';
import { useStudioStore } from '../../store/studioStore';

export const StatusBar: React.FC = () => {
  const unit = useStudioStore((s) => s.unit);
  const cursor = useStudioStore((s) => s.cursorWorld);
  const snap = useStudioStore((s) => s.activeSnap);
  const zoomPercent = useStudioStore((s) => s.zoomPercent);
  const snapshot = useStudioStore((s) => s.snapshot);
  const measureReadout = useStudioStore((s) => s.measureReadout);
  const statusMessage = useStudioStore((s) => s.statusMessage);
  const t = useT();

  const selectedPiece = snapshot?.pieces.find((p) => p.id === snapshot.selected_piece_id);

  return (
    <footer
      className="h-7 shrink-0 border-t px-3 flex items-center justify-between text-[11px] font-mono-num select-none"
      style={{
        background: 'var(--bg-surface)',
        borderColor: 'var(--border-subtle)',
        color: 'var(--text-secondary)',
      }}
    >
      <div className="flex items-center gap-4 min-w-0">
        <span>
          X {formatMm(cursor.x, unit, 1)} · Y {formatMm(cursor.y, unit, 1)}
        </span>
        {snap && (
          <span className="px-1.5 py-0.2 rounded bg-emerald-500/15 text-emerald-600 dark:text-emerald-400">
            {t('status.snap', { kind: snap.kind.toUpperCase() })}
          </span>
        )}
        {measureReadout && (
          <span className="px-2 py-0.2 rounded" style={{ background: 'var(--accent-soft)', color: 'var(--accent)' }}>
            {t('status.measure', {
              dist: formatMm(measureReadout.dist, unit, 1),
              dx: formatMm(measureReadout.dx, unit, 1),
              dy: formatMm(measureReadout.dy, unit, 1),
              angle: measureReadout.angleDeg.toFixed(1),
            })}
          </span>
        )}
        {statusMessage && <span className="truncate opacity-85">{statusMessage}</span>}
      </div>

      <div className="flex items-center gap-4 shrink-0">
        {selectedPiece && (
          <span className="hidden md:inline">
            {t('status.piece', {
              code: selectedPiece.code,
              perim: formatMm(selectedPiece.perimeter_mm, unit, 1),
              area: (selectedPiece.area_mm2 / 100).toFixed(1),
            })}
          </span>
        )}
        <span>{t('status.zoom', { zoom: zoomPercent.toFixed(0) })}</span>
        {snapshot && <span>{t('status.kernel', { ms: snapshot.solve_ms.toFixed(2) })}</span>}
        <span className="text-emerald-600 dark:text-emerald-400">{t('status.localFirst')}</span>
      </div>
    </footer>
  );
};
