import React, { Suspense, lazy, useEffect, useRef, useState } from 'react';
import { initKernel, kernel } from '../../lib/kernelClient';
import { loadAutosave } from '../../lib/localFiles';
import { useStudioStore } from '../../store/studioStore';
import { ActionRibbon } from './ActionRibbon';
import { ExportModal } from './ExportModal';
import { LeftPanel } from './LeftPanel';
import { ParametricTimeline } from './ParametricTimeline';
import { RightInspector } from './RightInspector';
import { SettingsModal } from './SettingsModal';
import { ShortcutToolbox } from './ShortcutToolbox';
import { useT } from '../../lib/i18n';
import { StatusBar } from './StatusBar';
import { TopBar } from './TopBar';
import { Viewport2D } from './Viewport2D';

// Strictly lazy-loaded 3D Virtual Fit Room (never loaded during 2D drafting!)
const LazyViewport3D = lazy(() => import('./Viewport3D'));

export const StudioRoot: React.FC = () => {
  const t = useT();
  const ready = useStudioStore((s) => s.ready);
  const error = useStudioStore((s) => s.error);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const splitRatio = useStudioStore((s) => s.splitRatio);
  const leftPanelOpen = useStudioStore((s) => s.leftPanelOpen);
  const rightPanelOpen = useStudioStore((s) => s.rightPanelOpen);
  const setReady = useStudioStore((s) => s.setReady);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setSplitRatio = useStudioStore((s) => s.setSplitRatio);
  const requestFitView = useStudioStore((s) => s.requestFitView);

  const stageRef = useRef<HTMLDivElement | null>(null);
  const draggingSplitRef = useRef(false);
  const lastCursorRef = useRef<{ x: number; y: number }>({ x: 520, y: 340 });
  const [autosaveCandidate, setAutosaveCandidate] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const params = new URLSearchParams(window.location.search);
    const tmpl = params.get('template') || 'bodice_sleeve';
    // Shareable deep links: ?mode=3d|split|2d and ?lang=fr|en
    const mode = params.get('mode');
    if (mode === '2d' || mode === 'split' || mode === '3d') {
      useStudioStore.getState().setWorkspaceMode(mode);
    }
    const lang = params.get('lang');
    if (lang === 'fr' || lang === 'en') {
      useStudioStore.getState().setLanguage(lang);
    }
    const tab = params.get('tab');
    if (tab === 'measurements' || tab === 'piece' || tab === 'fabric' || tab === 'grading' || tab === 'formulas') {
      useStudioStore.getState().setActiveTab(tab);
    }

    initKernel(tmpl)
      .then(async (snap) => {
        if (!active) return;
        setSnapshot(snap);
        setReady(true);
        requestFitView();
        const saved = await loadAutosave();
        if (saved && saved.jsonText && active) {
          setAutosaveCandidate(saved.jsonText);
        }
      })
      .catch((err) => {
        if (!active) return;
        setReady(false, (err as Error).message);
      });

    return () => {
      active = false;
    };
  }, [requestFitView, setReady, setSnapshot]);

  // Smooth draggable split divider (3D | 2D)
  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      if (!draggingSplitRef.current || !stageRef.current) return;
      const rect = stageRef.current.getBoundingClientRect();
      if (rect.width < 100) return;
      const ratio = (e.clientX - rect.left) / rect.width;
      setSplitRatio(ratio);
    };
    const onUp = () => {
      if (draggingSplitRef.current) {
        draggingSplitRef.current = false;
        requestFitView();
      }
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  }, [requestFitView, setSplitRatio]);

  // Global Studio Shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT')) {
        return;
      }
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        const next = e.shiftKey ? kernel.redo() : kernel.undo();
        if (next) setSnapshot(next);
        return;
      }
      if (mod && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        useStudioStore
          .getState()
          .toggleShortcutToolbox(lastCursorRef.current.x, lastCursorRef.current.y);
        return;
      }
      const store = useStudioStore.getState();
      switch (e.key.toLowerCase()) {
        case 'v':
          store.setActiveTool('select');
          break;
        case 'a':
          store.setActiveTool('direct_edit');
          break;
        case 'p':
          store.setActiveTool('draft_piece');
          break;
        case 'e':
          store.openCreateSeamCommand();
          break;
        case 'w':
          store.setActiveTool('walk_seam');
          break;
        case 't':
          store.setActiveTool('true_corner');
          break;
        case 'd':
          store.setActiveTool('dart_pivot');
          break;
        case 'f':
          store.setActiveTool('slash_spread');
          break;
        case 's':
          e.preventDefault();
          store.toggleShortcutToolbox(lastCursorRef.current.x, lastCursorRef.current.y);
          break;
        case 'k':
          store.setActiveTool('style_split');
          break;
        case 'n':
          store.setActiveTool('notch');
          break;
        case 'b':
          store.setActiveTool('drill_hole');
          break;
        case 'm':
          store.setActiveTool('measure');
          break;
        case 'h':
          store.setActiveTool('pan');
          break;
        case '1':
          store.setWorkspaceMode('2d');
          break;
        case '2':
          store.setWorkspaceMode('split');
          break;
        case '3':
          store.setWorkspaceMode('3d');
          break;
        case '[':
          store.toggleLeftPanel();
          break;
        case ']':
          store.toggleRightPanel();
          break;
        case '0':
          store.requestFitView();
          break;
        case 'escape':
          if (store.shortcutToolbox?.open) {
            store.closeShortcutToolbox();
          } else if (store.editingOpId) {
            store.closeEditOp();
          } else if (store.seamCommand) {
            store.closeSeamCommand();
          } else {
            store.setActiveTool('select');
          }
          break;
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [setSnapshot]);

  if (error) {
    return (
      <div className="h-screen w-screen flex items-center justify-center p-6">
        <div className="max-w-md rounded-xl border p-6 space-y-3" style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}>
          <h1 className="text-base font-semibold text-rose-600">{t('root.kernelFailed')}</h1>
          <p className="text-sm font-mono-num">{error}</p>
        </div>
      </div>
    );
  }

  if (!ready) {
    return (
      <div className="h-screen w-screen flex items-center justify-center">
        <div className="text-sm font-mono-num" style={{ color: 'var(--text-secondary)' }}>
          {t('root.booting')}
        </div>
      </div>
    );
  }

  const show3d = workspaceMode === 'split' || workspaceMode === '3d';
  const show2d = workspaceMode === 'split' || workspaceMode === '2d';

  return (
    <div
      onPointerMove={(e) => {
        lastCursorRef.current = { x: e.clientX, y: e.clientY };
      }}
      className="h-screen w-screen flex flex-col overflow-hidden relative"
      style={{ background: 'var(--bg-canvas)' }}
    >
      <TopBar />
      {show2d && <ActionRibbon />}

      {/* Non-intrusive floating local autosave toast */}
      {autosaveCandidate && (
        <div
          className="fixed bottom-16 right-4 z-40 rounded-xl border shadow-lg px-3 py-2 flex items-center gap-2.5 text-xs backdrop-blur-md"
          style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
        >
          <span style={{ color: 'var(--text-secondary)' }}>{t('root.autosave')}</span>
          <button
            onClick={() => {
              try {
                setSnapshot(kernel.loadProjectJson(autosaveCandidate));
                requestFitView();
              } finally {
                setAutosaveCandidate(null);
              }
            }}
            className="px-2 py-0.5 rounded font-medium text-white cursor-pointer"
            style={{ background: 'var(--accent)' }}
          >
            {t('root.restore')}
          </button>
          <button
            onClick={() => setAutosaveCandidate(null)}
            className="px-2 py-0.5 rounded border cursor-pointer"
            style={{ borderColor: 'var(--border-subtle)' }}
          >
            {t('root.dismiss')}
          </button>
        </div>
      )}

      <div className="flex-1 flex min-h-0 overflow-hidden">
        {leftPanelOpen && <LeftPanel />}

        {/* Main Stage: 3D LEFT | Draggable Divider | 2D RIGHT */}
        <div ref={stageRef} className="flex-1 flex min-w-0 h-full overflow-hidden">
          {show3d && (
            <Suspense
              fallback={
                <div
                  style={{
                    width: workspaceMode === 'split' ? `${(splitRatio * 100).toFixed(1)}%` : '100%',
                    borderColor: 'var(--border-strong)',
                    color: 'var(--text-secondary)',
                  }}
                  className="h-full flex items-center justify-center text-xs font-mono-num"
                >
                  {t('root.loading3d')}
                </div>
              }
            >
              <LazyViewport3D
                style={{
                  width: workspaceMode === 'split' ? `${(splitRatio * 100).toFixed(1)}%` : '100%',
                  flexShrink: 0,
                }}
              />
            </Suspense>
          )}

          {workspaceMode === 'split' && (
            <div
              onPointerDown={() => {
                draggingSplitRef.current = true;
              }}
              title={t('root.dragSplit')}
              className="w-1.5 h-full shrink-0 cursor-col-resize hover:bg-[var(--accent)]/40 transition flex items-center justify-center"
              style={{ background: 'var(--border-subtle)' }}
            >
              <div className="w-0.5 h-10 rounded-full bg-black/20 dark:bg-white/20" />
            </div>
          )}

          {show2d && <Viewport2D />}
        </div>

        {rightPanelOpen && <RightInspector />}
      </div>

      {/* Fusion 360 Bottom Parametric Feature Timeline */}
      <ParametricTimeline />
      <StatusBar />
      <ExportModal />
      <SettingsModal />
      <ShortcutToolbox />
    </div>
  );
};

export default StudioRoot;
