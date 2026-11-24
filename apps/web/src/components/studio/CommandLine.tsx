import React, { useMemo, useRef, useState } from 'react';
import { ChevronRight } from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import { MODELIST_COMMANDS } from '../../lib/cad/command-palette';
import { displayToMm, formatMm } from '../../lib/types';
import { useCatalogT, useT, type MessageKey } from '../../lib/i18n';
import { useStudioStore } from '../../store/studioStore';
import { TOOL_PROMPTS } from './ToolOptions';

interface Suggestion {
  id: string;
  label: string;
  hint?: string;
  /** Text put in the input instead of running (parameters: "bust = "). */
  fill?: string;
  run?: () => void;
}

const ASSIGN = /^\s*([A-Za-z_][\w]*)\s*=\s*(.+?)\s*$/;

/**
 * Keyboard-first command line under the timeline. Type part of a command name and press Enter
 * to run it, or `name = value` to set a body measurement, an ease or a formula variable.
 * While a tool runs, the prompt tells what it expects next. Focus it with "/".
 */
export const CommandLine: React.FC = () => {
  const t = useT();
  const ct = useCatalogT();
  const snapshot = useStudioStore((s) => s.snapshot);
  const unit = useStudioStore((s) => s.unit);
  const tool = useStudioStore((s) => s.activeTool);
  const seamCommand = useStudioStore((s) => s.seamCommand);
  const [text, setText] = useState('');
  const [focused, setFocused] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const history = useRef<string[]>([]);
  const historyPos = useRef(-1);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const st = useStudioStore.getState;

  const commands: Suggestion[] = useMemo(() => {
    const list: Suggestion[] = MODELIST_COMMANDS.map((c) => ({
      id: c.id,
      label: t(`cmd.${c.id}.label` as MessageKey),
      hint: c.shortcut,
      run: () => st().executeCommand(c.id),
    }));
    const extra: Array<[string, MessageKey, () => void]> = [
      ['params', 'appbar.params', () => st().openPanelPage('params')],
      ['materials', 'panel.materials', () => st().openPanelPage('materials')],
      ['grading', 'panel.grading', () => st().openPanelPage('grading')],
      ['view2d', 'appbar.view2d', () => st().setWorkspaceMode('2d')],
      ['view3d', 'appbar.view3d', () => st().setWorkspaceMode('3d')],
      ['split', 'layout.split', () => st().setWorkspaceMode('split')],
      ['settings', 'settings.title', () => st().setSettingsOpen(true)],
    ];
    for (const [id, key, run] of extra) list.push({ id, label: t(key), run });
    return list;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [t]);

  const suggestions: Suggestion[] = useMemo(() => {
    const q = text.trim().toLowerCase();
    if (!q || !snapshot) return [];
    const assign = ASSIGN.exec(text);
    if (assign) return [];
    const scored: Array<[number, Suggestion]> = [];
    for (const c of commands) {
      const label = c.label.toLowerCase();
      let score = -1;
      if (c.hint && c.hint.toLowerCase() === q) score = 0;
      else if (label.startsWith(q)) score = 1;
      else if (label.split(/[\s/&(]+/).some((w) => w.startsWith(q))) score = 2;
      else if (label.includes(q) || c.id.includes(q)) score = 3;
      if (score >= 0) scored.push([score, c]);
    }
    // Parameters by name: progressive discovery of what can be typed as "name = value".
    const params: Suggestion[] = [
      ...snapshot.measurements.map((m) => ({
        id: `m:${m.key}`,
        label: `${m.key} = ${formatMm(m.value_mm, unit, 1)}`,
        hint: ct('measure', m.key, m.label),
        fill: `${m.key} = `,
      })),
      ...snapshot.ease.map((m) => ({
        id: `e:${m.key}`,
        label: `${m.key} = ${formatMm(m.value_mm, unit, 1)}`,
        hint: ct('measure', m.key, m.label),
        fill: `${m.key} = `,
      })),
      ...snapshot.variables.map((v) => ({
        id: `v:${v.name}`,
        label: `${v.name} = ${v.expr}`,
        hint: v.value === null ? v.error ?? '' : v.value.toFixed(1),
        fill: `${v.name} = `,
      })),
    ];
    for (const p of params) {
      const name = p.id.slice(2).toLowerCase();
      if (name.startsWith(q)) scored.push([4, p]);
      else if (name.includes(q)) scored.push([5, p]);
    }
    scored.sort((a, b) => a[0] - b[0] || a[1].label.localeCompare(b[1].label));
    return scored.slice(0, 8).map((s) => s[1]);
  }, [text, commands, snapshot, unit, ct]);

  const runAssignment = (name: string, value: string) => {
    if (!snapshot) return;
    const num = Number(value.replace(',', '.'));
    const measurement = snapshot.measurements.find((m) => m.key === name);
    const ease = snapshot.ease.find((m) => m.key === name);
    try {
      if ((measurement || ease) && Number.isFinite(num)) {
        const mm = displayToMm(num, unit);
        st().setSnapshot(measurement ? kernel.setMeasurement(name, mm) : kernel.setEase(name, mm));
        st().setStatusMessage(t('cmdline.set', { name, value: formatMm(mm, unit, 1) }));
      } else {
        st().setSnapshot(kernel.setVariable(name, value));
        st().setStatusMessage(t('cmdline.set', { name, value }));
      }
    } catch (e) {
      st().setStatusMessage(t('cmdline.error', { error: (e as Error).message }));
    }
  };

  const submit = (pick?: Suggestion) => {
    const raw = text.trim();
    if (!raw && !pick) return;
    const choice = pick ?? suggestions[highlight];
    if (choice?.fill && !ASSIGN.test(raw)) {
      setText(choice.fill);
      return;
    }
    const assign = ASSIGN.exec(raw);
    if (assign) runAssignment(assign[1], assign[2]);
    else if (choice?.run) choice.run();
    else {
      st().setStatusMessage(t('cmdline.unknown', { text: raw }));
      return;
    }
    history.current = [raw, ...history.current.filter((h) => h !== raw)].slice(0, 30);
    historyPos.current = -1;
    setText('');
    setHighlight(0);
  };

  const toolPrompt = seamCommand
    ? t('prompt.seam', { slot: seamCommand.activeSlot.toUpperCase() })
    : TOOL_PROMPTS[tool]
      ? t(TOOL_PROMPTS[tool]!)
      : null;

  return (
    <div className="relative flex-1 min-w-0 h-full flex items-center gap-1.5 pl-2">
      <ChevronRight className="w-3.5 h-3.5 shrink-0" style={{ color: focused ? 'var(--accent)' : 'var(--text-muted)' }} />
      {toolPrompt && !focused && !text && (
        <span className="shrink-0 text-[11px] font-medium" style={{ color: 'var(--accent)' }}>
          {t(`cmd.${seamCommand ? 'sew_segment' : tool}.label` as MessageKey)}:
        </span>
      )}
      <input
        id="studio-command-line"
        ref={inputRef}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setHighlight(0);
        }}
        onFocus={() => setFocused(true)}
        onBlur={() => setTimeout(() => setFocused(false), 120)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            submit();
          } else if (e.key === 'Escape') {
            setText('');
            inputRef.current?.blur();
          } else if (e.key === 'ArrowDown' && suggestions.length) {
            e.preventDefault();
            setHighlight((h) => (h + 1) % suggestions.length);
          } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            if (suggestions.length && text) setHighlight((h) => (h - 1 + suggestions.length) % suggestions.length);
            else if (history.current.length) {
              historyPos.current = Math.min(history.current.length - 1, historyPos.current + 1);
              setText(history.current[historyPos.current]);
            }
          } else if (e.key === 'Tab' && suggestions[highlight]) {
            e.preventDefault();
            const s = suggestions[highlight];
            setText(s.fill ?? s.label);
          }
        }}
        spellCheck={false}
        placeholder={toolPrompt ?? t('cmdline.placeholder')}
        className="flex-1 min-w-0 h-[20px] bg-transparent outline-none text-[11px] font-mono-num"
        style={{ color: 'var(--text-primary)' }}
        aria-label={t('cmdline.title')}
      />
      {focused && suggestions.length > 0 && (
        <div className="floating-panel absolute left-1 bottom-full mb-1 z-50 w-[420px] py-1 text-xs" role="listbox">
          {suggestions.map((s, i) => (
            <button
              key={s.id}
              role="option"
              aria-selected={i === highlight}
              onMouseDown={(e) => {
                e.preventDefault();
                submit(s);
              }}
              onMouseEnter={() => setHighlight(i)}
              className="w-full h-[26px] px-2.5 flex items-center gap-2 text-left cursor-pointer"
              style={{ background: i === highlight ? 'var(--bg-hover)' : undefined }}
            >
              <span className={`flex-1 truncate ${s.fill ? 'font-mono-num' : ''}`}>{s.label}</span>
              {s.hint && (
                <span className="shrink-0 max-w-[180px] truncate text-[10px] font-mono-num" style={{ color: 'var(--text-muted)' }}>
                  {s.hint}
                </span>
              )}
            </button>
          ))}
          <div className="px-2.5 pt-1 mt-1 border-t text-[10px]" style={{ borderColor: 'var(--border-subtle)', color: 'var(--text-muted)' }}>
            {t('cmdline.keys')}
          </div>
        </div>
      )}
    </div>
  );
};
