import React from 'react';
import {
  Box,
  Columns2,
  Download,
  FolderOpen,
  Moon,
  PanelLeft,
  PanelRight,
  Redo2,
  Save,
  Settings,
  Square,
  Sun,
  Undo2,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { openTextFileFromDisk, saveProjectToDisk } from '../../lib/localFiles';
import { getTemplateCatalog } from '../../lib/templateCatalog';
import { useCatalogT, useT, type MessageKey } from '../../lib/i18n';
import type { DisplayUnit, WorkspaceMode } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

export const TopBar: React.FC = () => {
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const theme = useStudioStore((s) => s.theme);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const leftPanelOpen = useStudioStore((s) => s.leftPanelOpen);
  const rightPanelOpen = useStudioStore((s) => s.rightPanelOpen);

  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setUnit = useStudioStore((s) => s.setUnit);
  const setTheme = useStudioStore((s) => s.setTheme);
  const setWorkspaceMode = useStudioStore((s) => s.setWorkspaceMode);
  const toggleLeftPanel = useStudioStore((s) => s.toggleLeftPanel);
  const toggleRightPanel = useStudioStore((s) => s.toggleRightPanel);
  const setExportModalOpen = useStudioStore((s) => s.setExportModalOpen);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);
  const requestFitView = useStudioStore((s) => s.requestFitView);
  const setSettingsOpen = useStudioStore((s) => s.setSettingsOpen);
  const t = useT();
  const ct = useCatalogT();

  if (!snapshot) return null;

  const handleSave = async () => {
    try {
      const savedName = await saveProjectToDisk(snapshot.project_name, kernel.serializeProject());
      setStatusMessage(t('topbar.saved', { name: savedName }));
    } catch {
      // Cancelled
    }
  };

  const handleOpen = async () => {
    const file = await openTextFileFromDisk('.modelist.json,.modelist,.json');
    if (!file) return;
    try {
      const next = kernel.loadProjectJson(file.text);
      setSnapshot(next);
      requestFitView();
      setStatusMessage(t('topbar.opened', { name: file.name }));
    } catch (e) {
      setStatusMessage(t('topbar.openFailed', { error: (e as Error).message }));
    }
  };

  return (
    <header
      className="h-11 shrink-0 border-b px-2.5 flex items-center justify-between gap-2 select-none"
      style={{
        background: 'var(--bg-surface)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {/* Left: Brand, Panel Toggle, Block Library, Project Name */}
      <div className="flex items-center gap-1.5 min-w-0">
        <a
          href="/"
          className="w-7 h-7 rounded-md flex items-center justify-center font-mono-num text-xs font-bold text-white shrink-0"
          style={{ background: 'var(--accent)' }}
          title={t('topbar.home')}
        >
          M
        </a>

        <button
          onClick={toggleLeftPanel}
          className="p-1.5 rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{
            borderColor: leftPanelOpen ? 'var(--border-strong)' : 'var(--border-subtle)',
            background: leftPanelOpen ? 'var(--bg-panel)' : 'transparent',
            color: leftPanelOpen ? 'var(--text-primary)' : 'var(--text-muted)',
          }}
          title={t('topbar.toggleBrowser')}
        >
          <PanelLeft className="w-3.5 h-3.5" />
        </button>

        <select
          value={snapshot.template_id}
          onChange={(e) => {
            setSnapshot(kernel.loadTemplate(e.target.value));
            requestFitView();
          }}
          className="text-xs font-medium rounded-md border px-2 py-1 outline-none cursor-pointer"
          style={{
            background: 'var(--bg-panel)',
            borderColor: 'var(--border-subtle)',
            color: 'var(--text-primary)',
          }}
          title={t('topbar.library')}
        >
          <optgroup label={t('topbar.blocks')}>
            {getTemplateCatalog()
              .filter((tp) => tp.category === 'block')
              .map((tp) => (
                <option key={tp.id} value={tp.id} title={ct('template', `${tp.id}.description`, tp.description)}>
                  {ct('template', `${tp.id}.name`, tp.name)}
                </option>
              ))}
          </optgroup>
          <optgroup label={t('topbar.styles')}>
            {getTemplateCatalog()
              .filter((tp) => tp.category === 'style')
              .map((tp) => (
                <option key={tp.id} value={tp.id} title={ct('template', `${tp.id}.description`, tp.description)}>
                  {ct('template', `${tp.id}.name`, tp.name)}
                </option>
              ))}
          </optgroup>
        </select>

        <input
          type="text"
          value={snapshot.project_name}
          onChange={(e) => setSnapshot(kernel.setProjectName(e.target.value))}
          className="hidden md:block text-xs font-semibold bg-transparent border border-transparent hover:border-[var(--border-subtle)] focus:border-[var(--accent)] rounded px-1.5 py-0.5 outline-none w-40 truncate"
          style={{ color: 'var(--text-primary)' }}
          title={t('topbar.projectName')}
        />

        <span
          className="hidden xl:inline-flex text-[10px] font-mono-num px-1.5 py-0.5 rounded border"
          style={{
            borderColor: 'var(--border-subtle)',
            background: 'var(--bg-panel)',
            color: 'var(--text-secondary)',
          }}
        >
          {t('topbar.size', { size: snapshot.active_size })}
        </span>
      </div>

      {/* Center: CLO3D / Browzwear Workspace Mode Switcher + Undo/Redo */}
      <div className="flex items-center gap-2">
        <div
          className="flex items-center rounded-lg border p-0.5 text-[11px] font-medium"
          style={{ borderColor: 'var(--border-strong)', background: 'var(--bg-panel)' }}
        >
          {(
            [
              ['2d', 'topbar.mode2d', Square],
              ['split', 'topbar.modeSplit', Columns2],
              ['3d', 'topbar.mode3d', Box],
            ] as Array<[WorkspaceMode, MessageKey, React.FC<{ className?: string }>]>
          ).map(([mode, label, Icon]) => {
            const active = workspaceMode === mode;
            return (
              <button
                key={mode}
                onClick={() => setWorkspaceMode(mode)}
                className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md transition cursor-pointer"
                style={{
                  background: active ? 'var(--bg-surface)' : 'transparent',
                  color: active ? 'var(--accent)' : 'var(--text-secondary)',
                  boxShadow: active ? '0 1px 2px rgba(0,0,0,0.08)' : 'none',
                }}
                title={t('topbar.switchTo', { mode: t(label) })}
              >
                <Icon className="w-3.5 h-3.5" />
                <span>{t(label)}</span>
              </button>
            );
          })}
        </div>

        <div className="flex items-center border rounded-md overflow-hidden" style={{ borderColor: 'var(--border-subtle)' }}>
          <button
            onClick={() => {
              const next = kernel.undo();
              if (next) setSnapshot(next);
            }}
            disabled={!snapshot.can_undo}
            className="p-1.5 disabled:opacity-35 hover:bg-[var(--bg-subtle)] transition cursor-pointer"
            title={t('topbar.undo')}
          >
            <Undo2 className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => {
              const next = kernel.redo();
              if (next) setSnapshot(next);
            }}
            disabled={!snapshot.can_redo}
            className="p-1.5 border-l disabled:opacity-35 hover:bg-[var(--bg-subtle)] transition cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
            title={t('topbar.redo')}
          >
            <Redo2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Right: Units, Theme, File I/O, Export, Right Inspector Toggle */}
      <div className="flex items-center gap-1.5 shrink-0">
        <div
          className="flex items-center rounded-md border p-0.5 text-[10px] font-mono-num"
          style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}
        >
          {(['mm', 'cm', 'in'] as DisplayUnit[]).map((u) => (
            <button
              key={u}
              onClick={() => setUnit(u)}
              className="px-1.5 py-0.5 rounded transition cursor-pointer"
              style={{
                background: unit === u ? 'var(--bg-surface)' : 'transparent',
                color: unit === u ? 'var(--text-primary)' : 'var(--text-muted)',
              }}
            >
              {u}
            </button>
          ))}
        </div>

        <button
          onClick={() => setTheme(theme === 'paper' ? 'dark' : 'paper')}
          className="p-1.5 rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}
          title={theme === 'paper' ? t('topbar.toDark') : t('topbar.toPaper')}
        >
          {theme === 'paper' ? <Moon className="w-3.5 h-3.5" /> : <Sun className="w-3.5 h-3.5" />}
        </button>

        <button
          onClick={handleOpen}
          className="hidden sm:inline-flex items-center gap-1 px-2 py-1 text-xs font-medium rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{ borderColor: 'var(--border-subtle)' }}
          title={t('topbar.openTitle')}
        >
          <FolderOpen className="w-3.5 h-3.5" />
          <span className="hidden lg:inline">{t('topbar.open')}</span>
        </button>

        <button
          onClick={handleSave}
          className="hidden sm:inline-flex items-center gap-1 px-2 py-1 text-xs font-medium rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{ borderColor: 'var(--border-subtle)' }}
          title={t('topbar.saveTitle')}
        >
          <Save className="w-3.5 h-3.5" />
          <span className="hidden lg:inline">{t('topbar.save')}</span>
        </button>

        <button
          onClick={() => setExportModalOpen(true)}
          className="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-semibold rounded-md text-white shadow-xs hover:opacity-95 transition cursor-pointer"
          style={{ background: 'var(--accent)' }}
        >
          <Download className="w-3.5 h-3.5" />
          <span>{t('topbar.export')}</span>
        </button>

        <button
          onClick={() => setSettingsOpen(true)}
          className="p-1.5 rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}
          title={t('settings.open')}
          aria-label={t('settings.open')}
        >
          <Settings className="w-3.5 h-3.5" />
        </button>

        <button
          onClick={toggleRightPanel}
          className="p-1.5 rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{
            borderColor: rightPanelOpen ? 'var(--border-strong)' : 'var(--border-subtle)',
            background: rightPanelOpen ? 'var(--bg-panel)' : 'transparent',
            color: rightPanelOpen ? 'var(--text-primary)' : 'var(--text-muted)',
          }}
          title={t('topbar.toggleInspector')}
        >
          <PanelRight className="w-3.5 h-3.5" />
        </button>
      </div>
    </header>
  );
};
