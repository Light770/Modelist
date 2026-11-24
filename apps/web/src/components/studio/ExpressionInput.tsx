import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useCatalogT, useT } from '../../lib/i18n';
import { createPortal } from 'react-dom';
import { Check, Plus, Sparkles } from 'lucide-react';
import { kernel } from '../../lib/kernelClient';
import {
  applyAutocomplete,
  BUILTIN_FUNCTIONS,
  evaluateExpression,
  getAutocompleteSuggestions,
  parseInlineDefinition,
  type ExpressionUnit,
  type SymbolEntry,
} from '../../lib/cad/expression-assist';
import { formatMm, mmToDisplay } from '../../lib/types';
import { useStudioStore } from '../../store/studioStore';

interface ExpressionInputProps {
  /** Current value in millimeters (for numeric dimension inputs) */
  valueMm?: number;
  /** Current formula string (for DAG variable / expression fields) */
  valueText?: string;
  unit?: ExpressionUnit;
  mode?: 'numeric' | 'formula';
  onCommitMm?: (mm: number, rawExpr: string) => void;
  onCommitFormula?: (expr: string) => void;
  placeholder?: string;
  className?: string;
  style?: React.CSSProperties;
  title?: string;
  autoFocus?: boolean;
  /** Disable unit suffix formatting in numeric display (e.g. percentages/ratios) */
  decimals?: number;
}

export const ExpressionInput: React.FC<ExpressionInputProps> = ({
  valueMm,
  valueText,
  unit = 'mm',
  mode = 'numeric',
  onCommitMm,
  onCommitFormula,
  placeholder,
  className = '',
  style,
  title,
  autoFocus = false,
  decimals,
}) => {
  const t = useT();
  const ct = useCatalogT();
  const snapshot = useStudioStore((s) => s.snapshot);
  const setSnapshot = useStudioStore((s) => s.setSnapshot);
  const setStatusMessage = useStudioStore((s) => s.setStatusMessage);

  const formattedValue = useMemo(() => {
    if (mode === 'formula') return valueText ?? '';
    if (valueMm === undefined || !Number.isFinite(valueMm)) return '';
    if (unit === 'mm' || unit === 'cm' || unit === 'in') {
      const dec = decimals ?? (unit === 'mm' ? 1 : unit === 'cm' ? 2 : 3);
      return `${Number(mmToDisplay(valueMm, unit).toFixed(dec))} ${unit}`;
    }
    return String(Number(valueMm.toFixed(decimals ?? 2)));
  }, [decimals, mode, unit, valueMm, valueText]);

  const [draft, setDraft] = useState(formattedValue);
  const [focused, setFocused] = useState(false);
  const [cursorPos, setCursorPos] = useState(0);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [suppressDropdown, setSuppressDropdown] = useState(false);
  const [pendingCreateVar, setPendingCreateVar] = useState<{
    name: string;
    fullExpr: string;
    initialVal: string;
  } | null>(null);

  const inputRef = useRef<HTMLInputElement | null>(null);
  const createInputRef = useRef<HTMLInputElement | null>(null);
  const [anchorRect, setAnchorRect] = useState<{ left: number; top: number; width: number } | null>(null);

  useEffect(() => {
    if (!focused) {
      setDraft(formattedValue);
    }
  }, [focused, formattedValue]);

  const { symbolsList, symbolMap } = useMemo(() => {
    const list: SymbolEntry[] = [];
    const map = new Map<string, number>();
    if (snapshot) {
      for (const m of snapshot.measurements) {
        list.push({ name: m.key, label: m.label, kind: 'pom', valueMm: m.value_mm });
        map.set(m.key, m.value_mm);
      }
      for (const e of snapshot.ease) {
        list.push({ name: e.key, label: e.label, kind: 'ease', valueMm: e.value_mm });
        map.set(e.key, e.value_mm);
      }
      for (const v of snapshot.variables) {
        list.push({
          name: v.name,
          label: v.description || v.expr,
          kind: 'var',
          valueMm: v.value ?? undefined,
        });
        if (v.value !== null && Number.isFinite(v.value)) {
          map.set(v.name, v.value);
        }
      }
    }
    list.push(...BUILTIN_FUNCTIONS);
    return { symbolsList: list, symbolMap: map };
  }, [snapshot]);

  const suggestions = useMemo(() => {
    if (!focused || suppressDropdown) return { start: 0, end: 0, prefix: '', items: [] as SymbolEntry[] };
    return getAutocompleteSuggestions(draft, cursorPos, symbolsList, 7);
  }, [cursorPos, draft, focused, suppressDropdown, symbolsList]);

  useEffect(() => {
    setSelectedIdx(0);
  }, [suggestions.prefix, suggestions.items.length]);

  useEffect(() => {
    if ((focused && (suggestions.items.length > 0 || pendingCreateVar)) && inputRef.current) {
      const r = inputRef.current.getBoundingClientRect();
      setAnchorRect({ left: r.left, top: r.bottom + 3, width: Math.max(r.width, 220) });
    }
  }, [focused, pendingCreateVar, suggestions.items.length, draft]);

  const liveEval = useMemo(() => {
    const def = parseInlineDefinition(draft);
    const exprToEval = def ? def.expr : draft;
    return evaluateExpression(exprToEval, symbolMap, unit);
  }, [draft, symbolMap, unit]);

  const acceptSuggestion = (item: SymbolEntry) => {
    const { nextText, nextCursor } = applyAutocomplete(draft, suggestions.start, suggestions.end, item);
    setDraft(nextText);
    setCursorPos(nextCursor);
    requestAnimationFrame(() => {
      if (inputRef.current) {
        inputRef.current.focus();
        inputRef.current.setSelectionRange(nextCursor, nextCursor);
      }
    });
  };

  const tryCommit = (textToCommit: string) => {
    const trimmed = textToCommit.trim();
    if (!trimmed) {
      setDraft(formattedValue);
      return;
    }

    // 1. Inline named parameter definition: `hem_depth = 35 mm`
    const inlineDef = parseInlineDefinition(trimmed);
    if (inlineDef) {
      const res = evaluateExpression(inlineDef.expr, symbolMap, unit);
      if (!res.ok && res.unknownVars.length > 0) {
        setPendingCreateVar({ name: res.unknownVars[0], fullExpr: trimmed, initialVal: '15 mm' });
        requestAnimationFrame(() => createInputRef.current?.focus());
        return;
      }
      if (res.ok && res.valueMm !== null) {
        const nextSnap = kernel.setVariable(inlineDef.name, inlineDef.expr, t('expr.inlineDesc'));
        setSnapshot(nextSnap);
        setStatusMessage(t('expr.defined', { name: inlineDef.name, value: res.valueMm.toFixed(1) }));
        if (mode === 'formula') {
          onCommitFormula?.(inlineDef.expr);
        } else {
          onCommitMm?.(res.valueMm, inlineDef.name);
        }
        setFocused(false);
        inputRef.current?.blur();
        return;
      }
    }

    // 2. Normal formula evaluation
    const res = evaluateExpression(trimmed, symbolMap, unit);
    if (!res.ok && res.unknownVars.length > 0) {
      setPendingCreateVar({
        name: res.unknownVars[0],
        fullExpr: trimmed,
        initialVal: unit === 'cm' ? '1.5 cm' : unit === 'in' ? '0.5 in' : '15 mm',
      });
      requestAnimationFrame(() => createInputRef.current?.focus());
      return;
    }

    if (mode === 'formula') {
      if (trimmed !== (valueText ?? '')) {
        onCommitFormula?.(trimmed);
      }
      setFocused(false);
      inputRef.current?.blur();
      return;
    }

    if (res.ok && res.valueMm !== null && onCommitMm) {
      onCommitMm(res.valueMm, trimmed);
      setFocused(false);
      inputRef.current?.blur();
    } else {
      setDraft(formattedValue);
    }
  };

  const confirmCreateVar = () => {
    if (!pendingCreateVar) return;
    const rhsEval = evaluateExpression(pendingCreateVar.initialVal, symbolMap, unit);
    if (!rhsEval.ok || rhsEval.valueMm === null) return;
    const nextSnap = kernel.setVariable(
      pendingCreateVar.name,
      pendingCreateVar.initialVal.trim(),
      t('expr.createdDesc'),
    );
    setSnapshot(nextSnap);
    setStatusMessage(
      t('expr.created', { name: pendingCreateVar.name, value: rhsEval.valueMm.toFixed(1) }),
    );

    const updatedMap = new Map(symbolMap);
    updatedMap.set(pendingCreateVar.name, rhsEval.valueMm);
    const fullToEval = parseInlineDefinition(pendingCreateVar.fullExpr)?.expr ?? pendingCreateVar.fullExpr;
    const reEval = evaluateExpression(fullToEval, updatedMap, unit);
    const exprSaved = pendingCreateVar.fullExpr;
    setPendingCreateVar(null);

    if (mode === 'formula') {
      onCommitFormula?.(exprSaved);
    } else if (reEval.ok && reEval.valueMm !== null) {
      onCommitMm?.(reEval.valueMm, exprSaved);
    }
    setFocused(false);
  };

  const showLivePreview =
    focused &&
    draft.trim() !== formattedValue &&
    liveEval.ok &&
    liveEval.valueMm !== null &&
    /[a-zA-Z+\-*/()]/.test(draft);

  return (
    <>
      <div className="relative flex items-center min-w-0 flex-1">
        <input
          ref={inputRef}
          type="text"
          value={draft}
          autoFocus={autoFocus}
          placeholder={placeholder}
          title={title ?? t('expr.title')}
          onFocus={(e) => {
            setFocused(true);
            setSuppressDropdown(false);
            setCursorPos(e.target.selectionStart ?? draft.length);
            e.target.select();
          }}
          onBlur={() => {
            // Delay blur commit slightly so clicking an autocomplete item or popover works cleanly
            setTimeout(() => {
              if (pendingCreateVar) return;
              if (document.activeElement !== inputRef.current) {
                setFocused(false);
                if (draft.trim() !== formattedValue && liveEval.ok) {
                  tryCommit(draft);
                } else {
                  setDraft(formattedValue);
                }
              }
            }, 130);
          }}
          onChange={(e) => {
            setDraft(e.target.value);
            setSuppressDropdown(false);
            setPendingCreateVar(null);
            setCursorPos(e.target.selectionStart ?? e.target.value.length);
          }}
          onClick={(e) => {
            setCursorPos((e.target as HTMLInputElement).selectionStart ?? draft.length);
          }}
          onKeyUp={(e) => {
            setCursorPos((e.target as HTMLInputElement).selectionStart ?? draft.length);
          }}
          onKeyDown={(e) => {
            if (suggestions.items.length > 0 && !suppressDropdown) {
              if (e.key === 'ArrowDown') {
                e.preventDefault();
                setSelectedIdx((i) => (i + 1) % suggestions.items.length);
                return;
              }
              if (e.key === 'ArrowUp') {
                e.preventDefault();
                setSelectedIdx((i) => (i - 1 + suggestions.items.length) % suggestions.items.length);
                return;
              }
              if (e.key === 'Tab' || e.key === 'Enter') {
                e.preventDefault();
                e.stopPropagation();
                acceptSuggestion(suggestions.items[selectedIdx] ?? suggestions.items[0]);
                return;
              }
              if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                setSuppressDropdown(true);
                return;
              }
            }

            if (e.key === 'Enter') {
              e.preventDefault();
              e.stopPropagation();
              tryCommit(draft);
            } else if (e.key === 'Escape') {
              e.stopPropagation();
              setPendingCreateVar(null);
              setDraft(formattedValue);
              setFocused(false);
              inputRef.current?.blur();
            }
          }}
          className={`font-mono-num outline-none transition ${className}`}
          style={{
            borderColor:
              focused && !liveEval.ok && draft.trim().length > 0
                ? '#e11d48'
                : style?.borderColor,
            ...style,
          }}
        />
        {showLivePreview && liveEval.valueMm !== null && (
          <span
            className="pointer-events-none absolute right-1.5 px-1 py-0.2 rounded text-[9px] font-mono-num"
            style={{ background: 'var(--accent-soft)', color: 'var(--accent)' }}
          >
            ={unit === 'deg' ? `${liveEval.valueMm.toFixed(1)}°` : formatMm(liveEval.valueMm, unit === 'cm' || unit === 'in' ? unit : 'mm', 1)}
          </span>
        )}
      </div>

      {/* Autocomplete Portal */}
      {focused &&
        suggestions.items.length > 0 &&
        !pendingCreateVar &&
        anchorRect &&
        typeof document !== 'undefined' &&
        createPortal(
          <div
            onMouseDown={(e) => e.preventDefault()}
            className="fixed z-50 rounded border shadow-xl py-1 text-[11px] max-h-56 overflow-y-auto"
            style={{
              left: Math.min(anchorRect.left, window.innerWidth - anchorRect.width - 12),
              top: Math.min(anchorRect.top, window.innerHeight - 220),
              width: anchorRect.width,
              background: 'var(--bg-surface)',
              borderColor: 'var(--border-strong)',
            }}
          >
            <div
              className="px-2 py-0.5 text-[9px] font-semibold uppercase tracking-wider flex items-center justify-between"
              style={{ color: 'var(--text-muted)' }}
            >
              <span>{t('expr.params')}</span>
              <span>{t('expr.tabEnter')}</span>
            </div>
            {suggestions.items.map((item, idx) => (
              <button
                key={item.name}
                type="button"
                onClick={() => acceptSuggestion(item)}
                className="w-full text-left px-2 py-1 flex items-center justify-between gap-2 cursor-pointer"
                style={{
                  background: idx === selectedIdx ? 'var(--accent-soft)' : 'transparent',
                  color: idx === selectedIdx ? 'var(--accent)' : 'var(--text-primary)',
                }}
              >
                <div className="min-w-0">
                  <div className="font-mono-num font-semibold truncate">{item.name}</div>
                  <div className="text-[9px] truncate" style={{ color: 'var(--text-muted)' }}>
                    {item.kind === 'pom' || item.kind === 'ease'
                      ? ct('measure', item.name, item.label)
                      : item.label}
                  </div>
                </div>
                <span
                  className="font-mono-num text-[10px] shrink-0 px-1 rounded"
                  style={{
                    background: 'var(--bg-panel)',
                    color: 'var(--text-secondary)',
                  }}
                >
                  {item.valueMm !== undefined
                    ? `${item.valueMm.toFixed(1)} mm`
                    : item.kind.toUpperCase()}
                </span>
              </button>
            ))}
          </div>,
          document.body,
        )}

      {/* Quick-Create Unknown Variable Popover (Fission ProposerNom pattern) */}
      {pendingCreateVar &&
        anchorRect &&
        typeof document !== 'undefined' &&
        createPortal(
          <div
            onMouseDown={(e) => e.stopPropagation()}
            className="fixed z-50 rounded-md border shadow-2xl p-2.5 text-xs space-y-2"
            style={{
              left: Math.min(anchorRect.left, window.innerWidth - 268),
              top: Math.min(anchorRect.top, window.innerHeight - 130),
              width: 256,
              background: 'var(--bg-surface)',
              borderColor: 'var(--accent)',
            }}
          >
            <div className="flex items-center gap-1.5 font-semibold text-[11px]">
              <Sparkles className="w-3.5 h-3.5" style={{ color: 'var(--accent)' }} />
              <span>{t('expr.createPrompt', { name: pendingCreateVar.name })}</span>
            </div>
            <div className="flex items-center gap-1.5">
              <input
                ref={createInputRef}
                type="text"
                value={pendingCreateVar.initialVal}
                onChange={(e) =>
                  setPendingCreateVar((p) => (p ? { ...p, initialVal: e.target.value } : null))
                }
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    confirmCreateVar();
                  } else if (e.key === 'Escape') {
                    e.preventDefault();
                    setPendingCreateVar(null);
                  }
                }}
                className="flex-1 rounded border px-2 py-1 font-mono-num text-[11px]"
                style={{ background: 'var(--bg-panel)', borderColor: 'var(--border-subtle)' }}
                placeholder={t('expr.valuePlaceholder')}
              />
              <button
                type="button"
                onClick={confirmCreateVar}
                className="px-2 py-1 rounded text-white font-medium text-[11px] inline-flex items-center gap-1 cursor-pointer"
                style={{ background: 'var(--accent)' }}
              >
                <Plus className="w-3 h-3" />
                <span>{t('common.add')}</span>
              </button>
            </div>
            <div className="flex items-center justify-between text-[10px]" style={{ color: 'var(--text-muted)' }}>
              <span>{t('expr.saved')}</span>
              <button
                type="button"
                onClick={() => setPendingCreateVar(null)}
                className="hover:underline cursor-pointer"
              >
                Cancel
              </button>
            </div>
          </div>,
          document.body,
        )}
    </>
  );
};

export const ExpressionCheckIcon = Check;
