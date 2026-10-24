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
