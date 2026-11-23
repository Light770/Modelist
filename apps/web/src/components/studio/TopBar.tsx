import React from 'react';
import {
  Download,
  FolderOpen,
  Moon,
  Redo2,
  Save,
  Sun,
  Undo2,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { openTextFileFromDisk, saveProjectToDisk } from '../../lib/localFiles';
import type { DisplayUnit } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

export const TopBar: React.FC = () => {
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const theme = useStudioStore((s) => s.theme);
  const showSA = useStudioStore((s) => s.showSeamAllowance);
  const showNotches = useStudioStore((s) => s.showNotches);
  const showGrain = useStudioStore((s) => s.showGrainlines);
  const showNest = useStudioStore((s) => s.showGradedNest);
  const showEase = useStudioStore((s) => s.showEaseBadges);

  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setUnit = useStudioStore((s) => s.setUnit);
  const setTheme = useStudioStore((s) => s.setTheme);
  const toggleLayer = useStudioStore((s) => s.toggleLayer);
  const setExportModalOpen = useStudioStore((s) => s.setExportModalOpen);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);
  const requestFitView = useStudioStore((s) => s.requestFitView);

  if (!snapshot) return null;

  const handleSave = async () => {
    try {
      const savedName = await saveProjectToDisk(snapshot.project_name, kernel.serializeProject());
      setStatusMessage(`Saved ${savedName}`);
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
      setStatusMessage(`Opened ${file.name}`);
    } catch (e) {
      setStatusMessage(`Open failed: ${(e as Error).message}`);
    }
  };

  return (
    <header
      className="h-12 shrink-0 border-b px-3 flex items-center justify-between gap-2 select-none"
      style={{
        background: 'var(--bg-surface)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {/* Left: Brand, Template, Project Name */}
      <div className="flex items-center gap-2.5 min-w-0">
        <a
          href="/"
          className="w-7 h-7 rounded-md flex items-center justify-center font-mono-num text-xs font-bold text-white shrink-0"
          style={{ background: 'var(--accent)' }}
          title="Back to Modelist Home"
        >
          M
        </a>

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
          title="Switch Starter Block"
        >
          <option value="bodice_sleeve">Block: Bodice &amp; Sleeve</option>
          <option value="aline_skirt">Block: A-Line Skirt</option>
        </select>

        <input
          type="text"
          value={snapshot.project_name}
          onChange={(e) => setSnapshot(kernel.setProjectName(e.target.value))}
          className="text-sm font-semibold bg-transparent border border-transparent hover:border-[var(--border-subtle)] focus:border-[var(--accent)] rounded px-2 py-0.5 outline-none w-48 truncate"
          style={{ color: 'var(--text-primary)' }}
          title="Project Name"
        />

        <span
          className="hidden xl:inline-flex text-[11px] font-mono-num px-2 py-0.5 rounded border"
          style={{
            borderColor: 'var(--border-subtle)',
            background: 'var(--bg-panel)',
            color: 'var(--text-secondary)',
          }}
        >
          Size {snapshot.active_size}
        </span>
      </div>

      {/* Center: Undo/Redo + Layer Visibility Pills */}
      <div className="flex items-center gap-1.5">
        <div className="flex items-center border rounded-md overflow-hidden" style={{ borderColor: 'var(--border-subtle)' }}>
          <button
            onClick={() => {
              const next = kernel.undo();
              if (next) setSnapshot(next);
            }}
            disabled={!snapshot.can_undo}
            className="p-1.5 disabled:opacity-35 hover:bg-[var(--bg-subtle)] transition cursor-pointer"
            title="Undo (Ctrl/Cmd+Z)"
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
            title="Redo (Ctrl/Cmd+Shift+Z)"
          >
            <Redo2 className="w-3.5 h-3.5" />
          </button>
        </div>

        <div className="hidden md:flex items-center gap-1 pl-2">
          {(
            [
              ['showSeamAllowance', 'SA', showSA],
              ['showNotches', 'Notches', showNotches],
              ['showGrainlines', 'Grain', showGrain],
              ['showGradedNest', 'Nest', showNest],
              ['showEaseBadges', 'Ease', showEase],
            ] as const
          ).map(([key, label, active]) => (
            <button
              key={key}
              onClick={() => toggleLayer(key)}
              className="px-2 py-1 text-[11px] font-medium rounded-md border transition cursor-pointer"
              style={{
                borderColor: active ? 'var(--accent)' : 'var(--border-subtle)',
                background: active ? 'var(--accent-soft)' : 'transparent',
                color: active ? 'var(--accent)' : 'var(--text-secondary)',
              }}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Right: Units, Theme, Open/Save, Export */}
      <div className="flex items-center gap-1.5 shrink-0">
        <div
          className="flex items-center rounded-md border p-0.5 text-[11px] font-mono-num"
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
          title={theme === 'paper' ? 'Switch to Atelier Dark' : 'Switch to Drafting Paper'}
        >
          {theme === 'paper' ? <Moon className="w-3.5 h-3.5" /> : <Sun className="w-3.5 h-3.5" />}
        </button>

        <button
          onClick={handleOpen}
          className="hidden sm:inline-flex items-center gap-1 px-2.5 py-1 text-xs font-medium rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{ borderColor: 'var(--border-subtle)' }}
          title="Open Local .modelist.json"
        >
          <FolderOpen className="w-3.5 h-3.5" />
          Open
        </button>

        <button
          onClick={handleSave}
          className="hidden sm:inline-flex items-center gap-1 px-2.5 py-1 text-xs font-medium rounded-md border hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{ borderColor: 'var(--border-subtle)' }}
          title="Save Local .modelist.json"
        >
          <Save className="w-3.5 h-3.5" />
          Save
        </button>

        <button
          onClick={() => setExportModalOpen(true)}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold rounded-md text-white shadow-sm hover:opacity-95 transition cursor-pointer"
          style={{ background: 'var(--accent)' }}
        >
          <Download className="w-3.5 h-3.5" />
          Export
        </button>
      </div>
    </header>
  );
};
