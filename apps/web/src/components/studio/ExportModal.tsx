import React, { useState } from 'react';
import { FileCode2, FileSpreadsheet, FileText, Projector, Upload, X } from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { downloadBytesFile, downloadTextFile, openTextFileFromDisk } from '../../lib/localFiles';
import { generatePdfBytes } from '../../lib/pdfExporter';
import { useStudioStore } from '../../store/studioStore';

export const ExportModal: React.FC = () => {
  const open = useStudioStore((s) => s.exportModalOpen);
  const snapshot = useStudioStore((s) => s.snapshot);
  const setOpen = useStudioStore((s) => s.setExportModalOpen);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);
  const requestFitView = useStudioStore((s) => s.requestFitView);

  const [pdfPreset, setPdfPreset] = useState<'a4_tiled' | 'letter_tiled' | 'a0' | 'projector'>('a4_tiled');
  const [unfoldOnFold, setUnfoldOnFold] = useState(true);
  const [darkProjector, setDarkProjector] = useState(true);
  const [showGrid, setShowGrid] = useState(true);
  const [busy, setBusy] = useState(false);

  if (!open || !snapshot) return null;

  const baseName = snapshot.project_name.replace(/[^a-zA-Z0-9_-]+/g, '_').toLowerCase() || 'pattern';

  const handleExportPdf = async () => {
    setBusy(true);
    try {
      const plan = kernel.getPdfLayoutPlan(
        pdfPreset,
        pdfPreset === 'projector' ? unfoldOnFold : false,
      );
      const bytes = await generatePdfBytes(plan, {
        preset: pdfPreset,
        unfoldOnFold,
        darkProjector,
        showGrid,
        projectName: snapshot.project_name,
      });
      downloadBytesFile(`${baseName}_${pdfPreset}.pdf`, bytes);
      setStatusMessage(`Exported ${pdfPreset.toUpperCase()} PDF (${plan.tiles.length} sheet(s))`);
    } finally {
      setBusy(false);
    }
  };

  const handleExportDxf = () => {
    const dxf = kernel.exportDxfAama();
    downloadTextFile(`${baseName}_astm_r12.dxf`, dxf, 'application/dxf');
    setStatusMessage('Exported strict DXF-AAMA / ASTM D6673-10 R12');
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
      setStatusMessage(`DXF import error: ${(e as Error).message}`);
    }
  };

  const handleExportSvg = (includeNest: boolean) => {
    const svg = kernel.exportSvgMm(includeNest);
    downloadTextFile(`${baseName}_${includeNest ? 'nested' : snapshot.active_size}.svg`, svg, 'image/svg+xml');
    setStatusMessage('Exported exact-millimeter SVG');
  };

  const handleExportPom = () => {
    const csv = kernel.exportPomCsv();
    downloadTextFile(`${baseName}_pom_specs.csv`, csv, 'text/csv');
    setStatusMessage('Exported POM tech-pack CSV');
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/45 backdrop-blur-xs flex items-center justify-center p-4">
      <div
        className="w-full max-w-3xl rounded-2xl border shadow-2xl overflow-hidden flex flex-col max-h-[90vh]"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
      >
        <div className="px-5 py-3.5 border-b flex items-center justify-between" style={{ borderColor: 'var(--border-subtle)' }}>
          <div>
            <h2 className="text-base font-semibold">Export &amp; Industrial Interop</h2>
            <p className="text-xs" style={{ color: 'var(--text-secondary)' }}>
              All geometry computed in Rust/WASM exact millimeters before serialization
            </p>
          </div>
          <button onClick={() => setOpen(false)} className="p-1.5 rounded-lg hover:bg-[var(--bg-subtle)] cursor-pointer">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-5 text-xs">
          {/* PDF Section */}
          <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
            <div className="flex items-center justify-between">
              <div className="font-semibold text-sm flex items-center gap-2">
                <FileText className="w-4 h-4 text-blue-600" />
                Indie &amp; Atelier PDF (Tiled, A0, Projector)
              </div>
              <span className="font-mono-num text-[11px]" style={{ color: 'var(--text-muted)' }}>
                1:1 True Scale + 100 mm Calibration
              </span>
            </div>

            <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
              {(
                [
                  ['a4_tiled', 'A4 Tiled + Map'],
                  ['letter_tiled', 'US Letter Tiled'],
                  ['a0', 'A0 Copyshop'],
                  ['projector', 'Projector Native'],
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
                  <input
                    type="checkbox"
                    checked={unfoldOnFold}
                    onChange={(e) => setUnfoldOnFold(e.target.checked)}
                  />
                  Auto-unfold Cut-on-Fold pieces
                </label>
                <label className="flex items-center gap-1.5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={darkProjector}
                    onChange={(e) => setDarkProjector(e.target.checked)}
                  />
                  Dark high-contrast mode
                </label>
                <label className="flex items-center gap-1.5 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={showGrid}
                    onChange={(e) => setShowGrid(e.target.checked)}
                  />
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
              {busy ? 'Generating PDF...' : `Download ${pdfPreset.toUpperCase()} PDF`}
            </button>
          </div>

          {/* Industrial CAD + SVG Grid */}
          <div className="grid sm:grid-cols-2 gap-4">
            <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
              <div className="font-semibold text-sm flex items-center gap-2">
                <FileCode2 className="w-4 h-4 text-amber-600" />
                DXF-AAMA / ASTM D6673-10 R12
              </div>
              <p style={{ color: 'var(--text-secondary)' }}>
                Strict AC1009 BLOCKS export (layers 1–15) compatible with Gerber, Lectra, Tukatech &amp; CLO, plus Schneider Bézier refitting on import.
              </p>
              <div className="flex flex-wrap gap-2">
                <button
                  onClick={handleExportDxf}
                  className="px-3 py-1.5 rounded-lg border font-medium cursor-pointer"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                >
                  Export DXF R12
                </button>
                <button
                  onClick={handleImportDxf}
                  className="px-3 py-1.5 rounded-lg border font-medium inline-flex items-center gap-1.5 cursor-pointer"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                >
                  <Upload className="w-3.5 h-3.5" />
                  Import DXF
                </button>
              </div>
            </div>

            <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
              <div className="font-semibold text-sm">Exact-mm Vector SVG</div>
              <p style={{ color: 'var(--text-secondary)' }}>
                Preserves true millimeter viewBox, per-size &lt;g&gt; layers, seam/cut/grain/notch hierarchy for laser cutters or web publishing.
              </p>
              <div className="flex flex-wrap gap-2">
                <button
                  onClick={() => handleExportSvg(false)}
                  className="px-3 py-1.5 rounded-lg border font-medium cursor-pointer"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                >
                  SVG ({snapshot.active_size})
                </button>
                <button
                  onClick={() => handleExportSvg(true)}
                  className="px-3 py-1.5 rounded-lg border font-medium cursor-pointer"
                  style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
                >
                  SVG (Graded Nest)
                </button>
              </div>
            </div>
          </div>

          {/* POM Spec Sheet Preview */}
          <div className="p-4 rounded-xl border space-y-3" style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}>
            <div className="flex items-center justify-between">
              <div className="font-semibold text-sm flex items-center gap-2">
                <FileSpreadsheet className="w-4 h-4 text-emerald-600" />
                Points of Measurement (POM) Spec Matrix
              </div>
              <button
                onClick={handleExportPom}
                className="px-3 py-1 rounded-lg border font-medium cursor-pointer"
                style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
              >
                Export CSV
              </button>
            </div>

            <div className="overflow-x-auto rounded-lg border" style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-surface)' }}>
              <table className="w-full text-left font-mono-num text-[11px]">
                <thead>
                  <tr className="border-b" style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}>
                    <th className="py-1.5 px-2.5 font-sans">Measurement Point</th>
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
        </div>
      </div>
    </div>
  );
};
