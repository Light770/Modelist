import { create } from 'zustand';
import { kernel } from '../lib/kernelClient';
import type {
  DisplayUnit,
  SeamWalkFrame,
  SnapHit,
  ThemeMode,
  ToolId,
  UiSnapshot,
} from '../lib/types';

export interface StudioState {
  ready: boolean;
  error: string | null;
  snapshot: UiSnapshot | null;
  activeTool: ToolId;
  unit: DisplayUnit;
  theme: ThemeMode;
  showSeamAllowance: boolean;
  showNotches: boolean;
  showGrainlines: boolean;
  showGradedNest: boolean;
  showEaseBadges: boolean;
  activeTab: 'measurements' | 'piece' | 'grading' | 'formulas';
  exportModalOpen: boolean;
  activeWalkLinkId: string | null;
  walkProgress: number;
  walkFrame: SeamWalkFrame | null;
  slashCount: number;
  slashSpreadMm: number;
  slashHingeMode: 'edge_hinge' | 'parallel_shift';
  dartPivotAngleDeg: number;
  notchStyle: 'slit' | 'v' | 'u' | 't' | 'castle';
  notchCount: number;
  cursorWorld: { x: number; y: number };
  zoomPercent: number;
  activeSnap: SnapHit | null;
  measureReadout: { dx: number; dy: number; dist: number; angleDeg: number } | null;
  statusMessage: string | null;
  fitViewSeq: number;
  zoomStepDir: number;

  setSnapshot: (s: UiSnapshot) => void;
  setReady: (ready: boolean, error?: string | null) => void;
  setActiveTool: (tool: ToolId) => void;
  setUnit: (unit: DisplayUnit) => void;
  setTheme: (theme: ThemeMode) => void;
  toggleLayer: (key: 'showSeamAllowance' | 'showNotches' | 'showGrainlines' | 'showGradedNest' | 'showEaseBadges') => void;
  setActiveTab: (tab: StudioState['activeTab']) => void;
  setExportModalOpen: (open: boolean) => void;
  startWalkSeam: (linkId: string) => void;
  updateWalkProgress: (u: number) => void;
  stopWalkSeam: () => void;
  setSlashParams: (count: number, spreadMm: number, hinge: 'edge_hinge' | 'parallel_shift') => void;
  setDartPivotAngle: (deg: number) => void;
  setNotchOptions: (style: StudioState['notchStyle'], count: number) => void;
  setCursorTelemetry: (pt: { x: number; y: number }, snap: SnapHit | null, zoomPct?: number) => void;
  setMeasureReadout: (r: StudioState['measureReadout']) => void;
  setStatusMessage: (msg: string | null) => void;
  requestFitView: () => void;
  requestZoomStep: (dir: number) => void;
}

export const useStudioStore = create<StudioState>((set, get) => ({
  ready: false,
  error: null,
  snapshot: null,
  activeTool: 'select',
  unit: 'mm',
  theme: 'paper',
  showSeamAllowance: true,
  showNotches: true,
  showGrainlines: true,
  showGradedNest: true,
  showEaseBadges: true,
  activeTab: 'measurements',
  exportModalOpen: false,
  activeWalkLinkId: null,
  walkProgress: 0.5,
  walkFrame: null,
  slashCount: 3,
  slashSpreadMm: 18,
  slashHingeMode: 'edge_hinge',
  dartPivotAngleDeg: 14,
  notchStyle: 'v',
  notchCount: 1,
  cursorWorld: { x: 0, y: 0 },
  zoomPercent: 100,
  activeSnap: null,
  measureReadout: null,
  statusMessage: null,
  fitViewSeq: 1,
  zoomStepDir: 0,

  setSnapshot: (snapshot) => set({ snapshot }),
  setReady: (ready, error = null) => set({ ready, error }),
  setActiveTool: (activeTool) => {
    const prev = get().activeTool;
    if (prev === 'walk_seam' && activeTool !== 'walk_seam') {
      try {
        kernel.clearSeamWalk();
      } catch {
        // Ignore before init
      }
      set({ activeTool, activeWalkLinkId: null, walkFrame: null });
      return;
    }
    if (activeTool === 'walk_seam' && !get().activeWalkLinkId) {
      const firstLink = get().snapshot?.stitching_links[0]?.id ?? null;
      if (firstLink) {
        const wf = kernel.computeSeamWalk(firstLink, get().walkProgress);
        set({ activeTool, activeWalkLinkId: firstLink, walkFrame: wf });
        return;
      }
    }
    set({ activeTool, measureReadout: null });
  },
  setUnit: (unit) => set({ unit }),
  setTheme: (theme) => {
    document.documentElement.setAttribute('data-theme', theme === 'dark' ? 'dark' : 'light');
    set({ theme });
  },
  toggleLayer: (key) => set((s) => ({ [key]: !s[key] })),
  setActiveTab: (activeTab) => set({ activeTab }),
  setExportModalOpen: (exportModalOpen) => set({ exportModalOpen }),
  startWalkSeam: (linkId) => {
    const frame = kernel.computeSeamWalk(linkId, get().walkProgress);
    set({
      activeTool: 'walk_seam',
      activeWalkLinkId: linkId,
      walkFrame: frame,
    });
  },
  updateWalkProgress: (u) => {
    const linkId = get().activeWalkLinkId;
    if (!linkId) return;
    const clamped = Math.max(0, Math.min(1, u));
    const frame = kernel.computeSeamWalk(linkId, clamped);
    set({ walkProgress: clamped, walkFrame: frame });
  },
  stopWalkSeam: () => {
    kernel.clearSeamWalk();
    set({ activeWalkLinkId: null, walkFrame: null, activeTool: 'select' });
  },
  setSlashParams: (slashCount, slashSpreadMm, slashHingeMode) =>
    set({ slashCount, slashSpreadMm, slashHingeMode }),
  setDartPivotAngle: (dartPivotAngleDeg) => set({ dartPivotAngleDeg }),
  setNotchOptions: (notchStyle, notchCount) => set({ notchStyle, notchCount }),
  setCursorTelemetry: (cursorWorld, activeSnap, zoomPercent) =>
    set((s) => ({
      cursorWorld,
      activeSnap,
      zoomPercent: zoomPercent ?? s.zoomPercent,
    })),
  setMeasureReadout: (measureReadout) => set({ measureReadout }),
  setStatusMessage: (statusMessage) => set({ statusMessage }),
  requestFitView: () => set((s) => ({ fitViewSeq: s.fitViewSeq + 1 })),
  requestZoomStep: (dir) => set((s) => ({ zoomStepDir: s.zoomStepDir + dir })),
}));
