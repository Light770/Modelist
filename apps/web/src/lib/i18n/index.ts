/**
 * Studio UI translations.
 *
 * `en` is the source of truth for keys; `fr` must define every key (enforced by its type).
 * Components call `const t = useT()` and `t('topbar.save')`, with `{name}` placeholders filled
 * from the optional second argument. Pattern data (piece names, edge labels) is user content and
 * is never translated; catalog entries coming from the kernel are translated by id.
 */
import { useStudioStore } from '../../store/studioStore';
import { en, type MessageKey } from './en';
import { fr } from './fr';

export type Lang = 'en' | 'fr';
export type { MessageKey };

export const LANGUAGES: Array<{ id: Lang; label: string }> = [
  { id: 'en', label: 'English' },
  { id: 'fr', label: 'Français' },
];

const DICTS: Record<Lang, Record<MessageKey, string>> = { en, fr };
const LANG_KEY = 'modelist.lang';

export function detectLanguage(): Lang {
  try {
    const saved = localStorage.getItem(LANG_KEY);
    if (saved === 'en' || saved === 'fr') return saved;
  } catch {
    // storage unavailable (private mode)
  }
  if (typeof navigator !== 'undefined' && navigator.language?.toLowerCase().startsWith('fr')) {
    return 'fr';
  }
  return 'en';
}

export function persistLanguage(lang: Lang) {
  try {
    localStorage.setItem(LANG_KEY, lang);
  } catch {
    // ignore
  }
  if (typeof document !== 'undefined') document.documentElement.lang = lang;
}

export function translate(
  lang: Lang,
  key: MessageKey,
  vars?: Record<string, string | number>,
): string {
  let s = DICTS[lang]?.[key] ?? en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) s = s.split(`{${k}}`).join(String(v));
  }
  return s;
}

export type TFn = (key: MessageKey, vars?: Record<string, string | number>) => string;

/** React hook: translation function bound to the current studio language. */
export function useT(): TFn {
  const lang = useStudioStore((s) => s.language);
  return (key, vars) => translate(lang, key, vars);
}

/** Non-React access (event handlers outside components, status messages). */
export function t(key: MessageKey, vars?: Record<string, string | number>): string {
  return translate(useStudioStore.getState().language, key, vars);
}

/** Translates a kernel catalog entry by id, falling back to the kernel's English text. */
export function tCatalog(prefix: string, id: string, fallback: string): string {
  const key = `${prefix}.${id}` as MessageKey;
  const lang = useStudioStore.getState().language;
  return (DICTS[lang] as Record<string, string>)[key] ?? (en as Record<string, string>)[key] ?? fallback;
}

export function useCatalogT() {
  const lang = useStudioStore((s) => s.language);
  return (prefix: string, id: string, fallback: string): string => {
    const key = `${prefix}.${id}`;
    return (DICTS[lang] as Record<string, string>)[key] ?? (en as Record<string, string>)[key] ?? fallback;
  };
}
