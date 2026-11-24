import React, { useEffect, useRef, useState } from 'react';
import {
  FileCode2,
  FileSpreadsheet,
  FileText,
  LayoutGrid,
  Projector,
  Upload,
  X,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { downloadBytesFile, downloadTextFile, openTextFileFromDisk } from '../../lib/localFiles';
import type { MarkerPlan } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';
import { useT } from '../../lib/i18n';

export const ExportModal: React.FC = () => {
  const t = useT();
  const open = useStudioStore((s) => s.exportModalOpen);
  const snapshot = useStudioStore((s) => s.snapshot);
  const setOpen = useStudioStore((s) => s.setExportModalOpen);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);
  const requestFitView = useStudioStore((s) => s.requestFitView);

  const [tab, setTab] = useState<'exports' | 'marker'>('exports');
  const [pdfPreset, setPdfPreset] = useState<'a4_tiled' | 'letter_tiled' | 'a0' | 'projector'>('a4_tiled');
  const [unfoldOnFold, setUnfoldOnFold] = useState(true);
  const [darkProjector, setDarkProjector] = useState(true);
  const [showGrid, setShowGrid] = useState(true);
  const [busy, setBusy] = useState(false);

  // Stage 10 Marker Nesting & BOM Costing state
  const [rollWidthMm, setRollWidthMm] = useState(1450);
  const [kerfMm, setKerfMm] = useState(4);
  const [costPerMeter, setCostPerMeter] = useState(16);
  const [bundleQty, setBundleQty] = useState<Record<string, number>>({ S: 0, M: 1, L: 0 });
  const [markerPlan, setMarkerPlan] = useState<MarkerPlan | null>(null);
  const markerCanvasRef = useRef<HTMLCanvasElement | null>(null);

  const runMarkerNesting = () => {
    if (!snapshot) return;
    const bundle = Object.entries(bundleQty)
      .filter(([, q]) => q > 0)
      .map(([size, quantity]) => ({ size, quantity }));
    if (bundle.length === 0) {
      bundle.push({ size: snapshot.active_size, quantity: 1 });
    }
    const plan = kernel.computeMarkerPlan({
      roll_width_mm: rollWidthMm,
      kerf_mm: kerfMm,
      cost_per_meter: costPerMeter,
      bundle,
    });
    setMarkerPlan(plan);
  };

  useEffect(() => {
    if (open && tab === 'marker') {
      runMarkerNesting();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, tab]);

  useEffect(() => {
    const canvas = markerCanvasRef.current;
    if (!canvas || !markerPlan) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = Math.max(1, Math.floor(rect.width * dpr));
    canvas.height = Math.max(1, Math.floor(rect.height * dpr));
    ctx.save();
    ctx.scale(dpr, dpr);

    const w = rect.width;
    const h = rect.height;
    ctx.fillStyle = '#f8fafc';
    ctx.fillRect(0, 0, w, h);

    const pad = 18;
    const rollLen = Math.max(markerPlan.used_length_mm, 400);
    const scale = Math.min((w - 2 * pad) / rollLen, (h - 2 * pad) / markerPlan.roll_width_mm);
    const ox = pad;
    const oy = pad;
    const rwPx = rollLen * scale;
    const rhPx = markerPlan.roll_width_mm * scale;

    ctx.fillStyle = '#ffffff';
    ctx.fillRect(ox, oy, rwPx, rhPx);
    ctx.strokeStyle = '#64748b';
    ctx.setLineDash([5, 4]);
    ctx.strokeRect(ox, oy, rwPx, rhPx);
    ctx.setLineDash([]);

    const colors = [
      'rgba(37, 99, 235, 0.16)',
      'rgba(14, 159, 110, 0.16)',
      'rgba(217, 119, 6, 0.16)',
      'rgba(124, 58, 237, 0.16)',
    ];
    markerPlan.placements.forEach((pl, i) => {
      if (pl.polygon.length < 3) return;
      ctx.beginPath();
      pl.polygon.forEach((pt, pi) => {
        const px = ox + pt.x * scale;
        const py = oy + (markerPlan.roll_width_mm - pt.y) * scale;
        if (pi === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      });
      ctx.closePath();
      ctx.fillStyle = colors[i % colors.length];
      ctx.fill();
      ctx.strokeStyle = '#1e293b';
      ctx.lineWidth = 1.1;
      ctx.stroke();

      const lx = ox + pl.label_pos.x * scale;
      const ly = oy + (markerPlan.roll_width_mm - pl.label_pos.y) * scale;
      ctx.fillStyle = '#0f172a';
      ctx.font = '600 9px "JetBrains Mono", monospace';
      ctx.textAlign = 'center';
      ctx.fillText(`${pl.piece_code} ${pl.size_name}${pl.mirrored ? ' (R)' : ''}`, lx, ly);
    });

    ctx.restore();
  }, [markerPlan]);

  if (!open || !snapshot) return null;

  const baseName = snapshot.project_name.replace(/[^a-zA-Z0-9_-]+/g, '_').toLowerCase() || 'pattern';

  const handleExportPdf = async () => {
    setBusy(true);
    try {
      const [{ generatePdfBytes }, plan] = await Promise.all([
        import('../../lib/pdfExporter'),
        Promise.resolve(
          kernel.getPdfLayoutPlan(pdfPreset, pdfPreset === 'projector' ? unfoldOnFold : false),
        ),
      ]);
      const bytes = await generatePdfBytes(plan, {
        preset: pdfPreset,
        unfoldOnFold,
        darkProjector,
        showGrid,
        projectName: snapshot.project_name,
      });
      downloadBytesFile(`${baseName}_${pdfPreset}.pdf`, bytes);
      setStatusMessage(t('export.pdfDone', { preset: pdfPreset.toUpperCase(), sheets: plan.tiles.length }));
    } finally {
      setBusy(false);
    }
  };

  const handleExportDxf = () => {
    downloadTextFile(`${baseName}_astm_r12.dxf`, kernel.exportDxfAama(), 'application/dxf');
    setStatusMessage(t('export.dxfDone'));
  };

  const handleExportRul = () => {
    downloadTextFile(`${baseName}_aama.rul`, kernel.exportAamaRul(), 'text/plain');
    setStatusMessage(t('export.rulDone'));
  };

  const handleImportDxf = async () => {
    const file = await openTextFileFromDisk('.dxf,.DXF');
    if (!file) return;
    try {
      const { message, snapshot: next } = kernel.importDxfAama(file.text);
      setSnapshot(next);
      requestFitView();
      setStatusMessage(message);
      setOpen(false);
    } catch (e) {
      setStatusMessage(t('export.dxfImportError', { error: (e as Error).message }));
    }
  };

  const handleImportGarmentCode = async () => {
    const file = await openTextFileFromDisk('.json');
    if (!file) return;
    try {
      const { message, snapshot: next } = kernel.importGarmentCodeJson(file.text);
      setSnapshot(next);
      requestFitView();
      setStatusMessage(message);
      setOpen(false);
    } catch (e) {
      setStatusMessage(t('export.gcImportError', { error: (e as Error).message }));
    }
  };

  const handleExportMarkerSvg = () => {
    const bundle = Object.entries(bundleQty)
      .filter(([, q]) => q > 0)
      .map(([size, quantity]) => ({ size, quantity }));
    const svg = kernel.exportMarkerSvg({
      roll_width_mm: rollWidthMm,
      kerf_mm: kerfMm,
      cost_per_meter: costPerMeter,
      bundle: bundle.length ? bundle : [{ size: snapshot.active_size, quantity: 1 }],
    });
    downloadTextFile(`${baseName}_marker.svg`, svg, 'image/svg+xml');
    setStatusMessage(t('export.markerDone'));
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/45 backdrop-blur-xs flex items-center justify-center p-4">
      <div
        className="w-full max-w-4xl rounded-2xl border shadow-2xl overflow-hidden flex flex-col max-h-[90vh]"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
      >
        <div className="px-5 py-3 border-b flex items-center justify-between" style={{ borderColor: 'var(--border-subtle)' }}>
          <div className="flex items-center gap-4">
            <h2 className="text-base font-semibold">{t('export.title')}</h2>
            <div className="flex items-center gap-1 text-xs">
              <button
                onClick={() => setTab('exports')}
                className="px-3 py-1 rounded-md font-medium cursor-pointer"
                style={{
                  background: tab === 'exports' ? 'var(--accent-soft)' : 'transparent',
                  color: tab === 'exports' ? 'var(--accent)' : 'var(--text-secondary)',
                }}
              >
                PDF · DXF/RUL · GarmentCode
              </button>
              <button
                onClick={() => setTab('marker')}
                className="px-3 py-1 rounded-md font-medium inline-flex items-center gap-1.5 cursor-pointer"
                style={{
                  background: tab === 'marker' ? 'var(--accent-soft)' : 'transparent',
                  color: tab === 'marker' ? 'var(--accent)' : 'var(--text-secondary)',
                }}
              >
                <LayoutGrid className="w-3.5 h-3.5" />
                {t('export.tabMarker')}
              </button>
            </div>
          </div>
          <button onClick={() => setOpen(false)} className="p-1.5 rounded-lg hover:bg-[var(--bg-subtle)] cursor-pointer">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-5 text-xs">
          {tab === 'marker' ? (
            <div className="space-y-4">
              <div className="grid sm:grid-cols-4 gap-3 p-3.5 rounded-xl border" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                <label className="space-y-1">
                  <span className="block text-[11px]" style={{ color: 'var(--text-secondary)' }}>{t('export.rollWidth')}</span>
                  <input
                    type="number"
                    value={rollWidthMm}
                    onChange={(e) => setRollWidthMm(Number(e.target.value))}
                    className="w-full rounded border px-2 py-1 font-mono-num"
                    style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                  />
                </label>
                <label className="space-y-1">
                  <span className="block text-[11px]" style={{ color: 'var(--text-secondary)' }}>{t('export.kerf')}</span>
                  <input
                    type="number"
                    value={kerfMm}
                    onChange={(e) => setKerfMm(Number(e.target.value))}
                    className="w-full rounded border px-2 py-1 font-mono-num"
                    style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                  />
                </label>
                <label className="space-y-1">
                  <span className="block text-[11px]" style={{ color: 'var(--text-secondary)' }}>{t('export.cost')}</span>
                  <input
                    type="number"
                    value={costPerMeter}
                    onChange={(e) => setCostPerMeter(Number(e.target.value))}
                    className="w-full rounded border px-2 py-1 font-mono-num"
                    style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                  />
                </label>
                <div className="flex items-end">
                  <button
                    onClick={runMarkerNesting}
                    className="w-full py-1.5 rounded-lg font-semibold text-white cursor-pointer"
                    style={{ background: 'var(--accent)' }}
                  >
                    {t('export.renest')}
                  </button>
                </div>
              </div>

              {/* Multi-size cut bundle quantities */}
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-semibold text-[11px]" style={{ color: 'var(--text-secondary)' }}>{t('export.bundle')}</span>
                {snapshot.sizes.map((sz) => (
                  <label
                    key={sz}
                    className="inline-flex items-center gap-1.5 px-2 py-1 rounded border font-mono-num"
                    style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                  >
                    <span>{sz}</span>
                    <input
                      type="number"
                      min={0}
                      max={6}
                      value={bundleQty[sz] ?? 0}
                      onChange={(e) =>
                        setBundleQty((prev) => ({ ...prev, [sz]: Number(e.target.value) }))
                      }
                      className="w-10 text-right rounded border px-1 py-0.5"
                      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)' }}
                    />
                  </label>
                ))}
              </div>

              {markerPlan && (
                <>
                  <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 font-mono-num">
                    <div className="p-3 rounded-xl border" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                      <div className="text-[10px] font-sans" style={{ color: 'var(--text-muted)' }}>{t('export.yield')}</div>
                      <div className="text-base font-bold mt-0.5">{(markerPlan.used_length_mm / 1000).toFixed(2)} m</div>
                    </div>
                    <div className="p-3 rounded-xl border" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                      <div className="text-[10px] font-sans" style={{ color: 'var(--text-muted)' }}>{t('export.efficiency')}</div>
                      <div className="text-base font-bold mt-0.5 text-emerald-600">{markerPlan.efficiency_pct.toFixed(1)}%</div>
                    </div>
                    <div className="p-3 rounded-xl border" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                      <div className="text-[10px] font-sans" style={{ color: 'var(--text-muted)' }}>{t('export.waste')}</div>
                      <div className="text-base font-bold mt-0.5 text-amber-600">{markerPlan.waste_pct.toFixed(1)}%</div>
                    </div>
                    <div className="p-3 rounded-xl border" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                      <div className="text-[10px] font-sans" style={{ color: 'var(--text-muted)' }}>{t('export.estCost', { count: markerPlan.piece_count })}</div>
                      <div className="text-base font-bold mt-0.5">{markerPlan.estimated_cost.toFixed(2)}</div>
                    </div>
                  </div>

                  <div className="rounded-xl border overflow-hidden" style={{ borderColor: 'var(--border-strong)' }}>
                    <canvas ref={markerCanvasRef} className="w-full h-64 block" />
                  </div>

                  <div className="flex justify-end">
                    <button
                      onClick={handleExportMarkerSvg}
                      className="px-4 py-2 rounded-lg font-semibold text-white cursor-pointer"
                      style={{ background: 'var(--accent)' }}
                    >
                      {t('export.downloadMarker')}
                    </button>
                  </div>
                </>
              )}
            </div>
          ) : (
            <>
              {/* PDF Section */}
              <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                <div className="flex items-center justify-between">
                  <div className="font-semibold text-sm flex items-center gap-2">
                    <FileText className="w-4 h-4 text-blue-600" />
                    {t('export.pdfTitle')}
                  </div>
                  <span className="font-mono-num text-[11px]" style={{ color: 'var(--text-muted)' }}>
                    {t('export.pdfSubtitle')}
                  </span>
                </div>

                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                  {(
                    [
                      ['a4_tiled', t('export.a4')],
                      ['letter_tiled', t('export.letter')],
                      ['a0', t('export.a0')],
                      ['projector', t('export.projector')],
                    ] as const
                  ).map(([id, label]) => (
                    <button
                      key={id}
                      onClick={() => setPdfPreset(id)}
                      className="px-3 py-2 rounded-lg border font-medium text-left cursor-pointer"
                      style={{
                        background: pdfPreset === id ? 'var(--accent-soft)' : 'var(--bg-surface)',
                        borderColor: pdfPreset === id ? 'var(--accent)' : 'var(--border-subtle)',
                        color: pdfPreset === id ? 'var(--accent)' : 'var(--text-primary)',
                      }}
                    >
                      {label}
                    </button>
                  ))}
                </div>

                {pdfPreset === 'projector' && (
                  <div className="flex flex-wrap items-center gap-4 pt-1">
                    <label className="flex items-center gap-1.5 cursor-pointer">
                      <input type="checkbox" checked={unfoldOnFold} onChange={(e) => setUnfoldOnFold(e.target.checked)} />
                      {t('export.unfold')}
                    </label>
                    <label className="flex items-center gap-1.5 cursor-pointer">
                      <input type="checkbox" checked={darkProjector} onChange={(e) => setDarkProjector(e.target.checked)} />
                      {t('export.dark')}
                    </label>
                    <label className="flex items-center gap-1.5 cursor-pointer">
                      <input type="checkbox" checked={showGrid} onChange={(e) => setShowGrid(e.target.checked)} />
                      50 mm calibration grid
                    </label>
                  </div>
                )}

                <button
                  onClick={handleExportPdf}
                  disabled={busy}
                  className="px-4 py-2 rounded-lg font-semibold text-white inline-flex items-center gap-2 cursor-pointer disabled:opacity-50"
                  style={{ background: 'var(--accent)' }}
                >
                  {pdfPreset === 'projector' ? <Projector className="w-4 h-4" /> : <FileText className="w-4 h-4" />}
                  {busy ? t('export.generating') : t('export.downloadPdf', { preset: pdfPreset.toUpperCase() })}
                </button>
              </div>

              {/* Industrial CAD + GarmentCode + SVG Grid */}
              <div className="grid sm:grid-cols-2 gap-4">
                <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                  <div className="font-semibold text-sm flex items-center gap-2">
                    <FileCode2 className="w-4 h-4 text-amber-600" />
                    {t('export.factory')}
                  </div>
                  <p style={{ color: 'var(--text-secondary)' }}>
                    Strict ASTM D6673-10 R12 export + ANSI/AAMA .rul grade table sidecar, plus DXF Schneider Bézier refit &amp; GarmentCode JSON import.
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <button
                      onClick={handleExportDxf}
                      className="px-3 py-1.5 rounded-lg border font-medium cursor-pointer"
                      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                    >
                      {t('export.exportDxf')}
                    </button>
                    <button
                      onClick={handleExportRul}
                      className="px-3 py-1.5 rounded-lg border font-medium cursor-pointer"
                      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                    >
                      {t('export.exportRul')}
                    </button>
                    <button
                      onClick={handleImportDxf}
                      className="px-3 py-1.5 rounded-lg border font-medium inline-flex items-center gap-1 cursor-pointer"
                      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                    >
                      <Upload className="w-3.5 h-3.5" />
                      {t('export.importDxf')}
                    </button>
                    <button
                      onClick={handleImportGarmentCode}
                      className="px-3 py-1.5 rounded-lg border font-medium inline-flex items-center gap-1 cursor-pointer"
                      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                    >
                      <Upload className="w-3.5 h-3.5" />
                      {t('export.importGc')}
                    </button>
                  </div>
                </div>

                <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                  <div className="font-semibold text-sm">{t('export.svgTitle')}</div>
                  <p style={{ color: 'var(--text-secondary)' }}>
                    {t('export.svgHelp')}
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <button
                      onClick={() => {
                        downloadTextFile(`${baseName}_${snapshot.active_size}.svg`, kernel.exportSvgMm(false), 'image/svg+xml');
                        setStatusMessage(t('export.svgDone'));
                      }}
                      className="px-3 py-1.5 rounded-lg border font-medium cursor-pointer"
                      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                    >
                      SVG ({snapshot.active_size})
                    </button>
                    <button
                      onClick={() => {
                        downloadTextFile(`${baseName}_nested.svg`, kernel.exportSvgMm(true), 'image/svg+xml');
                        setStatusMessage(t('export.nestDone'));
                      }}
                      className="px-3 py-1.5 rounded-lg border font-medium cursor-pointer"
                      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                    >
                      {t('export.svgNest')}
                    </button>
                  </div>
                </div>
              </div>

              {/* POM Spec Sheet Preview */}
              <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
                <div className="flex items-center justify-between">
                  <div className="font-semibold text-sm flex items-center gap-2">
                    <FileSpreadsheet className="w-4 h-4 text-emerald-600" />
                    {t('export.pomTitle')}
                  </div>
                  <button
                    onClick={() => {
                      downloadTextFile(`${baseName}_pom_specs.csv`, kernel.exportPomCsv(), 'text/csv');
                      setStatusMessage(t('export.pomDone'));
                    }}
                    className="px-3 py-1 rounded-lg border font-medium cursor-pointer"
                    style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                  >
                    {t('export.exportCsv')}
                  </button>
                </div>

                <div className="overflow-x-auto rounded-lg border" style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-surface)' }}>
                  <table className="w-full text-left font-mono-num text-[11px]">
                    <thead>
                      <tr className="border-b" style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}>
                        <th className="py-1.5 px-2.5 font-sans">{t('export.measurePoint')}</th>
                        {snapshot.sizes.map((s) => (
                          <th key={s} className="py-1.5 px-2 text-right">{s}</th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {snapshot.pom_rows.slice(0, 8).map((row) => (
                        <tr key={row.pom_name} className="border-b last:border-b-0" style={{ borderColor: 'var(--border-subtle)' }}>
                          <td className="py-1 px-2.5 font-sans truncate max-w-[200px]">{row.pom_name}</td>
                          {snapshot.sizes.map((s) => (
                            <td key={s} className="py-1 px-2 text-right">
                              {(row.values_mm[s] ?? 0).toFixed(1)}
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
