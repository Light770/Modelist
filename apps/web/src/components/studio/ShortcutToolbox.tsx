import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Bookmark,
  Box,
  Compass,
  Copy,
  Crosshair,
  Download,
  FlipHorizontal2,
  Footprints,
  Hand,
  Link2,
  Maximize2,
  MousePointer2,
  PenTool,
  Pin,
  PinOff,
  Redo2,
  Ruler,
  Scissors,
  Search,
  Sparkles,
  SplitSquareVertical,
  SquareCode,
  SquarePlus,
  Undo2,
  X,
} from 'lucide-react';
import {
  DEFAULT_PINS_2D,
  DEFAULT_PINS_3D,
  loadPinnedCommands,
  MODELIST_COMMANDS,
  savePinnedCommands,
  searchCommands,
  togglePinnedCommand,
  type CommandDef,
} from '../../lib/cad/command-palette';
import { useStudioStore } from '../../store/studioStore';
import { useCatalogT, useT } from '../../lib/i18n';

function iconForCommand(id: string) {
  switch (id) {
    case 'sew_segment':
      return Link2;
    case 'walk_seam':
      return Footprints;
    case 'auto_detect_seams':
      return Sparkles;
    case 'select':
      return MousePointer2;
    case 'direct_edit':
      return PenTool;
    case 'draft_piece':
      return SquarePlus;
    case 'style_split':
      return SplitSquareVertical;
    case 'notch':
      return Bookmark;
    case 'drill_hole':
      return Crosshair;
    case 'true_corner':
      return SquareCode;
    case 'dart_pivot':
      return Compass;
    case 'slash_spread':
      return Scissors;
    case 'mirror_piece':
      return FlipHorizontal2;
    case 'duplicate_piece':
      return Copy;
    case 'measure':
      return Ruler;
    case 'pan':
      return Hand;
    case 'fit_view':
      return Maximize2;
    case 'toggle_3d':
      return Box;
    case 'undo':
      return Undo2;
    case 'redo':
      return Redo2;
    case 'export':
      return Download;
    default:
      return Sparkles;
  }
}

export const ShortcutToolbox: React.FC = () => {
  const t = useT();
  const ct = useCatalogT();
  const toolbox = useStudioStore((s) => s.shortcutToolbox);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const closeShortcutToolbox = useStudioStore((s) => s.closeShortcutToolbox);
  const executeCommand = useStudioStore((s) => s.executeCommand);

  const storageKey =
    workspaceMode === '3d' ? 'modelist.pins.3d.v1' : 'modelist.pins.2d.v1';
  const defaultPins = workspaceMode === '3d' ? DEFAULT_PINS_3D : DEFAULT_PINS_2D;

  const [pins, setPins] = useState<string[]>(() => loadPinnedCommands(storageKey, defaultPins));
  const [query, setQuery] = useState('');
  const [selectedIdx, setSelectedIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setPins(loadPinnedCommands(storageKey, defaultPins));
  }, [storageKey, defaultPins]);

  useEffect(() => {
    if (toolbox?.open) {
      setQuery('');
      setSelectedIdx(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [toolbox?.open]);

  useEffect(() => {
    if (!toolbox?.open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        closeShortcutToolbox();
      }
    };
    window.addEventListener('pointerdown', onPointerDown);
    return () => window.removeEventListener('pointerdown', onPointerDown);
  }, [closeShortcutToolbox, toolbox?.open]);

  const results = useMemo(
    () => searchCommands(query, workspaceMode, MODELIST_COMMANDS),
    [query, workspaceMode],
  );

  const pinnedCommands = useMemo(
    () =>
      pins
        .map((id) => MODELIST_COMMANDS.find((c) => c.id === id))
        .filter((c): c is CommandDef => Boolean(c && c.workspaces.includes(workspaceMode))),
    [pins, workspaceMode],
  );

  if (!toolbox?.open) return null;

  const winW = typeof window !== 'undefined' ? window.innerWidth : 1280;
  const winH = typeof window !== 'undefined' ? window.innerHeight : 800;
  const width = 296;
  const left = Math.max(12, Math.min(toolbox.x - width * 0.5, winW - width - 16));
  const top = Math.max(54, Math.min(toolbox.y - 28, winH - 360));

  const runCmd = (id: string) => {
    closeShortcutToolbox();
    executeCommand(id);
  };

  const handleTogglePin = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    const next = togglePinnedCommand(pins, id, 12);
    setPins(next);
    savePinnedCommands(storageKey, next);
  };

  return (
    <div
      ref={panelRef}
      className="fixed z-50 rounded-md border shadow-2xl overflow-hidden select-none backdrop-blur-md"
      style={{
        left,
        top,
        width,
        background: 'var(--bg-surface)',
        borderColor: 'var(--border-strong)',
      }}
    >
      {/* Header + Pinned 6-Column Grid (Fusion 360 S-Key Shortcut Toolbox) */}
      <div
        className="px-2.5 pt-2 pb-1.5 border-b space-y-1.5"
        style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
      >
        <div className="flex items-center justify-between text-[10px]">
          <span className="font-semibold uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>
            {workspaceMode === '3d' ? t('toolbox.titleFit') : t('toolbox.titlePattern')}
          </span>
          <button
            onClick={closeShortcutToolbox}
            className="p-0.5 rounded hover:bg-[var(--bg-subtle)] cursor-pointer"
            title={t('toolbox.close')}
          >
            <X className="w-3 h-3" />
          </button>
        </div>

        <div className="grid grid-cols-6 gap-1">
          {pinnedCommands.map((cmd) => {
            const Icon = iconForCommand(cmd.id);
            return (
              <button
                key={cmd.id}
                onClick={() => runCmd(cmd.id)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  handleTogglePin(e, cmd.id);
                }}
                title={`${ct('cmd', `${cmd.id}.label`, cmd.label)}${cmd.shortcut ? ` (${cmd.shortcut})` : ''} — ${t('toolbox.unpinHint')}`}
                className="h-9 rounded border flex flex-col items-center justify-center gap-0.5 hover:border-[var(--accent)] hover:bg-[var(--accent-soft)] transition cursor-pointer"
                style={{
                  background: 'var(--bg-surface)',
                  borderColor: 'var(--border-subtle)',
                  color: 'var(--text-primary)',
                }}
              >
                <Icon className="w-3.5 h-3.5" />
                {cmd.shortcut && (
                  <span className="text-[8px] font-mono-num leading-none opacity-65">
                    {cmd.shortcut}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {/* Instant Fuzzy Search Input */}
      <div className="px-2.5 py-2 border-b flex items-center gap-1.5" style={{ borderColor: 'var(--border-subtle)' }}>
        <Search className="w-3.5 h-3.5 shrink-0 opacity-60" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSelectedIdx(0);
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setSelectedIdx((i) => (results.length ? (i + 1) % results.length : 0));
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              setSelectedIdx((i) => (results.length ? (i - 1 + results.length) % results.length : 0));
            } else if (e.key === 'Enter' && results.length > 0) {
              e.preventDefault();
              runCmd(results[selectedIdx]?.id ?? results[0].id);
            } else if (e.key === 'Escape') {
              e.preventDefault();
              closeShortcutToolbox();
            }
          }}
          placeholder={t('toolbox.placeholder')}
          className="w-full bg-transparent text-xs outline-none"
        />
      </div>

      {/* Matching Commands List */}
      <div className="max-h-56 overflow-y-auto py-1">
        {results.length === 0 ? (
          <div className="px-3 py-4 text-center text-[11px]" style={{ color: 'var(--text-muted)' }}>
            No matching command
          </div>
        ) : (
          results.slice(0, 10).map((cmd, idx) => {
            const Icon = iconForCommand(cmd.id);
            const isPinned = pins.includes(cmd.id);
            const activeRow = idx === selectedIdx;
            return (
              <div
                key={cmd.id}
                onMouseEnter={() => setSelectedIdx(idx)}
                onClick={() => runCmd(cmd.id)}
                className="px-2.5 py-1.5 flex items-center gap-2 text-xs cursor-pointer transition"
                style={{
                  background: activeRow ? 'var(--accent-soft)' : 'transparent',
                }}
              >
                <Icon
                  className="w-3.5 h-3.5 shrink-0"
                  style={{ color: activeRow ? 'var(--accent)' : 'var(--text-secondary)' }}
                />
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-[11px] truncate">{ct('cmd', `${cmd.id}.label`, cmd.label)}</div>
                  <div className="text-[9px] truncate" style={{ color: 'var(--text-muted)' }}>
                    {ct('cmd', `${cmd.id}.desc`, cmd.description)}
                  </div>
                </div>
                {cmd.shortcut && (
                  <kbd
                    className="text-[9px] font-mono-num px-1 py-0.2 rounded border shrink-0"
                    style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-muted)' }}
                  >
                    {cmd.shortcut}
                  </kbd>
                )}
                <button
                  type="button"
                  onClick={(e) => handleTogglePin(e, cmd.id)}
                  title={isPinned ? t('toolbox.unpin') : t('toolbox.pin')}
                  className="p-1 rounded hover:bg-black/10 dark:hover:bg-white/10 shrink-0 cursor-pointer"
                  style={{ color: isPinned ? 'var(--accent)' : 'var(--text-muted)' }}
                >
                  {isPinned ? <PinOff className="w-3 h-3" /> : <Pin className="w-3 h-3" />}
                </button>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
