export type SectorDir = 'N' | 'NE' | 'E' | 'SE' | 'S' | 'SW' | 'W' | 'NW';

export interface RadialItem {
  id: string;
  label: string;
  shortLabel?: string;
  commandId?: string;
  subMenu?: 'pattern_ops';
  disabled?: boolean;
}

export type FlickAction =
  | { type: 'command'; commandId: string; label: string }
  | { type: 'repeat' }
  | { type: 'commit' }
  | { type: 'cancel' }
  | { type: 'undo' }
  | { type: 'redo' }
  | { type: 'flip_last' };

export const ALL_SECTORS: SectorDir[] = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];

/** Compass angle in degrees matching screen coords (+X right, +Y down). */
export const SECTOR_ANGLES_DEG: Record<SectorDir, number> = {
  N: -90,
  NE: -45,
  E: 0,
  SE: 45,
  S: 90,
  SW: 135,
  W: 180,
  NW: -135,
};

/**
 * Determines which of the 8 compass sectors `(dx, dy)` falls into, or `null` inside the dead zone.
 */
export function sectorFromDelta(dx: number, dy: number, deadZonePx = 18): SectorDir | null {
  const dist = Math.hypot(dx, dy);
  if (dist < deadZonePx) return null;
  let deg = (Math.atan2(dy, dx) * 180) / Math.PI;
  if (deg < -180) deg += 360;
  if (deg >= -112.5 && deg < -67.5) return 'N';
  if (deg >= -67.5 && deg < -22.5) return 'NE';
  if (deg >= -22.5 && deg < 22.5) return 'E';
  if (deg >= 22.5 && deg < 67.5) return 'SE';
  if (deg >= 67.5 && deg < 112.5) return 'S';
  if (deg >= 112.5 && deg < 157.5) return 'SW';
  if (deg >= -157.5 && deg < -112.5) return 'NW';
  return 'W';
}

/**
 * Recognizes blind Fusion-360 quick-drag flick gestures (RMB drag >= 36 px within 360 ms).
 */
export function detectFlickGesture(
  dx: number,
  dy: number,
  durationMs: number,
  inCommand: boolean,
): FlickAction | null {
  const dist = Math.hypot(dx, dy);
  if (dist < 36 || durationMs > 360) return null;
  const dir = sectorFromDelta(dx, dy, 18);
  if (!dir) return null;

  if (dir === 'N') return { type: 'repeat' };
  if (dir === 'E') return inCommand ? { type: 'commit' } : { type: 'redo' };
  if (dir === 'W') return inCommand ? { type: 'cancel' } : { type: 'undo' };
  if (dir === 'S') {
    return inCommand
      ? { type: 'flip_last' }
      : { type: 'command', commandId: 'sew_segment', label: 'Sew Interfaces' };
  }
  if (dir === 'NE') return { type: 'command', commandId: 'direct_edit', label: 'Edit Curve' };
  if (dir === 'SE') return { type: 'command', commandId: 'walk_seam', label: 'Walk Seam' };
  if (dir === 'SW') return { type: 'command', commandId: 'notch', label: 'Place Notch' };
  if (dir === 'NW') return { type: 'command', commandId: 'fit_view', label: 'Fit View' };
  return null;
}

export function build2DRadialWheel(
  lastCommandLabel: string | null,
  inCommand: boolean,
): Record<SectorDir, RadialItem> {
  return {
    N: {
      id: 'repeat',
      label: lastCommandLabel ? `Repeat: ${lastCommandLabel}` : 'Repeat Last',
      shortLabel: 'Repeat',
      commandId: '__repeat__',
      disabled: !lastCommandLabel,
    },
    NE: {
      id: 'direct_edit',
      label: 'Edit Curve',
      commandId: 'direct_edit',
    },
    E: inCommand
      ? { id: 'ok_cmd', label: 'OK / Commit', commandId: '__commit_command__' }
      : { id: 'redo', label: 'Redo', commandId: 'redo' },
    SE: {
      id: 'sew_segment',
      label: 'Sew Interfaces',
      shortLabel: 'Sew',
      commandId: 'sew_segment',
    },
    S: {
      id: 'pattern_ops',
      label: 'Pattern Ops ▸',
      shortLabel: 'Ops ▸',
      subMenu: 'pattern_ops',
    },
    SW: {
      id: 'walk_seam',
      label: 'Walk Seam',
      commandId: 'walk_seam',
    },
    W: inCommand
      ? { id: 'cancel_cmd', label: 'Cancel Command', shortLabel: 'Cancel', commandId: '__cancel_command__' }
      : { id: 'undo', label: 'Undo', commandId: 'undo' },
    NW: {
      id: 'fit_view',
      label: 'Fit View',
      commandId: 'fit_view',
    },
  };
}

export function buildPatternOpsSubWheel(): Record<SectorDir, RadialItem> {
  return {
    N: { id: 'draft_piece', label: 'Draft Panel', commandId: 'draft_piece' },
    NE: { id: 'style_split', label: 'Style Split', commandId: 'style_split' },
    E: { id: 'dart_pivot', label: 'Pivot Dart', commandId: 'dart_pivot' },
    SE: { id: 'slash_spread', label: 'Slash & Spread', commandId: 'slash_spread' },
    S: { id: 'true_corner', label: 'True Corner', commandId: 'true_corner' },
    SW: { id: 'notch', label: 'Place Notch', commandId: 'notch' },
    W: { id: 'drill_hole', label: 'Drill Hole', commandId: 'drill_hole' },
    NW: { id: 'measure', label: 'Measure', commandId: 'measure' },
  };
}
