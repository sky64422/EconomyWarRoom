import { test } from 'node:test';
import assert from 'node:assert/strict';
import { liquidContrast, mountLiquidContrast } from '../../src/ui/liquid-contrast.ts';
const gray=value=>new Uint8ClampedArray([value,value,value,255]);
test('Liquid contrast keeps readable market ink with hysteresis',()=>{
 assert.equal(liquidContrast(gray(220)), 'light');
 assert.equal(liquidContrast(gray(35),'light'), 'dark');
 assert.equal(liquidContrast(gray(158),'light'), 'light');
 assert.equal(liquidContrast(gray(158),'dark'), 'dark');
});
test('contrast probes serialize and late results cannot repaint disabled surfaces',async t=>{
 t.mock.timers.enable({apis:['setTimeout']});
 let resolve,calls=0;
 const old={window:globalThis.window,document:globalThis.document,devicePixelRatio:globalThis.devicePixelRatio};
 globalThis.window={__TAURI_INTERNALS__:{invoke:()=>{calls++;return new Promise(done=>resolve=done);}}};
 globalThis.document={hidden:false};globalThis.devicePixelRatio=1;
 const surface=()=>({dataset:{},hidden:false,classList:{contains:()=>false},getBoundingClientRect:()=>({x:10,y:10,width:64,height:100})});
 const rail=surface(),detail=surface(),controller=mountLiquidContrast(rail,rail,detail);
 try {
  controller.setActive(true);t.mock.timers.tick(5000);assert.equal(calls,1);
  controller.setActive(false);resolve(new ArrayBuffer(0));
  for(let i=0;i<5;i++)await Promise.resolve();
  t.mock.timers.tick(5000);assert.equal(calls,1);assert.equal(rail.dataset.contrast,undefined);
 } finally {controller.setActive(false);Object.assign(globalThis,old);}
});


test('Liquid contrast switches promptly without lag when background alternates between white and black across random intervals', () => {
  // Deterministic pseudo-random sequence of short (30-150ms) and long (1000-3000ms) intervals
  const pseudoRandomIntervals = [
    { ms: 40, isWhite: true },
    { ms: 1500, isWhite: false },
    { ms: 60, isWhite: true },
    { ms: 2200, isWhite: false },
    { ms: 30, isWhite: true },
    { ms: 80, isWhite: false },
    { ms: 3000, isWhite: true },
    { ms: 120, isWhite: false },
    { ms: 1100, isWhite: true },
    { ms: 50, isWhite: false },
    { ms: 2500, isWhite: true },
    { ms: 45, isWhite: false },
    { ms: 1800, isWhite: true },
    { ms: 90, isWhite: false },
    { ms: 1300, isWhite: true },
    { ms: 35, isWhite: false },
  ];

  let currentContrast = 'dark';
  for (const { ms, isWhite } of pseudoRandomIntervals) {
    const targetContrast = isWhite ? 'light' : 'dark';
    const pixelVal = isWhite ? 245 : 15;
    const frame = gray(pixelVal);

    // Prompt response: The very first evaluation on frame change MUST match target
    currentContrast = liquidContrast(frame, currentContrast);
    assert.equal(
      currentContrast,
      targetContrast,
      `Contrast must switch to '${targetContrast}' immediately on first frame of ${isWhite ? 'white' : 'black'} (interval ${ms}ms)`
    );

    // Throughout the interval (simulating 30fps frames = ~33ms step)
    const frameCount = Math.max(1, Math.floor(ms / 33));
    for (let f = 1; f < frameCount; f++) {
      currentContrast = liquidContrast(frame, currentContrast);
      assert.equal(
        currentContrast,
        targetContrast,
        `Contrast must remain '${targetContrast}' on frame ${f} of ${isWhite ? 'white' : 'black'} (interval ${ms}ms)`
      );
    }
  }
});
