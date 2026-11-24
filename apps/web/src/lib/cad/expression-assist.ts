export interface SymbolEntry {
  name: string;
  label: string;
  kind: 'pom' | 'ease' | 'var' | 'fn';
  valueMm?: number;
  description?: string;
}

export interface ExprEvalResult {
  ok: boolean;
  valueMm: number | null;
  error: string | null;
  unknownVars: string[];
  usedExplicitUnits: boolean;
  usedVars: boolean;
}

export type ExpressionUnit = 'mm' | 'cm' | 'in' | 'deg' | 'pct' | 'ratio';

export const BUILTIN_FUNCTIONS: SymbolEntry[] = [
  { name: 'min', label: 'min(a, b)', kind: 'fn', description: 'Smaller of two values' },
  { name: 'max', label: 'max(a, b)', kind: 'fn', description: 'Larger of two values' },
  { name: 'clamp', label: 'clamp(v, lo, hi)', kind: 'fn', description: 'Clamp value to range' },
  { name: 'abs', label: 'abs(x)', kind: 'fn', description: 'Absolute value' },
  { name: 'sqrt', label: 'sqrt(x)', kind: 'fn', description: 'Square root' },
  { name: 'hypot', label: 'hypot(x, y)', kind: 'fn', description: 'Euclidean hypotenuse' },
  { name: 'sin', label: 'sin(deg)', kind: 'fn', description: 'Sine (degrees)' },
  { name: 'cos', label: 'cos(deg)', kind: 'fn', description: 'Cosine (degrees)' },
  { name: 'tan', label: 'tan(deg)', kind: 'fn', description: 'Tangent (degrees)' },
  { name: 'atan2', label: 'atan2(y, x)', kind: 'fn', description: 'Arctangent in degrees' },
  { name: 'if', label: 'if(cond, a, b)', kind: 'fn', description: 'Conditional branch' },
];

/**
 * Recognizes an inline named parameter definition like `hem_depth = 35 mm` or `ease_arm = (armscye_depth * 0.1)`.
 * Ignores `==`, `<=`, `>=`, or `=` inside parentheses such as `if(a = b, 1, 2)`.
 */
export function parseInlineDefinition(raw: string): { name: string; expr: string } | null {
  let depth = 0;
  let eqPos = -1;
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (ch === '(') depth += 1;
    else if (ch === ')') depth = Math.max(0, depth - 1);
    else if (ch === '=' && depth === 0) {
      const prev = raw[i - 1] ?? '';
      const next = raw[i + 1] ?? '';
      if (prev === '<' || prev === '>' || prev === '!' || prev === '=' || next === '=') {
        continue;
      }
      if (eqPos !== -1) return null;
      eqPos = i;
    }
  }
  if (eqPos === -1) return null;
  const lhs = raw.slice(0, eqPos).trim();
  const rhs = raw.slice(eqPos + 1).trim();
  if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(lhs) || rhs.length === 0) {
    return null;
  }
  return { name: lhs, expr: rhs };
}

type Tok =
  | { t: 'num'; v: number }
  | { t: 'id'; v: string }
  | { t: 'op'; v: '+' | '-' | '*' | '/' | '(' | ')' | ',' | '>' | '<' | '=' | '%' };

function tokenize(input: string): Tok[] {
  const toks: Tok[] = [];
  let i = 0;
  while (i < input.length) {
    const ch = input[i];
    if (/\s/.test(ch)) {
      i += 1;
      continue;
    }
    if (/[0-9.]/.test(ch)) {
      const start = i;
      while (i < input.length && /[0-9.]/.test(input[i])) i += 1;
      const slice = input.slice(start, i);
      const val = Number(slice);
      if (!Number.isFinite(val) || slice === '.') {
        throw new Error(`Invalid number "${slice}"`);
      }
      toks.push({ t: 'num', v: val });
      continue;
    }
    if (/[a-zA-Z_@]/.test(ch)) {
      const start = i;
      i += 1;
      while (i < input.length && /[a-zA-Z0-9_.]/.test(input[i])) i += 1;
      toks.push({ t: 'id', v: input.slice(start, i) });
      continue;
    }
    if (
      ch === '+' ||
      ch === '-' ||
      ch === '*' ||
      ch === '/' ||
      ch === '(' ||
      ch === ')' ||
      ch === ',' ||
      ch === '>' ||
      ch === '<' ||
      ch === '=' ||
      ch === '%'
    ) {
      toks.push({ t: 'op', v: ch });
      i += 1;
      continue;
    }
    throw new Error(`Unexpected character "${ch}"`);
  }
  return toks;
}

const UNIT_FACTORS: Record<string, number> = {
  mm: 1,
  cm: 10,
  in: 25.4,
  deg: 1,
};

/**
 * Evaluates a user formula expression safely with optional physical units (`mm`, `cm`, `in`, `deg`, `%`),
 * POM / DAG variables, and math functions.
 *
 * When `defaultUnit` is `cm` or `in` and the expression is a pure unitless arithmetic expression
 * (no explicit unit suffixes and no variable references, e.g. `2.5` or `1 + 0.5`), converts the
 * result from `defaultUnit` into millimeters automatically.
 */
export function evaluateExpression(
  raw: string,
  symbols: Map<string, number>,
  defaultUnit: ExpressionUnit = 'mm',
): ExprEvalResult {
  const trimmed = raw.trim();
  if (!trimmed) {
    return {
      ok: false,
      valueMm: null,
      error: 'Empty expression',
      unknownVars: [],
      usedExplicitUnits: false,
      usedVars: false,
    };
  }

  let toks: Tok[];
  try {
    toks = tokenize(trimmed);
  } catch (err) {
    return {
      ok: false,
      valueMm: null,
      error: (err as Error).message,
      unknownVars: [],
      usedExplicitUnits: false,
      usedVars: false,
    };
  }

  let pos = 0;
  let usedExplicitUnits = false;
  let usedVars = false;
  const unknownSet = new Set<string>();

  const peek = () => toks[pos];
  const next = () => toks[pos++];

  const parseExpr = (): number => parseCmp();

  const parseCmp = (): number => {
    let lhs = parseAdd();
    while (true) {
      const p = peek();
      if (p?.t === 'op' && (p.v === '>' || p.v === '<' || p.v === '=')) {
        next();
        const rhs = parseAdd();
        lhs = p.v === '>' ? (lhs > rhs ? 1 : 0) : p.v === '<' ? (lhs < rhs ? 1 : 0) : Math.abs(lhs - rhs) < 1e-9 ? 1 : 0;
      } else {
        break;
      }
    }
    return lhs;
  };

  const parseAdd = (): number => {
    let lhs = parseMul();
    while (true) {
      const p = peek();
      if (p?.t === 'op' && (p.v === '+' || p.v === '-')) {
        next();
        const rhs = parseMul();
        lhs = p.v === '+' ? lhs + rhs : lhs - rhs;
      } else {
        break;
      }
    }
    return lhs;
  };

  const parseMul = (): number => {
    let lhs = parseUnary();
    while (true) {
      const p = peek();
      if (p?.t === 'op' && (p.v === '*' || p.v === '/')) {
        next();
        const rhs = parseUnary();
        if (p.v === '/') {
          if (Math.abs(rhs) < 1e-12) throw new Error('Division by zero');
          lhs = lhs / rhs;
        } else {
          lhs = lhs * rhs;
        }
      } else {
        break;
      }
    }
    return lhs;
  };

  const parseUnary = (): number => {
    const p = peek();
    if (p?.t === 'op' && p.v === '-') {
      next();
      return -parseUnary();
    }
    if (p?.t === 'op' && p.v === '+') {
      next();
      return parseUnary();
    }
    return parsePrimary();
  };

  const parsePrimary = (): number => {
    const tok = next();
    if (!tok) throw new Error('Unexpected end of expression');
    if (tok.t === 'num') {
      const nxt = peek();
      if (nxt?.t === 'id') {
        const mult = UNIT_FACTORS[nxt.v.toLowerCase()];
        if (mult !== undefined) {
          next();
          usedExplicitUnits = true;
          return tok.v * mult;
        }
      } else if (nxt?.t === 'op' && nxt.v === '%') {
        next();
        usedExplicitUnits = true;
        return tok.v;
      }
      return tok.v;
    }
    if (tok.t === 'id') {
      const p = peek();
      if (p?.t === 'op' && p.v === '(') {
        next();
        const args: number[] = [];
        const closeCheck = peek();
        if (!(closeCheck?.t === 'op' && closeCheck.v === ')')) {
          while (true) {
            args.push(parseExpr());
            const comma = peek();
            if (comma?.t === 'op' && comma.v === ',') {
              next();
            } else {
              break;
            }
          }
        }
        const close = next();
        if (!(close?.t === 'op' && close.v === ')')) {
          throw new Error(`Expected ")" after ${tok.v}(...)`);
        }
        return callBuiltin(tok.v, args);
      }
      if (tok.v === 'pi') return Math.PI;
      usedVars = true;
      const val = symbols.get(tok.v);
      if (val === undefined) {
        unknownSet.add(tok.v);
        return NaN;
      }
      return val;
    }
    if (tok.t === 'op' && tok.v === '(') {
      const inner = parseExpr();
      const close = next();
      if (!(close?.t === 'op' && close.v === ')')) {
        throw new Error('Expected closing ")"');
      }
      return inner;
    }
    throw new Error(`Unexpected token "${tok.v}"`);
  };

  try {
    let val = parseExpr();
    if (pos < toks.length) {
      throw new Error('Unexpected trailing input');
    }
    const unknownVars = Array.from(unknownSet);
    if (unknownVars.length > 0) {
      return {
        ok: false,
        valueMm: null,
        error: `Unknown parameter: ${unknownVars.join(', ')}`,
        unknownVars,
        usedExplicitUnits,
        usedVars,
      };
    }
    if (!Number.isFinite(val)) {
      throw new Error('Non-finite result');
    }
    if (!usedExplicitUnits && !usedVars) {
      if (defaultUnit === 'cm') val *= 10;
      else if (defaultUnit === 'in') val *= 25.4;
    }
    return {
      ok: true,
      valueMm: val,
      error: null,
      unknownVars: [],
      usedExplicitUnits,
      usedVars,
    };
  } catch (err) {
    return {
      ok: false,
      valueMm: null,
      error: (err as Error).message,
      unknownVars: Array.from(unknownSet),
      usedExplicitUnits,
      usedVars,
    };
  }
}

function callBuiltin(name: string, args: number[]): number {
  switch (name) {
    case 'min':
      if (args.length === 2) return Math.min(args[0], args[1]);
      break;
    case 'max':
      if (args.length === 2) return Math.max(args[0], args[1]);
      break;
    case 'clamp':
      if (args.length === 3) return Math.min(args[2], Math.max(args[1], args[0]));
      break;
    case 'abs':
      if (args.length === 1) return Math.abs(args[0]);
      break;
    case 'sqrt':
      if (args.length === 1) return Math.sqrt(Math.max(0, args[0]));
      break;
    case 'hypot':
      if (args.length === 2) return Math.hypot(args[0], args[1]);
      break;
    case 'sin':
      if (args.length === 1) return Math.sin((args[0] * Math.PI) / 180);
      break;
    case 'cos':
      if (args.length === 1) return Math.cos((args[0] * Math.PI) / 180);
      break;
    case 'tan':
      if (args.length === 1) return Math.tan((args[0] * Math.PI) / 180);
      break;
    case 'atan2':
      if (args.length === 2) return (Math.atan2(args[0], args[1]) * 180) / Math.PI;
      break;
    case 'if':
      if (args.length === 3) return Math.abs(args[0]) > 1e-9 ? args[1] : args[2];
      break;
  }
  throw new Error(`Unknown function or arity: ${name}(${args.length} args)`);
}

export function getAutocompleteSuggestions(
  text: string,
  cursorPos: number,
  symbols: SymbolEntry[],
  limit = 8,
): { start: number; end: number; prefix: string; items: SymbolEntry[] } {
  const clamped = Math.max(0, Math.min(text.length, cursorPos));
  let start = clamped;
  while (start > 0 && /[a-zA-Z0-9_]/.test(text[start - 1])) {
    start -= 1;
  }
  let end = clamped;
  while (end < text.length && /[a-zA-Z0-9_]/.test(text[end])) {
    end += 1;
  }
  const prefix = text.slice(start, clamped);
  if (!prefix || /^\d/.test(prefix)) {
    return { start, end, prefix: '', items: [] };
  }
  const q = prefix.toLowerCase();
  const scored: Array<{ rank: number; item: SymbolEntry }> = [];
  for (const item of symbols) {
    const n = item.name.toLowerCase();
    const l = item.label.toLowerCase();
    if (n === q) scored.push({ rank: 0, item });
    else if (n.startsWith(q)) scored.push({ rank: 1, item });
    else if (n.includes(q)) scored.push({ rank: 2, item });
    else if (l.includes(q)) scored.push({ rank: 3, item });
  }
  scored.sort((a, b) => a.rank - b.rank || a.item.name.localeCompare(b.item.name));
  return {
    start,
    end,
    prefix,
    items: scored.slice(0, limit).map((s) => s.item),
  };
}

export function applyAutocomplete(
  text: string,
  start: number,
  end: number,
  item: SymbolEntry,
): { nextText: string; nextCursor: number } {
  const insertion = item.kind === 'fn' ? `${item.name}(` : item.name;
  const nextText = text.slice(0, start) + insertion + text.slice(end);
  return {
    nextText,
    nextCursor: start + insertion.length,
  };
}
