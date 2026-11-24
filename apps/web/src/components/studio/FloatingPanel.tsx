import React, { useRef } from 'react';
import { ChevronDown, ChevronRight, X } from 'lucide-react';

/**
 * A window that floats over the canvas, docked to its left or right edge (Fusion's browser and
 * command dialogs). It can collapse to its title bar and be resized from its inner edge; the
 * canvas underneath always keeps the full window size.
 */
export const FloatingPanel: React.FC<{
  side: 'left' | 'right';
  title: string;
  width: number;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onResize: (width: number) => void;
  onClose?: () => void;
  closeTitle?: string;
  /** Distance from the canvas top, px. */
  top?: number;
  /** Space kept free at the canvas bottom (navigation bar), px. */
  bottom?: number;
  /** Extra offset from the docked edge, px (to sit beside another panel). */
  edgeOffset?: number;
  headerExtra?: React.ReactNode;
  /** The body scrolls by itself (true) or lets its content manage scrolling (false). */
  scrollBody?: boolean;
  /** Always span the available height (inspector) instead of fitting the content (browser). */
  fill?: boolean;
  children: React.ReactNode;
}> = ({
  side,
  title,
  width,
  collapsed,
  onToggleCollapsed,
  onResize,
  onClose,
  closeTitle,
  top = 8,
  bottom = 52,
  edgeOffset = 0,
  headerExtra,
  scrollBody = true,
  fill = false,
  children,
}) => {
  const dragRef = useRef<{ startX: number; startW: number } | null>(null);

  const onHandleDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    dragRef.current = { startX: e.clientX, startW: width };
  };
  const onHandleMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const d = dragRef.current;
    if (!d) return;
    const dx = e.clientX - d.startX;
    onResize(side === 'left' ? d.startW + dx : d.startW - dx);
  };
  const onHandleUp = () => {
    dragRef.current = null;
  };

  return (
    <section
      data-floating-panel={side}
      className="floating-panel absolute z-20 flex flex-col overflow-hidden select-none"
      style={{
        top,
        [side]: 8 + edgeOffset,
        width,
        ...(collapsed
          ? {}
          : fill
            ? { height: `calc(100% - ${top + bottom}px)` }
            : { maxHeight: `calc(100% - ${top + bottom}px)` }),
      }}
      aria-label={title}
    >
      <header
        className="h-[26px] shrink-0 flex items-center gap-1 pl-1 pr-1 border-b"
        style={{
          background: 'var(--bg-panel)',
          borderColor: collapsed ? 'transparent' : 'var(--border-subtle)',
        }}
        onDoubleClick={onToggleCollapsed}
      >
        <button
          onClick={onToggleCollapsed}
          className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-hover)] cursor-pointer"
          style={{ color: 'var(--text-secondary)' }}
          aria-expanded={!collapsed}
        >
          {collapsed ? <ChevronRight className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
        </button>
        <span
          className="flex-1 min-w-0 truncate text-[11px] font-semibold uppercase tracking-wide"
          style={{ color: 'var(--text-secondary)' }}
        >
          {title}
        </span>
        {headerExtra}
        {onClose && (
          <button
            onClick={onClose}
            className="w-5 h-5 flex items-center justify-center rounded hover:bg-[var(--bg-hover)] cursor-pointer"
            style={{ color: 'var(--text-muted)' }}
            title={closeTitle}
            aria-label={closeTitle}
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </header>
      {!collapsed && (
        <div className={`flex-1 min-h-0 flex flex-col ${scrollBody ? 'overflow-y-auto' : 'overflow-hidden'}`}>
          {children}
        </div>
      )}
      {!collapsed && (
        <div
          onPointerDown={onHandleDown}
          onPointerMove={onHandleMove}
          onPointerUp={onHandleUp}
          className="absolute top-0 bottom-0 w-1.5 cursor-ew-resize hover:bg-[var(--accent-soft)]"
          style={side === 'left' ? { right: 0 } : { left: 0 }}
        />
      )}
    </section>
  );
};
