import React, { useState } from 'react';
import { AlertTriangle, CheckCircle2, ChevronDown, ChevronRight, Link2, Plus, Trash2 } from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { findTemplate } from '../../lib/templateCatalog';
import { useCatalogT, useT, type MessageKey } from '../../lib/i18n';
import {
  displayToMm,
  formatMm,
  mmToDisplay,
  type CornerRule,
  type GradeMode,
  type GrainRotationMode,
  type StackAnchor,
} from '../../lib/types';
import { useStudioStore, type PanelPage } from '../../store/studioStore';
import { ExpressionInput } from './ExpressionInput';
import { FabricPanel } from './FabricPanel';

const STITCH_SA_PRESETS: Array<{ label: MessageKey; width: number; rule: CornerRule }> = [
  { label: 'inspector.saFold', width: 0, rule: { kind: 'square' } },
  { label: 'inspector.saOverlock', width: 7, rule: { kind: 'miter', limit: 2.5 } },
  { label: 'inspector.saPlain', width: 10, rule: { kind: 'miter', limit: 2.5 } },
  { label: 'inspector.saFrench', width: 15, rule: { kind: 'miter', limit: 2.5 } },
  { label: 'inspector.saFlatFell', width: 16, rule: { kind: 'matched_seam' } },
  { label: 'inspector.saHem', width: 30, rule: { kind: 'folded_hem' } },
];

const isFabricVar = (name: string) => name.startsWith('fabric_');

/**
 * Pages of the properties panel: parameters (body measurements, ease, formulas), the selected
 * piece, materials, grading. Which one shows is decided by `PropertiesPanel`.
 */
export const RightInspector: React.FC<{ view: PanelPage }> = ({ view }) => {
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const t = useT();
  const ct = useCatalogT();

  const [showOtherBody, setShowOtherBody] = useState(false);
  const [newVarName, setNewVarName] = useState('');
  const [newVarExpr, setNewVarExpr] = useState('');

  if (!snapshot) return null;

  const selectedPiece =
    snapshot.pieces.find((p) => p.id === snapshot.selected_piece_id) ?? snapshot.pieces[0];
  const selectedEdge =
    selectedPiece?.edges.find((e) => e.edge_idx === snapshot.selected_edge_idx) ??
    selectedPiece?.edges[0];

  // Each template declares the measurements and global ease it drafts from in the kernel
  // catalog; starter styles keep their ease in named variables instead.
  const templateInfo = findTemplate(snapshot.template_id) ?? findTemplate('bodice_sleeve');
  const primaryKeys = new Set(templateInfo?.measurement_keys ?? []);
  const primaryEaseKeys = new Set(templateInfo?.ease_keys ?? []);

  const primaryMeasurements = snapshot.measurements.filter((m) => primaryKeys.has(m.key));
  const otherMeasurements = snapshot.measurements.filter((m) => !primaryKeys.has(m.key));
  const primaryEase = snapshot.ease.filter((m) => primaryEaseKeys.has(m.key));

  const handleRuleChange = (edgeIdx: number, widthMm: number, kind: CornerRule['kind']) => {
    if (!selectedPiece) return;
    const rule: CornerRule = kind === 'miter' ? { kind: 'miter', limit: 2.5 } : { kind };
    setSnapshot(kernel.setEdgeSa(selectedPiece.id, edgeIdx, widthMm, rule));
  };

  const toggleVisibleSize = (size: string) => {
    const curr = new Set(snapshot.visible_sizes);
    if (curr.has(size) && size !== snapshot.active_size) {
      curr.delete(size);
    } else {
      curr.add(size);
    }
    setSnapshot(kernel.setVisibleSizes(Array.from(curr)));
  };

  const renderSlider = (
    m: { key: string; label: string; value_mm: number; min_mm: number; max_mm: number },
    isEase = false,
  ) => (
    <div key={m.key} className="space-y-1">
      <div className="flex items-center justify-between gap-2">
        <span className="truncate">{ct('measure', m.key, m.label)}</span>
        <input
          type="number"
          step={unit === 'mm' ? 1 : 0.1}
          value={Number(mmToDisplay(m.value_mm, unit).toFixed(unit === 'mm' ? 0 : 1))}
          onChange={(e) => {
            const v = parseFloat(e.target.value);
            if (!Number.isNaN(v)) {
              setSnapshot(
                isEase
                  ? kernel.setEase(m.key, displayToMm(v, unit))
                  : kernel.setMeasurement(m.key, displayToMm(v, unit)),
              );
            }
          }}
          className="w-16 text-right font-mono-num rounded border px-1.5 py-0.5 text-[11px]"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
        />
      </div>
      <input
        type="range"
        min={m.min_mm}
        max={m.max_mm}
        step={1}
        value={m.value_mm}
        onChange={(e) =>
          setSnapshot(
            isEase
              ? kernel.setEase(m.key, parseFloat(e.target.value))
              : kernel.setMeasurement(m.key, parseFloat(e.target.value)),
          )
        }
        className="w-full h-1.5 cursor-pointer"
      />
    </div>
  );

  return (
    <div className="flex-1 min-h-0 flex flex-col overflow-hidden select-none">
      {/* Tab Body */}
      <div className="flex-1 min-h-0 overflow-y-auto p-3 space-y-4 text-xs">
        {snapshot.components.length > 1 && (view === 'params' || view === 'materials' || view === 'piece') && (
          <div
            className="px-2 py-1 rounded-[3px] text-[11px] font-medium"
            style={{ background: 'var(--accent-soft)', color: 'var(--accent)' }}
          >
            {t('panel.garmentScope', {
              name: snapshot.components.find((c) => c.id === snapshot.active_component_id)?.name ?? '',
            })}
          </div>
        )}
        {view === 'params' && (
          <>
            <div>
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>
                  {t('inspector.pom', { unit })}
                </h3>
                <span className="font-mono-num text-[10px]" style={{ color: 'var(--accent)' }}>
                  {t('inspector.base', { size: snapshot.active_size })}
                </span>
              </div>
              <div className="space-y-2">{primaryMeasurements.map((m) => renderSlider(m, false))}</div>
            </div>

            <div className="pt-2.5 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
              <h3 className="text-[10px] font-semibold uppercase tracking-wider mb-2" style={{ color: 'var(--text-secondary)' }}>
                {t('inspector.ease', { unit })}
              </h3>
              {primaryEase.length > 0 ? (
                <div className="space-y-2">{primaryEase.map((m) => renderSlider(m, true))}</div>
              ) : (
                <button
                  onClick={() => document.getElementById('params-formulas')?.scrollIntoView({ behavior: 'smooth' })}
                  className="text-[11px] text-left leading-snug underline-offset-2 hover:underline cursor-pointer"
                  style={{ color: 'var(--text-secondary)' }}
                >
                  {t('inspector.styleEase')}
                </button>
              )}
            </div>

            {otherMeasurements.length > 0 && (
              <div className="pt-2 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
                <button
                  onClick={() => setShowOtherBody((v) => !v)}
                  className="w-full flex items-center justify-between text-[10px] font-semibold uppercase tracking-wider py-1 cursor-pointer"
                  style={{ color: 'var(--text-muted)' }}
                >
                  <span>{t('inspector.fullChart', { count: otherMeasurements.length })}</span>
                  {showOtherBody ? (
                    <ChevronDown className="w-3.5 h-3.5" />
                  ) : (
                    <ChevronRight className="w-3.5 h-3.5" />
                  )}
                </button>
                {showOtherBody && (
                  <div className="mt-2 space-y-2">
                    {otherMeasurements.map((m) => renderSlider(m, false))}
                  </div>
                )}
              </div>
            )}
          </>
        )}

        {view === 'piece' && selectedPiece && (
          <>
            <div className="p-2.5 rounded border space-y-2" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
              <div className="flex items-center justify-between">
                <span className="font-semibold">{selectedPiece.name}</span>
                <span className="font-mono-num text-[10px]" style={{ color: 'var(--text-muted)' }}>
                  {selectedPiece.code}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-1.5 text-[10px] font-mono-num" style={{ color: 'var(--text-secondary)' }}>
                <div>{t('inspector.net', { area: (selectedPiece.area_mm2 / 100).toFixed(1) })}</div>
                <div>{t('inspector.cut', { area: (selectedPiece.cut_area_mm2 / 100).toFixed(1) })}</div>
              </div>

              <div className="grid grid-cols-2 gap-1.5">
                <button
                  onClick={() => setSnapshot(kernel.mirrorPiece(selectedPiece.id))}
                  className="px-2 py-1 rounded border text-[11px] hover:bg-[var(--bg-subtle)] cursor-pointer"
                  style={{ borderColor: 'var(--border-subtle)' }}
                >
                  {t('inspector.mirror')}
                </button>
                <button
                  onClick={() => setSnapshot(kernel.duplicatePiece(selectedPiece.id))}
                  className="px-2 py-1 rounded border text-[11px] hover:bg-[var(--bg-subtle)] cursor-pointer"
                  style={{ borderColor: 'var(--border-subtle)' }}
                >
                  {t('inspector.duplicate')}
                </button>
              </div>

              <div>
                <label className="block text-[10px] mb-1 uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>
                  {t('inspector.grain')}
                </label>
                <select
                  value={selectedPiece.grain_rotation}
                  onChange={(e) =>
                    setSnapshot(
                      kernel.setGrainRotation(selectedPiece.id, e.target.value as GrainRotationMode),
                    )
                  }
                  className="w-full rounded border px-2 py-1 text-[11px]"
                  style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="one_way">{t('inspector.grainOneWay')}</option>
                  <option value="two_way">{t('inspector.grainTwoWay')}</option>
                  <option value="bias">{t('inspector.grainBias')}</option>
                  <option value="free">{t('inspector.grainFree')}</option>
                </select>
              </div>

              {selectedPiece.warnings.length > 0 && (
                <div className="p-2 rounded bg-amber-500/15 text-amber-700 dark:text-amber-300 text-[10px] space-y-1">
                  {selectedPiece.warnings.map((w, i) => (
                    <div key={i} className="flex items-start gap-1">
                      <AlertTriangle className="w-3 h-3 shrink-0 mt-0.5" />
                      <span>{w}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {selectedEdge && (
              <div className="p-2.5 rounded border space-y-2.5" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
                <div className="flex items-center justify-between">
                  <span className="font-semibold truncate">
                    #{selectedEdge.edge_idx + 1} {selectedEdge.label}
                  </span>
                  <span className="font-mono-num text-[11px]">
                    {formatMm(selectedEdge.length_mm, unit, 1)}
                  </span>
                </div>

                {/* Contextual Seam Interface Shortcut (Fusion 360 Progressive Discovery) */}
                {(() => {
                  const owner = snapshot.stitching_links.find(
                    (l) =>
                      l.edges_a.some((er) => er.edge_id === selectedEdge.edge_id) ||
                      l.edges_b.some((er) => er.edge_id === selectedEdge.edge_id),
                  );
                  return owner ? (
                    <button
                      onClick={() => useStudioStore.getState().openEditSeamCommand(owner)}
                      className="w-full px-2 py-1.5 rounded-md border text-[11px] flex items-center justify-between gap-1.5 cursor-pointer hover:opacity-90"
                      style={{
                        borderColor: 'var(--accent)',
                        background: 'var(--accent-soft)',
                        color: 'var(--accent)',
                      }}
                    >
                      <span className="inline-flex items-center gap-1 truncate font-medium">
                        <Link2 className="w-3 h-3 shrink-0" />
                        <span className="truncate">{owner.label}</span>
                      </span>
                      <span className="text-[10px] font-semibold shrink-0">{t('inspector.editArrow')}</span>
                    </button>
                  ) : (
                    <button
                      onClick={() =>
                        useStudioStore.getState().openCreateSeamCommand({
                          piece_id: selectedPiece.id,
                          piece_code: selectedPiece.code,
                          edge_id: selectedEdge.edge_id,
                          edge_idx: selectedEdge.edge_idx,
                          edge_label: selectedEdge.label,
                          length_mm: selectedEdge.length_mm,
                          dir: 1,
                        })
                      }
                      className="w-full px-2 py-1 rounded-md border border-dashed text-[11px] flex items-center justify-center gap-1 cursor-pointer hover:bg-[var(--bg-subtle)]"
                      style={{ borderColor: 'var(--border-strong)', color: 'var(--text-secondary)' }}
                    >
                      <Link2 className="w-3 h-3" />
                      <span>{t('inspector.bindSeam')}</span>
                    </button>
                  );
                })()}

                {/* CLO3D-style Exact Change Edge Length + Curve Toggle */}
                <div className="grid grid-cols-2 gap-1.5">
                  <button
                    onClick={() =>
                      setSnapshot(
                        kernel.convertEdgeCurve(
                          selectedPiece.id,
                          selectedEdge.edge_idx,
                          !selectedEdge.is_cubic,
                        ),
                      )
                    }
                    className="px-2 py-1 rounded border text-[10px] hover:bg-[var(--bg-subtle)] cursor-pointer"
                    style={{ borderColor: 'var(--border-subtle)' }}
                  >
                    {selectedEdge.is_cubic ? t('inspector.makeStraight') : t('inspector.makeCurve')}
                  </button>
                  <button
                    onClick={() =>
                      setSnapshot(kernel.splitEdgePoint(selectedPiece.id, selectedEdge.edge_idx, 0.5))
                    }
                    className="px-2 py-1 rounded border text-[10px] hover:bg-[var(--bg-subtle)] cursor-pointer"
                    style={{ borderColor: 'var(--border-subtle)' }}
                  >
                    {t('inspector.splitMid')}
                  </button>
                </div>

                <div className="flex items-center gap-1.5">
                  <ExpressionInput
                    valueMm={selectedEdge.length_mm}
                    unit={unit}
                    onCommitMm={(mm) => {
                      if (Number.isFinite(mm) && mm > 5) {
                        setSnapshot(
                          kernel.setEdgeLength(
                            selectedPiece.id,
                            selectedEdge.edge_idx,
                            mm,
                          ),
                        );
                      }
                    }}
                    placeholder={selectedEdge.length_mm.toFixed(1)}
                    className="w-full rounded border px-2 py-1 text-[11px]"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                    title={t('inspector.edgeLengthTitle')}
                  />
                  <span
                    className="px-2 py-1 rounded text-[10px] font-mono-num shrink-0"
                    style={{ background: 'var(--bg-panel)', color: 'var(--text-muted)' }}
                  >
                    {unit}
                  </span>
                </div>

                <div>
                  <label className="block text-[10px] mb-1" style={{ color: 'var(--text-secondary)' }}>
                    {t('inspector.saPresets', { unit })}
                  </label>
                  <div className="grid grid-cols-3 gap-1 mb-1.5">
                    {STITCH_SA_PRESETS.map((pr) => (
                      <button
                        key={pr.label}
                        onClick={() =>
                          setSnapshot(
                            kernel.setEdgeSa(selectedPiece.id, selectedEdge.edge_idx, pr.width, pr.rule),
                          )
                        }
                        className="px-1 py-1 rounded border font-mono-num text-[10px] cursor-pointer text-center"
                        style={{
                          borderColor:
                            Math.abs(selectedEdge.sa_width_mm - pr.width) < 0.2
                              ? 'var(--accent)'
                              : 'var(--border-subtle)',
                          background:
                            Math.abs(selectedEdge.sa_width_mm - pr.width) < 0.2
                              ? 'var(--accent-soft)'
                              : 'transparent',
                        }}
                      >
                        {t(pr.label)}
                      </button>
                    ))}
                  </div>
                  <div className="grid grid-cols-2 gap-1.5">
                    <ExpressionInput
                      valueMm={selectedEdge.sa_width_mm}
                      unit={unit}
                      onCommitMm={(mm) => {
                        if (Number.isFinite(mm)) {
                          handleRuleChange(
                            selectedEdge.edge_idx,
                            Math.max(0, Math.min(80, mm)),
                            selectedEdge.corner_rule.kind,
                          );
                        }
                      }}
                      className="w-full rounded border px-2 py-1 text-[11px]"
                      style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                      title={t('inspector.saTitle')}
                    />
                    <select
                      value={selectedEdge.corner_rule.kind}
                      onChange={(e) =>
                        handleRuleChange(
                          selectedEdge.edge_idx,
                          selectedEdge.sa_width_mm,
                          e.target.value as CornerRule['kind'],
                        )
                      }
                      className="rounded border px-1.5 py-1 text-[10px]"
                      style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                    >
                      <option value="miter">{t('inspector.ruleMiter')}</option>
                      <option value="folded_hem">{t('inspector.ruleFoldedHem')}</option>
                      <option value="matched_seam">{t('inspector.ruleMatched')}</option>
                      <option value="square">{t('inspector.ruleSquare')}</option>
                      <option value="bevel">{t('inspector.ruleBevel')}</option>
                      <option value="dart_cap">{t('inspector.ruleDartCap')}</option>
                    </select>
                  </div>
                </div>

                {/* Hem (Ourlet) */}
                <div className="pt-1.5 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-[10px]" style={{ color: 'var(--text-secondary)' }}>
                      {t('inspector.hemLabel')}
                    </span>
                    <select
                      value={selectedEdge.hem ? String(selectedEdge.hem.fold_count) : '0'}
                      onChange={(e) => {
                        const fc = Number(e.target.value);
                        const nextHem =
                          fc === 0
                            ? null
                            : {
                                depth_mm: selectedEdge.hem?.depth_mm ?? 25,
                                fold_count: fc,
                                turn_out: selectedEdge.hem?.turn_out ?? false,
                              };
                        setSnapshot(
                          kernel.setEdgeFinish(
                            selectedPiece.id,
                            selectedEdge.edge_idx,
                            nextHem,
                            selectedEdge.elastic ?? null,
                          ),
                        );
                      }}
                      className="rounded border px-1.5 py-0.5 text-[10px]"
                      style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                    >
                      <option value="0">{t('inspector.hemNone')}</option>
                      <option value="1">{t('inspector.hemSingle')}</option>
                      <option value="2">{t('inspector.hemDouble')}</option>
                    </select>
                  </div>
                  {selectedEdge.hem && (
                    <div className="grid grid-cols-2 gap-1.5 items-center">
                      <ExpressionInput
                        valueMm={selectedEdge.hem.depth_mm}
                        unit={unit}
                        onCommitMm={(mm) => {
                          if (!Number.isFinite(mm) || !selectedEdge.hem) return;
                          setSnapshot(
                            kernel.setEdgeFinish(
                              selectedPiece.id,
                              selectedEdge.edge_idx,
                              { ...selectedEdge.hem, depth_mm: Math.max(8, Math.min(80, mm)) },
                              selectedEdge.elastic ?? null,
                            ),
                          );
                        }}
                        className="w-full rounded border px-2 py-0.5 text-[10px]"
                        style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                      />
                      <label className="flex items-center gap-1 text-[10px] cursor-pointer">
                        <input
                          type="checkbox"
                          checked={Boolean(selectedEdge.hem.turn_out)}
                          onChange={(e) => {
                            if (!selectedEdge.hem) return;
                            setSnapshot(
                              kernel.setEdgeFinish(
                                selectedPiece.id,
                                selectedEdge.edge_idx,
                                { ...selectedEdge.hem, turn_out: e.target.checked },
                                selectedEdge.elastic ?? null,
                              ),
                            );
                          }}
                        />
                        {t('inspector.hemTurnOut')}
                      </label>
                    </div>
                  )}
                </div>

                {/* Elastic Band */}
                <div className="pt-1.5 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
                  <label className="flex items-center justify-between text-[10px] cursor-pointer mb-1">
                    <span style={{ color: 'var(--text-secondary)' }}>{t('inspector.elasticLabel')}</span>
                    <input
                      type="checkbox"
                      checked={Boolean(selectedEdge.elastic)}
                      onChange={(e) => {
                        const nextEl = e.target.checked
                          ? { target_ratio: 0.75, width_mm: 35, stiffness_n_m: 200 }
                          : null;
                        setSnapshot(
                          kernel.setEdgeFinish(
                            selectedPiece.id,
                            selectedEdge.edge_idx,
                            selectedEdge.hem ?? null,
                            nextEl,
                          ),
                        );
                      }}
                    />
                  </label>
                  {selectedEdge.elastic && (
                    <div className="flex items-center gap-2 text-[10px]">
                      <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                        {t('inspector.elasticGather')}
                      </span>
                      <input
                        type="range"
                        min={50}
                        max={95}
                        step={1}
                        value={Math.round(selectedEdge.elastic.target_ratio * 100)}
                        onChange={(e) => {
                          if (!selectedEdge.elastic) return;
                          setSnapshot(
                            kernel.setEdgeFinish(
                              selectedPiece.id,
                              selectedEdge.edge_idx,
                              selectedEdge.hem ?? null,
                              { ...selectedEdge.elastic, target_ratio: Number(e.target.value) / 100 },
                            ),
                          );
                        }}
                        className="flex-1"
                      />
                      <span className="font-mono-num w-8 text-right">
                        {Math.round(selectedEdge.elastic.target_ratio * 100)}%
                      </span>
                    </div>
                  )}
                </div>

                {/* Drawstring & Eyelets */}
                <div className="pt-1 border-t space-y-1.5" style={{ borderColor: 'var(--border-subtle)' }}>
                  <label className="flex items-center justify-between text-[11px] cursor-pointer">
                    <span style={{ color: 'var(--text-secondary)' }}>{t('inspector.drawstringLabel')}</span>
                    <input
                      type="checkbox"
                      checked={Boolean(selectedEdge.drawstring)}
                      onChange={(e) => {
                        const nextDs = e.target.checked
                          ? {
                              id: selectedEdge.edge_id,
                              label: `${selectedEdge.label} Drawstring`,
                              casing_edge_id: selectedEdge.edge_id,
                              opposite_edge_id: null,
                              tail_length_mm: 190,
                              cord_radius_mm: 3.0,
                              eyelet_offset_mm: 20,
                              tighten_ratio: 0.92,
                              color_hex: '#ebe7de',
                            }
                          : null;
                        setSnapshot(
                          kernel.setEdgeDrawstring(
                            selectedPiece.id,
                            selectedEdge.edge_idx,
                            nextDs,
                          ),
                        );
                      }}
                    />
                  </label>
                  {selectedEdge.drawstring && (
                    <div className="space-y-1.5 text-[10px]">
                      <div className="flex items-center gap-2">
                        <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                          {t('inspector.drawstringCinch')}
                        </span>
                        <input
                          type="range"
                          min={70}
                          max={100}
                          step={1}
                          value={Math.round(selectedEdge.drawstring.tighten_ratio * 100)}
                          onChange={(e) => {
                            if (!selectedEdge.drawstring) return;
                            setSnapshot(
                              kernel.setEdgeDrawstring(selectedPiece.id, selectedEdge.edge_idx, {
                                ...selectedEdge.drawstring,
                                tighten_ratio: Number(e.target.value) / 100,
                              }),
                            );
                          }}
                          className="flex-1"
                        />
                        <span className="font-mono-num w-8 text-right">
                          {Math.round(selectedEdge.drawstring.tighten_ratio * 100)}%
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                          {t('inspector.drawstringTailLen')}
                        </span>
                        <input
                          type="range"
                          min={80}
                          max={320}
                          step={5}
                          value={Math.round(selectedEdge.drawstring.tail_length_mm)}
                          onChange={(e) => {
                            if (!selectedEdge.drawstring) return;
                            setSnapshot(
                              kernel.setEdgeDrawstring(selectedPiece.id, selectedEdge.edge_idx, {
                                ...selectedEdge.drawstring,
                                tail_length_mm: Number(e.target.value),
                              }),
                            );
                          }}
                          className="flex-1"
                        />
                        <span className="font-mono-num w-9 text-right">
                          {Math.round(selectedEdge.drawstring.tail_length_mm)}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                          {t('inspector.drawstringRadius')}
                        </span>
                        <input
                          type="range"
                          min={1.5}
                          max={5.0}
                          step={0.5}
                          value={selectedEdge.drawstring.cord_radius_mm}
                          onChange={(e) => {
                            if (!selectedEdge.drawstring) return;
                            setSnapshot(
                              kernel.setEdgeDrawstring(selectedPiece.id, selectedEdge.edge_idx, {
                                ...selectedEdge.drawstring,
                                cord_radius_mm: Number(e.target.value),
                              }),
                            );
                          }}
                          className="flex-1"
                        />
                        <span className="font-mono-num w-8 text-right">
                          {selectedEdge.drawstring.cord_radius_mm.toFixed(1)}
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                          {t('inspector.drawstringEyeletOffset')}
                        </span>
                        <input
                          type="range"
                          min={10}
                          max={40}
                          step={1}
                          value={Math.round(selectedEdge.drawstring.eyelet_offset_mm)}
                          onChange={(e) => {
                            if (!selectedEdge.drawstring) return;
                            setSnapshot(
                              kernel.setEdgeDrawstring(selectedPiece.id, selectedEdge.edge_idx, {
                                ...selectedEdge.drawstring,
                                eyelet_offset_mm: Number(e.target.value),
                              }),
                            );
                          }}
                          className="flex-1"
                        />
                        <span className="font-mono-num w-8 text-right">
                          {Math.round(selectedEdge.drawstring.eyelet_offset_mm)}
                        </span>
                      </div>
                      <label className="flex items-center justify-between">
                        <span style={{ color: 'var(--text-muted)' }}>
                          {t('inspector.drawstringColor')}
                        </span>
                        <input
                          type="color"
                          value={selectedEdge.drawstring.color_hex}
                          onChange={(e) => {
                            if (!selectedEdge.drawstring) return;
                            setSnapshot(
                              kernel.setEdgeDrawstring(selectedPiece.id, selectedEdge.edge_idx, {
                                ...selectedEdge.drawstring,
                                color_hex: e.target.value,
                              }),
                            );
                          }}
                          className="w-6 h-5 p-0 border-0 bg-transparent cursor-pointer"
                        />
                      </label>
                    </div>
                  )}
                </div>

                {/* Hardware: Zipper, Button, Belt Loops */}
                <div className="pt-1 border-t space-y-1.5" style={{ borderColor: 'var(--border-subtle)' }}>
                  <label className="flex items-center justify-between text-[11px] cursor-pointer">
                    <span style={{ color: 'var(--text-secondary)' }}>{t('inspector.zipperLabel')}</span>
                    <input
                      type="checkbox"
                      checked={Boolean(selectedEdge.zipper)}
                      onChange={(e) => {
                        const nextZ = e.target.checked
                          ? {
                              id: selectedEdge.edge_id,
                              label: `${selectedEdge.label} Zipper`,
                              edge_id: selectedEdge.edge_id,
                              zip_fraction: 1.0,
                              teeth_width_mm: 5.0,
                              color_hex: '#b5b8be',
                            }
                          : null;
                        setSnapshot(
                          kernel.setEdgeHardware(
                            selectedPiece.id,
                            selectedEdge.edge_idx,
                            nextZ,
                            selectedEdge.button ?? null,
                            selectedEdge.belt_loop ?? null,
                          ),
                        );
                      }}
                    />
                  </label>
                  {selectedEdge.zipper && (
                    <div className="flex items-center gap-2 text-[10px]">
                      <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                        {t('inspector.zipperFraction')}
                      </span>
                      <input
                        type="range"
                        min={0}
                        max={100}
                        step={1}
                        value={Math.round(selectedEdge.zipper.zip_fraction * 100)}
                        onChange={(e) => {
                          if (!selectedEdge.zipper) return;
                          const frac = Number(e.target.value) / 100;
                          setSnapshot(
                            kernel.setEdgeHardware(
                              selectedPiece.id,
                              selectedEdge.edge_idx,
                              { ...selectedEdge.zipper, zip_fraction: frac },
                              selectedEdge.button ?? null,
                              selectedEdge.belt_loop ?? null,
                            ),
                          );
                        }}
                        className="flex-1"
                      />
                      <span className="font-mono-num w-8 text-right">
                        {Math.round(selectedEdge.zipper.zip_fraction * 100)}%
                      </span>
                    </div>
                  )}

                  <label className="flex items-center justify-between text-[11px] cursor-pointer">
                    <span style={{ color: 'var(--text-secondary)' }}>{t('inspector.buttonLabel')}</span>
                    <input
                      type="checkbox"
                      checked={Boolean(selectedEdge.button)}
                      onChange={(e) => {
                        const nextB = e.target.checked
                          ? {
                              id: selectedEdge.edge_id,
                              label: `${selectedEdge.label} Button`,
                              edge_id: selectedEdge.edge_id,
                              param_t: 0.1,
                              diameter_mm: 16.0,
                              color_hex: '#38332e',
                            }
                          : null;
                        setSnapshot(
                          kernel.setEdgeHardware(
                            selectedPiece.id,
                            selectedEdge.edge_idx,
                            selectedEdge.zipper ?? null,
                            nextB,
                            selectedEdge.belt_loop ?? null,
                          ),
                        );
                      }}
                    />
                  </label>
                  {selectedEdge.button && (
                    <div className="flex items-center gap-2 text-[10px]">
                      <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                        {t('inspector.buttonDiameter')}
                      </span>
                      <input
                        type="range"
                        min={10}
                        max={30}
                        step={1}
                        value={Math.round(selectedEdge.button.diameter_mm)}
                        onChange={(e) => {
                          if (!selectedEdge.button) return;
                          setSnapshot(
                            kernel.setEdgeHardware(
                              selectedPiece.id,
                              selectedEdge.edge_idx,
                              selectedEdge.zipper ?? null,
                              { ...selectedEdge.button, diameter_mm: Number(e.target.value) },
                              selectedEdge.belt_loop ?? null,
                            ),
                          );
                        }}
                        className="flex-1"
                      />
                      <span className="font-mono-num w-8 text-right">
                        {Math.round(selectedEdge.button.diameter_mm)}
                      </span>
                    </div>
                  )}

                  <label className="flex items-center justify-between text-[11px] cursor-pointer">
                    <span style={{ color: 'var(--text-secondary)' }}>{t('inspector.beltLoopLabel')}</span>
                    <input
                      type="checkbox"
                      checked={Boolean(selectedEdge.belt_loop)}
                      onChange={(e) => {
                        const nextL = e.target.checked
                          ? {
                              id: selectedEdge.edge_id,
                              label: `${selectedEdge.label} Belt Loops`,
                              base_edge_id: selectedEdge.edge_id,
                              top_edge_id: null,
                              positions_t: [0.2, 0.5, 0.85],
                              width_mm: 11.0,
                              arch_mm: 5.5,
                            }
                          : null;
                        setSnapshot(
                          kernel.setEdgeHardware(
                            selectedPiece.id,
                            selectedEdge.edge_idx,
                            selectedEdge.zipper ?? null,
                            selectedEdge.button ?? null,
                            nextL,
                          ),
                        );
                      }}
                    />
                  </label>
                  {selectedEdge.belt_loop && (
                    <div className="flex items-center gap-2 text-[10px]">
                      <span className="shrink-0" style={{ color: 'var(--text-muted)' }}>
                        {t('inspector.beltLoopArch')}
                      </span>
                      <input
                        type="range"
                        min={2}
                        max={12}
                        step={0.5}
                        value={selectedEdge.belt_loop.arch_mm}
                        onChange={(e) => {
                          if (!selectedEdge.belt_loop) return;
                          setSnapshot(
                            kernel.setEdgeHardware(
                              selectedPiece.id,
                              selectedEdge.edge_idx,
                              selectedEdge.zipper ?? null,
                              selectedEdge.button ?? null,
                              { ...selectedEdge.belt_loop, arch_mm: Number(e.target.value) },
                            ),
                          );
                        }}
                        className="flex-1"
                      />
                      <span className="font-mono-num w-8 text-right">
                        {selectedEdge.belt_loop.arch_mm.toFixed(1)}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            )}

            <div>
              <h4 className="text-[10px] font-semibold uppercase tracking-wider mb-1.5" style={{ color: 'var(--text-secondary)' }}>
                {t('inspector.edgesOn', { code: selectedPiece.code })}
              </h4>
              <div className="space-y-1">
                {selectedPiece.edges.map((edge) => (
                  <button
                    key={edge.edge_id}
                    onClick={() => setSnapshot(kernel.setSelection(selectedPiece.id, edge.edge_idx))}
                    className="w-full text-left px-2 py-1 rounded border flex items-center justify-between text-[11px] cursor-pointer"
                    style={{
                      background:
                        selectedEdge?.edge_idx === edge.edge_idx ? 'var(--bg-surface)' : 'transparent',
                      borderColor:
                        selectedEdge?.edge_idx === edge.edge_idx
                          ? 'var(--accent)'
                          : 'var(--border-subtle)',
                    }}
                  >
                    <span className="truncate">
                      #{edge.edge_idx + 1} {edge.label}
                    </span>
                    <span className="font-mono-num text-[10px] shrink-0 ml-2" style={{ color: 'var(--text-secondary)' }}>
                      {formatMm(edge.length_mm, unit, 0)}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          </>
        )}

        {view === 'materials' && <FabricPanel snapshot={snapshot} />}

        {view === 'grading' && (
          <>
            <div className="space-y-2.5">
              <div>
                <label className="block text-[10px] font-semibold uppercase tracking-wider mb-1" style={{ color: 'var(--text-secondary)' }}>
                  {t('inspector.gradeMode')}
                </label>
                <select
                  value={snapshot.grade_mode}
                  onChange={(e) => setSnapshot(kernel.setGradeMode(e.target.value as GradeMode))}
                  className="w-full rounded border px-2 py-1.5 text-[11px]"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="parametric_mtm">{t('inspector.gradeMtm')}</option>
                  <option value="rule_based_rtw">{t('inspector.gradeRtw')}</option>
                  <option value="hybrid_differential">{t('inspector.gradeHybrid')}</option>
                </select>
              </div>

              <div>
                <label className="block text-[10px] font-semibold uppercase tracking-wider mb-1" style={{ color: 'var(--text-secondary)' }}>
                  {t('inspector.stackAnchor')}
                </label>
                <select
                  value={snapshot.stack_anchor}
                  onChange={(e) => setSnapshot(kernel.setStackAnchor(e.target.value as StackAnchor))}
                  className="w-full rounded border px-2 py-1.5 text-[11px]"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="grainline_midpoint">{t('inspector.anchorGrain')}</option>
                  <option value="bust_or_waist_center">{t('inspector.anchorBust')}</option>
                  <option value="top_left_anchor">{t('inspector.anchorTopLeft')}</option>
                </select>
              </div>

              <div>
                <label className="block text-[10px] font-semibold uppercase tracking-wider mb-1.5" style={{ color: 'var(--text-secondary)' }}>
                  {t('inspector.baseNest')}
                </label>
                <div className="grid grid-cols-3 gap-1.5">
                  {snapshot.sizes.map((sz) => {
                    const isBase = snapshot.active_size === sz;
                    const isVis = snapshot.visible_sizes.includes(sz);
                    return (
                      <div
                        key={sz}
                        className="rounded border p-1.5 flex items-center justify-between text-[11px] font-mono-num"
                        style={{
                          background: isBase ? 'var(--accent-soft)' : 'var(--bg-surface)',
                          borderColor: isBase ? 'var(--accent)' : 'var(--border-subtle)',
                        }}
                      >
                        <button
                          onClick={() => setSnapshot(kernel.setActiveSize(sz))}
                          className="font-semibold cursor-pointer"
                        >
                          {sz}
                        </button>
                        <input
                          type="checkbox"
                          checked={isVis}
                          disabled={isBase}
                          onChange={() => toggleVisibleSize(sz)}
                        />
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>

            <div className="pt-2.5 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
              <h3 className="text-[10px] font-semibold uppercase tracking-wider mb-2" style={{ color: 'var(--text-secondary)' }}>
                {t('inspector.qaMatrix')}
              </h3>
              <div className="space-y-2">
                {snapshot.qa_matrix.map((row) => (
                  <div
                    key={row.link_id}
                    className="rounded border p-2"
                    style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                  >
                    <div className="font-medium text-[11px] mb-1 truncate">{row.label}</div>
                    <div className="grid grid-cols-3 gap-1">
                      {row.cells.map((cell) => {
                        const cls =
                          cell.status === 'ok'
                            ? 'bg-emerald-500/12 text-emerald-700 dark:text-emerald-400'
                            : cell.status === 'tight'
                              ? 'bg-rose-500/15 text-rose-700 dark:text-rose-400'
                              : 'bg-amber-500/15 text-amber-700 dark:text-amber-400';
                        return (
                          <div
                            key={cell.size_name}
                            className={`px-1.5 py-0.5 rounded text-[10px] font-mono-num flex items-center justify-between ${cls}`}
                          >
                            <span>{cell.size_name}</span>
                            <span>{cell.ease_mm >= 0 ? '+' : ''}{cell.ease_mm.toFixed(1)}</span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </>
        )}

        {view === 'params' && (
          <div id="params-formulas" className="space-y-2.5 pt-3 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
            <h3 className="text-[10px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>
              {t('panel.formulas')}
            </h3>
            <p className="text-[10px] leading-relaxed" style={{ color: 'var(--text-secondary)' }}>
              {t('inspector.dagHelp')} {t('inspector.fabricVarsHelp')}
            </p>
            <div className="space-y-1.5">
              {snapshot.variables.map((v) => (
                <div
                  key={v.name}
                  className="p-2 rounded border space-y-1"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-mono-num font-semibold text-[11px]">{v.name}</span>
                    <div className="flex items-center gap-1">
                      {v.error ? (
                        <span className="text-[10px] text-rose-500 font-mono-num">{t('common.error')}</span>
                      ) : (
                        <span className="text-[10px] font-mono-num text-emerald-600 dark:text-emerald-400 flex items-center gap-0.5">
                          <CheckCircle2 className="w-3 h-3" />
                          {v.value !== null ? formatMm(v.value, unit, 1) : '—'}
                        </span>
                      )}
                      {isFabricVar(v.name) ? (
                        <span className="text-[9px] px-1 rounded" style={{ background: 'var(--bg-panel)', color: 'var(--text-muted)' }}>
                          {t('inspector.readOnly')}
                        </span>
                      ) : (
                        <button
                          onClick={() => setSnapshot(kernel.deleteVariable(v.name))}
                          className="p-0.5 hover:text-rose-500 cursor-pointer"
                          title={t('common.delete')}
                        >
                          <Trash2 className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                  </div>
                  {isFabricVar(v.name) ? (
                    <div className="text-[10px] font-mono-num px-1.5 py-1" style={{ color: 'var(--text-muted)' }}>
                      = {v.expr}
                    </div>
                  ) : (
                  <ExpressionInput
                    mode="formula"
                    valueText={v.expr}
                    unit={unit}
                    onCommitFormula={(expr) => {
                      if (expr.trim() && expr !== v.expr) {
                        setSnapshot(kernel.setVariable(v.name, expr.trim(), v.description));
                      }
                    }}
                    className="w-full text-[10px] rounded border px-1.5 py-1"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  />
                  )}
                  {v.error && <div className="text-[10px] text-rose-500">{v.error}</div>}
                </div>
              ))}
            </div>

            <div className="p-2 rounded border space-y-1.5" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
              <div className="text-[10px] font-semibold">{t('inspector.addVariable')}</div>
              <input
                type="text"
                placeholder={t('inspector.varNamePlaceholder')}
                value={newVarName}
                onChange={(e) => setNewVarName(e.target.value)}
                className="w-full font-mono-num text-[10px] rounded border px-1.5 py-1"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              />
              <ExpressionInput
                mode="formula"
                valueText={newVarExpr}
                unit={unit}
                placeholder={t('inspector.varExprPlaceholder')}
                onCommitFormula={(expr) => setNewVarExpr(expr)}
                className="w-full text-[10px] rounded border px-1.5 py-1"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              />
              <button
                onClick={() => {
                  if (!newVarName.trim() || !newVarExpr.trim()) return;
                  setSnapshot(kernel.setVariable(newVarName.trim(), newVarExpr.trim()));
                  setNewVarName('');
                  setNewVarExpr('');
                }}
                className="w-full py-1 rounded font-medium text-white flex items-center justify-center gap-1 cursor-pointer text-[11px]"
                style={{ background: 'var(--accent)' }}
              >
                <Plus className="w-3 h-3" />
                {t('inspector.addFormula')}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
