import { test } from 'node:test';
import assert from 'node:assert/strict';

const invocations = [];
globalThis.window = {
  __TAURI_INTERNALS__: {
    invoke: async (cmd, args) => {
      invocations.push({ cmd, args });
      return null;
    },
  },
};

const { createNotchDrag, mountNotchDrag } = await import('../../src/ui/notch-drag.ts');

function fixture(begin = async () => {}) {
  const finished = [];
  const notices = [];
  const errors = [];
  const captured = new Set();
  const capture = {
    setPointerCapture: id => { captured.add(id); },
    hasPointerCapture: id => captured.has(id),
    releasePointerCapture: id => { captured.delete(id); },
  };
  const drag = createNotchDrag({
    begin,
    finish: async (id, cancel) => { finished.push([id, cancel]); },
    onStart: () => {},
    onNotice: event => notices.push(event),
    onState: () => {},
    fail: error => errors.push(error),
  });
  const start = () => drag.start({ id: 7, x: 100, y: 100, capture });
  const notice = (id, active, done) => ({
    id, active, finished: done, error: null,
    placement: { edge: 'right', monitor_hint: null, offset: 0.5 },
  });
  return { drag, start, captured, finished, notices, errors, notice };
}

test('drag.cancel() releases pointer capture, finishes native drag and requests release_liquid_backdrop', async () => {
  invocations.length = 0;
  const f = fixture();
  f.start();
  const id = f.drag.sessionId;
  assert.equal(f.captured.has(7), true);

  f.drag.cancel();

  assert.equal(f.captured.size, 0);
  assert.equal(f.drag.sessionId, null);
  assert.equal(f.drag.dragging, false);
  assert.ok(invocations.some(i => i.cmd === 'release_liquid_backdrop'));

  await f.drag.settled();
  assert.deepEqual(f.finished, [[id, true]]);
});

test('onNotice with finished: true releases liquid backdrop and cleans up session', async () => {
  invocations.length = 0;
  const f = fixture();
  f.start();
  const id = f.drag.sessionId;

  f.drag.notice(f.notice(id, true, true));

  assert.equal(f.captured.size, 0);
  assert.equal(f.drag.sessionId, null);
  assert.equal(f.drag.dragging, false);
});

test('pointer capture release handles throwing elements gracefully without breaking cancel', async () => {
  invocations.length = 0;
  const finished = [];
  const drag = createNotchDrag({
    begin: async () => {},
    finish: async (id, cancel) => { finished.push([id, cancel]); },
    onStart: () => {},
    onNotice: () => {},
    onState: () => {},
    fail: () => {},
  });
  const faultyCapture = {
    setPointerCapture: () => {},
    hasPointerCapture: () => true,
    releasePointerCapture: () => { throw new Error('DOMException: invalid pointer ID'); },
  };
  drag.start({ id: 99, x: 10, y: 10, capture: faultyCapture });
  assert.equal(drag.sessionId !== null, true);

  // Must not throw despite faulty releasePointerCapture
  drag.cancel();
  assert.equal(drag.sessionId, null);
  assert.ok(invocations.some(i => i.cmd === 'release_liquid_backdrop'));
});

test('drag.cancel() invokes yield_notch_focus with forwardEscape: false', async () => {
  invocations.length = 0;
  const f = fixture();
  f.start();
  f.drag.cancel();
  assert.ok(invocations.some(i => i.cmd === 'yield_notch_focus' && i.args?.forwardEscape === false));
});

test('normal pointer release plays the snap animation while unexpected loss cancels', async () => {
  const listeners = {};
  const captured = new Set();
  const classes = new Set();
  globalThis.window.addEventListener = () => {};
  const rail = {
    offsetWidth: 64,
    addEventListener: (type, handler) => { listeners[type] = handler; },
    setPointerCapture: id => captured.add(id),
    hasPointerCapture: id => captured.has(id),
    releasePointerCapture: id => captured.delete(id),
    classList: {
      toggle: (name, value) => value ? classes.add(name) : classes.delete(name),
      add: name => classes.add(name), remove: name => classes.delete(name),
    },
  };
  const finish = [];
  const drag = mountNotchDrag(rail, () => ({ metrics: { drag_threshold: 5 } }), {
    begin: async () => {}, finish: async (id, cancel) => finish.push([id, cancel]),
    onStart: () => {}, onNotice: () => {}, fail: error => { throw error; },
  });
  const start = () => listeners.pointerdown({ button: 0, pointerId: 7, screenX: 100, screenY: 100, target: rail });
  const notice = (id, active, finished) => ({ id, active, finished, error: null,
    placement: { edge: 'right', monitor_hint: null, offset: .5 } });
  start();
  const id = drag.sessionId;
  drag.notice(notice(id, true, false));
  listeners.pointerup({ pointerId: 7 });
  listeners.lostpointercapture({ pointerId: 7, buttons: 0 });
  drag.notice(notice(id, true, true));
  assert.equal(classes.has('is-snapped'), true);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(finish.some(([itemId, cancel]) => itemId === id && cancel), false);
  classes.delete('is-snapped');
  start();
  const cancelled = drag.sessionId;
  drag.notice(notice(cancelled, true, false));
  listeners.lostpointercapture({ pointerId: 7, buttons: 1 });
  assert.equal(drag.sessionId, null);
  start();
  drag.notice(notice(drag.sessionId, false, true));
  assert.equal(classes.has('is-snapped'), false);
  drag.destroy();
});

test('mountNotchDrag Escape and PrintScreen keydown yield focus and forward Escape', async () => {
  invocations.length = 0;
  const windowListeners = [];
  globalThis.window.addEventListener = (type, handler) => {
    windowListeners.push({ type, handler });
  };
  const railListeners = [];
  const rail = {
    addEventListener: (type, handler) => { railListeners.push({ type, handler }); },
    classList: { toggle: () => {}, remove: () => {}, add: () => {} },
  };

  mountNotchDrag(rail, () => null, {
    begin: async () => {},
    finish: async () => {},
    onStart: () => {},
    onNotice: () => {},
    fail: () => {},
  });

  const keydown = windowListeners.find(l => l.type === 'keydown')?.handler;
  assert.ok(keydown, 'keydown listener must be registered on window');

  // Test Escape
  keydown({ key: 'Escape' });
  assert.ok(
    invocations.some(i => i.cmd === 'yield_notch_focus' && i.args?.forwardEscape === true),
    'Escape must invoke yield_notch_focus with forwardEscape: true'
  );

  // Test PrintScreen
  invocations.length = 0;
  keydown({ key: 'PrintScreen' });
  assert.ok(
    invocations.some(i => i.cmd === 'yield_notch_focus' && i.args?.forwardEscape === false),
    'PrintScreen must invoke yield_notch_focus with forwardEscape: false'
  );
});
