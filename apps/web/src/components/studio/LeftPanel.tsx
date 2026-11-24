import React, { useState } from 'react';
import {
  ChevronDown,
  ChevronRight,
  Copy,
  Eye,
  EyeOff,
  FlipHorizontal2,
  Footprints,
  Layers,
  Link2,
  Pencil,
  Plus,
  Ruler,
  SlidersHorizontal,
  Sparkles,
  SwatchBook,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { formatMm, SEAM_PALETTE } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';
import { useT } from '../../lib/i18n';

export const LeftPanel: React.FC = () => {
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const startWalkSeam = useStudioStore((s) => s.startWalkSeam);
  const openCreateSeamCommand = useStudioStore((s) => s.openCreateSeamCommand);
  const openEditSeamCommand = useStudioStore((s) => s.openEditSeamCommand);
  const openPanelPage = useStudioStore((s) => s.openPanelPage);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);
  const t = useT();

  // Fusion 360 Collapsible Browser Tree sections
  const [piecesOpen, setPiecesOpen] = useState(true);
  const [seamsOpen, setSeamsOpen] = useState(true);
  const [setupOpen, setSetupOpen] = useState(false);

  if (!snapshot) return null;

  const handleAutoDetect = () => {
    const { count, snapshot: next } = kernel.autoDetectSeams();
    setSnapshot(next);
    setStatusMessage(
      count > 0 ? t('left.autoDetected', { count }) : t('left.allBound'),
    );
  };

  const openInspectorTab = (tab: 'measurements' | 'fabric' | 'grading') => {
    openPanelPage(tab === 'measurements' ? 'params' : tab === 'fabric' ? 'materials' : 'grading');
  };

  return (
    <div className="flex flex-col select-none text-xs">

      {/* Node 1: Document Setup / Calibration / Fabric (Collapsed by default like Fusion 360) */}
      <div className="border-b" style={{ borderColor: 'var(--border-subtle)' }}>
        <button
          onClick={() => setSetupOpen((v) => !v)}
          className="w-full px-2 h-[26px] flex items-center gap-1.5 hover:bg-[var(--bg-hover)] cursor-pointer text-[11px] font-semibold"
        >
          {setupOpen ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
          <SlidersHorizontal className="w-3.5 h-3.5 opacity-70" />
          <span>{t('left.setup')}</span>
        </button>

        {setupOpen && (
          <div className="px-3 pb-2.5 space-y-1 text-[11px]">
            <button
              onClick={() => openInspectorTab('measurements')}
              className="w-full flex items-center justify-between px-2 py-1 rounded hover:bg-[var(--bg-subtle)] cursor-pointer"
            >
              <span className="inline-flex items-center gap-1.5">
                <Ruler className="w-3 h-3 opacity-70" />
                {t('left.bodyEase')}
              </span>
              <span className="font-mono-num text-[10px]" style={{ color: 'var(--text-muted)' }}>
                {snapshot.active_size}
              </span>
            </button>
            <button
              onClick={() => openInspectorTab('fabric')}
              className="w-full flex items-center justify-between px-2 py-1 rounded hover:bg-[var(--bg-subtle)] cursor-pointer"
            >
              <span className="inline-flex items-center gap-1.5">
                <SwatchBook className="w-3 h-3 opacity-70" />
                {t('left.fabric')}
              </span>
              <span className="inline-flex items-center gap-1 font-mono-num text-[10px]" style={{ color: 'var(--text-muted)' }}>
                <span
                  className="w-2.5 h-2.5 rounded-full border"
                  style={{ background: snapshot.fabric.color_hex, borderColor: 'rgba(0,0,0,0.2)' }}
                />
                {snapshot.fabric.density_gsm}g
              </span>
            </button>
            <button
              onClick={() => openInspectorTab('grading')}
              className="w-full flex items-center justify-between px-2 py-1 rounded hover:bg-[var(--bg-subtle)] cursor-pointer"
            >
              <span>{t('left.grading', { count: snapshot.sizes.length })}</span>
              <span className="font-mono-num text-[10px]" style={{ color: 'var(--text-muted)' }}>
                {t('left.visible', { count: snapshot.visible_sizes.length })}
              </span>
            </button>
          </div>
        )}
      </div>

      {/* Node 2: Pattern Pieces Tree */}
      <div className="border-b" style={{ borderColor: 'var(--border-subtle)' }}>
        <div className="px-2 h-[26px] flex items-center justify-between">
          <button
            onClick={() => setPiecesOpen((v) => !v)}
            className="flex items-center gap-1.5 cursor-pointer text-[11px] font-semibold"
          >
            {piecesOpen ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
            <Layers className="w-3.5 h-3.5 opacity-70" />
            <span>{t('left.pieces', { count: snapshot.pieces.length })}</span>
          </button>
        </div>

        {piecesOpen && (
          <div className="px-1.5 pb-2 space-y-px">
            {snapshot.pieces.map((p) => {
              const selected = snapshot.selected_piece_id === p.id;
              return (
                <div
                  key={p.id}
                  onClick={() => setSnapshot(kernel.setSelection(p.id, null))}
                  className="px-2 py-1 rounded-[3px] cursor-pointer group hover:bg-[var(--bg-hover)]"
                  style={{
                    background: selected ? 'var(--accent-soft)' : undefined,
                  }}
                >
                  <div className="flex items-center justify-between gap-1">
                    <div className="min-w-0 flex items-center gap-1.5">
                      <span
                        className="font-mono-num text-[9.5px] px-1 py-0.2 rounded border shrink-0"
                        style={{
                          borderColor: 'var(--border-subtle)',
                          color: selected ? 'var(--accent)' : 'var(--text-muted)',
                        }}
                      >
                        {p.code}
                      </span>
                      <span className="text-[11px] font-medium truncate">{p.name}</span>
                    </div>
                    <div className="flex items-center gap-0.5 shrink-0">
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setSnapshot(kernel.mirrorPiece(p.id));
                        }}
                        className="p-0.5 rounded hover:bg-[var(--bg-subtle)] opacity-0 group-hover:opacity-80 hover:!opacity-100"
                        title={t('left.mirror')}
                      >
                        <FlipHorizontal2 className="w-3 h-3" />
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setSnapshot(kernel.duplicatePiece(p.id));
                        }}
                        className="p-0.5 rounded hover:bg-[var(--bg-subtle)] opacity-0 group-hover:opacity-80 hover:!opacity-100"
                        title={t('left.duplicate')}
                      >
                        <Copy className="w-3 h-3" />
                      </button>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setSnapshot(kernel.togglePieceVisibility(p.id));
                        }}
                        className="p-0.5 rounded hover:bg-[var(--bg-subtle)]"
                        title={p.visible ? t('left.hide') : t('left.show')}
                      >
                        {p.visible ? (
                          <Eye className="w-3 h-3 opacity-75" />
                        ) : (
                          <EyeOff className="w-3 h-3 opacity-35" />
                        )}
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Node 3: Seam Interfaces Tree (Direct Click-to-Select + Fusion 360 Floating Editor) */}
      <div className="flex-1 flex flex-col">
        <div className="px-2 h-[26px] flex items-center justify-between">
          <button
            onClick={() => setSeamsOpen((v) => !v)}
            className="flex items-center gap-1.5 cursor-pointer text-[11px] font-semibold"
          >
            {seamsOpen ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
            <Link2 className="w-3.5 h-3.5 opacity-70" />
            <span>{t('left.seams', { count: snapshot.stitching_links.length })}</span>
          </button>
          <div className="flex items-center gap-1">
            <button
              onClick={handleAutoDetect}
              className="p-1 rounded border hover:bg-[var(--bg-subtle)] cursor-pointer"
              style={{ borderColor: 'var(--border-subtle)' }}
              title={t('left.autoDetect')}
            >
              <Sparkles className="w-3 h-3 text-amber-500" />
            </button>
            <button
              onClick={() => openCreateSeamCommand()}
              className="p-1 rounded border hover:bg-[var(--bg-subtle)] cursor-pointer"
              style={{ borderColor: 'var(--accent)', color: 'var(--accent)' }}
              title={t('left.createSeam')}
            >
              <Plus className="w-3 h-3" />
            </button>
          </div>
        </div>

        {seamsOpen && (
          <div className="px-1.5 pb-2 space-y-px">
            {snapshot.stitching_links.length === 0 ? (
              <button
                onClick={() => openCreateSeamCommand()}
                className="w-full p-2 rounded-[3px] border border-dashed text-center text-[11px] hover:bg-[var(--bg-hover)] cursor-pointer"
                style={{ borderColor: 'var(--border-strong)', color: 'var(--text-secondary)' }}
              >
                {t('left.firstSeam')}
              </button>
            ) : (
              snapshot.stitching_links.map((link) => {
                const isSelected = snapshot.selected_binding_id === link.id;
                const swatchColor =
                  link.status === 'ok'
                    ? SEAM_PALETTE[link.color_idx % SEAM_PALETTE.length]
                    : '#e11d48';
                const badgeColor =
                  link.status === 'ok'
                    ? 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400'
                    : link.status === 'tight'
                      ? 'bg-rose-500/15 text-rose-700 dark:text-rose-400'
                      : 'bg-amber-500/15 text-amber-700 dark:text-amber-400';

                return (
                  <div
                    key={link.id}
                    onClick={() => setSnapshot(kernel.setSelectedBinding(link.id))}
                    onDoubleClick={() => openEditSeamCommand(link)}
                    className="px-2 py-1.5 rounded-[3px] border cursor-pointer space-y-1 hover:bg-[var(--bg-hover)]"
                    style={{
                      background: isSelected ? 'var(--accent-soft)' : undefined,
                      borderColor: isSelected ? swatchColor : 'transparent',
                    }}
                    title={t('left.seamRowHint')}
                  >
                    <div className="flex items-center justify-between gap-1">
                      <div className="flex items-center gap-1.5 min-w-0">
                        <span
                          className="w-2.5 h-2.5 rounded-full shrink-0"
                          style={{ background: swatchColor }}
                        />
                        <span className="font-medium text-[11px] truncate">{link.label}</span>
                      </div>
                      <div className="flex items-center gap-0.5 shrink-0">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            openEditSeamCommand(link);
                          }}
                          className="p-1 rounded hover:bg-[var(--bg-subtle)] cursor-pointer"
                          title={t('left.editSeam')}
                        >
                          <Pencil className="w-3 h-3" style={{ color: 'var(--accent)' }} />
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            startWalkSeam(link.id);
                          }}
                          className="p-1 rounded hover:bg-[var(--bg-subtle)] cursor-pointer"
                          title={t('left.walkSeam')}
                        >
                          <Footprints className="w-3 h-3 text-emerald-600 dark:text-emerald-400" />
                        </button>
                      </div>
                    </div>

                    <div className="flex items-center justify-between text-[10px] font-mono-num">
                      <span style={{ color: 'var(--text-muted)' }}>
                        {link.edge_count_a}e ({formatMm(link.len_a_mm, unit, 0)}) ↔ {link.edge_count_b}e (
                        {formatMm(link.len_b_mm, unit, 0)})
                      </span>
                      <span className={`px-1.5 py-0.2 rounded ${badgeColor}`}>
                        {link.ease_mm >= 0 ? '+' : ''}
                        {formatMm(link.ease_mm, unit, 1)}
                      </span>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        )}
      </div>
    </div>
  );
};
