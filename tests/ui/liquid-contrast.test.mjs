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
