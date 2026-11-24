import React from 'react';
import { formatMm, type DisplayUnit } from '../../lib/types';
import { useT } from '../../lib/i18n';
import { useStudioStore } from '../../store/studioStore';
import { CommandLine } from './CommandLine';
import { ParametricTimeline } from './ParametricTimeline';

const Sep = () => <div className="w-px h-3.5 shrink-0" style={{ background: 'var(--border-subtle)' }} />;

/**
 * Bottom of the window: the parametric history (timeline) across the full width, and under it
 * the command line with live status on the right (messages, snap, cursor, zoom, units, 3D).
 */
export const BottomBar: React.FC = () => {
  const t = useT();
  const unit = useStudioStore((s) => s.unit);
  const setUnit = useStudioStore((s) => s.setUnit);
  const cursor = useStudioStore((s) => s.cursorWorld);
  const snap = useStudioStore((s) => s.activeSnap);
  const zoomPercent = useStudioStore((s) => s.zoomPercent);
  const solveMs = useStudioStore((s) => s.snapshot?.solve_ms ?? 0);
  const measureReadout = useStudioStore((s) => s.measureReadout);
  const statusMessage = useStudioStore((s) => s.statusMessage);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const stats = useStudioStore((s) => s.sim3dStats);
  const quality = useStudioStore((s) => s.sim3dQuality);

  const show2d = workspaceMode !== '3d';
  const show3d = workspaceMode !== '2d';

  return (
    <footer className="shrink-0 border-t select-none" style={{ background: 'var(--toolbar)', borderColor: 'var(--border-strong)' }}>
      <div className="h-[30px] flex items-center border-b" style={{ borderColor: 'var(--border-subtle)' }}>
        <ParametricTimeline />
      </div>
      <div className="h-[26px] flex items-center" style={{ background: 'var(--bg-surface)' }}>
      <CommandLine />
      <div
        className="h-full shrink-0 pl-3 pr-2 flex items-center gap-2.5 border-l text-[11px] font-mono-num"
        style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}
      >
        {statusMessage && (
          <>
            <span className="max-w-[300px] truncate font-sans" title={statusMessage}>
              {statusMessage}
            </span>
            <Sep />
          </>
        )}
        {snap && (
          <span className="px-1.5 rounded-[2px] bg-emerald-500/15 text-emerald-700 dark:text-emerald-400">
            {t('status.snap', { kind: snap.kind.toUpperCase() })}
          </span>
        )}
        {measureReadout && (
          <span className="px-1.5 rounded-[2px]" style={{ background: 'var(--accent-soft)', color: 'var(--accent)' }}>
            {t('status.measure', {
              dist: formatMm(measureReadout.dist, unit, 1),
              dx: formatMm(measureReadout.dx, unit, 1),
              dy: formatMm(measureReadout.dy, unit, 1),
              angle: measureReadout.angleDeg.toFixed(1),
            })}
          </span>
        )}
        {show3d && stats && (
          <>
            <span
              className="inline-flex items-center gap-1.5"
              title={t('v3d.telemetry', {
                tris: stats.triangle_count,
                stitches: stats.sew_pair_count,
                darts: stats.darts_closed ?? 0,
                height: (stats.measured_height_mm / 10).toFixed(0),
                bust: stats.measured_bust_mm.toFixed(0),
                waist: stats.measured_waist_mm.toFixed(0),
                hip: stats.measured_hip_mm.toFixed(0),
              })}
            >
              <span
                className="w-1.5 h-1.5 rounded-full"
                style={{ background: (stats.motion_mm_s ?? 0) < 15 ? '#10b981' : '#f59e0b' }}
              />
              <span>
                3D {(stats.motion_mm_s ?? 0) < 15 ? t('v3d.settled') : t('v3d.settling')}
              </span>
              <span className={stats.max_strain_pct > 15 ? 'text-amber-600' : undefined}>
                {t('status.strain', { pct: stats.max_strain_pct.toFixed(1) })}
              </span>
              <span>{t('v3d.easeShort', { ease: stats.avg_clearance_mm.toFixed(0) })}</span>
              {quality && (
                <span className={quality.poke_through_count > 0 ? 'text-amber-600' : 'text-emerald-700 dark:text-emerald-400'}>
                  {quality.poke_through_count > 0
                    ? t('v3d.poke', { count: quality.poke_through_count })
                    : t('v3d.cleanFit')}
                </span>
              )}
            </span>
            <Sep />
          </>
        )}
        {show2d && (
          <>
            <span className="w-[150px] text-right">
              X {formatMm(cursor.x, unit, 1)} Y {formatMm(cursor.y, unit, 1)}
            </span>
            <span className="w-[42px] text-right" title={t('status.zoomTitle')}>
              {zoomPercent.toFixed(0)}%
            </span>
            <Sep />
          </>
        )}
        <select
          value={unit}
          onChange={(e) => setUnit(e.target.value as DisplayUnit)}
          className="h-[20px] rounded-[3px] border px-0.5 text-[11px] cursor-pointer outline-none"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)', color: 'var(--text-primary)' }}
          title={t('settings.units')}
          aria-label={t('settings.units')}
        >
          <option value="mm">mm</option>
          <option value="cm">cm</option>
          <option value="in">in</option>
        </select>
        <span title={t('status.kernelTitle')}>{t('status.kernel', { ms: solveMs.toFixed(1) })}</span>
      </div>
      </div>
    </footer>
  );
};
