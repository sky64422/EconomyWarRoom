import { test } from 'node:test';
import assert from 'node:assert/strict';

class MockElement extends EventTarget {
  dataset = {};
  hidden = false;
  className = '';
  classes = new Set();
  classList = {
    contains: name => this.classes.has(name),
    toggle: (name, force) => {
      if (force ?? !this.classes.has(name)) this.classes.add(name);
      else this.classes.delete(name);
    },
    add: name => this.classes.add(name),
    remove: name => this.classes.delete(name),
  };
  children = [];
  style = {
    setProperty: (k, v) => { this.style[k] = v; },
    getPropertyValue: k => this.style[k] ?? '',
  };
  width = 0;
  height = 0;
  rect = { x: 10, y: 20, width: 64, height: 300 };
  offsetWidth = 0;
  offsetHeight = 0;
  offsetLeft = 0;
  offsetTop = 0;
  offsetParent = null;
  setAttribute() {}
  append(...nodes) { this.children.push(...nodes); }
  prepend(...nodes) { this.children.unshift(...nodes); }
  before(...nodes) { this.children.push(...nodes); }
  querySelector() {
    return { before: (...nodes) => this.children.push(...nodes), removeAttribute: () => {} };
  }
  querySelectorAll() { return []; }
  remove() { this.hidden = true; }
  getBoundingClientRect() { return this.rect; }
  getContext() {
    return {
      putImageData: () => {},
      drawImage: () => {},
      getImageData: () => ({ data: new Uint8ClampedArray(16) }),
      createImageData: () => ({ data: new Uint8ClampedArray(16) }),
      setTransform: () => {},
      resetTransform: () => {},
      fill: () => {},
      beginPath: () => {},
      roundRect: () => {},
    };
  }
}

const flush = async () => {
  for (let i = 0; i < 15; i++) await Promise.resolve();
};

const invocations = [];
let now = 1000;
let frameCb = null;
let customBackdropColor = null;

globalThis.matchMedia = () => ({ matches: false });
globalThis.getComputedStyle = () => ({ getPropertyValue: () => '' });
globalThis.requestAnimationFrame = cb => { frameCb = cb; return 1; };
globalThis.cancelAnimationFrame = () => {};
globalThis.devicePixelRatio = 2;
globalThis.innerWidth = 1920;
globalThis.innerHeight = 1080;

globalThis.window = Object.assign(new EventTarget(), {
  __TAURI_INTERNALS__: {
    invoke: async (cmd, args) => {
      invocations.push({ cmd, args });
      if (cmd === 'capture_liquid_backdrop') {
        if (customBackdropColor !== null) {
          const w = args.width, h = args.height;
          const pixels = new Uint8Array(w * h * 4);
          const color = customBackdropColor === 'white' ? 250 : 15;
          pixels.fill(color);
          for (let i = 3; i < pixels.length; i += 4) pixels[i] = 255;
          const buf = new ArrayBuffer(12 + pixels.length);
          const v = new DataView(buf);
          new Uint8Array(buf).set([76, 81, 48, 49]); // "LQ01"
          v.setUint32(4, w, true);
          v.setUint32(8, h, true);
          new Uint8Array(buf, 12).set(pixels);
          return buf;
        }
        return new ArrayBuffer(0);
      }
      return null;
    },
  },
  devicePixelRatio: 2,
  innerWidth: 1920,
  innerHeight: 1080,
  requestAnimationFrame: globalThis.requestAnimationFrame,
  cancelAnimationFrame: globalThis.cancelAnimationFrame,
  matchMedia: globalThis.matchMedia,
  getComputedStyle: globalThis.getComputedStyle,
  performance: { now: () => now },
});

globalThis.document = Object.assign(new EventTarget(), {
  hidden: false,
  createElement: () => new MockElement(),
  createElementNS: () => new MockElement(),
});

globalThis.MutationObserver = class {
  observe() {}
  disconnect() {}
};

globalThis.ImageData = class {
  constructor(data, width, height) {
    this.data = data;
    this.width = width;
    this.height = height;
  }
};

const { mountLiquidGlass } = await import('../../src/ui/liquid-glass.ts');

function fixture() {
  const shell = new MockElement(), rail = new MockElement(), detail = new MockElement();
  shell.dataset.theme = 'liquid';
  rail.dataset.edge = 'right';
  return {
    shell,
    rail,
    detail,
    renderer: mountLiquidGlass(shell, rail, detail),
  };
}

test('window blur releases liquid backdrop and strictly DOES NOT schedule capture_liquid_backdrop (TC-CAP-07)', async () => {
  invocations.length = 0;
  const { renderer, rail } = fixture();
  renderer.setEnabled(true);
  frameCb?.(now);
  await flush();

  // Advance time beyond STATIC_AFTER_MS so it is settled
  now += 2500;
  frameCb?.(now);
  await flush();

  // Clear startup calls
  invocations.length = 0;

  // Dispatch blur event (e.g. user hits capture hotkey Alt + ` or Snipping Tool opens)
  globalThis.window.dispatchEvent(new Event('blur'));
  await flush();

  const releasedOnBlur = invocations.some(i => i.cmd === 'release_liquid_backdrop');
  assert.equal(releasedOnBlur, true, 'blur must invoke release_liquid_backdrop immediately');

  const capturedOnBlur = invocations.some(i => i.cmd === 'capture_liquid_backdrop');
  assert.equal(capturedOnBlur, false, 'blur MUST NOT trigger capture_liquid_backdrop (would cause 0x11 screenshot exclusion)');

  renderer.destroy();
});

test('pointer interactions trigger single-shot refresh while settled', async () => {
  invocations.length = 0;
  const { renderer, rail } = fixture();
  renderer.setEnabled(true);
  frameCb?.(now);
  await flush();

  now += 2500;
  frameCb?.(now);
  await flush();
  invocations.length = 0;

  // Hovering rail
  rail.dispatchEvent(new Event('pointerenter'));
  frameCb?.(now);
  await flush();
  assert.equal(invocations.some(i => i.cmd === 'capture_liquid_backdrop'), true);

  renderer.destroy();
});

test('Liquid theme contrast responds promptly when backdrop alternates between white and black at random intervals (short and long)', async () => {
  invocations.length = 0;
  const { renderer, rail } = fixture();
  renderer.setEnabled(true);

  // Alternating sequence of pseudo-random short intervals (30-150ms) and long intervals (1000-3000ms)
  const sequence = [
    { ms: 45, color: 'white' },
    { ms: 1200, color: 'black' },
    { ms: 60, color: 'white' },
    { ms: 2500, color: 'black' },
    { ms: 35, color: 'white' },
    { ms: 90, color: 'black' },
    { ms: 1800, color: 'white' },
    { ms: 40, color: 'black' },
    { ms: 3000, color: 'white' },
    { ms: 75, color: 'black' },
    { ms: 1500, color: 'white' },
    { ms: 50, color: 'black' },
  ];

  for (const step of sequence) {
    customBackdropColor = step.color;
    // Wake up/refresh if settled
    rail.dispatchEvent(new Event('pointerenter'));

    // Advance 1 frame (~33.3ms)
    now += 33.3;
    frameCb?.(now);
    await flush();

    const expectedContrast = step.color === 'white' ? 'light' : 'dark';
    assert.equal(
      rail.dataset.contrast,
      expectedContrast,
      `Contrast must promptly become '${expectedContrast}' after switching to ${step.color} background (interval: ${step.ms}ms)`
    );

    // If interval has remaining duration, advance through it
    const remaining = step.ms - 33.3;
    if (remaining > 0) {
      now += remaining;
      frameCb?.(now);
      await flush();
      assert.equal(
        rail.dataset.contrast,
        expectedContrast,
        `Contrast must remain '${expectedContrast}' throughout the ${step.ms}ms ${step.color} interval`
      );
    }
  }

  customBackdropColor = null;
  renderer.destroy();
});
