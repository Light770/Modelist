import React, { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import {
  Activity,
  Bookmark,
  Box,
  Check,
  ChevronDown,
  Compass,
  Copy,
  Crosshair,
  Download,
  FileInput,
  FlipHorizontal2,
  FolderOpen,
  Footprints,
  Gauge,
  Grid3x3,
  Hand,
  Layers,
  LayoutGrid,
  Link2,
  MousePointer2,
  Pause,
  PenTool,
  PersonStanding,
  Play,
  Repeat,
  RotateCcw,
  Ruler,
  Save,
  Scan,
  ScanEye,
  Scissors,
  Search,
  Settings,
  Shirt,
  Sparkles,
  SplitSquareVertical,
  SquareCode,
  SquarePlus,
  SwatchBook,
  Table2,
  Variable,
  X,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { importPattern, MOD_LABEL, openProject, saveProject } from '../../lib/projectActions';
import { useCatalogT, useT, type MessageKey } from '../../lib/i18n';
import type { AvatarBodyForm, AvatarPose, Camera3DPreset, FitViewMode, ToolId } from '../../lib/types';
import { useStudioStore, WORKSPACE_TABS, type ToolbarTab, type WorkspaceId } from '../../store/studioStore';
import { Menu, type MenuEntry } from './Menu';
import { TOOLS_WITH_OPTIONS } from './ToolOptions';

type IconType = React.FC<{ className?: string; style?: React.CSSProperties }>;

/** One toolbar command: shown as an icon button, listed in its group menu, explained on hover. */
interface ToolCommand {
  id: string;
  icon: IconType;
  label: string;
  desc?: string;
  shortcut?: string;
  active?: boolean;
  disabled?: boolean;
  /** Listed in the group menu only (keeps the button row short). */
  menuOnly?: boolean;
  run: () => void;
}

interface ToolGroup {
  id: string;
  label: string;
  commands: ToolCommand[];
  /** Compact form controls shown after the buttons (pose, fabric, size). */
  controls?: React.ReactNode;
  /** Highlighted contextual group (the running command). */
  contextual?: boolean;
}

const TAB_LABELS: Record<ToolbarTab, MessageKey> = {
  patterning: 'toolbar.tab.patterning',
  sewing: 'toolbar.tab.sewing',
  styling: 'toolbar.tab.styling',
  rendering: 'toolbar.tab.rendering',
  materials: 'toolbar.tab.materials',
  tools: 'toolbar.tab.tools',
  grading: 'toolbar.tab.grading',
  output: 'toolbar.tab.output',
};

const WORKSPACE_LABELS: Record<WorkspaceId, MessageKey> = {
  design: 'workspace.design',
  production: 'workspace.production',
};

const ToolbarSelect: React.FC<{
  value: string;
  title: string;
  width: number;
  onChange: (v: string) => void;
  children: React.ReactNode;
}> = ({ value, title, width, onChange, children }) => (
  <select
    value={value}
    title={title}
    aria-label={title}
    onChange={(e) => onChange(e.target.value)}
    className="h-[24px] rounded-[3px] border px-1 text-[11px] cursor-pointer outline-none focus:border-[var(--accent)]"
    style={{ width, background: 'var(--bg-surface)', borderColor: 'var(--border-subtle)', color: 'var(--text-primary)' }}
  >
    {children}
  </select>
);

/**
 * Command toolbar under the app bar. The "Design ▾" switch picks a workspace, its tabs pick a
 * set of command groups; a group's label opens a menu listing all of its commands. While a
 * command runs, a highlighted group at the end offers to finish or cancel it.
 */
export const Toolbar: React.FC = () => {
  const t = useT();
  const ct = useCatalogT();
  const workspace = useStudioStore((s) => s.workspace);
  const tab = useStudioStore((s) => s.toolbarTab);
  const setTab = useStudioStore((s) => s.setToolbarTab);
  const setWorkspace = useStudioStore((s) => s.setWorkspace);
  const snapshot = useStudioStore((s) => s.snapshot);
  const activeTool = useStudioStore((s) => s.activeTool);
  const seamCommand = useStudioStore((s) => s.seamCommand);
  const workspaceMode = useStudioStore((s) => s.workspaceMode);
  const playing = useStudioStore((s) => s.sim3dPlaying);
  const viewMode = useStudioStore((s) => s.sim3dViewMode);
  const pose = useStudioStore((s) => s.sim3dPose);
  const bodyForm = useStudioStore((s) => s.sim3dBodyForm);
  const cameraPreset = useStudioStore((s) => s.sim3dCameraPreset);
  const showAvatar = useStudioStore((s) => s.show3dAvatar);
  const showTape = useStudioStore((s) => s.show3dTapeLines);
  const showFloor = useStudioStore((s) => s.show3dFloor);
  const showNest = useStudioStore((s) => s.showGradedNest);
  const panelPage = useStudioStore((s) => s.panelPage);
  const shortcutOpen = useStudioStore((s) => Boolean(s.shortcutToolbox?.open));

  const [openGroup, setOpenGroup] = useState<{ id: string; rect: DOMRect } | null>(null);
  const [wsMenu, setWsMenu] = useState<DOMRect | null>(null);
  const [tip, setTip] = useState<{ cmd: ToolCommand; rect: DOMRect } | null>(null);
  const tipTimer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (tipTimer.current) window.clearTimeout(tipTimer.current);
    },
    [],
  );

  if (!snapshot) return null;

  const st = useStudioStore.getState;
  const show3d = workspaceMode !== '2d';
  const ensure3d = () => {
    if (st().workspaceMode === '2d') st().setWorkspaceMode('split');
  };
  const page = (id: string, icon: IconType, which: 'params' | 'materials' | 'grading' | 'piece', label: MessageKey, desc: MessageKey): ToolCommand => ({
    id,
    icon,
    label: t(label),
    desc: t(desc),
    active: panelPage === which,
    run: () => (st().panelPage === which ? st().closePanelPage() : st().openPanelPage(which)),
  });
  const tool = (id: ToolId, icon: IconType, shortcut?: string): ToolCommand => ({
    id,
    icon,
    label: t(`cmd.${id}.label` as MessageKey),
    desc: t(`cmd.${id}.desc` as MessageKey),
    shortcut,
    active: id === 'sew_segment' ? Boolean(seamCommand) : activeTool === id,
    run: () => st().executeCommand(id),
  });
  const action = (id: string, icon: IconType): ToolCommand => ({
    id,
    icon,
    label: t(`cmd.${id}.label` as MessageKey),
    desc: t(`cmd.${id}.desc` as MessageKey),
    run: () => st().executeCommand(id),
  });
  const hasSelection = Boolean(snapshot.selected_piece_id);
  const mirror = { ...action('mirror_piece', FlipHorizontal2), disabled: !hasSelection };
  const duplicate = { ...action('duplicate_piece', Copy), disabled: !hasSelection };

  const viewModes: Array<[FitViewMode, IconType, MessageKey, MessageKey]> = [
    ['material', Shirt, 'v3d.modeDrape', 'tb.viewDrapeDesc'],
    ['strain', Activity, 'v3d.modeStrain', 'tb.viewStrainDesc'],
    ['tension', Gauge, 'v3d.modeTension', 'tb.viewTensionDesc'],
    ['clearance', Scan, 'v3d.modeEase', 'tb.viewEaseDesc'],
    ['xray', ScanEye, 'v3d.modeXray', 'tb.viewXrayDesc'],
  ];

  const simulateGroup: ToolGroup = {
    id: 'simulate',
    label: t('toolbar.group.simulate'),
    commands: [
      {
        id: 'sim_play',
        icon: show3d && playing ? Pause : Play,
        label: show3d && playing ? t('v3d.pause') : t('v3d.play'),
        desc: t('tb.simPlayDesc'),
        active: show3d && playing,
        run: () => {
          if (!show3d) {
            st().setWorkspaceMode('split');
            st().setSim3dPlaying(true);
          } else {
            st().setSim3dPlaying(!playing);
          }
        },
      },
      {
        id: 'sim_reset',
        icon: RotateCcw,
        label: t('v3d.reset'),
        desc: t('tb.simResetDesc'),
        run: () => {
          ensure3d();
          st().requestSim3dReset();
        },
      },
    ],
  };

  const groupsByTab: Record<ToolbarTab, ToolGroup[]> = {
    patterning: [
      {
        id: 'create',
        label: t('toolbar.group.create'),
        commands: [
          tool('draft_piece', SquarePlus, 'P'),
          tool('style_split', SplitSquareVertical, 'K'),
          tool('notch', Bookmark, 'N'),
          tool('drill_hole', Crosshair, 'B'),
        ],
      },
      {
        id: 'modify',
        label: t('toolbar.group.modify'),
        commands: [
          tool('direct_edit', PenTool, 'A'),
          tool('dart_pivot', Compass, 'D'),
          tool('slash_spread', Scissors, 'F'),
          tool('true_corner', SquareCode, 'T'),
          mirror,
          duplicate,
        ],
      },
      {
        id: 'assemble',
        label: t('toolbar.group.assemble'),
        commands: [tool('sew_segment', Link2, 'E'), action('auto_detect_seams', Sparkles)],
      },
      {
        id: 'select',
        label: t('toolbar.group.select'),
        commands: [tool('select', MousePointer2, 'V'), tool('pan', Hand, 'H')],
      },
      {
        id: 'inspect',
        label: t('toolbar.group.inspect'),
        commands: [tool('measure', Ruler, 'M'), tool('walk_seam', Footprints, 'W')],
      },
      {
        id: 'insert',
        label: t('toolbar.group.insert'),
        commands: [
          {
            id: 'insert_dxf',
            icon: FileInput,
            label: t('tb.insertDxf'),
            desc: t('tb.insertDxfDesc'),
            run: () => void importPattern('dxf'),
          },
          {
            id: 'insert_garmentcode',
            icon: Shirt,
            label: t('tb.insertGc'),
            desc: t('tb.insertGcDesc'),
            run: () => void importPattern('garmentcode'),
          },
        ],
      },
    ],
    sewing: [
      {
        id: 'sew',
        label: t('toolbar.group.sew'),
        commands: [tool('sew_segment', Link2, 'E'), action('auto_detect_seams', Sparkles), tool('walk_seam', Footprints, 'W')],
      },
      {
        id: 'pieces',
        label: t('toolbar.group.pieces'),
        commands: [mirror, duplicate, page('piece_props', Layers, 'piece', 'tb.pieceProps', 'tb.piecePropsDesc')],
      },
      {
        id: 'try',
        label: t('toolbar.group.tryOn'),
        commands: [
          {
            id: 'open_split',
            icon: Box,
            label: t('tb.openFit'),
            desc: t('tb.openFitDesc'),
            shortcut: '2',
            active: show3d,
            run: () => st().toggleView('3d'),
          },
        ],
      },
    ],
    styling: [
      simulateGroup,
      {
        id: 'avatar',
        label: t('toolbar.group.avatar'),
        commands: [],
        controls: (
          <>
            <ToolbarSelect value={bodyForm} width={84} title={t('v3d.bodyForm')} onChange={(v) => st().setSim3dBodyForm(v as AvatarBodyForm)}>
              <option value="female">{t('v3d.female')}</option>
              <option value="neutral">{t('v3d.neutral')}</option>
              <option value="male">{t('v3d.male')}</option>
            </ToolbarSelect>
            <ToolbarSelect value={pose} width={84} title={t('v3d.pose')} onChange={(v) => st().setSim3dPose(v as AvatarPose)}>
              <option value="a_pose">{t('v3d.poseA')}</option>
              <option value="arms_raised">{t('v3d.poseArmsUp')}</option>
              <option value="t_pose">{t('v3d.poseT')}</option>
            </ToolbarSelect>
          </>
        ),
      },
      {
        id: 'fabric_quick',
        label: t('toolbar.group.fabric'),
        commands: [page('fabric_props', SwatchBook, 'materials', 'tb.fabricProps', 'tb.fabricPropsDesc')],
      },
    ],
    rendering: [
      {
        id: 'display',
        label: t('toolbar.group.display'),
        commands: viewModes.map(([mode, icon, label, desc]) => ({
          id: `view_${mode}`,
          icon,
          label: t(label),
          desc: t(desc),
          active: show3d && viewMode === mode,
          run: () => {
            st().setSim3dViewMode(mode);
            ensure3d();
          },
        })),
      },
      {
        id: 'scene',
        label: t('toolbar.group.scene'),
        commands: [
          { id: 'avatar_show', icon: PersonStanding, label: t('v3d.avatarTitle'), active: showAvatar, run: () => st().toggle3dAvatar() },
          { id: 'avatar_tape', icon: Ruler, label: t('v3d.tapeTitle'), active: showTape, run: () => st().toggle3dTapeLines() },
          { id: 'avatar_floor', icon: Grid3x3, label: t('v3d.floorTitle'), active: showFloor, run: () => st().toggle3dFloor() },
        ],
      },
      {
        id: 'camera',
        label: t('toolbar.group.camera'),
        commands: [],
        controls: (
          <ToolbarSelect
            value={cameraPreset}
            width={84}
            title={t('toolbar.group.camera')}
            onChange={(v) => {
              ensure3d();
              st().request3dCamera(v as Camera3DPreset);
            }}
          >
            <option value="front">{t('v3d.front')}</option>
            <option value="three_quarter">{t('v3d.threeQuarter')}</option>
            <option value="left">{t('v3d.side')}</option>
            <option value="back">{t('v3d.back')}</option>
          </ToolbarSelect>
        ),
      },
    ],
    materials: [
      {
        id: 'fabric',
        label: t('toolbar.group.fabric'),
        commands: [page('fabric_props', SwatchBook, 'materials', 'tb.fabricProps', 'tb.fabricPropsDesc')],
        controls: (
          <ToolbarSelect value={snapshot.fabric.id} width={168} title={t('v3d.fabricPreset')} onChange={(v) => st().setSnapshot(kernel.setFabricPreset(v))}>
            {snapshot.fabric_catalog.map((f) => (
              <option key={f.id} value={f.id}>
                {ct('fabricPreset', f.id, f.name).split(' (')[0]}
              </option>
            ))}
          </ToolbarSelect>
        ),
      },
      simulateGroup,
    ],
    tools: [
      {
        id: 'inspect',
        label: t('toolbar.group.inspect'),
        commands: [tool('measure', Ruler, 'M'), tool('walk_seam', Footprints, 'W')],
      },
      {
        id: 'parameters',
        label: t('toolbar.group.parameters'),
        commands: [page('params', Variable, 'params', 'tb.formulas', 'tb.formulasDesc')],
      },
      {
        id: 'commands',
        label: t('toolbar.group.commands'),
        commands: [
          {
            id: 'search',
            icon: Search,
            label: t('toolbar.search'),
            desc: t('ribbon.shortcutsTitle'),
            shortcut: 'S',
            active: shortcutOpen,
            run: () => st().toggleShortcutToolbox(),
          },
          {
            id: 'repeat',
            icon: Repeat,
            label: t('tb.repeat'),
            desc: t('tb.repeatDesc'),
            run: () => st().repeatLastCommand(),
          },
          { id: 'settings', icon: Settings, label: t('settings.title'), run: () => st().setSettingsOpen(true) },
        ],
      },
    ],
    grading: [
      {
        id: 'sizes',
        label: t('toolbar.group.sizes'),
        commands: [
          page('grade_table', Table2, 'grading', 'tb.gradeTable', 'tb.gradeTableDesc'),
          {
            id: 'grade_nest',
            icon: Layers,
            label: t('tb.nest'),
            desc: t('tb.nestDesc'),
            active: showNest,
            run: () => st().toggleLayer('showGradedNest'),
          },
        ],
        controls: (
          <ToolbarSelect value={snapshot.active_size} width={64} title={t('tb.activeSize')} onChange={(v) => st().setSnapshot(kernel.setActiveSize(v))}>
            {snapshot.sizes.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </ToolbarSelect>
        ),
      },
      {
        id: 'body',
        label: t('toolbar.group.body'),
        commands: [page('body_measures', Variable, 'params', 'tb.bodyMeasures', 'tb.bodyMeasuresDesc')],
      },
    ],
    output: [
      {
        id: 'file',
        label: t('toolbar.group.file'),
        commands: [
          { id: 'open', icon: FolderOpen, label: t('topbar.openTitle'), shortcut: `${MOD_LABEL}O`, run: () => void openProject() },
          { id: 'save', icon: Save, label: t('topbar.saveTitle'), shortcut: `${MOD_LABEL}S`, run: () => void saveProject() },
        ],
      },
      {
        id: 'export',
        label: t('toolbar.group.export'),
        commands: [
          {
            id: 'export_files',
            icon: Download,
            label: t('cmd.export.label'),
            desc: t('cmd.export.desc'),
            run: () => st().openExportModal('exports'),
          },
          {
            id: 'export_marker',
            icon: LayoutGrid,
            label: t('tb.marker'),
            desc: t('tb.markerDesc'),
            run: () => st().openExportModal('marker'),
          },
        ],
      },
    ],
  };

  const groups: ToolGroup[] = [...groupsByTab[tab]];

  // Contextual group while a command runs: finish (commit) or cancel it.
  const running = seamCommand ? 'sew_segment' : TOOLS_WITH_OPTIONS.has(activeTool) ? activeTool : null;
  if (running) {
    const finish = () => {
      if (st().seamCommand) {
        const cmd = st().seamCommand!;
        if (cmd.edgesA.length > 0 && cmd.edgesB.length > 0) st().commitSeamCommand();
        else st().closeSeamCommand();
      } else if (running === 'walk_seam') st().stopWalkSeam();
      else st().setActiveTool('select');
    };
    groups.push({
      id: 'running',
      label: t(`cmd.${running}.label` as MessageKey),
      contextual: true,
      commands: [
        { id: 'finish', icon: Check, label: t('panel.finish'), desc: t('tb.finishDesc'), shortcut: 'Enter', run: finish },
        {
          id: 'cancel',
          icon: X,
          label: t('common.cancel'),
          shortcut: 'Esc',
          run: () => (st().seamCommand ? st().closeSeamCommand() : running === 'walk_seam' ? st().stopWalkSeam() : st().setActiveTool('select')),
        },
      ],
    });
  }

  const hideTip = () => {
    if (tipTimer.current) window.clearTimeout(tipTimer.current);
    tipTimer.current = null;
    setTip(null);
  };
  const scheduleTip = (cmd: ToolCommand, el: HTMLElement) => {
    hideTip();
    const rect = el.getBoundingClientRect();
    tipTimer.current = window.setTimeout(() => setTip({ cmd, rect }), 420);
  };

  const groupMenu = openGroup ? groups.find((g) => g.id === openGroup.id) : undefined;
  const menuEntries: MenuEntry[] = (groupMenu?.commands ?? []).map((c) => ({
    id: c.id,
    label: c.label,
    icon: c.icon,
    shortcut: c.shortcut,
    checked: c.active,
    disabled: c.disabled,
    title: c.desc,
    onSelect: c.run,
  }));
  const wsEntries: MenuEntry[] = (Object.keys(WORKSPACE_TABS) as WorkspaceId[]).map((ws) => ({
    id: ws,
    label: t(WORKSPACE_LABELS[ws]),
    title: t(`workspace.${ws}Desc` as MessageKey),
    checked: ws === workspace,
    onSelect: () => setWorkspace(ws),
  }));

  return (
    <div className="shrink-0 border-b select-none" style={{ background: 'var(--toolbar)', borderColor: 'var(--border-strong)' }}>
      {/* Workspace switch + tab strip */}
      <div className="h-[26px] px-1.5 flex items-end gap-0.5">
        <button
          data-menu-trigger="workspace"
          onClick={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            setWsMenu((m) => (m ? null : r));
          }}
          className="h-[22px] mb-0.5 mr-1.5 pl-2 pr-1 flex items-center gap-1 rounded-[3px] border text-[11px] font-semibold uppercase tracking-wide cursor-pointer hover:bg-[var(--bg-hover)]"
          style={{ borderColor: 'var(--border-strong)', background: 'var(--bg-surface)', color: 'var(--text-primary)' }}
          title={t('workspace.title')}
          aria-haspopup="menu"
        >
          {t(WORKSPACE_LABELS[workspace])}
          <ChevronDown className="w-3 h-3 opacity-70" />
        </button>
        {wsMenu && <Menu anchor={wsMenu} entries={wsEntries} triggerId="workspace" onClose={() => setWsMenu(null)} minWidth={200} />}
        <div className="flex items-end gap-0.5" role="tablist">
          {WORKSPACE_TABS[workspace].map((id) => {
            const active = tab === id;
            return (
              <button
                key={id}
                role="tab"
                aria-selected={active}
                onClick={() => {
                  setTab(id);
                  setOpenGroup(null);
                }}
                className="h-[24px] px-2.5 text-[11px] font-semibold uppercase tracking-wide cursor-pointer border-b-2 hover:text-[var(--text-primary)]"
                style={{
                  color: active ? 'var(--accent)' : 'var(--text-secondary)',
                  borderColor: active ? 'var(--accent)' : 'transparent',
                }}
              >
                {t(TAB_LABELS[id])}
              </button>
            );
          })}
        </div>
      </div>

      {/* Command groups */}
      <div className="h-[50px] px-1.5 flex items-stretch border-t" style={{ borderColor: 'var(--border-subtle)' }}>
        {groups.map((g, gi) => (
          <React.Fragment key={g.id}>
            {gi > 0 && <div className="w-px my-1.5" style={{ background: 'var(--border-subtle)' }} />}
            <div
              className="px-1 flex flex-col items-center justify-between pt-1 pb-0.5"
              style={g.contextual ? { background: 'var(--accent-soft)', borderRadius: 3, margin: '3px 2px' } : undefined}
            >
              <div className="flex items-center gap-0.5">
                {g.commands
                  .filter((c) => !c.menuOnly)
                  .map((c) => {
                    const Icon = c.icon;
                    return (
                      <button
                        key={c.id}
                        onClick={() => {
                          hideTip();
                          c.run();
                        }}
                        onPointerEnter={(e) => scheduleTip(c, e.currentTarget)}
                        onPointerLeave={hideTip}
                        disabled={c.disabled}
                        aria-label={c.label}
                        aria-pressed={c.active}
                        className="w-[30px] h-[28px] flex items-center justify-center rounded-[3px] cursor-pointer disabled:cursor-default disabled:opacity-35 hover:bg-[var(--bg-hover)] disabled:hover:bg-transparent"
                        style={{
                          background: c.active ? 'var(--accent-soft)' : undefined,
                          color: g.contextual ? 'var(--accent)' : c.active ? 'var(--accent)' : 'var(--text-primary)',
                          boxShadow: c.active ? 'inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent)' : undefined,
                        }}
                      >
                        <Icon className="w-[18px] h-[18px]" />
                      </button>
                    );
                  })}
                {g.controls && <div className="ml-1 flex items-center gap-1">{g.controls}</div>}
              </div>
              <button
                data-menu-trigger={`group-${g.id}`}
                onClick={(e) => {
                  const rect = e.currentTarget.getBoundingClientRect();
                  setOpenGroup((o) => (o?.id === g.id ? null : { id: g.id, rect }));
                  hideTip();
                }}
                className="h-[15px] max-w-[160px] px-1 flex items-center gap-0.5 rounded-[2px] text-[10px] font-medium uppercase tracking-wide cursor-pointer hover:bg-[var(--bg-hover)] hover:text-[var(--text-primary)]"
                style={{
                  color: g.contextual ? 'var(--accent)' : 'var(--text-secondary)',
                  background: openGroup?.id === g.id ? 'var(--bg-hover)' : undefined,
                }}
                aria-haspopup="menu"
              >
                <span className="truncate">{g.label}</span>
                <ChevronDown className="w-2.5 h-2.5 shrink-0" />
              </button>
            </div>
          </React.Fragment>
        ))}

        {/* Command search (S) at the far end, like Fusion's toolbox */}
        <div className="ml-auto px-1 flex items-center">
          <button
            onClick={() => st().toggleShortcutToolbox()}
            className="h-[26px] pl-2 pr-1.5 flex items-center gap-1.5 rounded-[3px] border text-[11px] cursor-pointer hover:bg-[var(--bg-hover)]"
            style={{
              borderColor: shortcutOpen ? 'var(--accent)' : 'var(--border-subtle)',
              background: 'var(--bg-surface)',
              color: 'var(--text-secondary)',
            }}
            title={t('ribbon.shortcutsTitle')}
          >
            <Search className="w-3.5 h-3.5" />
            <span>{t('toolbar.search')}</span>
            <kbd className="ml-2 text-[9px] font-mono-num px-1 rounded-[2px] border" style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-muted)' }}>
              S
            </kbd>
          </button>
        </div>
      </div>

      {openGroup && groupMenu && (
        <Menu
          key={openGroup.id}
          anchor={openGroup.rect}
          entries={menuEntries}
          triggerId={`group-${openGroup.id}`}
          onClose={() => setOpenGroup(null)}
          minWidth={240}
        />
      )}

      {tip &&
        createPortal(
          <div
            className="floating-panel fixed z-[70] px-2.5 py-2 text-xs pointer-events-none"
            style={{ left: Math.min(tip.rect.left, window.innerWidth - 280), top: tip.rect.bottom + 22, width: 264 }}
            role="tooltip"
          >
            <div className="flex items-center justify-between gap-2">
              <span className="font-semibold">{tip.cmd.label}</span>
              {tip.cmd.shortcut && (
                <kbd className="text-[10px] font-mono-num px-1 rounded-[2px] border" style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}>
                  {tip.cmd.shortcut}
                </kbd>
              )}
            </div>
            {tip.cmd.desc && (
              <p className="mt-1 leading-snug" style={{ color: 'var(--text-secondary)' }}>
                {tip.cmd.desc}
              </p>
            )}
          </div>,
          document.body,
        )}
    </div>
  );
};
