import React, { useState } from 'react';
import { AlertTriangle, CheckCircle2, Plus, Trash2 } from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import {
  displayToMm,
  formatMm,
  mmToDisplay,
  type CornerRule,
  type GradeMode,
  type StackAnchor,
} from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

export const RightInspector: React.FC = () => {
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const activeTab = useStudioStore((s) => s.activeTab);
  const setActiveTab = useStudioStore((s) => s.setActiveTab);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);

  const [newVarName, setNewVarName] = useState('');
  const [newVarExpr, setNewVarExpr] = useState('');

  if (!snapshot) return null;

  const selectedPiece =
    snapshot.pieces.find((p) => p.id === snapshot.selected_piece_id) ?? snapshot.pieces[0];
  const selectedEdge =
    selectedPiece?.edges.find((e) => e.edge_idx === snapshot.selected_edge_idx) ??
    selectedPiece?.edges[0];

  const handleRuleChange = (edgeIdx: number, widthMm: number, kind: CornerRule['kind']) => {
    if (!selectedPiece) return;
    const rule: CornerRule =
      kind === 'miter' ? { kind: 'miter', limit: 2.5 } : { kind };
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

  return (
    <aside
      className="w-80 shrink-0 border-l flex flex-col overflow-hidden select-none"
      style={{
        background: 'var(--bg-panel)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {/* Tab Header */}
      <div
        className="grid grid-cols-4 border-b text-[11px] font-medium"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
      >
        {(
          [
            ['measurements', 'Body'],
            ['piece', 'Piece & SA'],
            ['grading', 'Grade QA'],
            ['formulas', 'DAG'],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            onClick={() => setActiveTab(id)}
            className="py-2.5 border-b-2 transition cursor-pointer"
            style={{
              borderColor: activeTab === id ? 'var(--accent)' : 'transparent',
              color: activeTab === id ? 'var(--accent)' : 'var(--text-secondary)',
            }}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Tab Body */}
      <div className="flex-1 overflow-y-auto p-3.5 space-y-4 text-xs">
        {activeTab === 'measurements' && (
          <>
            <div>
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-[11px] font-semibold uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>
                  Body Measurements ({unit})
                </h3>
                <span className="font-mono-num text-[11px]" style={{ color: 'var(--accent)' }}>
                  Base {snapshot.active_size}
                </span>
              </div>
              <div className="space-y-2.5">
                {snapshot.measurements.map((m) => (
                  <div key={m.key} className="space-y-1">
                    <div className="flex items-center justify-between">
                      <span>{m.label}</span>
                      <input
                        type="number"
                        step={unit === 'mm' ? 1 : 0.1}
                        value={Number(mmToDisplay(m.value_mm, unit).toFixed(unit === 'mm' ? 0 : 1))}
                        onChange={(e) => {
                          const v = parseFloat(e.target.value);
                          if (!Number.isNaN(v)) {
                            setSnapshot(kernel.setMeasurement(m.key, displayToMm(v, unit)));
                          }
                        }}
                        className="w-18 text-right font-mono-num rounded border px-1.5 py-0.5 text-[11px]"
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
                        setSnapshot(kernel.setMeasurement(m.key, parseFloat(e.target.value)))
                      }
                      className="w-full h-1.5 cursor-pointer"
                    />
                  </div>
                ))}
              </div>
            </div>

            <div className="pt-3 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
              <h3 className="text-[11px] font-semibold uppercase tracking-wider mb-2" style={{ color: 'var(--text-secondary)' }}>
                Garment Ease Table ({unit})
              </h3>
              <div className="space-y-2.5">
                {snapshot.ease.map((m) => (
                  <div key={m.key} className="space-y-1">
                    <div className="flex items-center justify-between">
                      <span>{m.label}</span>
                      <span className="font-mono-num text-[11px]" style={{ color: 'var(--text-secondary)' }}>
                        {formatMm(m.value_mm, unit, 1)}
                      </span>
                    </div>
                    <input
                      type="range"
                      min={m.min_mm}
                      max={m.max_mm}
                      step={1}
                      value={m.value_mm}
                      onChange={(e) =>
                        setSnapshot(kernel.setEase(m.key, parseFloat(e.target.value)))
                      }
                      className="w-full h-1.5 cursor-pointer"
                    />
                  </div>
                ))}
              </div>
            </div>
          </>
        )}

        {activeTab === 'piece' && selectedPiece && (
          <>
            <div className="p-2.5 rounded-lg border" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
              <div className="flex items-center justify-between">
                <span className="font-semibold">{selectedPiece.name}</span>
                <span className="font-mono-num text-[11px]" style={{ color: 'var(--text-muted)' }}>
                  {selectedPiece.code}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-2 mt-2 text-[11px] font-mono-num" style={{ color: 'var(--text-secondary)' }}>
                <div>Net Area: {(selectedPiece.area_mm2 / 100).toFixed(1)} cm²</div>
                <div>Cut Area: {(selectedPiece.cut_area_mm2 / 100).toFixed(1)} cm²</div>
              </div>
              {selectedPiece.warnings.length > 0 && (
                <div className="mt-2 p-2 rounded bg-amber-500/15 text-amber-700 dark:text-amber-300 text-[11px] space-y-1">
                  {selectedPiece.warnings.map((w, i) => (
                    <div key={i} className="flex items-start gap-1.5">
                      <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
                      <span>{w}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {selectedEdge && (
              <div className="p-3 rounded-lg border space-y-3" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
                <div className="flex items-center justify-between">
                  <span className="font-semibold">
                    Edge #{selectedEdge.edge_idx + 1}: {selectedEdge.label}
                  </span>
                  <span className="font-mono-num text-[11px]">
                    {formatMm(selectedEdge.length_mm, unit, 1)}
                  </span>
                </div>

                <div>
                  <label className="block text-[11px] mb-1" style={{ color: 'var(--text-secondary)' }}>
                    Seam Allowance Width (mm)
                  </label>
                  <div className="flex flex-wrap gap-1 mb-2">
                    {[0, 6, 10, 15, 30].map((w) => (
                      <button
                        key={w}
                        onClick={() =>
                          handleRuleChange(selectedEdge.edge_idx, w, selectedEdge.corner_rule.kind)
                        }
                        className="px-2 py-0.5 rounded border font-mono-num text-[11px] cursor-pointer"
                        style={{
                          borderColor:
                            Math.abs(selectedEdge.sa_width_mm - w) < 0.2
                              ? 'var(--accent)'
                              : 'var(--border-subtle)',
                          background:
                            Math.abs(selectedEdge.sa_width_mm - w) < 0.2
                              ? 'var(--accent-soft)'
                              : 'transparent',
                        }}
                      >
                        {w}
                      </button>
                    ))}
                  </div>
                  <input
                    type="number"
                    min={0}
                    max={80}
                    step={1}
                    value={selectedEdge.sa_width_mm}
                    onChange={(e) => {
                      const val = parseFloat(e.target.value);
                      if (!Number.isNaN(val)) {
                        handleRuleChange(
                          selectedEdge.edge_idx,
                          val,
                          selectedEdge.corner_rule.kind,
                        );
                      }
                    }}
                    className="w-full rounded border px-2 py-1 font-mono-num"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  />
                </div>

                <div>
                  <label className="block text-[11px] mb-1" style={{ color: 'var(--text-secondary)' }}>
                    Apparel Corner Resolution
                  </label>
                  <select
                    value={selectedEdge.corner_rule.kind}
                    onChange={(e) =>
                      handleRuleChange(
                        selectedEdge.edge_idx,
                        selectedEdge.sa_width_mm,
                        e.target.value as CornerRule['kind'],
                      )
                    }
                    className="w-full rounded border px-2 py-1"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  >
                    <option value="miter">Miter (Auto-Bevel Limit)</option>
                    <option value="folded_hem">Folded Hem Flare</option>
                    <option value="matched_seam">Matched Partner Seam</option>
                    <option value="square">Square Cap</option>
                    <option value="bevel">Bevel</option>
                    <option value="dart_cap">Dart Cap Reflection</option>
                  </select>
                </div>
              </div>
            )}

            <div>
              <h4 className="text-[11px] font-semibold uppercase tracking-wider mb-1.5" style={{ color: 'var(--text-secondary)' }}>
                All Edges on {selectedPiece.code}
              </h4>
              <div className="space-y-1">
                {selectedPiece.edges.map((edge) => (
                  <button
                    key={edge.edge_idx}
                    onClick={() => setSnapshot(kernel.setSelection(selectedPiece.id, edge.edge_idx))}
                    className="w-full text-left px-2.5 py-1.5 rounded border flex items-center justify-between text-[11px] cursor-pointer"
                    style={{
                      background:
                        selectedEdge?.edge_idx === edge.edge_idx
                          ? 'var(--bg-surface)'
                          : 'transparent',
                      borderColor:
                        selectedEdge?.edge_idx === edge.edge_idx
                          ? 'var(--accent)'
                          : 'var(--border-subtle)',
                    }}
                  >
                    <span className="truncate">
                      #{edge.edge_idx + 1} {edge.label}
                    </span>
                    <span className="font-mono-num shrink-0 ml-2" style={{ color: 'var(--text-secondary)' }}>
                      {formatMm(edge.length_mm, unit, 0)} · SA {edge.sa_width_mm.toFixed(0)}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          </>
        )}

        {activeTab === 'grading' && (
          <>
            <div className="space-y-2.5">
              <div>
                <label className="block text-[11px] font-semibold uppercase tracking-wider mb-1" style={{ color: 'var(--text-secondary)' }}>
                  Grading Engine Mode
                </label>
                <select
                  value={snapshot.grade_mode}
                  onChange={(e) => setSnapshot(kernel.setGradeMode(e.target.value as GradeMode))}
                  className="w-full rounded border px-2 py-1.5"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="parametric_mtm">Parametric MTM (Full DAG Scale)</option>
                  <option value="rule_based_rtw">Rule-Based RTW (Proportional Handles)</option>
                  <option value="hybrid_differential">Hybrid (Parametric + Overrides)</option>
                </select>
              </div>

              <div>
                <label className="block text-[11px] font-semibold uppercase tracking-wider mb-1" style={{ color: 'var(--text-secondary)' }}>
                  Nest Stacking Anchor
                </label>
                <select
                  value={snapshot.stack_anchor}
                  onChange={(e) => setSnapshot(kernel.setStackAnchor(e.target.value as StackAnchor))}
                  className="w-full rounded border px-2 py-1.5"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                >
                  <option value="grainline_midpoint">Grainline Midpoint</option>
                  <option value="bust_or_waist_center">Bust / Dart Apex</option>
                  <option value="top_left_anchor">Top-Left Corner</option>
                </select>
              </div>

              <div>
                <label className="block text-[11px] font-semibold uppercase tracking-wider mb-1.5" style={{ color: 'var(--text-secondary)' }}>
                  Active Size &amp; Visible Nest
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
                          title={`Make ${sz} base size`}
                        >
                          {sz}
                        </button>
                        <input
                          type="checkbox"
                          checked={isVis}
                          disabled={isBase}
                          onChange={() => toggleVisibleSize(sz)}
                          title={`Toggle ${sz} in graded nest`}
                        />
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>

            <div className="pt-3 border-t" style={{ borderColor: 'var(--border-subtle)' }}>
              <h3 className="text-[11px] font-semibold uppercase tracking-wider mb-2" style={{ color: 'var(--text-secondary)' }}>
                Cross-Size Seam Ease QA Matrix
              </h3>
              <div className="space-y-2">
                {snapshot.qa_matrix.map((row) => (
                  <div
                    key={row.link_id}
                    className="rounded-lg border p-2"
                    style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                  >
                    <div className="font-medium text-[11px] mb-1.5">{row.label}</div>
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
                            className={`px-1.5 py-1 rounded text-[10px] font-mono-num flex items-center justify-between ${cls}`}
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

        {activeTab === 'formulas' && (
          <div className="space-y-3">
            <p className="text-[11px] leading-relaxed" style={{ color: 'var(--text-secondary)' }}>
              Evaluated topologically via Rust Kahn&apos;s DAG. Supports arithmetic, <code className="font-mono-num">min/max/clamp/sqrt/sin/cos</code>, and body/ease tokens.
            </p>
            <div className="space-y-2">
              {snapshot.variables.map((v) => (
                <div
                  key={v.name}
                  className="p-2.5 rounded-lg border space-y-1.5"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-mono-num font-semibold text-[11px]">{v.name}</span>
                    <div className="flex items-center gap-1.5">
                      {v.error ? (
                        <span className="text-[10px] text-rose-500 font-mono-num">Error</span>
                      ) : (
                        <span className="text-[11px] font-mono-num text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                          <CheckCircle2 className="w-3 h-3" />
                          {v.value !== null ? formatMm(v.value, unit, 1) : '—'}
                        </span>
                      )}
                      <button
                        onClick={() => setSnapshot(kernel.deleteVariable(v.name))}
                        className="p-0.5 hover:text-rose-500 cursor-pointer"
                        title="Delete variable"
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                  </div>
                  <input
                    type="text"
                    defaultValue={v.expr}
                    onBlur={(e) => {
                      if (e.target.value !== v.expr) {
                        setSnapshot(kernel.setVariable(v.name, e.target.value, v.description));
                      }
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        setSnapshot(
                          kernel.setVariable(v.name, (e.target as HTMLInputElement).value, v.description),
                        );
                      }
                    }}
                    className="w-full font-mono-num text-[11px] rounded border px-2 py-1"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  />
                  {v.error && <div className="text-[10px] text-rose-500">{v.error}</div>}
                </div>
              ))}
            </div>

            <div className="p-2.5 rounded-lg border space-y-2" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}>
              <div className="text-[11px] font-semibold">Add Parametric Variable</div>
              <input
                type="text"
                placeholder="name (e.g. hem_sweep)"
                value={newVarName}
                onChange={(e) => setNewVarName(e.target.value)}
                className="w-full font-mono-num text-[11px] rounded border px-2 py-1"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              />
              <input
                type="text"
                placeholder="expr (e.g. (hip + ease_hip) / 2)"
                value={newVarExpr}
                onChange={(e) => setNewVarExpr(e.target.value)}
                className="w-full font-mono-num text-[11px] rounded border px-2 py-1"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
              />
              <button
                onClick={() => {
                  if (!newVarName.trim() || !newVarExpr.trim()) return;
                  setSnapshot(kernel.setVariable(newVarName.trim(), newVarExpr.trim()));
                  setNewVarName('');
                  setNewVarExpr('');
                }}
                className="w-full py-1.5 rounded font-medium text-white flex items-center justify-center gap-1 cursor-pointer"
                style={{ background: 'var(--accent)' }}
              >
                <Plus className="w-3.5 h-3.5" />
                Add DAG Formula
              </button>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
};
