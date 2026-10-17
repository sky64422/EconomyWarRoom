import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decodeLiquidFrame, liquidWash } from '../../src/ui/liquid-frame.ts';
function packet(w,h,pixels) {
  const b=new ArrayBuffer(12+pixels.length),v=new DataView(b);
  new Uint8Array(b).set([76,81,48,49]);v.setUint32(4,w,true);v.setUint32(8,h,true);
  new Uint8Array(b,12).set(pixels);return b;
}
test('raw Liquid frames retain RGBA bytes without a decoded image allocation',()=>{
 const b=packet(2,1,[255,0,0,255,0,128,255,255]);
 const p=decodeLiquidFrame(b,2,1);assert.equal(p.buffer,b);
 assert.deepEqual([...p],[255,0,0,255,0,128,255,255]);
});
test('stale sizes, truncated data and invalid signatures fail instead of stretching stale pixels',()=>{
 assert.throws(()=>decodeLiquidFrame(packet(2,1,[0,0,0,255]),2,1));
 assert.throws(()=>decodeLiquidFrame(packet(1,1,[0,0,0,255]),2,1));
 const b=packet(1,1,[0,0,0,255]);new Uint8Array(b)[0]=0;
 assert.throws(()=>decodeLiquidFrame(b,1,1));
 assert.throws(()=>decodeLiquidFrame(packet(0,0,[]),0,0));
});
test('bright backdrops receive a stronger neutral wash while dark backdrops retain translucency',()=>{
 const dark=new Uint8ClampedArray(256*4),bright=new Uint8ClampedArray(256*4).fill(255);
 assert.ok(liquidWash(bright)>liquidWash(dark));
 const whiteThroughGlass=255*(1-liquidWash(bright));
 const luminance=((whiteThroughGlass/255+.055)/1.055)**2.4;
 assert.ok((1.05)/(luminance+.05)>=4.5);
});
