import React from 'react';
import {
  Bookmark,
  Compass,
  Crosshair,
  Footprints,
  Hand,
  Link2,
  MousePointer2,
  PenTool,
  Ruler,
  Scissors,
  SplitSquareVertical,
  SquareCode,
  SquarePlus,
} from 'lucide-react';
import type { ToolId } from '../../lib/types';
import { useT, type MessageKey } from '../../lib/i18n';
import { useStudioStore } from '../../store/studioStore';

const TOOL_GROUPS: Array<Array<{ id: ToolId; label: string; keyHint: string; Icon: React.FC<{ className?: string }> }>> = [
  [
    { id: 'select', label: 'Select & Move Piece', keyHint: 'V', Icon: MousePointer2 },
    { id: 'direct_edit', label: 'Edit Vertices & Bézier Curves', keyHint: 'A', Icon: PenTool },
    { id: 'draft_piece', label: 'Draft New Panel (Rectangle)', keyHint: 'P', Icon: SquarePlus },
  ],
  [
    { id: 'sew_segment', label: 'Segment Sewing (Click Edge A -> B)', keyHint: 'E', Icon: Link2 },
    { id: 'walk_seam', label: 'Walk & True Seam', keyHint: 'W', Icon: Footprints },
    { id: 'true_corner', label: 'True Corner (90° Square / G1)', keyHint: 'T', Icon: SquareCode },
  ],
  [
    { id: 'dart_pivot', label: 'Pivot Dart', keyHint: 'D', Icon: Compass },
    { id: 'slash_spread', label: 'Slash & Spread Fullness', keyHint: 'F', Icon: Scissors },
    { id: 'style_split', label: 'Style-Line Split (Yoke / Princess)', keyHint: 'K', Icon: SplitSquareVertical },
  ],
  [
    { id: 'notch', label: 'Place Notch on Seam', keyHint: 'N', Icon: Bookmark },
    { id: 'drill_hole', label: 'Place Drill Hole Punch', keyHint: 'B', Icon: Crosshair },
    { id: 'measure', label: 'Caliper Measure', keyHint: 'M', Icon: Ruler },
    { id: 'pan', label: 'Pan Canvas (or Space Drag)', keyHint: 'H', Icon: Hand },
  ],
];

export const ToolRail: React.FC = () => {
  const activeTool = useStudioStore((s) => s.activeTool);
  const setActiveTool = useStudioStore((s) => s.setActiveTool);
  const t = useT();

  return (
    <aside
      className="w-11 shrink-0 border-r py-1.5 flex flex-col items-center gap-1 select-none overflow-y-auto"
      style={{
        background: 'var(--bg-surface)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {TOOL_GROUPS.map((group, gi) => (
        <React.Fragment key={gi}>
          {gi > 0 && <div className="w-6 my-0.5 border-t" style={{ borderColor: 'var(--border-subtle)' }} />}
          {group.map(({ id, label, keyHint, Icon }) => {
            const active = activeTool === id;
            return (
              <button
                key={id}
                onClick={() => setActiveTool(id)}
                title={`${t(`tool.${id}` as MessageKey) || label} (${keyHint})`}
                className="w-8 h-8 rounded-lg flex items-center justify-center relative transition cursor-pointer shrink-0"
                style={{
                  background: active ? 'var(--accent-soft)' : 'transparent',
                  color: active ? 'var(--accent)' : 'var(--text-secondary)',
                }}
              >
                <Icon className="w-3.5 h-3.5" />
                <span className="absolute bottom-0.5 right-0.5 text-[7.5px] font-mono-num opacity-60">
                  {keyHint}
                </span>
              </button>
            );
          })}
        </React.Fragment>
      ))}
    </aside>
  );
};
