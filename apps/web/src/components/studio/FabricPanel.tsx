import React, { useMemo, useState } from 'react';
import { ChevronDown, ChevronRight, Info, RotateCcw } from 'lucide-react';
import { getFabricPropertySpecs, kernel } from '../../lib/kernelClient';
import { useCatalogT, useT } from '../../lib/i18n';
import type { MessageKey } from '../../lib/i18n';
import {
  FABRIC_SWATCHES,
  type FabricFinish,
  type FabricPropertySpec,
  type FabricSpec,
  type UiSnapshot,
} from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

const FINISHES: FabricFinish[] = ['plain', 'twill', 'satin', 'jersey', 'rib', 'sheer', 'terry'];
const GROUPS: FabricPropertySpec['group'][] = ['Body', 'Stretch', 'Drape', 'Contact', 'Finishing'];

/** Slider position (0..1000) <-> value, logarithmic for stiffnesses spanning decades. */
function toSlider(spec: FabricPropertySpec, v: number): number {
  if (spec.log_scale) {
    const lo = Math.log(spec.min);
    const hi = Math.log(spec.max);
    return Math.round(((Math.log(Math.max(v, spec.min)) - lo) / (hi - lo)) * 1000);
  }
  return Math.round(((v - spec.min) / (spec.max - spec.min)) * 1000);
}

function fromSlider(spec: FabricPropertySpec, pos: number): number {
  const t = pos / 1000;
  if (spec.log_scale) {
    const v = Math.exp(Math.log(spec.min) + t * (Math.log(spec.max) - Math.log(spec.min)));
    return v >= 100 ? Math.round(v) : Math.round(v * 10) / 10;
  }
  const v = spec.min + t * (spec.max - spec.min);
  return Math.round(v / spec.step) * spec.step;
}

function formatValue(spec: FabricPropertySpec, v: number): string {
  if (spec.step >= 1) return v.toFixed(0);
  if (spec.step >= 0.1) return v.toFixed(1);
  return v.toFixed(2);
}

const PropertyRow: React.FC<{
  spec: FabricPropertySpec;
  value: number;
  onChange: (v: number) => void;
}> = ({ spec, value, onChange }) => {
  const t = useT();
  const label = t(`fabricProp.${spec.key}.label` as MessageKey);
  const help = t(`fabricProp.${spec.key}.help` as MessageKey);
  return (
    <div className="space-y-0.5">
      <div className="flex items-center justify-between gap-2">
        <span className="min-w-0 leading-tight inline-flex items-center gap-1" title={help}>
          {label}
          <Info className="w-2.5 h-2.5 opacity-40 shrink-0" />
        </span>
        <span className="inline-flex items-center gap-1 shrink-0">
          <input
            type="number"
            step={spec.step}
            min={spec.min}
            max={spec.max}
            value={formatValue(spec, value)}
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              if (Number.isFinite(v)) onChange(v);
            }}
            className="w-16 text-right font-mono-num rounded border px-1 py-0.5 text-[10px]"
            style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
          />
          <span className="w-9 text-[9px] font-mono-num" style={{ color: 'var(--text-muted)' }}>
            {spec.unit}
          </span>
        </span>
      </div>
      <input
        type="range"
        min={0}
        max={1000}
        step={1}
        value={toSlider(spec, value)}
        onChange={(e) => onChange(fromSlider(spec, parseFloat(e.target.value)))}
        className="w-full h-1.5 cursor-pointer"
        title={help}
      />
    </div>
  );
};

const PresetSelect: React.FC<{
  value: string;
  catalog: FabricSpec[];
  includeShell?: boolean;
  onChange: (id: string) => void;
}> = ({ value, catalog, includeShell, onChange }) => {
  const t = useT();
  const ct = useCatalogT();
  const groups = (['woven', 'knit', 'nonwoven'] as const)
    .map((c) => ({ c, items: catalog.filter((f) => f.construction === c) }))
    .filter((g) => g.items.length > 0);
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full rounded border px-1.5 py-1 text-[11px]"
      style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
    >
      {includeShell && <option value="">{t('fabric.sameAsShell')}</option>}
      {groups.map((g) => (
        <optgroup key={g.c} label={t(`fabric.${g.c}` as MessageKey)}>
          {g.items.map((f) => (
            <option key={f.id} value={f.id}>
              {ct('fabricPreset', f.id, f.name)}
            </option>
          ))}
        </optgroup>
      ))}
    </select>
  );
};

export const FabricPanel: React.FC<{ snapshot: UiSnapshot }> = ({ snapshot }) => {
  const t = useT();
  const ct = useCatalogT();
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const specs = useMemo(() => getFabricPropertySpecs(), []);
  const [openGroups, setOpenGroups] = useState<Record<string, boolean>>({ Stretch: true, Drape: true });
  const fabric = snapshot.fabric;
  const metrics = snapshot.fabric_metrics;

  return (
    <div className="space-y-3">
      {/* Shell fabric: preset, drape summary, colour, finish */}
      <div className="p-2.5 rounded-lg border space-y-2" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
        <div className="flex items-center justify-between">
          <span className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>
            {t('fabric.active')}
          </span>
          {snapshot.fabric_edited && (
            <button
              onClick={() => setSnapshot(kernel.resetFabricToPreset())}
              className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-[10px] cursor-pointer"
              style={{ borderColor: 'var(--accent)', color: 'var(--accent)', background: 'var(--accent-soft)' }}
              title={t('fabric.resetPreset')}
            >
              <RotateCcw className="w-2.5 h-2.5" />
              {t('fabric.edited')}
            </button>
          )}
        </div>
        <PresetSelect
          value={fabric.id}
          catalog={snapshot.fabric_catalog}
          onChange={(id) => setSnapshot(kernel.setFabricPreset(id))}
        />
        <div className="text-[10px]" style={{ color: 'var(--text-muted)' }}>
          {fabric.composition} · {t('fabric.gsm', { gsm: fabric.density_gsm.toFixed(0) })}
        </div>
        <div className="grid grid-cols-2 gap-1.5 text-[10px]">
          <div className="rounded px-1.5 py-1" style={{ background: 'var(--bg-panel)' }}>
            <div style={{ color: 'var(--text-muted)' }}>{t('fabric.drape')}</div>
            <div className="font-semibold">{t(`drape.${metrics.drape}` as MessageKey)}</div>
          </div>
          <div className="rounded px-1.5 py-1" style={{ background: 'var(--bg-panel)' }} title={t('fabric.bendingLength')}>
            <div style={{ color: 'var(--text-muted)' }}>{t('fabric.bendingLength')}</div>
            <div className="font-mono-num">
              {t('fabric.bendingLengthValue', {
                warp: metrics.bending_length_warp_cm.toFixed(1),
                weft: metrics.bending_length_weft_cm.toFixed(1),
              })}
            </div>
          </div>
        </div>

        <div className="text-[10px] font-semibold uppercase tracking-wider pt-1" style={{ color: 'var(--text-secondary)' }}>
          {t('fabric.colorway')}
        </div>
        <div className="grid grid-cols-9 gap-1 items-center">
          {FABRIC_SWATCHES.map((sw) => {
            const active = fabric.color_hex.toLowerCase() === sw.hex.toLowerCase();
            return (
              <button
                key={sw.hex}
                onClick={() => setSnapshot(kernel.setFabricColor(sw.hex))}
                className="w-6 h-6 rounded-full border cursor-pointer"
                style={{
                  background: sw.hex,
                  borderColor: active ? 'var(--accent)' : 'rgba(0,0,0,0.2)',
                  boxShadow: active ? '0 0 0 2px var(--accent-soft)' : 'none',
                }}
                title={ct('swatch', sw.hex, sw.name)}
              />
            );
          })}
          <input
            type="color"
            value={fabric.color_hex}
            onChange={(e) => setSnapshot(kernel.setFabricColor(e.target.value))}
            className="w-6 h-6 rounded-full border cursor-pointer p-0"
            style={{ borderColor: 'var(--border-subtle)' }}
            title={t('fabric.customColor')}
          />
        </div>
        <label className="block">
          <span className="text-[10px]" style={{ color: 'var(--text-muted)' }} title={t('fabric.finishHelp')}>
            {t('fabric.finish')}
          </span>
          <select
            value={fabric.finish}
            onChange={(e) => setSnapshot(kernel.setFabricFinish(e.target.value as FabricFinish))}
            className="w-full rounded border px-1.5 py-1 text-[11px] mt-0.5"
            style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
          >
            {FINISHES.map((f) => (
              <option key={f} value={f}>
                {t(`finish.${f}` as MessageKey)}
              </option>
            ))}
          </select>
        </label>
      </div>

      {/* Physical properties, grouped */}
      <div className="p-2.5 rounded-lg border space-y-1.5" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
        <div className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>
          {t('fabric.properties')}
        </div>
        <p className="text-[10px] leading-snug" style={{ color: 'var(--text-muted)' }}>
          {t('fabric.propertiesHelp')}
        </p>
        {GROUPS.map((g) => {
          const rows = specs.filter((s) => s.group === g);
          if (rows.length === 0) return null;
          const open = openGroups[g] ?? false;
          return (
            <div key={g} className="border-t pt-1" style={{ borderColor: 'var(--border-subtle)' }}>
              <button
                onClick={() => setOpenGroups((o) => ({ ...o, [g]: !open }))}
                className="w-full flex items-center gap-1 text-[10px] font-semibold py-0.5 cursor-pointer"
              >
                {open ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                {t(`fabricGroup.${g}` as MessageKey)}
              </button>
              {open && (
                <div className="space-y-1.5 pl-1 pt-0.5">
                  {rows.map((spec) => (
                    <PropertyRow
                      key={spec.key}
                      spec={spec}
                      value={fabric[spec.key]}
                      onChange={(v) => setSnapshot(kernel.setFabricProperty(spec.key, v))}
                    />
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* Per-role fabrics (rib, lining, contrast...) */}
      {snapshot.role_fabrics.length > 0 && (
        <div className="p-2.5 rounded-lg border space-y-1.5" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
          <div className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>
            {t('fabric.roles')}
          </div>
          <p className="text-[10px] leading-snug" style={{ color: 'var(--text-muted)' }}>
            {t('fabric.rolesHelp')}
          </p>
          {snapshot.role_fabrics.map((r) => (
            <div key={r.role} className="space-y-0.5">
              <div className="flex items-center justify-between text-[11px]">
                <span className="font-medium">{r.label}</span>
                {r.fabric && (
                  <input
                    type="color"
                    value={r.fabric.color_hex}
                    onChange={(e) => setSnapshot(kernel.setRoleFabricColor(r.role, e.target.value))}
                    className="w-5 h-5 rounded-full border cursor-pointer p-0"
                    style={{ borderColor: 'var(--border-subtle)' }}
                    title={t('fabric.customColor')}
                  />
                )}
              </div>
              <PresetSelect
                value={r.fabric?.id ?? ''}
                catalog={snapshot.fabric_catalog}
                includeShell
                onChange={(id) => setSnapshot(kernel.setRoleFabricPreset(r.role, id))}
              />
            </div>
          ))}
        </div>
      )}

      <p className="text-[10px] leading-snug px-0.5" style={{ color: 'var(--text-muted)' }}>
        {t('fabric.formulaHint')}
      </p>
    </div>
  );
};
