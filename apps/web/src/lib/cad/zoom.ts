/**
 * Shared wheel/pinch zoom helpers for 2D/3D viewports and global browser zoom locking.
 */

export interface WheelDeltaInput {
  deltaY: number;
  deltaMode: number;
}

export interface WheelZoomInput extends WheelDeltaInput {
  ctrlKey: boolean;
  metaKey: boolean;
}

/** Normalize browser wheel deltaY across pixel (0), line (1), and page (2) modes into px. */
export function normalizeWheelDeltaY(e: WheelDeltaInput): number {
  if (e.deltaMode === 1) return e.deltaY * 16;
  if (e.deltaMode === 2) return e.deltaY * 400;
  return e.deltaY;
}

/**
 * Convert a wheel or trackpad-pinch (ctrlKey/metaKey) event into a smooth multiplicative
 * zoom factor clamped to [0.75, 1.35]. Values > 1 zoom in; values < 1 zoom out.
 */
export function wheelZoomFactor(e: WheelZoomInput): number {
  const dy = normalizeWheelDeltaY(e);
  const sensitivity = e.ctrlKey || e.metaKey ? 0.01 : 0.0015;
  const factor = Math.exp(-dy * sensitivity);
  return Math.max(0.75, Math.min(1.35, factor));
}

