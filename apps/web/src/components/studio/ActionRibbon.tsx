import React, { useEffect, useRef, useState } from 'react';
import {
  Bookmark,
  ChevronDown,
  Compass,
  Copy,
  Crosshair,
  FlipHorizontal2,
  Footprints,
  Hand,
  Link2,
  MousePointer2,
  PenTool,
  Ruler,
  Scissors,
  Sparkles,
  SplitSquareVertical,
  SquareCode,
  SquarePlus,
} from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import type { ToolId } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';
import { useT } from '../../lib/i18n';

type MenuGroup = 'create' | 'modify' | 'assemble' | 'inspect' | null;

export const ActionRibbon: React.FC = () => {
  const t = useT();
  const activeTool = useStudioStore((s) => s.activeTool);
  const seamCommand = useStudioStore((s) => s.seamCommand);
  const snapshot = useStudioStore((s) => s.snapshot);
  const setActiveTool = useStudioStore((s) => s.setActiveTool);
  const openCreateSeamCommand = useStudioStore((s) => s.openCreateSeamCommand);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);

  const [openMenu, setOpenMenu] = useState<MenuGroup>(null);
  const ribbonRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const onDocClick = (e: MouseEvent) => {
      if (ribbonRef.current && !ribbonRef.current.contains(e.target as Node)) {
        setOpenMenu(null);
      }
    };
    window.addEventListener('pointerdown', onDocClick);
    return () => window.removeEventListener('pointerdown', onDocClick);
  }, []);

  const pickTool = (tool: ToolId) => {
    setActiveTool(tool);
    setOpenMenu(null);
  };

  const selectedPiece = snapshot?.pieces.find((p) => p.id === snapshot.selected_piece_id);

  const btnStyle = (active: boolean): React.CSSProperties => ({
    background: active ? 'var(--accent-soft)' : 'transparent',
    color: active ? 'var(--accent)' : 'var(--text-primary)',
    borderColor: active ? 'var(--accent)' : 'transparent',
  });

  return (
    <div
      ref={ribbonRef}
      className="h-10 shrink-0 border-b px-2.5 flex items-center gap-1 select-none text-xs relative z-20"
      style={{
        background: 'var(--bg-panel)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {/* GROUP 1: CREATE */}
      <div className="relative flex items-center gap-0.5 pr-2 border-r" style={{ borderColor: 'var(--border-subtle)' }}>
        <button
          onClick={() => pickTool('draft_piece')}
          className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md border font-medium hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={btnStyle(activeTool === 'draft_piece')}
          title={t('ribbon.draftTitle')}
        >
          <SquarePlus className="w-3.5 h-3.5" />
          <span>{t('ribbon.draft')}</span>
        </button>
        <button
          onClick={() => setOpenMenu(openMenu === 'create' ? null : 'create')}
          className="inline-flex items-center gap-0.5 px-1.5 py-1 rounded-md text-[10px] font-semibold uppercase tracking-wider hover:bg-[var(--bg-subtle)] cursor-pointer"
          style={{ color: 'var(--text-secondary)' }}
        >
          <span>{t('ribbon.create')}</span>
          <ChevronDown className="w-3 h-3" />
        </button>

        {openMenu === 'create' && (
          <div
            className="absolute left-0 top-full mt-1 w-56 rounded-xl border shadow-xl py-1.5 z-40"
            style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
          >
            <MenuItem
              Icon={SquarePlus}
              title={t('ribbon.newPanel')}
              desc={t('ribbon.newPanelDesc')}
              shortcut="P"
              active={activeTool === 'draft_piece'}
              onClick={() => pickTool('draft_piece')}
            />
            <MenuItem
              Icon={SplitSquareVertical}
              title={t('ribbon.styleSplit')}
              desc={t('ribbon.styleSplitDesc')}
              shortcut="K"
              active={activeTool === 'style_split'}
              onClick={() => pickTool('style_split')}
            />
            <MenuItem
              Icon={Bookmark}
              title={t('ribbon.notch')}
              desc={t('ribbon.notchDesc')}
              shortcut="N"
              active={activeTool === 'notch'}
              onClick={() => pickTool('notch')}
            />
            <MenuItem
              Icon={Crosshair}
              title={t('ribbon.drill')}
              desc={t('ribbon.drillDesc')}
              shortcut="B"
              active={activeTool === 'drill_hole'}
              onClick={() => pickTool('drill_hole')}
            />
          </div>
        )}
      </div>

      {/* GROUP 2: MODIFY */}
      <div className="relative flex items-center gap-0.5 px-1.5 border-r" style={{ borderColor: 'var(--border-subtle)' }}>
        <button
          onClick={() => pickTool('select')}
          className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md border font-medium hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={btnStyle(activeTool === 'select')}
          title={t('ribbon.selectTitle')}
        >
          <MousePointer2 className="w-3.5 h-3.5" />
          <span>{t('ribbon.select')}</span>
        </button>
        <button
          onClick={() => pickTool('direct_edit')}
          className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md border font-medium hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={btnStyle(activeTool === 'direct_edit')}
          title={t('ribbon.editCurveTitle')}
        >
          <PenTool className="w-3.5 h-3.5" />
          <span>{t('ribbon.editCurve')}</span>
        </button>
        <button
          onClick={() => pickTool('true_corner')}
          className="hidden lg:inline-flex items-center gap-1.5 px-2 py-1 rounded-md border font-medium hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={btnStyle(activeTool === 'true_corner')}
          title={t('ribbon.trueCornerTitle')}
        >
          <SquareCode className="w-3.5 h-3.5" />
          <span>{t('ribbon.trueCorner')}</span>
        </button>
        <button
          onClick={() => setOpenMenu(openMenu === 'modify' ? null : 'modify')}
          className="inline-flex items-center gap-0.5 px-1.5 py-1 rounded-md text-[10px] font-semibold uppercase tracking-wider hover:bg-[var(--bg-subtle)] cursor-pointer"
          style={{ color: 'var(--text-secondary)' }}
        >
          <span>{t('ribbon.modify')}</span>
          <ChevronDown className="w-3 h-3" />
        </button>

        {openMenu === 'modify' && (
          <div
            className="absolute left-0 top-full mt-1 w-60 rounded-xl border shadow-xl py-1.5 z-40"
            style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
          >
            <MenuItem
              Icon={SquareCode}
              title={t('ribbon.trueCornerItem')}
              desc={t('ribbon.trueCornerDesc')}
              shortcut="T"
              active={activeTool === 'true_corner'}
              onClick={() => pickTool('true_corner')}
            />
            <MenuItem
              Icon={Compass}
              title={t('ribbon.pivotDart')}
              desc={t('ribbon.pivotDartDesc')}
              shortcut="D"
              active={activeTool === 'dart_pivot'}
              onClick={() => pickTool('dart_pivot')}
            />
            <MenuItem
              Icon={Scissors}
              title={t('ribbon.slash')}
              desc={t('ribbon.slashDesc')}
              shortcut="F"
              active={activeTool === 'slash_spread'}
              onClick={() => pickTool('slash_spread')}
            />
            {selectedPiece && (
              <>
                <div className="my-1 border-t" style={{ borderColor: 'var(--border-subtle)' }} />
                <MenuItem
                  Icon={FlipHorizontal2}
                  title={t('ribbon.mirrorPiece', { code: selectedPiece.code })}
                  desc={t('ribbon.mirrorDesc')}
                  onClick={() => {
                    setSnapshot(kernel.mirrorPiece(selectedPiece.id));
                    setOpenMenu(null);
                  }}
                />
                <MenuItem
                  Icon={Copy}
                  title={t('ribbon.duplicatePiece', { code: selectedPiece.code })}
                  desc={t('ribbon.duplicateDesc')}
                  onClick={() => {
                    setSnapshot(kernel.duplicatePiece(selectedPiece.id));
                    setOpenMenu(null);
                  }}
                />
              </>
            )}
          </div>
        )}
      </div>

      {/* GROUP 3: ASSEMBLE (INTERDEPENDENT SEAMS — CORE WORKFLOW) */}
      <div className="relative flex items-center gap-0.5 px-1.5 border-r" style={{ borderColor: 'var(--border-subtle)' }}>
        <button
          onClick={() => {
            openCreateSeamCommand();
            setOpenMenu(null);
          }}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md border font-semibold transition cursor-pointer"
          style={
            seamCommand
              ? { background: 'var(--accent)', color: '#ffffff', borderColor: 'var(--accent)' }
              : {
                  background: 'var(--accent-soft)',
                  color: 'var(--accent)',
                  borderColor: 'var(--accent)',
                }
          }
          title={t('ribbon.sewTitle')}
        >
          <Link2 className="w-3.5 h-3.5" />
          <span>{t('ribbon.sew')}</span>
        </button>

        <button
          onClick={() => pickTool('walk_seam')}
          className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md border font-medium hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={btnStyle(activeTool === 'walk_seam')}
          title={t('ribbon.walkTitle')}
        >
          <Footprints className="w-3.5 h-3.5" />
          <span className="hidden sm:inline">{t('ribbon.walk')}</span>
        </button>

        <button
          onClick={() => setOpenMenu(openMenu === 'assemble' ? null : 'assemble')}
          className="inline-flex items-center gap-0.5 px-1.5 py-1 rounded-md text-[10px] font-semibold uppercase tracking-wider hover:bg-[var(--bg-subtle)] cursor-pointer"
          style={{ color: 'var(--text-secondary)' }}
        >
          <span>{t('ribbon.assemble')}</span>
          <ChevronDown className="w-3 h-3" />
        </button>

        {openMenu === 'assemble' && (
          <div
            className="absolute left-0 top-full mt-1 w-64 rounded-xl border shadow-xl py-1.5 z-40"
            style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
          >
            <MenuItem
              Icon={Link2}
              title={t('ribbon.sewItem')}
              desc={t('ribbon.sewItemDesc')}
              shortcut="E"
              active={Boolean(seamCommand)}
              onClick={() => {
                openCreateSeamCommand();
                setOpenMenu(null);
              }}
            />
            <MenuItem
              Icon={Footprints}
              title={t('ribbon.walkItem')}
              desc={t('ribbon.walkItemDesc')}
              shortcut="W"
              active={activeTool === 'walk_seam'}
              onClick={() => pickTool('walk_seam')}
            />
            <MenuItem
              Icon={Sparkles}
              title={t('ribbon.autoDetect')}
              desc={t('ribbon.autoDetectDesc')}
              onClick={() => {
                const { count, snapshot: next } = kernel.autoDetectSeams();
                setSnapshot(next);
                setStatusMessage(
                  count > 0
                    ? t('left.autoDetected', { count })
                    : t('ribbon.allBound'),
                );
                setOpenMenu(null);
              }}
            />
          </div>
        )}
      </div>

      {/* GROUP 4: INSPECT */}
      <div className="flex items-center gap-0.5 pl-1 flex-1">
        <button
          onClick={() => pickTool('measure')}
          className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md border font-medium hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={btnStyle(activeTool === 'measure')}
          title={t('ribbon.measureTitle')}
        >
          <Ruler className="w-3.5 h-3.5" />
          <span className="hidden md:inline">{t('ribbon.measure')}</span>
        </button>
        <button
          onClick={() => pickTool('pan')}
          className="inline-flex items-center gap-1 px-1.5 py-1 rounded-md border font-medium hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={btnStyle(activeTool === 'pan')}
          title={t('ribbon.panTitle')}
        >
          <Hand className="w-3.5 h-3.5" />
        </button>

        <button
          onClick={() => useStudioStore.getState().toggleShortcutToolbox()}
          className="ml-auto inline-flex items-center gap-1.5 px-2 py-1 rounded-md border text-[11px] hover:bg-[var(--bg-subtle)] transition cursor-pointer"
          style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-secondary)' }}
          title={t('ribbon.shortcutsTitle')}
        >
          <span className="hidden sm:inline">{t('ribbon.shortcuts')}</span>
          <kbd
            className="text-[9px] font-mono-num px-1 rounded border"
            style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-muted)' }}
          >
            S
          </kbd>
        </button>
      </div>
    </div>
  );
};

interface MenuItemProps {
  Icon: React.FC<{ className?: string }>;
  title: string;
  desc: string;
  shortcut?: string;
  active?: boolean;
  onClick: () => void;
}

const MenuItem: React.FC<MenuItemProps> = ({ Icon, title, desc, shortcut, active, onClick }) => (
  <button
    onClick={onClick}
    className="w-full text-left px-3 py-1.5 hover:bg-[var(--bg-subtle)] flex items-start gap-2.5 cursor-pointer transition"
    style={{ background: active ? 'var(--accent-soft)' : undefined }}
  >
    <Icon className="w-3.5 h-3.5 mt-0.5 shrink-0 opacity-80" />
    <div className="flex-1 min-w-0">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-xs">{title}</span>
        {shortcut && (
          <kbd
            className="text-[9px] font-mono-num px-1 py-0.2 rounded border"
            style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-muted)' }}
          >
            {shortcut}
          </kbd>
        )}
      </div>
      <div className="text-[10px] leading-tight mt-0.5" style={{ color: 'var(--text-muted)' }}>
        {desc}
      </div>
    </div>
  </button>
);
