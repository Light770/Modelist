import React, { Suspense, lazy, useEffect, useRef, useState } from 'react';
import { initKernel, kernel } from '../../lib/kernelClient';
import { loadAutosave } from '../../lib/localFiles';
import { openProject, redo, registerInitialDocument, saveProject, undo } from '../../lib/projectActions';
import { useStudioStore } from '../../store/studioStore';
import { useT } from '../../lib/i18n';
import { AppBar } from './AppBar';
import { BottomBar } from './BottomBar';
import { ExportModal } from './ExportModal';
import { FloatingPanel } from './FloatingPanel';
import { LeftPanel } from './LeftPanel';
import { GarmentPicker } from './templateMenu';
import { DataPanel } from './DataPanel';
import { PropertiesPanel } from './PropertiesPanel';
import { SettingsModal } from './SettingsModal';
import { ShortcutToolbox } from './ShortcutToolbox';
import { Toolbar } from './Toolbar';
import { TOOLS_WITH_OPTIONS } from './ToolOptions';
import { Viewport2D } from './Viewport2D';

// Strictly lazy-loaded 3D Virtual Fit Room (never loaded during 2D drafting!)
const LazyViewport3D = lazy(() => import('./Viewport3D'));

/** Rulers drawn along the top and left edges of the 2D canvas, px (see Viewport2D). */
const RULER_PX = 24;

/**
 * Desktop studio shell: app bar and command toolbar on top, a full-bleed canvas (2D, 3D or
 * both) with the browser and inspector floating over it, and the timeline + status strip at
 * the bottom.
 */
export const StudioRoot: React.FC = () => {
  const t = useT();
  const ready = useStudioStore((s) => s.ready);
  const error = useStudioStore((s) => s.error);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const splitRatio = useStudioStore((s) => s.splitRatio);
  const leftPanelOpen = useStudioStore((s) => s.leftPanelOpen);
  const leftPanelWidth = useStudioStore((s) => s.leftPanelWidth);
  const leftPanelCollapsed = useStudioStore((s) => s.leftPanelCollapsed);
  const activeSize = useStudioStore((s) => s.snapshot?.active_size);
  const setReady = useStudioStore((s) => s.setReady);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setSplitRatio = useStudioStore((s) => s.setSplitRatio);
  const requestFitView = useStudioStore((s) => s.requestFitView);
  const setPanelWidth = useStudioStore((s) => s.setPanelWidth);
  const togglePanelCollapsed = useStudioStore((s) => s.togglePanelCollapsed);
  const toggleLeftPanel = useStudioStore((s) => s.toggleLeftPanel);
  const activeDocId = useStudioStore((s) => s.activeDocId);

  const stageRef = useRef<HTMLDivElement | null>(null);
  const draggingSplitRef = useRef(false);
  const lastCursorRef = useRef<{ x: number; y: number }>({ x: 520, y: 340 });
  const [autosaveCandidate, setAutosaveCandidate] = useState<Uint8Array | null>(null);

  useEffect(() => {
    let active = true;
    const params = new URLSearchParams(window.location.search);
    const tmpl = params.get('template') || 'bodice_sleeve';
    // Shareable deep links: ?mode=3d|split|2d, ?lang=fr|en, ?tab=<inspector tab>
    const mode = params.get('mode');
    if (mode === '2d' || mode === 'split' || mode === '3d') {
      useStudioStore.getState().setWorkspaceMode(mode);
      if (mode !== '2d') useStudioStore.getState().setToolbarTab('styling');
    }
    const lang = params.get('lang');
    if (lang === 'fr' || lang === 'en') {
      useStudioStore.getState().setLanguage(lang);
    }
    const tab = params.get('tab');
    const pages = {
      measurements: 'params',
      formulas: 'params',
      params: 'params',
      fabric: 'materials',
      materials: 'materials',
      grading: 'grading',
    } as const;
    if (tab && tab in pages) {
      useStudioStore.getState().openPanelPage(pages[tab as keyof typeof pages]);
    }

    initKernel(tmpl)
      .then(async (snap) => {
        if (!active) return;
        setSnapshot(registerInitialDocument(snap));
        setReady(true);
        requestFitView();
        const saved = await loadAutosave();
        if (saved && active) {
          // Container bytes, or JSON text from builds before the container.
          const bytes = saved.bytes ?? (saved.jsonText ? new TextEncoder().encode(saved.jsonText) : null);
          if (bytes) setAutosaveCandidate(bytes);
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

  // Draggable 3D | 2D split divider
  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      if (!draggingSplitRef.current || !stageRef.current) return;
      const rect = stageRef.current.getBoundingClientRect();
      if (rect.width < 100) return;
      setSplitRatio((e.clientX - rect.left) / rect.width);
    };
    const onUp = () => {
      if (draggingSplitRef.current) {
        draggingSplitRef.current = false;
        document.body.style.cursor = '';
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

  // Global studio shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT')) {
        return;
      }
      const mod = e.metaKey || e.ctrlKey;
      if (mod) {
        const key = e.key.toLowerCase();
        if (key === 'z') {
          e.preventDefault();
          if (e.shiftKey) redo();
          else undo();
        } else if (key === 'y') {
          e.preventDefault();
          redo();
        } else if (key === 's') {
          e.preventDefault();
          void saveProject();
        } else if (key === 'o') {
          e.preventDefault();
          void openProject();
        } else if (key === 'k') {
          e.preventDefault();
          useStudioStore.getState().toggleShortcutToolbox(lastCursorRef.current.x, lastCursorRef.current.y);
        }
        return;
      }
      if (e.altKey) return;
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
        case '/':
          e.preventDefault();
          document.getElementById('studio-command-line')?.focus();
          break;
        case 'enter':
          // Enter finishes the running tool (the seam dialog handles its own Enter).
          if (!store.seamCommand && TOOLS_WITH_OPTIONS.has(store.activeTool)) {
            if (store.activeTool === 'walk_seam') store.stopWalkSeam();
            else store.setActiveTool('select');
          }
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
  }, []);

  if (error) {
    return (
      <div className="h-screen w-screen flex items-center justify-center p-6">
        <div className="floating-panel max-w-md p-5 space-y-2">
          <h1 className="text-sm font-semibold text-rose-600">{t('root.kernelFailed')}</h1>
          <p className="text-xs font-mono-num">{error}</p>
        </div>
      </div>
    );
  }

  if (!ready) {
    return (
      <div className="h-screen w-screen flex items-center justify-center">
        <div className="text-xs font-mono-num" style={{ color: 'var(--text-secondary)' }}>
          {t('root.booting')}
        </div>
      </div>
    );
  }

  const show3d = workspaceMode === 'split' || workspaceMode === '3d';
  const show2d = workspaceMode === 'split' || workspaceMode === '2d';
  const view3dWidth = workspaceMode === 'split' ? `${(splitRatio * 100).toFixed(1)}%` : '100%';
  // Panels line up inside the 2D rulers when they float over the 2D canvas.
  const leftOver2d = workspaceMode === '2d';
  const rightOver2d = workspaceMode !== '3d';

  return (
    <div
      onPointerMove={(e) => {
        lastCursorRef.current = { x: e.clientX, y: e.clientY };
      }}
      className="h-screen w-screen flex flex-col overflow-hidden"
      style={{ background: 'var(--bg-canvas)' }}
    >
      <AppBar />
      <Toolbar />

      {/* Canvas area: views fill it entirely; panels float on top. */}
      <main ref={stageRef} className="flex-1 min-h-0 relative flex overflow-hidden">
        {show3d && (
          <Suspense
            fallback={
              <div
                style={{ width: view3dWidth, color: 'var(--text-secondary)' }}
                className="h-full shrink-0 flex items-center justify-center text-xs font-mono-num"
              >
                {t('root.loading3d')}
              </div>
            }
          >
            {/* One simulation per document: switching tabs re-creates the 3D view. */}
            <LazyViewport3D key={activeDocId} style={{ width: view3dWidth, flexShrink: 0 }} />
          </Suspense>
        )}

        {workspaceMode === 'split' && (
          <div
            onPointerDown={(e) => {
              e.preventDefault();
              draggingSplitRef.current = true;
              document.body.style.cursor = 'col-resize';
            }}
            onDoubleClick={() => {
              setSplitRatio(0.46);
              requestFitView();
            }}
            title={t('root.dragSplit')}
            className="w-[5px] h-full shrink-0 cursor-col-resize relative group"
            style={{ background: 'var(--toolbar)' }}
          >
            <div
              className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 group-hover:w-[3px] transition-[width]"
              style={{ background: 'var(--border-strong)' }}
            />
          </div>
        )}

        {show2d && <Viewport2D />}

        {leftPanelOpen && (
          <FloatingPanel
            side="left"
            title={t('left.browser')}
            width={leftPanelWidth}
            collapsed={leftPanelCollapsed}
            onToggleCollapsed={() => togglePanelCollapsed('left')}
            onResize={(w) => setPanelWidth('left', w)}
            onClose={toggleLeftPanel}
            closeTitle={t('topbar.toggleBrowser')}
            top={leftOver2d ? RULER_PX + 8 : 8}
            edgeOffset={leftOver2d ? RULER_PX : 0}
            headerExtra={
              activeSize ? (
                <span className="text-[10px] font-mono-num pr-1" style={{ color: 'var(--text-muted)' }}>
                  {t('left.base', { size: activeSize })}
                </span>
              ) : null
            }
          >
            <LeftPanel />
          </FloatingPanel>
        )}

        <PropertiesPanel top={rightOver2d ? RULER_PX + 8 : 8} />
        <DataPanel />
        <GarmentPicker />

        {/* Local autosave offer */}
        {autosaveCandidate && (
          <div
            className="floating-panel absolute bottom-12 left-1/2 -translate-x-1/2 z-40 px-3 py-2 flex items-center gap-2.5 text-xs"
            role="status"
          >
            <span style={{ color: 'var(--text-secondary)' }}>{t('root.autosave')}</span>
            <button
              onClick={() => {
                try {
                  setSnapshot(kernel.loadProjectFile(autosaveCandidate).snapshot);
                  requestFitView();
                } finally {
                  setAutosaveCandidate(null);
                }
              }}
              className="h-[22px] px-2 rounded-[3px] font-medium text-white cursor-pointer"
              style={{ background: 'var(--accent)' }}
            >
              {t('root.restore')}
            </button>
            <button
              onClick={() => setAutosaveCandidate(null)}
              className="h-[22px] px-2 rounded-[3px] border cursor-pointer hover:bg-[var(--bg-hover)]"
              style={{ borderColor: 'var(--border-subtle)' }}
            >
              {t('root.dismiss')}
            </button>
          </div>
        )}
      </main>

      <BottomBar />
      <ExportModal />
      <SettingsModal />
      <ShortcutToolbox />
    </div>
  );
};

export default StudioRoot;
