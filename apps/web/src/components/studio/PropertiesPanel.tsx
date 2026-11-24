import React, { useEffect } from 'react';
import { Footprints, Link2 } from 'lucide-react';
import { formatMm, SEAM_PALETTE, type UiSnapshot } from '../../lib/types';
import { useT, type MessageKey } from '../../lib/i18n';
import { useStudioStore, type PanelPage } from '../../store/studioStore';
import { FloatingPanel } from './FloatingPanel';
import { RightInspector } from './RightInspector';
import { SeamCommandDialog } from './SeamCommandDialog';
import { ToolOptions, TOOLS_WITH_OPTIONS } from './ToolOptions';

const PAGE_TITLES: Record<PanelPage, MessageKey> = {
  params: 'panel.params',
  materials: 'panel.materials',
  grading: 'panel.grading',
  piece: 'panel.piece',
};

/** Selected edge summary with the next sewing step (sew it, or edit / walk its seam). */
const SelectedEdgeCard: React.FC<{ snapshot: UiSnapshot }> = ({ snapshot }) => {
  const t = useT();
  const unit = useStudioStore((s) => s.unit);
  const piece = snapshot.pieces.find((p) => p.id === snapshot.selected_piece_id);
  const edge = piece?.edges.find((e) => e.edge_idx === snapshot.selected_edge_idx);
  if (!piece || !edge) return null;
  const bound = snapshot.stitching_links.find(
    (l) => l.edges_a.some((er) => er.edge_id === edge.edge_id) || l.edges_b.some((er) => er.edge_id === edge.edge_id),
  );
  const st = useStudioStore.getState;
  const btn = 'h-[22px] px-2 rounded inline-flex items-center gap-1 text-[11px] cursor-pointer';
  return (
    <div className="mx-3 mt-3 p-2 rounded border space-y-1.5 text-xs" style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}>
      <div className="flex items-center justify-between gap-2">
        <span className="font-semibold truncate">
          {t('panel.edge')} · {edge.label}
        </span>
        <span className="font-mono-num text-[11px]" style={{ color: 'var(--text-secondary)' }}>
          {formatMm(edge.length_mm, unit, 1)}
        </span>
      </div>
      {bound ? (
        <>
          <div className="flex items-center gap-1.5 text-[11px]" style={{ color: 'var(--text-secondary)' }}>
            <span className="w-2 h-2 rounded-full shrink-0" style={{ background: SEAM_PALETTE[bound.color_idx % SEAM_PALETTE.length] }} />
            <span className="truncate">
              {bound.label} ({bound.ease_mm >= 0 ? '+' : ''}
              {formatMm(bound.ease_mm, unit, 1)})
            </span>
          </div>
          <div className="flex gap-1.5">
            <button onClick={() => st().openEditSeamCommand(bound)} className={`${btn} text-white`} style={{ background: 'var(--accent)' }}>
              <Link2 className="w-3 h-3" />
              {t('v2d.editSeamBtn')}
            </button>
            <button
              onClick={() => st().startWalkSeam(bound.id)}
              className={`${btn} border hover:bg-[var(--bg-hover)]`}
              style={{ borderColor: 'var(--border-subtle)' }}
            >
              <Footprints className="w-3 h-3" />
              {t('v2d.walkBtn')}
            </button>
          </div>
        </>
      ) : (
        <button
          onClick={() =>
            st().openCreateSeamCommand({
              piece_id: piece.id,
              piece_code: piece.code,
              edge_id: edge.edge_id,
              edge_idx: edge.edge_idx,
              edge_label: edge.label,
              length_mm: edge.length_mm,
              dir: 1,
            })
          }
          className={`${btn} text-white`}
          style={{ background: 'var(--accent)' }}
        >
          <Link2 className="w-3 h-3" />
          {t('v2d.sewEdge')}
        </button>
      )}
    </div>
  );
};

/**
 * Right-hand properties panel. It follows what the user is doing: a page opened on purpose
 * (parameters, materials, grading), else the options of the active command, else the selected
 * piece. With nothing to show it disappears, leaving the canvas free.
 */
export const PropertiesPanel: React.FC<{ top: number }> = ({ top }) => {
  const t = useT();
  const snapshot = useStudioStore((s) => s.snapshot);
  const page = useStudioStore((s) => s.panelPage);
  const seamCommand = useStudioStore((s) => s.seamCommand);
  const tool = useStudioStore((s) => s.activeTool);
  const open = useStudioStore((s) => s.rightPanelOpen);
  const width = useStudioStore((s) => s.rightPanelWidth);
  const collapsed = useStudioStore((s) => s.rightPanelCollapsed);
  const dismissedKey = useStudioStore((s) => s.panelDismissedKey);
  const st = useStudioStore.getState;

  const piece = snapshot?.pieces.find((p) => p.id === snapshot.selected_piece_id);

  let ctx: {
    key: string;
    title: string;
    extra?: string;
    body: React.ReactNode;
    fill: boolean;
    scroll: boolean;
    onClose: () => void;
  } | null = null;

  if (page) {
    ctx = {
      key: `page:${page}`,
      title: t(PAGE_TITLES[page]),
      body: <RightInspector view={page} />,
      fill: true,
      scroll: false,
      onClose: () => st().closePanelPage(),
    };
  } else if (seamCommand) {
    ctx = {
      key: 'seam',
      title: seamCommand.mode === 'edit' ? t('seam.editTitle') : t('seam.createTitle'),
      body: <SeamCommandDialog />,
      fill: false,
      scroll: false,
      onClose: () => st().closeSeamCommand(),
    };
  } else if (TOOLS_WITH_OPTIONS.has(tool)) {
    ctx = {
      key: `tool:${tool}`,
      title: t(`cmd.${tool}.label` as MessageKey),
      body: <ToolOptions />,
      fill: false,
      scroll: true,
      // Closing a command's options cancels the command, as in Fusion.
      onClose: () => (tool === 'walk_seam' ? st().stopWalkSeam() : st().setActiveTool('select')),
    };
  } else if (piece && snapshot) {
    const key = `sel:${piece.id}:${snapshot.selected_edge_idx ?? ''}`;
    ctx = {
      key,
      title: piece.name,
      extra: piece.code,
      body: (
        <>
          <SelectedEdgeCard snapshot={snapshot} />
          <RightInspector view="piece" />
        </>
      ),
      fill: true,
      scroll: false,
      onClose: () => st().dismissPanel(key),
    };
  }

  // A dismissal only lasts while the context stays the same.
  const key = ctx?.key ?? null;
  useEffect(() => {
    if (dismissedKey && dismissedKey !== key) st().dismissPanel(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  if (!open || !ctx || dismissedKey === ctx.key) return null;

  return (
    <FloatingPanel
      side="right"
      title={ctx.title}
      width={width}
      collapsed={collapsed}
      onToggleCollapsed={() => st().togglePanelCollapsed('right')}
      onResize={(w) => st().setPanelWidth('right', w)}
      onClose={ctx.onClose}
      closeTitle={t('common.close')}
      top={top}
      fill={ctx.fill}
      scrollBody={ctx.scroll}
      headerExtra={
        ctx.extra ? (
          <span className="text-[10px] font-mono-num pr-1" style={{ color: 'var(--text-muted)' }}>
            {ctx.extra}
          </span>
        ) : null
      }
    >
      {ctx.body}
    </FloatingPanel>
  );
};
