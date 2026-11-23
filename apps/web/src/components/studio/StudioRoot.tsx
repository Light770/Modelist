import React, { useEffect, useState } from 'react';
import { initKernel, kernel } from '../../lib/kernelClient';
import { loadAutosave } from '../../lib/localFiles';
import { useStudioStore } from '../../store/studioStore';
import { ExportModal } from './ExportModal';
import { LeftPanel } from './LeftPanel';
import { RightInspector } from './RightInspector';
import { StatusBar } from './StatusBar';
import { ToolRail } from './ToolRail';
import { TopBar } from './TopBar';
import { Viewport2D } from './Viewport2D';

export const StudioRoot: React.FC = () => {
  const ready = useStudioStore((s) => s.ready);
  const error = useStudioStore((s) => s.error);
  const setReady = useStudioStore((s) => s.setReady);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const requestFitView = useStudioStore((s) => s.requestFitView);

  const [autosaveCandidate, setAutosaveCandidate] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const params = new URLSearchParams(window.location.search);
    const tmpl = params.get('template') || 'bodice_sleeve';

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
      const store = useStudioStore.getState();
      switch (e.key.toLowerCase()) {
        case 'v':
          store.setActiveTool('select');
          break;
        case 'a':
          store.setActiveTool('direct_edit');
          break;
        case 'w':
          store.setActiveTool('walk_seam');
          break;
        case 'd':
          store.setActiveTool('dart_pivot');
          break;
        case 's':
          store.setActiveTool('slash_spread');
          break;
        case 'n':
          store.setActiveTool('notch');
          break;
        case 'm':
          store.setActiveTool('measure');
          break;
        case 'h':
          store.setActiveTool('pan');
          break;
        case '0':
          store.requestFitView();
          break;
        case 'escape':
          store.setActiveTool('select');
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
          <h1 className="text-base font-semibold text-rose-600">WASM Kernel Failed to Initialize</h1>
          <p className="text-sm font-mono-num">{error}</p>
        </div>
      </div>
    );
  }

  if (!ready) {
    return (
      <div className="h-screen w-screen flex items-center justify-center">
        <div className="text-sm font-mono-num" style={{ color: 'var(--text-secondary)' }}>
          Booting Rust/WASM Pattern Kernel...
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden" style={{ background: 'var(--bg-canvas)' }}>
      <TopBar />

      {autosaveCandidate && (
        <div
          className="px-4 py-1.5 border-b flex items-center justify-between text-xs"
          style={{ background: 'var(--accent-soft)', borderColor: 'var(--border-subtle)' }}
        >
          <span>Local-first autosave snapshot found in browser IndexedDB.</span>
          <div className="flex items-center gap-2">
            <button
              onClick={() => {
                try {
                  setSnapshot(kernel.loadProjectJson(autosaveCandidate));
                  requestFitView();
                } finally {
                  setAutosaveCandidate(null);
                }
              }}
              className="px-2.5 py-0.5 rounded font-medium text-white cursor-pointer"
              style={{ background: 'var(--accent)' }}
            >
              Restore Previous Work
            </button>
            <button
              onClick={() => setAutosaveCandidate(null)}
              className="px-2 py-0.5 rounded border cursor-pointer"
              style={{ borderColor: 'var(--border-strong)' }}
            >
              Dismiss
            </button>
          </div>
        </div>
      )}

      <div className="flex-1 flex min-h-0 overflow-hidden">
        <ToolRail />
        <LeftPanel />
        <Viewport2D />
        <RightInspector />
      </div>

      <StatusBar />
      <ExportModal />
    </div>
  );
};

export default StudioRoot;
