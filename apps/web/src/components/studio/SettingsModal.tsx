import React, { useEffect } from 'react';
import { Languages, Moon, Ruler, Sun, X } from 'lucide-react';
import { LANGUAGES, useT, type Lang } from '../../lib/i18n';
import type { DisplayUnit } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

/** Studio settings: language first, then display preferences. */
export const SettingsModal: React.FC = () => {
  const t = useT();
  const open = useStudioStore((s) => s.settingsOpen);
  const setOpen = useStudioStore((s) => s.setSettingsOpen);
  const language = useStudioStore((s) => s.language);
  const setLanguage = useStudioStore((s) => s.setLanguage);
  const unit = useStudioStore((s) => s.unit);
  const setUnit = useStudioStore((s) => s.setUnit);
  const theme = useStudioStore((s) => s.theme);
  const setTheme = useStudioStore((s) => s.setTheme);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, setOpen]);

  if (!open) return null;

  const row = (icon: React.ReactNode, title: string, help: string, control: React.ReactNode) => (
    <div className="py-3 border-b last:border-b-0 flex items-start justify-between gap-4" style={{ borderColor: 'var(--border-subtle)' }}>
      <div className="min-w-0">
        <div className="flex items-center gap-1.5 text-xs font-semibold">
          {icon}
          {title}
        </div>
        {help && (
          <p className="mt-0.5 text-[11px] leading-snug" style={{ color: 'var(--text-muted)' }}>
            {help}
          </p>
        )}
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );

  const segmented = <V extends string>(value: V, options: Array<[V, string]>, onPick: (v: V) => void) => (
    <div className="flex items-center rounded-md border p-0.5 text-[11px]" style={{ borderColor: 'var(--border-subtle)', background: 'var(--bg-panel)' }}>
      {options.map(([v, label]) => (
        <button
          key={v}
          onClick={() => onPick(v)}
          className="px-2 py-0.5 rounded cursor-pointer"
          style={{
            background: value === v ? 'var(--bg-surface)' : 'transparent',
            color: value === v ? 'var(--accent)' : 'var(--text-secondary)',
            boxShadow: value === v ? '0 1px 2px rgba(0,0,0,0.08)' : 'none',
          }}
        >
          {label}
        </button>
      ))}
    </div>
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-[2px]"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) setOpen(false);
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={t('settings.title')}
        className="w-[440px] max-w-[92vw] rounded-md border shadow-xl"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-strong)' }}
      >
        <div className="px-4 py-3 border-b flex items-center justify-between" style={{ borderColor: 'var(--border-subtle)' }}>
          <h2 className="text-sm font-semibold">{t('settings.title')}</h2>
          <button onClick={() => setOpen(false)} className="p-1 rounded hover:bg-[var(--bg-subtle)] cursor-pointer" title={t('common.close')}>
            <X className="w-4 h-4" />
          </button>
        </div>
        <div className="px-4">
          {row(
            <Languages className="w-3.5 h-3.5" />,
            t('settings.language'),
            t('settings.languageHelp'),
            <select
              value={language}
              onChange={(e) => setLanguage(e.target.value as Lang)}
              className="rounded border px-2 py-1 text-xs"
              style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
            >
              {LANGUAGES.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.label}
                </option>
              ))}
            </select>,
          )}
          {row(
            <Ruler className="w-3.5 h-3.5" />,
            t('settings.units'),
            t('settings.unitsHelp'),
            segmented<DisplayUnit>(unit, [['mm', 'mm'], ['cm', 'cm'], ['in', 'in']], setUnit),
          )}
          {row(
            theme === 'paper' ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />,
            t('settings.theme'),
            '',
            segmented(theme, [['paper', t('settings.themePaper')], ['dark', t('settings.themeDark')]], setTheme),
          )}
        </div>
        <p className="px-4 py-3 text-[11px] border-t" style={{ color: 'var(--text-muted)', borderColor: 'var(--border-subtle)' }}>
          {t('settings.about')}
        </p>
      </div>
    </div>
  );
};
