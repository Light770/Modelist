import React, { useState } from 'react';
import {
  Box,
  Check,
  ChevronDown,
  Download,
  FilePlus,
  FileText,
  FolderOpen,
  LayoutGrid,
  PanelLeft,
  Plus,
  Redo2,
  Save,
  Settings,
  Square,
  Undo2,
  Variable,
  X,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { getTemplateCatalog } from '../../lib/templateCatalog';
import {
  activateDocument,
  closeDocumentTab,
  MOD_LABEL as MOD,
  newFromTemplate,
  openProject,
  redo,
  saveProject,
  undo,
} from '../../lib/projectActions';
import { useCatalogT, useT } from '../../lib/i18n';
import { useStudioStore } from '../../store/studioStore';
import { Menu, type MenuEntry } from './Menu';

/** Thin icon button used across the app bar. */
const BarButton: React.FC<{
  title: string;
  onClick: (e: React.MouseEvent<HTMLButtonElement>) => void;
  disabled?: boolean;
  active?: boolean;
  trigger?: string;
  children: React.ReactNode;
}> = ({ title, onClick, disabled, active, trigger, children }) => (
  <button
    onClick={onClick}
    disabled={disabled}
    title={title}
    aria-label={title}
    aria-pressed={active}
    data-menu-trigger={trigger}
    className="w-[26px] h-[22px] shrink-0 flex items-center justify-center rounded-[3px] cursor-pointer disabled:cursor-default disabled:opacity-35 hover:bg-[var(--bg-hover)] disabled:hover:bg-transparent"
    style={{
      color: active ? 'var(--accent)' : 'var(--text-secondary)',
      background: active ? 'var(--accent-soft)' : undefined,
    }}
  >
    {children}
  </button>
);

const Divider = () => <div className="w-px h-4 mx-1 shrink-0" style={{ background: 'var(--border-strong)' }} />;

/** 2D / 3D view toggle: a check box, so both can be on at once (side by side). */
const ViewToggle: React.FC<{ on: boolean; label: string; title: string; icon: React.ReactNode; onClick: () => void }> = ({
  on,
  label,
  title,
  icon,
  onClick,
}) => (
  <button
    role="checkbox"
    aria-checked={on}
    onClick={onClick}
    title={title}
    className="h-[22px] pl-1 pr-2 flex items-center gap-1 rounded-[3px] text-[11px] cursor-pointer hover:bg-[var(--bg-hover)]"
    style={{ color: on ? 'var(--text-primary)' : 'var(--text-muted)', fontWeight: on ? 600 : 400 }}
  >
    <span
      className="w-3.5 h-3.5 rounded-[2px] border flex items-center justify-center"
      style={{
        borderColor: on ? 'var(--accent)' : 'var(--border-strong)',
        background: on ? 'var(--accent)' : 'var(--bg-surface)',
      }}
    >
      {on && <Check className="w-2.5 h-2.5 text-white" strokeWidth={3} />}
    </span>
    {icon}
    {label}
  </button>
);

/**
 * Application bar: project space, file menu and quick access on the left, the open document
 * tabs, then parameters and the 2D / 3D view toggles on the right.
 */
export const AppBar: React.FC = () => {
  const t = useT();
  const ct = useCatalogT();
  const snapshot = useStudioStore((s) => s.snapshot);
  const documents = useStudioStore((s) => s.documents);
  const activeDocId = useStudioStore((s) => s.activeDocId);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const leftPanelOpen = useStudioStore((s) => s.leftPanelOpen);
  const dataPanelOpen = useStudioStore((s) => s.dataPanelOpen);
  const panelPage = useStudioStore((s) => s.panelPage);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const toggleView = useStudioStore((s) => s.toggleView);
  const toggleLeftPanel = useStudioStore((s) => s.toggleLeftPanel);
  const toggleDataPanel = useStudioStore((s) => s.toggleDataPanel);
  const setExportModalOpen = useStudioStore((s) => s.setExportModalOpen);
  const setSettingsOpen = useStudioStore((s) => s.setSettingsOpen);
  const openPanelPage = useStudioStore((s) => s.openPanelPage);
  const closePanelPage = useStudioStore((s) => s.closePanelPage);
  const [fileMenu, setFileMenu] = useState<DOMRect | null>(null);
  const [newMenu, setNewMenu] = useState<DOMRect | null>(null);
  const [renaming, setRenaming] = useState(false);

  if (!snapshot) return null;

  const catalog = getTemplateCatalog();
  const templateItems = (category: 'block' | 'style'): MenuEntry[] =>
    catalog
      .filter((tp) => tp.category === category)
      .map((tp) => ({
        id: `tpl-${tp.id}`,
        label: ct('template', `${tp.id}.name`, tp.name),
        title: ct('template', `${tp.id}.description`, tp.description),
        onSelect: () => void newFromTemplate(tp.id),
      }));
  const templateEntries: MenuEntry[] = [
    { kind: 'heading', id: 'h-blocks', label: t('topbar.blocks') },
    ...templateItems('block'),
    { kind: 'heading', id: 'h-styles', label: t('topbar.styles') },
    ...templateItems('style'),
  ];

  const fileEntries: MenuEntry[] = [
    { id: 'new', label: t('appbar.newFromTemplate'), icon: FilePlus, submenu: templateEntries },
    { id: 'open', label: t('appbar.open'), icon: FolderOpen, shortcut: `${MOD}O`, onSelect: () => void openProject() },
    { id: 'save', label: t('appbar.save'), icon: Save, shortcut: `${MOD}S`, onSelect: () => void saveProject() },
    { kind: 'separator', id: 's1' },
    { id: 'export', label: t('appbar.export'), icon: Download, onSelect: () => setExportModalOpen(true) },
    { kind: 'separator', id: 's2' },
    { id: 'settings', label: t('appbar.settings'), icon: Settings, onSelect: () => setSettingsOpen(true) },
  ];

  const show2d = workspaceMode !== '3d';
  const show3d = workspaceMode !== '2d';

  return (
    <header
      className="h-[32px] shrink-0 border-b px-1.5 flex items-end gap-0.5 select-none"
      style={{ background: 'var(--app-bar)', borderColor: 'var(--border-strong)' }}
    >
      {/* Left: project space, file menu, quick access */}
      <div className="h-full flex items-center gap-0.5 shrink-0">
        <BarButton title={t('data.title')} onClick={() => toggleDataPanel()} active={dataPanelOpen} trigger="data">
          <LayoutGrid className="w-3.5 h-3.5" />
        </BarButton>
        <button
          data-menu-trigger="file"
          onClick={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            setFileMenu((m) => (m ? null : r));
          }}
          className="h-[22px] pl-1.5 pr-1 flex items-center gap-0.5 rounded-[3px] text-xs font-medium cursor-pointer hover:bg-[var(--bg-hover)]"
          style={{ background: fileMenu ? 'var(--bg-hover)' : undefined }}
          aria-haspopup="menu"
          aria-expanded={Boolean(fileMenu)}
        >
          {t('appbar.file')}
          <ChevronDown className="w-3 h-3 opacity-70" />
        </button>
        {fileMenu && <Menu anchor={fileMenu} entries={fileEntries} triggerId="file" onClose={() => setFileMenu(null)} />}
        <BarButton title={`${t('appbar.save')} (${MOD}S)`} onClick={() => void saveProject()}>
          <Save className="w-3.5 h-3.5" />
        </BarButton>
        <BarButton title={t('topbar.undo')} onClick={undo} disabled={!snapshot.can_undo}>
          <Undo2 className="w-3.5 h-3.5" />
        </BarButton>
        <BarButton title={t('topbar.redo')} onClick={redo} disabled={!snapshot.can_redo}>
          <Redo2 className="w-3.5 h-3.5" />
        </BarButton>
        <Divider />
      </div>

      {/* Document tabs (one per open file) */}
      <div className="h-full flex-1 min-w-0 flex items-end gap-px overflow-hidden" role="tablist" aria-label={t('tabs.title')}>
        {documents.map((d) => {
          const active = d.id === activeDocId;
          return (
            <div
              key={d.id}
              role="tab"
              aria-selected={active}
              onClick={() => activateDocument(d.id)}
              onDoubleClick={() => active && setRenaming(true)}
              title={active ? t('tabs.renameHint') : d.name}
              className="group min-w-[120px] max-w-[240px] pl-2 pr-1 flex items-center gap-1.5 rounded-t-[4px] border border-b-0 cursor-pointer"
              style={{
                background: active ? 'var(--toolbar)' : 'transparent',
                borderColor: active ? 'var(--border-strong)' : 'transparent',
                marginBottom: active ? -1 : 0,
                height: active ? 28 : 27,
              }}
            >
              <FileText className="w-3.5 h-3.5 shrink-0" style={{ color: active ? 'var(--accent)' : 'var(--text-muted)' }} />
              {active && renaming ? (
                <input
                  autoFocus
                  defaultValue={snapshot.project_name}
                  onBlur={(e) => {
                    setSnapshot(kernel.setProjectName(e.target.value.trim() || snapshot.project_name));
                    setRenaming(false);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                    if (e.key === 'Escape') setRenaming(false);
                  }}
                  className="flex-1 min-w-0 h-[20px] px-1 text-xs rounded-[2px] border outline-none"
                  style={{ borderColor: 'var(--accent)', background: 'var(--bg-surface)' }}
                />
              ) : (
                <span
                  className="flex-1 min-w-0 truncate text-xs"
                  style={{ color: active ? 'var(--text-primary)' : 'var(--text-secondary)', fontWeight: active ? 600 : 400 }}
                >
                  {d.name}
                </span>
              )}
              {documents.length > 1 && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    closeDocumentTab(d.id);
                  }}
                  className={`w-4 h-4 shrink-0 flex items-center justify-center rounded-[2px] cursor-pointer hover:bg-[var(--bg-hover)] ${active ? '' : 'opacity-0 group-hover:opacity-100'}`}
                  style={{ color: 'var(--text-muted)' }}
                  aria-label={t('tabs.close')}
                  title={t('tabs.close')}
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          );
        })}
        <div className="h-full flex items-center pl-0.5">
          <BarButton
            title={t('tabs.new')}
            trigger="new-doc"
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              setNewMenu((m) => (m ? null : r));
            }}
          >
            <Plus className="w-3.5 h-3.5" />
          </BarButton>
          {newMenu && <Menu anchor={newMenu} entries={templateEntries} triggerId="new-doc" onClose={() => setNewMenu(null)} />}
        </div>
      </div>

      {/* Right: parameters, view toggles, browser, settings */}
      <div className="h-full flex items-center gap-0.5 shrink-0">
        <button
          onClick={() => (panelPage === 'params' ? closePanelPage() : openPanelPage('params'))}
          className="h-[22px] px-2 flex items-center gap-1 rounded-[3px] text-[11px] cursor-pointer hover:bg-[var(--bg-hover)]"
          style={{
            background: panelPage === 'params' ? 'var(--accent-soft)' : undefined,
            color: panelPage === 'params' ? 'var(--accent)' : 'var(--text-secondary)',
          }}
          title={t('appbar.paramsTitle')}
          aria-pressed={panelPage === 'params'}
        >
          <Variable className="w-3.5 h-3.5" />
          {t('appbar.params')}
        </button>
        <Divider />
        <ViewToggle on={show2d} label="2D" title={t('appbar.view2d')} icon={<Square className="w-3 h-3" />} onClick={() => toggleView('2d')} />
        <ViewToggle on={show3d} label="3D" title={t('appbar.view3d')} icon={<Box className="w-3 h-3" />} onClick={() => toggleView('3d')} />
        <Divider />
        <BarButton title={t('topbar.toggleBrowser')} onClick={toggleLeftPanel} active={leftPanelOpen}>
          <PanelLeft className="w-3.5 h-3.5" />
        </BarButton>
        <BarButton title={t('settings.open')} onClick={() => setSettingsOpen(true)}>
          <Settings className="w-3.5 h-3.5" />
        </BarButton>
      </div>
    </header>
  );
};
