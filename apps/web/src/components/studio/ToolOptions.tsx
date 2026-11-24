import React from 'react';
import { Check } from 'lucide-react';
import { formatMm, type ToolId } from '../../lib/types';
import { useT, type MessageKey } from '../../lib/i18n';
import { useStudioStore } from '../../store/studioStore';

/** One-line instruction per tool, shown at the top of its options and as the command-line prompt. */
export const TOOL_PROMPTS: Partial<Record<ToolId, MessageKey>> = {
  draft_piece: 'v2d.draftHint',
  direct_edit: 'prompt.directEdit',
  dart_pivot: 'v2d.pivotHint',
  slash_spread: 'v2d.slashHint',
  notch: 'v2d.notchHint',
  true_corner: 'v2d.trueCornerHint',
  style_split: 'v2d.styleTwoEdges',
  drill_hole: 'v2d.drillProd',
  walk_seam: 'prompt.walkSeam',
  measure: 'prompt.measure',
};

/** Tools whose options live in the properties panel. */
export const TOOLS_WITH_OPTIONS = new Set<ToolId>(Object.keys(TOOL_PROMPTS) as ToolId[]);

const Field: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
  <label className="flex items-center justify-between gap-3 text-[11px]">
    <span style={{ color: 'var(--text-secondary)' }}>{label}</span>
    {children}
  </label>
);

const inputStyle: React.CSSProperties = {
  borderColor: 'var(--border-subtle)',
  background: 'var(--bg-panel)',
};

const Segmented: React.FC<{ options: Array<[boolean, string, () => void]> }> = ({ options }) => (
  <div className="flex rounded border p-px" style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}>
    {options.map(([active, label, onClick]) => (
      <button
        key={label}
        onClick={onClick}
        className="px-2 h-[22px] rounded-[2px] text-[11px] cursor-pointer"
        style={{
          background: active ? 'var(--bg-surface)' : 'transparent',
          color: active ? 'var(--accent)' : 'var(--text-secondary)',
          boxShadow: active ? '0 0 0 1px var(--border-subtle)' : undefined,
        }}
      >
        {label}
      </button>
    ))}
  </div>
);

/** Options and instructions for the active 2D tool (shown in the properties panel). */
export const ToolOptions: React.FC = () => {
  const t = useT();
  const tool = useStudioStore((s) => s.activeTool);
  const unit = useStudioStore((s) => s.unit);
  const snapshot = useStudioStore((s) => s.snapshot);
  const dartPivotAngleDeg = useStudioStore((s) => s.dartPivotAngleDeg);
  const slashCount = useStudioStore((s) => s.slashCount);
  const slashSpreadMm = useStudioStore((s) => s.slashSpreadMm);
  const slashHingeMode = useStudioStore((s) => s.slashHingeMode);
  const notchStyle = useStudioStore((s) => s.notchStyle);
  const notchCount = useStudioStore((s) => s.notchCount);
  const trueCornerSquare90 = useStudioStore((s) => s.trueCornerSquare90);
  const styleSplitAnchor = useStudioStore((s) => s.styleSplitAnchor);
  const activeWalkLinkId = useStudioStore((s) => s.activeWalkLinkId);
  const walkProgress = useStudioStore((s) => s.walkProgress);
  const walkFrame = useStudioStore((s) => s.walkFrame);
  const measureReadout = useStudioStore((s) => s.measureReadout);
  const st = useStudioStore.getState;

  const promptKey = TOOL_PROMPTS[tool];
  const prompt =
    tool === 'style_split' && styleSplitAnchor
      ? t('v2d.anchorLocked', { index: styleSplitAnchor.edgeIdx + 1 })
      : promptKey
        ? t(promptKey)
        : '';

  let body: React.ReactNode = null;
  switch (tool) {
    case 'dart_pivot':
      body = (
        <Field label={t('panel.angle')}>
          <span className="flex items-center gap-2">
            <input
              type="range"
              min={4}
              max={30}
              step={1}
              value={dartPivotAngleDeg}
              onChange={(e) => st().setDartPivotAngle(Number(e.target.value))}
              className="w-28"
            />
            <span className="font-mono-num w-8 text-right">{dartPivotAngleDeg}°</span>
          </span>
        </Field>
      );
      break;
    case 'slash_spread':
      body = (
        <>
          <Field label={t('v2d.slashes')}>
            <input
              type="number"
              min={1}
              max={6}
              value={slashCount}
              onChange={(e) => st().setSlashParams(Number(e.target.value), slashSpreadMm, slashHingeMode)}
              className="w-16 h-[22px] rounded border px-1.5 font-mono-num text-right"
              style={inputStyle}
            />
          </Field>
          <Field label={t('v2d.spreadMm')}>
            <input
              type="number"
              min={5}
              max={80}
              value={slashSpreadMm}
              onChange={(e) => st().setSlashParams(slashCount, Number(e.target.value), slashHingeMode)}
              className="w-16 h-[22px] rounded border px-1.5 font-mono-num text-right"
              style={inputStyle}
            />
          </Field>
          <Field label={t('panel.hinge')}>
            <Segmented
              options={[
                [slashHingeMode === 'edge_hinge', t('timeline.edgeHinge'), () => st().setSlashParams(slashCount, slashSpreadMm, 'edge_hinge')],
                [
                  slashHingeMode === 'parallel_shift',
                  t('v2d.parallel'),
                  () => st().setSlashParams(slashCount, slashSpreadMm, 'parallel_shift'),
                ],
              ]}
            />
          </Field>
        </>
      );
      break;
    case 'notch':
      body = (
        <>
          <Field label={t('panel.notchStyle')}>
            <select
              value={notchStyle}
              onChange={(e) => st().setNotchOptions(e.target.value as typeof notchStyle, notchCount)}
              className="h-[22px] rounded border px-1"
              style={inputStyle}
            >
              <option value="slit">{t('timeline.notchSlit')}</option>
              <option value="v">{t('timeline.notchV')}</option>
              <option value="t">{t('timeline.notchT')}</option>
              <option value="u">{t('timeline.notchU')}</option>
              <option value="castle">{t('timeline.notchCastle')}</option>
            </select>
          </Field>
          <Field label={t('panel.count')}>
            <Segmented
              options={[1, 2, 3].map((n) => [notchCount === n, `${n}×`, () => st().setNotchOptions(notchStyle, n)])}
            />
          </Field>
        </>
      );
      break;
    case 'true_corner':
      body = (
        <Field label={t('panel.cornerMode')}>
          <Segmented
            options={[
              [trueCornerSquare90, t('v2d.square90'), () => st().setTrueCornerSquare90(true)],
              [!trueCornerSquare90, t('v2d.g1Smooth'), () => st().setTrueCornerSquare90(false)],
            ]}
          />
        </Field>
      );
      break;
    case 'walk_seam':
      body = snapshot && (
        <>
          <Field label={t('panel.seam')}>
            <select
              value={activeWalkLinkId ?? ''}
              onChange={(e) => st().startWalkSeam(e.target.value)}
              className="h-[22px] rounded border px-1 max-w-[170px]"
              style={inputStyle}
            >
              {snapshot.stitching_links.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.label}
                </option>
              ))}
            </select>
          </Field>
          <Field label={t('panel.position')}>
            <span className="flex items-center gap-2">
              <input
                type="range"
                min={0}
                max={1}
                step={0.01}
                value={walkProgress}
                onChange={(e) => st().updateWalkProgress(parseFloat(e.target.value))}
                className="w-28"
              />
              <span className="font-mono-num w-9 text-right">{(walkProgress * 100).toFixed(0)}%</span>
            </span>
          </Field>
          {walkFrame && (
            <div className="grid grid-cols-3 gap-1 text-[11px] font-mono-num">
              <span>A {formatMm(walkFrame.walked_a_mm, unit)}</span>
              <span>B {formatMm(walkFrame.walked_b_mm, unit)}</span>
              <span className="text-emerald-700 dark:text-emerald-400">
                {t('v2d.ease')} {walkFrame.ease_delta_mm >= 0 ? '+' : ''}
                {formatMm(walkFrame.ease_delta_mm, unit)}
              </span>
            </div>
          )}
        </>
      );
      break;
    case 'measure':
      body = measureReadout && (
        <div className="text-[11px] font-mono-num" style={{ color: 'var(--accent)' }}>
          {t('status.measure', {
            dist: formatMm(measureReadout.dist, unit, 1),
            dx: formatMm(measureReadout.dx, unit, 1),
            dy: formatMm(measureReadout.dy, unit, 1),
            angle: measureReadout.angleDeg.toFixed(1),
          })}
        </div>
      );
      break;
    case 'direct_edit':
      body = (
        <p className="text-[11px]" style={{ color: 'var(--text-muted)' }}>
          <kbd className="font-mono-num font-semibold">Shift</kbd> {t('v2d.g1Lock')}
        </p>
      );
      break;
  }

  return (
    <div className="p-3 space-y-3 text-xs">
      {prompt && (
        <p className="text-[11px] leading-snug" style={{ color: 'var(--text-secondary)' }}>
          {prompt}
        </p>
      )}
      {body && <div className="space-y-2.5">{body}</div>}
      <div className="pt-1 flex justify-end">
        <button
          onClick={() => (tool === 'walk_seam' ? st().stopWalkSeam() : st().setActiveTool('select'))}
          className="h-[24px] px-3 rounded inline-flex items-center gap-1 font-medium text-white cursor-pointer"
          style={{ background: 'var(--accent)' }}
          title="Esc"
        >
          <Check className="w-3.5 h-3.5" />
          {t('panel.finish')}
        </button>
      </div>
    </div>
  );
};
