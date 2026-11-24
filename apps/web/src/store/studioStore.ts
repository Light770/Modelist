import { create } from 'zustand';
import { kernel } from '../lib/kernelClient';
import { detectLanguage, persistLanguage, type Lang } from '../lib/i18n';
import type {
  AvatarBodyForm,
  AvatarPose,
  Camera3DPreset,
  DisplayUnit,
  EaseSpec,
  EdgeRefInfo,
  FitViewMode,
  SeamDriver,
  SeamType,
  SeamWalkFrame,
  SnapHit,
  StitchLinkSummary,
  ThemeMode,
  ToolId,
  UiSnapshot,
  WorkspaceMode,
} from '../lib/types';

export interface SeamCommandState {
  mode: 'create' | 'edit';
  bindingId: string | null;
  label: string;
  activeSlot: 'a' | 'b';
  edgesA: EdgeRefInfo[];
  edgesB: EdgeRefInfo[];
  driver: SeamDriver;
  ease: EaseSpec;
  seamType: SeamType;
  autoPartnerNotches: boolean;
  syncRecommendedSa: boolean;
}

export interface StudioState {
  ready: boolean;
  error: string | null;
  snapshot: UiSnapshot | null;
  activeTool: ToolId;
  unit: DisplayUnit;
  theme: ThemeMode;
  language: Lang;
  settingsOpen: boolean;

  // Fusion 360 / CLO3D Workspace Layout State
  workspaceMode: WorkspaceMode;
  splitRatio: number;
  leftPanelOpen: boolean;
  rightPanelOpen: boolean;

  // Fusion 360 Floating Seam Interface Command Dialog State
  seamCommand: SeamCommandState | null;

  // 2D Display Overlay Toggles
  showSeamAllowance: boolean;
  showNotches: boolean;
  showGrainlines: boolean;
  showGradedNest: boolean;
  showDimensions: boolean;
  showSeamColors: boolean;
  showMirrorUnfold: boolean;

  // 3D Virtual Fit Room State (strictly decoupled)
  showSim3d: boolean;
  sim3dPose: AvatarPose;
  sim3dBodyForm: AvatarBodyForm;
  sim3dViewMode: FitViewMode;
  show3dAvatar: boolean;
  show3dFloor: boolean;
  show3dTapeLines: boolean;
  sim3dCameraPreset: Camera3DPreset;
  sim3dCameraSeq: number;

  // Inspector & Tool Options
  activeTab: 'measurements' | 'piece' | 'grading' | 'formulas' | 'fabric';
  exportModalOpen: boolean;
  activeWalkLinkId: string | null;
  walkProgress: number;
  walkFrame: SeamWalkFrame | null;
  slashCount: number;
  slashSpreadMm: number;
  slashHingeMode: 'edge_hinge' | 'parallel_shift';
  dartPivotAngleDeg: number;
  trueCornerSquare90: boolean;
  styleSplitAnchor: { pieceId: string; edgeIdx: number; paramT: number } | null;
  notchStyle: 'slit' | 'v' | 'u' | 't' | 'castle';
  notchCount: number;

  // Telemetry & View Controls
  cursorWorld: { x: number; y: number };
  zoomPercent: number;
  activeSnap: SnapHit | null;
  measureReadout: { dx: number; dy: number; dist: number; angleDeg: number } | null;
  statusMessage: string | null;
  fitViewSeq: number;
  zoomStepDir: number;

  // Fission-inspired Muscle-Memory & Timeline State
  lastCommandId: string | null;
  shortcutToolbox: { open: boolean; x: number; y: number } | null;
  editingOpId: string | null;
  timelineFilterBySelectedPiece: boolean;

  setSnapshot: (s: UiSnapshot) => void;
  setReady: (ready: boolean, error?: string | null) => void;
  setActiveTool: (tool: ToolId) => void;
  setUnit: (unit: DisplayUnit) => void;
  setTheme: (theme: ThemeMode) => void;
  setLanguage: (language: Lang) => void;
  setSettingsOpen: (open: boolean) => void;
  setWorkspaceMode: (mode: WorkspaceMode) => void;
  setSplitRatio: (ratio: number) => void;
  toggleLeftPanel: () => void;
  toggleRightPanel: () => void;
  setRightPanelOpen: (open: boolean) => void;

  // Fusion 360 Progressive Seam Command Actions
  openCreateSeamCommand: (initialEdge?: EdgeRefInfo) => void;
  openEditSeamCommand: (link: StitchLinkSummary) => void;
  closeSeamCommand: () => void;
  setSeamActiveSlot: (slot: 'a' | 'b') => void;
  toggleSeamCommandEdge: (edge: EdgeRefInfo) => void;
  flipSeamCommandEdgeDir: (slot: 'a' | 'b', edgeId: string) => void;
  removeSeamCommandEdge: (slot: 'a' | 'b', edgeId: string) => void;
  patchSeamCommand: (patch: Partial<SeamCommandState>) => void;
  commitSeamCommand: () => void;
  deleteActiveSeamCommand: () => void;

  toggleLayer: (
    key:
      | 'showSeamAllowance'
      | 'showNotches'
      | 'showGrainlines'
      | 'showGradedNest'
      | 'showDimensions'
      | 'showSeamColors',
  ) => void;
  toggleMirrorUnfold: () => void;
  toggleSim3d: () => void;
  setSim3dPose: (pose: AvatarPose) => void;
  setSim3dBodyForm: (form: AvatarBodyForm) => void;
  setSim3dViewMode: (mode: FitViewMode) => void;
  toggle3dAvatar: () => void;
  toggle3dFloor: () => void;
  toggle3dTapeLines: () => void;
  request3dCamera: (preset: Camera3DPreset) => void;
  setActiveTab: (tab: StudioState['activeTab']) => void;
  setExportModalOpen: (open: boolean) => void;
  startWalkSeam: (linkId: string) => void;
  updateWalkProgress: (u: number) => void;
  stopWalkSeam: () => void;
  setSlashParams: (count: number, spreadMm: number, hinge: 'edge_hinge' | 'parallel_shift') => void;
  setDartPivotAngle: (deg: number) => void;
  setTrueCornerSquare90: (sq: boolean) => void;
  setStyleSplitAnchor: (anchor: StudioState['styleSplitAnchor']) => void;
  setNotchOptions: (style: StudioState['notchStyle'], count: number) => void;
  setCursorTelemetry: (pt: { x: number; y: number }, snap: SnapHit | null, zoomPct?: number) => void;
  setMeasureReadout: (r: StudioState['measureReadout']) => void;
  setStatusMessage: (msg: string | null) => void;
  requestFitView: () => void;
  requestZoomStep: (dir: number) => void;

  // Fission-inspired Muscle-Memory & Timeline Actions
  openShortcutToolbox: (x?: number, y?: number) => void;
  closeShortcutToolbox: () => void;
  toggleShortcutToolbox: (x?: number, y?: number) => void;
  openEditOp: (opId: string) => void;
  closeEditOp: () => void;
  setTimelineFilterBySelectedPiece: (v: boolean) => void;
  executeCommand: (commandId: string) => void;
  repeatLastCommand: () => void;
}

const REPEATABLE_COMMANDS = new Set([
  'sew_segment',
  'direct_edit',
  'draft_piece',
  'notch',
  'drill_hole',
  'true_corner',
  'dart_pivot',
  'slash_spread',
  'style_split',
  'walk_seam',
  'measure',
  'mirror_piece',
  'duplicate_piece',
  'auto_detect_seams',
]);

function syncDraftPreview(cmd: SeamCommandState | null) {
  try {
    if (!cmd) {
      kernel.setSeamDraftPreview([], []);
    } else {
      kernel.setSeamDraftPreview(
        cmd.edgesA.map((e) => ({ piece_id: e.piece_id, edge_id: e.edge_id, dir: e.dir })),
        cmd.edgesB.map((e) => ({ piece_id: e.piece_id, edge_id: e.edge_id, dir: e.dir })),
      );
    }
  } catch {
    // Ignore before kernel boot
  }
}

export const useStudioStore = create<StudioState>((set, get) => ({
  ready: false,
  error: null,
  snapshot: null,
  activeTool: 'select',
  unit: 'mm',
  theme: 'paper',
  language: typeof window === 'undefined' ? 'en' : detectLanguage(),
  settingsOpen: false,

  workspaceMode: '2d',
  splitRatio: 0.46,
  leftPanelOpen: true,
  rightPanelOpen: true,

  seamCommand: null,

  showSeamAllowance: true,
  showNotches: true,
  showGrainlines: true,
  showGradedNest: true,
  showDimensions: true,
  showSeamColors: true,
  showMirrorUnfold: true,

  showSim3d: false,
  sim3dPose: 'a_pose',
  sim3dBodyForm: 'female',
  sim3dViewMode: 'material',
  show3dAvatar: true,
  show3dFloor: true,
  show3dTapeLines: true,
  sim3dCameraPreset: 'three_quarter',
  sim3dCameraSeq: 1,

  activeTab: 'measurements',
  exportModalOpen: false,
  activeWalkLinkId: null,
  walkProgress: 0.5,
  walkFrame: null,
  slashCount: 3,
  slashSpreadMm: 18,
  slashHingeMode: 'edge_hinge',
  dartPivotAngleDeg: 14,
  trueCornerSquare90: true,
  styleSplitAnchor: null,
  notchStyle: 'v',
  notchCount: 1,
  cursorWorld: { x: 0, y: 0 },
  zoomPercent: 100,
  activeSnap: null,
  measureReadout: null,
  statusMessage: null,
  fitViewSeq: 1,
  zoomStepDir: 0,

  lastCommandId: 'sew_segment',
  shortcutToolbox: null,
  editingOpId: null,
  timelineFilterBySelectedPiece: false,

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
    }
    if (activeTool === 'sew_segment') {
      if (!get().seamCommand) {
        get().openCreateSeamCommand();
      } else {
        set({ activeTool, lastCommandId: 'sew_segment' });
      }
      return;
    }
    // Fusion 360 Auto-Commit rule: if a valid seam command is open and another tool launches, commit it!
    const openSeam = get().seamCommand;
    let nextSnapshot = get().snapshot;
    let autoCommittedMsg: string | null = null;
    if (openSeam) {
      if (openSeam.edgesA.length > 0 && openSeam.edgesB.length > 0) {
        const committed = kernel.upsertSeamBinding({
          bindingId: openSeam.bindingId,
          label: openSeam.label,
          edgesA: openSeam.edgesA.map((e) => ({ piece_id: e.piece_id, edge_id: e.edge_id, dir: e.dir })),
          edgesB: openSeam.edgesB.map((e) => ({ piece_id: e.piece_id, edge_id: e.edge_id, dir: e.dir })),
          driver: openSeam.driver,
          ease: openSeam.ease,
          seamType: openSeam.seamType,
          autoPartnerNotches: openSeam.autoPartnerNotches,
          syncRecommendedSa: openSeam.syncRecommendedSa,
        });
        nextSnapshot = committed.snapshot;
        autoCommittedMsg = `Auto-committed seam "${openSeam.label || 'Custom Seam'}"`;
      } else {
        syncDraftPreview(null);
      }
    }

    const nextLast = REPEATABLE_COMMANDS.has(activeTool) ? activeTool : get().lastCommandId;

    if (activeTool === 'walk_seam' && !get().activeWalkLinkId) {
      const firstLink =
        nextSnapshot?.selected_binding_id ?? nextSnapshot?.stitching_links[0]?.id ?? null;
      if (firstLink) {
        const wf = kernel.computeSeamWalk(firstLink, get().walkProgress);
        set({
          snapshot: nextSnapshot,
          activeTool,
          seamCommand: null,
          activeWalkLinkId: firstLink,
          walkFrame: wf,
          styleSplitAnchor: null,
          lastCommandId: nextLast,
          ...(autoCommittedMsg ? { statusMessage: autoCommittedMsg } : {}),
        });
        return;
      }
    }
    set({
      snapshot: nextSnapshot,
      activeTool,
      seamCommand: null,
      activeWalkLinkId: activeTool === 'walk_seam' ? get().activeWalkLinkId : null,
      walkFrame: activeTool === 'walk_seam' ? get().walkFrame : null,
      measureReadout: null,
      styleSplitAnchor: null,
      lastCommandId: nextLast,
      ...(autoCommittedMsg ? { statusMessage: autoCommittedMsg } : {}),
    });
  },
  setUnit: (unit) => set({ unit }),
  setLanguage: (language) => {
    persistLanguage(language);
    set({ language });
  },
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setTheme: (theme) => {
    document.documentElement.setAttribute('data-theme', theme === 'dark' ? 'dark' : 'light');
    set({ theme });
  },
  setWorkspaceMode: (workspaceMode) => {
    const showSim3d = workspaceMode !== '2d';
    set((s) => ({
      workspaceMode,
      showSim3d,
      fitViewSeq: workspaceMode !== '3d' ? s.fitViewSeq + 1 : s.fitViewSeq,
    }));
  },
  setSplitRatio: (ratio) => set({ splitRatio: Math.max(0.24, Math.min(0.76, ratio)) }),
  toggleLeftPanel: () => set((s) => ({ leftPanelOpen: !s.leftPanelOpen, fitViewSeq: s.fitViewSeq + 1 })),
  toggleRightPanel: () => set((s) => ({ rightPanelOpen: !s.rightPanelOpen, fitViewSeq: s.fitViewSeq + 1 })),
  setRightPanelOpen: (rightPanelOpen) =>
    set((s) => ({ rightPanelOpen, fitViewSeq: s.fitViewSeq + 1 })),

  openCreateSeamCommand: (initialEdge) => {
    const cmd: SeamCommandState = {
      mode: 'create',
      bindingId: null,
      label: initialEdge ? `${initialEdge.piece_code} ${initialEdge.edge_label} Seam` : '',
      activeSlot: initialEdge ? 'b' : 'a',
      edgesA: initialEdge ? [initialEdge] : [],
      edgesB: [],
      driver: 'driver_a',
      ease: { kind: 'exact' },
      seamType: 'plain',
      autoPartnerNotches: true,
      syncRecommendedSa: false,
    };
    syncDraftPreview(cmd);
    set({
      activeTool: 'sew_segment',
      lastCommandId: 'sew_segment',
      seamCommand: cmd,
      statusMessage: initialEdge
        ? `Interface A locked (${initialEdge.piece_code} ${initialEdge.edge_label}) — click partner edge(s) for Interface B`
        : 'Select one or more edges on the 2D canvas for Interface A',
    });
  },

  openEditSeamCommand: (link) => {
    const cmd: SeamCommandState = {
      mode: 'edit',
      bindingId: link.id,
      label: link.label,
      activeSlot: 'a',
      edgesA: link.edges_a.map((e) => ({ ...e })),
      edgesB: link.edges_b.map((e) => ({ ...e })),
      driver: link.driver,
      ease: link.ease,
      seamType: link.seam_type,
      autoPartnerNotches: link.auto_partner_notches,
      syncRecommendedSa: false,
    };
    const nextSnap = kernel.setSelectedBinding(link.id);
    syncDraftPreview(cmd);
    set({
      snapshot: nextSnap,
      activeTool: 'sew_segment',
      seamCommand: cmd,
      statusMessage: `Editing seam "${link.label}" — click 2D edges to add/remove, flip direction, or tune ease`,
    });
  },

  closeSeamCommand: () => {
    syncDraftPreview(null);
    set({ seamCommand: null, activeTool: 'select' });
  },

  setSeamActiveSlot: (activeSlot) => {
    const curr = get().seamCommand;
    if (!curr) return;
    set({ seamCommand: { ...curr, activeSlot } });
  },

  toggleSeamCommandEdge: (edge) => {
    const curr = get().seamCommand;
    if (!curr) return;
    const slot = curr.activeSlot;
    const inA = curr.edgesA.some((e) => e.edge_id === edge.edge_id);
    const inB = curr.edgesB.some((e) => e.edge_id === edge.edge_id);

    let edgesA = [...curr.edgesA];
    let edgesB = [...curr.edgesB];
    let nextSlot: 'a' | 'b' = slot;

    if (slot === 'a') {
      if (inA) {
        edgesA = edgesA.filter((e) => e.edge_id !== edge.edge_id);
      } else {
        edgesB = edgesB.filter((e) => e.edge_id !== edge.edge_id);
        edgesA.push(edge);
        // Progressive auto-advance on 1st edge only when B is still empty & creating a simple seam?
        // Keep A active if user wants multi-edge OR show quick Tab hint — actually Fusion 360 stays in the collector slot so M:N works effortlessly, and highlights "Interface B" ready!
      }
    } else {
      if (inB) {
        edgesB = edgesB.filter((e) => e.edge_id !== edge.edge_id);
      } else {
        edgesA = edgesA.filter((e) => e.edge_id !== edge.edge_id);
        edgesB.push(edge);
      }
    }

    let nextLabel = curr.label;
    if (curr.mode === 'create' && edgesA.length > 0 && edgesB.length > 0 && !curr.label.includes('↔')) {
      nextLabel = `${edgesA[0].piece_code} ${edgesA[0].edge_label} ↔ ${edgesB[0].piece_code} ${edgesB[0].edge_label}`;
    }

    const updated: SeamCommandState = {
      ...curr,
      edgesA,
      edgesB,
      activeSlot: nextSlot,
      label: nextLabel,
    };
    syncDraftPreview(updated);
    set({ seamCommand: updated });
  },

  flipSeamCommandEdgeDir: (slot, edgeId) => {
    const curr = get().seamCommand;
    if (!curr) return;
    const flip = (list: EdgeRefInfo[]) =>
      list.map((e) => (e.edge_id === edgeId ? { ...e, dir: (e.dir === 1 ? -1 : 1) as 1 | -1 } : e));
    const updated: SeamCommandState = {
      ...curr,
      edgesA: slot === 'a' ? flip(curr.edgesA) : curr.edgesA,
      edgesB: slot === 'b' ? flip(curr.edgesB) : curr.edgesB,
    };
    syncDraftPreview(updated);
    set({ seamCommand: updated });
  },

  removeSeamCommandEdge: (slot, edgeId) => {
    const curr = get().seamCommand;
    if (!curr) return;
    const updated: SeamCommandState = {
      ...curr,
      edgesA: slot === 'a' ? curr.edgesA.filter((e) => e.edge_id !== edgeId) : curr.edgesA,
      edgesB: slot === 'b' ? curr.edgesB.filter((e) => e.edge_id !== edgeId) : curr.edgesB,
    };
    syncDraftPreview(updated);
    set({ seamCommand: updated });
  },

  patchSeamCommand: (patch) => {
    const curr = get().seamCommand;
    if (!curr) return;
    const updated = { ...curr, ...patch };
    syncDraftPreview(updated);
    set({ seamCommand: updated });
  },

  commitSeamCommand: () => {
    const curr = get().seamCommand;
    if (!curr || curr.edgesA.length === 0 || curr.edgesB.length === 0) return;
    const { snapshot } = kernel.upsertSeamBinding({
      bindingId: curr.bindingId,
      label: curr.label,
      edgesA: curr.edgesA.map((e) => ({ piece_id: e.piece_id, edge_id: e.edge_id, dir: e.dir })),
      edgesB: curr.edgesB.map((e) => ({ piece_id: e.piece_id, edge_id: e.edge_id, dir: e.dir })),
      driver: curr.driver,
      ease: curr.ease,
      seamType: curr.seamType,
      autoPartnerNotches: curr.autoPartnerNotches,
      syncRecommendedSa: curr.syncRecommendedSa,
    });
    set({
      snapshot,
      seamCommand: null,
      activeTool: 'select',
      statusMessage: `${curr.mode === 'edit' ? 'Updated' : 'Created'} seam "${curr.label || 'Custom Seam'}"`,
    });
  },

  deleteActiveSeamCommand: () => {
    const curr = get().seamCommand;
    if (!curr?.bindingId) return;
    syncDraftPreview(null);
    const snapshot = kernel.deleteSeamBinding(curr.bindingId);
    set({
      snapshot,
      seamCommand: null,
      activeTool: 'select',
      statusMessage: `Deleted seam "${curr.label}"`,
    });
  },

  toggleLayer: (key) => set((s) => ({ [key]: !s[key] })),
  toggleMirrorUnfold: () => {
    const next = !get().showMirrorUnfold;
    const snap = kernel.setShowMirrorUnfold(next);
    set({ showMirrorUnfold: next, snapshot: snap });
  },
  toggleSim3d: () => {
    const curr = get().workspaceMode;
    get().setWorkspaceMode(curr === '2d' ? 'split' : '2d');
  },
  setSim3dPose: (sim3dPose) => set({ sim3dPose }),
  setSim3dBodyForm: (sim3dBodyForm) => set({ sim3dBodyForm }),
  setSim3dViewMode: (sim3dViewMode) => set({ sim3dViewMode }),
  toggle3dAvatar: () => set((s) => ({ show3dAvatar: !s.show3dAvatar })),
  toggle3dFloor: () => set((s) => ({ show3dFloor: !s.show3dFloor })),
  toggle3dTapeLines: () => set((s) => ({ show3dTapeLines: !s.show3dTapeLines })),
  request3dCamera: (sim3dCameraPreset) =>
    set((s) => ({ sim3dCameraPreset, sim3dCameraSeq: s.sim3dCameraSeq + 1 })),
  setActiveTab: (activeTab) => set({ activeTab }),
  setExportModalOpen: (exportModalOpen) => set({ exportModalOpen }),
  startWalkSeam: (linkId) => {
    syncDraftPreview(null);
    const frame = kernel.computeSeamWalk(linkId, get().walkProgress);
    set({
      seamCommand: null,
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
  setTrueCornerSquare90: (trueCornerSquare90) => set({ trueCornerSquare90 }),
  setStyleSplitAnchor: (styleSplitAnchor) => set({ styleSplitAnchor }),
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

  openShortcutToolbox: (x, y) =>
    set({
      shortcutToolbox: {
        open: true,
        x: x ?? (typeof window !== 'undefined' ? window.innerWidth * 0.5 : 500),
        y: y ?? (typeof window !== 'undefined' ? window.innerHeight * 0.42 : 340),
      },
    }),
  closeShortcutToolbox: () => set({ shortcutToolbox: null }),
  toggleShortcutToolbox: (x, y) => {
    const curr = get().shortcutToolbox;
    if (curr?.open) {
      set({ shortcutToolbox: null });
    } else {
      get().openShortcutToolbox(x, y);
    }
  },
  openEditOp: (editingOpId) => set({ editingOpId }),
  closeEditOp: () => set({ editingOpId: null }),
  setTimelineFilterBySelectedPiece: (timelineFilterBySelectedPiece) =>
    set({ timelineFilterBySelectedPiece }),

  executeCommand: (commandId) => {
    const st = get();
    const recordLast = REPEATABLE_COMMANDS.has(commandId);
    if (recordLast) {
      set({ lastCommandId: commandId });
    }

    switch (commandId) {
      case 'select':
      case 'direct_edit':
      case 'draft_piece':
      case 'style_split':
      case 'notch':
      case 'drill_hole':
      case 'true_corner':
      case 'dart_pivot':
      case 'slash_spread':
      case 'walk_seam':
      case 'measure':
      case 'pan':
        st.setActiveTool(commandId);
        return;
      case 'sew_segment':
        st.openCreateSeamCommand();
        set({ lastCommandId: 'sew_segment' });
        return;
      case 'auto_detect_seams': {
        const { count, snapshot: next } = kernel.autoDetectSeams();
        set({
          snapshot: next,
          statusMessage:
            count > 0
              ? `Auto-detected ${count} matching seam binding(s)`
              : 'All compatible seams are already bound',
        });
        return;
      }
      case 'mirror_piece': {
        const pid = st.snapshot?.selected_piece_id;
        if (!pid) {
          set({ statusMessage: 'Select a pattern piece first to mirror' });
          return;
        }
        set({ snapshot: kernel.mirrorPiece(pid), statusMessage: 'Mirrored selected piece' });
        return;
      }
      case 'duplicate_piece': {
        const pid = st.snapshot?.selected_piece_id;
        if (!pid) {
          set({ statusMessage: 'Select a pattern piece first to duplicate' });
          return;
        }
        set({ snapshot: kernel.duplicatePiece(pid), statusMessage: 'Duplicated selected piece' });
        return;
      }
      case 'fit_view':
        st.requestFitView();
        return;
      case 'toggle_3d':
        st.toggleSim3d();
        return;
      case 'rollback_end':
        set({ snapshot: kernel.setRollbackIndex(null), statusMessage: 'Timeline rolled to end' });
        return;
      case 'undo': {
        const next = kernel.undo();
        if (next) set({ snapshot: next, statusMessage: 'Undid last change' });
        return;
      }
      case 'redo': {
        const next = kernel.redo();
        if (next) set({ snapshot: next, statusMessage: 'Redid change' });
        return;
      }
      case 'export':
        st.setExportModalOpen(true);
        return;
    }
  },

  repeatLastCommand: () => {
    const id = get().lastCommandId;
    if (!id) {
      set({ statusMessage: 'No repeatable command yet' });
      return;
    }
    get().executeCommand(id);
    set({ statusMessage: `Repeated: ${id.replace(/_/g, ' ')}` });
  },
}));
