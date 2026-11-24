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
import { normalizeWheelDeltaY, wheelZoomFactor } from '../zoom';
import {
  MAX_CAMERA_DIST,
  MIN_CAMERA_DIST,
  type OrbitCamera3D,
  applyFreeMove,
  applyOrbitDelta,
  applyPanDelta,
  applyZoomFactor,
  cameraBasis,
  cameraClipPlanes,
} from '../../sim3d/camera3d';

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

test('zoom: wheel delta normalization and pinch/wheel factor scaling', () => {
  assert.equal(normalizeWheelDeltaY({ deltaY: 100, deltaMode: 0 }), 100);
  assert.equal(normalizeWheelDeltaY({ deltaY: 3, deltaMode: 1 }), 48);
  assert.equal(normalizeWheelDeltaY({ deltaY: 1, deltaMode: 2 }), 400);

  // Negative deltaY zooms in (> 1); positive deltaY zooms out (< 1)
  assert.ok(wheelZoomFactor({ deltaY: -100, deltaMode: 0, ctrlKey: false, metaKey: false }) > 1);
  assert.ok(wheelZoomFactor({ deltaY: 100, deltaMode: 0, ctrlKey: false, metaKey: false }) < 1);
  assert.equal(wheelZoomFactor({ deltaY: 0, deltaMode: 0, ctrlKey: true, metaKey: false }), 1);

  // Trackpad pinch (ctrlKey) has higher sensitivity than mouse wheel
  const wheelIn = wheelZoomFactor({ deltaY: -20, deltaMode: 0, ctrlKey: false, metaKey: false });
  const pinchIn = wheelZoomFactor({ deltaY: -20, deltaMode: 0, ctrlKey: true, metaKey: false });
  assert.ok(pinchIn > wheelIn);

  // Clamped to [0.75, 1.35]
  assert.equal(wheelZoomFactor({ deltaY: -10000, deltaMode: 0, ctrlKey: true, metaKey: false }), 1.35);
  assert.equal(wheelZoomFactor({ deltaY: 10000, deltaMode: 0, ctrlKey: true, metaKey: false }), 0.75);
});

test('camera3d: singularity-free 360x360 orbit, 3D pan, macro/scene zoom & free move', () => {
  const dot = (a: [number, number, number], b: [number, number, number]) =>
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const norm = (a: [number, number, number]) => Math.hypot(a[0], a[1], a[2]);

  for (const pitch of [0, Math.PI * 0.5, -Math.PI * 0.5, Math.PI]) {
    const b = cameraBasis({
      yaw: 0.75,
      pitch,
      dist: 1.5,
      targetX: 0.2,
      targetY: 1.1,
      targetZ: -0.3,
    });
    assert.ok(Math.abs(norm(b.right) - 1) < 1e-6);
    assert.ok(Math.abs(norm(b.up) - 1) < 1e-6);
    assert.ok(Math.abs(norm(b.fz) - 1) < 1e-6);
    assert.ok(Math.abs(dot(b.right, b.up)) < 1e-6);
    assert.ok(Math.abs(dot(b.right, b.fz)) < 1e-6);
    assert.ok(Math.abs(dot(b.up, b.fz)) < 1e-6);
  }

  const cam: OrbitCamera3D = {
    yaw: 0,
    pitch: 0,
    dist: 1.6,
    targetX: 0,
    targetY: 1.1,
    targetZ: 0,
  };
  applyOrbitDelta(cam, -500, 800, 1);
  assert.ok(cam.pitch > Math.PI * 0.5, 'pitch must orbit freely past the top pole');
  applyOrbitDelta(cam, 0, -1600, 1);
  assert.ok(cam.pitch < -Math.PI * 0.5, 'pitch must orbit freely past the bottom pole');

  cam.yaw = 0;
  cam.pitch = 0;
  applyPanDelta(cam, 100, 50, 800);
  assert.ok(cam.targetX < 0 && cam.targetY > 1.1);

  applyZoomFactor(cam, 1000);
  assert.equal(cam.dist, MIN_CAMERA_DIST);
  assert.ok(cameraClipPlanes(cam.dist).near < cam.dist);
  applyZoomFactor(cam, 0.001);
  assert.equal(cam.dist, MAX_CAMERA_DIST);

  const beforeZ = cam.targetZ;
  applyFreeMove(cam, { forward: 1, right: 0, up: 1, sprint: true }, 0.1);
  assert.ok(cam.targetZ < beforeZ && cam.targetY > 1.1);
});
