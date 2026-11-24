import React, { useEffect, useState } from 'react';
import { Download, FileInput, FileText, FolderOpen, Plus, Shirt, Trash2, X } from 'lucide-react';
import { listSavedGarments, thumbnailUrl, type SavedGarment } from '../../lib/garmentLibrary';
import { getTemplateCatalog } from '../../lib/templateCatalog';
import {
  activateDocument,
  closeDocumentTab,
  deleteLibraryGarment,
  exportGarmentFile,
  importGarmentFile,
  insertSavedGarment,
  newFromTemplate,
  openProject,
} from '../../lib/projectActions';
import { useCatalogT, useT } from '../../lib/i18n';
import { useStudioStore } from '../../store/studioStore';

/** One saved garment: preview, name, and insert / export / delete. */
const SavedGarmentRow: React.FC<{ g: SavedGarment }> = ({ g }) => {
  const t = useT();
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    const u = thumbnailUrl(g);
    setUrl(u);
    return () => {
      if (u) URL.revokeObjectURL(u);
    };
  }, [g]);
  const iconBtn = 'w-6 h-6 flex items-center justify-center rounded hover:bg-[var(--bg-subtle)] cursor-pointer';
  return (
    <div className="group px-2 py-1.5 flex items-center gap-2 rounded hover:bg-[var(--bg-hover)]" data-saved-garment={g.id}>
      <div
        className="w-10 h-10 shrink-0 rounded border flex items-center justify-center overflow-hidden"
        style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-surface)' }}
      >
        {url ? <img src={url} alt="" className="max-w-full max-h-full" /> : <Shirt className="w-4 h-4 opacity-40" />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="font-medium truncate">{g.name}</div>
        <div className="text-[10.5px]" style={{ color: 'var(--text-muted)' }}>
          {t('library.savedOn', { date: new Date(g.saved_at).toLocaleDateString() })}
        </div>
      </div>
      <button onClick={() => void insertSavedGarment(g.id)} className={iconBtn} title={t('library.insert')} aria-label={t('library.insert')}>
        <Plus className="w-3.5 h-3.5" style={{ color: 'var(--accent)' }} />
      </button>
      <button
        onClick={() => void exportGarmentFile({ entryId: g.id })}
        className={`${iconBtn} opacity-0 group-hover:opacity-100`}
        title={t('library.export')}
        aria-label={t('library.export')}
      >
        <Download className="w-3.5 h-3.5" />
      </button>
      <button
        onClick={() => void deleteLibraryGarment(g.id)}
        className={`${iconBtn} opacity-0 group-hover:opacity-100`}
        title={t('library.delete')}
        aria-label={t('library.delete')}
      >
        <Trash2 className="w-3.5 h-3.5" />
      </button>
    </div>
  );
};

/**
 * Project space (the grid button in the app bar): the open documents, the user's saved
 * garments, and the starter library to begin a new document. Opens over the canvas and closes
 * on Escape or after a choice.
 */

export const DataPanel: React.FC = () => {
  const t = useT();
  const ct = useCatalogT();
  const open = useStudioStore((s) => s.dataPanelOpen);
  const documents = useStudioStore((s) => s.documents);
  const activeDocId = useStudioStore((s) => s.activeDocId);
  const toggle = useStudioStore((s) => s.toggleDataPanel);
  const libraryVersion = useStudioStore((s) => s.libraryVersion);
  const [saved, setSaved] = useState<SavedGarment[]>([]);
  useEffect(() => {
    if (open) void listSavedGarments().then(setSaved);
  }, [open, libraryVersion]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') toggle(false);
    };
    const onDown = (e: PointerEvent) => {
      const target = e.target as HTMLElement | null;
      if (!target?.closest('[data-data-panel]') && !target?.closest('[data-menu-trigger="data"]')) toggle(false);
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('pointerdown', onDown, true);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('pointerdown', onDown, true);
    };
  }, [open, toggle]);

  if (!open) return null;
  const catalog = getTemplateCatalog();
  const section = (title: string) => (
    <div className="px-3 pt-3 pb-1 text-[10px] font-semibold uppercase tracking-wide" style={{ color: 'var(--text-muted)' }}>
      {title}
    </div>
  );

  return (
    <aside
      data-data-panel
      className="floating-panel absolute top-2 left-2 bottom-2 z-40 w-[320px] flex flex-col overflow-hidden select-none text-xs"
      aria-label={t('data.title')}
    >
      <header className="h-[30px] shrink-0 px-3 flex items-center justify-between border-b" style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}>
        <span className="text-[11px] font-semibold uppercase tracking-wide" style={{ color: 'var(--text-secondary)' }}>
          {t('data.title')}
        </span>
        <button onClick={() => toggle(false)} className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-hover)] cursor-pointer" aria-label={t('common.close')}>
          <X className="w-3.5 h-3.5" />
        </button>
      </header>
      <div className="flex-1 min-h-0 overflow-y-auto pb-3">
        {section(t('data.openDocuments'))}
        <div className="px-1.5 space-y-px">
          {documents.map((d) => (
            <div
              key={d.id}
              onClick={() => {
                activateDocument(d.id);
                toggle(false);
              }}
              className="group h-[28px] px-2 flex items-center gap-2 rounded cursor-pointer hover:bg-[var(--bg-hover)]"
              style={{ background: d.id === activeDocId ? 'var(--accent-soft)' : undefined }}
            >
              <FileText className="w-3.5 h-3.5 shrink-0" style={{ color: d.id === activeDocId ? 'var(--accent)' : 'var(--text-secondary)' }} />
              <span className="flex-1 truncate">{d.name}</span>
              {documents.length > 1 && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    closeDocumentTab(d.id);
                  }}
                  className="w-5 h-5 flex items-center justify-center rounded opacity-0 group-hover:opacity-100 hover:bg-[var(--bg-subtle)] cursor-pointer"
                  aria-label={t('tabs.close')}
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          ))}
          <button
            onClick={() => {
              toggle(false);
              void openProject();
            }}
            className="w-full h-[28px] px-2 flex items-center gap-2 rounded cursor-pointer hover:bg-[var(--bg-hover)]"
            style={{ color: 'var(--text-secondary)' }}
          >
            <FolderOpen className="w-3.5 h-3.5" />
            {t('appbar.open')}
          </button>
        </div>
        {section(t('library.myGarments'))}
        <div className="px-1.5 space-y-px">
          {saved.length === 0 ? (
            <p className="px-2 py-1 text-[11px] leading-snug" style={{ color: 'var(--text-muted)' }}>
              {t('library.empty')}
            </p>
          ) : (
            saved.map((g) => <SavedGarmentRow key={g.id} g={g} />)
          )}
          <button
            onClick={() => void importGarmentFile()}
            className="w-full h-[28px] px-2 flex items-center gap-2 rounded cursor-pointer hover:bg-[var(--bg-hover)]"
          >
            <FileInput className="w-3.5 h-3.5" />
            <span>{t('library.importFile')}</span>
          </button>
        </div>

        {(['block', 'style'] as const).map((cat) => (
          <React.Fragment key={cat}>
            {section(cat === 'block' ? t('topbar.blocks') : t('topbar.styles'))}
            <div className="px-1.5 space-y-px">
              {catalog
                .filter((tp) => tp.category === cat)
                .map((tp) => (
                  <button
                    key={tp.id}
                    onClick={() => {
                      toggle(false);
                      void newFromTemplate(tp.id);
                    }}
                    className="w-full px-2 py-1.5 rounded text-left cursor-pointer hover:bg-[var(--bg-hover)]"
                    title={t('data.newTab')}
                  >
                    <div className="font-medium">{ct('template', `${tp.id}.name`, tp.name)}</div>
                    <div className="text-[11px] leading-snug line-clamp-2" style={{ color: 'var(--text-muted)' }}>
                      {ct('template', `${tp.id}.description`, tp.description)}
                    </div>
                  </button>
                ))}
            </div>
          </React.Fragment>
        ))}
      </div>
    </aside>
  );
};
