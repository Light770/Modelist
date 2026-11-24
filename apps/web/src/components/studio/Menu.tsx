import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronRight } from 'lucide-react';

type IconType = React.FC<{ className?: string; style?: React.CSSProperties }>;

export type MenuEntry =
  | {
      kind?: 'item';
      id: string;
      label: string;
      icon?: IconType;
      shortcut?: string;
      /** Shows a check mark (toggles, current choice). */
      checked?: boolean;
      disabled?: boolean;
      title?: string;
      onSelect?: () => void;
      submenu?: MenuEntry[];
    }
  | { kind: 'separator'; id: string }
  | { kind: 'heading'; id: string; label: string };

/**
 * Desktop dropdown menu, positioned under an anchor rectangle and kept inside the window.
 * Closes on outside click, Escape, or after an item runs. The trigger that opened it carries
 * `data-menu-trigger={triggerId}` so clicking it again toggles instead of re-opening.
 * Rendered into <body> so no panel or toolbar stacking context can cover it.
 */
export const Menu: React.FC<{
  anchor: DOMRect;
  entries: MenuEntry[];
  onClose: () => void;
  triggerId?: string;
  minWidth?: number;
  /** Open to the right of the anchor (submenus) instead of below it. */
  placement?: 'below' | 'right';
  /** Internal: the root menu handles outside clicks for the whole tree. */
  nested?: boolean;
}> = ({ anchor, entries, onClose, triggerId, minWidth = 220, placement = 'below', nested = false }) => {
  const ref = useRef<HTMLDivElement | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>(() =>
    placement === 'below'
      ? { left: anchor.left, top: anchor.bottom + 2 }
      : { left: anchor.right - 2, top: anchor.top - 4 },
  );
  const [openSub, setOpenSub] = useState<{ id: string; rect: DOMRect } | null>(null);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    let { left, top } = pos;
    if (left + r.width > window.innerWidth - 4) {
      left = placement === 'below' ? window.innerWidth - r.width - 4 : anchor.left - r.width + 2;
    }
    if (top + r.height > window.innerHeight - 4) top = Math.max(4, window.innerHeight - r.height - 4);
    if (left !== pos.left || top !== pos.top) setPos({ left, top });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (nested) return;
    const onDown = (e: PointerEvent) => {
      const target = e.target as HTMLElement | null;
      if (target?.closest('[data-menu-root]')) return;
      if (triggerId && target?.closest(`[data-menu-trigger="${triggerId}"]`)) return;
      onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('pointerdown', onDown, true);
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('pointerdown', onDown, true);
      window.removeEventListener('keydown', onKey, true);
    };
  }, [nested, onClose, triggerId]);

  const subEntry = openSub
    ? entries.find((e) => e.kind !== 'separator' && e.kind !== 'heading' && e.id === openSub.id)
    : undefined;
  const submenu = subEntry && subEntry.kind !== 'separator' && subEntry.kind !== 'heading' ? subEntry.submenu : undefined;

  return createPortal(
    <>
      <div
        ref={ref}
        data-menu-root
        role="menu"
        className="floating-panel fixed z-[60] py-1 text-xs select-none"
        style={{ left: pos.left, top: pos.top, minWidth }}
      >
        {entries.map((entry) => {
          if (entry.kind === 'separator') {
            return <div key={entry.id} className="my-1 border-t" style={{ borderColor: 'var(--border-subtle)' }} />;
          }
          if (entry.kind === 'heading') {
            return (
              <div
                key={entry.id}
                className="px-3 pt-1.5 pb-0.5 text-[10px] font-semibold uppercase tracking-wide"
                style={{ color: 'var(--text-muted)' }}
              >
                {entry.label}
              </div>
            );
          }
          const Icon = entry.icon;
          const hasSub = Boolean(entry.submenu?.length);
          const subOpen = openSub?.id === entry.id;
          return (
            <button
              key={entry.id}
              role="menuitem"
              disabled={entry.disabled}
              title={entry.title}
              onPointerEnter={(e) => {
                if (hasSub) setOpenSub({ id: entry.id, rect: e.currentTarget.getBoundingClientRect() });
                else setOpenSub(null);
              }}
              onClick={(e) => {
                if (hasSub) {
                  setOpenSub({ id: entry.id, rect: e.currentTarget.getBoundingClientRect() });
                  return;
                }
                entry.onSelect?.();
                onClose();
              }}
              className="w-full h-7 pl-2 pr-3 flex items-center gap-2 text-left cursor-pointer disabled:cursor-default disabled:opacity-40 hover:bg-[var(--bg-hover)] disabled:hover:bg-transparent"
              style={{ background: subOpen ? 'var(--bg-hover)' : undefined }}
            >
              <span className="w-4 h-4 flex items-center justify-center shrink-0" style={{ color: 'var(--text-secondary)' }}>
                {entry.checked ? (
                  <Check className="w-3.5 h-3.5" style={{ color: 'var(--accent)' }} />
                ) : Icon ? (
                  <Icon className="w-3.5 h-3.5" />
                ) : null}
              </span>
              <span className="flex-1 min-w-0 truncate">{entry.label}</span>
              {entry.shortcut && (
                <span className="pl-4 text-[10px] font-mono-num" style={{ color: 'var(--text-muted)' }}>
                  {entry.shortcut}
                </span>
              )}
              {hasSub && <ChevronRight className="w-3.5 h-3.5 -mr-1" style={{ color: 'var(--text-muted)' }} />}
            </button>
          );
        })}
      </div>
      {openSub && submenu && (
        <Menu
          key={openSub.id}
          nested
          placement="right"
          anchor={openSub.rect}
          entries={submenu}
          onClose={onClose}
          minWidth={240}
        />
      )}
    </>,
    document.body,
  );
};
