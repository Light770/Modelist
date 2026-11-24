import assert from 'node:assert/strict';
import test from 'node:test';
import {
  applyAutocomplete,
  evaluateExpression,
  getAutocompleteSuggestions,
  parseInlineDefinition,
  type SymbolEntry,
} from '../expression-assist';
import {
  build2DRadialWheel,
  detectFlickGesture,
  sectorFromDelta,
} from '../radial-menu';
import {
  MODELIST_COMMANDS,
  searchCommands,
  togglePinnedCommand,
} from '../command-palette';
import {
  isDropSlotValid,
  nextRollbackStep,
  opCategory,
  targetIndexFromDropSlot,
} from '../timeline-deps';

test('expression-assist: inline definitions & mixed physical unit arithmetic', () => {
  assert.deepEqual(parseInlineDefinition('hem_depth = 35 mm'), {
    name: 'hem_depth',
    expr: '35 mm',
  });
  assert.equal(parseInlineDefinition('if(bust = waist, 1, 2)'), null);
  assert.equal(parseInlineDefinition('10 cm + 5 mm'), null);

  const syms = new Map<string, number>([
    ['bust', 920],
    ['ease_bust', 50],
  ]);

  const mixed = evaluateExpression('10 cm + 5 mm', syms, 'mm');
  assert.equal(mixed.ok, true);
  assert.ok(Math.abs((mixed.valueMm ?? 0) - 105) < 1e-6);

  const inches = evaluateExpression('2 in + 1 cm', syms, 'mm');
  assert.equal(inches.ok, true);
  assert.ok(Math.abs((inches.valueMm ?? 0) - 60.8) < 1e-6);

  // Unitless numeric expression uses active UI default unit
  const implicitCm = evaluateExpression('2.5', syms, 'cm');
  assert.equal(implicitCm.ok, true);
  assert.ok(Math.abs((implicitCm.valueMm ?? 0) - 25) < 1e-6);

  // Formula mixing POM variables (mm) & unit suffixes
  const formula = evaluateExpression('(bust + ease_bust) / 4 + 1 cm', syms, 'cm');
  assert.equal(formula.ok, true);
  assert.ok(Math.abs((formula.valueMm ?? 0) - 252.5) < 1e-6);

  // Unknown parameter detection for quick-create popover
  const unk = evaluateExpression('collar_stand + 1 cm', syms, 'mm');
  assert.equal(unk.ok, false);
  assert.deepEqual(unk.unknownVars, ['collar_stand']);
});

test('expression-assist: autocomplete ranking & token splicing', () => {
  const entries: SymbolEntry[] = [
    { name: 'waist', label: 'Waist Circumference', kind: 'pom', valueMm: 740 },
    { name: 'waist_to_hip', label: 'Waist to Hip', kind: 'pom', valueMm: 205 },
    { name: 'max', label: 'max(a, b)', kind: 'fn' },
  ];
  const text = '(wai + 20)';
  const sug = getAutocompleteSuggestions(text, 4, entries);
  assert.equal(sug.prefix, 'wai');
  assert.equal(sug.items[0].name, 'waist');

  const applied = applyAutocomplete(text, sug.start, sug.end, sug.items[0]);
  assert.equal(applied.nextText, '(waist + 20)');
  assert.equal(applied.nextCursor, 6);

  const fnSug = getAutocompleteSuggestions('ma', 2, entries);
  const fnApplied = applyAutocomplete('ma', fnSug.start, fnSug.end, fnSug.items[0]);
  assert.equal(fnApplied.nextText, 'max(');
  assert.equal(fnApplied.nextCursor, 4);
});

test('radial-menu: all 8 sectors & quick-flick thresholds', () => {
  assert.equal(sectorFromDelta(4, 5), null); // Dead zone
  assert.equal(sectorFromDelta(0, -40), 'N');
  assert.equal(sectorFromDelta(40, -40), 'NE');
  assert.equal(sectorFromDelta(50, 0), 'E');
  assert.equal(sectorFromDelta(40, 40), 'SE');
  assert.equal(sectorFromDelta(0, 50), 'S');
  assert.equal(sectorFromDelta(-40, 40), 'SW');
  assert.equal(sectorFromDelta(-50, 0), 'W');
  assert.equal(sectorFromDelta(-40, -40), 'NW');

  assert.deepEqual(detectFlickGesture(0, -60, 180, false), { type: 'repeat' });
  assert.deepEqual(detectFlickGesture(60, 0, 150, true), { type: 'commit' });
  assert.deepEqual(detectFlickGesture(-60, 0, 150, true), { type: 'cancel' });
  assert.deepEqual(detectFlickGesture(-60, 0, 150, false), { type: 'undo' });
  assert.equal(detectFlickGesture(60, 0, 600, true), null); // Too slow
  assert.equal(detectFlickGesture(10, 0, 100, true), null); // Too short

  const wheel = build2DRadialWheel('Sew Interfaces', true);
  assert.equal(wheel.E.commandId, '__commit_command__');
  assert.equal(wheel.W.commandId, '__cancel_command__');
  assert.equal(wheel.N.disabled, false);
});

test('command-palette: fuzzy search ranking & pin management', () => {
  const byShortcut = searchCommands('e', '2d', MODELIST_COMMANDS);
  assert.equal(byShortcut[0].id, 'sew_segment');

  const byWord = searchCommands('dart', '2d', MODELIST_COMMANDS);
  assert.equal(byWord[0].id, 'dart_pivot');

  const view3d = searchCommands('draft', '3d', MODELIST_COMMANDS);
  assert.equal(view3d.length, 0);

  const pinned = togglePinnedCommand(['a', 'b'], 'c');
  assert.deepEqual(pinned, ['a', 'b', 'c']);
  assert.deepEqual(togglePinnedCommand(pinned, 'b'), ['a', 'c']);
});

test('timeline-deps: drop slot validation & rollback stepping', () => {
  assert.equal(targetIndexFromDropSlot(2, 0), 0);
  assert.equal(targetIndexFromDropSlot(1, 3), 2);
  assert.equal(isDropSlotValid(2, 0, 1, 4), false); // Above min bound
  assert.equal(isDropSlotValid(2, 1, 1, 4), true);
  assert.equal(isDropSlotValid(2, 2, 1, 4), false); // No-op before self
  assert.equal(isDropSlotValid(2, 3, 1, 4), false); // No-op after self
  assert.equal(isDropSlotValid(2, 4, 1, 4), true);

  assert.equal(nextRollbackStep(null, 5, 'prev'), 4);
  assert.equal(nextRollbackStep(4, 5, 'next'), null);
  assert.equal(nextRollbackStep(null, 5, 'start'), 0);
  assert.equal(nextRollbackStep(0, 5, 'end'), null);
  assert.equal(opCategory('split_style_line'), 'cut');
  assert.equal(opCategory('replace_seam_binding'), 'seam');
});
