export type CommandCategory = 'Create' | 'Modify' | 'Assemble' | 'Inspect' | 'View';

export interface CommandDef {
  id: string;
  label: string;
  category: CommandCategory;
  shortcut?: string;
  description: string;
  workspaces: Array<'2d' | 'split' | '3d'>;
  repeatable?: boolean;
}

export const MODELIST_COMMANDS: CommandDef[] = [
  {
    id: 'sew_segment',
    label: 'Sew Seam Interfaces',
    category: 'Assemble',
    shortcut: 'E',
    description: 'Link matching 1:1 or M:N edges across pieces with ease rules',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'walk_seam',
    label: 'Walk & True Seam',
    category: 'Assemble',
    shortcut: 'W',
    description: 'Roll mating edges together to inspect ease and corner alignment',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'auto_detect_seams',
    label: 'Auto-Detect Seams',
    category: 'Assemble',
    description: 'Infer matching seam interfaces across unlinked panels',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'select',
    label: 'Select & Move Pieces',
    category: 'Modify',
    shortcut: 'V',
    description: 'Select edges, pieces, and drag panels on the drafting table',
    workspaces: ['2d', 'split'],
  },
  {
    id: 'direct_edit',
    label: 'Edit Vertices & Curves',
    category: 'Modify',
    shortcut: 'A',
    description: 'Drag pattern vertices and Bézier curve handles (Shift for G1)',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'draft_piece',
    label: 'Draft New Panel',
    category: 'Create',
    shortcut: 'P',
    description: 'Drag a parametric rectangle panel (waistband, pocket, facing)',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'style_split',
    label: 'Style-Line Split',
    category: 'Create',
    shortcut: 'K',
    description: 'Cut a piece along a style line into auto-sewn panels',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'notch',
    label: 'Place Notch',
    category: 'Create',
    shortcut: 'N',
    description: 'Add alignment notch along seamline with automatic partner sync',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'drill_hole',
    label: 'Place Drill Hole',
    category: 'Create',
    shortcut: 'B',
    description: 'Mark internal dart apex or pocket punch (DXF Layer 13)',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'true_corner',
    label: 'True Corner (90° / G1)',
    category: 'Modify',
    shortcut: 'T',
    description: 'Square off corner intersections to 90° or smooth G1 tangent',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'dart_pivot',
    label: 'Pivot & Transfer Dart',
    category: 'Modify',
    shortcut: 'D',
    description: 'Rotate dart intake cleanly to another boundary edge',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'slash_spread',
    label: 'Slash & Spread Fullness',
    category: 'Modify',
    shortcut: 'F',
    description: 'Open fullness (flares, gathers, pleats) via hinge or parallel shift',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'mirror_piece',
    label: 'Flip / Mirror Piece',
    category: 'Modify',
    description: 'Horizontally mirror the active selected pattern piece',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'duplicate_piece',
    label: 'Duplicate Piece',
    category: 'Modify',
    description: 'Clone the selected pattern piece onto the drafting table',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'measure',
    label: 'Measure Caliper',
    category: 'Inspect',
    shortcut: 'M',
    description: 'Measure distance, delta X/Y, and angle between any two points',
    workspaces: ['2d', 'split'],
    repeatable: true,
  },
  {
    id: 'pan',
    label: 'Pan Canvas',
    category: 'View',
    shortcut: 'H',
    description: 'Pan across the 2D drafting table (or hold Space + drag)',
    workspaces: ['2d', 'split'],
  },
  {
    id: 'fit_view',
    label: 'Fit All Pieces',
    category: 'View',
    shortcut: '0',
    description: 'Zoom and center all active pattern pieces in the viewport',
    workspaces: ['2d', 'split', '3d'],
  },
  {
    id: 'toggle_3d',
    label: 'Toggle 3D Virtual Fit',
    category: 'View',
    shortcut: '2',
    description: 'Show/hide the decoupled XPBD 3D mannequin fit viewport',
    workspaces: ['2d', 'split', '3d'],
  },
  {
    id: 'rollback_end',
    label: 'Roll Timeline to End',
    category: 'View',
    description: 'Restore timeline rollback marker to the final constructed state',
    workspaces: ['2d', 'split', '3d'],
  },
  {
    id: 'undo',
    label: 'Undo',
    category: 'Modify',
    shortcut: 'Ctrl+Z',
    description: 'Step back one parametric project revision',
    workspaces: ['2d', 'split', '3d'],
  },
  {
    id: 'redo',
    label: 'Redo',
    category: 'Modify',
    shortcut: 'Ctrl+Shift+Z',
    description: 'Re-apply undone parametric revision',
    workspaces: ['2d', 'split', '3d'],
  },
  {
    id: 'export',
    label: 'Export & Marker Nest',
    category: 'View',
    description: 'Export DXF-AAMA, RUL, multi-layer PDF, SVG, or fabric marker BOM',
    workspaces: ['2d', 'split', '3d'],
  },
];

export const DEFAULT_PINS_2D = [
  'sew_segment',
  'direct_edit',
  'draft_piece',
  'notch',
  'dart_pivot',
  'slash_spread',
  'style_split',
  'true_corner',
  'walk_seam',
  'measure',
  'mirror_piece',
  'fit_view',
];

export const DEFAULT_PINS_3D = [
  'toggle_3d',
  'fit_view',
  'sew_segment',
  'walk_seam',
  'export',
  'undo',
];

export function searchCommands(
  query: string,
  workspace: '2d' | 'split' | '3d',
  commands: readonly CommandDef[] = MODELIST_COMMANDS,
): CommandDef[] {
  const available = commands.filter((c) => c.workspaces.includes(workspace));
  const q = query.trim().toLowerCase();
  if (!q) return available;

  const scored: Array<{ score: number; cmd: CommandDef }> = [];
  for (const cmd of available) {
    const label = cmd.label.toLowerCase();
    const sc = (cmd.shortcut ?? '').toLowerCase();
    const cat = cmd.category.toLowerCase();
    const desc = cmd.description.toLowerCase();

    if (sc === q) scored.push({ score: 0, cmd });
    else if (label.startsWith(q)) scored.push({ score: 1, cmd });
    else if (label.split(/\s+/).some((w) => w.startsWith(q))) scored.push({ score: 2, cmd });
    else if (label.includes(q)) scored.push({ score: 3, cmd });
    else if (cat.includes(q) || desc.includes(q)) scored.push({ score: 4, cmd });
  }
  scored.sort((a, b) => a.score - b.score || a.cmd.label.localeCompare(b.cmd.label));
  return scored.map((s) => s.cmd);
}

export function togglePinnedCommand(pins: readonly string[], commandId: string, maxPins = 12): string[] {
  if (pins.includes(commandId)) {
    return pins.filter((id) => id !== commandId);
  }
  const next = [...pins, commandId];
  return next.length > maxPins ? next.slice(next.length - maxPins) : next;
}

export function loadPinnedCommands(storageKey: string, fallback: readonly string[]): string[] {
  try {
    if (typeof window === 'undefined' || !window.localStorage) return [...fallback];
    const raw = window.localStorage.getItem(storageKey);
    if (!raw) return [...fallback];
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed) && parsed.every((x) => typeof x === 'string')) {
      return parsed;
    }
  } catch {
    // Ignore storage access errors in SSR or sandboxed contexts
  }
  return [...fallback];
}

export function savePinnedCommands(storageKey: string, pins: readonly string[]): void {
  try {
    if (typeof window !== 'undefined' && window.localStorage) {
      window.localStorage.setItem(storageKey, JSON.stringify(pins));
    }
  } catch {
    // Ignore storage access errors
  }
}
