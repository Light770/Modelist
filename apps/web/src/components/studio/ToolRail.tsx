import React from 'react';
import {
  Bookmark,
  Compass,
  Footprints,
  Hand,
  MousePointer2,
  PenTool,
  Ruler,
  Scissors,
} from 'lucide-react';
import type { ToolId } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

const TOOLS: Array<{ id: ToolId; label: string; keyHint: string; Icon: React.FC<{ className?: string }> }> = [
  { id: 'select', label: 'Select Piece / Edge', keyHint: 'V', Icon: MousePointer2 },
  { id: 'direct_edit', label: 'Direct Vertex & Bezier Edit', keyHint: 'A', Icon: PenTool },
  { id: 'walk_seam', label: 'Walk & True Seam', keyHint: 'W', Icon: Footprints },
  { id: 'dart_pivot', label: 'Pivot Dart', keyHint: 'D', Icon: Compass },
  { id: 'slash_spread', label: 'Slash & Spread Fullness', keyHint: 'S', Icon: Scissors },
  { id: 'notch', label: 'Place Notch on Seam', keyHint: 'N', Icon: Bookmark },
  { id: 'measure', label: 'Caliper Measure', keyHint: 'M', Icon: Ruler },
  { id: 'pan', label: 'Pan Canvas (or Middle/Space Drag)', keyHint: 'H', Icon: Hand },
];

export const ToolRail: React.FC = () => {
  const activeTool = useStudioStore((s) => s.activeTool);
  const setActiveTool = useStudioStore((s) => s.setActiveTool);

  return (
    <aside
      className="w-12 shrink-0 border-r py-2 flex flex-col items-center gap-1.5 select-none"
      style={{
        background: 'var(--bg-surface)',
        borderColor: 'var(--border-subtle)',
      }}
    >
      {TOOLS.map(({ id, label, keyHint, Icon }) => {
        const active = activeTool === id;
        return (
          <button
            key={id}
            onClick={() => setActiveTool(id)}
            title={`${label} (${keyHint})`}
            className="w-9 h-9 rounded-lg flex items-center justify-center relative transition cursor-pointer"
            style={{
              background: active ? 'var(--accent-soft)' : 'transparent',
              color: active ? 'var(--accent)' : 'var(--text-secondary)',
            }}
          >
            <Icon className="w-4 h-4" />
            <span className="absolute bottom-0.5 right-1 text-[8px] font-mono-num opacity-60">
              {keyHint}
            </span>
          </button>
        );
      })}
    </aside>
  );
};
