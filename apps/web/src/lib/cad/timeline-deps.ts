export type TimelineCategory = 'seam' | 'cut' | 'curve' | 'marking' | 'transform';

/**
 * Given a dragged item at `fromIdx` dropped at boundary gap `dropSlot` (0..opsLen),
 * returns the final post-removal insertion index `toIdx` used by `reorderOp(fromIdx, toIdx)`.
 */
export function targetIndexFromDropSlot(fromIdx: number, dropSlot: number): number {
  return dropSlot > fromIdx ? dropSlot - 1 : dropSlot;
}

/**
 * Returns whether dropping item `fromIdx` into gap `dropSlot` respects its inclusive
 * dependency bounds `[reorderMin, reorderMax]` and actually moves the item.
 */
export function isDropSlotValid(
  fromIdx: number,
  dropSlot: number,
  reorderMin: number,
  reorderMax: number,
): boolean {
  const toIdx = targetIndexFromDropSlot(fromIdx, dropSlot);
  if (toIdx === fromIdx) return false;
  return toIdx >= reorderMin && toIdx <= reorderMax;
}

/**
 * Computes the next `rollback_index` when clicking timeline transport controls.
 * `null` denotes rolled completely to the end (`totalOps`).
 */
export function nextRollbackStep(
  currentRollback: number | null,
  totalOps: number,
  dir: 'start' | 'prev' | 'next' | 'end',
): number | null {
  if (totalOps <= 0) return null;
  const effective = currentRollback === null ? totalOps : Math.max(0, Math.min(totalOps, currentRollback));
  if (dir === 'start') return 0;
  if (dir === 'end') return null;
  if (dir === 'prev') return Math.max(0, effective - 1);
  // dir === 'next'
  const next = effective + 1;
  return next >= totalOps ? null : next;
}

export function opCategory(kind: string): TimelineCategory {
  if (
    kind === 'add_seam_binding' ||
    kind === 'replace_seam_binding' ||
    kind === 'update_seam_binding' ||
    kind === 'delete_seam_binding'
  ) {
    return 'seam';
  }
  if (kind === 'split_style_line' || kind === 'slash_spread' || kind === 'pivot_dart') {
    return 'cut';
  }
  if (kind === 'add_notch' || kind === 'add_drill_hole' || kind === 'set_grain_rotation' || kind === 'set_seam_allowance') {
    return 'marking';
  }
  if (kind === 'move_piece' || kind === 'mirror_piece') {
    return 'transform';
  }
  return 'curve';
}
